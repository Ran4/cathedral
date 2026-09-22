"""Verify archived command records and current component inputs; no Cargo invocation."""
from pathlib import Path
import datetime
import gzip
import hashlib
import importlib.util
import json
import re
import sys
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ENUMERATOR = HERE.parents[1] / "component_input_sources.py"
def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024): digest.update(chunk)
    return digest.hexdigest()
def read(path): return json.loads(path.read_text())
def ordered(start, launched, end):
    parse = datetime.datetime.fromisoformat
    assert parse(start["utc"]) <= parse(launched["utc"]) <= parse(end["utc"])
    assert launched["pid"] > 0
for name in ("focused-01", "focused-02"):
    directory = HERE / name
    run = read(directory / "run.json")
    assert run["helper_sha256"] == sha(HERE / "run_tests.py")
    assert run["enumerator_sha256"] == sha(ENUMERATOR)
    baseline = read(directory / "sources-before.json")
    assert read(directory / "sources-after.json") == baseline
    summary = read(directory / "summary.json")
    assert summary["exit_code"] == 0 and summary["sources_unchanged"]
    count = 0
    for suite in run["suites"]:
        result = read(directory / f"{suite}-result.json")
        start = read(directory / f"{suite}-start.json")
        ordered(start, read(directory / f"{suite}-launched.json"), result)
        assert result["cargo_exit_code"] == 0 and result["sources_unchanged"]
        assert result["successful_nonempty_test_summary"]
        assert result["source_map_sha256"] == sha(directory / "sources-before.json")
        assert result["after_source_map_sha256"] == sha(directory / f"{suite}-sources-after.json")
        assert read(directory / f"{suite}-sources-after.json") == baseline
        raw = directory / f"{suite}.log"
        compressed = directory / f"{suite}.log.gz"
        assert result["raw_sha256"] == sha(raw)
        assert result["archive_sha256"] == sha(compressed)
        assert gzip.decompress(compressed.read_bytes()) == raw.read_bytes()
        observed = sum(int(n) for n in re.findall(r"test result: ok\. (\d+) passed;", raw.read_text()))
        assert observed == result["passed_tests"] and observed > 0
        count += observed
    assert count == summary["passed_tests"]
a = read(HERE / "focused-01/sources-before.json")
b = read(HERE / "focused-02/sources-before.json")
delta = read(HERE / "source-delta-01-02.json")
changed = {p for p in a.keys() | b.keys() if a.get(p) != b.get(p)}
assert changed == set(delta["changed"]) == {
    "src/installed_recipe/startup.rs", "src/installed_recipe/startup/tests.rs", "src/smart_actors/local_engine.rs",
}
assert all(a[p] == b.get(p) for p in a if p.startswith(("crates/cathedral-backends/", "crates/cathedral-sim/")))
build = HERE / "production-01"
start = read(build / "start.json")
result = read(build / "result.json")
ordered(start, read(build / "launched.json"), result)
assert start["helper_sha256"] == sha(HERE / "run_build.py")
assert start["shared_runner_sha256"] == sha(HERE / "run_tests.py")
assert start["enumerator_sha256"] == sha(ENUMERATOR)
assert result["cargo_exit_code"] == 0 and result["sources_unchanged"]
assert result["executable_was_run"] is False
assert read(build / "sources-before.json") == read(build / "sources-after.json") == b
assert result["source_map_sha256"] == sha(build / "sources-before.json")
assert result["after_source_map_sha256"] == sha(build / "sources-after.json")
assert result["raw_sha256"] == sha(build / "production-build.log")
assert result["archive_sha256"] == sha(build / "production-build.log.gz")
assert gzip.decompress((build / "production-build.log.gz").read_bytes()) == (build / "production-build.log").read_bytes()
assert result["executable_sha256"] == sha(Path(result["executable"]))
spec = importlib.util.spec_from_file_location("component_sources", ENUMERATOR)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
assert module.sources() == b
print(json.dumps({"audit": "passed", "source_map_sha256": sha(build / "sources-before.json"),
    "distinct_focused_tests": 22, "production_build_exit": result["cargo_exit_code"],
    "executable_sha256": result["executable_sha256"], "current_sources_match": True}, sort_keys=True))
