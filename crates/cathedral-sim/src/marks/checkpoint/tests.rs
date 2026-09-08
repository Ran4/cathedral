use super::*;
use crate::{
    Vec3,
    checkpoint::{CheckpointBudget, Cohort},
};
use serde_json::{Value, json};
pub(crate) fn actor(s: &str) -> ActorId {
    ActorId::from_raw(s)
}
pub(crate) fn active() -> World {
    let mut w = World::new();
    for name in ["player", "debtor"] {
        w.add_character(crate::notices::checkpoint::tests::person(name, 0.0, None));
    }
    w.current_time = Some(WorldTime::from_game_days(2.25));
    w.places
        .add_home(&actor("debtor"), "The debtor", Vec3::ZERO);
    let cross = draw_or_refresh(
        &mut w,
        MarkKind::ChalkCross,
        MarkAnchor::Household(actor("debtor")),
        Some(actor("old_hand")),
        2.0,
    )
    .unwrap()
    .id;
    w.marks.get_mut(cross).unwrap().strength = 0.349;
    for (kind, name, strength, strokes) in [
        (MarkKind::WellTally, "The lost well", 0.23, 12),
        (MarkKind::WardSign, "The former resort", 0.78, 1),
    ] {
        w.marks.insert(Mark {
            kind,
            anchor: MarkAnchor::Place(name.into()),
            about: Some(actor("historical_subject")),
            author: None,
            drawn_game_days: -2.0,
            last_decayed_game_days: 2.125,
            strength,
            strokes,
        });
    }
    // Remove an allocated row: sparse IDs and a consumed allocator are state.
    let (old, _) = w.marks.insert(w.marks.get(cross).unwrap().clone());
    scrub(&mut w, old);
    w.marks.last_sweep_game_days = 2.25;
    w.marks.last_beat_game_days = 2.125;
    w.marks.decay_scale = 3.5;
    w.marks_enabled = false;
    w.mark_kinds = MarkKindSwitches {
        cross: false,
        tally: true,
        ward_sign: false,
    };
    w
}
fn now() -> LogicalTime {
    LogicalTime::new(10.0).unwrap()
}
fn saved(w: &World) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    w.export_marks_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn decoded(bytes: &[u8], w: &World, b: &CheckpointBudget) -> Result<Admitted<WorldMarksDtoV1>> {
    WorldMarksDtoV1::decode(
        bytes,
        b.reserve(Cohort::LoadCandidate, bytes.len() + 4096)?,
        MarksCheckpointContext::from_world(w, now()),
    )
}
#[test]
fn all_fields_strict_nullable_corruption_and_duplicates() {
    let w = active();
    let original: Value = serde_json::from_slice(saved(&w).value()).unwrap();
    let closed = [
        "",
        "/context",
        "/sampled_time",
        "/marks",
        "/marks/live/1",
        "/marks/live/2",
        "/marks/live/3",
        "/mark_kinds",
    ];
    let mut cases = Vec::new();
    for path in closed {
        for key in original.pointer(path).unwrap().as_object().unwrap().keys() {
            let mut v = original.clone();
            v.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            cases.push(v);
        }
        let mut v = original.clone();
        v.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(0));
        cases.push(v);
    }
    for (path, value) in [
        ("/version", json!(2)),
        ("/boundary", json!(9.0)),
        ("/marks/next_id", json!(0)),
        ("/marks/live/1/strength", json!(1.01)),
        ("/marks/live/1/strokes", json!(2)),
        ("/marks/live/2/strokes", json!(13)),
        ("/marks/live/1/author", json!("")),
        ("/marks/live/1/drawn_game_days", json!(1e100)),
        ("/marks/last_sweep_game_days", json!({"at":null})),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        cases.push(v);
    }
    for v in cases {
        let b = CheckpointBudget::default();
        let bytes = serde_json::to_vec(&v).unwrap();
        assert!(decoded(&bytes, &w, &b).is_err(), "accepted {v}");
        assert_eq!(b.retained_bytes(), 0);
    }
    let wire = String::from_utf8(saved(&w).value().clone()).unwrap();
    let row = serde_json::to_string(&original["marks"]["live"]["1"]).unwrap();
    let duplicate = wire.replace("\"live\":{", &format!("\"live\":{{\"1\":{row},"));
    for (bytes, reason) in [
        (
            wire.replace("\"next_id\":4", "\"next_id\":4,\"next_id\":4"),
            "duplicate field",
        ),
        (duplicate, "duplicate owner key"),
    ] {
        let b = CheckpointBudget::default();
        let error = decoded(bytes.as_bytes(), &w, &b).unwrap_err().to_string();
        assert!(error.contains(reason), "{error}");
        assert_eq!(b.retained_bytes(), 0);
    }
    // Explicit nulls, including absent historical authors, remain supported.
    assert!(decoded(wire.as_bytes(), &w, &CheckpointBudget::default()).is_ok());
}
#[test]
fn preflight_refusal_releases_and_raw_charge_survives_candidate() {
    let w = active();
    let mut raw = saved(&w).value().clone();
    let canonical = raw.len();
    raw.extend(std::iter::repeat_n(b' ', 1024 * 1024));
    let b = CheckpointBudget::default();
    let d = decoded(&raw, &w, &b).unwrap();
    let expected = d.reserved_bytes();
    let c = d
        .into_candidate(MarksCheckpointContext::from_world(&w, now()))
        .unwrap();
    assert_eq!(c.reserved_bytes(), expected);
    assert!(expected >= 4_194_304 + 3 * (canonical + 1024 * 1024));
    drop(c);
    assert_eq!(b.retained_bytes(), 0);
    let large = format!("[{}]", "{},".repeat(270_000) + "{}");
    let err = decoded(large.as_bytes(), &w, &b).unwrap_err().to_string();
    assert!(err.contains("aggregate expanded"), "{err}");
    assert_eq!(b.retained_bytes(), 0);
    let deep = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    assert!(
        decoded(deep.as_bytes(), &w, &b)
            .unwrap_err()
            .to_string()
            .contains("nesting")
    );
    assert_eq!(b.retained_bytes(), 0);
    let held = b
        .reserve(Cohort::Running, checkpoint::MAX_RESIDENT_BYTES - 4096)
        .unwrap();
    assert!(
        w.export_marks_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
    drop(held);
    assert_eq!(b.retained_bytes(), 0);
}
#[test]
fn catalog_binding_retains_order_docs_and_rejects_nonfinite_without_null_collision() {
    let mut w = active();
    let bytes = saved(&w);
    let original = w.mark_catalog.clone();
    for change in 0..5 {
        let mut catalog = (*original).clone();
        let s = catalog.kinds.get_mut(&MarkKind::WardSign).unwrap();
        match change {
            0 => s._places_doc = Some("different retained documentation".into()),
            1 => s
                .places
                .insert("unused".into(), "Unknown resort".into())
                .map(|_| ())
                .unwrap_or(()),
            2 => s.anchors.push(AnchorSlot::Place),
            3 => s.half_life_days_dry = f64::INFINITY,
            _ => s.sheltered_multiplier = f64::NAN,
        }
        w.mark_catalog = std::sync::Arc::new(catalog);
        let b = CheckpointBudget::default();
        assert!(decoded(bytes.value(), &w, &b).is_err());
        assert_eq!(b.retained_bytes(), 0);
    }
    w.mark_catalog = original;
    assert!(decoded(bytes.value(), &w, &CheckpointBudget::default()).is_ok());
}
#[test]
fn allocation_cap_eviction_and_last_supported_allocator_continue() {
    let mut w = active();
    w.marks = Marks::default();
    let row = Mark {
        kind: MarkKind::ChalkCross,
        anchor: MarkAnchor::Household(actor("debtor")),
        about: Some(actor("debtor")),
        author: None,
        drawn_game_days: 2.0,
        last_decayed_game_days: 2.0,
        strength: 0.5,
        strokes: 1,
    };
    for _ in 0..MARKS_MAX {
        w.marks.insert(row.clone());
    }
    w.marks.next_id = u64::MAX - HANDLE_HEADROOM;
    let bytes = saved(&w);
    let b = CheckpointBudget::default();
    let d = decoded(bytes.value(), &w, &b)
        .unwrap()
        .into_candidate(MarksCheckpointContext::from_world(&w, now()))
        .unwrap();
    let mut resumed = d.value().marks().clone();
    for i in 0..HANDLE_HEADROOM {
        let a = w.marks.insert(row.clone());
        let c = resumed.insert(row.clone());
        assert_eq!(a, c);
        assert_eq!(a.0.0, u64::MAX - HANDLE_HEADROOM + i + 1);
        assert!(a.1.is_some());
        if i == 0 {
            assert_eq!(a.1, Some(MarkId(1)));
        }
    }
    assert_eq!(w.marks, resumed);
    assert_eq!(w.marks.len(), MARKS_MAX);
    assert!(
        w.export_marks_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
}
#[test]
fn supported_world_marks_fixture() {
    assert_eq!(
        saved(&active()).value().as_slice(),
        include_bytes!("../../../tests/fixtures/checkpoint_v1/world_marks.json")
    );
}
#[test]
#[ignore = "create only the new M2a7 fixture"]
fn create_world_marks_fixture() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/world_marks.json"
        ),
        saved(&active()).value(),
    )
    .unwrap();
}
#[test]
#[ignore = "closed owner layout diagnostic"]
fn checkpoint_marks_layout() {
    println!(
        "Marks={} Mark={} Anchor={} Switches={} WorldDto={} WorldCandidate={} EngineDto={} EngineCandidate={} EngineMessage={} ChalkableHere={} OptionEngineMessage={}",
        std::mem::size_of::<Marks>(),
        std::mem::size_of::<Mark>(),
        std::mem::size_of::<MarkAnchor>(),
        std::mem::size_of::<MarkKindSwitches>(),
        std::mem::size_of::<WorldMarksDtoV1>(),
        std::mem::size_of::<WorldMarksCandidate>(),
        std::mem::size_of::<crate::engine::marks_checkpoint::EngineMarksDtoV1>(),
        std::mem::size_of::<crate::engine::marks_checkpoint::EngineMarksCandidate>(),
        std::mem::size_of::<crate::EngineMessage>(),
        std::mem::size_of::<crate::engine::ChalkableHere>(),
        std::mem::size_of::<Option<crate::EngineMessage>>()
    );
}

