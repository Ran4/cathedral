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

from component_input_sources import SOURCE_SCOPE, sources as component_sources

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
    source_scope = identity.get("source_scope", "legacy-component")
    assert source_scope in ("legacy-component", SOURCE_SCOPE)
    if source_scope == SOURCE_SCOPE:
        assert "component_input_sources.py" in identity["runner_sha256"]
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
        validation_working_bytes = cost.get("validation_working_bytes", 0)
        assert isinstance(validation_working_bytes, int) and validation_working_bytes >= 0
        if "validation_working_bytes" in cost:
            assert "definition_working_bytes" not in cost
            counts = data["counts"]
            if data["scenario"] == "forced_storm_with_old_rate_bells":
                assert validation_working_bytes == 64 * 1024
                assert counts["bell_strokes"] == 3
                assert counts["weather_forced"] is True
                assert counts["weather_residue"] is False
            elif data["scenario"] == "carried_news_with_historical_receipts_and_caches":
                assert validation_working_bytes == (256 + 4096) * 1024
                assert counts["holding_actors"] == counts["characters"]
                assert counts["holdings"] == 6 * counts["characters"]
                assert 6 <= counts["facts"] <= 256
                assert 1 <= counts["air"] <= 192
                assert 1 <= counts["player_receipts"] <= 64
                assert 1 <= counts["seated_keys"] <= 3
                assert counts["occasions"] >= 1
                assert counts["journal_cached"] is True
                assert 1 <= counts["journal_entries"] <= 24
                assert counts["ward_heat_rows"] == 8
                witnesses = data["witnesses"]
                assert 1 <= witnesses["historical_receipts"] <= counts["player_receipts"]
                assert 1 <= witnesses["historical_seated_keys"] <= counts["seated_keys"]
                assert 1 <= witnesses["offered_occasions"] <= counts["occasions"]
            elif data["scenario"] == "law-obligations-v1":
                assert validation_working_bytes == 4096 * 1024
                expected_counts = {
                    "arrests": 3, "authored": 8, "cached_custody": True,
                    "cached_notices": 3, "closing": 1, "committed": 9,
                    "custody_records": 11, "dated_unissued_summons": 1,
                    "hearsay": 1, "holders": 1, "law_cached": True,
                    "notices": 4, "served_notices": 3, "served_pairs": 6,
                    "summons": 3, "undated_summons": 1, "warrants": 1,
                }
                assert all(counts[key] == value for key, value in expected_counts.items())
                assert data["witnesses"] == {
                    "historical_notice_links": 1, "historical_officers": 1,
                }
            elif data["scenario"] == "marks-obligations-v1":
                assert validation_working_bytes == 4096 * 1024
                expected_counts = {
                    "marks": 3, "crosses": 1, "tallies": 1, "ward_signs": 1,
                    "households": 1, "places": 2, "faint": 1,
                    "historical_authors": 2, "historical_subjects": 0,
                    "sweep_taken": True, "beat_taken": True,
                    "chalk_cached": True, "cached_pen": True,
                    "cached_anchors": 3 if expected_extra else 1,
                    "cached_kinds": 3 if expected_extra else 1,
                }
                assert all(counts[key] == value for key, value in expected_counts.items())
                witnesses = data["witnesses"]
                for key in ("scrubbed", "same_day_suppressed", "sweep_changed", "cache_deduped"):
                    assert witnesses[key] is True
                assert witnesses["chalk_publications"] == 1
                assert 0 < witnesses["cross_strength"] < 1
                assert witnesses["revision_after_setup"] == (5111 if expected_extra else 1111)
            elif data["scenario"] == "animals-obligations-v1":
                assert validation_working_bytes == 1024 * 1024
                assert counts == {
                    "characters": 520 + expected_extra, "dogs": 10, "decided": 2,
                    "path_dogs": 2, "waypoints": 16, "resting": 8, "moving": 1,
                    "turning": 1, "stop_pending": 0,
                    "dogs_published": True, "engine_nav": True,
                }
                witnesses = data["witnesses"]
                for key in ("initial_all_resting", "turning_observed", "acceleration_observed"):
                    assert witnesses[key] is True
                assert witnesses["initial_resting_publications"] == 1
                assert witnesses["quiet_publications"] == 0
                assert witnesses["movement_publications"] == 27
                assert witnesses["boundary_seconds"] == 6.061000000000001
                assert witnesses["movement_now_seconds"] == 6.0499999999999865
                assert witnesses["cadence_residual_seconds"] == (
                    witnesses["boundary_seconds"] - witnesses["movement_now_seconds"]
                )
                assert 0 < witnesses["cadence_residual_seconds"] < 0.05
            elif data["scenario"] == "night-obligations-v1":
                assert validation_working_bytes == 4096 * 1024
                assert counts == {
                    "characters": 520 + expected_extra, "bedtimes": 30, "queued": 6,
                    "queued_admitted": 0, "queued_people": 0, "queued_wards": 6,
                    "in_flight": True, "held_success": True, "held_error": False,
                    "enabled": True, "seeded": True, "prompt_bytes": 4987,
                    "held_bytes": 60, "stamps": 8, "reflected": 1, "dropped": 0,
                    "ward_moods": 1, "ward_mood_bytes": 39,
                }
                witnesses = data["witnesses"]
                assert witnesses["held_deferred_observed"] is True
                assert witnesses == {
                    "ambient_reroll_observed": True, "busy_admitted_observed": True,
                    "submitted_observed": True, "held_deferred_observed": True,
                    "completed_reflections": 1, "provider_attempts": 3,
                    "provider_submissions": 2, "maximum_submitted_prompt_bytes": 5301,
                    "receipt_recent": 4096, "receipt_retained": 256,
                    "receipt_protected": 19, "boundary_seconds": 20.200000000000003,
                }
            elif data["scenario"] == "social-continuity-v1":
                assert validation_working_bytes == 64 * 1024
                assert counts == {
                    "characters": 520 + expected_extra, "engaged": True,
                    "reciprocal": True, "focus": True, "invitation": True,
                    "next_utterance": 1, "latest_applied_utterance": 1,
                    "warm_pairs": 1, "novelty_told": 1,
                    "novelty_memories": 6 if expected_extra else 2,
                    "witnesses": 6 if expected_extra else 3,
                }
                witnesses = data["witnesses"]
                assert witnesses["partner_retained"] is True
                assert witnesses["boundary_seconds"] == 0.3
                assert witnesses["provider_submissions"] == 1
                assert witnesses["maximum_submitted_prompt_bytes"] == (19029 if expected_extra else 19849)
                assert witnesses["submitted_actor"] == witnesses["partner"]
                assert len(witnesses["speech_messages"]) == 3
                expected_witness_hash = (
                    "d08eeb93362977d208c95e23009e8ea78b5cf821c4093dc4d11968b5e538ae14"
                    if expected_extra else
                    "1a2e46149cb77aa29b8f4fabfdad4535a2bd28a9db6a59b3321dad76c29426b0"
                )
                assert sha(json.dumps(witnesses, sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_witness_hash
            elif data["scenario"] == "scheduler-held-and-retry-v1":
                assert validation_working_bytes == 4096 * 1024
                assert counts == {
                    "characters": 520 + expected_extra, "order_slots": 719 + expected_extra,
                    "round_robin_index": 0, "priority_handoffs": 1, "player_reactions": 1,
                    "retry_work": 1, "in_flight": True, "flight_player_reaction": True,
                    "held_success": True, "held_error": False, "held_bytes": 59,
                    "prompt_bytes": 19295 if expected_extra else 20119,
                    "drained_rows": 1, "drained_bytes": 136 if expected_extra else 141,
                    "presented_rows": 1, "presented_bytes": 136 if expected_extra else 141,
                    "provider_failures": 1, "running": True, "submitted": False,
                }
                witnesses = data["witnesses"]
                assert witnesses["floor_busy_hold"] is True
                assert witnesses["boundary_seconds"] == 5.3
                assert witnesses["provider_submissions"] == 3
                assert witnesses["submitted_actor"] == witnesses["partner"]
                assert witnesses["coarse_discard_diagnostics"] == 0
                assert witnesses["poll_count"] == 136
                assert 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                prompts = witnesses["submitted_prompts"]
                assert len(prompts) == 3
                assert len(prompts[-1][0].encode()) == counts["prompt_bytes"]
                assert witnesses["late_utterance"] not in prompts[-1][0]
                assert len(witnesses["held_reply"].encode()) == counts["held_bytes"]
                assert len(witnesses["speech_messages"]) == 3
                assert witnesses["all_message_digest_algorithm"] == "fnv1a64-debug-stream-v1"
                # The FNV publication digest is diagnostic; this SHA-256 binds
                # all exact primary prompt/budget/text/identity/step witnesses.
                expected_witness_hash = (
                    "2780215cd1a54cc48f1cda9dc1c41d7a236cb0f42606d5c57d3e1a26376655c4"
                    if expected_extra else
                    "1806248a4bcb736d0ea6b4f56203ee0ef28eda102a17d7823c37ba38437a0459"
                )
                assert sha(json.dumps(witnesses, sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_witness_hash
            elif data["scenario"] == "continuity-ordinary-voiced-reading-cadence-v1":
                assert validation_working_bytes == 0
                assert counts == {
                    "characters": 520 + expected_extra,
                    "configured_tts_selected": "cloud", "tts_selected": "local",
                    "floor": {
                        "awaiting": 1, "foreground_awaiting": 1, "background_awaiting": 0,
                        "background_pacing": False, "foreground_pacing": True,
                        "player_hold": True, "event_id_bytes": 8,
                    },
                    "lamp_revision_sent": 1,
                    "last_snapshot_revision": 1103 + 2 * expected_extra,
                    "ready_emitted": True, "sound_ever_emitted": True,
                    "startup_diagnostic_bytes": 0, "startup_diagnostics": 0,
                    "startup_message_bytes": 0,
                }
                witnesses = data["witnesses"]
                assert witnesses["coarse_discard_diagnostics"] == 0
                assert witnesses["poll_count"] == 25
                assert 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                assert witnesses["boundary_seconds"] == 0.72
                assert len(witnesses["submitted_prompts"]) == 3
                assert len(witnesses["all_speech_messages"]) == 5
                voices = witnesses["tts_requests"]
                assert [v["accepted"] for v in voices] == [True, True, True, False]
                assert [v["kind"] for v in voices] == ["cloud", "cloud", "local", "local"]
                assert witnesses["ack_ids"] == [v["event_id"] for v in voices[:2]]
                assert witnesses["all_message_digest_algorithm"] == "fnv1a64-debug-stream-v1"
                expected_witness_hash = (
                    "993679db7679e17e80680308de17afa7ae9bf37d170b0ca0df473ccea97a983c"
                    if expected_extra else
                    "4defe8f119eca892a1a2e966347dba8e9b2e5b686b6c0c21aaef7b664ec7e84b"
                )
                assert sha(json.dumps(witnesses, sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_witness_hash
                expected_boundary_hash = (
                    "f0cc2a94ccb3b3703abb790514454d32f07037a069ff58f59d4dd194c3768498"
                    if expected_extra else
                    "510ad3ff0161f9d0d122d317709d3a253bb773be73766c727bebb41dd63d277a"
                )
                assert sha(json.dumps(data["boundary_continuity"], sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_boundary_hash
                assert data["shared_reserved_peak_excluding_running_bytes"] == 2 * cost["peak_bytes"]
            elif data["scenario"] == "speech-ordinary-interrupted-inputs-v1":
                assert validation_working_bytes == 4096 * 1024
                assert counts == {'accepted_recordings': 3,
                 'available_text_bytes': 85,
                 'available_texts': 1,
                 'basename_bytes': 84,
                 'batch_pending': 2,
                 'captures': 3,
                 'characters': 520 + expected_extra,
                 'parked': 1,
                 'request_id_bytes': 50,
                 'semantic_receipts': 3,
                 'streams': 2,
                 'terminal_receipts': 0,
                 'unique_roots': 2}
                witnesses = data["witnesses"]
                assert witnesses["coarse_discard_diagnostics"] == 0
                assert witnesses["poll_count"] == 17
                assert 0 < witnesses["maximum_poll_step_seconds"] <= 0.05 + 1e-12
                assert witnesses["boundary_seconds"] == 0.34
                assert len(witnesses["submitted_prompts"]) == 2
                assert len(witnesses["all_speech_messages"]) == 2
                assert len(witnesses["submitted_inputs"]) == 13
                assert len(witnesses["recording_receipts"]) == 3
                assert witnesses["provider_submissions"] == 2
                assert witnesses["all_message_digest_algorithm"] == "fnv1a64-debug-stream-v1"
                voices = witnesses["tts_requests"]
                assert len(voices) == 1 and voices[0]["accepted"] and voices[0]["kind"] == "cloud"
                expected_witness_hash = {'authored': '7050dbf38118bd6a20b1e3defd0294149b6c713f97c0291c32e0391ed814c152', 'populated': '24fb8c6a244d97f83920a9daaccd66ea31b6d5b7aba36afcff8c8ea6adf87dca'}[mode]
                assert sha(json.dumps(witnesses, sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_witness_hash
                expected_boundary_hash = {'authored': 'c2ddace801ea300da6ffd66c61e85713d12fdc33f7e5c5934625de0372617240', 'populated': '3db32db9acdf1d93e81e37b3b48f29a530b9941067949c056907835bac9cb3c4'}[mode]
                assert sha(json.dumps(data["boundary_speech"], sort_keys=True,
                                      separators=(",", ":")).encode()) == expected_boundary_hash
                assert data["shared_reserved_peak_excluding_running_bytes"] == 2 * cost["peak_bytes"]
            else:
                raise AssertionError("unrecognized component validation workload")
        assert cost["peak_bytes"] == (
            4096 + 4 * cost["expanded_upper_bytes"] + 3 * cost["encoded_bytes"]
            + definition_working_bytes + validation_working_bytes
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
        if source_scope == SOURCE_SCOPE:
            assert component_sources() == identity["source_sha256"]
        else:
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
        "source_scope": source_scope,
        "input_enumerator_sha256": sha((HERE / "component_input_sources.py").read_bytes()),
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
