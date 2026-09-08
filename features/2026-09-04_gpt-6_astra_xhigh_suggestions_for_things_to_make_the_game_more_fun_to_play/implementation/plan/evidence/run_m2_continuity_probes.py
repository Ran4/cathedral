# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Measure M2a12 floor and Engine continuity on authored and +2,000 resident worlds.

Run only after compilation and source changes stop. These component API costs
exclude complete Engine capture/hydration, host adoption, disk and rendering.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

from run_m1_comparison import command, digest, percentiles
from run_m2_checkpoint_probes import partition
from component_input_sources import ROOT, SOURCE_SCOPE, sources


PHASES = (
    "preflight_us", "export_us", "encode_us", "decode_validate_us",
    "candidate_validate_us", "drop_us",
)
MODES = ("authored", "populated")

# Reviewed functional debug workloads; no private owner mutation in the probe.
EXPECTED_COUNTS = {'authored': {'characters': 520,
              'configured_tts_selected': 'cloud',
              'floor': {'awaiting': 1,
                        'background_awaiting': 0,
                        'background_pacing': False,
                        'event_id_bytes': 8,
                        'foreground_awaiting': 1,
                        'foreground_pacing': True,
                        'player_hold': True},
              'lamp_revision_sent': 1,
              'last_snapshot_revision': 1103,
              'ready_emitted': True,
              'sound_ever_emitted': True,
              'startup_diagnostic_bytes': 0,
              'startup_diagnostics': 0,
              'startup_message_bytes': 0,
              'tts_selected': 'local'},
 'populated': {'characters': 2520,
               'configured_tts_selected': 'cloud',
               'floor': {'awaiting': 1,
                         'background_awaiting': 0,
                         'background_pacing': False,
                         'event_id_bytes': 8,
                         'foreground_awaiting': 1,
                         'foreground_pacing': True,
                         'player_hold': True},
               'lamp_revision_sent': 1,
               'last_snapshot_revision': 5103,
               'ready_emitted': True,
               'sound_ever_emitted': True,
               'startup_diagnostic_bytes': 0,
               'startup_diagnostics': 0,
               'startup_message_bytes': 0,
               'tts_selected': 'local'}}
EXPECTED_WITNESS_HASHES = {'authored': '4defe8f119eca892a1a2e966347dba8e9b2e5b686b6c0c21aaef7b664ec7e84b',
 'populated': '993679db7679e17e80680308de17afa7ae9bf37d170b0ca0df473ccea97a983c'}
EXPECTED_BOUNDARY_HASHES = {'authored': '510ad3ff0161f9d0d122d317709d3a253bb773be73766c727bebb41dd63d277a',
 'populated': 'f0cc2a94ccb3b3703abb790514454d32f07037a069ff58f59d4dd194c3768498'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    if any(args.output_dir.iterdir()):
        raise RuntimeError("choose an empty output directory")
    binary = ROOT / "target/release/examples/alibi_continuity_cost"
    before = sources()
    binary_before = digest(binary)
    runners = [Path(__file__), Path(__file__).with_name("run_m1_comparison.py"),
               Path(__file__).with_name("run_m2_checkpoint_probes.py"),
               Path(__file__).with_name("component_input_sources.py")]
    runner_hashes = {p.name: digest(p) for p in runners}
    repeats, sample_count = (1, 2) if args.smoke else (3, 100)
    identity = {
        "schema": 1,
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "head": command("/usr/bin/git", "rev-parse", "HEAD"),
        "status": command("/usr/bin/git", "status", "--short"),
        "platform": platform.platform(),
        "cpu": command("/usr/bin/lscpu"),
        "meminfo": Path("/proc/meminfo").read_text(),
        "rustc": command("/home/ran/.cargo/bin/rustc", "-Vv"),
        "source_scope": SOURCE_SCOPE,
        "source_sha256": before,
        "binary_sha256": binary_before,
        "runner_sha256": runner_hashes,
        "build_environment": {key: os.environ.get(key) for key in (
            "PATH", "RUSTC", "RUSTDOC", "CARGO_HOME", "LDFLAGS",
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL", "CATHEDRAL_HEADLESS",
            "CATHEDRAL_FAKE_BACKEND")},
        "smoke_only": args.smoke,
        "measurement": "Sequential release Floor/Engine continuity component calls; six raw phases. "
            "Decode and candidate validation include scoped floor, config and player binding gates. "
            "Ordinary speech, scripted provider/TTS values, acknowledgements, voice selection, "
            "sound cooldown and microphone onset establish the boundary through bounded polls. "
            "Original input, every Speech publication and exact initial/boundary records are retained. "
            "Admission bounds are not allocator measurements; process RSS includes Engine setup. "
            "No complete save/load, host, disk or renderer measurement.",
        "unchanged_source_binary_and_runners_at_end": False,
    }
    (args.output_dir / "IDENTITY.json").write_text(json.dumps(identity, indent=2) + "\n")
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
            if data["scenario"] != "continuity-ordinary-voiced-reading-cadence-v1" or counts != EXPECTED_COUNTS[mode]:
                raise RuntimeError("floor/Engine continuity counts differ from the reviewed workload")
            if not (witnesses["poll_count"] == 25
                    and 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                    and witnesses["coarse_discard_diagnostics"] == 0):
                raise RuntimeError("ordinary setup did not retain bounded physical time")
            if len(witnesses["all_speech_messages"]) != 5 or len(witnesses["submitted_prompts"]) != 3:
                raise RuntimeError("original provider input or committed Speech output is missing")
            voices = witnesses["tts_requests"]
            if [v["accepted"] for v in voices] != [True, True, True, False]:
                raise RuntimeError("voiced and reading-fallback work differs")
            if [v["kind"] for v in voices] != ["cloud", "cloud", "local", "local"]:
                raise RuntimeError("queue-time voice selection was not retained")
            witness_hash = hashlib.sha256(json.dumps(
                witnesses, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if witness_hash != EXPECTED_WITNESS_HASHES[mode]:
                raise RuntimeError("ordinary continuity inputs or publications differ")
            boundary_hash = hashlib.sha256(json.dumps(
                data["boundary_continuity"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            if boundary_hash != EXPECTED_BOUNDARY_HASHES[mode]:
                raise RuntimeError("exact boundary pacing, caches or configuration differ")
            cost = data["cost"]
            payload_limit = (128 if extra else 64) * 1024 * 1024
            if not 0 < cost["encoded_bytes"] <= payload_limit:
                raise RuntimeError("component payload exceeds its population budget")
            if not 0 < cost["expanded_upper_bytes"] <= 128 * 1024 * 1024:
                raise RuntimeError("component exceeds aggregate expansion admission")
            if cost["validation_working_bytes"] != 0:
                raise RuntimeError("Continuity validation working charge differs from the reviewed bound")
            if cost["peak_bytes"] != (4096 + 4 * cost["expanded_upper_bytes"]
                    + 3 * cost["encoded_bytes"] + cost["validation_working_bytes"]):
                raise RuntimeError("Continuity component peak accounting is inconsistent")
            retained = data["shared_reserved_peak_excluding_running_bytes"]
            if not retained == 2 * cost["peak_bytes"] <= 1024**3:
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
