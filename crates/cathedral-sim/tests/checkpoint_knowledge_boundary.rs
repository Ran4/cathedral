//! Independent private-knowledge boundary witnesses; no city is adopted here.
mod prompt_support;

use std::sync::Arc;

use cathedral_sim::{
    FactId, World, WorldTime,
    checkpoint::{CheckpointBudget, Cohort},
    knowledge::checkpoint::{KnowledgeCheckpointContext, KnowledgeDtoV1, WorldKnowledgeDtoV1},
    knowledge::{self, AreaAdjacency, FactCatalog, Telling},
    timeline::LogicalTime,
    world::WorldBackboneDtoV1,
};
use prompt_support::actor;

const PRIVATE_MARKER: &str = "sealed-cause-marker-never-for-the-journal";

fn pack(template: &str) -> FactCatalog {
    FactCatalog::from_json(
        &serde_json::json!({
            "schema_version": 1,
            "facts": [
                {
                    "id": "checkpoint.history",
                    "topic": "law",
                    "said": "a person was taken in charge",
                    "seeded": ["sv3n1"],
                    "source": {"custody": "departed"}
                },
                {
                    "id": "checkpoint.current",
                    "topic": "talk",
                    "said": template,
                    "seeded": ["sv3n1"],
                    "source": {"quest_phase": {"quest": PRIVATE_MARKER, "phase": 3}}
                }
            ]
        })
        .to_string(),
    )
    .unwrap()
}

fn active_world() -> World {
    let mut world = prompt_support::seed_world();
    world.current_time = Some(WorldTime::from_game_days(2.0));
    world.area_adjacency = Arc::new(AreaAdjacency::build(&world.area_map));
    let catalog = Arc::new(pack("a quiet word passed through the city"));
    assert!(catalog.seed(&mut world).is_empty());
    world.fact_catalog = catalog;
    let first = world.knowledge.facts().next().unwrap().0;
    knowledge::learn(
        &mut world,
        &actor("player"),
        first,
        Telling {
            hops: 2,
            from: Some(actor("departed")),
            heat: 0.8,
            view: Default::default(),
        },
        Some(2.0),
    );
    world.knowledge.note_seated(&actor("sv3n1"), vec![first]);
    world.knowledge.note_occasion(
        &actor("sv3n1"),
        Some(actor("departed")),
        Some(actor("departed")),
        2.0,
    );
    world.knowledge.offer_occasion(&actor("sv3n1"));
    world.knowledge.expire_occasions(3.0);
    assert!(world.knowledge.occasion(&actor("sv3n1")).unwrap().offered);
    world
}

