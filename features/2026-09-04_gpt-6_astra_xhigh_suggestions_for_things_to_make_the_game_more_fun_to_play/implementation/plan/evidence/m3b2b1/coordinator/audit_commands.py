"""Audit recorded M3b2b1 commands and original logs without invoking Cargo."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"
EVIDENCE = HERE.parents[1]


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read(path: Path):
    return json.loads(path.read_bytes())


parser = argparse.ArgumentParser()
parser.add_argument("--require-complete", action="store_true")
parser.add_argument("--final-map", type=Path)
args = parser.parse_args()
final_map = read(args.final_map) if args.final_map else None
helpers: dict[str, list[str]] = {}
for path in OWNER.rglob("*.py"):
    helpers.setdefault(sha(path.read_bytes()), []).append(str(path.relative_to(OWNER)))
enumerator = sha((EVIDENCE / "component_input_sources.py").read_bytes())
rows = []
pending = []
names = set()
for start_path in sorted(OWNER.glob("*-start.json")):
    name = start_path.name.removesuffix("-start.json")
    names.add(name)
    result_path = OWNER / f"{name}-result.json"
    if not result_path.exists():
        pending.append(name)
        continue
    start, result = read(start_path), read(result_path)
    raw_path = Path(start["raw_log"])
    assert raw_path.parent == Path("/tmp")
    assert raw_path.name == f"alibi-m3b2b1-{name}.log"
    raw = raw_path.read_bytes()
    archive = (OWNER / f"{name}.log.gz").read_bytes()
    source_bytes = (OWNER / f"{name}-sources.json").read_bytes()
    sources = json.loads(source_bytes)
    assert result["raw_log"] == str(raw_path)
    assert sha(raw) == result["raw_sha256"]
    assert sha(archive) == result["archive_sha256"]
    assert archive[4:8] == bytes(4)
    assert gzip.decompress(archive) == raw
    assert sha(source_bytes) == start["source_map_sha256"] == result["source_map_sha256"]
    assert sources and all(re.fullmatch(r"[0-9a-f]{64}", value) for value in sources.values())
    assert start["enumerator_sha256"] == enumerator
    assert start["helper_sha256"] in helpers, f"missing historical runner: {name}"
    env = start["environment"]
    for key in [
        "LDFLAGS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
    ]:
        assert env[key] is None
    assert env["CARGO_HOME"] == "/tmp/alibi-m1b-cargo"
    assert env["RUSTC"] == "/home/ran/.cargo/bin/rustc"
    assert env["RUSTDOC"] == "/home/ran/.cargo/bin/rustdoc"
    for key in ["CATHEDRAL_HEADLESS", "CATHEDRAL_FAKE_BACKEND", "PYTHONDONTWRITEBYTECODE"]:
        assert env[key] == "1"
    command = start["command"]
    assert command and all(isinstance(arg, str) for arg in command)
    if command[0] == "/home/ran/.cargo/bin/cargo":
        assert "--offline" in command and "-j1" in command
    # The fresh-process fixture prints captured stdout verbatim. Its nested
    # summary is useful evidence, but is not another outer Cargo test group.
    summary_pattern = rb"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;"
    child_blocks = re.findall(
        rb"^fresh-process child stdout:.*?^test result: [^\n]*\n",
        raw, re.M | re.S,
    )
    child_summaries = [
        group for block in child_blocks
        for group in re.findall(summary_pattern, block, re.M)
    ]
    outer = raw
    for block in child_blocks:
        outer = outer.replace(block, b"", 1)
    summaries = re.findall(summary_pattern, outer, re.M)
    rows.append({
        "name": name,
        "command": command,
        "exit_code": result["exit_code"],
        "wall_seconds": result["wall_seconds"],
        "sources_unchanged": result["sources_unchanged"],
        "matches_final_map": sources == final_map if final_map is not None else None,
        "source_map_sha256": sha(source_bytes),
        "source_count": len(sources),
        "runner_sha256": start["helper_sha256"],
        "preserved_runner_paths": helpers[start["helper_sha256"]],
        "raw_log": str(raw_path),
        "raw_sha256": sha(raw),
        "raw_bytes": len(raw),
        "archive_sha256": sha(archive),
        "test_groups": len(summaries),
        "test_totals": [
            sum(int(group[column]) for group in summaries) for column in range(3)
        ],
        "printed_child_groups": len(child_summaries),
        "printed_child_totals": [
            sum(int(group[column]) for group in child_summaries) for column in range(3)
        ],
    })

maps_without_start = sorted(
    path.name.removesuffix("-sources.json")
    for path in OWNER.glob("*-sources.json")
    if path.name.removesuffix("-sources.json") not in names
)
assert not args.require_complete or not pending, f"unfinished commands: {pending}"
print(json.dumps({
    "auditor_sha256": sha(Path(__file__).read_bytes()),
    "enumerator_sha256": enumerator,
    "commands": rows,
    "pending": pending,
    # Setup maps are not silently counted as executed/verified commands.
    "maps_without_started_command": maps_without_start,
}, sort_keys=True, indent=2))
