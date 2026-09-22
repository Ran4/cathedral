#!/usr/bin/env python3
"""Compile the diagnostic shim and run exactly the ignored native witness.

Requires a successful backend_capture suite on the identical source map first.
The shim is preloaded only into the probe command (nice then the test), never
Cargo or production. Counters stay unarmed in the nice wrapper.
"""
import argparse
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
spec = importlib.util.spec_from_file_location("common", HERE / "run_tests.py")
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
LIBC = Path("/lib/x86_64-linux-gnu/libc.so.6")
LIBC_SHA = "b2cf6c33b74d2f22543b7a469a75b538911e690f769d0b238843a49465b83793"


def execute(folder, label, command, env, metadata):
    raw = folder / f"{label}.log"
    record = dict(metadata, event="START", utc=common.utc(), command=command,
                  cwd=str(common.ROOT), raw_log=str(raw))
    common.write(folder / f"{label}-start.json", record)
    print(json.dumps(record, sort_keys=True), flush=True)
    start = time.monotonic()
    with raw.open("xb") as output:
        process = subprocess.Popen(command, cwd=common.ROOT, env=env,
                                   stdout=output, stderr=subprocess.STDOUT)
        common.write(folder / f"{label}-launched.json", {"utc": common.utc(), "pid": process.pid})
        code = process.wait()
    common.archive(raw, folder / f"{label}.log.gz")
    result = {"event": "END", "utc": common.utc(), "exit_code": code,
              "wall_seconds": time.monotonic() - start, "raw_sha256": common.sha(raw),
              "archive_sha256": common.sha(folder / f"{label}.log.gz")}
    common.write(folder / f"{label}-result.json", result)
    print(json.dumps(result, sort_keys=True), flush=True)
    return code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-name", required=True)
    parser.add_argument("--focused-run", required=True)
    parser.add_argument("--after-freeze", action="store_true")
    args = parser.parse_args()
    if not args.after_freeze or not all(re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]*", name)
                                      for name in (args.run_name, args.focused_run)):
        parser.error("explicit ownership authorization and plain unique run names required")
    env, removed = common.environment()
    module = common.source_module()
    before = module.sources()
    previous = HERE / "evidence" / args.focused_run
    assert json.loads((previous / "sources-before.json").read_text()) == before
    backend = json.loads((previous / "backend_capture-result.json").read_text())
    assert backend["cargo_exit_code"] == 0 and backend["passed_tests"] == 9
    assert common.sha(LIBC) == LIBC_SHA and os.sysconf("SC_PAGE_SIZE") == 4096
    matches = re.findall(r"Running unittests .* \((target/debug/deps/[^)]+)\)",
                         (previous / "backend_capture.log").read_text())
    assert len(matches) == 1, matches
    executable = common.ROOT / matches[0]
    folder = HERE / "evidence" / args.run_name
    folder.mkdir(parents=True, exist_ok=False)
    common.write(folder / "sources-before.json", before)
    compiler = Path("/usr/bin/cc").resolve()
    version = subprocess.check_output([str(compiler), "--version"], env=env, text=True)
    shared = folder / "native_probe.so"
    metadata = {"source_map_sha256": common.sha(folder / "sources-before.json"),
                "runner_sha256": common.sha(Path(__file__)),
                "shared_runner_sha256": common.sha(HERE / "run_tests.py"),
                "enumerator_sha256": common.sha(common.ENUMERATOR),
                "probe_c_sha256": common.sha(HERE / "native_probe.c"),
                "compiler": str(compiler), "compiler_sha256": common.sha(compiler),
                "compiler_version": version, "libc": str(LIBC), "libc_sha256": common.sha(LIBC),
                "system_page_bytes": 4096, "test_executable": str(executable),
                "test_executable_sha256": common.sha(executable),
                "removed_environment_value_sha256": removed,
                "environment": {key: env.get(key) for key in sorted(common.CLEAR | {
                    "HOME", "PATH", "CATHEDRAL_HEADLESS", "CATHEDRAL_FAKE_BACKEND", "RUST_TEST_THREADS"})}}
    compile_command = ["/usr/bin/nice", "-n", "15", str(compiler), "-shared", "-fPIC",
                       "-O2", "-std=c11", "-Wall", "-Wextra", "-Werror", "-fno-builtin",
                       str(HERE / "native_probe.c"), "-o", str(shared), "-ldl"]
    code = execute(folder, "compile", compile_command, env, metadata)
    if code:
        return code
    env["LD_PRELOAD"] = str(shared)
    metadata["environment"]["LD_PRELOAD"] = str(shared)
    metadata["probe_library_sha256"] = common.sha(shared)
    command = ["/usr/bin/nice", "-n", "15", str(executable), "--exact",
               "world_data::capture::tests::native_probe::native_directory_allocator_lifetime_probe",
               "--ignored", "--test-threads=1", "--nocapture"]
    code = execute(folder, "probe", command, env, metadata)
    after = module.sources()
    common.write(folder / "sources-after.json", after)
    log = (folder / "probe.log").read_text()
    passed = "test result: ok. 1 passed; 0 failed;" in log
    common.write(folder / "summary.json", {"utc": common.utc(), "exit_code": code,
        "sources_unchanged": before == after, "successful_exact_test": passed,
        "source_map_sha256": common.sha(folder / "sources-before.json"),
        "after_source_map_sha256": common.sha(folder / "sources-after.json"),
        "probe_c_sha256": common.sha(HERE / "native_probe.c"),
        "probe_library_sha256": common.sha(shared), "test_executable_sha256": common.sha(executable)})
    return code or (0 if before == after and passed else 2)


if __name__ == "__main__":
    raise SystemExit(main())
