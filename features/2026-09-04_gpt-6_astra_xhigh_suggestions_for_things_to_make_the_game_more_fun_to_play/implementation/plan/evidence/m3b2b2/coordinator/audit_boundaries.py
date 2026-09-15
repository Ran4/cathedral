"""Require actual native work and predecessor lifetime witnesses in the final run."""
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
    "native_review_dns_answer_and_executor_keep_separate_charges_after_runtime_join",
    "native_review_cancelled_dns_waiters_keep_slots_and_charge_until_off_frame_join",
    "native_review_recording_disposal_retains_full_queue_and_generation_through_join",
    "dns_answer_retains_capacity_until_actual_iterator_disposal",
    "dns_rejects_retained_host_and_answer_capacity_before_keeping_it",
    "native_close_saturation_is_finite_and_timeout_disposes_its_transport",
    "native_close_cancellation_keeps_endpoint_until_blocked_transport_drop_finishes",
    "native_child_cleanup_slots_include_reaped_but_unjoined_threads",
]
prior = [
    "archive_review_byte_refusal_returns_the_same_input_without_spending_a_filename",
    "archive_review_event_clones_keep_capacity_after_the_writer_finishes",
    "archive_review_forks_preserve_order_and_foreign_sessions_return_the_same_arc",
    "archive_review_actual_admitted_worker_and_permit_keep_the_persistent_charge",
    "archive_review_shared_actor_id_spare_capacity_cannot_bypass_admission",
    "archive_review_unadmitted_shared_input_refuses_and_convenience_owns_a_separate_copy",
    "archive_scheduler_backpressure_retains_held_and_same_poll_exchange_until_last_clone",
    "archive_scheduler_refused_resumed_request_preserves_exact_obligation_and_inputs",
    "archive_scheduler_stale_actor_and_failed_results_keep_their_admitted_archive",
    "archive_night_refusal_preserves_queued_and_resumed_semantics",
    "archive_night_held_result_owns_its_permit_until_the_actual_exchange_dies",
    "archive_complete_held_service_binding_reserves_both_or_preserves_the_candidate",

    "promotion_review_dropped_ticket_keeps_live_root_and_all_original_roles",
    "promotion_review_children_follow_their_world_and_release_observers_wait_for_them",
    "promotion_review_foreign_and_persistent_roots_refuse_without_mutation",
    "promotion_review_worker_resize_and_release_never_debit_the_new_running_group",
    "promotion_review_opaque_admitted_payload_keeps_its_charge_through_disposal",
    "retirement_review_last_worker_owner_pins_charge_without_the_observer_pinning_it",
    "retirement_review_fence_preserves_payloads_until_worker_drain_and_last_endpoint_release",
    "preparation_review_shutdown_preserves_every_delivery_stage_behind_blocked_retirement",
    "preparation_review_admission_failure_returns_a_disposal_only_candidate_without_freeing_it",
    "real_local_retirement_drains_queues_flushes_promptlog_and_pins_external_owners",
    "retirement_generation_refusal_returns_whole_local_bundle_unchanged",
    "service_factory_refusal_mismatch_unwind_and_repeat_binding_release_all_owners",
    "retained_service_mismatch_keeps_real_services_charged_until_candidate_disposal",
    "actual_saved_file_prepares_all_sixteen_owners_in_a_fresh_process",
]
for test in required + prior:
    assert re.search(r"test [\w:]+::" + re.escape(test) + r" \.\.\. ok", text), test
print(json.dumps({
    "auditor_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "command_name": name,
    "source_map_sha256": result["source_map_sha256"],
    "raw_log": result["raw_log"], "raw_sha256": result["raw_sha256"],
    "native_witnesses_passed": required,
    "predecessor_boundaries_passed": prior,
    "scope": "Actual bounded DNS, child/realtime cleanup and STT disposal lifetimes, with shared/generation retention and native joins. No complete App adoption, startup/configuration census, two-world residency or frame acceptance.",
}, sort_keys=True, indent=2))
