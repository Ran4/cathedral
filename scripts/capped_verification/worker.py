#!/usr/bin/env python3
"""Durable, sequential execution of reviewed evidence runners inside one CPU cap."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ("features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/"
            "implementation/plan/evidence")
RUNNER_PATTERN = re.compile(re.escape(EVIDENCE) + r"/m(?:[0-9]|1[0-9])_[a-z0-9_]+/owner/run_capped\.py\Z")
NAME_PATTERN = re.compile(r"[a-zA-Z0-9][a-zA-Z0-9_.-]{0,79}\Z")
MAX_JSON = 32768
MAX_PENDING = 256


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_new(path, value):
    # O_EXCL is also the permanent, atomic claim primitive. Never overwrite evidence.
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "w") as target:
        json.dump(value, target, indent=2, sort_keys=True)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())
    sync_directory(path.parent)


def read_json(path):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"Not a regular JSON file: {path}")
    with path.open("rb") as source:
        raw = source.read(MAX_JSON + 1)
    if len(raw) > MAX_JSON:
        raise ValueError("Oversized JSON")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("Duplicate JSON key")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique)


def regular_beneath(root, relative):
    path = root / relative
    # Reject all symlink components, including symlinks that point inside the repo.
    current = root
    for part in Path(relative).parts:
        if part in ("..", "."):
            raise ValueError("Path traversal")
        current = current / part
        if current.is_symlink():
            raise ValueError(f"Symlink refused: {current}")
    if not path.resolve().is_relative_to(root.resolve()) or not stat.S_ISREG(path.stat().st_mode):
        raise ValueError("Runner/helper must be a regular file inside the repository")
    return path


def validate_job(job, root):
    if not isinstance(job, dict) or set(job) != {"runner", "run_name"}:
        raise ValueError("Job requires exactly runner and run_name")
    if not isinstance(job["run_name"], str) or not NAME_PATTERN.fullmatch(job["run_name"]):
        raise ValueError("Unsafe run name (maximum 80 ASCII filename characters)")
    if not isinstance(job["runner"], str) or not RUNNER_PATTERN.fullmatch(job["runner"]):
        raise ValueError("Runner must be evidence/m0..m19_<name>/owner/run_capped.py")
    return regular_beneath(root, job["runner"])


def identities(job, root):
    runner = validate_job(job, root)
    relatives = [str(runner.relative_to(root))]
    relatives += [str((runner.parent / name).relative_to(root)) for name in
                  ("run_tests.py", "run_build.py", "reviewed-sources.json")]
    relatives.append(EVIDENCE + "/component_input_sources.py")
    return {relative: sha(regular_beneath(root, relative)) for relative in relatives}


def parse_cap(quota_text, burst_text):
    pieces = quota_text.split()
    if len(pieces) != 2 or any(not p.isascii() or not p.isdecimal() for p in pieces):
        raise ValueError("Missing/invalid finite cpu.max")
    quota, period = map(int, pieces)
    if quota <= 0 or period <= 0 or quota * 5 > period * 2:
        raise ValueError("Kernel CPU quota exceeds 40% of one core")
    if burst_text.strip() != "0":
        raise ValueError("Kernel CPU burst must be zero")
    return {"quota_usec": quota, "period_usec": period, "burst_usec": 0}


def require_cap():
    groups = [line[3:] for line in Path("/proc/self/cgroup").read_text().splitlines()
              if line.startswith("0::")]
    if len(groups) != 1 or not groups[0].startswith("/") or ".." in Path(groups[0]).parts:
        raise ValueError("Cannot identify unified kernel cgroup")
    directory = Path("/sys/fs/cgroup") / groups[0].lstrip("/")
    # Missing burst control fails closed; no unsupported fallback to an assumed zero.
    result = parse_cap((directory / "cpu.max").read_text(), (directory / "cpu.max.burst").read_text())
    return {"cgroup": groups[0], **result}


def setup(root):
    queue = root / "logs/capped_verification"
    for directory in (queue, queue / "staging", queue / "pending", queue / "claimed"):
        directory.mkdir(parents=True, exist_ok=True)
        if directory.is_symlink() or directory.resolve() != directory.absolute():
            raise ValueError("Queue directories must not use symlinks")
        sync_directory(directory.parent)
    return queue


@contextmanager
def lock(path, nonblocking=False):
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX | (fcntl.LOCK_NB if nonblocking else 0))
        yield


def submit(root, job):
    hashes = identities(job, root)
    queue = setup(root)
    with lock(queue / "queue.lock"):
        if (queue / "STOP").exists():
            raise ValueError("Stop marker present; refusing submission")
        name = job["run_name"]
        if any((queue / state / name).exists() for state in ("staging", "pending", "claimed")):
            raise ValueError("Run name already reserved; never reuse it")
        if sum(1 for _ in (queue / "pending").iterdir()) >= MAX_PENDING:
            raise ValueError("Queue full")
        staging = queue / "staging" / name
        staging.mkdir(mode=0o700)
        sync_directory(staging.parent)
        write_new(staging / "job.json", job)
        write_new(staging / "review.json", {"submitted_utc": utc(), "job": job,
                  "helper_sha256": hashes, "worker_sha256": sha(Path(__file__))})
        staging.rename(queue / "pending" / name)
        sync_directory(queue / "pending")
        sync_directory(queue / "staging")
    return queue / "pending" / name


def evidence_json(root, pin):
    """One bounded immutable repo-relative JSON identity; reject raw traversal."""
    if (not isinstance(pin, dict) or set(pin) != {"path", "sha256"}
            or not isinstance(pin["path"], str) or not 0 < len(pin["path"]) <= 4096
            or pin["path"].startswith("/")
            or any(part in ("", ".", "..") for part in pin["path"].split("/"))
            or not isinstance(pin["sha256"], str)
            or not re.fullmatch(r"[0-9a-f]{64}", pin["sha256"])):
        raise ValueError("Invalid evidence path/hash pin")
    path = regular_beneath(root, pin["path"])
    # Open without following the final component, validate the actual descriptor,
    # and hash exactly the same bounded bytes that are parsed below.
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError("Evidence must be a regular file")
        raw = source.read(MAX_JSON + 1)
    if len(raw) > MAX_JSON or hashlib.sha256(raw).hexdigest() != pin["sha256"]:
        raise ValueError("Evidence bytes do not match their sealed hash")
    def unique(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError("Duplicate evidence JSON key")
            value[key] = item
        return value
    value = json.loads(raw, object_pairs_hook=unique)
    if not isinstance(value, dict):
        raise ValueError("Evidence JSON must be an object")
    return value


INTERRUPTION_FIELDS = {"schema_version", "status", "job_id", "runner_exit_code",
                       "stopped_invocation_id", "cgroup_empty", "source_freeze_released", "stop_evidence"}


def validate_interruption(root, job_id, value, *, receipt=True):
    """Shared worker/controller authority check; never invent a runner exit."""
    if not isinstance(value, dict) or not INTERRUPTION_FIELDS <= value.keys():
        raise ValueError("Incomplete interruption evidence")
    if (type(value["schema_version"]) is not int or value["schema_version"] != 1
            or value["status"] != "verified_interrupted" or value["job_id"] != job_id
            or value["runner_exit_code"] is not None or "exit_code" in value
            or value["cgroup_empty"] is not True or value["source_freeze_released"] is not True
            or not isinstance(value["stopped_invocation_id"], str)
            or not re.fullmatch(r"[0-9a-f]{32}", value["stopped_invocation_id"])):
        raise ValueError("Invalid interruption identity/status/authorization")
    if receipt:
        if set(value) != INTERRUPTION_FIELDS | {"audit_authorization", "recorded_utc"}:
            raise ValueError("Invalid abort receipt fields")
        if not isinstance(value["recorded_utc"], str) or not value["recorded_utc"]:
            raise ValueError("Missing abort recording time")
        authorization = evidence_json(root, value["audit_authorization"])
        validate_interruption(root, job_id, authorization, receipt=False)
        if any(authorization[key] != value[key] for key in INTERRUPTION_FIELDS):
            raise ValueError("Abort differs from sealed independent audit authorization")
    stop = evidence_json(root, value["stop_evidence"])
    state = stop.get("state")
    if (stop.get("status") != "unit_stopped_cgroup_empty" or stop.get("job_id") != job_id
            or stop.get("prior_invocation") != value["stopped_invocation_id"]
            or stop.get("cgroup_empty") is not True or not isinstance(state, dict)
            or state.get("ActiveState") not in ("inactive", "failed") or state.get("Job") != ""):
        raise ValueError("Stop evidence does not prove the exact stopped empty invocation")
    return value


def terminal_receipt(root, directory):
    """None means unresolved. Completed and independently audited aborts differ.

    This is the single authority used before worker claims and lifecycle actions.
    An aborted directory remains permanently claimed and is never rerunnable.
    """
    if directory.is_symlink() or not directory.is_dir() or directory.resolve() != directory.absolute():
        raise ValueError("Invalid claimed job directory")
    result_path, abort_path = directory / "result.json", directory / "abort.json"
    result_present, abort_present = os.path.lexists(result_path), os.path.lexists(abort_path)
    if result_present and abort_present:
        raise ValueError("Completed and aborted receipts cannot coexist")
    if result_present:
        result = read_json(result_path)
        if (not isinstance(result, dict) or result.get("status") != "completed"
                or result.get("job_id") != directory.name or type(result.get("exit_code")) is not int):
            raise ValueError("Invalid completed result requires manual review")
        return result
    if abort_present:
        value = read_json(abort_path)
        validate_interruption(root, directory.name, value)
        original_job = read_json(directory / "job.json")
        validate_job(original_job, root)
        if original_job["run_name"] != directory.name:
            raise ValueError("Aborted claimed job identity mismatch")
        return value
    return None


def claim_next(queue):
    with lock(queue / "queue.lock"):
        # Refuse ambiguity before considering any fresh work, including after restart.
        for prior in (queue / "claimed").iterdir():
            if terminal_receipt(queue.parents[1], prior) is None:
                raise ValueError(f"Unfinished/invalid claimed job requires manual review: {prior}")
        candidates = sorted((queue / "pending").iterdir(), key=lambda p: p.name)
        for candidate in candidates:
            if candidate.is_symlink() or not candidate.is_dir() or not NAME_PATTERN.fullmatch(candidate.name):
                raise ValueError("Invalid pending queue entry")
            if (candidate / "claim.json").exists():
                raise ValueError(f"Interrupted pending claim requires manual review: {candidate}")
        if not candidates:
            return None
        selected = candidates[0]
        # This marker is fsynced BEFORE moving. A crash cannot turn claimed work back
        # into runnable work even if a directory rename is lost during recovery.
        write_new(selected / "claim.json", {"claimed_utc": utc(), "worker_pid": os.getpid()})
        destination = queue / "claimed" / selected.name
        if destination.exists():
            raise ValueError("Duplicate claimed job")
        selected.rename(destination)
        sync_directory(destination.parent)
        sync_directory(queue / "pending")
        return destination


def should_stop(queue):
    # An empty claim poll is only a snapshot. Submission may have won the lock
    # before STOP was written, so decide drain completion under that same lock.
    with lock(queue / "queue.lock"):
        return (queue / "STOP").exists() and not any((queue / "pending").iterdir())


def execute(root, directory, worker_hash, cap_reader=require_cap):
    base = {"job_id": directory.name, "worker_sha256": worker_hash}
    try:
        job = read_json(directory / "job.json")
        runner = validate_job(job, root)
        if job["run_name"] != directory.name:
            raise ValueError("Queue identity differs from run name")
        review = read_json(directory / "review.json")
        current = identities(job, root)
        base.update(job=job, helper_sha256=current)
        if review["job"] != job or review["helper_sha256"] != current:
            raise ValueError("Reviewed job/runner/helpers changed since submission")
        if review["worker_sha256"] != worker_hash or sha(Path(__file__)) != worker_hash:
            raise ValueError("Worker changed; review and restart required")
        cap = cap_reader()
        base["cap"] = cap
        command = [sys.executable, "-I", "-B", str(runner), "--run-name", job["run_name"]]
        write_new(directory / "start.json", {**base, "utc": utc(), "command": command})
        with (directory / "output.log").open("xb") as output:
            sync_directory(directory)
            process = subprocess.Popen(command, cwd=root, stdin=subprocess.DEVNULL,
                                       stdout=output, stderr=subprocess.STDOUT)
            write_new(directory / "launched.json", {"utc": utc(), "pid": process.pid})
            code = process.wait()
            output.flush()
            os.fsync(output.fileno())
        write_new(directory / "result.json", {**base, "utc": utc(), "status": "completed",
                  "exit_code": code, "output_sha256": sha(directory / "output.log")})
        return code
    except Exception as error:
        # A launch receipt without a final result is deliberately left ambiguous.
        # Never write a completed record after an error while a child may be alive.
        write_new(directory / "refused.json", {**base, "utc": utc(), "error": str(error)})
        raise


def serve(root):
    queue = setup(root)
    with lock(queue / "worker.lock", nonblocking=True):
        worker_hash = sha(Path(__file__))
        print(json.dumps({"event": "worker_started", "utc": utc(), "cap": require_cap(),
                          "worker_sha256": worker_hash, "nice": os.getpriority(os.PRIO_PROCESS, 0)}), flush=True)
        while True:
            directory = claim_next(queue)
            if directory is not None:
                code = execute(root, directory, worker_hash)
                print(json.dumps({"event": "job_finished", "job_id": directory.name,
                                  "exit_code": code, "utc": utc()}), flush=True)
                continue
            # STOP drains all queued work; submission is refused while it exists.
            if should_stop(queue):
                print(json.dumps({"event": "worker_stopped", "utc": utc()}), flush=True)
                return
            time.sleep(2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    enqueue = commands.add_parser("submit", help="Only after reviewing and freezing sources/helpers")
    enqueue.add_argument("--runner", required=True)
    enqueue.add_argument("--run-name", required=True)
    commands.add_parser("serve")
    commands.add_parser("stop", help="Drain submitted jobs, then exit (does not kill active work)")
    args = parser.parse_args()
    if args.action == "submit":
        print(submit(ROOT, {"runner": args.runner, "run_name": args.run_name}))
    elif args.action == "serve":
        serve(ROOT)
    else:
        queue = setup(ROOT)
        with lock(queue / "queue.lock"):
            write_new(queue / "STOP", {"utc": utc()})


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"REFUSED: {error}", file=sys.stderr, flush=True)
        raise SystemExit(2)
