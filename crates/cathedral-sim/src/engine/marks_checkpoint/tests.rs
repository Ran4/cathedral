use super::*;
use crate::{
    NullSight, NullTranscription, NullTts,
    checkpoint::{CheckpointBudget, Cohort},
};
use serde_json::json;

struct Unavailable;
impl Cognition for Unavailable {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        Err(crate::CognitionBusy)
    }
}
fn engine() -> Engine {
    Engine::new(
        EngineConfig {
            fake_mode: true,
            clock: WorldClock::new(60.0, Office::Dayspring, 2, 0.05),
            ..Default::default()
        },
        &crate::WorldSeed::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/demo_seed.json"
        )))
        .unwrap(),
        AreaMap::from_json_str(include_str!("../../../../../assets/world/areas.json")).unwrap(),
        SoundCatalog::from_toml_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sounds/catalog.toml"
        )))
        .unwrap(),
        PromptEnv::new(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/turn.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/night.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/strings.toml"
            )),
        )
        .unwrap(),
        Box::new(Unavailable),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::default(),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}

fn logical(n: f64) -> LogicalTime {
    LogicalTime::new(n).unwrap()
}
fn bytes(e: &Engine, now: f64) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    e.export_marks_checkpoint(logical(now), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn candidate(e: &Engine, now: f64) -> Admitted<EngineMarksCandidate> {
    let b = CheckpointBudget::default();
    let saved = bytes(e, now);
    let c = MarksCheckpointContext::from_world(&e.world, logical(now));
    EngineMarksDtoV1::decode(
        saved.value(),
        b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap()
}

use owner::tests::actor;
fn prefix(e: &mut Engine) {
    e.poll(0.0, vec![]);
    e.world
        .add_character(crate::notices::checkpoint::tests::person(
            "debtor", 0.0, None,
        ));
    e.world
        .places
        .add_home(&actor("debtor"), "The debtor", Vec3::ZERO);
    e.world
        .characters
        .get_mut(&actor("player"))
        .unwrap()
        .state
        .position_m = Vec3::ZERO;
    e.publish_chalk_standing(&mut Vec::new());
    assert!(
        matches!(&e.last_chalk_standing,Some(EngineMessage::ChalkStanding {anchors,..}) if anchors.len()==1)
    );
    let days = e.world.current_time.unwrap().game_days();
    e.world
        .notices
        .raise(
            "The debtor".into(),
            "unpaid debt".into(),
            None,
            None,
            Some(days - 3.0),
            actor("historical_officer"),
            Some(actor("debtor")),
            None,
            None,
        )
        .unwrap();
    notices::chalk_the_debtors(&mut e.world, days);
    let id = e
        .world
        .marks
        .find(
            crate::marks::MarkKind::ChalkCross,
            &crate::marks::MarkAnchor::Household(actor("debtor")),
        )
        .unwrap()
        .0;
    crate::marks::scrub(&mut e.world, id);
    // Spent daily beat + consumed ID, but empty clean wall is a real state.
    e.world.marks.decay_scale = 2.5;
    e.world.mark_kinds.tally = false;
}
// Component-only install; every independent Engine has driven its own prefix.
// Other owners, saved climate, registry and revision are matched independently.
fn install(d: &EngineMarksCandidate, e: &mut Engine) {
    let d = &d.data;
    e.world.marks = crate::marks::Marks::default();
    e.world.marks_enabled = !d.world.marks_enabled;
    e.world.mark_kinds = crate::marks::MarkKindSwitches {
        cross: false,
        tally: false,
        ward_sign: false,
    };
    e.config.player_id = actor("wrong");
    e.config.marks_enabled = !d.config_marks_enabled;
    e.config.mark_kinds = crate::marks::MarkKindSwitches {
        cross: false,
        tally: false,
        ward_sign: false,
    };
    e.config.marks_decay_scale = -55.0;
    e.last_chalk_standing = None;
    e.world.marks = d.world.marks.clone();
    e.world.marks_enabled = d.world.marks_enabled;
    e.world.mark_kinds = d.world.mark_kinds;
    e.config.player_id = d.player_id.clone();
    e.config.marks_enabled = d.config_marks_enabled;
    e.config.mark_kinds = d.config_mark_kinds;
    e.config.marks_decay_scale = d.config_marks_decay_scale;
    e.last_chalk_standing = d.last_chalk_standing.clone();
    assert_eq!(
        bytes(e, d.boundary.seconds()).value(),
        &serde_json::to_vec(d).unwrap()
    );
}
#[test]
fn marks_scrub_daily_decay_and_cache_continuation_after_scrambled_install() {
    let mut control = engine();
    let mut resumed = engine();
    prefix(&mut control);
    prefix(&mut resumed);
    let d = candidate(&control, 0.0);
    install(d.value(), &mut resumed);
    let rev = control.world.world_revision;
    for now in [0.01, 0.02, 1.0] {
        let a = control.poll(now, vec![]);
        let b = resumed.poll(now, vec![]);
        assert_eq!(a, b);
        assert!(
            control.world.marks.is_empty(),
            "spent daily beat must preserve the scrub window"
        );
        assert_eq!(bytes(&control, now).value(), bytes(&resumed, now).value());
    }
    assert_eq!(control.world.world_revision, rev);
    let mut pubs = 0;
    for now in [61.0, 61.01, 91.0, 121.0] {
        let a = control.poll(now, vec![]);
        let b = resumed.poll(now, vec![]);
        assert_eq!(a, b);
        pubs += a
            .iter()
            .filter(|m| matches!(m, EngineMessage::ChalkStanding { .. }))
            .count();
        assert_eq!(bytes(&control, now).value(), bytes(&resumed, now).value());
        assert_eq!(control.world.world_revision, resumed.world.world_revision);
    }
    assert!(pubs > 0);
    let (id, m) = control.world.marks.iter().next().unwrap();
    assert_eq!(id.0, 2);
    assert_eq!(m.kind, crate::marks::MarkKind::ChalkCross);
    assert!(control.world.world_revision > rev);
}
#[test]
fn virgin_and_stale_chalk_cache_publish_exactly_once() {
    let mut e = engine();
    assert!(candidate(&e, 0.0).value().last_chalk_standing().is_none());
    prefix(&mut e);
    let cached = e.last_chalk_standing.clone();
    e.world.marks_enabled = false;
    let c = candidate(&e, 0.0);
    assert_eq!(c.value().last_chalk_standing(), cached.as_ref());
    install(c.value(), &mut e);
    let a = e.poll(0.01, vec![]);
    assert_eq!(
        a.iter()
            .filter(|m| matches!(m, EngineMessage::ChalkStanding { .. }))
            .count(),
        1
    );
    let b = e.poll(0.02, vec![]);
    assert!(
        !b.iter()
            .any(|m| matches!(m, EngineMessage::ChalkStanding { .. }))
    );
}
#[test]
fn every_engine_field_and_nullable_cache_are_required() {
    let mut e = engine();
    prefix(&mut e);
    let original: serde_json::Value = serde_json::from_slice(bytes(&e, 0.0).value()).unwrap();
    let mut cases = Vec::new();
    for path in [
        "",
        "/last_chalk_standing",
        "/last_chalk_standing/anchors/0",
        "/config_mark_kinds",
    ] {
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
        ("/player_id", json!("missing")),
        ("/last_chalk_standing/anchors/0/kinds", json!([])),
        (
            "/last_chalk_standing/anchors/0/kinds",
            json!(["chalk_cross", "chalk_cross"]),
        ),
        ("/config_marks_decay_scale", json!(null)),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        cases.push(v);
    }
    for v in cases {
        let b = CheckpointBudget::default();
        let wire = serde_json::to_vec(&v).unwrap();
        assert!(
            EngineMarksDtoV1::decode(
                &wire,
                b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
                MarksCheckpointContext::from_world(&e.world, logical(0.0))
            )
            .is_err(),
            "{v}"
        );
        assert_eq!(b.retained_bytes(), 0);
    }
    // Original config survives later World switches, scale fallback and NaN payload.
    e.config.marks_decay_scale = f64::from_bits(0x7ff8_0000_0000_0042);
    let c = candidate(&e, 0.0);
    assert_eq!(
        c.value().data.config_marks_decay_scale.to_bits(),
        0x7ff8_0000_0000_0042
    );
    assert_ne!(
        c.value().data.world.marks.decay_scale.to_bits(),
        c.value().data.config_marks_decay_scale.to_bits()
    );
}
#[test]
fn cached_duplicate_place_handles_and_historical_labels_are_valid() {
    let mut e = engine();
    prefix(&mut e);
    if let Some(EngineMessage::ChalkStanding { anchors, .. }) = &mut e.last_chalk_standing {
        anchors[0].handle = "The former doorway".into();
        anchors[0].label = "The former owner".into();
        anchors.push(anchors[0].clone());
    }
    let c = candidate(&e, 0.0);
    assert_eq!(
        c.value().last_chalk_standing(),
        e.last_chalk_standing.as_ref()
    );
}
#[test]
fn supported_engine_marks_fixture() {
    let mut e = engine();
    prefix(&mut e);
    assert_eq!(
        bytes(&e, 0.0).value().as_slice(),
        include_bytes!("../../../tests/fixtures/checkpoint_v1/engine_marks.json")
    );
}
#[test]
#[ignore = "create only the new M2a7 fixture"]
fn create_engine_marks_fixture() {
    let mut e = engine();
    prefix(&mut e);
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/engine_marks.json"
        ),
        bytes(&e, 0.0).value(),
    )
    .unwrap();
}
