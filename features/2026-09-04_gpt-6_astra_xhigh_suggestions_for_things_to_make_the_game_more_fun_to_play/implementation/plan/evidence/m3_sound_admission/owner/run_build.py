#!/usr/bin/env python3
"""One production build after source/build ownership is explicitly ceded."""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("capture_runner", HERE / "run_tests.py")
common = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(common)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-name", required=True)
    parser.add_argument("--after-freeze", action="store_true")
    args = parser.parse_args()
    if not args.after_freeze or not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]*", args.run_name):
        parser.error("Requires --after-freeze authorization and a unique plain run name")
    env, removed = common.environment()
    module = common.source_module()
    evidence = HERE / "evidence" / args.run_name
    evidence.mkdir(parents=True, exist_ok=False)
    before = module.sources()
    common.write(evidence / "sources-before.json", before)
    toolchain = {}
    for name, metadata_command in (
        ("rustc", ["/usr/bin/nice", "-n", "15", "/home/ran/.cargo/bin/rustc", "-Vv"]),
        ("cargo", ["/usr/bin/nice", "-n", "15", "/home/ran/.cargo/bin/cargo", "-V"]),
        ("head", ["/usr/bin/git", "rev-parse", "HEAD"]),
    ):
        result = subprocess.run(metadata_command, cwd=common.ROOT, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        toolchain[name] = {"command": metadata_command, "exit_code": result.returncode, "output": result.stdout}
        if result.returncode:
            common.write(evidence / "preflight-failure.json", toolchain)
            return result.returncode
    command = ["/usr/bin/nice", "-n", "15", "/home/ran/.cargo/bin/cargo", "build",
               "-p", "cathedralbevy", "--bin", "cathedralbevy", "--locked", "--offline", "-j", "1"]
    raw = evidence / "production-build.log"
    start = {
        "event": "START", "utc": common.utc(), "command": command,
        "cwd": str(common.ROOT), "raw_log": str(raw),
        "source_scope": module.SOURCE_SCOPE,
        "toolchain": toolchain,
        "source_map_sha256": common.sha(evidence / "sources-before.json"),
        "helper_sha256": common.sha(Path(__file__)),
        "shared_runner_sha256": common.sha(HERE / "run_tests.py"),
        "enumerator_sha256": common.sha(common.ENUMERATOR),
        "removed_environment_value_sha256": removed,
        "environment": {key: env.get(key) for key in sorted(common.CLEAR | {
            "HOME", "PATH", "CATHEDRAL_HEADLESS", "CATHEDRAL_FAKE_BACKEND",
            "PYTHONDONTWRITEBYTECODE", "CARGO_TERM_COLOR", "RUST_TEST_THREADS",
        })},
    }
    common.write(evidence / "start.json", start)
    print(json.dumps(start, sort_keys=True), flush=True)
    started = time.monotonic()
    code = None
    launch_error = None
    with raw.open("xb") as output:
        try:
            process = subprocess.Popen(command, cwd=common.ROOT, env=env,
                                       stdout=output, stderr=subprocess.STDOUT)
        except OSError as error:
            launch_error = str(error)
        else:
            common.write(evidence / "launched.json", {"utc": common.utc(), "pid": process.pid})
            code = process.wait()
    after = module.sources()
    common.write(evidence / "sources-after.json", after)
    compressed = evidence / "production-build.log.gz"
    common.archive(raw, compressed)
    executable = common.ROOT / "target/debug/cathedralbevy"
    result = {
        "event": "END", "utc": common.utc(), "cargo_exit_code": code,
        "launch_error": launch_error, "wall_seconds": time.monotonic() - started,
        "raw_log": str(raw), "raw_sha256": common.sha(raw), "archive_sha256": common.sha(compressed),
        "source_map_sha256": common.sha(evidence / "sources-before.json"),
        "after_source_map_sha256": common.sha(evidence / "sources-after.json"),
        "sources_unchanged": before == after,
        "executable": str(executable),
        "executable_sha256": common.sha(executable) if code == 0 and executable.is_file() else None,
        "executable_was_run": False,
    }
    common.write(evidence / "result.json", result)
    print(json.dumps(result, sort_keys=True), flush=True)
    if code != 0 or before != after or result["executable_sha256"] is None:
        return code if code is not None and code > 0 else 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
