//! Independent M3a review of the pure storage admission boundary.
use super::complete::{
    CompleteCheckpointCandidate, CompleteCheckpointInput, RUNNING_AUTHORITY_ALLOWANCE_BYTES,
};
use super::{Admitted, CheckpointBudget, Cohort, MAX_RESIDENT_BYTES};
use std::sync::Arc;

#[test]
fn storage_identity_survives_worker_transfer_and_rejects_an_equal_foreign_charge() {
    fn assert_send<T: Send>() {}
    assert_send::<Admitted<CompleteCheckpointCandidate>>();
    assert_send::<CompleteCheckpointInput>();

    let budget = Arc::new(CheckpointBudget::default());
    let foreign = CheckpointBudget::default();
    let charge = budget.reserve(Cohort::SavePayload, 8192).unwrap();
    let foreign_charge = foreign.reserve(Cohort::SavePayload, 8192).unwrap();
    let owner = Admitted::new(vec![17u8; 64], charge);
    assert!(budget.owns_admitted(&owner, Cohort::SavePayload));
    assert!(!budget.owns_admitted(&owner, Cohort::LoadCandidate));
    assert!(!foreign.owns_admitted(&owner, Cohort::SavePayload));

    let worker_budget = Arc::clone(&budget);
    let owner = std::thread::spawn(move || {
        assert!(worker_budget.owns_admitted(&owner, Cohort::SavePayload));
        assert_eq!(owner.value().as_slice(), &[17u8; 64]);
        assert!(worker_budget.reserve(Cohort::SavePayload, 1).is_err());
        owner
    })
    .join()
    .unwrap();
    assert_eq!(budget.retained_bytes(), 8192);
    assert!(!foreign.owns_admitted(&owner, Cohort::SavePayload));
    drop(owner);
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(foreign.retained_bytes(), 8192);
    drop(foreign_charge);
}

#[test]
fn storage_running_overhead_cannot_bypass_a_full_budget_or_release_a_live_child() {
    let budget = CheckpointBudget::default();
    assert!(budget.reserve_running_overhead(16384).is_err());
    assert_eq!(budget.retained_bytes(), 0);
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - 32768)
        .unwrap();
    assert!(budget.reserve_running_overhead(0).is_err());
    assert!(budget.reserve_running_overhead(usize::MAX).is_err());
    let mut worker = budget.reserve_running_overhead(16384).unwrap();
    let candidate = budget.reserve(Cohort::LoadCandidate, 16384).unwrap();
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    assert!(budget.reserve_running_overhead(1).is_err());
    assert!(worker.resize(16385).is_err());
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);

    drop(running);
    assert_eq!(budget.retained_bytes(), 32768);
    assert!(budget.reserve(Cohort::Running, 1).is_err());
    drop(worker);
    assert_eq!(budget.retained_bytes(), 16384);
    let replacement = budget.reserve(Cohort::Running, 1).unwrap();
    drop((replacement, candidate));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn storage_envelope_inspection_refuses_scratch_then_preserves_prior_resolver_charge() {
    let budget = CheckpointBudget::default();
    let charge = budget.reserve(Cohort::LoadCandidate, 4098).unwrap();
    let mut bytes = Vec::with_capacity(2);
    bytes.extend_from_slice(b"{}");
    let mut input = CompleteCheckpointInput::from_owned(bytes, charge).unwrap();
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - 4098 - 1024)
        .unwrap();
    let before = budget.retained_bytes();
    let peak = budget.peak_retained_bytes();
    assert_eq!(input.inspect_envelope().unwrap_err().owner, "admission");
    assert_eq!(budget.retained_bytes(), before);
    assert_eq!(budget.peak_retained_bytes(), peak);
    drop(running);

    let error = input.inspect_envelope().unwrap_err();
    assert_eq!(error.owner, "complete");
    assert!(error.reason.contains("missing field"));
    assert!(error.reason.capacity() <= 512);
    assert_eq!(budget.retained_bytes(), 4098);

    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    input.prepare_definition_resolution().unwrap();
    let with_resolver = budget.retained_bytes();
    assert!(with_resolver >= RUNNING_AUTHORITY_ALLOWANCE_BYTES + 64 * 1024 * 1024);
    assert_eq!(input.inspect_envelope().unwrap_err().owner, "complete");
    assert_eq!(budget.retained_bytes(), with_resolver);
    drop(input);
    assert_eq!(budget.retained_bytes(), RUNNING_AUTHORITY_ALLOWANCE_BYTES);
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn storage_hostile_metadata_returns_only_a_bounded_error_and_releases_parse_scratch() {
    let budget = CheckpointBudget::default();
    let capacity = 64 * 1024;
    let charge = budget
        .reserve(Cohort::LoadCandidate, capacity + 4096)
        .unwrap();
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(b"{\"version\":\"");
    bytes.resize(bytes.len() + 32768, b'x');
    bytes.extend_from_slice(
        b"\",\"profile\":{},\"world_identity\":{},\"manifest\":{},\"boundary\":{}",
    );
    for name in [
        "ledger",
        "operations",
        "backbone",
        "round",
        "climate",
        "knowledge",
        "law",
        "marks",
        "animals",
        "social",
        "continuity",
        "scheduler",
        "night",
        "speech",
        "cognition_inputs",
        "host",
    ] {
        bytes.extend_from_slice(b",\"");
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(b"\":{}");
    }
    bytes.push(b'}');
    assert_eq!(bytes.capacity(), capacity);
    let mut input = CompleteCheckpointInput::from_owned(bytes, charge).unwrap();
    let error = input.inspect_envelope().unwrap_err();
    assert_eq!(error.owner, "complete");
    assert!(error.reason.starts_with("invalid type: string"));
    assert!(error.reason.len() <= 512);
    assert!(error.reason.capacity() <= 512);
    assert_eq!(budget.retained_bytes(), capacity + 4096);
    drop(input);
    assert_eq!(budget.retained_bytes(), 0);
    assert!(error.reason.len() <= 512); // Survives without the input or scratch.
}
