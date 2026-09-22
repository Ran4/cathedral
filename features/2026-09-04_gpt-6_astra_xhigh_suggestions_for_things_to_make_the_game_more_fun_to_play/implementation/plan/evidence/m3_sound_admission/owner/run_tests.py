#!/usr/bin/env python3
"""Run only after root cedes the source/build freeze; never applies the patch.

Preparation of this file is not an execution record. START records are created
only by actual invocation, immediately before the corresponding Cargo launch.
"""
from __future__ import annotations

import argparse
import datetime
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = Path("/home/ran/src/rust/cathedralbevy")
ENUMERATOR = ROOT / (
    "features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/"
    "implementation/plan/evidence/component_input_sources.py"
)
PREFIX = ["/usr/bin/nice", "-n", "15", "/home/ran/.cargo/bin/cargo", "test",
          "--locked", "--offline", "-j", "1"]
SUITES = {
    "sim_sound_admission": ["-p", "cathedral-sim", "--lib", "sounds::admission::tests"],
    "sim_sound_legacy": ["-p", "cathedral-sim", "--lib", "sounds::tests"],
    "installed_startup": ["-p", "cathedralbevy", "--bin", "cathedralbevy", "installed_recipe::startup::tests"],
    "local_engine_startup": ["-p", "cathedralbevy", "--bin", "cathedralbevy", "smart_actors::local_engine::startup_tests"],
}
CLEAR = {
    "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "CARGO_BUILD_TARGET_DIR",
    "RUSTC", "RUSTDOC", "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_BUILD_RUSTFLAGS", "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTDOC",
    "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "LDFLAGS", "CARGO_BUILD_JOBS", "CARGO_MAKEFLAGS", "MAKEFLAGS",
    "LD_PRELOAD", "LD_AUDIT", "GLIBC_TUNABLES",
}


def utc() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def write(path: Path, value: object) -> None:
    # A reused evidence filename is a mistake, never an invitation to overwrite.
    with path.open("x") as target:
        json.dump(value, target, sort_keys=True, indent=2)
        target.write("\n")


def environment() -> tuple[dict[str, str], dict[str, str]]:
    env = dict(os.environ)
    if env.get("HOME") != "/home/ran":
        raise SystemExit("Unexpected HOME: refuse to change the root run's default Cargo home")
    removed = CLEAR | {key for key in env if key.startswith(("CARGO_TARGET_", "CARGO_PROFILE_", "MALLOC_"))}
    # Record override identity without dumping inherited variable values.
    inherited = {key: hashlib.sha256(env[key].encode()).hexdigest()
                 for key in sorted(removed) if key in env}
    for key in removed:
        env.pop(key, None)
    env.update(
        PATH="/usr/bin:/bin:/home/ran/.local/bin:/home/ran/.cargo/bin",
        CATHEDRAL_HEADLESS="1", CATHEDRAL_FAKE_BACKEND="1",
        PYTHONDONTWRITEBYTECODE="1", CARGO_TERM_COLOR="never",
        RUST_TEST_THREADS="1",
        GLIBC_TUNABLES="glibc.malloc.hugetlb=0",
    )
    return env, inherited


