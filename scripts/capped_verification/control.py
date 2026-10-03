#!/usr/bin/env python3
"""Fixed-unit lifecycle requests; never runs privileged commands or edits job receipts."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

ROOT = Path('/home/ran/src/rust/cathedralbevy')
CONTROL = ROOT / 'logs/capped_verification/control'
MAIN = 'cathedral-alibi-build.service'
STOP = 'cathedral-alibi-build-stop.service'
START_PATH = 'cathedral-alibi-build-start.path'
STOP_PATH = 'cathedral-alibi-build-stop.path'
CGROUP = Path('/sys/fs/cgroup/system.slice/cathedral-alibi-build.service')
PROPERTIES = ('LoadState', 'ActiveState', 'SubState', 'MainPID', 'ControlGroup',
              'Job', 'ExecMainStartTimestampMonotonic', 'ExecMainExitTimestampMonotonic',
              'ExecMainStatus', 'Result', 'InvocationID', 'Nice')
DEADLINE = None


# Reuse the worker's terminal receipt validator, including every evidence pin.
# The sibling worker is part of the reviewed tooling packet and startup pin.
_receipts_spec = importlib.util.spec_from_file_location("capped_receipts", Path(__file__).with_name("worker.py"))
receipts = importlib.util.module_from_spec(_receipts_spec)
_receipts_spec.loader.exec_module(receipts)


def command_timeout():
    remaining = 3 if DEADLINE is None else min(3, DEADLINE - time.monotonic())
    if remaining <= 0:
        raise TimeoutError('Lifecycle deadline expired; pending request retained')
    return remaining


def show(unit):
    result = subprocess.run(['/usr/bin/systemctl', 'show', unit,
                             *('--property=' + prop for prop in PROPERTIES)],
                            check=True, capture_output=True, text=True, timeout=command_timeout())
    return dict(line.split('=', 1) for line in result.stdout.splitlines() if '=' in line)


def unpopulated(directory=CGROUP):
    try:
        fields = dict(line.split() for line in (directory / 'cgroup.events').read_text().splitlines())
    except FileNotFoundError:
        return not directory.exists()
    return fields.get('populated') == '0'


def quiescent(state):
    return state.get('ActiveState') in ('inactive', 'failed') and not state.get('Job')


def waiting(state):
    return state.get('ActiveState') == 'active' and state.get('SubState') == 'waiting' and not state.get('Job')


def verify_running(state):
    if state.get('ActiveState') != 'active' or state.get('SubState') != 'running' or state.get('Job'):
        raise ValueError('Worker is not stably running')
    if state.get('ControlGroup') != '/system.slice/cathedral-alibi-build.service':
        raise ValueError('Unexpected worker cgroup')
    if (CGROUP / 'cpu.max').read_text().split() != ['40000', '100000']:
        raise ValueError('Worker does not have the exact 40% CPU quota')
    if (CGROUP / 'cpu.max.burst').read_text().strip() != '0':
        raise ValueError('Worker CPU burst is not zero')
    if int(state['MainPID']) <= 0:
        raise ValueError('Worker has no live main PID')
    # MainPID is in the host namespace; it need not exist in this client's /proc.
    # Nice is the live unit configuration. Actual startup nice is checked in the
    # current invocation's receipt, where the worker can observe its own PID.
    if state.get('Nice') != '15':
        raise ValueError('Worker unit is not configured for nice 15')


def started_receipt(state):
    invocation = state.get('InvocationID', '')
    if len(invocation) != 32 or any(c not in '0123456789abcdef' for c in invocation):
        return False
    # On systemd 249, forward -n 1 seeks to the last entry before applying
    # --grep, hiding startup once a job finishes. Reverse traversal counts the
    # matching entry instead. Output stays bounded to one receipt and the
    # existing command timeout bounds the search through this invocation.
    result = subprocess.run(['/usr/bin/journalctl', '--quiet', '--no-pager', '--reverse', '-o', 'json',
                             '--output-fields=MESSAGE,_SYSTEMD_INVOCATION_ID,_SYSTEMD_UNIT',
                             '--grep="event": "worker_started"', '-n', '1',
                             f'_SYSTEMD_INVOCATION_ID={invocation}', f'_SYSTEMD_UNIT={MAIN}'],
                            check=False, capture_output=True, text=True, timeout=command_timeout())
    if result.returncode == 1 and not result.stdout.strip() and not result.stderr.strip():
        return False  # journalctl --grep has no startup match yet.
    result.check_returncode()
    for line in result.stdout.splitlines():
        try:
            record = json.loads(line)
            if (not isinstance(record, dict)
                    or record.get('_SYSTEMD_INVOCATION_ID') != invocation
                    or record.get('_SYSTEMD_UNIT') != MAIN):
                continue
            entry = json.loads(record.get('MESSAGE', ''))
        except (ValueError, TypeError):
            continue
        if isinstance(entry, dict) and entry.get('event') == 'worker_started':
            expected_worker = hashlib.sha256((ROOT / 'scripts/capped_verification/worker.py').read_bytes()).hexdigest()
            return (entry.get('cap') == {'cgroup': '/system.slice/cathedral-alibi-build.service',
                                        'quota_usec': 40000, 'period_usec': 100000, 'burst_usec': 0}
                    and entry.get('nice') == 15
                    and entry.get('worker_sha256') == expected_worker)
    return False


def require_installed():
    for unit in (MAIN, STOP, START_PATH, STOP_PATH):
        if show(unit).get('LoadState') != 'loaded':
            raise ValueError('Permanent worker setup missing; run the reviewed install_capped.sh once as administrator')
    for unit in (START_PATH, STOP_PATH):
        if show(unit).get('ActiveState') != 'active':
            raise ValueError(f'{unit} is not active; administrator setup/recovery is required')


def safe_open(path, flags):
    return os.open(path, flags | os.O_NOFOLLOW, 0o600)


def sync_control():
    fd = os.open(CONTROL, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def begin(action):
    command_timeout()  # Do not issue a new mutation after the overall deadline.
    prior = show(STOP if action == 'stop' else MAIN)
    request = {'action': action, 'prior_start': int(prior.get('ExecMainStartTimestampMonotonic') or 0),
               'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'token': uuid.uuid4().hex}
    # Persist intent first. A timeout or client crash must not allow a late STOP
    # to race a later START. Subsequent calls reconcile this same request first.
    with os.fdopen(safe_open(CONTROL / 'request.json', os.O_WRONLY | os.O_CREAT | os.O_EXCL), 'w') as out:
        json.dump(request, out)
        out.flush()
        os.fsync(out.fileno())
    sync_control()
    with os.fdopen(safe_open(CONTROL / action, os.O_WRONLY | os.O_TRUNC), 'w') as out:
        out.write(request['token'] + '\n')
        out.flush()
        os.fsync(out.fileno())
    return request


def settle(request, deadline):
    if request['boot_id'] != Path('/proc/sys/kernel/random/boot_id').read_text().strip():
        raise ValueError('Unresolved lifecycle request predates boot; review before removing its request.json')
    while time.monotonic() < deadline:
        main = show(MAIN)
        if request['action'] == 'stop':
            broker = show(STOP)
            started = int(broker.get('ExecMainStartTimestampMonotonic') or 0)
            ended = int(broker.get('ExecMainExitTimestampMonotonic') or 0)
            if started > request['prior_start'] and ended >= started and quiescent(broker):
                if broker.get('Result') != 'success' or broker.get('ExecMainStatus') != '0':
                    raise ValueError('Stop broker failed; request retained for review')
                if quiescent(main) and unpopulated() and waiting(show(START_PATH)) and waiting(show(STOP_PATH)):
                    break
        else:
            started = int(main.get('ExecMainStartTimestampMonotonic') or 0)
            if started > request['prior_start']:
                if main.get('ActiveState') == 'active' and main.get('SubState') == 'running' and not main.get('Job'):
                    verify_running(main)
                    if started_receipt(main):
                        current = show(MAIN)
                        if current.get('InvocationID') == main.get('InvocationID'):
                            verify_running(current)
                            break
                if quiescent(main) and waiting(show(START_PATH)) and unpopulated():
                    (CONTROL / 'request.json').unlink()
                    sync_control()
                    raise ValueError('Worker refused/stopped after start; inspect journal and preserved job receipts')
        time.sleep(0.2)
    else:
        raise TimeoutError('Lifecycle request not settled within timeout; request.json retained, no subsequent action issued')
    (CONTROL / 'request.json').unlink()
    sync_control()


@contextmanager
def idle_queue():
    queue = ROOT / 'logs/capped_verification'
    if queue.resolve() != queue:
        raise ValueError('Queue path must not use symlinks')
    with os.fdopen(safe_open(queue / 'queue.lock', os.O_RDWR | os.O_CREAT), 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        for state in ('pending', 'staging', 'claimed'):
            directory = queue / state
            if directory.is_symlink() or not directory.is_dir():
                raise ValueError(f'Invalid queue directory: {directory}')
            for job in directory.iterdir():
                if job.is_symlink() or not job.is_dir():
                    raise ValueError(f'Invalid job directory: {job}')
                if state != 'claimed':
                    raise ValueError(f'Lifecycle requires idle queue; work remains: {job}')
                if receipts.terminal_receipt(ROOT, job) is None:
                    raise FileNotFoundError(f'Unresolved job requires evidence review: {job}')
        yield


def lifecycle(action):
    global DEADLINE
    deadline = DEADLINE = time.monotonic() + 40
    require_installed()
    if CONTROL.is_symlink() or CONTROL.resolve() != CONTROL:
        raise ValueError('Control directory must not use symlinks')
    with os.fdopen(safe_open(CONTROL / 'control.lock', os.O_RDWR | os.O_CREAT), 'w') as lock, idle_queue():
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        pending = CONTROL / 'request.json'
        if pending.exists():
            with os.fdopen(safe_open(pending, os.O_RDONLY)) as source:
                settle(json.load(source), deadline)
        main = show(MAIN)
        if action in ('stop', 'restart'):
            # Always require a new completed broker invocation: seeing an
            # already-inactive main alone does not acknowledge async stop.
            if not waiting(show(STOP_PATH)):
                raise ValueError('Stop path is not rearmed')
            settle(begin('stop'), deadline)
        if action in ('start', 'restart'):
            if (ROOT / 'logs/capped_verification/STOP').exists():
                raise ValueError('STOP marker remains; review and clear it explicitly before starting')
            main = show(MAIN)
            if main.get('ActiveState') == 'active':
                if not quiescent(show(STOP)) or not waiting(show(STOP_PATH)):
                    raise ValueError('Stop broker/path is not quiescent')
                verify_running(main)
                if not started_receipt(main):
                    raise ValueError('No worker startup receipt for current invocation')
                return
            if not (quiescent(main) and unpopulated() and waiting(show(START_PATH))
                    and quiescent(show(STOP)) and waiting(show(STOP_PATH))):
                raise ValueError('Worker or lifecycle paths are not quiescent; no start request issued')
            settle(begin('start'), deadline)


def record_abort(job_id, authorization_path, authorization_sha256):
    """Record audited interruption only; does not stop/start or fabricate result."""
    from contextlib import ExitStack
    global DEADLINE
    DEADLINE = time.monotonic() + 40
    if not isinstance(job_id, str) or not receipts.NAME_PATTERN.fullmatch(job_id):
        raise ValueError('Invalid aborted job identity')
    queue = ROOT / 'logs/capped_verification'
    if (queue.resolve() != queue or queue.is_symlink()
            or CONTROL.resolve() != CONTROL or CONTROL.is_symlink()):
        raise ValueError('Queue/control paths must not use symlinks')
    with ExitStack() as stack:
        for lock_path in (CONTROL / 'control.lock', queue / 'queue.lock'):
            handle = stack.enter_context(os.fdopen(safe_open(lock_path, os.O_RDWR | os.O_CREAT), 'w'))
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if os.path.lexists(CONTROL / 'request.json'):
            raise ValueError('Unresolved lifecycle request; no abort recorded')
        for name in ('pending', 'staging', 'claimed'):
            directory = queue / name
            if directory.is_symlink() or not directory.is_dir():
                raise ValueError('Invalid queue directory')
            if name != 'claimed' and any(directory.iterdir()):
                raise ValueError('Queued work prevents abort recording')
        unresolved = []
        for index, prior in enumerate((queue / 'claimed').iterdir()):
            command_timeout()
            if index >= 4096:
                raise ValueError('Claimed-history inspection bound exceeded; review required')
            if receipts.terminal_receipt(ROOT, prior) is None:
                unresolved.append(prior.name)
        if unresolved != [job_id]:
            raise ValueError('Abort requires exactly the specified unresolved claimed job')
        pin = {'path': authorization_path, 'sha256': authorization_sha256}
        authorization = receipts.evidence_json(ROOT, pin)
        receipts.validate_interruption(ROOT, job_id, authorization, receipt=False)
        value = {key: authorization[key] for key in receipts.INTERRUPTION_FIELDS}
        value.update(audit_authorization=pin, recorded_utc=receipts.utc())
        receipts.validate_interruption(ROOT, job_id, value)
        # Verify the preserved job identity without rewriting any launch receipt.
        job = queue / 'claimed' / job_id
        original_job = receipts.read_json(job / 'job.json')
        receipts.validate_job(original_job, ROOT)
        if original_job['run_name'] != job_id:
            raise ValueError('Claimed job identity mismatch')
        state = show(MAIN)
        if not quiescent(state) or not unpopulated():
            raise ValueError('Abort recording requires stopped main service and empty cgroup')
        if state.get('InvocationID') not in ('', None, value['stopped_invocation_id']):
            raise ValueError('Different service invocation; review again')
        if receipts.terminal_receipt(ROOT, job) is not None:
            raise ValueError('Job gained a terminal receipt during review')
        command_timeout()
        receipts.write_new(job / 'abort.json', value)
        receipts.terminal_receipt(ROOT, job)
        return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('start', 'stop', 'restart', 'status', 'record-abort'))
    parser.add_argument('--job-id')
    parser.add_argument('--audit-authorization')
    parser.add_argument('--audit-sha256')
    args = parser.parse_args()
    if args.action == 'record-abort':
        if not all((args.job_id, args.audit_authorization, args.audit_sha256)):
            parser.error('record-abort requires --job-id, --audit-authorization and --audit-sha256')
        record_abort(args.job_id, args.audit_authorization, args.audit_sha256)
        print('record-abort: verified interruption recorded; no runner exit code, no restart')
    elif any((args.job_id, args.audit_authorization, args.audit_sha256)):
        parser.error('audit arguments are only valid with record-abort')
    elif args.action == 'status':
        print(json.dumps({unit: show(unit) for unit in (MAIN, STOP, START_PATH, STOP_PATH)}, indent=2))
    else:
        lifecycle(args.action)
        print(f'{args.action}: verified')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise SystemExit(f'REFUSED: {error}')
