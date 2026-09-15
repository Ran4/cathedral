"""Audit M2d command provenance and exact logs without running Cargo.

Development runs remain evidence even when source changed or the command failed.
Pass final unchanged-source run names to require their map to match the current
component-inputs-v2 map. No historical command is silently reclassified as final.
"""
import datetime
import gzip
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"
EVIDENCE = HERE.parents[1]
ENUMERATOR = EVIDENCE / "component_input_sources.py"
spec = importlib.util.spec_from_file_location("inputs", ENUMERATOR)
inputs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(inputs)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return json.loads(path.read_text())


required = set(sys.argv[1:])
current = inputs.sources()
rows = []
pending = []
helper_sha = sha((OWNER / "run.py").read_bytes())
enumerator_sha = sha(ENUMERATOR.read_bytes())
test_pattern = re.compile(
    r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;"
)
for start_path in sorted(OWNER.glob("*-start.json")):
    name = start_path.name.removesuffix("-start.json")
    result_path = OWNER / (name + "-result.json")
    if not result_path.exists():
        pending.append(name)
        continue
    start, result = read(start_path), read(result_path)
    map_path = OWNER / (name + "-sources.json")
    source_map = read(map_path)
    assert sha(map_path.read_bytes()) == start["source_map_sha256"]
    assert start["source_map_sha256"] == result["source_map_sha256"]
    assert start["helper_sha256"] == helper_sha, (name, "runner identity")
    assert start["enumerator_sha256"] == enumerator_sha
    assert start["raw_log"] == result["raw_log"]
    raw = Path(result["raw_log"]).read_bytes()
    archive = (OWNER / (name + ".log.gz")).read_bytes()
    assert sha(raw) == result["raw_sha256"]
    assert sha(archive) == result["archive_sha256"]
    assert archive[:3] == b"\x1f\x8b\x08" and archive[4:8] == bytes(4)
    assert gzip.decompress(archive) == raw
    env = start["environment"]
    for key in (
        "LDFLAGS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
    ):
        assert env[key] is None, (name, key)
    assert env["CARGO_HOME"] == "/tmp/alibi-m1b-cargo"
    assert env["CATHEDRAL_HEADLESS"] == env["CATHEDRAL_FAKE_BACKEND"] == "1"
    assert env["PYTHONDONTWRITEBYTECODE"] == "1"
    assert start["cwd"] == str(inputs.ROOT)
    command = start["command"]
    if command[0] == "/home/ran/.cargo/bin/cargo":
        assert "--offline" in command and "-j1" in command
    groups = [
        {"status": status, "passed": int(passed), "failed": int(failed),
         "ignored": int(ignored)}
        for status, passed, failed, ignored in test_pattern.findall(
            raw.decode("utf-8", errors="replace")
        )
    ]
    if name in required:
        assert result["exit_code"] == 0, (name, "failed acceptance command")
        assert result["sources_unchanged"], (name, "changed during command")
        assert source_map == current, (name, "not the current source map")
        assert all(group["failed"] == 0 for group in groups)
    rows.append({
        "name": name, "command": command, "utc": start["utc"],
        "exit_code": result["exit_code"],
        "wall_seconds": result["wall_seconds"],
        "sources_unchanged_during_command": result["sources_unchanged"],
        "matches_current_sources": source_map == current,
        "source_count": len(source_map),
        "source_map_sha256": start["source_map_sha256"],
        "raw_sha256": result["raw_sha256"],
        "archive_sha256": result["archive_sha256"],
        "exact_raw_archive_verified": True,
        "test_groups": groups,
        "totals": {key: sum(group[key] for group in groups)
                   for key in ("passed", "failed", "ignored")},
    })
assert required <= {row["name"] for row in rows}, "missing acceptance run"
print(json.dumps({
    "audit_utc": datetime.datetime.now(datetime.UTC).isoformat(),
    "audit_sha256": sha(Path(__file__).read_bytes()),
    "helper_sha256": helper_sha, "enumerator_sha256": enumerator_sha,
    "required_final_runs": sorted(required),
    "pending_runs": pending, "commands": rows,
}, indent=2, sort_keys=True))
