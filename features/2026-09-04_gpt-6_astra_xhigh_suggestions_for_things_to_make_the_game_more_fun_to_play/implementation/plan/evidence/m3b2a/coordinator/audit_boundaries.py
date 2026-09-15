"""Require the migration witnesses and predecessor lifetime boundaries to pass."""
import hashlib
import json
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
OWNER = HERE.parent / "owner"
name = sys.argv[1]
result = json.loads((OWNER / f"{name}-result.json").read_bytes())
assert result["exit_code"] == 0 and result["sources_unchanged"]
raw = Path(result["raw_log"]).read_bytes()
assert hashlib.sha256(raw).hexdigest() == result["raw_sha256"]
text = raw.decode("utf-8")
required = [
    "promotion_review_dropped_ticket_keeps_live_root_and_all_original_roles",
    "promotion_review_children_follow_their_world_and_release_observers_wait_for_them",
    "promotion_review_foreign_and_persistent_roots_refuse_without_mutation",
    "promotion_review_worker_resize_and_release_never_debit_the_new_running_group",
    "promotion_review_opaque_admitted_payload_keeps_its_charge_through_disposal",
    "opaque_admitted_owner_promotes_and_keeps_actual_payload_charged_through_drop",
    "promotion_retirement_observation_tracks_children_without_pinning_them",
    "previously_used_retirement_group_cannot_be_rewrapped_for_another_world",
]
prior = [
    "retirement_review_last_worker_owner_pins_charge_without_the_observer_pinning_it",
    "retirement_review_recipe_cannot_outlive_its_charge_or_bypass_a_full_shared_budget",
    "retirement_review_cannot_wrap_a_save_or_load_charge_as_a_retiring_generation",
    "retirement_review_fence_preserves_payloads_until_worker_drain_and_last_endpoint_release",
    "retirement_review_pin_refusals_preserve_the_original_live_mailbox_and_lease",
    "retirement_review_every_terminal_family_is_inert_after_fence_with_live_reused_ids",
    "preparation_review_shutdown_preserves_every_delivery_stage_behind_blocked_retirement",
    "preparation_review_admission_failure_returns_a_disposal_only_candidate_without_freeing_it",
    "real_local_retirement_drains_queues_flushes_promptlog_and_pins_external_owners",
    "retirement_generation_refusal_returns_whole_local_bundle_unchanged",
    "service_factory_refusal_mismatch_unwind_and_repeat_binding_release_all_owners",
    "retained_service_mismatch_keeps_real_services_charged_until_candidate_disposal",
]
for test in required + prior:
    assert re.search(r"test [\w:]+::" + re.escape(test) + r" \.\.\. ok", text), test
print(json.dumps({
    "auditor_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "command_name": name,
    "source_map_sha256": result["source_map_sha256"],
    "raw_log": result["raw_log"], "raw_sha256": result["raw_sha256"],
    "migration_witnesses_passed": required,
    "predecessor_lifetime_boundaries_passed": prior,
    "scope": "Stable group/promotion/retirement identity and existing transport compatibility; no complete App adoption, numerical two-world residency or frame acceptance.",
}, sort_keys=True, indent=2))
