"""Audited interruption recovery: temporary files/fake systemd only."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


worker, control = module('worker'), module('control')
INACTIVE = {'ActiveState': 'inactive', 'SubState': 'dead', 'Job': '', 'MainPID': '0', 'InvocationID': ''}


class AbortRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.job_id = 'interrupted-1'
        self.runner = worker.EVIDENCE + '/m3_fake_abort/owner/run_capped.py'
        source = self.root / self.runner
        source.parent.mkdir(parents=True)
        source.write_text("print('fake')\n")
        for name in ('run_tests.py', 'run_build.py', 'reviewed-sources.json'):
            source.with_name(name).write_text('{}\n')
        (self.root / worker.EVIDENCE / 'component_input_sources.py').write_text('# fake\n')
        self.job_value = {'runner': self.runner, 'run_name': self.job_id}
        worker.submit(self.root, self.job_value)
        self.queue = worker.setup(self.root)
        self.job = worker.claim_next(self.queue)
        # Existing launch/start/output receipts remain byte-for-byte immutable.
        for name, value in [('start.json', {'original': 'start'}), ('launched.json', {'pid': 99})]:
            worker.write_new(self.job / name, value)
        (self.job / 'output.log').write_text('actual incomplete output\n')
        self.directory = self.queue / 'control'
        self.directory.mkdir()
        self.stop = self.root / worker.EVIDENCE / 'm3_fake_abort/owner/after-stop.json'
        self.stop_value = {'status': 'unit_stopped_cgroup_empty', 'job_id': self.job_id,
                           'prior_invocation': 'a' * 32, 'cgroup_empty': True, 'state': INACTIVE}
        self.stop.write_text(json.dumps(self.stop_value))
        self.audit = self.stop.with_name('abort-authorization.json')
        self.authorization = {'schema_version': 1, 'status': 'verified_interrupted', 'job_id': self.job_id,
                              'runner_exit_code': None, 'stopped_invocation_id': 'a' * 32,
                              'cgroup_empty': True, 'source_freeze_released': True,
                              'stop_evidence': self.pin(self.stop)}
        self.audit.write_text(json.dumps(self.authorization))
        for key, value in [('ROOT', self.root), ('CONTROL', self.directory), ('DEADLINE', None)]:
            mock = patch.object(control, key, value)
            mock.start()
            self.addCleanup(mock.stop)

    def pin(self, path):
        return {'path': str(path.relative_to(self.root)), 'sha256': worker.sha(path)}

    def receipt(self):
        return dict(self.authorization, audit_authorization=self.pin(self.audit), recorded_utc='2026-10-02T00:00:00+00:00')

    def record(self, **kwargs):
        with patch.object(control, 'show', return_value=kwargs.get('state', INACTIVE)), \
                patch.object(control, 'unpopulated', return_value=kwargs.get('empty', True)), \
                patch.object(control, 'begin') as begin, patch.object(control, 'settle') as settle:
            value = control.record_abort(self.job_id, self.pin(self.audit)['path'], self.pin(self.audit)['sha256'])
            begin.assert_not_called()
            settle.assert_not_called()
            return value

    def publish(self):
        worker.write_new(self.job / 'abort.json', self.receipt())

    def test_record_preserves_every_old_receipt_and_uses_null_exit(self):
        before = {p.name: p.read_bytes() for p in self.job.iterdir()}
        value = self.record()
        self.assertEqual(value['status'], 'verified_interrupted')
        self.assertIsNone(value['runner_exit_code'])
        self.assertNotIn('exit_code', value)
        self.assertFalse((self.job / 'result.json').exists())
        self.assertEqual({name: (self.job / name).read_bytes() for name in before}, before)
        self.assertEqual(worker.terminal_receipt(self.root, self.job), value)
        with control.idle_queue():
            pass
        self.assertIsNone(worker.claim_next(self.queue))

    def test_record_holds_both_locks_during_live_check(self):
        def show(_):
            for path in (self.directory / 'control.lock', self.queue / 'queue.lock'):
                with worker.lock(path, nonblocking=True):
                    self.fail('a recording lock was not held')
        def checked_show(unit):
            for path in (self.directory / 'control.lock', self.queue / 'queue.lock'):
                with self.assertRaises(BlockingIOError), worker.lock(path, nonblocking=True):
                    pass
            return INACTIVE
        with patch.object(control, 'show', side_effect=checked_show), patch.object(control, 'unpopulated', return_value=True):
            control.record_abort(self.job_id, self.pin(self.audit)['path'], self.pin(self.audit)['sha256'])

    def test_record_is_once_only_and_name_is_never_reusable(self):
        self.record()
        with self.assertRaises(ValueError):
            self.record()
        with self.assertRaises(ValueError):
            worker.submit(self.root, self.job_value)
        worker.submit(self.root, dict(self.job_value, run_name='fresh-2'))
        self.assertEqual(worker.claim_next(self.queue).name, 'fresh-2')
        self.assertTrue(self.job.is_dir())

    def test_record_refuses_running_populated_or_different_invocation(self):
        for state, empty in [(dict(INACTIVE, ActiveState='active'), True), (INACTIVE, False),
                             (dict(INACTIVE, Job='123'), True), (dict(INACTIVE, InvocationID='b' * 32), True)]:
            with self.subTest(state=state, empty=empty), self.assertRaises(ValueError):
                self.record(state=state, empty=empty)
            self.assertFalse((self.job / 'abort.json').exists())

    def test_record_refuses_pending_staging_and_second_unresolved_claim(self):
        for folder in ('pending', 'staging', 'claimed'):
            directory = self.queue / folder / 'other'
            directory.mkdir()
            with self.subTest(folder=folder), self.assertRaises(ValueError):
                self.record()
            self.assertFalse((self.job / 'abort.json').exists())
            directory.rmdir()

    def test_record_refuses_unsettled_lifecycle_request(self):
        (self.directory / 'request.json').write_text('{}')
        with self.assertRaises(ValueError):
            self.record()
        self.assertFalse((self.job / 'abort.json').exists())
        self.assertTrue((self.directory / 'request.json').exists())

    def test_completed_receipts_keep_existing_semantics(self):
        completed = {'status': 'completed', 'job_id': self.job_id, 'exit_code': 7}
        worker.write_new(self.job / 'result.json', completed)
        self.assertEqual(worker.terminal_receipt(self.root, self.job), completed)
        with control.idle_queue():
            pass
        with self.assertRaises(ValueError):
            self.record()
        self.assertFalse((self.job / 'abort.json').exists())

    def test_completed_and_aborted_never_coexist_even_with_broken_symlink(self):
        self.publish()
        (self.job / 'result.json').symlink_to('missing-result')
        with self.assertRaises(ValueError):
            worker.terminal_receipt(self.root, self.job)
        with self.assertRaises(ValueError):
            worker.claim_next(self.queue)
        with self.assertRaises(ValueError), control.idle_queue():
            pass

    def test_abort_schema_rejects_missing_false_wrong_and_invented_exit(self):
        original = self.receipt()
        invalid = [dict(original, schema_version=True), dict(original, schema_version=2),
                   dict(original, status='completed'), dict(original, job_id='other'),
                   dict(original, runner_exit_code=0), dict(original, runner_exit_code=False),
                   dict(original, exit_code=137), dict(original, cgroup_empty=False),
                   dict(original, source_freeze_released=1), dict(original, stopped_invocation_id='x' * 32),
                   dict(original, recorded_utc=''), dict(original, extra='no')]
        invalid += [{k: v for k, v in original.items() if k != removed} for removed in original]
        for value in invalid:
            with self.subTest(value=value), self.assertRaises((ValueError, OSError)):
                worker.validate_interruption(self.root, self.job_id, value)

    def test_audit_authorization_must_agree_with_every_interruption_field(self):
        receipt = self.receipt()
        altered = dict(self.authorization, stopped_invocation_id='b' * 32)
        self.audit.write_text(json.dumps(altered))
        receipt['audit_authorization'] = self.pin(self.audit)
        with self.assertRaises(ValueError):
            worker.validate_interruption(self.root, self.job_id, receipt)

    def test_tampered_audit_or_stop_blocks_worker_and_lifecycle(self):
        self.publish()
        for evidence in (self.audit, self.stop):
            original = evidence.read_bytes()
            evidence.write_bytes(original + b' ')
            with self.subTest(evidence=evidence), self.assertRaises(ValueError):
                worker.claim_next(self.queue)
            with self.assertRaises(ValueError), control.idle_queue():
                pass
            evidence.write_bytes(original)
        self.assertIsNone(worker.claim_next(self.queue))

    def test_stop_evidence_requires_matching_job_invocation_inactive_nojob_and_empty(self):
        for changes in [{'status': 'stopping'}, {'job_id': 'other'}, {'prior_invocation': 'b' * 32},
                        {'cgroup_empty': False}, {'state': dict(INACTIVE, ActiveState='active')},
                        {'state': dict(INACTIVE, Job='8')}, {'state': None}]:
            with self.subTest(changes=changes):
                self.stop.write_text(json.dumps(dict(self.stop_value, **changes)))
                auth = dict(self.authorization, stop_evidence=self.pin(self.stop))
                with self.assertRaises(ValueError):
                    worker.validate_interruption(self.root, self.job_id, auth, receipt=False)

    def test_evidence_paths_reject_raw_dot_escape_absolute_and_symlinks(self):
        pin = self.pin(self.audit)
        for path in ['/tmp/outside', '../outside', './' + pin['path'], pin['path'].replace('/owner/', '/owner/./'),
                     pin['path'].replace('/owner/', '/owner/../owner/'), pin['path'].replace('/owner/', '//owner/'), '']:
            with self.subTest(path=path), self.assertRaises(ValueError):
                worker.evidence_json(self.root, dict(pin, path=path))
        link = self.audit.with_name('symlink.json')
        link.symlink_to(self.audit)
        with self.assertRaises(ValueError):
            worker.evidence_json(self.root, {'path': str(link.relative_to(self.root)), 'sha256': pin['sha256']})
        directory = self.root / 'evidence-link'
        directory.symlink_to(self.audit.parent, target_is_directory=True)
        with self.assertRaises(ValueError):
            worker.evidence_json(self.root, {'path': 'evidence-link/' + self.audit.name, 'sha256': pin['sha256']})

    def test_bounded_unique_json_evidence_and_symlink_abort_required(self):
        for raw in [b'{"job_id":1,"job_id":2}', b'[]', b'{', b' ' * (worker.MAX_JSON + 1)]:
            self.audit.write_bytes(raw)
            with self.subTest(raw=raw[:40]), self.assertRaises(ValueError):
                worker.evidence_json(self.root, self.pin(self.audit))
        (self.job / 'abort.json').symlink_to(self.audit)
        with self.assertRaises(ValueError):
            worker.terminal_receipt(self.root, self.job)

    def test_malformed_original_job_cannot_gain_abort(self):
        (self.job / 'job.json').write_text(json.dumps(dict(self.job_value, run_name='other')))
        with self.assertRaises(ValueError):
            self.record()
        self.assertFalse((self.job / 'abort.json').exists())

    def test_completed_during_final_live_check_preserves_result_and_refuses_abort(self):
        def complete(_):
            worker.write_new(self.job / 'result.json', {'status': 'completed', 'job_id': self.job_id, 'exit_code': 1})
            return INACTIVE
        with patch.object(control, 'show', side_effect=complete), patch.object(control, 'unpopulated', return_value=True):
            with self.assertRaises(ValueError):
                control.record_abort(self.job_id, self.pin(self.audit)['path'], self.pin(self.audit)['sha256'])
        self.assertFalse((self.job / 'abort.json').exists())
        self.assertTrue((self.job / 'result.json').exists())

    def test_existing_lifecycle_and_queue_locks_refuse_without_writes(self):
        for path in (self.directory / 'control.lock', self.queue / 'queue.lock'):
            with worker.lock(path, nonblocking=True), self.assertRaises(BlockingIOError):
                self.record()
            self.assertFalse((self.job / 'abort.json').exists())


if __name__ == '__main__':
    unittest.main()
