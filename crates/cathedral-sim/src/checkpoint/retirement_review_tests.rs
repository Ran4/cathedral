//! Independent review of the lifetime/admission seams used by M3b1 workers.
//! Actual service and Engine disposal is exercised by the host/backend tests.
use super::{
    CheckpointBudget, Cohort, MAX_RESIDENT_BYTES, RetirementLease, RetirementRelease,
    complete::CompleteCheckpointInput,
};
use std::sync::{Arc, mpsc};

#[test]
fn retirement_review_last_worker_owner_pins_charge_without_the_observer_pinning_it() {
    fn send<T: Send>() {}
    send::<RetirementLease>();
    send::<RetirementRelease>();

    let budget = Arc::new(CheckpointBudget::default());
    let foreign = CheckpointBudget::default();
    let lease = RetirementLease::new(
        budget
            .reserve(Cohort::RetiringGeneration, 16 * 1024)
            .unwrap(),
    )
    .unwrap();
    let other = RetirementLease::new(
        foreign
            .reserve(Cohort::RetiringGeneration, 16 * 1024)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(lease.bytes(), other.bytes());
    assert!(budget.owns_retirement_lease(&lease));
    assert!(!budget.owns_retirement_lease(&other));
    assert!(!foreign.owns_retirement_lease(&lease));
    assert!(!lease.same_owner(&other));

    let observer = lease.release_observer();
    let another_observer = observer.clone();
    let worker_owner = lease.clone();
    assert!(lease.same_owner(&worker_owner));
    let worker_budget = Arc::clone(&budget);
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        assert!(worker_budget.owns_retirement_lease(&worker_owner));
        entered.send(()).unwrap();
        wait.recv().unwrap();
        drop(worker_owner);
    });
    ready.recv().unwrap();
    drop(lease);
    assert_eq!(budget.retained_bytes(), 16 * 1024);
    assert!(!observer.released());
    assert!(budget.reserve(Cohort::RetiringGeneration, 1).is_err());
    release.send(()).unwrap();
    worker.join().unwrap();

    assert!(observer.released());
    assert!(another_observer.released());
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(foreign.retained_bytes(), 16 * 1024);
    // Retaining an old completion observer cannot hold the old cohort open,
    // nor can a new reservation make that old observer report active again.
    let replacement = RetirementLease::new(
        budget
            .reserve(Cohort::RetiringGeneration, 16 * 1024)
            .unwrap(),
    )
    .unwrap();
    assert!(observer.released());
    assert!(!replacement.release_observer().released());
    drop((replacement, other));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(foreign.retained_bytes(), 0);
}

#[test]
fn retirement_review_recipe_cannot_outlive_its_charge_or_bypass_a_full_shared_budget() {
    let budget = CheckpointBudget::default();
    let foreign = CheckpointBudget::default();
    assert!(budget.reserve_load_overhead(4096).is_err());
    let charge = budget.reserve(Cohort::LoadCandidate, 4098).unwrap();
    assert!(budget.owns_reservation(&charge, Cohort::LoadCandidate));
    assert!(!budget.owns_reservation(&charge, Cohort::SavePayload));
    assert!(!foreign.owns_reservation(&charge, Cohort::LoadCandidate));
    let mut bytes = Vec::with_capacity(2);
    bytes.extend_from_slice(b"{}");
    let input = CompleteCheckpointInput::from_owned(bytes, charge).unwrap();
    assert!(budget.owns_complete_input(&input));
    assert!(!foreign.owns_complete_input(&input));

    let recipe = budget.reserve_load_overhead(32 * 1024).unwrap();
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - 4098 - 32 * 1024)
        .unwrap();
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    let peak = budget.peak_retained_bytes();
    assert!(budget.reserve_load_overhead(1).is_err());
    assert!(budget.reserve_load_overhead(usize::MAX).is_err());
    assert!(budget.reserve_load_overhead(0).is_err());
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    assert_eq!(budget.peak_retained_bytes(), peak);

    drop(input);
    drop(running);
    assert_eq!(budget.retained_bytes(), 32 * 1024);
    assert!(budget.reserve(Cohort::LoadCandidate, 1).is_err());
    drop(recipe);
    assert_eq!(budget.retained_bytes(), 0);
    let replacement = budget.reserve(Cohort::LoadCandidate, 1).unwrap();
    drop(replacement);
}

#[test]
fn retirement_review_cannot_wrap_a_save_or_load_charge_as_a_retiring_generation() {
    for cohort in [Cohort::Running, Cohort::SavePayload, Cohort::LoadCandidate] {
        let budget = CheckpointBudget::default();
        let charge = budget.reserve(cohort, 4096).unwrap();
        assert!(RetirementLease::new(charge).is_err());
        assert_eq!(budget.retained_bytes(), 0);
    }
}
