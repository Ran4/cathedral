use super::*;
use crate::checkpoint::{CheckpointBudget, Cohort};
fn at(t: f64) -> LogicalTime {
    LogicalTime::new(t).unwrap()
}
fn bytes(f: &ConversationFloor, t: f64) -> Vec<u8> {
    f.export_checkpoint(
        at(t),
        CheckpointBudget::default()
            .reserve(Cohort::SavePayload, 4096)
            .unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
    .value()
    .clone()
}
fn restored(f: &ConversationFloor, t: f64) -> ConversationFloor {
    let raw = bytes(f, t);
    let d = FloorDtoV1::decode(
        &raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        at(t),
    )
    .unwrap()
    .into_candidate(at(t))
    .unwrap();
    let mut scrambled = ConversationFloor {
        awaiting: vec![AwaitedSpeech {
            event_id: SpeechEventId("scrambled".into()),
            deadline: 99.0,
            blocks_player_reaction: true,
        }],
        foreground_floor_until: 88.0,
        background_floor_until: 77.0,
        player_hold_until: 66.0,
    };
    assert_ne!(bytes(&scrambled, t), raw);
    scrambled = copy(d.value().floor());
    assert_eq!(
        bytes(&scrambled, t),
        raw,
        "immediate canonical equality before continuation"
    );
    scrambled
}
fn id(s: &str) -> SpeechEventId {
    SpeechEventId(s.into())
}
#[test]
fn checkpoint_floor_refresh_order_cap_and_trim_before_duplicate_refresh() {
    let mut a = ConversationFloor::new();
    for i in 0..32 {
        a.acquire_scoped(i as f64, &id(&i.to_string()), "", true, i % 2 == 0);
    }
    // Trims oldest BEFORE refreshing an existing later entry, leaving 31 rows.
    a.acquire_scoped(100.0, &id("8"), "", true, false);
    assert_eq!(a.awaiting.len(), 31);
    assert_eq!(a.awaiting[0].event_id, id("1"));
    assert_eq!(a.awaiting[7].event_id, id("8"));
    assert_eq!(a.awaiting[7].deadline, 108.0);
    let mut b = restored(&a, 1.0);
    for f in [&mut a, &mut b] {
        f.acquire(101.0, &id("new"), "", true);
        f.acquire(102.0, &id("1"), "", true);
        assert_eq!(f.awaiting.len(), 32);
        assert_eq!(f.awaiting[0].event_id, id("2"));
        assert_eq!(f.awaiting.last().unwrap().event_id, id("1"));
    }
    assert_eq!(bytes(&a, 1.0), bytes(&b, 1.0));
}
#[test]
fn checkpoint_floor_same_scope_final_ack_beat_unknown_duplicate_and_exact_expiry() {
    let mut a = ConversationFloor::new();
    a.acquire_scoped(0.0, &id("fore1"), "", true, true);
    a.acquire_scoped(0.0, &id("fore2"), "", true, true);
    a.acquire_scoped(0.0, &id("back"), "", true, false);
    let mut b = restored(&a, 0.0);
    for f in [&mut a, &mut b] {
        f.release(1.0, &id("fore1"));
        assert_eq!(f.foreground_floor_until, 0.0);
        f.release(2.0, &id("fore2"));
        assert_eq!(f.foreground_floor_until, 2.4);
        f.release(3.0, &id("fore2"));
        f.release(3.0, &id("unknown"));
        assert_eq!(f.foreground_floor_until, 2.4);
        assert!(!f.busy_for_player_reaction(2.4));
        assert!(f.busy(2.4));
        assert!(!f.busy(8.0));
        assert_eq!(
            f.background_floor_until, 0.0,
            "exact expiry supplies no beat"
        );
        f.release(8.0, &id("back"));
        assert_eq!(f.background_floor_until, 0.0);
    }
    assert_eq!(bytes(&a, 8.0), bytes(&b, 8.0));
}
#[test]
fn checkpoint_floor_reading_background_and_rolling_player_hold() {
    let mut a = ConversationFloor::new();
    a.acquire_scoped(0.0, &id("reading"), "", false, false);
    assert_eq!(a.awaiting_len(), 0);
    let mut b = restored(&a, 0.0);
    for f in [&mut a, &mut b] {
        f.release(0.1, &id("reading"));
        assert!(!f.busy_for_player_reaction(0.1));
        assert!(f.busy(0.1));
        f.bump_player_hold(0.0, 4.0);
        f.bump_player_hold(1.0, 1.0);
        assert_eq!(f.player_hold_until(), 4.0);
        assert!(f.busy_for_player_reaction(3.0));
        f.clear_player_hold();
        assert!(!f.busy(3.0));
        f.acquire_scoped(4.0, &id("audible reading"), "", false, true);
        assert!(f.busy_for_player_reaction(6.9));
        assert!(!f.busy_for_player_reaction(7.0));
    }
    assert_eq!(bytes(&a, 7.0), bytes(&b, 7.0));
}
#[test]
fn checkpoint_floor_exact_future_historic_signed_zero_and_infinite_anchors() {
    let a = ConversationFloor {
        awaiting: vec![AwaitedSpeech {
            event_id: id("\0\n\t odd é 🤔"),
            deadline: f64::MAX,
            blocks_player_reaction: false,
        }],
        foreground_floor_until: -0.0,
        background_floor_until: f64::INFINITY,
        player_hold_until: f64::MAX,
    };
    let b = restored(&a, -0.0);
    assert_eq!(b.foreground_floor_until.to_bits(), (-0.0f64).to_bits());
    assert_eq!(b.background_floor_until, f64::INFINITY);
    assert_eq!(b.awaiting[0].deadline, f64::MAX);
    let raw = bytes(&a, -0.0);
    assert!(
        FloorDtoV1::decode(
            &raw,
            CheckpointBudget::default()
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            at(0.0)
        )
        .is_err()
    );
    // Public calls make finite historical future anchors and infinity directly.
    let mut f = ConversationFloor::new();
    f.acquire(f64::MAX, &id(""), "", true);
    f.bump_player_hold(f64::MAX, f64::MAX);
    assert_eq!(restored(&f, 1.0), f);
}
#[test]
fn checkpoint_floor_refuses_unsupported_without_repair() {
    for bad in [f64::NAN, f64::NEG_INFINITY, -1.0] {
        let f = ConversationFloor {
            player_hold_until: bad,
            ..Default::default()
        };
        assert!(
            f.export_checkpoint(
                at(0.0),
                CheckpointBudget::default()
                    .reserve(Cohort::SavePayload, 4096)
                    .unwrap()
            )
            .is_err()
        );
    }
    let mut f = ConversationFloor::new();
    f.acquire(0.0, &id("same"), "", true);
    f.awaiting.push(f.awaiting[0].clone());
    assert!(validate(&f).is_err());
    f.awaiting.pop();
    f.awaiting[0].event_id = id(&"x".repeat(MAX_EVENT_ID_BYTES + 1));
    assert!(
        f.export_checkpoint(
            at(0.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap()
        )
        .is_err()
    );
    f.awaiting[0].event_id = id("ok");
    f.awaiting.resize(33, f.awaiting[0].clone());
    assert!(validate(&f).is_err());
}
#[test]
fn checkpoint_floor_maximum_string_rows_admission_and_layout() {
    let mut f = ConversationFloor::new();
    for i in 0..MAX_FLOOR_AWAITING {
        f.acquire_scoped(
            0.0,
            &id(&format!(
                "{i:02}{}",
                "é".repeat((MAX_EVENT_ID_BYTES - 2) / 2)
            )),
            "",
            true,
            i % 2 == 0,
        );
    }
    assert_eq!(restored(&f, 0.0), f);
    let b = CheckpointBudget::default();
    let dto = f
        .export_checkpoint(at(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    let cost = dto.value().cost().unwrap();
    assert_eq!(cost.validation_working_bytes, 0);
    assert!(b.retained_bytes() >= cost.peak_bytes);
    drop(dto);
    assert_eq!(b.retained_bytes(), 0);
    assert!(std::mem::size_of::<AwaitedSpeech>() <= 64);
    assert!(std::mem::size_of::<ConversationFloor>() <= 512);
    assert!(std::mem::size_of::<FloorDtoV1>() <= 512);
    println!(
        "continuity_layout awaited={} floor={} floor_dto={} floor_candidate={} string={} vec={} floor_wire={}",
        std::mem::size_of::<AwaitedSpeech>(),
        std::mem::size_of::<ConversationFloor>(),
        std::mem::size_of::<FloorDtoV1>(),
        std::mem::size_of::<FloorCandidate>(),
        std::mem::size_of::<String>(),
        std::mem::size_of::<Vec<AwaitedSpeech>>(),
        std::mem::size_of::<Wire>()
    );
}
fn fixture() -> ConversationFloor {
    let mut f = ConversationFloor::new();
    f.acquire_scoped(0.0, &id("\0 historical"), "", true, false);
    f.acquire(1.0, &id(""), "voiced", true);
    f.acquire_scoped(2.0, &id("\0 historical"), "refreshed", true, true);
    f.acquire_scoped(2.0, &id("reading"), "", false, false);
    f.bump_player_hold(2.0, 5.0);
    f
}
#[test]
fn checkpoint_floor_supported_fixtures() {
    for (name, f, t) in [
        ("floor-initial-v1.json", ConversationFloor::new(), -0.0),
        ("floor-active-v1.json", fixture(), 2.0),
    ] {
        let raw = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/checkpoint_continuity")
                .join(name),
        )
        .unwrap();
        assert_eq!(raw, bytes(&f, t));
        assert_eq!(restored(&f, t), f);
    }
}
#[test]
#[ignore = "explicit new continuity floor fixtures"]
fn regenerate_checkpoint_continuity_floor_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_continuity");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, f, t) in [
        ("floor-initial-v1.json", ConversationFloor::new(), -0.0),
        ("floor-active-v1.json", fixture(), 2.0),
    ] {
        std::fs::write(dir.join(name), bytes(&f, t)).unwrap();
    }
}
#[test]
fn checkpoint_floor_sparse_live_capacity_is_not_copied() {
    let mut text = String::with_capacity(1_000_000);
    text.push_str("id");
    let mut f = ConversationFloor {
        awaiting: Vec::with_capacity(100_000),
        ..Default::default()
    };
    f.awaiting.push(AwaitedSpeech {
        event_id: SpeechEventId(text),
        deadline: 10.0,
        blocks_player_reaction: true,
    });
    let d = f
        .export_checkpoint(
            at(0.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap(),
        )
        .unwrap();
    assert!(d.value().state.awaiting.capacity() <= 4);
    assert_eq!(d.value().state.awaiting[0].event_id.0.capacity(), 2);
}
