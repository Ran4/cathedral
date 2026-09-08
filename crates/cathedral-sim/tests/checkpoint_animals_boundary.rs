//! Independent animal-owner boundaries; these candidates never adopt a city.
mod prompt_support;

use cathedral_sim::{
    Capabilities, Engine, EngineConfig, FakeCognition, NavData, NullSight, NullTranscription,
    NullTts, TtsBackendKind, Vec3, World, WorldSeed,
    checkpoint::{Admitted, CheckpointBudget, Cohort},
    dogs::{
        self,
        checkpoint::{AnimalsCheckpointContext, WorldAnimalsCandidate, WorldAnimalsDtoV1},
    },
    engine::animals_checkpoint::EngineAnimalsDtoV1,
    ids::DogId,
    timeline::LogicalTime,
    world::WorldBackboneDtoV1,
};
use std::sync::Arc;

fn line_nav(extra_whitespace: bool) -> NavData {
    let text = r#"{
      "schema_version": 1,
      "grid": {"x0": -5.0, "z0": -5.0, "cell_m": 1.0, "w": 60, "h": 10,
                "agent_radius_m": 0.35, "bitset_file": "x.bin",
                "bitset_bits": 600, "bitset_sha256": ""},
      "nodes": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]],
      "edges": [[0, 1, 2.0], [1, 2, 2.0], [2, 3, 2.0]],
      "places": [{"name": "a", "node": 0, "kind": "place"},
                 {"name": "b", "node": 3, "kind": "place"}],
      "sites": [], "doors": [], "reference": {"forecourt": 0}
    }"#;
    let source = if extra_whitespace {
        format!("{text}\n")
    } else {
        text.to_string()
    };
    NavData::from_parts(&source, &[0xff; 75]).unwrap()
}

