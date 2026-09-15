"""Audit final-source preparation, fresh-process and real-retirement observations."""
import hashlib
import json
import math
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"
name = sys.argv[1]
start = json.loads((OWNER / f"{name}-start.json").read_bytes())
result = json.loads((OWNER / f"{name}-result.json").read_bytes())
raw = Path(start["raw_log"]).read_bytes()
assert hashlib.sha256(raw).hexdigest() == result["raw_sha256"]
assert result["exit_code"] == 0 and result["sources_unchanged"]
text = raw.decode("utf-8")
arrays = re.findall(
    r"M3b1 authored raw phase samples seconds=(\[.*?\]); debug renderer-free fixture, no frame/p99 acceptance",
    text,
)
assert len(arrays) == 1
samples = json.loads(arrays[0])
assert len(samples) == 8
fields = {"host_construction", "host_continuation", "host_return", "worker_disposal", "worker_phases", "worker_total"}
for sample in samples:
    assert set(sample) == fields
    assert len(sample["worker_phases"]) == 5
    values = sample["worker_phases"] + [sample[key] for key in fields - {"worker_phases"}]
    assert all(isinstance(value, (int, float)) and math.isfinite(value) and value >= 0 for value in values)
    assert sum(sample["worker_phases"]) <= sample["worker_total"] + 1e-9

shutdown = [
    {"stage": int(stage), "held_bytes": int(held), "retained_save_bytes": int(saved), "host_return_seconds": float(seconds)}
    for stage, held, saved, seconds in re.findall(
        r"M3b1 review shutdown stage=(\d+) held_bytes=(\d+) retained_save_bytes=(\d+) host_return_seconds=([\d.e+-]+)", text,
    )
]
assert len(shutdown) == 4 and {row["stage"] for row in shutdown} == {0, 1, 2, 3}
shutdown.sort(key=lambda row: row["stage"])
assert len({row["retained_save_bytes"] for row in shutdown}) == 1
assert shutdown[0]["held_bytes"] == shutdown[1]["held_bytes"]
assert shutdown[2]["held_bytes"] - shutdown[1]["held_bytes"] == 128 * 1024
assert shutdown[3]["held_bytes"] - shutdown[2]["held_bytes"] == 64 * 1024
assert all(row["held_bytes"] <= 1024 ** 3 and row["retained_save_bytes"] > 0 for row in shutdown)
assert all(math.isfinite(row["host_return_seconds"]) and 0 <= row["host_return_seconds"] < 2 for row in shutdown)

retirement = re.findall(
    r"local retirement detach_seconds=([\d.e+-]+) worker=Some\(([\d.e+-]+)\); injected PromptLog destructor gate included", text,
)
assert len(retirement) == 1
detach, disposal = map(float, retirement[0])
assert all(math.isfinite(value) and value >= 0 for value in (detach, disposal))
inventory = re.findall(r"retirement fixture inventory=([^\n]+)", text)
assert len(inventory) == 1 and "trusted_bound=167772160" in inventory[0]
assert "fresh-process real file: all sixteen actual owner digests equal; construction and M2c preparation without seed/poll" in text
assert re.search(r"test checkpoint_preparation::tests::actual_saved_file_prepares_all_sixteen_owners_in_a_fresh_process \.\.\. ok", text)
required_tests = [
    "retirement_review_last_worker_owner_pins_charge_without_the_observer_pinning_it",
    "retirement_review_recipe_cannot_outlive_its_charge_or_bypass_a_full_shared_budget",
    "retirement_review_cannot_wrap_a_save_or_load_charge_as_a_retiring_generation",
    "retirement_review_fence_preserves_payloads_until_worker_drain_and_last_endpoint_release",
    "retirement_review_pin_refusals_preserve_the_original_live_mailbox_and_lease",
    "retirement_review_every_terminal_family_is_inert_after_fence_with_live_reused_ids",
    "preparation_review_shutdown_preserves_every_delivery_stage_behind_blocked_retirement",
    "preparation_review_admission_failure_returns_a_disposal_only_candidate_without_freeing_it",
    "store_unlocks_on_owner_disposal_while_a_duplicate_descriptor_survives",
    "post_lock_startup_sync_failure_unlocks_while_a_duplicate_survives",
    "storage_review_damaged_active_can_recover_and_save_again_without_losing_previous",
    "real_local_retirement_drains_queues_flushes_promptlog_and_pins_external_owners",
    "retirement_generation_refusal_returns_whole_local_bundle_unchanged",
    "service_factory_refusal_mismatch_unwind_and_repeat_binding_release_all_owners",
    "retained_service_mismatch_keeps_real_services_charged_until_candidate_disposal",
]
for test in required_tests:
    assert re.search(r"test [\w:]+::" + re.escape(test) + r" \.\.\. ok", text), test

print(json.dumps({
    "auditor_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "command_name": name,
    "source_map_sha256": result["source_map_sha256"],
    "raw_log": start["raw_log"], "raw_sha256": result["raw_sha256"],
    "scope": "Eight debug authored fixture observations and one gated real LocalEngine retirement; no whole-App, heap, populated-city or frame/p99 acceptance.",
    "fresh_process": "Parent captures and writes actual M3a slot; current_exe child reads that slot and compares all sixteen source owner digests after construction, without a poll or seed.",
    "required_boundary_tests_passed": required_tests,
    "worker_phase_order": ["assets", "definitions", "typed_owners", "raw_disposal", "retention"],
    "preparation_samples_seconds": samples,
    "preparation_maxima_seconds": {key: max(sample[key] for sample in samples) for key in sorted(fields - {"worker_phases"})},
    "shutdown_stage_observations": shutdown,
    "shutdown_deadlock_guard_seconds": 2,
    "retirement": {"host_detach_seconds": detach, "worker_disposal_seconds": disposal, "injected_promptlog_gate_included": True, "fixture_inventory_and_assumptions": inventory[0]},
}, sort_keys=True, indent=2))
