//! Independent migration boundaries. Small charges exercise ownership identity,
//! not the application's complete Running or heap-residency contract.
use super::{Admitted, CheckpointBudget, Cohort, Reservation, RetirementLease};
use std::sync::{Arc, Barrier};

fn retirement(budget: &CheckpointBudget, bytes: usize) -> RetirementLease {
    RetirementLease::new(budget.reserve(Cohort::RetiringGeneration, bytes).unwrap()).unwrap()
}

#[test]
fn promotion_review_dropped_ticket_keeps_live_root_and_all_original_roles() {
    let budget = CheckpointBudget::default();
    let mut running = Some(budget.reserve(Cohort::Running, 100).unwrap());
    let child = running.as_ref().unwrap().sublease(11).unwrap();
    let persistent = budget.reserve_running_overhead(7).unwrap();
    let candidate = budget.reserve(Cohort::LoadCandidate, 200).unwrap();
    let retired = retirement(&budget, 3);
    let before = budget.retained_bytes();
    for _ in 0..3 {
        let ticket = budget
            .prepare_promotion(&mut running, &candidate, &retired)
            .unwrap();
        assert_eq!(budget.retained_bytes(), before);
        assert_eq!(child.cohort(), Cohort::Running);
        assert_eq!(candidate.cohort(), Cohort::LoadCandidate);
        // No explicit cancel call is required to preserve the live owner.
        drop(ticket);
        assert_eq!(running.as_ref().unwrap().bytes(), 100);
        assert!(budget.owns_reservation(running.as_ref().unwrap(), Cohort::Running));
        assert_eq!(budget.cohort_bytes(Cohort::Running), 118);
        assert_eq!(budget.cohort_bytes(Cohort::LoadCandidate), 200);
        assert_eq!(retired.bytes(), 3);
        assert_eq!(budget.retained_bytes(), before);
    }
    drop((running, child, persistent, candidate, retired));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn promotion_review_children_follow_their_world_and_release_observers_wait_for_them() {
    let budget = CheckpointBudget::default();
    let mut running = Some(budget.reserve(Cohort::Running, 100).unwrap());
    let mut old_child = running.as_ref().unwrap().sublease(11).unwrap();
    let persistent = budget.reserve_running_overhead(7).unwrap();
    let candidate = budget.reserve(Cohort::LoadCandidate, 200).unwrap();
    let candidate_child = candidate.sublease(13).unwrap();
    let save = budget.reserve(Cohort::SavePayload, 5).unwrap();
    let retired = retirement(&budget, 3);
    let release = retired.release_observer();
    let delayed_endpoint = retired.clone();
    let before = budget.retained_bytes();
    assert_eq!(before, 339);

    budget
        .prepare_promotion(&mut running, &candidate, &retired)
        .unwrap()
        .commit();
    assert!(running.is_none());
    assert_eq!(budget.retained_bytes(), before);
    assert_eq!(budget.cohort_bytes(Cohort::Running), 220);
    assert_eq!(budget.cohort_bytes(Cohort::RetiringGeneration), 114);
    assert_eq!(budget.cohort_bytes(Cohort::LoadCandidate), 0);
    assert_eq!(budget.cohort_bytes(Cohort::SavePayload), 5);
    assert_eq!(persistent.cohort(), Cohort::Running);
    assert_eq!(candidate.cohort(), Cohort::Running);
    assert_eq!(candidate_child.cohort(), Cohort::Running);
    assert_eq!(old_child.cohort(), Cohort::RetiringGeneration);
    assert_eq!(retired.bytes(), 103);
    assert!(budget.reserve_load_overhead(1).is_err());

    drop(retired);
    assert!(!release.released());
    drop(delayed_endpoint);
    assert_eq!(budget.cohort_bytes(Cohort::RetiringGeneration), 11);
    assert!(
        !release.released(),
        "migrated child still owns retired storage"
    );
    assert!(budget.reserve(Cohort::RetiringGeneration, 3).is_err());
    old_child.resize(17).unwrap();
    assert_eq!(budget.cohort_bytes(Cohort::RetiringGeneration), 17);
    assert_eq!(budget.cohort_bytes(Cohort::Running), 220);
    drop(old_child);
    assert!(release.released());

    // Reusing the cohort and same numeric charge cannot revive an old observer.
    let next_retired = retirement(&budget, 3);
    let next_release = next_retired.release_observer();
    assert!(release.released());
    assert!(!next_release.released());
    drop(next_retired);
    assert!(next_release.released());
    drop((candidate, candidate_child, save));
    assert_eq!(budget.retained_bytes(), persistent.bytes());
    drop(persistent);
    assert_eq!(
        budget.retained_bytes(),
        0,
        "observers must not pin byte charges"
    );
}

#[test]
fn promotion_review_foreign_and_persistent_roots_refuse_without_mutation() {
    let budget = CheckpointBudget::default();
    let foreign = CheckpointBudget::default();
    let mut running = Some(budget.reserve(Cohort::Running, 100).unwrap());
    let mut persistent = Some(budget.reserve_running_overhead(7).unwrap());
    let candidate = budget.reserve(Cohort::LoadCandidate, 200).unwrap();
    let foreign_candidate = foreign.reserve(Cohort::LoadCandidate, 200).unwrap();
    let retired = retirement(&budget, 3);
    let foreign_retired = retirement(&foreign, 3);
    let before = budget.retained_bytes();
    assert!(
        budget
            .prepare_promotion(&mut running, &foreign_candidate, &retired)
            .is_err()
    );
    assert!(
        budget
            .prepare_promotion(&mut running, &candidate, &foreign_retired)
            .is_err()
    );
    assert!(
        foreign
            .prepare_promotion(&mut running, &candidate, &retired)
            .is_err()
    );
    assert!(
        budget
            .prepare_promotion(&mut persistent, &candidate, &retired)
            .is_err()
    );
    assert_eq!(running.as_ref().unwrap().bytes(), 100);
    assert_eq!(persistent.as_ref().unwrap().bytes(), 7);
    assert_eq!(candidate.cohort(), Cohort::LoadCandidate);
    assert_eq!(budget.retained_bytes(), before);
    assert_eq!(retired.bytes(), 3);

    budget
        .prepare_promotion(&mut running, &candidate, &retired)
        .unwrap()
        .commit();
    let next_candidate = budget.reserve(Cohort::LoadCandidate, 9).unwrap();
    let mut promoted = Some(candidate);
    assert!(
        budget
            .prepare_promotion(&mut promoted, &next_candidate, &retired)
            .is_err()
    );
    assert!(
        promoted.is_some(),
        "occupied retirement must leave the live owner intact"
    );
    assert_eq!(next_candidate.cohort(), Cohort::LoadCandidate);
    drop((promoted, next_candidate, retired, persistent));
    assert_eq!(budget.retained_bytes(), 0);
    drop((foreign_candidate, foreign_retired));
    assert_eq!(foreign.retained_bytes(), 0);
}

#[test]
fn promotion_review_worker_resize_and_release_never_debit_the_new_running_group() {
    for _ in 0..32 {
        let budget = Arc::new(CheckpointBudget::default());
        let mut running = Some(budget.reserve(Cohort::Running, 100).unwrap());
        let mut worker_child: Reservation = running.as_ref().unwrap().sublease(11).unwrap();
        let persistent = budget.reserve_running_overhead(7).unwrap();
        let candidate = budget.reserve(Cohort::LoadCandidate, 200).unwrap();
        let retired = retirement(&budget, 3);
        let release = retired.release_observer();
        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = Arc::clone(&barrier);
        let worker = std::thread::spawn(move || {
            // Observe the old role before the race; mutations must resolve its
            // stable identity again while holding the shared accounting lock.
            assert_eq!(worker_child.cohort(), Cohort::Running);
            worker_barrier.wait();
            worker_child.resize(17).unwrap();
            drop(worker_child);
        });
        let ticket = budget
            .prepare_promotion(&mut running, &candidate, &retired)
            .unwrap();
        barrier.wait();
        ticket.commit();
        worker.join().unwrap();
        assert_eq!(budget.cohort_bytes(Cohort::Running), 207);
        assert_eq!(budget.cohort_bytes(Cohort::RetiringGeneration), 103);
        assert_eq!(budget.retained_bytes(), 310);
        assert!(!release.released());
        drop(retired);
        assert!(release.released());
        assert_eq!(budget.retained_bytes(), 207);
        drop((candidate, persistent));
        assert_eq!(budget.retained_bytes(), 0);
    }
}

#[test]
fn promotion_review_opaque_admitted_payload_keeps_its_charge_through_disposal() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Payload {
        bytes: Vec<u8>,
        budget: Arc<CheckpointBudget>,
        dropped: Arc<AtomicBool>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            assert_eq!(self.bytes, [7; 32]);
            assert_eq!(self.budget.cohort_bytes(Cohort::Running), 220);
            assert!(!self.dropped.swap(true, Ordering::SeqCst));
        }
    }
    let budget = Arc::new(CheckpointBudget::default());
    let mut running = Some(budget.reserve(Cohort::Running, 100).unwrap());
    let persistent = budget.reserve_running_overhead(7).unwrap();
    let charge = budget.reserve(Cohort::LoadCandidate, 200).unwrap();
    let child = charge.sublease(13).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let candidate = Admitted::new(
        Payload {
            bytes: vec![7; 32],
            budget: Arc::clone(&budget),
            dropped: Arc::clone(&dropped),
        },
        charge,
    );
    let retired = retirement(&budget, 3);
    candidate
        .prepare_promotion(&budget, &mut running, &retired)
        .unwrap()
        .commit();
    assert!(budget.owns_admitted(&candidate, Cohort::Running));
    assert!(!budget.owns_admitted(&candidate, Cohort::LoadCandidate));
    assert_eq!(candidate.reserved_bytes(), 200);
    assert!(!dropped.load(Ordering::SeqCst));
    drop(candidate);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.cohort_bytes(Cohort::Running), 20);
    assert_eq!(child.cohort(), Cohort::Running);
    drop((child, persistent, retired));
    assert_eq!(budget.retained_bytes(), 0);
}
