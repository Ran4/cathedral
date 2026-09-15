"""Verify original owner command evidence and final frozen-source identity."""
from __future__ import annotations

import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "sources", HERE.parents[1] / "component_input_sources.py"
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


names = [
    "check-01", "tests-02", "tests-03", "format-04", "workspace-05",
    "format-06", "tests-07", "tests-08", "format-09", "workspace-10", "diff-11",
]
helpers = {digest(HERE / name) for name in ("run-v1.py", "run-v2.py")}
records = []
for name in names:
    start = json.loads((HERE / f"{name}-start.json").read_text())
    result = json.loads((HERE / f"{name}-result.json").read_text())
    raw_path = Path(start["raw_log"])
    raw = raw_path.read_bytes()
    archive = HERE / f"{name}.log.gz"
    assert gzip.decompress(archive.read_bytes()) == raw, name
    assert hashlib.sha256(raw).hexdigest() == result["raw_sha256"], name
    assert digest(archive) == result["archive_sha256"], name
    source_hash = digest(HERE / f"{name}-sources.json")
    assert source_hash == start["source_map_sha256"] == result["source_map_sha256"], name
    assert start["helper_sha256"] in helpers, name
    assert result["exit_code"] == (101 if name == "tests-07" else 0), name
    assert result["sources_unchanged"] == (name not in {"tests-02", "workspace-05"}), name
    assert start["environment"]["CATHEDRAL_HEADLESS"] == "1", name
    assert start["environment"]["CATHEDRAL_FAKE_BACKEND"] == "1", name
    if start["command"][0].endswith("/cargo"):
        assert "--offline" in start["command"] and "-j1" in start["command"], name
    groups = [tuple(map(int, group)) for group in re.findall(
        rb"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", raw
    )]
    records.append({
        "name": name, **result, "test_groups": len(groups),
        "tests_passed": sum(g[0] for g in groups),
        "tests_failed": sum(g[1] for g in groups),
        "tests_ignored": sum(g[2] for g in groups),
    })

final_sources_path = HERE / "workspace-10-sources.json"
final_sources = json.loads(final_sources_path.read_text())
assert final_sources == module.sources()
for name in ("tests-08", "format-09"):
    assert json.loads((HERE / f"{name}-sources.json").read_text()) == final_sources
log = Path("/tmp/alibi-m2b-workspace-10.log").read_text()
binary = module.ROOT / re.search(
    r"Running unittests src/main.rs \(([^)]+)\)", log
).group(1)
report = {
    "schema": "m2b-owner-verification-v1",
    "sources": len(final_sources),
    "source_map_sha256": digest(final_sources_path),
    "current_sources_equal_final": True,
    "commands": records,
    "final_debug_host_test_binary": {
        "path": str(binary), "bytes": binary.stat().st_size, "sha256": digest(binary),
    },
    "auditor_sha256": digest(Path(__file__)),
    "enumerator_sha256": digest(HERE.parents[1] / "component_input_sources.py"),
}
(HERE / "verification.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
print(json.dumps(report, indent=2, sort_keys=True))
