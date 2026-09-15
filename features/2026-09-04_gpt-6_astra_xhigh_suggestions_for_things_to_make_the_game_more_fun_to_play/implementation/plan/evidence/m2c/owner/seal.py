"""Audit sealed owner records and describe the final M2c verification boundary.

Run with uv --no-project. This reads source/log/binary inputs and writes only
../verification.json; it never runs Cargo or changes a source input.
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


final_path = EVIDENCE / "source_hashes.json"
final_map = read(final_path)
assert final_path.read_bytes() == (HERE / "workspace-01-sources.json").read_bytes()
assert final_map == module.sources()
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
    assert archive.read_bytes()[4:8] == b"\0" * 4
    archived_raw = gzip.decompress(archive.read_bytes())
    assert hashlib.sha256(archived_raw).hexdigest() == result["raw_sha256"]
    assert sha(Path(result["raw_log"])) == result["raw_sha256"]
    groups = re.findall(
        rb"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        archived_raw,
    )
    counts = [sum(int(group[i]) for group in groups) for i in range(3)]
    commands.append({
        "name": name,
        "start": start,
        "result": result,
        "archive": str(archive.relative_to(EVIDENCE)),
        "test_groups": len(groups),
        "test_counts": dict(zip(("passed", "failed", "ignored"), counts)),
    })
commands.sort(key=lambda record: record["start"]["utc"])
workspace = next(record for record in commands if record["name"] == "workspace-01")
assert workspace["result"]["exit_code"] == 0
assert workspace["result"]["sources_unchanged"]
assert workspace["test_counts"] == {"passed": 2215, "failed": 0, "ignored": 44}
assert workspace["test_groups"] == 46
formatting = next(record for record in commands if record["name"] == "format-check-02")
assert formatting["result"]["exit_code"] == 0
assert formatting["result"]["sources_unchanged"]
assert formatting["start"]["source_map_sha256"] == sha(final_path)
formatted_files = [arg for arg in formatting["start"]["command"] if arg.endswith(".rs")]
assert len(formatted_files) == 39

binaries = []
for relative in [
    "target/debug/deps/cathedralbevy-eafe86af140beaa1",
    "target/debug/deps/cathedral_sim-29e329e3d5b1c8e4",
]:
    path = module.ROOT / relative
    binaries.append({"path": relative, "bytes": path.stat().st_size, "sha256": sha(path)})

probes = []
for name in ["probe-authored-02", "probe-populated-01"]:
    path = HERE / f"{name}-report.json"
    report = read(path)
    prior_map = read(HERE / f"{name}-sources.json")
    delta = sorted(p for p in prior_map.keys() | final_map.keys()
                   if prior_map.get(p) != final_map.get(p))
    assert delta == [
        "crates/cathedral-sim/tests/checkpoint_cognition_inputs_boundary.rs",
        "crates/cathedral-sim/tests/checkpoint_night_boundary.rs",
    ]
    assert bytes(report["host_image"]).hex() == binaries[0]["sha256"]
    assert report["unchanged_categories_equal"]
    assert report["unchanged_category_count"] == 8
    assert report["second_preparation_exact_bytes"]
    assert report["fixture_sha256"] == report["resave_sha256"]
    probes.append({
        "name": name,
        "report": str(path.relative_to(EVIDENCE)),
        "report_sha256": sha(path),
        "input_sha256": [bytes(value).hex() for value in report["input_sha256"]],
        "fixture_sha256": bytes(report["fixture_sha256"]).hex(),
        "resave_sha256": bytes(report["resave_sha256"]).hex(),
        "source_delta_to_final": delta,
        "interpretation": "One debug sample: reachability/admission and same-process immediate V2 re-save; no release distribution or fresh-process reader claim.",
    })

result = {
    "status": "owner verification complete; Cargo ceded to coordinator; release acceptance pending",
    "predecessor_commit": "019f451c12bf7c94c9823678403f50244c88d421",
    "source_scope": module.SOURCE_SCOPE,
    "source_count": len(final_map),
    "source_map": "source_hashes.json",
    "source_map_sha256": sha(final_path),
    "source_map_matches_current": True,
    "seal_helper_sha256": sha(Path(__file__)),
    "owner_command_count": len(commands),
    "workspace": {"test_counts": workspace["test_counts"], "groups": workspace["test_groups"]},
    "formatted_rust_files": formatted_files,
    "binaries": binaries,
    "debug_probes": probes,
    "commands": commands,
}
target = EVIDENCE / "verification.json"
target.write_text(json.dumps(result, sort_keys=True, indent=2) + "\n")
print(json.dumps({"written": str(target), "commands": len(commands), "sources": len(final_map),
                  "source_map_sha256": sha(final_path), "workspace": result["workspace"]}))
