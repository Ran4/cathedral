use super::*;
use crate::checkpoint::{BoundedVec, CheckpointBudget};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[test]
fn subordinate_owner_keeps_slot_and_charge_until_actual_drop() {
    struct ObservedDrop {
        budget: Arc<CheckpointBudget>,
        seen: Arc<AtomicBool>,
    }
    impl Drop for ObservedDrop {
        fn drop(&mut self) {
            assert_eq!(self.budget.retained_bytes(), 16384);
            self.seen.store(true, Ordering::SeqCst);
        }
    }
    let budget = Arc::new(CheckpointBudget::default());
    let parent = budget.reserve(Cohort::SavePayload, 4096).unwrap();
    let child = parent.sublease(16384).unwrap();
    let seen = Arc::new(AtomicBool::new(false));
    let owner = Admitted::new(
        ObservedDrop {
            budget: Arc::clone(&budget),
            seen: Arc::clone(&seen),
        },
        child,
    );
    assert_eq!(budget.retained_bytes(), 20480);
    drop(parent);
    assert!(budget.reserve(Cohort::SavePayload, 1).is_err());
    std::thread::spawn(move || drop(owner)).join().unwrap();
    assert!(seen.load(Ordering::SeqCst));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(budget.peak_retained_bytes(), 20480);
}
#[test]
fn numeric_and_newtype_map_keys_preserve_json_key_semantics() {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
    struct Key(usize);
    let b = CheckpointBudget::default();
    let mut r = b.reserve(Cohort::LoadCandidate, 4096).unwrap();
    let m = meter::DecodeMeter::new(&mut r, 128).unwrap();
    let map: BTreeMap<Key, String> = m.decode(br#"{"0":"zero","25":"twenty five"}"#).unwrap();
    assert_eq!(map[&Key(0)], "zero");
    assert_eq!(map[&Key(25)], "twenty five");
    assert!(m.expanded() >= 11 * std::mem::size_of::<(Key, String)>() + 128);
}
#[test]
fn bounded_vec_minimum_capacity_and_expansion_refusal_precede_element_decode() {
    let b = CheckpointBudget::default();
    let mut r = b.reserve(Cohort::LoadCandidate, 4096).unwrap();
    let m = meter::DecodeMeter::new(&mut r, 1024).unwrap();
    let row = vec![0u64; 32];
    let bytes = serde_json::to_vec(&vec![row]).unwrap();
    let decoded: BoundedVec<[u64; 32], 8> = m.decode(&bytes).unwrap();
    assert_eq!(decoded.0.capacity(), 8);
    assert!(m.expanded() >= decoded.0.capacity() * std::mem::size_of::<[u64; 32]>());
    drop(decoded);
    let prior = m.expanded();
    m.charge(meter::MAX_EXPANSION - prior - 1000).unwrap();
    #[allow(dead_code)]
    struct Large([u64; 512]);
    impl<'de> serde::Deserialize<'de> for Large {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> std::result::Result<Self, D::Error> {
            panic!("unadmitted element visitor ran")
        }
    }
    assert!(m.decode::<Vec<Large>>(b"[null]").is_err());
    assert!(m.expanded() > prior); // disposal never resets complete expansion
}
#[test]
fn typed_wrong_type_and_identifier_diagnostics_are_bounded() {
    let b = CheckpointBudget::default();
    let mut r = b.reserve(Cohort::LoadCandidate, 4096).unwrap();
    let m = meter::DecodeMeter::new(&mut r, 65536).unwrap();
    let large = format!("\"{}\"", "x".repeat(32768));
    let error = m.decode::<u64>(large.as_bytes()).unwrap_err();
    assert!(error.reason.len() <= 512);
    #[derive(Debug, serde::Deserialize)]
    enum Unit {
        One,
    }
    let error = m.decode::<Unit>(large.as_bytes()).unwrap_err();
    assert!(error.reason.len() <= 512);
    #[allow(dead_code)]
    #[derive(Debug, serde::Deserialize)]
    enum Payload {
        One,
        Pair(u64, u64),
        Record { n: u64 },
    }
    for key in ["One", "Pair", "Record"] {
        let input = format!("{{\"{key}\":{large}}}");
        let error = m.decode::<Payload>(input.as_bytes()).unwrap_err();
        assert!(error.reason.len() <= 512);
        assert!(m.diagnostic_scratch() >= 32 * 32768);
    }
}

#[test]
fn complete_world_event_fence_refuses_without_draining_or_writing() {
    let mut world = crate::World::new();
    world.emit(crate::DomainEvent::speech(
        crate::ActorId::from_raw("player"),
        None,
        "pending ordinary speech".into(),
        crate::Vec3::ZERO,
        Vec::new(),
    ));
    let sequence = world.event_sequence;
    let mut output = Vec::new();
    let error = world.complete_write_backbone(&mut output).unwrap_err();
    assert_eq!(error.reason, "unflushed domain events");
    assert!(output.is_empty());
    assert_eq!(world.event_sequence, sequence);
    let events = world.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].text.as_deref(), Some("pending ordinary speech"));
}