def source_module():
    spec = importlib.util.spec_from_file_location("component_input_sources", ENUMERATOR)
    if spec is None or spec.loader is None:
        raise RuntimeError("Cannot load shared source/input enumerator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if module.ROOT.resolve() != ROOT:
        raise RuntimeError("Enumerator points outside the intended repository")
    return module


def archive(raw: Path, destination: Path) -> None:
    with raw.open("rb") as source, destination.open("xb") as file:
        with gzip.GzipFile(filename="", mode="wb", fileobj=file, mtime=0) as target:
            while chunk := source.read(1024 * 1024):
                target.write(chunk)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-name", required=True, help="Unique evidence directory name")
    parser.add_argument("--after-freeze", action="store_true",
                        help="Required only after root has explicitly ceded the source/build freeze")
    parser.add_argument("--suite", choices=SUITES, action="append",
                        help="Repeat to select suites; default is all four sequentially")
    args = parser.parse_args()
    if not args.after_freeze:
        parser.error("Do not execute until root cedes the freeze; then supply --after-freeze")
    if not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]*", args.run_name):
        parser.error("Run name must be a plain filename component")
    selected = args.suite or list(SUITES)
    if len(selected) != len(set(selected)):
        parser.error("A suite may run only once in one evidence record")
    if not (ROOT / "crates/cathedral-backends/src/world_data/capture.rs").is_file():
        raise SystemExit("Source-capture patch is not applied; refusing a zero-test pre-patch run")
    env, removed = environment()
    module = source_module()
    evidence = HERE / "evidence" / args.run_name
    evidence.mkdir(parents=True, exist_ok=False)
    baseline = module.sources()
    write(evidence / "sources-before.json", baseline)
    toolchain = {}
    for name, command in (
        ("rustc", ["/usr/bin/nice", "-n", "15", "/home/ran/.cargo/bin/rustc", "-Vv"]),
        ("cargo", ["/usr/bin/nice", "-n", "15", "/home/ran/.cargo/bin/cargo", "-V"]),
        ("head", ["/usr/bin/git", "rev-parse", "HEAD"]),
    ):
        result = subprocess.run(command, cwd=ROOT, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        toolchain[name] = {"command": command, "exit_code": result.returncode, "output": result.stdout}
        if result.returncode:
            write(evidence / "preflight-failure.json", toolchain)
            return result.returncode
    write(evidence / "run.json", {
        "prepared_at_execution_utc": utc(), "source_scope": module.SOURCE_SCOPE,
        "cwd": str(ROOT), "suites": selected, "toolchain": toolchain,
        "helper_sha256": sha(Path(__file__)), "enumerator_sha256": sha(ENUMERATOR),
        "source_map_sha256": sha(evidence / "sources-before.json"),
        "removed_environment_value_sha256": removed,
        "environment": {key: env.get(key) for key in sorted(CLEAR | {
            "HOME", "PATH", "CATHEDRAL_HEADLESS", "CATHEDRAL_FAKE_BACKEND",
            "PYTHONDONTWRITEBYTECODE", "CARGO_TERM_COLOR", "RUST_TEST_THREADS",
        })},
        "defaults": {"cargo_home": "/home/ran/.cargo", "target_directory": str(ROOT / "target"),
                     "profile": "repository default dev/test", "test_threads": 1, "nice": 15},
    })
    completed = []
    overall = 0
    for name in selected:
        before = module.sources()
        if before != baseline:
            overall = 2
            write(evidence / f"{name}-not-started.json", {
                "utc": utc(), "reason": "source/input map changed before launch",
            })
            break
        command = PREFIX + SUITES[name] + ["--", "--test-threads=1", "--nocapture"]
        raw = evidence / f"{name}.log"
        started = time.monotonic()
        start = {"event": "START", "utc": utc(), "command": command, "cwd": str(ROOT),
                 "raw_log": str(raw), "source_map_sha256": sha(evidence / "sources-before.json")}
        write(evidence / f"{name}-start.json", start)
        print(json.dumps(start, sort_keys=True), flush=True)
        launch_error = None
        returncode = None
        with raw.open("xb") as output:
            try:
                process = subprocess.Popen(command, cwd=ROOT, env=env,
                                           stdout=output, stderr=subprocess.STDOUT)
            except OSError as error:
                launch_error = str(error)
            else:
                write(evidence / f"{name}-launched.json", {"utc": utc(), "pid": process.pid})
                returncode = process.wait()
        elapsed = time.monotonic() - started
        after = module.sources()
        write(evidence / f"{name}-sources-after.json", after)
        text = raw.read_text(errors="replace")
        matches = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed;", text)
        passed = sum(int(ok) for ok, _ in matches)
        has_tests = bool(matches) and passed > 0
        compressed = evidence / f"{name}.log.gz"
        archive(raw, compressed)
        result = {
            "event": "END", "utc": utc(), "suite": name, "wall_seconds": elapsed,
            "cargo_exit_code": returncode, "launch_error": launch_error,
            "raw_log": str(raw), "raw_sha256": sha(raw), "archive_sha256": sha(compressed),
            "source_map_sha256": sha(evidence / "sources-before.json"),
            "after_source_map_sha256": sha(evidence / f"{name}-sources-after.json"),
            "sources_unchanged": after == baseline, "passed_tests": passed,
            "successful_nonempty_test_summary": has_tests,
        }
        write(evidence / f"{name}-result.json", result)
        print(json.dumps(result, sort_keys=True), flush=True)
        completed.append(result)
        if returncode != 0 or after != baseline or not has_tests:
            overall = returncode if returncode is not None and returncode > 0 else 2
            break
    final = module.sources()
    if final != baseline:
        overall = overall or 2
    write(evidence / "sources-after.json", final)
    write(evidence / "summary.json", {
        "utc": utc(), "exit_code": overall, "completed_suites": [r["suite"] for r in completed],
        "planned_suites": selected, "sources_unchanged": final == baseline,
        "before_sha256": sha(evidence / "sources-before.json"),
        "after_sha256": sha(evidence / "sources-after.json"),
        "passed_tests": sum(r["passed_tests"] for r in completed),
    })
    return overall


if __name__ == "__main__":
    raise SystemExit(main())
