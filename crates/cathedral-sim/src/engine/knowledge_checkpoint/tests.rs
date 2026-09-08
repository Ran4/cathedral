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
            clock: WorldClock::new(60.0, Office::Dayspring, 0, 0.05),
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
    e.export_knowledge_checkpoint(logical(now), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn candidate(e: &Engine, now: f64) -> Admitted<EngineKnowledgeCandidate> {
    let b = CheckpointBudget::default();
    let saved = bytes(e, now);
    let c = KnowledgeCheckpointContext::from_world(&e.world, logical(now));
    EngineKnowledgeDtoV1::decode(
        saved.value(),
        b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap()
}
fn prefix(e: &mut Engine) {
    e.config.idle_mode = IdleCognitionMode::All;
    e.poll(0.0, vec![]);
    knowledge::checkpoint::tests::active(&mut e.world);
    e.door_shut_until.insert(
        (e.config.player_id.clone(), ActorId::from_raw("departed")),
        -2.0,
    );
    e.poll(0.01, vec![]);
    assert!(
        matches!(&e.last_journal,Some(EngineMessage::Journal{entries,..}) if !entries.is_empty())
    );
    assert!(matches!(&e.last_ward_heat,Some(EngineMessage::WardHeat{wards}) if wards.len()==8));
}
// Test-only component install after independently preparing every other owner.
// Deliberately scramble all covered fields and compare canonical bytes before
// polling can overwrite a missing cursor. This is not whole-engine adoption.
fn install(d: &EngineKnowledgeCandidate, e: &mut Engine) {
    let d = &d.data;
    e.world.knowledge = knowledge::Knowledge::default();
    e.world.knowledge_enabled = !d.world.knowledge_enabled;
    e.world.pollen_no_salience = !d.world.pollen_no_salience;
    e.world.area_adjacency = std::sync::Arc::new(knowledge::AreaAdjacency::default());
    e.config.player_id = ActorId::from_raw("wrong");
    e.next_stage_hop_at = 99.0;
    e.next_player_pollen_game_days = 100.0;
    e.door_shut_until.clear();
    e.last_journal = None;
    e.last_journal_receipts = 123;
    e.last_journal_at = f64::NEG_INFINITY;
    e.last_ward_heat = None;
    e.world.knowledge = d.world.knowledge.clone();
    e.world.knowledge_enabled = d.world.knowledge_enabled;
    e.world.pollen_no_salience = d.world.pollen_no_salience;
    e.world.area_adjacency = std::sync::Arc::new(d.world.area_adjacency.clone());
    e.config.player_id = d.player_id.clone();
    e.next_stage_hop_at = d.next_stage_hop_at;
    e.next_player_pollen_game_days = d.next_player_pollen_game_days.legacy();
    e.door_shut_until = d.door_shut_until.clone();
    e.last_journal = d.last_journal.clone();
    e.last_journal_receipts = d.last_journal_receipts;
    e.last_journal_at = d.last_journal_at.legacy();
    e.last_ward_heat = d.last_ward_heat.clone();
    assert_eq!(
        bytes(e, d.boundary.seconds()).value(),
        &serde_json::to_vec(d).unwrap()
    );
}
#[test]
fn pollen_journal_and_obligation_continuation_after_scrambled_component_install() {
    let mut control = engine();
    let mut resumed = engine();
    prefix(&mut control);
    prefix(&mut resumed);
    let saved = candidate(&control, 0.01);
    install(saved.value(), &mut resumed);
    assert!(saved.value().history_counts().historical_receipts > 0);
    assert!(saved.value().history_counts().historical_seated_keys > 0);
    let mut emitted = 0;
    for n in 2..=130 {
        let now = n as f64 * 0.02;
        let a = control.poll(now, vec![]);
        let b = resumed.poll(now, vec![]);
        emitted += a
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    EngineMessage::Journal { .. } | EngineMessage::WardHeat { .. }
                )
            })
            .count();
        assert_eq!(a, b, "events at {now}");
        assert_eq!(bytes(&control, now).value(), bytes(&resumed, now).value());
    }
    assert!(emitted > 0);
    assert!(control.door_shut_until.values().any(|v| *v < 0.0));
    let who = ActorId::from_raw("sv3n1");
    assert_eq!(
        control.world.knowledge.take_seated(&who),
        resumed.world.knowledge.take_seated(&who)
    );
    assert!(control.world.knowledge.spend_occasion(&who));
    assert!(resumed.world.knowledge.spend_occasion(&who));
}
#[test]
fn stale_journal_and_initial_empty_cache_are_preserved_without_refresh() {
    let mut e = engine();
    assert!(e.last_journal.is_none());
    let virgin = candidate(&e, 0.0);
    assert!(virgin.value().data.last_journal.is_none());
    e.world.knowledge = knowledge::Knowledge::default();
    e.poll(0.0, vec![]);
    let cached = candidate(&e, 0.0);
    assert!(
        matches!(&cached.value().data.last_journal,Some(EngineMessage::Journal{entries,..}) if entries.is_empty())
    );
    prefix(&mut e);
    let before = e.last_journal.clone();
    let revision = e.last_journal_receipts;
    let key = e.world.knowledge.facts().next().unwrap().0;
    e.world.knowledge.invalidate(key);
    e.poll(0.02, vec![]);
    assert_eq!(e.last_journal, before);
    assert_eq!(e.last_journal_receipts, revision);
    let restored = candidate(&e, 0.02);
    assert_eq!(restored.value().data.last_journal, before);
}
#[test]
fn engine_nullable_caller_deadline_and_cache_corruption_refuse_and_release() {
    let mut e = engine();
    prefix(&mut e);
    let original: serde_json::Value = serde_json::from_slice(bytes(&e, 0.01).value()).unwrap();
    let b = CheckpointBudget::default();
    let c = KnowledgeCheckpointContext::from_world(&e.world, logical(0.01));
    let mut cases = Vec::new();
    for (path, value, reason) in [
        ("/door_shut_until/0/0", json!("sv3n1"), "caller"),
        ("/next_stage_hop_at", json!(100.0), "cadence"),
        ("/last_journal_at", json!({"at":1.0}), "future"),
        ("/last_ward_heat/wards/0/heat_pct", json!(101), "numeric"),
        (
            "/last_ward_heat/wards/0/at",
            json!([0.0, 0.0, 0.0]),
            "centroid",
        ),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        cases.push((v, reason));
    }
    for field in ["last_journal", "last_ward_heat"] {
        let mut v = original.clone();
        v.as_object_mut().unwrap().remove(field);
        cases.push((v, "missing field"));
    }
    let mut v = original.clone();
    let row = v["door_shut_until"][0].clone();
    v["door_shut_until"].as_array_mut().unwrap().push(row);
    cases.push((v, "duplicate"));
    for (v, reason) in cases {
        let raw = serde_json::to_vec(&v).unwrap();
        let error = EngineKnowledgeDtoV1::decode(
            &raw,
            b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
            c,
        )
        .unwrap_err();
        assert!(error.reason.contains(reason), "{error} expected {reason}");
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
#[ignore = "diagnostic fixture writer; updates only this owner fixture"]
fn write_engine_knowledge_fixture() {
    let mut e = engine();
    prefix(&mut e);
    let v: serde_json::Value = serde_json::from_slice(bytes(&e, 0.01).value()).unwrap();
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/engine_knowledge.json"
        ),
        serde_json::to_string_pretty(&v).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
fn uncached_centroids_match_ordinary_singleton_exactly() {
    let cold = knowledge::pollen::checkpoint::centroids().unwrap();
    let ordinary = knowledge::pollen::ward_centroids();
    for ward in crate::lore::PlanningWard::ALL {
        assert_eq!(
            cold[&ward].to_array().map(f64::to_bits),
            ordinary[&ward].to_array().map(f64::to_bits)
        );
    }
}

#[test]
#[ignore = "target-specific closed-layout evidence"]
fn engine_knowledge_layout() {
    macro_rules! row {($($t:ty),*)=>{$(println!("{}={}",stringify!($t),std::mem::size_of::<$t>());)*}}
    row!(
        Wire,
        EngineKnowledgeDtoV1,
        EngineKnowledgeCandidate,
        EngineMessage,
        JournalEntry,
        WardHeatRow
    );
}

#[test]
fn supported_engine_knowledge_fixture_exact_decode_and_reexport() {
    let mut e = engine();
    prefix(&mut e);
    let fixture = include_bytes!("../../../tests/fixtures/checkpoint_v1/engine_knowledge.json");
    let expected: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(bytes(&e, 0.01).value()).unwrap() == expected,
        "Engine knowledge fixture differs"
    );
    let b = CheckpointBudget::default();
    let c = KnowledgeCheckpointContext::from_world(&e.world, logical(0.01));
    let d = EngineKnowledgeDtoV1::decode(
        fixture,
        b.reserve(Cohort::LoadCandidate, fixture.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&d.value().data).unwrap())
            .unwrap()
            == expected,
        "Engine knowledge fixture re-export differs"
    );
}
