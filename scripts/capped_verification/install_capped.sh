#!/bin/sh
# One-time administrator installation of the reviewed, fixed worker lifecycle.
set -eu
[ "$#" -eq 0 ] || { echo 'No arguments accepted' >&2; exit 2; }
exec /usr/bin/python3 -I -B - <<'INSTALL_PY'
import hashlib
import fcntl
import json
import os
from pathlib import Path
import pwd
import stat
import subprocess
import tempfile

ROOT = Path('/home/ran/src/rust/cathedralbevy')
SOURCE = ROOT / 'scripts/capped_verification/systemd'
DEST = Path('/etc/systemd/system')
QUEUE = ROOT / 'logs/capped_verification'
MAIN = 'cathedral-alibi-build.service'
EXPECTED = {'cathedral-alibi-build-start.path': '9a069eb9f206d1d5bfeb2e167442cdd26202355de9532f75023f39962ff98f18', 'cathedral-alibi-build-stop.path': '964c26c2e5c9615130ae837e26cab32646b6e36af7c0fb301b2959ccfa16e3bd', 'cathedral-alibi-build-stop.service': 'b65dd65f4cb76e230bc632f53e57635fd9f12696d1a0d37157798f66750b660f', 'cathedral-alibi-build.service': 'a690e4cd1724faf115278c3bab92a2ad84ec2a8e7c91e3e65e5f2ad42920d429'}


def run(*args):
    return subprocess.run(args, check=True, text=True, capture_output=True, timeout=30).stdout


def plain(path, directory=False, optional=False):
    for parent in reversed(path.parents):
        if parent.is_symlink():
            raise ValueError(f'Symlink parent refused: {parent}')
    try:
        info = path.lstat()
    except FileNotFoundError:
        if optional:
            return
        raise
    if not (stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)):
        raise ValueError(f'Unexpected file type: {path}')


def show(unit):
    return dict(line.split('=', 1) for line in run('/usr/bin/systemctl', 'show', unit,
                '-p', 'ActiveState', '-p', 'Job', '-p', 'FragmentPath', '-p', 'DropInPaths').splitlines())


def install():
    if os.geteuid() != 0:
        raise ValueError('Run this reviewed installer once with sudo sh scripts/capped_verification/install_capped.sh')
    account = pwd.getpwnam('ran')
    plain(DEST, directory=True)
    # Do not execute repository Python as root or accept caller-selected paths.
    contents = {}
    for name, expected in EXPECTED.items():
        source = SOURCE / name
        plain(source)
        data = source.read_bytes()
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f'Reviewed unit checksum mismatch: {name}')
        target = DEST / name
        plain(target, optional=True)
        if target.exists() and target.read_bytes() != data:
            raise ValueError(f'Refusing to replace differing existing unit: {target}')
        state = show(name)
        if state.get('ActiveState') not in ('inactive', 'failed') or state.get('Job'):
            raise ValueError(f'Existing unit is active or has a job: {name}')
        if state.get('FragmentPath') not in ('', str(target)) or state.get('DropInPaths'):
            raise ValueError(f'Unexpected existing fragment/drop-in: {name}')
        for base in (Path('/run/systemd/system'), Path('/etc/systemd/system')):
            if (base / (name + '.d')).exists():
                raise ValueError(f'Existing unit override directory: {name}')
        contents[name] = data
    group = Path('/sys/fs/cgroup/system.slice') / MAIN
    if group.exists():
        fields = dict(line.split() for line in (group / 'cgroup.events').read_text().splitlines())
        if fields.get('populated') != '0':
            raise ValueError('Old worker cgroup still populated')
    plain(QUEUE, directory=True)
    queue_fd = os.open(QUEUE / 'queue.lock', os.O_RDONLY | os.O_NOFOLLOW)
    fcntl.flock(queue_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    # Held through enable/start; worker cannot claim and submitters cannot enqueue.
    if (QUEUE / 'STOP').exists():
        raise ValueError('STOP marker present; review before installation')
    for state in ('pending', 'staging', 'claimed'):
        directory = QUEUE / state
        plain(directory, directory=True)
        for job in directory.iterdir():
            plain(job, directory=True)
            if state != 'claimed':
                raise ValueError(f'Queue must be idle before installation: {job}')
            result = job / 'result.json'
            plain(result)
            data = json.loads(result.read_text())
            if (data.get('status') != 'completed' or data.get('job_id') != job.name
                    or type(data.get('exit_code')) is not int):
                raise ValueError(f'Unresolved or invalid prior claim: {job}')
    control = QUEUE / 'control'
    plain(control, directory=True, optional=True)
    if (control / 'request.json').exists():
        raise ValueError('Unresolved lifecycle request; inspect before installation')
    for name in ('start', 'stop', 'control.lock'):
        plain(control / name, optional=True)
    # Validate precisely the pinned bytes in a root-owned temporary directory;
    # never reread mutable repository units after their checksum validation.
    with tempfile.TemporaryDirectory(prefix='.cathedral-units-', dir=DEST) as temporary:
        staged = Path(temporary)
        for name, data in contents.items():
            target = staged / name
            target.write_bytes(data)
            target.chmod(0o644)
        run('/usr/bin/systemd-analyze', 'verify', *(str(staged / n) for n in EXPECTED))
        for name in contents:
            os.replace(staged / name, DEST / name)
    # Repository paths belong to ran. Bootstrap as ran, never root chown/chmod
    # through mutable directories. This is fixed inline code, not repo Python.
    bootstrap = '''import os
from pathlib import Path
control = Path('/home/ran/src/rust/cathedralbevy/logs/capped_verification/control')
if control.resolve() != control:
    raise SystemExit('Control directory symlink refused')
control.mkdir(mode=0o700, exist_ok=True)
for name in ('start', 'stop', 'control.lock'):
    fd = os.open(control / name, os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    os.close(fd)
'''
    subprocess.run(['/usr/bin/python3', '-I', '-B', '-c', bootstrap], check=True,
                   user=account.pw_uid, group=account.pw_gid, extra_groups=[], timeout=10)
    for path in (control, *(control / name for name in ('start', 'stop', 'control.lock'))):
        plain(path, directory=path == control)
        if path.lstat().st_uid != account.pw_uid:
            raise ValueError(f'Control path must be owned by ran: {path}')
    run('/usr/bin/systemctl', 'daemon-reload')
    run('/usr/bin/systemctl', 'enable', '--now',
        'cathedral-alibi-build-start.path', 'cathedral-alibi-build-stop.path', MAIN)
    print('Installed and enabled fixed capped worker and lifecycle paths. Verify with control.py status/start.')


try:
    install()
except (OSError, ValueError, subprocess.SubprocessError) as error:
    raise SystemExit(f'REFUSED: {error}')
INSTALL_PY
