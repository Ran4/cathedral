"""Seal M2d owner commands and frozen verification; never runs Cargo.

Run with PYTHONDONTWRITEBYTECODE=1 uv run --offline --no-project. Writes only
../source_hashes.json and ../verification.json after all verification succeeds.
"""
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parent
ENUMERATOR = EVIDENCE.parent / "component_input_sources.py"
spec = importlib.util.spec_from_file_location("sources", ENUMERATOR)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()

def read(path):
    return json.loads(path.read_text())

final_runs = ("focused-final-01", "format-final-01", "workspace-final-01")
final_path = HERE / "workspace-final-01-sources.json"
final_map = read(final_path)
assert final_map == module.sources()
assert len(final_map) == 987
commands = []
for start_path in sorted(HERE.glob("*-start.json")):
    name = start_path.name.removesuffix("-start.json")
    start = read(start_path)
    result = read(HERE / f"{name}-result.json")
    assert sha(HERE / f"{name}-sources.json") == start["source_map_sha256"]
    assert result["source_map_sha256"] == start["source_map_sha256"]
    assert sha(HERE / "run.py") == start["helper_sha256"]
    assert sha(ENUMERATOR) == start["enumerator_sha256"]
    archive = HERE / f"{name}.log.gz"
    assert sha(archive) == result["archive_sha256"]
    archived = archive.read_bytes()
    assert archived[4:8] == b"\0" * 4
    raw = gzip.decompress(archived)
    assert hashlib.sha256(raw).hexdigest() == result["raw_sha256"]
    assert Path(result["raw_log"]).read_bytes() == raw
    groups = re.findall(
        rb"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        raw,
    )
    counts = dict(zip(("passed", "failed", "ignored"),
                      (sum(int(group[i]) for group in groups) for i in range(3))))
    if name in final_runs:
        assert result["exit_code"] == 0 and result["sources_unchanged"]
        assert start["source_map_sha256"] == sha(final_path)
        assert read(HERE / f"{name}-sources.json") == final_map
        assert counts["failed"] == 0
    commands.append({
        "name": name, "start": start, "result": result,
        "archive": str(archive.relative_to(EVIDENCE)),
        "test_groups": len(groups), "test_counts": counts,
    })
commands.sort(key=lambda record: record["start"]["utc"])
by_name = {record["name"]: record for record in commands}
workspace = by_name["workspace-final-01"]
assert workspace["test_counts"] == {"passed": 2229, "failed": 0, "ignored": 44}
assert workspace["test_groups"] == 46
assert by_name["focused-final-01"]["test_counts"] == {
    "passed": 16, "failed": 0, "ignored": 0,
}
formatting = by_name["format-final-01"]
formatted = [arg for arg in formatting["start"]["command"] if arg.endswith(".rs")]
assert len(formatted) == 9
result = {
    "status": "owner frozen verification complete; coordinator review pending",
    "predecessor_commit": "d921864d32f44abc1c4428fe1b7052eb153d9670",
    "source_scope": module.SOURCE_SCOPE,
    "source_count": len(final_map),
    "source_map": "source_hashes.json",
    "source_map_sha256": sha(final_path),
    "source_map_matches_current": True,
    "seal_helper_sha256": sha(Path(__file__)),
    "owner_command_count": len(commands),
    "failed_development_commands": [r["name"] for r in commands
                                    if r["result"]["exit_code"] != 0],
    "final_runs": list(final_runs),
    "workspace": {"test_counts": workspace["test_counts"],
                  "groups": workspace["test_groups"]},
    "formatted_rust_files": formatted,
    "scope": "Test-only complete future behavior; no new production API, image fixture, release cost, aggregate heap, renderer or stress claim.",
    "commands": commands,
}
(EVIDENCE / "source_hashes.json").write_bytes(final_path.read_bytes())
(EVIDENCE / "verification.json").write_text(json.dumps(result, sort_keys=True, indent=2) + "\n")
print(json.dumps({"commands": len(commands), "sources": len(final_map),
                  "source_map_sha256": sha(final_path), "workspace": result["workspace"]}))
