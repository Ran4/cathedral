# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Measure the real M2a1 checkpoint components on a stable release build.

Run after compilation and source edits stop. These are component API costs,
not a complete city checkpoint, host adoption, disk, or renderer benchmark.
"""
from __future__ import annotations

import argparse
import gzip
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess

from run_m1_comparison import ROOT, command, digest, percentiles, sources


def partition(value, path="", samples=None):
    """Separate raw timing arrays from deterministic harness metadata."""
    if samples is None:
        samples = {}
    if isinstance(value, dict):
        return {key: partition(child, f"{path}.{key}" if path else key, samples)
                for key, child in value.items()}
    if path.endswith("_us") and isinstance(value, list):
        if not value or not all(isinstance(n, (int, float))
                                and math.isfinite(n) and n >= 0 for n in value):
            raise RuntimeError(f"invalid timing samples at {path}")
        samples[path] = value
        return {"raw_samples": path}
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise RuntimeError("choose an empty output directory")
    binary = ROOT / "target/release/examples/alibi_checkpoint_cost"
    before = sources()
    binary_before = digest(binary)
    repeats, sample_count = (1, 2) if args.smoke else (3, 100)
    identity = {
        "schema": 1,
        "head": command("/usr/bin/git", "rev-parse", "HEAD"),
        "status": command("/usr/bin/git", "status", "--short"),
        "platform": platform.platform(),
        "cpu": command("/usr/bin/lscpu"),
        "meminfo": Path("/proc/meminfo").read_text(),
        "rustc": command("/home/ran/.cargo/bin/rustc", "-Vv"),
        "source_sha256": before,
        "binary_sha256": binary_before,
        "runner_sha256": digest(__file__),
        "build_environment": {key: os.environ.get(key) for key in (
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL")},
        "smoke_only": args.smoke,
        "measurement": "Sequential release component API calls; raw per-phase timings. "
            "Peak process RSS includes fixture construction and unrelated engine state. "
            "Conservative component heap charges are not measured allocator bytes. "
            "No complete save/load, disk, host, or renderer measurement.",
    }
    rows = []
    by_mode = {mode: [] for mode in ("authored", "maximum")}
    # Alternate modes so each does not always occupy the same thermal position.
    for repeat in range(repeats):
        modes = ["authored", "maximum"] if repeat % 2 == 0 else ["maximum", "authored"]
        for mode in modes:
            name = f"{mode}-{repeat + 1}"
            raw = args.output_dir / f"{name}.json"
            timing = args.output_dir / f"{name}.time"
            stderr = args.output_dir / f"{name}.stderr"
            cmd = [str(binary), "--mode", mode, "--samples", str(sample_count),
                   "--output", str(raw)]
            with stderr.open("w") as errors:
                result = subprocess.run(
                    ["/usr/bin/time", "-v", "-o", str(timing), *cmd], cwd=ROOT,
                    stdout=subprocess.DEVNULL, stderr=errors, timeout=600)
            if result.returncode:
                raise RuntimeError(f"{name} exited {result.returncode}; inspect {stderr}")
            data = json.loads(raw.read_text())
            samples = {}
            metadata = partition(data, samples=samples)
            if len(samples) != 10 or any(len(v) != sample_count for v in samples.values()):
                raise RuntimeError(f"expected five phases for each of two owners: {samples.keys()}")
            if data["mode"] != mode or data["samples"] != sample_count:
                raise RuntimeError("harness options differ from requested workload")
            if mode == "maximum":
                for key, expected in (("active_count", 256), ("recent_count", 4096),
                                      ("retained_count", 256), ("protected_root_count", 256)):
                    if data[key] != expected:
                        raise RuntimeError(f"maximum workload did not reach {key}={expected}")
            process = {}
            for line in timing.read_text().splitlines():
                for label in ("User time (seconds)", "System time (seconds)",
                              "Maximum resident set size (kbytes)"):
                    if line.strip().startswith(label + ":"):
                        process[label] = float(line.split(":", 1)[1].strip())
            raw_hash = digest(raw)
            archive = Path(str(raw) + ".gz")
            with gzip.GzipFile(str(archive), "wb", mtime=0) as stream:
                stream.write(raw.read_bytes())
            raw.unlink()
            row = {
                "name": name, "command": cmd, "exit_code": result.returncode,
                "metadata": metadata, "sample_count_per_phase": sample_count,
                "phase_us": {phase: percentiles(values) for phase, values in samples.items()},
                "process": process,
                "artifacts": {"uncompressed_json_sha256": raw_hash,
                              **{p.name: digest(p) for p in (archive, timing, stderr)}},
            }
            if by_mode[mode] and metadata != by_mode[mode][0]["metadata"]:
                raise RuntimeError(f"semantic counters or byte charges changed in {name}")
            by_mode[mode].append(row)
            rows.append(row)
            (args.output_dir / "RESULTS.json").write_text(json.dumps(rows, indent=2) + "\n")
            print(json.dumps({"name": name, "phase_us": row["phase_us"]}), flush=True)
    summary = {}
    for mode, group in by_mode.items():
        summary[mode] = {
            "runs": len(group), "samples_per_phase": sample_count * len(group),
            "metadata": group[0]["metadata"],
            "median_run_us": {
                phase: {p: statistics.median(row["phase_us"][phase][p] for row in group)
                        for p in ("p50", "p95", "p99", "max")}
                for phase in group[0]["phase_us"]},
            "same_binary_semantic_counters_repeat": True,
        }
    if sources() != before or digest(binary) != binary_before:
        raise RuntimeError("source or executable changed during measurements")
    identity["unchanged_source_and_binary_at_end"] = True
    for filename, value in (("IDENTITY.json", identity), ("SUMMARY.json", summary)):
        (args.output_dir / filename).write_text(json.dumps(value, indent=2) + "\n")


if __name__ == "__main__":
    main()
