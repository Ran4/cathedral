//! Independent accepted-input boundaries; candidates do not adopt an Engine.
mod prompt_support;

use std::{cell::RefCell, rc::Rc};

use cathedral_sim::{
    ActorId, Capabilities, Engine, EngineConfig, NullSight, NullTranscription, NullTts,
    TtsBackendKind, Vec3, WorldSeed,
    checkpoint::{Admitted, CheckpointBudget, Cohort, MAX_RESIDENT_BYTES, Reservation},
    clock::{Office, WorldClock},
    engine::{
        cognition_inputs_checkpoint::{
            CognitionInputsCheckpointContext, EngineCognitionInputsDtoV1, NightInputSubject,
        },
        night_checkpoint::EngineNightDtoV1,
        scheduler_checkpoint::EngineSchedulerDtoV1,
    },
    ids::RequestId,
    lore::{LoreProfile, Significance},
    night::NightOfficeConfig,
    receipts::CommandLedgerDtoV1,
    timeline::LogicalTime,
    traits::{Cognition, CognitionBusy},
    world::WorldBackboneDtoV1,
};
use serde_json::{Value, json};

fn at(now: f64) -> LogicalTime {
    LogicalTime::new(now).unwrap()
}

fn input_lease(budget: &CheckpointBudget, bytes: &[u8]) -> Reservation {
    budget
        .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
        .unwrap()
}

#[derive(Clone, Debug, PartialEq)]
struct Submission {
    method: &'static str,
    prompt: String,
    budget: Option<u32>,
    request: Option<RequestId>,
}

#[derive(Default)]
struct Recorder {
    busy: bool,
    next_request: u64,
    submissions: Vec<Submission>,
}

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Recorder>>);

impl Capture {
    fn submit(
        &self,
        method: &'static str,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        let mut r = self.0.borrow_mut();
        let request = if r.busy {
            None
        } else {
            let id = RequestId(r.next_request);
            r.next_request += 1;
            Some(id)
        };
        r.submissions.push(Submission {
            method,
            prompt,
            budget,
            request,
        });
        request.ok_or(CognitionBusy)
    }
}

impl Cognition for Capture {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        panic!("the budget-bearing request method must be retained")
    }
    fn request_with_budget(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.submit("request_with_budget", prompt, budget)
    }
    fn request_night(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.submit("request_night", prompt, budget)
    }
}

fn profile(significance: Significance) -> LoreProfile {
    serde_json::from_value(json!({
        "significance": significance, "planning_ward": "fabric", "age": 40,
        "gender": "m", "district": "Fabric Ward", "children": [],
        "circumstances": [], "conditions": [],
        "core_character_description": "You remember the people you speak with.",
        "extended_character_description": "You work in this ward."
    }))
    .unwrap()
}

struct Harness {
    engine: Engine,
    capture: Capture,
    clock: WorldClock,
    now: f64,
}

