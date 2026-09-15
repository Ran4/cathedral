"""Independently audit completed M3a command records; never invokes Cargo."""
from __future__ import annotations

import gzip
import hashlib
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read(path: Path):
    return json.loads(path.read_bytes())


rows = []
pending = []
publication_phases = {
    "JournalWrite", "JournalFlush", "JournalReplace", "JournalSync",
    "PayloadWrite", "PayloadValidate", "PayloadFlush", "PayloadPublish", "PayloadSync",
    "RecoveryWrite", "RecoveryFlush", "RecoveryReplace", "RecoverySync",
    "ActiveWrite", "ActiveFlush", "ActiveReplace", "ActiveSync",
    "Cleanup", "CleanupSync", "PendingRemove", "PendingSync",
}
recovery_phases = {phase for phase in publication_phases if not phase.startswith("Payload")}
for start_path in sorted(OWNER.glob("*-start.json")):
    name = start_path.name.removesuffix("-start.json")
    result_path = OWNER / f"{name}-result.json"
    if not result_path.exists():
        pending.append(name)
        continue
    start, result = read(start_path), read(result_path)
    raw_path = Path(start["raw_log"])
    assert raw_path.parent == Path("/tmp") and raw_path.name.startswith("alibi-m3a-")
    raw = raw_path.read_bytes()
    archive = (OWNER / f"{name}.log.gz").read_bytes()
    source_map = (OWNER / f"{name}-sources.json").read_bytes()
    assert result["raw_log"] == str(raw_path)
    assert sha(raw) == result["raw_sha256"]
    assert sha(archive) == result["archive_sha256"]
    assert gzip.decompress(archive) == raw
    assert archive[4:8] == bytes(4), "gzip timestamp must be zero"
    assert sha(source_map) == start["source_map_sha256"] == result["source_map_sha256"]
    sources = json.loads(source_map)
    assert sources and all(re.fullmatch(r"[0-9a-f]{64}", value) for value in sources.values())
    assert start["enumerator_sha256"] == sha((HERE.parents[1] / "component_input_sources.py").read_bytes())
    assert start["helper_sha256"] == sha((OWNER / "run.py").read_bytes()), "changed runner needs its historical source preserved"
    env = start["environment"]
    for key in ["LDFLAGS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"]:
        assert env[key] is None
    assert env["CARGO_HOME"] == "/tmp/alibi-m1b-cargo"
    assert env["CATHEDRAL_HEADLESS"] == env["CATHEDRAL_FAKE_BACKEND"] == env["PYTHONDONTWRITEBYTECODE"] == "1"
    command = start["command"]
    if command[0] == "/home/ran/.cargo/bin/cargo":
        assert "--offline" in command and "-j1" in command
    nested_helper = None
    helper_bindings = [arg.split("=", 1)[1] for arg in command if arg.startswith("ALIBI_M3A_HELPER_SHA256=")]
    if helper_bindings:
        scripts = [Path(start["cwd"]) / arg for arg in command if arg.endswith(".py")]
        assert len(helper_bindings) == len(scripts) == 1
        assert scripts[0].resolve().is_relative_to(OWNER.resolve())
        assert sha(scripts[0].read_bytes()) == helper_bindings[0]
        nested_helper = {"path": str(scripts[0]), "sha256": helper_bindings[0]}
    summaries = re.findall(rb"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", raw, re.M)
    returned_faults = [(phase.decode(), edge.decode()) for phase, edge in
                       re.findall(rb"returned-fault (\w+) (Before|After):", raw)]
    expected_faults = {(phase, edge) for phase in publication_phases for edge in ["Before", "After"]}
    full_fault_matrix = len(returned_faults) == len(expected_faults) and set(returned_faults) == expected_faults
    children = []
    for line in raw.splitlines():
        begin = line.find(b'{"exit_code":')
        if begin < 0 or b'"subprocess":"m3a-storage"' not in line:
            continue
        child = json.loads(line[begin:])
        assert child["subprocess"] == "m3a-storage"
        assert child["mode"] in {"read", "publish", "recover"}
        assert child["exit_code"] == (0 if child["mode"] == "read" else 86)
        assert child["stderr"] == ""
        if child["mode"] == "read":
            assert child["phase"] is None
            assert "fresh same-image M2 validation passed" in child["stdout"]
        children.append(child)
    death_phases = {mode: [child["phase"] for child in children if child["mode"] == mode]
                    for mode in ["publish", "recover"]}
    readers = sum(child["mode"] == "read" for child in children)
    full_death_matrix = (
        len(death_phases["publish"]) == len(publication_phases)
        and set(death_phases["publish"]) == publication_phases
        and len(death_phases["recover"]) == len(recovery_phases)
        and set(death_phases["recover"]) == recovery_phases
        and readers == len(publication_phases) + len(recovery_phases)
    )
    rows.append({
        "name": name,
        "command": command,
        "exit_code": result["exit_code"],
        "sources_unchanged": result["sources_unchanged"],
        "source_map_sha256": sha(source_map),
        "source_count": len(sources),
        "raw_sha256": sha(raw),
        "raw_bytes": len(raw),
        "archive_sha256": sha(archive),
        "test_groups": len(summaries),
        "test_totals": [sum(int(group[column]) for group in summaries) for column in range(3)],
        "returned_fault_cases": len(returned_faults),
        "complete_returned_fault_matrix": full_fault_matrix,
        "subprocesses": len(children),
        "publication_deaths": len(death_phases["publish"]),
        "recovery_deaths": len(death_phases["recover"]),
        "fresh_validators": readers,
        "complete_death_matrix": full_death_matrix,
        "nested_helper": nested_helper,
    })

print(json.dumps({"auditor_sha256": sha(Path(__file__).read_bytes()), "commands": rows, "pending": pending}, sort_keys=True, indent=2))
