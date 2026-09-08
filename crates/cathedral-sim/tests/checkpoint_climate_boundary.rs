//! Independent public component checks; these candidates do not adopt a city.
mod prompt_support;

use cathedral_sim::{
    AreaMap, Capabilities, Engine, EngineConfig, FakeCognition, NullSight, NullTranscription,
    NullTts, SoundCatalog, TtsBackendKind, Vec3, WeatherConfig, WeatherKind, WeatherSample,
    WeatherTimeline, World, WorldSeed,
    checkpoint::{CheckpointBudget, Cohort},
    clock::WorldTime,
    engine::climate_checkpoint::{ClimateCheckpointContext, EngineClimateDtoV1, WorldClimateDtoV1},
    sounds::Sound,
    timeline::LogicalTime,
    weather::checkpoint::WeatherTimelineDtoV1,
    world::WorldBackboneDtoV1,
};

#[test]
fn disabled_schedule_preserves_explicit_override_and_ignored_residue() {
    let mut timeline = WeatherTimeline::new(WeatherConfig {
        enabled: false,
        frequency: 0.0,
        ..Default::default()
    });
    timeline.set_override(WeatherKind::Downpour, Some(0.8), 2.0);
    let budget = CheckpointBudget::default();
    let saved = timeline
        .export_checkpoint(budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let candidate = WeatherTimelineDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
    )
    .unwrap()
    .into_candidate()
    .unwrap();
    for at in [2.0, 2.1, 3.0] {
        let expected = timeline.sample(at);
        assert_eq!(expected.kind, WeatherKind::Downpour);
        assert_eq!(candidate.value().sample(at).unwrap(), expected);
    }
    drop((candidate, saved));

    timeline.clear_override(2.1);
    assert_eq!(timeline.sample(2.1), WeatherSample::CLEAR);
    let saved = timeline
        .export_checkpoint(budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    assert!(wire["timeline"]["forced"].is_null());
    assert!(wire["timeline"]["residue"]["wetness"].as_f64().unwrap() > 0.0);
    let candidate = WeatherTimelineDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
    )
    .unwrap()
    .into_candidate()
    .unwrap();
    assert_eq!(candidate.value().sample(2.2).unwrap(), WeatherSample::CLEAR);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

fn sound_catalog(heard: &str) -> SoundCatalog {
    SoundCatalog::new(
        vec![Sound::new("town_bell", "bell", 100.0, heard, None, "bell", 1.0, false).unwrap()],
        vec![],
    )
    .unwrap()
}

#[test]
fn equal_sound_ids_with_changed_percepts_refuse_the_context_and_release_admission() {
    let mut world = World::new();
    world.sound_catalog = sound_catalog("A bell rings.");
    let now = LogicalTime::new(0.0).unwrap();
    let budget = CheckpointBudget::default();
    let saved = world
        .export_climate_checkpoint(now, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let save_bytes = budget.retained_bytes();
    world.sound_catalog = sound_catalog("A distant bell rings.");
    let error = WorldClimateDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        ClimateCheckpointContext::from_world(&world, now),
    )
    .unwrap_err();
    assert!(error.reason.contains("context mismatch"));
    assert_eq!(budget.retained_bytes(), save_bytes);
    world.sound_catalog = sound_catalog("A bell rings.");
    let context = ClimateCheckpointContext::from_world(&world, now);
    let candidate = WorldClimateDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().current_weather(), None);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn sampled_world_time_binds_to_the_unadopted_backbone() {
    let mut world = World::new();
    world.current_time = Some(WorldTime::from_game_days(2.25));
    world.current_weather = Some(WeatherSample::CLEAR);
    let now = LogicalTime::new(12.0).unwrap();
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
    // Separate component budgets establish the borrowed seam only, not full
    // envelope coexistence. No new World is constructed from the candidate.
    let budget = CheckpointBudget::default();
    let saved = world
        .export_climate_checkpoint(now, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let context = ClimateCheckpointContext::from_backbone(
        backbone.value(),
        now,
        None,
        &world.shelters,
        &world.area_map,
        &world.sound_catalog,
    );
    let candidate = WorldClimateDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(
        candidate.value().current_weather(),
        Some(WeatherSample::CLEAR)
    );
    drop(candidate);
    let save_bytes = budget.retained_bytes();
    world.current_time = Some(WorldTime::from_game_days(2.5));
    let error = WorldClimateDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        ClimateCheckpointContext::from_world(&world, now),
    )
    .unwrap_err();
    assert!(error.reason.contains("sampled time disagrees"));
    assert_eq!(budget.retained_bytes(), save_bytes);
    drop((saved, backbone));
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(backbone_budget.retained_bytes(), 0);
}

#[test]
fn matching_corrupt_weather_copies_cannot_override_the_saved_timeline() {
    let engine = Engine::new(
        EngineConfig {
            weather: WeatherConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        &WorldSeed::from_json_str(&prompt_support::demo_seed()).unwrap(),
        AreaMap::default(),
        SoundCatalog::empty(),
        prompt_support::prompt_env(),
        Box::new(FakeCognition::default()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap();
    let now = LogicalTime::new(0.0).unwrap();
    let budget = CheckpointBudget::default();
    let saved = engine
        .export_climate_checkpoint(now, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    wire["world"]["current_weather"]["surface_wetness"] = 0.4.into();
    wire["last_weather_sample"]["surface_wetness"] = 0.4.into();
    let corrupt = serde_json::to_vec(&wire).unwrap();
    let save_bytes = budget.retained_bytes();
    let error = EngineClimateDtoV1::decode(
        &corrupt,
        budget
            .reserve(Cohort::LoadCandidate, corrupt.len() + 4096)
            .unwrap(),
        ClimateCheckpointContext::from_world(engine.world(), now),
    )
    .unwrap_err();
    assert!(error.reason.contains("sampled weather disagrees"));
    assert_eq!(engine.world().current_weather, Some(WeatherSample::CLEAR));
    assert_eq!(budget.retained_bytes(), save_bytes);
    drop(saved);
    assert_eq!(budget.retained_bytes(), 0);
}
