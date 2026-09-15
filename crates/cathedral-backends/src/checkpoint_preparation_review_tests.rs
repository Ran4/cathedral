//! Independent real-file tests of delivered ownership through failure/shutdown.
use super::{
    CheckpointPreparation, PreparationOutcome, RetirementStatus, SERVICE_ALLOWANCE_BYTES,
    tests::{Harness, ready, wait},
};
use crate::checkpoint_services::SendContinuationServices;
use cathedral_sim::{
    Capabilities, Cognition, CognitionBusy, NullSight, NullTranscription, NullTts, RequestId,
    checkpoint::{CheckpointBudget, Cohort, MAX_RESIDENT_BYTES},
};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    thread::ThreadId,
    time::{Duration, Instant},
};

// A bounded blocker exercises slot contention. Actual LocalEngine retirement
// has separate host tests; this fixture makes no full-world heap-bound claim.
struct BlockRetirement {
    entered: mpsc::SyncSender<ThreadId>,
    release: mpsc::Receiver<()>,
}
impl Drop for BlockRetirement {
    fn drop(&mut self) {
        let _ = self.entered.send(std::thread::current().id());
        let _ = self.release.recv_timeout(Duration::from_secs(20));
    }
}

struct ObserveServiceDrop {
    dropped: mpsc::SyncSender<ThreadId>,
    budget: Arc<CheckpointBudget>,
    minimum_charge: usize,
}
impl Cognition for ObserveServiceDrop {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        panic!("preparation must not submit cognition")
    }
}
impl Drop for ObserveServiceDrop {
    fn drop(&mut self) {
        assert!(self.budget.retained_bytes() >= self.minimum_charge);
        let _ = self.dropped.send(std::thread::current().id());
    }
}

fn eventually(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "worker disposal deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn preparation_review_shutdown_preserves_every_delivery_stage_behind_blocked_retirement() {
    for stage in 0..4 {
        let harness = Harness::new();
        let budget = harness.fixture.budget.clone();
        let service = CheckpointPreparation::start(budget.clone()).unwrap();
        let baseline = budget.retained_bytes();
        let mut delivered = ready(&service, harness.submit(&service));
        let original_reference = delivered.reference().clone();
        if stage >= 1 {
            delivered.construct().unwrap();
        }
        if stage >= 2 {
            delivered.prepare_continuation().unwrap();
        }
        let (dropped, service_drop) = mpsc::sync_channel(1);
        if stage == 3 {
            delivered
                .bind_services(64 * 1024, |generation| {
                    Ok(SendContinuationServices {
                        generation,
                        cognition: Box::new(ObserveServiceDrop {
                            dropped,
                            budget: budget.clone(),
                            minimum_charge: budget.retained_bytes(),
                        }),
                        transcription: Box::new(NullTranscription),
                        tts: Box::new(NullTts),
                        sight: Box::new(NullSight),
                        capabilities: Capabilities::default(),
                        runtime_dir: PathBuf::new(),
                    })
                })
                .unwrap();
            assert!(delivered.prepared().unwrap().ready_for_adoption());
        }
        assert_eq!(delivered.reference(), &original_reference);
        let retained_save = harness.fixture.capture();
        let after_disposal = baseline - SERVICE_ALLOWANCE_BYTES + retained_save.reserved_bytes();
        let (entered, retire_started) = mpsc::sync_channel(1);
        let (release, gate) = mpsc::sync_channel(1);
        let permit = service.reserve_retirement(64 * 1024).unwrap();
        let retirement_released = permit.lease().release_observer();
        permit.submit(BlockRetirement {
            entered,
            release: gate,
        });
        let host_thread = std::thread::current().id();
        assert_ne!(
            retire_started
                .recv_timeout(Duration::from_secs(20))
                .unwrap(),
            host_thread
        );
        let held = budget.retained_bytes();
        let started = Instant::now();
        service.close();
        drop(service);
        drop(delivered);
        let host_return = started.elapsed();
        // A generous deadlock guard, not acceptance of the 2 ms frame budget.
        assert!(host_return < Duration::from_secs(2));
        assert_eq!(budget.retained_bytes(), held, "stage {stage}");
        assert!(!retirement_released.released());
        assert!(budget.reserve(Cohort::LoadCandidate, 1).is_err());
        assert!(budget.reserve(Cohort::RetiringGeneration, 1).is_err());
        assert!(budget.reserve(Cohort::SavePayload, 1).is_err());
        assert!(service_drop.try_recv().is_err());
        release.send(()).unwrap();
        eventually(|| budget.retained_bytes() == after_disposal);
        assert!(retirement_released.released());
        if stage == 3 {
            assert_ne!(
                service_drop.recv_timeout(Duration::from_secs(1)).unwrap(),
                host_thread
            );
        }
        println!(
            "M3b1 review shutdown stage={stage} held_bytes={held} retained_save_bytes={} host_return_seconds={}",
            retained_save.reserved_bytes(),
            host_return.as_secs_f64()
        );
        drop(retained_save);
        assert_eq!(budget.retained_bytes(), baseline - SERVICE_ALLOWANCE_BYTES);
    }
}

#[test]
fn preparation_review_admission_failure_returns_a_disposal_only_candidate_without_freeing_it() {
    let harness = Harness::new();
    let budget = harness.fixture.budget.clone();
    let service = CheckpointPreparation::start(budget.clone()).unwrap();
    let baseline = budget.retained_bytes();
    let mut delivered = ready(&service, harness.submit(&service));
    delivered.construct().unwrap();
    let id = delivered.id();
    let before_pressure = budget.retained_bytes();
    let pressure = service
        .reserve_retirement(MAX_RESIDENT_BYTES - before_pressure)
        .unwrap();
    let retirement_id = pressure.id();
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    let error = delivered.prepare_continuation().unwrap_err();
    assert_eq!(error.owner, "admission");
    assert!(delivered.hydrated().is_some());
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES);
    drop(pressure);
    assert_eq!(budget.retained_bytes(), before_pressure);
    let error = delivered.prepare_continuation().unwrap_err();
    assert!(error.reason.contains("disposal-only"));
    assert!(delivered.hydrated().is_some());
    drop(delivered);
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    eventually(|| {
        matches!(
            service.take_retirement_result(retirement_id),
            Some(RetirementStatus { released: true, .. })
        )
    });
    assert_eq!(budget.retained_bytes(), baseline);
    assert!(service.join().is_ok());
    assert_eq!(budget.retained_bytes(), baseline - SERVICE_ALLOWANCE_BYTES);
}
