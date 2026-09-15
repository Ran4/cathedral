//! Independent Night boundaries through ordinary public APIs; no city adoption.
mod prompt_support;

use cathedral_sim::{
    Capabilities, Engine, EngineConfig, FakeCognition, NullSight, NullTranscription, NullTts,
    Presence, TtsBackendKind, Vec3, World, WorldSeed,
    checkpoint::{Admitted, CheckpointBudget, Cohort},
    clock::{Office, WorldClock},
    engine::night_checkpoint::EngineNightDtoV1,
    ids::RequestId,
    lore::PlanningWard,
    night::{
        NightGate, NightOffice, NightOfficeConfig,
        checkpoint::{NightCheckpointContext, NightOfficeDtoV1, WorldNightDtoV1},
    },
    round::Round,
    timeline::LogicalTime,
    traits::{Cognition, CognitionBusy, CognitionError, Completion},
};
use std::collections::BTreeMap;

fn clock() -> WorldClock {
    WorldClock::new(3600.0, Office::Waning, 0, 0.05)
}

fn at(seconds: f64) -> LogicalTime {
    LogicalTime::new(seconds).unwrap()
}

fn open() -> NightGate {
    NightGate {
        floor_busy: false,
        player_composing: false,
        stage_occupied: false,
        player_reaction: false,
    }
}

fn owed(world: &mut World, clock: &WorldClock) -> NightOffice {
    let mut night = NightOffice::new(
        NightOfficeConfig {
            enabled: true,
            wards: false,
            ambients: false,
            ..NightOfficeConfig::default()
        },
        0.0,
        clock,
    );
    let mut round = Round::new();
    night.seed(world, &round).unwrap();
    night.ring(901.0, world, &mut round, clock, &mut Vec::new());
    assert_eq!(night.owed(), 3, "the compact cast's three Majors are owed");
    night
}

#[derive(Default)]
struct Capture {
    busy: bool,
    prompts: Vec<String>,
}

impl Cognition for Capture {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        panic!("Night must use its own provider lane")
    }

    fn request_night(
        &mut self,
        prompt: String,
        _: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.prompts.push(prompt);
        if self.busy {
            Err(CognitionBusy)
        } else {
            Ok(RequestId(79))
        }
    }
}

fn save_night(
    night: &NightOffice,
    context: NightCheckpointContext<'_>,
    budget: &CheckpointBudget,
) -> Admitted<Vec<u8>> {
    night
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}

fn refuse_current_person_queue(
    night: &NightOffice,
    context: NightCheckpointContext<'_>,
    budget: &CheckpointBudget,
) {
    let before = budget.retained_bytes();
    let error = night
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap_err();
    assert!(error.reason.contains("V2"));
    assert_eq!(budget.retained_bytes(), before);
}

/// Explicit historical V1 codec input, not an export of today's person queue.
/// Current queue-time incarnation capture is exercised by complete V2 tests.
fn historical_night(
    context: NightCheckpointContext<'_>,
    budget: &CheckpointBudget,
    clock: &WorldClock,
    now: f64,
    edit: impl FnOnce(&mut serde_json::Value),
) -> Admitted<Vec<u8>> {
    let engine: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/checkpoint_v1/engine_night.json")).unwrap();
    let mut wire = engine["night"].clone();
    wire["boundary"] = now.into();
    wire["context"]["clock"] = serde_json::to_value(clock.checkpoint_v1(at(now)).unwrap()).unwrap();
    wire["night"]["config"]["wards"] = false.into();
    wire["night"]["config"]["ambients"] = false.into();
    edit(&mut wire["night"]);
    let bytes = serde_json::to_vec(&wire).unwrap();
    let candidate = NightOfficeDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    save_night(candidate.value().night(), context, budget)
}

fn historical_unsubmitted(night: &mut serde_json::Value) {
    let subject = night["in_flight"]["subject"].clone();
    night["queue"].as_array_mut().unwrap().insert(
        0,
        serde_json::json!({"semantic":null,"presence_epoch":null,"subject":subject,"day":0}),
    );
    night["in_flight"] = serde_json::Value::Null;
    night["next_attempt_at"] = serde_json::json!({"at":901.0});
}

