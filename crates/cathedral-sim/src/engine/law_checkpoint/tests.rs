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
    e.export_law_checkpoint(logical(now), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn candidate(e: &Engine, now: f64) -> Admitted<EngineLawCandidate> {
    let b = CheckpointBudget::default();
    let saved = bytes(e, now);
    let c = LawCheckpointContext::from_world(&e.world, logical(now));
    EngineLawDtoV1::decode(
        saved.value(),
        b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap()
}

use owner::tests::{active, actor};
fn prefix(e: &mut Engine) {
    e.poll(0.0, vec![]);
    let w = active();
    for (id, ch) in w.characters {
        if id.as_str() != "player" {
            e.world.add_character(ch);
        }
    }
    e.world
        .characters
        .get_mut(&actor("player"))
        .unwrap()
        .state
        .position_m = Vec3::ZERO;
    e.world.notices = w.notices;
    e.world.custody = w.custody;
    e.poll(10.0, vec![]);
    assert!(
        matches!(&e.last_law_standing,Some(EngineMessage::LawStanding{notices,custody:Some(c)}) if !notices.is_empty()&&c.held)
    );
}
// Component-only installation: other owners have independently driven history.
// Scramble every covered field, restore, and assert immediate canonical bytes.
fn install(d: &EngineLawCandidate, e: &mut Engine) {
    let d = &d.data;
    e.world.notices = notices::Notices::default();
    e.world.custody = custody::Custody::default();
    e.config.player_id = actor("wrong");
    e.last_law_standing = None;
    e.world.notices = d.world.notices.clone();
    e.world.custody = d.world.custody.clone();
    e.config.player_id = d.player_id.clone();
    e.last_law_standing = d.last_law_standing.clone();
    assert!(
        bytes(e, d.boundary.seconds()).value() == &serde_json::to_vec(d).unwrap(),
        "immediate covered law bytes differ"
    );
}
#[test]
fn grip_sentence_and_notice_continuation_after_scrambled_component_install() {
    let mut control = engine();
    let mut resumed = engine();
    prefix(&mut control);
    prefix(&mut resumed);
    let restored = candidate(&control, 10.0);
    install(restored.value(), &mut resumed);
    let mut publications = 0;
    for now in [10.1, 42.6, 62.1, 63.0, 242.1, 400.0, 1400.0] {
        let a = control.poll(now, vec![]);
        let b = resumed.poll(now, vec![]);
        assert_eq!(a, b, "events at {now}");
        publications += a
            .iter()
            .filter(|m| matches!(m, EngineMessage::LawStanding { .. }))
            .count();
        assert!(
            bytes(&control, now).value() == bytes(&resumed, now).value(),
            "law bytes at {now}"
        );
    }
    assert!(publications >= 2);
    assert!(control.world.custody.get(&actor("committed")).is_none());
    assert!(
        control.world.custody.get(&actor("inmate")).is_some(),
        "authored inmate exempt from logical ceiling"
    );
    assert!(
        !control
            .world
            .custody
            .get(&actor("player"))
            .unwrap()
            .is_held()
    );
    assert!(
        control
            .world
            .notices
            .live()
            .iter()
            .all(|n| n.raised_game_days.is_none())
    );
}
#[test]
fn virgin_and_stale_cache_preserve_then_publish_exactly_once() {
    let mut e = engine();
    assert!(candidate(&e, 0.0).value().last_law_standing().is_none());
    prefix(&mut e);
    let cached = e.last_law_standing.clone();
    e.world.custody.release(&actor("player"));
    e.world.notices.settle(2);
    let d = candidate(&e, 10.0);
    assert_eq!(d.value().data.last_law_standing, cached);
    install(d.value(), &mut e);
    let a = e.poll(10.1, vec![]);
    assert_eq!(
        a.iter()
            .filter(|m| matches!(m, EngineMessage::LawStanding { .. }))
            .count(),
        1
    );
    let b = e.poll(10.2, vec![]);
    assert!(
        !b.iter()
            .any(|m| matches!(m, EngineMessage::LawStanding { .. }))
    );
}
#[test]
fn engine_cache_corruption_and_required_nullable_fields_refuse() {
    let mut e = engine();
    prefix(&mut e);
    let original: serde_json::Value = serde_json::from_slice(bytes(&e, 10.0).value()).unwrap();
    let mut cases = Vec::new();
    for (path, value) in [
        ("/player_id", json!("absent")),
        (
            "/last_law_standing/notices/0/clears_when",
            json!("pay the invented toll"),
        ),
        ("/last_law_standing/custody/strain_seconds", json!(0.0)),
        ("/last_law_standing/custody/fee_sparks", json!(99)),
        ("/last_law_standing/custody/reach_m", json!(10.0)),
        ("/last_law_standing/custody/held", json!(false)),
        ("/last_law_standing/custody/strain_seconds", json!(-1.0)),
        ("/last_law_standing/custody/release_office", json!("wrong")),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        cases.push(v);
    }
    for (parent, key) in [
        ("", "last_law_standing"),
        ("/last_law_standing", "custody"),
        ("/last_law_standing/custody", "officer_id"),
        ("/last_law_standing/custody", "release_office"),
        ("/last_law_standing/custody", "booked_as"),
    ] {
        let mut v = original.clone();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        cases.push(v);
    }
    let c = LawCheckpointContext::from_world(&e.world, logical(10.0));
    for v in cases {
        let b = CheckpointBudget::default();
        let wire = serde_json::to_vec(&v).unwrap();
        assert!(
            EngineLawDtoV1::decode(
                &wire,
                b.reserve(Cohort::LoadCandidate, wire.len() + 4096).unwrap(),
                c
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn supported_engine_law_fixture() {
    let mut e = engine();
    prefix(&mut e);
    assert!(
        bytes(&e, 10.0).value().as_slice()
            == include_bytes!("../../../tests/fixtures/checkpoint_v1/engine_law.json").as_slice(),
        "supported Engine law fixture differs"
    );
}
#[test]
#[ignore = "regenerate only the new supported M2a6 Engine fixture"]
fn regenerate_engine_law_fixture() {
    let mut e = engine();
    prefix(&mut e);
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/engine_law.json"
        ),
        bytes(&e, 10.0).value(),
    )
    .unwrap();
}
