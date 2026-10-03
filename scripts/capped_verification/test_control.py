"""Lifecycle tests with fake state only: never starts/stops systemd or Rust."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('control', Path(__file__).with_name('control.py'))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)

INACTIVE = {'ActiveState': 'inactive', 'SubState': 'dead', 'Job': ''}
WAITING = {'ActiveState': 'active', 'SubState': 'waiting', 'Job': ''}
ACTIVE = {'ActiveState': 'active', 'SubState': 'running', 'Job': '', 'MainPID': '42',
          'ControlGroup': '/system.slice/cathedral-alibi-build.service',
          'ExecMainStartTimestampMonotonic': '11', 'InvocationID': 'a' * 32, 'Nice': '15'}


class ControlTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        worker = self.root / 'scripts/capped_verification/worker.py'
        worker.parent.mkdir(parents=True)
        worker.write_text('test worker identity')
        self.worker_hash = hashlib.sha256(worker.read_bytes()).hexdigest()
        self.queue = self.root / 'logs/capped_verification'
        self.directory = self.queue / 'control'
        self.directory.mkdir(parents=True)
        for state in ('pending', 'staging', 'claimed'):
            (self.queue / state).mkdir()
        for marker in ('start', 'stop'):
            (self.directory / marker).touch()
        for key, value in [('ROOT', self.root), ('CONTROL', self.directory), ('DEADLINE', None)]:
            mock = patch.object(control, key, value)
            mock.start()
            self.addCleanup(mock.stop)

    def request(self, action='stop'):
        request = {'action': action, 'prior_start': 10, 'token': 'token',
                   'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip()}
        (self.directory / 'request.json').write_text(json.dumps(request))
        return request

    def startup_entry(self):
        return {'event': 'worker_started', 'nice': 15, 'worker_sha256': self.worker_hash, 'cap': {
            'cgroup': '/system.slice/cathedral-alibi-build.service', 'quota_usec': 40000,
            'period_usec': 100000, 'burst_usec': 0}}

    def journal_record(self, entry, **fields):
        return json.dumps(dict({'_SYSTEMD_INVOCATION_ID': ACTIVE['InvocationID'],
                                '_SYSTEMD_UNIT': control.MAIN, 'MESSAGE': json.dumps(entry)}, **fields))

    def test_no_ack_from_old_broker_or_already_inactive_main(self):
        request = self.request()
        old = dict(INACTIVE, ExecMainStartTimestampMonotonic='10', ExecMainExitTimestampMonotonic='10',
                   Result='success', ExecMainStatus='0')
        with patch.object(control, 'show', side_effect=lambda u: old if u == control.STOP else INACTIVE), \
                patch.object(control.time, 'sleep'), patch.object(control, 'unpopulated', return_value=True):
            with self.assertRaises(TimeoutError):
                control.settle(request, time.monotonic() + 0.01)
        self.assertTrue((self.directory / 'request.json').exists())

    def test_stop_needs_completed_broker_empty_group_and_rearmed_paths(self):
        request = self.request()
        broker = dict(INACTIVE, ExecMainStartTimestampMonotonic='11', ExecMainExitTimestampMonotonic='12',
                      Result='success', ExecMainStatus='0')
        states = {control.MAIN: INACTIVE, control.STOP: broker,
                  control.START_PATH: WAITING, control.STOP_PATH: WAITING}
        with patch.object(control, 'show', side_effect=states.__getitem__), \
                patch.object(control, 'unpopulated', side_effect=[False, True]), \
                patch.object(control.time, 'sleep'):
            control.settle(request, time.monotonic() + 1)
        self.assertFalse((self.directory / 'request.json').exists())

    def test_start_waits_past_active_with_pending_job_and_requires_startup_receipt(self):
        request = self.request('start')
        states = [dict(ACTIVE, Job='123'), ACTIVE, ACTIVE, ACTIVE]
        with patch.object(control, 'show', side_effect=states), \
                patch.object(control, 'verify_running') as verify, \
                patch.object(control, 'started_receipt', side_effect=[False, True]), \
                patch.object(control.time, 'sleep'):
            control.settle(request, time.monotonic() + 1)
        self.assertEqual(verify.call_count, 3)
        self.assertFalse((self.directory / 'request.json').exists())

    def test_start_refusal_is_reported_without_retry(self):
        request = self.request('start')
        states = {control.MAIN: dict(INACTIVE, ExecMainStartTimestampMonotonic='11'), control.START_PATH: WAITING}
        with patch.object(control, 'show', side_effect=states.__getitem__), \
                patch.object(control, 'unpopulated', return_value=True):
            with self.assertRaisesRegex(ValueError, 'refused/stopped'):
                control.settle(request, time.monotonic() + 1)
        self.assertFalse((self.directory / 'request.json').exists())

    def test_crash_between_intent_and_marker_retains_intent(self):
        original = control.safe_open
        def fail_marker(path, flags):
            if path == self.directory / 'stop':
                raise OSError('simulated client crash')
            return original(path, flags)
        with patch.object(control, 'show', return_value=INACTIVE), patch.object(control, 'safe_open', side_effect=fail_marker):
            with self.assertRaises(OSError):
                control.begin('stop')
        self.assertTrue((self.directory / 'request.json').exists())
        self.assertEqual((self.directory / 'stop').read_text(), '')

    def test_idle_queue_refuses_pending_and_interrupted_claims(self):
        (self.queue / 'pending/job').mkdir()
        with self.assertRaises(ValueError), control.idle_queue():
            pass
        (self.queue / 'pending/job').rmdir()
        job = self.queue / 'claimed/job'
        job.mkdir()
        with self.assertRaises(FileNotFoundError), control.idle_queue():
            pass
        (job / 'result.json').write_text(json.dumps({'status': 'completed', 'job_id': 'job', 'exit_code': 1}))
        with control.idle_queue():
            with open(self.queue / 'queue.lock') as other:
                with self.assertRaises(BlockingIOError):
                    control.fcntl.flock(other, control.fcntl.LOCK_EX | control.fcntl.LOCK_NB)

    def test_stop_marker_is_never_removed(self):
        (self.queue / 'STOP').write_text('preserve')
        with patch.object(control, 'require_installed'), patch.object(control, 'show', return_value=INACTIVE):
            with self.assertRaisesRegex(ValueError, 'STOP marker remains'):
                control.lifecycle('start')
        self.assertEqual((self.queue / 'STOP').read_text(), 'preserve')

    def test_exact_kernel_cap_and_nice_checks(self):
        group = self.root / 'cgroup'
        group.mkdir()
        (group / 'cpu.max').write_text('40000 100000\n')
        (group / 'cpu.max.burst').write_text('1\n')
        with patch.object(control, 'CGROUP', group):
            with self.assertRaisesRegex(ValueError, 'burst'):
                control.verify_running(ACTIVE)
            (group / 'cpu.max.burst').write_text('0\n')
            with self.assertRaisesRegex(ValueError, 'nice'):
                control.verify_running(dict(ACTIVE, Nice='0'))
            with patch.object(control.os, 'getpriority', side_effect=ProcessLookupError):
                control.verify_running(ACTIVE)  # Host PID inaccessible to client.
            (group / 'cpu.max').write_text('40001 100000\n')
            with self.assertRaisesRegex(ValueError, 'quota'):
                control.verify_running(ACTIVE)

    def test_deadline_prevents_new_request(self):
        with patch.object(control, 'DEADLINE', time.monotonic() - 1):
            with self.assertRaises(TimeoutError):
                control.begin('stop')
        self.assertFalse((self.directory / 'request.json').exists())

    def test_startup_receipt_after_completed_jobs_uses_reverse_matching_limit(self):
        # Observed systemd 249 behavior for an invocation containing startup
        # followed by job_finished: forward --grep/-n1 returns no match;
        # --reverse returns the startup. Preserve that query distinction.
        def journal_query(args, **kwargs):
            self.assertIn('--grep="event": "worker_started"', args)
            self.assertEqual(args[args.index('-n') + 1], '1')
            self.assertEqual(args[args.index('-o') + 1], 'json')
            self.assertIn('_SYSTEMD_INVOCATION_ID=' + ACTIVE['InvocationID'], args)
            self.assertIn('_SYSTEMD_UNIT=' + control.MAIN, args)
            self.assertGreater(kwargs['timeout'], 0)
            self.assertLessEqual(kwargs['timeout'], 3)
            if '--reverse' not in args:
                return control.subprocess.CompletedProcess(args, 1, stdout='', stderr='')
            return control.subprocess.CompletedProcess(args, 0,
                stdout=self.journal_record(self.startup_entry()), stderr='')
        with patch.object(control.subprocess, 'run', side_effect=journal_query):
            self.assertTrue(control.started_receipt(ACTIVE))

    def test_receipt_rejects_missing_or_wrong_journal_identity(self):
        for fields in ({'_SYSTEMD_INVOCATION_ID': 'b' * 32}, {'_SYSTEMD_INVOCATION_ID': None},
                       {'_SYSTEMD_UNIT': 'other.service'}, {'_SYSTEMD_UNIT': None}):
            with self.subTest(fields=fields):
                result = control.subprocess.CompletedProcess([], 0,
                    stdout=self.journal_record(self.startup_entry(), **fields), stderr='')
                with patch.object(control.subprocess, 'run', return_value=result):
                    self.assertFalse(control.started_receipt(ACTIVE))

    def test_receipt_requires_valid_current_invocation_before_query(self):
        for invocation in (None, '', 'b' * 31, 'g' * 32):
            with self.subTest(invocation=invocation):
                state = dict(ACTIVE)
                if invocation is None:
                    del state['InvocationID']
                else:
                    state['InvocationID'] = invocation
                with patch.object(control.subprocess, 'run') as run:
                    self.assertFalse(control.started_receipt(state))
                run.assert_not_called()

    def test_receipt_rejects_absent_startup_or_malformed_message(self):
        for output in ('', 'not json', '[]', self.journal_record({'event': 'job_finished', 'exit_code': 0}),
                       self.journal_record({}, MESSAGE=None), self.journal_record({}, MESSAGE='not json')):
            with self.subTest(output=output):
                result = control.subprocess.CompletedProcess([], 0, stdout=output, stderr='')
                with patch.object(control.subprocess, 'run', return_value=result):
                    self.assertFalse(control.started_receipt(ACTIVE))

    def test_show_uses_systemd_compatible_property_arguments(self):
        result = control.subprocess.CompletedProcess([], 0, stdout='LoadState=loaded\nNice=15\n', stderr='')
        with patch.object(control.subprocess, 'run', return_value=result) as run:
            self.assertEqual(control.show(control.MAIN), {'LoadState': 'loaded', 'Nice': '15'})
        args = run.call_args.args[0]
        self.assertIn('--property=LoadState', args)
        self.assertIn('--property=Nice', args)
        self.assertFalse(any(arg.startswith('-p=') for arg in args))

    def test_receipt_rejects_wrong_missing_nice_and_stale_worker(self):
        entry = self.startup_entry()
        for change in ({'nice': 0}, {'nice': None}, {'worker_sha256': 'stale'}, {'worker_sha256': None}):
            with self.subTest(change=change):
                result = control.subprocess.CompletedProcess([], 0,
                    stdout=self.journal_record(dict(entry, **change)), stderr='')
                with patch.object(control.subprocess, 'run', return_value=result):
                    self.assertFalse(control.started_receipt(ACTIVE))

    def test_receipt_rejects_wrong_or_missing_startup_cap(self):
        entry = self.startup_entry()
        for change in ({'quota_usec': 40001}, {'period_usec': 99999}, {'burst_usec': 1}, {'cgroup': '/other'}):
            with self.subTest(change=change):
                entry['cap'] = dict(self.startup_entry()['cap'], **change)
                result = control.subprocess.CompletedProcess([], 0, stdout=self.journal_record(entry), stderr='')
                with patch.object(control.subprocess, 'run', return_value=result):
                    self.assertFalse(control.started_receipt(ACTIVE))
        del entry['cap']
        result = control.subprocess.CompletedProcess([], 0, stdout=self.journal_record(entry), stderr='')
        with patch.object(control.subprocess, 'run', return_value=result):
            self.assertFalse(control.started_receipt(ACTIVE))

    def test_journal_no_match_waits_but_real_errors_fail(self):
        for result, expected in ((control.subprocess.CompletedProcess([], 1, stdout='', stderr=''), False),
                                 (control.subprocess.CompletedProcess([], 1, stdout='', stderr='denied'), None)):
            with patch.object(control.subprocess, 'run', return_value=result):
                if expected is False:
                    self.assertFalse(control.started_receipt(ACTIVE))
                else:
                    with self.assertRaises(control.subprocess.CalledProcessError):
                        control.started_receipt(ACTIVE)

    def test_installer_pins_exact_fixed_unit_bytes(self):
        folder = Path(__file__).parent
        shell = (folder / 'install_capped.sh').read_text()
        import ast
        code = shell.split("<<'INSTALL_PY'\n", 1)[1].rsplit('\nINSTALL_PY', 1)[0]
        compile(code, 'install_capped.sh inline Python', 'exec')
        expected = ast.literal_eval(next(line[11:] for line in shell.splitlines() if line.startswith('EXPECTED = ')))
        self.assertEqual(len(expected), 4)
        for name, checksum in expected.items():
            self.assertEqual(hashlib.sha256((folder / 'systemd' / name).read_bytes()).hexdigest(), checksum)
        stop = (folder / 'systemd/cathedral-alibi-build-stop.service').read_text()
        self.assertIn('ExecStart=/usr/bin/systemctl --no-block stop cathedral-alibi-build.service\n', stop)
        self.assertNotIn('ExecStartPre', stop)
        for action in ('start', 'stop'):
            unit = (folder / f'systemd/cathedral-alibi-build-{action}.path').read_text()
            self.assertIn(f'/logs/capped_verification/control/{action}\n', unit)
            self.assertIn('PathChanged=', unit)
            self.assertNotIn('PathExists=', unit)


if __name__ == '__main__':
    unittest.main()
