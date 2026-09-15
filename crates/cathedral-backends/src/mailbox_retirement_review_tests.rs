//! Independent M3b1 review of real mailbox payloads and retirement lifetimes.
use crate::{BackendEvent, backend_channel_for, mailbox::RetirementPinError};
use cathedral_sim::{
    Completion, RealtimeResult, RequestId, RuntimeGeneration, SpeechEventId, TranscriptionJobId,
    checkpoint::{CheckpointBudget, Cohort, RetirementLease},
};
use std::sync::Arc;

const FIXTURE_RETIREMENT_BYTES: usize = 8 * 1024 * 1024;

fn lease(budget: &CheckpointBudget) -> RetirementLease {
    RetirementLease::new(
        budget
            .reserve(Cohort::RetiringGeneration, FIXTURE_RETIREMENT_BYTES)
            .unwrap(),
    )
    .unwrap()
}

fn reply(id: u64) -> BackendEvent {
    BackendEvent::LlmCompletion(Completion {
        request_id: RequestId(id),
        result: Ok("wait {}".into()),
        duration_seconds: 0.0,
    })
}

#[test]
fn retirement_review_fence_preserves_payloads_until_worker_drain_and_last_endpoint_release() {
    let budget = Arc::new(CheckpointBudget::default());
    let pin = lease(&budget);
    let released = pin.release_observer();
    let (old, receiver) = backend_channel_for(RuntimeGeneration(401));
    let retained_receiver = receiver.clone();
    let delayed_job = old.reserve(reply(7)).unwrap();
    let samples: Arc<[i16]> = vec![11; 4096].into();
    let samples_lifetime = Arc::downgrade(&samples);
    let wav: Arc<[u8]> = vec![19; 4096].into();
    let wav_lifetime = Arc::downgrade(&wav);
    assert!(old.try_send(BackendEvent::TtsChunk {
        event_id: SpeechEventId("old-voice".into()),
        seq: 0,
        sample_rate: 24_000,
        samples,
    }));
    assert!(old.try_send(BackendEvent::TtsDone {
        event_id: SpeechEventId("old-voice".into()),
        result: Ok(wav),
    }));
    assert_eq!(receiver.len(), 2);
    let before = receiver.usage();
    receiver.pin_retirement(&budget, &pin).unwrap();
    receiver.fence();
    assert!(!old.is_active());
    assert_eq!(receiver.len(), 2);
    assert_eq!(receiver.usage(), before);
    assert!(samples_lifetime.upgrade().is_some());
    assert!(wav_lifetime.upgrade().is_some());
    drop(pin);

    let host_thread = std::thread::current().id();
    let worker = std::thread::spawn(move || {
        assert_ne!(std::thread::current().id(), host_thread);
        // retire() includes the drain; only fence() belongs to the host frame.
        receiver.retire();
        assert!(samples_lifetime.upgrade().is_none());
        assert!(wav_lifetime.upgrade().is_none());
        assert_eq!(receiver.usage().records, 1); // the accepted delayed job
        drop((old, receiver));
    });
    worker.join().unwrap();
    assert!(retained_receiver.is_empty());
    assert!(!released.released());
    assert_eq!(budget.retained_bytes(), FIXTURE_RETIREMENT_BYTES);
    assert!(budget.reserve(Cohort::RetiringGeneration, 1).is_err());

    let (fresh, fresh_receiver) = backend_channel_for(RuntimeGeneration(402));
    let new_job = fresh.reserve(reply(7)).unwrap();
    assert!(!delayed_job.try_send(reply(7)));
    assert!(new_job.try_send(reply(7)));
    assert_eq!(fresh_receiver.try_recv().unwrap(), reply(7));
    assert!(retained_receiver.is_empty());
    drop(delayed_job);
    assert!(!released.released()); // the old receiver clone still owns the pin
    drop(retained_receiver);
    assert!(released.released());
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn retirement_review_pin_refusals_preserve_the_original_live_mailbox_and_lease() {
    let budget = CheckpointBudget::default();
    let foreign = CheckpointBudget::default();
    let pin = lease(&budget);
    let other = lease(&foreign);
    let released = pin.release_observer();
    let other_released = other.release_observer();
    let (sender, receiver) = backend_channel_for(RuntimeGeneration(411));
    assert_eq!(
        sender.pin_retirement(&budget, &other),
        Err(RetirementPinError::WrongBudget)
    );
    assert!(sender.is_active());
    assert!(sender.try_send(reply(1)));
    sender.pin_retirement(&budget, &pin).unwrap();
    receiver.pin_retirement(&budget, &pin.clone()).unwrap();
    assert_eq!(
        receiver.pin_retirement(&foreign, &other),
        Err(RetirementPinError::AlreadyPinned)
    );
    assert!(sender.is_active());
    assert_eq!(receiver.try_recv().unwrap(), reply(1));
    drop((pin, other));
    assert!(other_released.released());
    assert_eq!(foreign.retained_bytes(), 0);
    assert!(!released.released());
    assert_eq!(budget.retained_bytes(), FIXTURE_RETIREMENT_BYTES);
    receiver.fence();
    drop((sender, receiver));
    assert!(released.released());
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn retirement_review_every_terminal_family_is_inert_after_fence_with_live_reused_ids() {
    let cases = [
        reply(9),
        BackendEvent::TranscriptionDone {
            job: TranscriptionJobId(9),
            result: Ok("old transcript".into()),
        },
        BackendEvent::RealtimeResult(RealtimeResult::Transcript {
            key: "recording-9".into(),
            text: "old realtime transcript".into(),
        }),
        BackendEvent::RealtimeResult(RealtimeResult::Failure {
            key: Some("recording-9".into()),
            reason: "old failure".into(),
        }),
        BackendEvent::TtsDone {
            event_id: SpeechEventId("voice-9".into()),
            result: Ok(Arc::from([0u8; 32])),
        },
        BackendEvent::TtsStreamEnd {
            event_id: SpeechEventId("voice-9".into()),
            chunk_count: 1,
            first_chunk_ms: 12,
        },
    ];
    for event in cases {
        let budget = CheckpointBudget::default();
        let pin = lease(&budget);
        let released = pin.release_observer();
        let (old, old_receiver) = backend_channel_for(RuntimeGeneration(421));
        let (fresh, fresh_receiver) = backend_channel_for(RuntimeGeneration(422));
        let delayed = old.reserve(event.clone()).unwrap();
        let current = fresh.reserve(event.clone()).unwrap();
        old.pin_retirement(&budget, &pin).unwrap();
        old_receiver.fence();
        assert!(!delayed.try_send(event.clone()), "{event:?}");
        assert!(old_receiver.try_recv().is_err());
        assert!(current.try_send(event.clone()), "{event:?}");
        assert_eq!(fresh_receiver.try_recv().unwrap(), event);
        assert!(fresh_receiver.try_recv().is_err());
        drop((old, old_receiver, pin));
        assert!(!released.released());
        drop(delayed);
        assert!(released.released());
        assert_eq!(budget.retained_bytes(), 0);
    }
}