#[test]
fn an_engine_without_navigation_keeps_its_own_unspent_publication_and_clock() {
    let mut engine = Engine::new(
        EngineConfig::default(),
        &WorldSeed::from_json_str(&prompt_support::demo_seed()).unwrap(),
        prompt_support::areas(),
        prompt_support::catalog(),
        prompt_support::prompt_env(),
        Box::new(FakeCognition::new()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::ZERO, 0.0),
        0,
        0.0,
    )
    .unwrap();
    let nav = Arc::new(line_nav(false));
    engine.world_mut().dogs = dogs::seed_pack(&nav);
    engine.world_mut().nav = Some(nav);
    let budget = CheckpointBudget::default();
    let saved = engine
        .export_animals_checkpoint(now(), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let retained = budget.retained_bytes();
    let world_context = AnimalsCheckpointContext::from_world(engine.world(), now());
    assert!(
        EngineAnimalsDtoV1::decode(
            saved.value(),
            budget
                .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                .unwrap(),
            world_context,
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), retained);
    let context = world_context.with_engine_nav(None);
    let candidate = EngineAnimalsDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().dogs(), engine.world().dogs);
    assert!(!candidate.value().dogs_published());
    assert_eq!(
        candidate.value().movement_now().seconds().to_bits(),
        0.0f64.to_bits()
    );
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

fn world() -> World {
    let nav = Arc::new(line_nav(false));
    let mut world = prompt_support::seed_world();
    world.dogs = dogs::seed_pack(&nav);
    world.nav = Some(nav);
    world
}

fn now() -> LogicalTime {
    LogicalTime::new(30.0).unwrap()
}

fn save(world: &World, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
    world
        .export_animals_checkpoint(now(), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}

fn load(
    bytes: &[u8],
    context: AnimalsCheckpointContext<'_>,
    budget: &CheckpointBudget,
) -> Admitted<WorldAnimalsCandidate> {
    WorldAnimalsDtoV1::decode(
        bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap()
}

#[test]
fn a_reordered_resting_pack_keeps_its_next_drift_and_current_identity() {
    let mut world = world();
    world.dogs.reverse();
    world.dogs[0].id = DogId::new("a_different_dog").unwrap();
    world.dogs[0].name = "A new keeper's name".into();
    world.dogs[0].description = "a familiar dog with a new collar".into();
    let revision = world.world_revision;
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        AnimalsCheckpointContext::from_world(&world, now()),
        &budget,
    );
    assert_eq!(candidate.value().dogs(), world.dogs);
    // Test-local copies only; no production partial-install API is exposed.
    let mut restored = candidate.value().dogs().to_vec();
    let mut control = world.dogs.clone();
    let nav = world.nav.as_deref().unwrap();
    let mut moved = false;
    for _ in 0..800 {
        let expected = dogs::step_dogs(&mut control, 0.05, nav);
        assert_eq!(dogs::step_dogs(&mut restored, 0.05, nav), expected);
        assert_eq!(restored, control, "private path, rest and epoch must agree");
        moved |= expected;
    }
    assert!(moved);
    assert_eq!(world.world_revision, revision);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn a_planted_turn_keeps_the_remaining_path_and_next_decision() {
    let mut world = world();
    let nav = world.nav.clone().unwrap();
    let mut turned_in_place = false;
    for _ in 0..600 {
        let before = world.dogs.clone();
        dogs::step_dogs(&mut world.dogs, 0.05, &nav);
        if world.dogs.iter().zip(&before).any(|(after, before)| {
            after.position_m == before.position_m && after.facing_yaw != before.facing_yaw
        }) {
            turned_in_place = true;
            break;
        }
    }
    assert!(
        turned_in_place,
        "capture an actual ordinary turning boundary"
    );
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        AnimalsCheckpointContext::from_world(&world, now()),
        &budget,
    );
    assert_eq!(candidate.value().dogs(), world.dogs);
    let mut restored = candidate.value().dogs().to_vec();
    let mut control = world.dogs.clone();
    let initial = control.clone();
    for _ in 0..800 {
        assert_eq!(
            dogs::step_dogs(&mut restored, 0.05, &nav),
            dogs::step_dogs(&mut control, 0.05, &nav)
        );
        assert_eq!(restored, control);
    }
    assert!(
        control
            .iter()
            .zip(&initial)
            .any(|(a, b)| a.position_m != b.position_m)
    );
}

#[test]
fn the_saved_last_positive_speed_emits_one_stop_before_a_quiet_rest() {
    let mut world = world();
    world.dogs.truncate(1);
    // The seeded path is empty and its rest is positive. Arrival has exactly
    // this combination with the last stride's positive speed still retained.
    world.dogs[0].speed = 0.75;
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        AnimalsCheckpointContext::from_world(&world, now()),
        &budget,
    );
    let mut restored = candidate.value().dogs().to_vec();
    assert_eq!(restored[0].speed, 0.75);
    let nav = world.nav.as_deref().unwrap();
    assert!(dogs::step_dogs(&mut restored, 0.05, nav));
    assert_eq!(restored[0].speed, 0.0);
    assert!(!dogs::step_dogs(&mut restored, 0.05, nav));
}

#[test]
fn exact_navigation_binds_without_reseeding_and_none_preserves_a_pack() {
    let mut world = world();
    let original_nav = world.nav.clone();
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let save_charge = budget.retained_bytes();
    for replacement in [Some(Arc::new(line_nav(true))), None] {
        world.nav = replacement;
        assert!(
            WorldAnimalsDtoV1::decode(
                saved.value(),
                budget
                    .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                    .unwrap(),
                AnimalsCheckpointContext::from_world(&world, now()),
            )
            .is_err()
        );
        assert_eq!(budget.retained_bytes(), save_charge);
    }
    world.nav = original_nav;
    // Independent budgets test the borrowing seam, not full-envelope coexistence.
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
    let context =
        AnimalsCheckpointContext::from_backbone(backbone.value(), now(), world.nav.as_deref());
    let candidate = load(saved.value(), context, &budget);
    assert_eq!(candidate.value().dogs(), world.dogs);
    drop((candidate, saved, backbone));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(backbone_budget.retained_bytes(), 0);
    world.nav = None;
    let saved = save(&world, &budget);
    let candidate = load(
        saved.value(),
        AnimalsCheckpointContext::from_world(&world, now()),
        &budget,
    );
    assert_eq!(
        candidate.value().dogs(),
        world.dogs,
        "no navigation is not an empty pack"
    );
}

#[test]
fn malformed_animals_release_admission_and_raw_padding_remains_charged() {
    let world = world();
    let budget = CheckpointBudget::default();
    let saved = save(&world, &budget);
    let save_charge = budget.retained_bytes();
    let context = AnimalsCheckpointContext::from_world(&world, now());
    let original: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    for case in 0..3 {
        let mut wire = original.clone();
        match case {
            0 => wire["unrecognized_animal_authority"] = true.into(),
            1 => {
                assert!(
                    wire["dogs"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("path")
                        .is_some()
                );
            }
            _ => {
                let duplicate = wire["dogs"][0].clone();
                wire["dogs"].as_array_mut().unwrap().push(duplicate);
            }
        }
        let bytes = serde_json::to_vec(&wire).unwrap();
        let error = WorldAnimalsDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context,
        )
        .unwrap_err();
        if case == 2 {
            assert!(error.reason.contains("duplicate"), "{error:?}");
        }
        assert_eq!(budget.retained_bytes(), save_charge);
    }
    let padding = 1024 * 1024;
    let mut padded = vec![b' '; padding];
    padded.extend_from_slice(saved.value());
    let decoded = WorldAnimalsDtoV1::decode(
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
    assert_eq!(candidate.value().dogs(), world.dogs);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}
