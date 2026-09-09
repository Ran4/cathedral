use super::*;
use crate::checkpoint::{CheckpointBudget, Cohort};
use crate::*;
use serde_json::json;
#[derive(Default)]
struct Recorded(u64);
impl Cognition for Recorded {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        self.0 += 1;
        Ok(RequestId(self.0))
    }
}
fn engine() -> Engine {
    Engine::new(
        EngineConfig {
            fake_mode: true,
            nav: Some(crate::dogs::checkpoint::tests::nav()),
            clock: WorldClock::new(3600.0, Office::Waning, 0, 0.05),
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            turn_delay_seconds: 0.0,
            tts_selected: TtsBackendKind::Off,
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
        Box::new(Recorded::default()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}
#[derive(Default)]
struct Inputs(Vec<(CognitionRequestMethod, String, Option<u32>)>);
impl Cognition for Inputs {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        panic!("wrong method")
    }
    fn request_with_budget(
        &mut self,
        p: String,
        b: Option<u32>,
    ) -> std::result::Result<RequestId, CognitionBusy> {
        self.0
            .push((CognitionRequestMethod::RequestWithBudget, p, b));
        Ok(RequestId(self.0.len() as u64))
    }
    fn request_night(
        &mut self,
        p: String,
        b: Option<u32>,
    ) -> std::result::Result<RequestId, CognitionBusy> {
        self.0.push((CognitionRequestMethod::RequestNight, p, b));
        Ok(RequestId(self.0.len() as u64))
    }
}
fn at(t: f64) -> LogicalTime {
    LogicalTime::new(t).unwrap()
}
fn reserve(b: &CheckpointBudget, c: Cohort, n: usize) -> Reservation {
    b.reserve(c, n + 4096).unwrap()
}
fn bytes(e: &Engine, t: f64) -> Vec<u8> {
    e.export_cognition_inputs_checkpoint(
        at(t),
        reserve(&CheckpointBudget::default(), Cohort::SavePayload, 0),
    )
    .unwrap()
    .encode()
    .unwrap()
    .value()
    .clone()
}
fn dual() -> (Engine, Inputs) {
    let mut e = engine();
    e.world = crate::night::checkpoint::tests::world_with_cast();
    e.clock = WorldClock::new(1.0, Office::Waning, 0, 0.05);
    e.scheduler = NpcScheduler::new(vec![ActorId::from_raw("mjr01")], 0.0, 60.0, 0.0);
    e.scheduler.start(0.0);
    e.night = NightOffice::new(crate::night::checkpoint::tests::all_tiers(), 0.0, &e.clock);
    e.night.seed(&e.world, &crate::round::Round::new());
    let mut s = Inputs::default();
    e.scheduler.poll(
        0.3,
        &mut e.world,
        &mut vec![],
        &mut vec![],
        false,
        crate::attention::IdleGate::All,
        &mut s,
        &e.env,
    );
    e.night.ring(
        0.3,
        &mut e.world,
        &mut crate::round::Round::new(),
        &e.clock,
        &mut vec![],
    );
    e.night.poll(
        0.3,
        &mut e.world,
        &e.clock,
        &mut vec![],
        crate::night::checkpoint::tests::open(),
        &mut s,
        &e.env,
    );
    e.world.command_ledger.drain_updates();
    assert_eq!(s.0.len(), 2);
    assert!(e.scheduler.in_flight_actor_id().is_some() && e.night.in_flight_subject().is_some());
    (e, s)
}
fn refuse(raw: &[u8], e: &Engine) {
    let b = CheckpointBudget::default();
    assert!(
        EngineCognitionInputsDtoV1::decode(
            raw,
            reserve(&b, Cohort::LoadCandidate, raw.len()),
            e.cognition_inputs_checkpoint_context(at(0.3))
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
}
#[test]
fn checkpoint_cognition_inputs_simultaneous_flights_have_exact_inputs_after_subject_mutation() {
    let (mut e, s) = dual();
    let raw = bytes(&e, 0.3);
    e.world
        .characters
        .get_mut(&ActorId::from_raw("mjr01"))
        .unwrap()
        .sheet
        .lore = None;
    e.world
        .characters
        .get_mut(&ActorId::from_raw("mjr01"))
        .unwrap()
        .state
        .presence_epoch = 99;
    e.config.night_office = crate::night::NightOfficeConfig::default();
    assert_eq!(bytes(&e, 0.3), raw);
    let c = e.cognition_inputs_checkpoint_context(at(0.3));
    let candidate = EngineCognitionInputsDtoV1::decode(
        &raw,
        reserve(
            &CheckpointBudget::default(),
            Cohort::LoadCandidate,
            raw.len(),
        ),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    let a = candidate.value().scheduler().unwrap();
    let n = candidate.value().night().unwrap();
    assert_eq!(
        (a.method(), a.prompt(), a.output_token_budget()),
        (s.0[0].0, s.0[0].1.as_str(), s.0[0].2)
    );
    assert_eq!(
        (n.method(), n.prompt(), n.output_token_budget()),
        (s.0[1].0, s.0[1].1.as_str(), s.0[1].2)
    );
    assert_eq!(a.presence_epoch(), 0);
    assert_eq!(n.presence_epoch(), Some(0));
    assert_ne!(a.semantic(), n.semantic());
}
#[test]
fn checkpoint_cognition_inputs_every_closed_record_field_required_and_duplicate_unknown_refused() {
    let (e, _) = dual();
    let raw = bytes(&e, 0.3);
    let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    for path in [
        "",
        "/scheduler",
        "/scheduler/semantic",
        "/night",
        "/night/semantic",
    ] {
        let object = v.pointer(path).unwrap().as_object().unwrap();
        for (key, value) in object {
            let mut bad = v.clone();
            bad.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            refuse(&serde_json::to_vec(&bad).unwrap(), &e);
            let row = serde_json::to_string(object).unwrap();
            let duplicate = format!(
                "{{{}:{},{}",
                serde_json::to_string(key).unwrap(),
                value,
                &row[1..]
            );
            let original = String::from_utf8(raw.clone()).unwrap();
            // Canonical row ordering differs in Value; substitute in Value's
            // canonical serialization so duplicate lexical keys stay visible.
            let canonical = serde_json::to_string(&v).unwrap();
            let bad = canonical.replacen(&row, &duplicate, 1);
            assert_ne!(bad, canonical, "{original}");
            refuse(bad.as_bytes(), &e);
        }
        let mut bad = v.clone();
        bad.pointer_mut(path).unwrap()["unknown"] = json!(0);
        refuse(&serde_json::to_vec(&bad).unwrap(), &e);
    }
    for (path, value) in [
        ("/scheduler/method", json!({"request_with_budget":null})),
        ("/scheduler/lane", json!({"idle":null})),
        ("/night/method", json!("request_with_budget")),
        ("/night/subject", json!({"ward":{"weigh":null}})),
        ("/scheduler/semantic/producer", json!(3)),
        ("/scheduler/request_id", json!(99)),
        ("/night/owed_day", json!(1)),
        ("/scheduler/output_token_budget", json!(null)),
        ("/night/output_token_budget", json!(0)),
        ("/scheduler/prompt", json!("new prompt")),
        ("/night/presence_epoch", json!(null)),
        ("/scheduler", json!(null)),
        ("/night", json!(null)),
        ("/version", json!(2)),
        ("/player_id", json!("mjr01")),
        ("/scheduler/output_token_budget", json!(4294967296u64)),
        ("/night/output_token_budget", json!(-1)),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(path).unwrap() = value;
        refuse(&serde_json::to_vec(&bad).unwrap(), &e);
    }
}
#[test]
fn checkpoint_cognition_inputs_raw_admission_retained_and_context_does_not_allocate() {
    let (e, _) = dual();
    let raw = bytes(&e, 0.3);
    let mut padded = vec![b' '; 8192];
    padded.extend(&raw);
    padded.extend_from_slice(b"\n\t ");
    let b = CheckpointBudget::default();
    let c = e.cognition_inputs_checkpoint_context(at(0.3));
    let mut held = reserve(&b, Cohort::SavePayload, 0);
    held.resize(crate::checkpoint::MAX_RESIDENT_BYTES - 4096)
        .unwrap();
    assert!(
        EngineCognitionInputsDtoV1::decode(
            &padded,
            reserve(&CheckpointBudget::default(), Cohort::SavePayload, 0),
            c
        )
        .is_err(),
        "wrong cohort fails before context validation"
    );
    assert!(
        e.export_cognition_inputs_checkpoint(at(0.3), reserve(&b, Cohort::LoadCandidate, 0))
            .is_err()
    );
    drop(held);
    assert_eq!(b.retained_bytes(), 0);
    let d = EngineCognitionInputsDtoV1::decode(
        &padded,
        reserve(&b, Cohort::LoadCandidate, padded.len()),
        c,
    )
    .unwrap();
    let charge = b.retained_bytes();
    assert!(charge > d.value().cost().unwrap().peak_bytes);
    let candidate = d.into_candidate(c).unwrap();
    assert_eq!(b.retained_bytes(), charge);
    drop(candidate);
    assert_eq!(b.retained_bytes(), 0);
    use std::mem::size_of as sz;
    assert!(sz::<EngineCognitionInputsDtoV1>() < 512 + 5 * 64);
    assert!(sz::<SchedulerInput>() < 512 + 8 * 64);
    assert!(sz::<NightInput>() < 512 + 8 * 64);
    println!(
        "cognition_inputs_layout dto={} candidate={} scheduler_row={} night_row={} context={} budget={} working={}",
        sz::<EngineCognitionInputsDtoV1>(),
        sz::<EngineCognitionInputsCandidate>(),
        sz::<SchedulerInput>(),
        sz::<NightInput>(),
        sz::<CognitionInputsCheckpointContext>(),
        sz::<AcceptedOutputBudget>(),
        VALIDATION_WORKING_BYTES
    );
}
#[test]
fn checkpoint_cognition_inputs_new_fixture_and_legacy_copy_refuses_recapture() {
    let (mut e, _) = dual();
    let raw = bytes(&e, 0.3);
    assert_eq!(
        raw,
        include_bytes!("../../../tests/fixtures/checkpoint_cognition_inputs/dual-v1.json")
    );
    e.scheduler = scheduler_owner::copy(&e.scheduler);
    assert!(
        e.export_cognition_inputs_checkpoint(
            at(0.3),
            reserve(&CheckpointBudget::default(), Cohort::SavePayload, 0)
        )
        .is_err()
    );
    let (mut e, _) = dual();
    e.night = night_owner::copy(&e.night);
    assert!(
        e.checkpoint_cognition_inputs_cost(
            at(0.3),
            reserve(&CheckpointBudget::default(), Cohort::SavePayload, 0)
        )
        .is_err()
    );
}
#[test]
#[ignore = "create only new M2a14 fixture"]
fn create_cognition_inputs_fixture() {
    let (e, _) = dual();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_cognition_inputs/dual-v1.json");
    assert!(!path.exists() || path.metadata().unwrap().len() == 0);
    std::fs::write(path, bytes(&e, 0.3)).unwrap();
}
fn with_saved(
    e: &Engine,
    t: f64,
    scheduler_time: f64,
    mutate: impl FnOnce(&mut serde_json::Value, &mut serde_json::Value),
    test: impl FnOnce(CognitionInputsCheckpointContext<'_>),
) {
    use crate::engine::{
        night_checkpoint::EngineNightDtoV1, scheduler_checkpoint::EngineSchedulerDtoV1,
    };
    let b = CheckpointBudget::default();
    let body = e
        .world
        .export_backbone_checkpoint(reserve(&b, Cohort::SavePayload, 0))
        .unwrap()
        .encode()
        .unwrap();
    let backbone = crate::world::WorldBackboneDtoV1::decode(
        body.value(),
        reserve(&b, Cohort::LoadCandidate, body.value().len()),
        &e.world.item_catalog,
        &e.world.command_ledger,
    )
    .unwrap()
    .into_candidate(&e.world.item_catalog, &e.world.command_ledger)
    .unwrap();
    let ledger = e
        .world
        .command_ledger
        .checkpoint_v1(
            at(t),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, CommandLedgerDtoV1::WORKING_BYTES)
                .unwrap(),
        )
        .unwrap();
    let scheduler = e
        .export_scheduler_checkpoint(
            at(scheduler_time),
            reserve(&CheckpointBudget::default(), Cohort::SavePayload, 0),
        )
        .unwrap()
        .encode()
        .unwrap();
    let night = e
        .export_night_checkpoint(
            at(t),
            reserve(&CheckpointBudget::default(), Cohort::SavePayload, 0),
        )
        .unwrap()
        .encode()
        .unwrap();
    let mut sw: serde_json::Value = serde_json::from_slice(scheduler.value()).unwrap();
    let mut nw: serde_json::Value = serde_json::from_slice(night.value()).unwrap();
    mutate(&mut sw, &mut nw);
    let sw = serde_json::to_vec(&sw).unwrap();
    let nw = serde_json::to_vec(&nw).unwrap();
    let sc = EngineSchedulerCheckpointContext::from_backbone(
        backbone.value(),
        at(scheduler_time),
        ledger.value(),
        &e.config.player_id,
    );
    let nc =
        NightCheckpointContext::from_backbone(backbone.value(), at(t), &e.clock, ledger.value());
    let scheduler = EngineSchedulerDtoV1::decode(
        &sw,
        reserve(
            &CheckpointBudget::default(),
            Cohort::LoadCandidate,
            sw.len(),
        ),
        sc,
    )
    .unwrap()
    .into_candidate(sc)
    .unwrap();
    let night = EngineNightDtoV1::decode(
        &nw,
        reserve(
            &CheckpointBudget::default(),
            Cohort::LoadCandidate,
            nw.len(),
        ),
        nc,
    )
    .unwrap()
    .into_candidate(nc)
    .unwrap();
    test(CognitionInputsCheckpointContext::from_components(
        scheduler.value(),
        night.value(),
        backbone.value(),
        at(t),
        ledger.value(),
        &e.clock,
    ));
}
#[test]
fn checkpoint_cognition_inputs_saved_component_all_boundary_bits_are_independent_bindings() {
    let e = engine();
    let raw = bytes(&e, 0.0);
    for path in ["/boundary", "/night/boundary", "/world/boundary"] {
        with_saved(
            &e,
            0.0,
            0.0,
            |_, night| *night.pointer_mut(path).unwrap() = json!(-0.0),
            |c| {
                let b = CheckpointBudget::default();
                assert!(
                    EngineCognitionInputsDtoV1::decode(
                        &raw,
                        reserve(&b, Cohort::LoadCandidate, raw.len()),
                        c
                    )
                    .is_err(),
                    "{path}"
                );
                assert_eq!(b.retained_bytes(), 0);
            },
        );
    }
    with_saved(
        &e,
        0.0,
        -0.0,
        |_, _| {},
        |c| {
            assert!(
                EngineCognitionInputsDtoV1::decode(
                    &raw,
                    reserve(
                        &CheckpointBudget::default(),
                        Cohort::LoadCandidate,
                        raw.len()
                    ),
                    c
                )
                .is_err()
            );
        },
    );
}
#[test]
fn checkpoint_cognition_inputs_sidecar_preserves_full_option_domain_without_current_reconstruction()
{
    let (e, _) = dual();
    let raw = bytes(&e, 0.3);
    with_saved(
        &e,
        0.3,
        0.3,
        |_, _| {},
        |c| {
            for budget in [None, Some(0), Some(u32::MAX)] {
                let mut wire: serde_json::Value = serde_json::from_slice(&raw).unwrap();
                wire["scheduler"]["output_token_budget"] = json!(budget);
                wire["night"]["output_token_budget"] = json!(budget);
                let input = serde_json::to_vec(&wire).unwrap();
                let saved = EngineCognitionInputsDtoV1::decode(
                    &input,
                    reserve(
                        &CheckpointBudget::default(),
                        Cohort::LoadCandidate,
                        input.len(),
                    ),
                    c,
                )
                .unwrap()
                .into_candidate(c)
                .unwrap();
                assert_eq!(
                    saved.value().scheduler().unwrap().output_token_budget(),
                    budget
                );
                assert_eq!(saved.value().night().unwrap().output_token_budget(), budget);
            }
        },
    );
}
