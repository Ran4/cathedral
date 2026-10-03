"""Stdlib tests: temporary fake runners only; never invokes Cargo or systemd."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("worker", Path(__file__).with_name("worker.py"))
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.relative = worker.EVIDENCE + "/m3_fake_admission/owner/run_capped.py"
        self.runner = self.root / self.relative
        self.runner.parent.mkdir(parents=True)
        self.runner.write_text("print('fake runner executed')\n")
        for name in ("run_tests.py", "run_build.py", "reviewed-sources.json"):
            self.runner.with_name(name).write_text("{}\n")
        (self.root / worker.EVIDENCE / "component_input_sources.py").write_text("# fake\n")
        self.job = {"runner": self.relative, "run_name": "20260926-test"}
        self.worker_hash = worker.sha(Path(worker.__file__))

    def cap(self):
        return {"cgroup": "/fake-test-only", **worker.parse_cap("40000 100000", "0\n")}

    def submitted(self):
        worker.submit(self.root, self.job)
        return worker.setup(self.root)

    def test_startup_receipt_observes_worker_own_nice(self):
        import contextlib
        import io
        from unittest.mock import patch
        output = io.StringIO()
        with patch.object(worker, 'require_cap', return_value=self.cap()), \
                patch.object(worker, 'claim_next', return_value=None), \
                patch.object(worker, 'should_stop', return_value=True), \
                patch.object(worker.os, 'getpriority', return_value=15) as priority, \
                contextlib.redirect_stdout(output):
            worker.serve(self.root)
        receipt = json.loads(output.getvalue().splitlines()[0])
        self.assertEqual(receipt['event'], 'worker_started')
        self.assertEqual(receipt['nice'], 15)
        self.assertEqual(receipt['worker_sha256'], self.worker_hash)
        priority.assert_called_once_with(worker.os.PRIO_PROCESS, 0)

    def test_exact_cap_and_lower_cap(self):
        self.assertEqual(worker.parse_cap("40000 100000\n", "0")['quota_usec'], 40000)
        worker.parse_cap("1 100000", "0")

    def test_invalid_caps(self):
        for quota in ("max 100000", "40001 100000", "0 100000", "4 0", "-1 10", "4", "4 10 0"):
            with self.subTest(quota=quota), self.assertRaises(ValueError):
                worker.parse_cap(quota, "0")
        for burst in ("1", "", "-1", "max"):
            with self.subTest(burst=burst), self.assertRaises(ValueError):
                worker.parse_cap("4 10", burst)

    def test_schema_and_paths(self):
        worker.validate_job(self.job, self.root)
        invalid = [dict(self.job, command="cargo build"), dict(self.job, run_name="../bad"),
                   dict(self.job, run_name="-bad"), dict(self.job, run_name="x" * 81),
                   dict(self.job, runner="/bin/sh"), dict(self.job, runner="scripts/run_capped.py"),
                   dict(self.job, runner=self.relative.replace("m3_", "m20_")),
                   dict(self.job, runner=self.relative.replace("m3_fake_admission", "../m3_fake_admission")),
                   dict(self.job, runner=self.relative.replace("run_capped.py", "run_build.py"))]
        for job in invalid:
            with self.subTest(job=job), self.assertRaises(ValueError):
                worker.validate_job(job, self.root)

    def test_later_milestone_accepted_without_worker_restart(self):
        later = self.relative.replace("m3_", "m19_")
        destination = self.root / later
        destination.parent.mkdir(parents=True)
        destination.write_text("pass")
        self.assertEqual(worker.validate_job(dict(self.job, runner=later), self.root), destination)

    def test_symlink_runner_and_helper_refused(self):
        self.runner.unlink()
        fake = self.root / "fake.py"
        fake.write_text("print('no')")
        self.runner.symlink_to(fake)
        with self.assertRaises(ValueError):
            worker.submit(self.root, self.job)
        self.runner.unlink()
        self.runner.write_text("pass")
        helper = self.runner.with_name("run_tests.py")
        helper.unlink()
        helper.symlink_to(fake)
        with self.assertRaises(ValueError):
            worker.submit(self.root, self.job)

    def test_queue_execution_receipts_and_no_reuse(self):
        queue = self.submitted()
        with self.assertRaises(ValueError):
            worker.submit(self.root, self.job)
        claimed = worker.claim_next(queue)
        self.assertTrue((claimed / "claim.json").exists())
        self.assertFalse((queue / "pending" / self.job["run_name"]).exists())
        self.assertEqual(worker.execute(self.root, claimed, self.worker_hash, self.cap), 0)
        result = worker.read_json(claimed / "result.json")
        self.assertEqual(result["job"], self.job)
        self.assertEqual(result["worker_sha256"], self.worker_hash)
        self.assertEqual(len(result["helper_sha256"]), 5)
        self.assertIn("fake runner executed", (claimed / "output.log").read_text())
        self.assertIsNone(worker.claim_next(queue))
        with self.assertRaises(ValueError):
            worker.submit(self.root, self.job)

    def test_ambiguous_claim_blocks_later_work(self):
        queue = self.submitted()
        claimed = worker.claim_next(queue)
        worker.submit(self.root, dict(self.job, run_name="20260926-next"))
        with self.assertRaises(ValueError):
            worker.claim_next(queue)
        self.assertFalse((claimed / "output.log").exists())

    def test_claim_marker_before_rename_never_reexecutes(self):
        queue = self.submitted()
        directory = queue / "pending" / self.job["run_name"]
        worker.write_new(directory / "claim.json", {"crash": "before rename"})
        with self.assertRaises(ValueError):
            worker.claim_next(queue)
        self.assertFalse((directory / "output.log").exists())

    def test_later_pending_claim_blocks_earlier_fresh_job(self):
        queue = worker.setup(self.root)
        worker.submit(self.root, dict(self.job, run_name="z-interrupted"))
        interrupted = queue / "pending/z-interrupted"
        worker.write_new(interrupted / "claim.json", {"crash": "before rename"})
        worker.submit(self.root, dict(self.job, run_name="a-fresh"))
        with self.assertRaises(ValueError):
            worker.claim_next(queue)
        self.assertTrue((queue / "pending/a-fresh/job.json").exists())
        self.assertEqual(list((queue / "claimed").iterdir()), [])

    def test_stop_rechecks_work_accepted_after_empty_poll(self):
        queue = worker.setup(self.root)
        self.assertIsNone(worker.claim_next(queue))
        # Deterministically reproduce empty poll -> accepted submit -> STOP.
        worker.submit(self.root, self.job)
        with worker.lock(queue / "queue.lock"):
            worker.write_new(queue / "STOP", {})
        self.assertFalse(worker.should_stop(queue))
        claimed = worker.claim_next(queue)
        self.assertEqual(worker.execute(self.root, claimed, self.worker_hash, self.cap), 0)
        self.assertIsNone(worker.claim_next(queue))
        self.assertTrue(worker.should_stop(queue))

    def test_partial_result_blocks_later_work(self):
        queue = self.submitted()
        claimed = worker.claim_next(queue)
        (claimed / "result.json").write_text("{")
        with self.assertRaises(ValueError):
            worker.claim_next(queue)

    def test_changed_helper_refused_before_launch(self):
        queue = self.submitted()
        self.runner.with_name("run_tests.py").write_text("# changed after review")
        claimed = worker.claim_next(queue)
        with self.assertRaises(ValueError):
            worker.execute(self.root, claimed, self.worker_hash, self.cap)
        self.assertTrue((claimed / "refused.json").exists())
        self.assertFalse((claimed / "output.log").exists())

    def test_missing_cap_refused_before_launch(self):
        queue = self.submitted()
        claimed = worker.claim_next(queue)
        def missing():
            raise FileNotFoundError("cpu.max.burst absent")
        with self.assertRaises(FileNotFoundError):
            worker.execute(self.root, claimed, self.worker_hash, missing)
        self.assertFalse((claimed / "output.log").exists())

    def test_failure_is_completed_and_next_job_can_run(self):
        self.runner.write_text("raise SystemExit(7)\n")
        queue = self.submitted()
        claimed = worker.claim_next(queue)
        self.assertEqual(worker.execute(self.root, claimed, self.worker_hash, self.cap), 7)
        worker.submit(self.root, dict(self.job, run_name="20260926-next"))
        self.assertEqual(worker.claim_next(queue).name, "20260926-next")

    def test_stop_refuses_submission(self):
        queue = worker.setup(self.root)
        worker.write_new(queue / "STOP", {})
        with self.assertRaises(ValueError):
            worker.submit(self.root, self.job)

    def test_single_worker_lock(self):
        queue = worker.setup(self.root)
        with worker.lock(queue / "worker.lock", nonblocking=True):
            with self.assertRaises(BlockingIOError):
                with worker.lock(queue / "worker.lock", nonblocking=True):
                    self.fail("Second lock admitted")

    def test_malformed_json_refused(self):
        queue = self.submitted()
        claimed = worker.claim_next(queue)
        (claimed / "job.json").write_text('{"runner":"a","runner":"b"}')
        with self.assertRaises(ValueError):
            worker.execute(self.root, claimed, self.worker_hash, self.cap)
        self.assertFalse((claimed / "output.log").exists())


if __name__ == "__main__":
    unittest.main()
