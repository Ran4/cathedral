# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Independently verify archived M2 component measurements without rerunning them."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
PHASES = (
    "preflight_us", "export_us", "encode_us", "decode_validate_us",
    "candidate_validate_us", "drop_us",
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def quantiles(samples):
    ordered = sorted(samples)
    assert ordered and all(math.isfinite(v) and v >= 0 for v in ordered)
    return {
        name: ordered[math.ceil(len(ordered) * fraction) - 1]
        for name, fraction in (("p50", .5), ("p95", .95), ("p99", .99), ("max", 1))
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--check-current", type=Path, metavar="BINARY",
                        help="also require current source, runner and binary hashes")
    args = parser.parse_args()
    folder = args.directory.resolve()
    identity = json.loads((folder / "IDENTITY.json").read_text())
    rows = json.loads((folder / "RESULTS.json").read_text())
    summary = json.loads((folder / "SUMMARY.json").read_text())
    repeats, samples = (1, 2) if identity["smoke_only"] else (3, 100)
    expected_names = []
    for repeat in range(repeats):
        modes = ("authored", "populated") if repeat % 2 == 0 else ("populated", "authored")
        expected_names.extend(f"{mode}-{repeat + 1}" for mode in modes)
    assert [row["name"] for row in rows] == expected_names
    grouped = {mode: [] for mode in ("authored", "populated")}
    all_samples = {mode: {phase: [] for phase in PHASES} for mode in grouped}
    for row in rows:
        name = row["name"]
        raw = gzip.decompress((folder / f"{name}.json.gz").read_bytes())
        artifacts = row["artifacts"]
        assert set(artifacts) == {
            "uncompressed_json_sha256", f"{name}.json.gz", f"{name}.time", f"{name}.stderr",
        }
        assert sha(raw) == artifacts["uncompressed_json_sha256"]
        for filename, expected in artifacts.items():
            if filename == "uncompressed_json_sha256":
                continue
            path = (folder / filename).resolve()
            assert path.parent == folder
            assert sha(path.read_bytes()) == expected, filename
        data = json.loads(raw)
        assert row["exit_code"] == 0 and data["samples"] == samples
        assert row["sample_count_per_phase"] == samples
        metadata = {
            key: {"raw_samples": key} if key in PHASES else value
            for key, value in data.items()
        }
        assert metadata == row["metadata"], name
        mode = data["mode"]
        assert name.startswith(mode + "-")
        expected_extra = 2000 if mode == "populated" else 0
        assert data["placement"] == {
            "requested": expected_extra, "placed": expected_extra, "unplaced": 0,
        }
        cost = data["cost"]
        assert 0 < cost["encoded_bytes"] <= (128 if expected_extra else 64) * 1024**2
        assert 0 < cost["expanded_upper_bytes"] <= 128 * 1024**2
        definition_working_bytes = cost.get("definition_working_bytes", 0)
        assert isinstance(definition_working_bytes, int) and definition_working_bytes >= 0
        assert cost["peak_bytes"] == (
            4096 + 4 * cost["expanded_upper_bytes"] + 3 * cost["encoded_bytes"]
            + definition_working_bytes
        )
        assert cost["peak_bytes"] <= data["shared_reserved_peak_excluding_running_bytes"] <= 1024**3
        for phase in PHASES:
            assert len(data[phase]) == samples
            assert quantiles(data[phase]) == row["phase_us"][phase], (name, phase)
            all_samples[mode][phase].extend(data[phase])
        process = {}
        for line in (folder / f"{name}.time").read_text().splitlines():
            for label in ("User time (seconds)", "System time (seconds)",
                          "Maximum resident set size (kbytes)"):
                if line.strip().startswith(label + ":"):
                    process[label] = float(line.split(":", 1)[1].strip())
        assert process == row["process"]
        grouped[mode].append(row)
    assert set(summary) == set(grouped)
    global_max_us = {}
    for mode, group in grouped.items():
        assert all(row["metadata"] == group[0]["metadata"] for row in group)
        expected = {
            "runs": repeats, "samples_per_phase": repeats * samples,
            "metadata": group[0]["metadata"],
            "median_run_us": {
                phase: {
                    key: statistics.median(row["phase_us"][phase][key] for row in group)
                    for key in ("p50", "p95", "p99", "max")
                } for phase in PHASES
            },
            "same_binary_semantic_counters_repeat": True,
        }
        assert summary[mode] == expected, mode
        global_max_us[mode] = {phase: max(values) for phase, values in all_samples[mode].items()}
    assert (summary["populated"]["metadata"]["counts"]["characters"]
            - summary["authored"]["metadata"]["counts"]["characters"]) == 2000
    assert identity["unchanged_source_binary_and_runners_at_end"] is True
    if args.check_current:
        assert sha(args.check_current.read_bytes()) == identity["binary_sha256"]
        paths = subprocess.check_output(
            ["/usr/bin/git", "ls-files", "--cached", "--others", "--exclude-standard",
             "--", "Cargo*", "config.ron", "src", "crates", "assets/world",
             "assets/prompts", "assets/sounds/catalog.toml", "lore/characters",
             "lore/core_lore/occupations.json"], cwd=ROOT, text=True,
        ).splitlines()
        assert {p for p in paths if (ROOT / p).is_file()} == set(identity["source_sha256"])
        for filename, expected in identity["source_sha256"].items():
            path = (ROOT / filename).resolve()
            assert path.is_relative_to(ROOT)
            assert sha(path.read_bytes()) == expected, filename
        for filename, expected in identity["runner_sha256"].items():
            path = (HERE / filename).resolve()
            assert path.parent == HERE
            assert sha(path.read_bytes()) == expected, filename
    report = {
        "checked_at_utc": datetime.now(timezone.utc).isoformat(),
        "measurement_directory": str(folder),
        "result": "passed", "smoke_only": identity["smoke_only"],
        "runs": len(rows), "raw_phase_samples": len(rows) * len(PHASES) * samples,
        "checks": ["archive and original hashes", "phase lengths and nearest-rank percentiles",
                   "repeated metadata", "count and byte admission", "process timing metadata",
                   "all summary medians"],
        "current_source_binary_runner_hashes_checked": bool(args.check_current),
        "global_max_us": global_max_us,
        "auditor_sha256": sha(Path(__file__).read_bytes()),
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