#[test]
fn public_ward_moods_keep_exact_text_beyond_the_provider_writer_limit() {
    let mut world = prompt_support::seed_world();
    world.ward_moods = BTreeMap::from([
        (PlanningWard::Fabric, String::new()),
        (PlanningWard::Weigh, " \t\n".into()),
        (PlanningWard::Reed, format!("  {}\n", "å🕯".repeat(601))),
    ]);
    let revision = world.world_revision;
    let budget = CheckpointBudget::default();
    let saved = world
        .export_night_checkpoint(
            at(901.0),
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let clock = clock();
    let context = NightCheckpointContext::from_world(&world, at(901.0), &clock);
    let candidate = WorldNightDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().ward_moods(), &world.ward_moods);
    assert_eq!(world.world_revision, revision);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn ringing_twice_retains_queue_time_stamps_and_an_unspent_lane() {
    let mut world = prompt_support::seed_world();
    let clock = clock();
    let mut night = owed(&mut world, &clock);
    night.ring(
        901.0,
        &mut world,
        &mut Round::new(),
        &clock,
        &mut Vec::new(),
    );
    let budget = CheckpointBudget::default();
    let context = NightCheckpointContext::from_world(&world, at(901.0), &clock);
    assert_eq!(night.owed(), 3);
    assert_eq!(night.totals(), (0, 0));
    assert!(night.wants_slot(901.0));
    refuse_current_person_queue(&night, context, &budget);
    let saved = historical_night(context, &budget, &clock, 901.0, historical_unsubmitted);
    let wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    let data = &wire["night"];
    assert_eq!(data["queue"].as_array().unwrap().len(), 3);
    assert_eq!(data["last_reflected"].as_array().unwrap().len(), 3);
    assert_eq!(data["reflected"], 0);
    for row in data["queue"].as_array().unwrap() {
        assert_eq!(row["day"], 0);
        assert!(row["semantic"].is_null());
        assert!(row["presence_epoch"].is_null());
    }
    let candidate = NightOfficeDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    let restored = candidate.value().night();
    assert!(restored.enabled());
    assert_eq!(restored.owed(), 3);
    assert_eq!(restored.totals(), (0, 0));
    assert!(restored.wants_slot(901.0));
    let repeated = save_night(restored, context, &CheckpointBudget::default());
    assert_eq!(repeated.value(), saved.value());
}

#[test]
fn a_busy_duty_keeps_its_departed_incarnation_and_requires_its_ledger_root() {
    let mut world = prompt_support::seed_world();
    let clock = clock();
    let mut night = owed(&mut world, &clock);
    night.poll(
        901.0,
        &mut world,
        &clock,
        &mut Vec::new(),
        open(),
        &mut Capture {
            busy: true,
            ..Capture::default()
        },
        &prompt_support::prompt_env(),
    );
    let budget = CheckpointBudget::default();
    let initial_context = NightCheckpointContext::from_world(&world, at(901.0), &clock);
    refuse_current_person_queue(&night, initial_context, &budget);
    assert_eq!(night.owed(), 3);
    assert!(!night.wants_slot(905.999));
    assert!(night.wants_slot(906.0));
    let first = historical_night(initial_context, &budget, &clock, 901.0, |n| {
        let semantic = n["in_flight"]["semantic"].clone();
        historical_unsubmitted(n);
        n["queue"][0]["semantic"] = semantic;
        n["queue"][0]["presence_epoch"] = 0.into();
        n["next_attempt_at"] = serde_json::json!({"at":906.0});
    });
    let wire: serde_json::Value = serde_json::from_slice(first.value()).unwrap();
    let row = &wire["night"]["queue"][0];
    let actor = prompt_support::actor(row["subject"]["person"].as_str().unwrap());
    assert_eq!(row["presence_epoch"], 0);
    assert!(!row["semantic"].is_null());
    world
        .transition_presence(&[actor], Presence::BeyondTheWalls, &BTreeMap::new())
        .unwrap();
    let context = NightCheckpointContext::from_world(&world, at(901.0), &clock);
    let candidate = NightOfficeDtoV1::decode(
        first.value(),
        budget
            .reserve(Cohort::LoadCandidate, first.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().night().owed(), 3);
    assert!(!candidate.value().night().wants_slot(905.999));
    assert!(candidate.value().night().wants_slot(906.0));
    let repeated = save_night(
        candidate.value().night(),
        context,
        &CheckpointBudget::default(),
    );
    assert_eq!(
        first.value(),
        repeated.value(),
        "departure is handled by the next ordinary submit"
    );
    drop(candidate);
    let retained = budget.retained_bytes();
    let unrelated = prompt_support::seed_world();
    assert!(
        NightOfficeDtoV1::decode(
            first.value(),
            budget
                .reserve(Cohort::LoadCandidate, first.value().len() + 4096)
                .unwrap(),
            NightCheckpointContext::from_world(&unrelated, at(901.0), &clock),
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), retained);
}

#[test]
fn a_submitted_prompt_is_not_rendered_again_from_changed_live_intent() {
    let mut world = prompt_support::seed_world();
    let clock = clock();
    let mut night = owed(&mut world, &clock);
    let mut capture = Capture::default();
    night.poll(
        901.0,
        &mut world,
        &clock,
        &mut Vec::new(),
        open(),
        &mut capture,
        &prompt_support::prompt_env(),
    );
    let subject = night.in_flight_subject().unwrap();
    world
        .characters
        .get_mut(&prompt_support::actor(&subject))
        .unwrap()
        .state
        .goal = "This intent changed after submission and must not enter the saved prompt".into();
    let clock_changed = clock.with_scale(901.0, 2.0);
    let budget = CheckpointBudget::default();
    let context = NightCheckpointContext::from_world(&world, at(901.0), &clock);
    refuse_current_person_queue(&night, context, &budget);
    assert!(!night.wants_slot(10_000.0));
    let saved = historical_night(context, &budget, &clock, 901.0, |n| {
        n["in_flight"]["request_id"] = 79.into();
        // The historical fixture came from a fully initialized Engine; this
        // public Night-only world has no calendar/wallet/wayfinding setup.
        // Supply its independently recorded original request as the explicit
        // V1 input, then prove the codec ignores the later live goal edit.
        n["in_flight"]["prompt"] = capture.prompts[0].clone().into();
    });
    let wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    assert_eq!(wire["night"]["in_flight"]["prompt"], capture.prompts[0]);
    assert_eq!(wire["night"]["in_flight"]["request_id"], 79);
    assert!(
        NightOfficeDtoV1::decode(
            saved.value(),
            budget
                .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
                .unwrap(),
            NightCheckpointContext::from_world(&world, at(901.0), &clock_changed),
        )
        .is_err(),
        "an equal current calendar position does not identify the clock segment"
    );
    let candidate = NightOfficeDtoV1::decode(
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
        candidate.value().night().in_flight_subject().as_deref(),
        Some(subject.as_str())
    );
    assert!(!candidate.value().night().wants_slot(10_000.0));
    let repeated = save_night(
        candidate.value().night(),
        context,
        &CheckpointBudget::default(),
    );
    assert_eq!(repeated.value(), saved.value());
}

#[test]
fn an_overflowed_pace_survives_a_return_to_an_admitted_clock_scale() {
    let mut world = prompt_support::seed_world();
    let normal = clock();
    let mut night = owed(&mut world, &normal);
    let slow = normal.with_scale(901.0, f64::from_bits(1));
    night.poll(
        901.0,
        &mut world,
        &slow,
        &mut Vec::new(),
        open(),
        &mut Capture::default(),
        &prompt_support::prompt_env(),
    );
    let clock = slow.with_scale(902.0, 1.0);
    night.poll(
        902.0,
        &mut world,
        &clock,
        &mut vec![Completion {
            request_id: RequestId(79),
            result: Err(CognitionError::detailed(
                "Unavailable",
                "provider detail differs from kind",
            )),
            duration_seconds: 0.25,
        }],
        open(),
        &mut Capture::default(),
        &prompt_support::prompt_env(),
    );
    assert_eq!(night.owed(), 2);
    assert!(night.in_flight_subject().is_none());
    assert!(!night.wants_slot(f64::MAX));
    world.command_ledger.drain_updates();
    let budget = CheckpointBudget::default();
    let context = NightCheckpointContext::from_world(&world, at(902.0), &clock);
    refuse_current_person_queue(&night, context, &budget);
    let saved = historical_night(context, &budget, &clock, 902.0, |n| {
        n["in_flight"] = serde_json::Value::Null;
        n["next_attempt_at"] = "never".into();
        n["dropped"] = 1.into();
    });
    let wire: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    assert_eq!(wire["night"]["next_attempt_at"], "never");
    let candidate = NightOfficeDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert!(!candidate.value().night().wants_slot(f64::MAX));
    assert_eq!(candidate.value().night().totals(), night.totals());
    let repeated = save_night(
        candidate.value().night(),
        context,
        &CheckpointBudget::default(),
    );
    assert_eq!(repeated.value(), saved.value());
}

#[test]
fn engine_night_rejects_malformed_records_and_retains_the_raw_input_charge() {
    let config = EngineConfig {
        night_office: NightOfficeConfig {
            enabled: true,
            majors: false,
            wards: true,
            ambients: false,
        },
        ..EngineConfig::default()
    };
    let engine = Engine::new(
        config,
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
    let budget = CheckpointBudget::default();
    let saved = engine
        .export_night_checkpoint(at(0.0), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let context = engine.night_checkpoint_context(at(0.0));
    let charge = budget.retained_bytes();
    let original: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    for case in 0..3 {
        let mut value = original.clone();
        match case {
            0 => value["new_authority"] = true.into(),
            1 => {
                assert!(
                    value["night"]["night"]
                        .as_object_mut()
                        .unwrap()
                        .remove("held_result")
                        .is_some()
                );
            }
            _ => {
                value["night"]["night"]["queue"] = serde_json::json!([{"semantic":null,"presence_epoch":null,"subject":{"person":"sv3n1"},"day":0},{"semantic":null,"presence_epoch":null,"subject":{"person":"sv3n1"},"day":0}]);
                value["night"]["night"]["last_reflected"] =
                    serde_json::json!([{"subject":{"person":"sv3n1"},"day":0}]);
            }
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        let error = EngineNightDtoV1::decode(
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
        assert_eq!(budget.retained_bytes(), charge);
    }
    let padding = 1024 * 1024;
    let mut bytes = vec![b' '; padding];
    bytes.extend_from_slice(saved.value());
    let decoded = EngineNightDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained - charge >= canonical_peak + 3 * padding);
    let candidate = decoded.into_candidate(context).unwrap();
    assert!(candidate.value().night().enabled());
    assert_eq!(
        candidate.value().config_night_office(),
        NightOfficeConfig {
            enabled: true,
            majors: false,
            wards: true,
            ambients: false
        }
    );
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(candidate.value().ward_moods(), &engine.world().ward_moods);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}