#[test]
fn invalidated_fact_keeps_its_remembered_telling_and_pending_conversation() {
    let mut world = active_world();
    let history = FactId::from_raw("checkpoint.history");
    let key = world.knowledge.key_of(&history).unwrap();
    assert!(!world.characters.contains_key(&actor("departed")));
    assert_eq!(
        knowledge::invalidate_stale(&mut world),
        vec![history.clone()]
    );
    assert!(world.knowledge.fact(key).is_none());
    let receipt = &world.knowledge.player_learned[&history];
    assert_eq!(receipt.from, Some(actor("departed")));
    assert_eq!(receipt.tellings, 1);
    assert_eq!(world.knowledge.facts().next().unwrap().0.0, key.0 + 1);

    let budget = CheckpointBudget::default();
    let context = KnowledgeCheckpointContext::from_world(&world, LogicalTime::new(10.0).unwrap());
    let saved = world
        .knowledge
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let candidate = KnowledgeDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().knowledge(), &world.knowledge);
    // Inspect a test-local copy of the one-shot obligation, without extracting
    // the candidate or constructing a replacement World.
    let mut pending = candidate.value().knowledge().clone();
    assert_eq!(pending.take_seated(&actor("sv3n1")), vec![key]);
    assert!(pending.take_seated(&actor("sv3n1")).is_empty());
    assert!(pending.occasion(&actor("sv3n1")).unwrap().offered);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn stale_provenance_survives_while_debug_and_malformed_source_errors_stay_sealed() {
    let world = active_world();
    let budget = CheckpointBudget::default();
    let context = KnowledgeCheckpointContext::from_world(&world, LogicalTime::new(10.0).unwrap());
    let dto = world
        .knowledge
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    assert!(!format!("{:?}", dto.value()).contains(PRIVATE_MARKER));
    let saved = dto.encode().unwrap();
    assert!(
        std::str::from_utf8(saved.value())
            .unwrap()
            .contains(PRIVATE_MARKER)
    );
    let save_charge = budget.retained_bytes();
    let candidate = KnowledgeDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().knowledge(), &world.knowledge);
    assert!(!format!("{:?}", candidate.value()).contains(PRIVATE_MARKER));
    drop(candidate);

    let original: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    let mut unknown_variant = serde_json::Map::new();
    unknown_variant.insert(PRIVATE_MARKER.into(), serde_json::json!(PRIVATE_MARKER));
    let malformed = [
        serde_json::Value::Object(unknown_variant),
        serde_json::json!({"quest_phase": {"quest": [PRIVATE_MARKER], "phase": 3}}),
        serde_json::json!({"custody": [PRIVATE_MARKER]}),
    ];
    for source in malformed {
        let mut wire = original.clone();
        wire["knowledge"]["live"][1]["source"] = source;
        let bytes = serde_json::to_vec(&wire).unwrap();
        let error = KnowledgeDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context,
        )
        .unwrap_err();
        assert!(error.reason.contains("sealed fact provenance"));
        assert!(!format!("{error:?} {error}").contains(PRIVATE_MARKER));
        assert_eq!(budget.retained_bytes(), save_charge);
    }
    drop(saved);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn changed_catalog_text_refuses_context_then_original_binds_to_unadopted_backbone() {
    let mut world = active_world();
    let now = LogicalTime::new(10.0).unwrap();
    let expected = world.knowledge.clone();
    let budget = CheckpointBudget::default();
    let saved = world
        .export_knowledge_checkpoint(now, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let save_charge = budget.retained_bytes();
    let original_catalog = world.fact_catalog.clone();
    world.fact_catalog = Arc::new(pack("a different account is now in the catalog"));
    let error = WorldKnowledgeDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        KnowledgeCheckpointContext::from_world(&world, now),
    )
    .unwrap_err();
    assert!(error.reason.contains("context"));
    assert_eq!(world.knowledge, expected);
    assert_eq!(budget.retained_bytes(), save_charge);
    world.fact_catalog = original_catalog;

    // Separate component budgets exercise borrowing before adoption. They do
    // not claim that a complete envelope meets the global coexistence budget.
    let backbone_budget = CheckpointBudget::default();
    let backbone_bytes = world
        .export_backbone_checkpoint(backbone_budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let backbone = WorldBackboneDtoV1::decode(
        backbone_bytes.value(),
        backbone_budget
            .reserve(Cohort::LoadCandidate, backbone_bytes.value().len() + 4096)
            .unwrap(),
        &world.item_catalog,
        &world.command_ledger,
    )
    .unwrap()
    .into_candidate(&world.item_catalog, &world.command_ledger)
    .unwrap();
    drop(backbone_bytes);
    let context = KnowledgeCheckpointContext::from_backbone(
        backbone.value(),
        now,
        &world.area_map,
        &world.fact_catalog,
        &world.salience,
    );
    let candidate = WorldKnowledgeDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().knowledge(), &expected);
    assert_eq!(candidate.value().counts(context).facts, 2);
    drop((candidate, saved, backbone));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(backbone_budget.retained_bytes(), 0);
}

#[test]
fn duplicate_facts_refuse_and_padded_input_charge_survives_candidate_validation() {
    let world = active_world();
    let budget = CheckpointBudget::default();
    let context = KnowledgeCheckpointContext::from_world(&world, LogicalTime::new(10.0).unwrap());
    let saved = world
        .knowledge
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let save_charge = budget.retained_bytes();
    let mut wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    let facts = wire["knowledge"]["live"].as_array_mut().unwrap();
    facts.push(facts[0].clone());
    let bytes = serde_json::to_vec(&wire).unwrap();
    let error = KnowledgeDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap_err();
    assert!(error.reason.contains("duplicate knowledge key"));
    assert_eq!(budget.retained_bytes(), save_charge);

    let padding = 1024 * 1024;
    let mut padded = vec![b' '; padding];
    padded.extend_from_slice(saved.value());
    let decoded = KnowledgeDtoV1::decode(
        &padded,
        budget
            .reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained - save_charge >= canonical_peak + 3 * padding);
    let candidate = decoded.into_candidate(context).unwrap();
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(candidate.value().knowledge(), &world.knowledge);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn nonfinite_household_geometry_cannot_collapse_into_a_valid_context_hash() {
    let mut world = active_world();
    let now = LogicalTime::new(10.0).unwrap();
    let budget = CheckpointBudget::default();
    let saved = world
        .export_knowledge_checkpoint(now, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let save_charge = budget.retained_bytes();
    let export_budget = CheckpointBudget::default();
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        Arc::make_mut(&mut world.household_doors)
            .insert(actor("sv3n1"), cathedral_sim::Vec3::new(x, 0.0, 0.0));
        assert!(
            world
                .export_knowledge_checkpoint(
                    now,
                    export_budget.reserve(Cohort::SavePayload, 4096).unwrap(),
                )
                .is_err()
        );
        let context = KnowledgeCheckpointContext::from_world(&world, now);
        assert!(
            world
                .knowledge
                .export_checkpoint(
                    context,
                    export_budget.reserve(Cohort::SavePayload, 4096).unwrap(),
                )
                .is_err()
        );
        assert_eq!(export_budget.retained_bytes(), 0);
        assert!(
            WorldKnowledgeDtoV1::decode(
                saved.value(),
                budget
                    .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                    .unwrap(),
                context,
            )
            .is_err()
        );
        assert_eq!(budget.retained_bytes(), save_charge);
    }
    drop(saved);
    assert_eq!(budget.retained_bytes(), 0);
}
