# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Independently audit preserved sequential M0/current Engine::poll comparisons."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[5]
REFERENCE = "f7ba972a440a2d6d2690c4dd2afd230363e5778ebf2c3fe7d62d4becca3e9057"
COUNTERS = (
    "actors", "placement", "moved_actors_over_0_1m", "message_count",
    "measured_speech_events", "snapshot_publications", "snapshot_bytes_final",
    "knowledge_bytes_max", "cognition_calls_including_warmup", "prompt_bytes_max",
)
QUANTILES = {"p50": .5, "p95": .95, "p99": .99, "max": 1.0}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load(path):
    return json.loads(Path(path).read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--check-current", action="store_true")
    args = parser.parse_args()
    directory = args.directory.resolve()
    identity = load(directory / "IDENTITY.json")
    rows = load(directory / "RESULTS.json")
    pairs = load(directory / "PAIRS.json")
    summary = load(directory / "SUMMARY.json")
    smoke = identity["smoke_only"]
    workloads = [(0, True), (0, False), (1000, False), (2000, False), (20000, False)]
    if smoke:
        workloads = [(0, False)]
    expected = []
    for extra, idle in workloads:
        workload = f'{extra}-{"idle" if idle else "market"}'
        for pair in range(1 if extra == 20000 or smoke else 3):
            sides = ("m0", "m1") if pair % 2 == 0 else ("m1", "m0")
            for side in sides:
                expected.append((f"{workload}-{pair + 1}-{side}", workload, pair + 1, side))
    require(len(rows) == len(expected), "run count differs from comparison protocol")
    require(identity["binary_sha256"]["m0"] == REFERENCE, "wrong M0 reference")
    require(identity["unchanged_source_and_binaries_at_end"], "measurement did not freeze inputs")
    archives, total, by_pair = {}, 0, {}
    for row, (name, workload, pair, side) in zip(rows, expected):
        require((row["name"], row["workload"], row["pair"], row["binary"])
                == (name, workload, pair, side), f"run order differs: {name}")
        archive = directory / f"{name}.json.gz"
        raw = gzip.decompress(archive.read_bytes())
        data = json.loads(raw)
        values = data.pop("poll_us")
        n = 20 if smoke else 1000 if workload.startswith("20000-") else 1200
        require(len(values) == n == row["sample_count"], f"sample count: {name}")
        require(all(isinstance(v, (int, float)) and not isinstance(v, bool)
                    and math.isfinite(v) and v >= 0 for v in values), f"invalid sample: {name}")
        ordered = sorted(values)
        measured = {p: ordered[math.ceil(n * q) - 1] for p, q in QUANTILES.items()}
        require(measured == row["poll_us"], f"percentile mismatch: {name}")
        require(sum(values) == row["sample_sum_us"], f"sample sum mismatch: {name}")
        require(data.pop("workload") == ("idle" if workload.endswith("-idle") else "market_conversation"),
                f"harness workload mismatch: {name}")
        require(all(row[k] == v for k, v in data.items()), f"metadata mismatch: {name}")
        process = {}
        timing = directory / f"{name}.time"
        labels = ("User time (seconds)", "System time (seconds)",
                  "Maximum resident set size (kbytes)")
        for line in timing.read_text().splitlines():
            for label in labels:
                if line.strip().startswith(label + ":"):
                    process[label] = float(line.split(":", 1)[1].strip())
        require(process == row["process"], f"process metadata mismatch: {name}")
        require("Exit status: 0" in timing.read_text(), f"unsuccessful process: {name}")
        archives[name] = {
            "raw_sha256": hashlib.sha256(raw).hexdigest(),
            "gzip_sha256": sha(archive), "time_sha256": sha(timing),
            "stderr_sha256": sha(directory / f"{name}.stderr"),
        }
        total += n
        by_pair.setdefault((workload, pair), {})[side] = row
    expected_pairs = []
    for (workload, pair), members in by_pair.items():
        old, new = members["m0"], members["m1"]
        require(old["actors"] == new["actors"] and old["placement"] == new["placement"],
                f"population differs in matched pair: {workload}/{pair}")
        expected_pairs.append({
            "workload": workload, "pair": pair,
            "order": ["m0", "m1"] if pair % 2 else ["m1", "m0"],
            "delta_us": {p: new["poll_us"][p] - old["poll_us"][p] for p in QUANTILES},
            "percent_change": {p: (new["poll_us"][p] / old["poll_us"][p] - 1) * 100
                               for p in QUANTILES},
            "counter_differences": {k: {"m0": old[k], "m1": new[k]} for k in COUNTERS
                                    if old[k] != new[k]},
        })
    require(pairs == expected_pairs, "paired deltas or counters differ")
    require(set(summary) == {p["workload"] for p in pairs}, "summary workload set differs")
    for workload, saved in summary.items():
        group = [p for p in pairs if p["workload"] == workload]
        medians = {}
        for side in ("m0", "m1"):
            repeats = [r for r in rows if r["workload"] == workload and r["binary"] == side]
            require(all(all(r[k] == repeats[0][k] for k in COUNTERS) for r in repeats),
                    f"nonrepeatable counters: {workload}/{side}")
            medians[side] = {p: statistics.median(r["poll_us"][p] for r in repeats)
                             for p in QUANTILES}
        require(saved == {
            "pairs": len(group), "same_binary_semantic_counters_repeat": True,
            "per_binary_median_run_us": medians,
            "median_paired_delta_us": {p: statistics.median(r["delta_us"][p] for r in group)
                                       for p in QUANTILES},
            "median_paired_percent_change": {
                p: statistics.median(r["percent_change"][p] for r in group) for p in QUANTILES},
        }, f"summary median mismatch: {workload}")
    if args.check_current:
        paths = subprocess.check_output([
            "/usr/bin/git", "ls-files", "--cached", "--others", "--exclude-standard", "--",
            "Cargo*", "config.ron", "src", "crates", "assets/world", "assets/prompts",
            "assets/sounds/catalog.toml", "lore/characters", "lore/core_lore/occupations.json",
        ], cwd=ROOT, text=True).splitlines()
        current = {p: sha(ROOT / p) for p in sorted(set(paths)) if (ROOT / p).is_file()}
        require(current == identity["source_sha256"], "current source set/hash mismatch")
        for side in ("m0", "m1"):
            row = next(r for r in rows if r["binary"] == side)
            require(sha(row["command"][0]) == identity["binary_sha256"][side],
                    f"current binary hash mismatch: {side}")
        require(sha(Path(__file__).with_name("run_m1_comparison.py")) == identity["runner_sha256"],
                "current runner hash mismatch")
    report = {
        "checked_at_utc": datetime.now(timezone.utc).isoformat(), "result": "passed",
        "directory": str(directory), "smoke_only": smoke,
        "runs": len(rows), "pairs": len(pairs), "raw_poll_samples": total,
        "current_inputs_checked": args.check_current,
        "checks": ["run/order/count", "raw percentiles/sums/metadata", "paired deltas/counters",
                   "repeated counters", "all summary medians", "successful process metadata"],
        "counter_differences_by_pair": {f'{p["workload"]}/{p["pair"]}': p["counter_differences"]
                                        for p in pairs if p["counter_differences"]},
        "archive_hashes": archives, "auditor_sha256": sha(__file__),
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k not in (
        "archive_hashes", "counter_differences_by_pair")}, indent=2))


if __name__ == "__main__":
    main()
