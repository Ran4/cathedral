use super::*;
use crate::{
    NullSight, NullTranscription, NullTts,
    checkpoint::{CheckpointBudget, Cohort},
};
fn engine() -> Engine {
    Engine::new(
        EngineConfig {
            fake_mode: true,
            nav: Some(crate::dogs::checkpoint::tests::nav()),
            clock: WorldClock::new(3600.0, Office::Waning, 0, 0.05),
            night_office: owner::tests::all_tiers(),
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
        Box::new(owner::tests::Recorded::new()),
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
fn bytes(e: &Engine, now: f64) -> Vec<u8> {
    let b = CheckpointBudget::default();
    e.export_night_checkpoint(logical(now), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
        .value()
        .clone()
}
fn install(control: &Engine, other: &mut Engine, now: f64) {
    let raw = bytes(control, now);
    let b = CheckpointBudget::default();
    let c = other.night_checkpoint_context(logical(now));
    let candidate = EngineNightDtoV1::decode(
        &raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    other.night = NightOffice::new(NightOfficeConfig::default(), 999.0, &other.clock);
    other.world.ward_moods.clear();
    other.config.night_office = NightOfficeConfig::default();
    let d = &candidate.value().data;
    other.night = owner::copy(&d.night.night);
    other.world.ward_moods = d.world.ward_moods.clone();
    other.config.night_office = d.config_night_office;
    assert_eq!(
        bytes(other, now),
        raw,
        "canonical equality before ordinary Engine poll"
    );
}
fn prefix(e: &mut Engine) {
    // Engine dependencies are independently seeded. Move only the player so
    // ordinary Night gate yields an empty stage.
    e.world
        .characters
        .get_mut(&ActorId::from_raw("player"))
        .unwrap()
        .state
        .position_m = Vec3::new(10000.0, 0.0, 10000.0);
    e.poll(901.0, vec![]);
}
#[test]
fn checkpoint_night_engine_queue_and_unfinished_flight_continue_on_ordinary_poll() {
    let mut a = engine();
    let mut b = engine();
    prefix(&mut a);
    prefix(&mut b);
    assert!(a.night.in_flight_subject().is_some());
    // Independent original Engine config can differ from an owner's captured
    // configuration. A disabled current config does not erase live duties.
    a.config.night_office = NightOfficeConfig::default();
    a.world.ward_moods.insert(
        crate::lore::PlanningWard::Weigh,
        "  exact old mood  ".into(),
    );
    install(&a, &mut b, 901.0);
    let completion = Completion {
        request_id: crate::RequestId(1),
        result: Ok("remember {\"memory\":\"A checkpoint evening\"}".into()),
        duration_seconds: 0.45,
    };
    for now in [902.0, 903.0, 912.0] {
        let cmds = if now == 902.0 {
            vec![EngineCommand::LlmCompletion(completion.clone())]
        } else {
            vec![]
        };
        let x = a.poll(now, cmds.clone());
        let y = b.poll(now, cmds);
        assert_eq!(format!("{x:?}"), format!("{y:?}"));
        assert_eq!(bytes(&a, now), bytes(&b, now));
    }
    assert_eq!(a.night.totals().0, 1);
}
#[test]
fn checkpoint_night_engine_fixture() {
    let mut e = engine();
    prefix(&mut e);
    assert_eq!(
        bytes(&e, 901.0),
        include_bytes!("../../../tests/fixtures/checkpoint_v1/engine_night.json")
    );
}
#[test]
#[ignore = "create only new M2a9 fixtures"]
fn create_night_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checkpoint_v1");
    let mut e = engine();
    prefix(&mut e);
    for (name, raw) in [
        ("engine_night.json", bytes(&e, 901.0)),
        ("world_night.json", {
            e.world.ward_moods.insert(
                crate::lore::PlanningWard::Weigh,
                "  the river waits\n  ".into(),
            );
            e.world
                .ward_moods
                .insert(crate::lore::PlanningWard::Reed, "".into());
            let b = CheckpointBudget::default();
            e.world
                .export_night_checkpoint(
                    logical(901.0),
                    b.reserve(Cohort::SavePayload, 4096).unwrap(),
                )
                .unwrap()
                .encode()
                .unwrap()
                .value()
                .clone()
        }),
    ] {
        let path = dir.join(name);
        assert!(!path.exists() || path.metadata().unwrap().len() == 0);
        std::fs::write(path, raw).unwrap();
    }
}
#[test]
fn checkpoint_night_world_fixture() {
    let mut e = engine();
    prefix(&mut e);
    e.world.ward_moods.insert(
        crate::lore::PlanningWard::Weigh,
        "  the river waits\n  ".into(),
    );
    e.world
        .ward_moods
        .insert(crate::lore::PlanningWard::Reed, "".into());
    let b = CheckpointBudget::default();
    let raw = e
        .world
        .export_night_checkpoint(
            logical(901.0),
            b.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    assert_eq!(
        raw.value(),
        include_bytes!("../../../tests/fixtures/checkpoint_v1/world_night.json")
    );
}