#[test]
fn nonempty_private_owner_scramble_restores_before_ordinary_continuation() {
    let mut control = active();
    let mut resumed = active();
    let bytes = saved(&control);
    let b = CheckpointBudget::default();
    let c = MarksCheckpointContext::from_world(&resumed, now());
    let d = decoded(bytes.value(), &resumed, &b)
        .unwrap()
        .into_candidate(c)
        .unwrap();
    resumed.marks = Marks::default();
    resumed.marks_enabled = true;
    resumed.mark_kinds = MarkKindSwitches::default();
    resumed.marks = d.value().data.marks.clone();
    resumed.marks_enabled = d.value().data.marks_enabled;
    resumed.mark_kinds = d.value().data.mark_kinds;
    assert_eq!(saved(&resumed).value(), bytes.value());
    for days in [2.25, 2.2501, 2.5] {
        assert_eq!(sweep(&mut control, days), sweep(&mut resumed, days));
        assert_eq!(saved(&control).value(), saved(&resumed).value());
    }
    assert_eq!(control.marks.len(), 1);
    for w in [&mut control, &mut resumed] {
        w.marks_enabled = true;
        w.mark_kinds.cross = true;
    }
    assert_eq!(
        binding_mark_about(&control, MarkKind::ChalkCross, &actor("debtor")),
        binding_mark_about(&resumed, MarkKind::ChalkCross, &actor("debtor"))
    );
    let anchor = MarkAnchor::Household(actor("debtor"));
    let a = draw_or_refresh(
        &mut control,
        MarkKind::ChalkCross,
        anchor.clone(),
        None,
        1.0,
    )
    .unwrap();
    let c = draw_or_refresh(&mut resumed, MarkKind::ChalkCross, anchor, None, 1.0).unwrap();
    assert_eq!(a, c);
    assert!(!a.fresh);
    assert_eq!(control.marks.get(a.id).unwrap().drawn_game_days, 2.0);
    assert_eq!(control.world_revision, resumed.world_revision);
    assert_eq!(scrub(&mut control, a.id), scrub(&mut resumed, c.id));
    assert_eq!(control.world_revision, resumed.world_revision);
    assert_eq!(saved(&control).value(), saved(&resumed).value());
}
#[test]
fn zero_tally_retains_abandoned_stroke_reading() {
    let mut w = active();
    let id = MarkId(2);
    w.marks.get_mut(id).unwrap().strokes = 0;
    let bytes = saved(&w);
    let b = CheckpointBudget::default();
    let c = decoded(bytes.value(), &w, &b)
        .unwrap()
        .into_candidate(MarksCheckpointContext::from_world(&w, now()))
        .unwrap();
    assert_eq!(c.value().marks().get(id).unwrap().strokes, 0);
    assert_eq!(
        stroke_count(c.value().marks().get(id).unwrap().strokes),
        "no strokes yet"
    );
}