impl Harness {
    // Some(false) enables person reflections; Some(true) enables ward work.
    fn new(profiled: bool, night: Option<bool>, busy: bool) -> Self {
        let mut seed = WorldSeed::from_json_str(&prompt_support::demo_seed()).unwrap();
        for sheet in &mut seed.characters {
            if sheet.id.as_str() != "player" && profiled {
                sheet.lore = Some(profile(if night == Some(true) {
                    Significance::Minor
                } else {
                    Significance::Major
                }));
            }
        }
        let capture = Capture::default();
        capture.0.borrow_mut().busy = busy;
        let clock = WorldClock::new(4.0, Office::Waning, 0, 0.05);
        let engine = Engine::new(
            EngineConfig {
                clock,
                night_office: NightOfficeConfig {
                    enabled: night.is_some(),
                    majors: night == Some(false),
                    wards: night == Some(true),
                    ambients: false,
                },
                sounds_enabled: false,
                ring_the_offices: false,
                turn_delay_seconds: 0.0,
                maximum_backoff_seconds: 10.0,
                ..EngineConfig::default()
            },
            &seed,
            prompt_support::areas(),
            prompt_support::catalog(),
            prompt_support::prompt_env(),
            Box::new(capture.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (Vec3::ZERO, 0.0),
            0,
            0.0,
        )
        .unwrap();
        Self {
            engine,
            capture,
            clock,
            now: 0.0,
        }
    }

    fn poll(&mut self, now: f64) {
        assert!(now >= self.now && now - self.now <= 0.04 + 1e-12);
        self.engine.poll(now, Vec::new());
        self.now = now;
    }

    fn until_accepted(&mut self, method: &str) {
        for _ in 0..100 {
            if self
                .capture
                .0
                .borrow()
                .submissions
                .iter()
                .any(|s| s.method == method && s.request.is_some())
            {
                return;
            }
            self.poll(self.now + 0.04);
        }
        panic!("no accepted request on {method}");
    }

    fn saved(&self, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
        self.engine
            .export_cognition_inputs_checkpoint(
                at(self.now),
                budget.reserve(Cohort::SavePayload, 4096).unwrap(),
            )
            .unwrap()
            .encode()
            .unwrap()
    }

    fn wire(&self) -> Value {
        serde_json::from_slice(self.saved(&CheckpointBudget::default()).value()).unwrap()
    }
}

fn rejected(bytes: &[u8], context: CognitionInputsCheckpointContext<'_>) {
    let budget = CheckpointBudget::default();
    assert!(
        EngineCognitionInputsDtoV1::decode(bytes, input_lease(&budget, bytes), context,).is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn accepted_default_and_explicit_budgets_survive_later_actor_edits() {
    for profiled in [false, true] {
        let mut h = Harness::new(profiled, None, false);
        h.poll(0.0);
        let original = h.wire();
        let input = &original["scheduler"];
        let accepted = h.capture.0.borrow().submissions[0].clone();
        assert_eq!(accepted.budget, profiled.then_some(2400));
        assert_eq!(input["output_token_budget"], json!(accepted.budget));
        assert_eq!(input["prompt"], accepted.prompt);
        assert_eq!(input["method"], accepted.method);
        assert!(original["night"].is_null());

        let id = ActorId::from_raw(input["actor_id"].as_str().unwrap());
        let actor = h.engine.world_mut().characters.get_mut(&id).unwrap();
        actor.sheet.lore = Some(profile(Significance::Ambient));
        actor.state.goal = "A goal written after the accepted request.".into();
        actor.notify_percept("Later news must stay outside the submitted prompt.");
        assert_eq!(h.wire(), original);

        // An immutable service borrow also detects any attempted request during
        // preflight/export/decode/validation without a new simulation poll.
        let calls = h.capture.0.borrow();
        let budget = CheckpointBudget::default();
        let saved = h.saved(&budget);
        let context = h.engine.cognition_inputs_checkpoint_context(at(h.now));
        let candidate = EngineCognitionInputsDtoV1::decode(
            saved.value(),
            input_lease(&budget, saved.value()),
            context,
        )
        .unwrap()
        .into_candidate(context)
        .unwrap();
        let row = candidate.value().scheduler().unwrap();
        assert_eq!(row.prompt(), accepted.prompt);
        assert_eq!(row.output_token_budget(), accepted.budget);
        assert_eq!(row.request_id(), accepted.request.unwrap());
        assert_eq!(calls.submissions.len(), 1);
        drop((candidate, saved));
        assert_eq!(budget.retained_bytes(), 0);
    }
}

#[test]
fn person_and_ward_reflections_keep_their_original_input() {
    for ward in [false, true] {
        let mut h = Harness::new(true, Some(ward), false);
        h.poll(0.0);
        h.until_accepted("request_night");
        let original = h.wire();
        let accepted = h
            .capture
            .0
            .borrow()
            .submissions
            .iter()
            .find(|s| s.method == "request_night" && s.request.is_some())
            .unwrap()
            .clone();
        assert_eq!(accepted.budget, Some(if ward { 1400 } else { 2400 }));
        assert_eq!(original["night"]["prompt"], accepted.prompt);
        assert_eq!(original["night"]["method"], "request_night");
        for actor in h.engine.world_mut().characters.values_mut() {
            if actor.sheet.id.as_str() != "player" {
                actor.sheet.lore = Some(profile(Significance::Ambient));
                actor.notify_percept("This arrived after the Night prompt was sent.");
            }
        }
        assert_eq!(h.wire(), original);
        let budget = CheckpointBudget::default();
        let saved = h.saved(&budget);
        let context = h.engine.cognition_inputs_checkpoint_context(at(h.now));
        let candidate = EngineCognitionInputsDtoV1::decode(
            saved.value(),
            input_lease(&budget, saved.value()),
            context,
        )
        .unwrap()
        .into_candidate(context)
        .unwrap();
        let row = candidate.value().night().unwrap();
        assert_eq!(row.prompt(), accepted.prompt);
        assert_eq!(row.output_token_budget(), accepted.budget);
        assert_eq!(row.request_id(), accepted.request.unwrap());
        assert_eq!(matches!(row.subject(), NightInputSubject::Ward(_)), ward);
    }
}

#[test]
fn busy_attempt_has_no_accepted_input_and_later_acceptance_resolves_fresh_options() {
    let mut h = Harness::new(false, None, true);
    h.poll(0.0);
    assert_eq!(h.capture.0.borrow().submissions.len(), 1);
    assert!(h.capture.0.borrow().submissions[0].request.is_none());
    assert!(h.wire()["scheduler"].is_null());
    for actor in h.engine.world_mut().characters.values_mut() {
        if actor.sheet.id.as_str() != "player" {
            actor.sheet.lore = Some(profile(Significance::Major));
        }
    }
    h.capture.0.borrow_mut().busy = false;
    h.until_accepted("request_with_budget");
    let calls = h.capture.0.borrow();
    let accepted = calls.submissions.last().unwrap();
    assert_eq!(accepted.budget, Some(2400));
    assert!(accepted.request.is_some());
    let wire = h.wire();
    assert_eq!(wire["scheduler"]["output_token_budget"], 2400);
    assert_eq!(wire["scheduler"]["prompt"], accepted.prompt);
}

#[test]
fn new_wire_is_closed_and_legacy_wire_cannot_claim_the_new_authority() {
    let mut h = Harness::new(true, Some(false), false);
    h.poll(0.0);
    h.until_accepted("request_night");
    let wire = h.wire();
    let context = h.engine.cognition_inputs_checkpoint_context(at(h.now));
    for owner in [None, Some("scheduler"), Some("night")] {
        let object = owner.map_or(&wire, |name| &wire[name]).as_object().unwrap();
        for key in object.keys() {
            let mut bad = wire.clone();
            let target = if let Some(name) = owner {
                &mut bad[name]
            } else {
                &mut bad
            };
            target.as_object_mut().unwrap().remove(key);
            rejected(&serde_json::to_vec(&bad).unwrap(), context);
        }
        let mut bad = wire.clone();
        let target = if let Some(name) = owner {
            &mut bad[name]
        } else {
            &mut bad
        };
        target["unknown"] = json!(true);
        rejected(&serde_json::to_vec(&bad).unwrap(), context);
    }
    for (pointer, value) in [
        ("/version", json!(2)),
        ("/player_id", json!("sv3n1")),
        ("/scheduler", Value::Null),
        ("/night", Value::Null),
        ("/scheduler/method", json!({"request_with_budget": null})),
        ("/scheduler/lane", json!({"idle": null})),
        ("/scheduler/prompt", json!("A newly rendered replacement")),
        ("/scheduler/output_token_budget", Value::Null),
        ("/night/output_token_budget", json!(u32::MAX)),
        ("/night/method", json!("request_with_budget")),
        ("/night/subject", json!({"person": "missing-person"})),
        ("/night/owed_day", json!(99)),
    ] {
        let mut bad = wire.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        rejected(&serde_json::to_vec(&bad).unwrap(), context);
    }
    let duplicate = serde_json::to_string(&wire).unwrap().replacen(
        "\"output_token_budget\":",
        "\"output_token_budget\":null,\"output_token_budget\":",
        1,
    );
    rejected(duplicate.as_bytes(), context);

    let budget = CheckpointBudget::default();
    let old = h
        .engine
        .export_scheduler_checkpoint(
            at(h.now),
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let mut legacy: Value = serde_json::from_slice(old.value()).unwrap();
    assert!(legacy["scheduler"]["in_flight"].is_object());
    assert!(
        legacy["scheduler"]["in_flight"]
            .get("output_token_budget")
            .is_none()
    );
    legacy["scheduler"]["in_flight"]["output_token_budget"] = json!(2400);
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    assert!(
        EngineSchedulerDtoV1::decode(
            &legacy_bytes,
            input_lease(&budget, &legacy_bytes),
            h.engine.scheduler_checkpoint_context(at(h.now)),
        )
        .is_err()
    );
    drop(old);
    let old = h
        .engine
        .export_night_checkpoint(
            at(h.now),
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let mut legacy: Value = serde_json::from_slice(old.value()).unwrap();
    assert!(legacy["night"]["night"]["in_flight"].is_object());
    assert!(
        legacy["night"]["night"]["in_flight"]
            .get("output_token_budget")
            .is_none()
    );
    legacy["night"]["night"]["in_flight"]["output_token_budget"] = json!(2400);
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    assert!(
        EngineNightDtoV1::decode(
            &legacy_bytes,
            input_lease(&budget, &legacy_bytes),
            h.engine.night_checkpoint_context(at(h.now)),
        )
        .is_err()
    );
}

#[test]
fn padded_input_keeps_its_original_lease_through_candidate_conversion() {
    let mut h = Harness::new(false, None, false);
    h.poll(0.0);
    let canonical = serde_json::to_vec(&h.wire()).unwrap();
    let mut bytes = vec![b' '; 1024 * 1024];
    bytes.extend_from_slice(&canonical);
    let budget = CheckpointBudget::default();
    let context = h.engine.cognition_inputs_checkpoint_context(at(h.now));
    let admitted =
        EngineCognitionInputsDtoV1::decode(&bytes, input_lease(&budget, &bytes), context).unwrap();
    let charged = budget.retained_bytes();
    assert!(charged >= 3 * bytes.len());
    let candidate = admitted.into_candidate(context).unwrap();
    assert_eq!(budget.retained_bytes(), charged);
    assert!(
        candidate
            .value()
            .scheduler()
            .unwrap()
            .output_token_budget()
            .is_none()
    );
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - bytes.len() - 4096)
        .unwrap();
    assert!(
        EngineCognitionInputsDtoV1::decode(&bytes, input_lease(&budget, &bytes), context,).is_err()
    );
    assert_eq!(budget.retained_bytes(), running.bytes());
    drop(running);
    rejected(
        &canonical,
        h.engine.cognition_inputs_checkpoint_context(at(-0.0)),
    );
}

#[test]
fn exact_input_binds_unadopted_legacy_components_without_constructing_a_world() {
    let mut h = Harness::new(true, Some(false), false);
    h.poll(0.0);
    h.until_accepted("request_night");
    let now = at(h.now);
    let calls = h.capture.0.borrow();
    // Separate component budgets exercise reference binding only. This is not
    // a claim about complete-save cohort coexistence or a production loader.
    let backbone_budget = CheckpointBudget::default();
    let backbone = h
        .engine
        .world()
        .export_backbone_checkpoint(backbone_budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let backbone = WorldBackboneDtoV1::decode(
        backbone.value(),
        input_lease(&backbone_budget, backbone.value()),
        &h.engine.world().item_catalog,
        &h.engine.world().command_ledger,
    )
    .unwrap()
    .into_candidate(
        &h.engine.world().item_catalog,
        &h.engine.world().command_ledger,
    )
    .unwrap();
    let ledger_budget = CheckpointBudget::default();
    let ledger = h
        .engine
        .world()
        .command_ledger
        .checkpoint_v1(
            now,
            ledger_budget
                .reserve(Cohort::SavePayload, CommandLedgerDtoV1::WORKING_BYTES)
                .unwrap(),
        )
        .unwrap();
    let scheduler_budget = CheckpointBudget::default();
    let scheduler = h
        .engine
        .export_scheduler_checkpoint(
            now,
            scheduler_budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let scheduler_context = h.engine.scheduler_checkpoint_context(now);
    let scheduler = EngineSchedulerDtoV1::decode(
        scheduler.value(),
        input_lease(&scheduler_budget, scheduler.value()),
        scheduler_context,
    )
    .unwrap()
    .into_candidate(scheduler_context)
    .unwrap();
    let night_budget = CheckpointBudget::default();
    let night = h
        .engine
        .export_night_checkpoint(
            now,
            night_budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let night_context = h.engine.night_checkpoint_context(now);
    let night = EngineNightDtoV1::decode(
        night.value(),
        input_lease(&night_budget, night.value()),
        night_context,
    )
    .unwrap()
    .into_candidate(night_context)
    .unwrap();
    let context = CognitionInputsCheckpointContext::from_components(
        scheduler.value(),
        night.value(),
        backbone.value(),
        now,
        ledger.value(),
        &h.clock,
    );
    let budget = CheckpointBudget::default();
    let saved = h.saved(&budget);
    let candidate = EngineCognitionInputsDtoV1::decode(
        saved.value(),
        input_lease(&budget, saved.value()),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(
        candidate.value().scheduler().unwrap().output_token_budget(),
        Some(2400)
    );
    assert_eq!(
        candidate.value().night().unwrap().output_token_budget(),
        Some(2400)
    );
    assert_eq!(
        calls
            .submissions
            .iter()
            .filter(|s| s.request.is_some())
            .count(),
        2
    );
    let mut missing: Value = serde_json::from_slice(saved.value()).unwrap();
    missing["scheduler"]
        .as_object_mut()
        .unwrap()
        .remove("output_token_budget");
    rejected(&serde_json::to_vec(&missing).unwrap(), context);
    let wrong_clock = h.clock.with_scale(h.now, 2.0);
    rejected(
        saved.value(),
        CognitionInputsCheckpointContext::from_components(
            scheduler.value(),
            night.value(),
            backbone.value(),
            now,
            ledger.value(),
            &wrong_clock,
        ),
    );
}
