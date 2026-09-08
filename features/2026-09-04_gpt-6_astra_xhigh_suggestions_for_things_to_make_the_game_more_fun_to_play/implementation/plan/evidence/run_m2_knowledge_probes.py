# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Measure M2a5 private knowledge and rumor authority on authored and +2,000 resident worlds.

Run only after compilation and source changes stop. These component API costs
exclude complete Engine capture/hydration, host adoption, disk and rendering.
"""
from __future__ import annotations

import argparse
import gzip
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

from run_m1_comparison import ROOT, command, digest, percentiles, sources
from run_m2_checkpoint_probes import partition


PHASES = (
    "preflight_us", "export_us", "encode_us", "decode_validate_us",
    "candidate_validate_us", "drop_us",
)
MODES = ("authored", "populated")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise RuntimeError("choose an empty output directory")
    binary = ROOT / "target/release/examples/alibi_knowledge_cost"
    before = sources()
    binary_before = digest(binary)
    runners = [Path(__file__), Path(__file__).with_name("run_m1_comparison.py"),
               Path(__file__).with_name("run_m2_checkpoint_probes.py")]
    runner_hashes = {p.name: digest(p) for p in runners}
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
        "runner_sha256": runner_hashes,
        "build_environment": {key: os.environ.get(key) for key in (
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL")},
        "smoke_only": args.smoke,
        "measurement": "Sequential release component API calls; six raw phase timings. "
            "Decode and candidate validation include the owner reference/index gates. "
            "The diagnostic deliberately fills six carried holdings per real actor, "
            "warms ward air and retains historical receipts and pending conversation "
            "state before ordinary cache publication. Expanded/admission charges are "
            "conservative bounds, not allocator measurements; process RSS includes "
            "world generation and the running Engine. No complete save/load, host, "
            "disk or renderer measurement.",
    }
    rows = []
    by_mode = {mode: [] for mode in MODES}
    for repeat in range(repeats):
        modes = MODES if repeat % 2 == 0 else tuple(reversed(MODES))
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
                    stdout=subprocess.DEVNULL, stderr=errors, timeout=1200)
            if result.returncode:
                raise RuntimeError(f"{name} exited {result.returncode}; inspect {stderr}")
            data = json.loads(raw.read_text())
            samples = {}
            metadata = partition(data, samples=samples)
            if set(samples) != set(PHASES) or any(
                    len(v) != sample_count for v in samples.values()):
                raise RuntimeError(f"expected six phases of {sample_count} samples")
            if data["mode"] != mode or data["samples"] != sample_count:
                raise RuntimeError("harness options differ from requested workload")
            extra = 2000 if mode == "populated" else 0
            placement = data["placement"]
            if any(placement[key] != expected for key, expected in (
                    ("requested", extra), ("placed", extra), ("unplaced", 0))):
                raise RuntimeError(f"{mode} did not place the requested population")
            if data["counts"]["characters"] <= extra:
                raise RuntimeError("authored characters missing from measured world")
            counts = data["counts"]
            witnesses = data["witnesses"]
            if (data["scenario"] != "carried_news_with_historical_receipts_and_caches"
                    or counts["holding_actors"] != counts["characters"]
                    or counts["holdings"] != 6 * counts["characters"]
                    or not 6 <= counts["facts"] <= 256
                    or not 1 <= counts["air"] <= 8 * 24
                    or not 1 <= counts["player_receipts"] <= 64
                    or not 1 <= counts["seated_keys"] <= 3
                    or counts["occasions"] < 1
                    or counts["journal_cached"] is not True
                    or not 1 <= counts["journal_entries"] <= 24
                    or counts["ward_heat_rows"] != 8):
                raise RuntimeError("expected carried news, pending obligations and published caches")
            if (not 1 <= witnesses["historical_receipts"] <= counts["player_receipts"]
                    or not 1 <= witnesses["historical_seated_keys"] <= counts["seated_keys"]
                    or not 1 <= witnesses["offered_occasions"] <= counts["occasions"]):
                raise RuntimeError("expected retained history and an offered conversation occasion")
            cost = data["cost"]
            payload_limit = (128 if extra else 64) * 1024 * 1024
            if not 0 < cost["encoded_bytes"] <= payload_limit:
                raise RuntimeError("component payload exceeds its population budget")
            if not 0 < cost["expanded_upper_bytes"] <= 128 * 1024 * 1024:
                raise RuntimeError("component exceeds aggregate expansion admission")
            if cost["validation_working_bytes"] != (256 + 4096) * 1024:
                raise RuntimeError("knowledge validation working charge differs from the reviewed bound")
            if cost["peak_bytes"] != (4096 + 4 * cost["expanded_upper_bytes"]
                    + 3 * cost["encoded_bytes"] + cost["validation_working_bytes"]):
                raise RuntimeError("Knowledge component peak accounting is inconsistent")
            retained = data["shared_reserved_peak_excluding_running_bytes"]
            if not cost["peak_bytes"] <= retained <= 1024**3:
                raise RuntimeError("invalid or excessive shared cohort admission")
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
                "phase_us": {phase: percentiles(values)
                             for phase, values in samples.items()},
                "process": process,
                "artifacts": {"uncompressed_json_sha256": raw_hash,
                              **{p.name: digest(p) for p in (archive, timing, stderr)}},
            }
            if by_mode[mode] and metadata != by_mode[mode][0]["metadata"]:
                raise RuntimeError(f"semantic counters or charges changed in {name}")
            by_mode[mode].append(row)
            rows.append(row)
            (args.output_dir / "RESULTS.json").write_text(json.dumps(rows, indent=2) + "\n")
            print(json.dumps({"name": name, "phase_us": row["phase_us"]}), flush=True)
    authored = by_mode["authored"][0]["metadata"]["counts"]["characters"]
    populated = by_mode["populated"][0]["metadata"]["counts"]["characters"]
    if populated - authored != 2000:
        raise RuntimeError("measured character-count difference is not +2,000")
    summary = {
        mode: {
            "runs": len(group), "samples_per_phase": sample_count * len(group),
            "metadata": group[0]["metadata"],
            "median_run_us": {
                phase: {p: statistics.median(row["phase_us"][phase][p] for row in group)
                        for p in ("p50", "p95", "p99", "max")}
                for phase in PHASES},
            "same_binary_semantic_counters_repeat": True,
        }
        for mode, group in by_mode.items()
    }
    if sources() != before or digest(binary) != binary_before or {
            p.name: digest(p) for p in runners} != runner_hashes:
        raise RuntimeError("source, executable or runner changed during measurement")
    identity["unchanged_source_binary_and_runners_at_end"] = True
    for filename, value in (("IDENTITY.json", identity), ("SUMMARY.json", summary)):
        (args.output_dir / filename).write_text(json.dumps(value, indent=2) + "\n")


if __name__ == "__main__":
    main()
