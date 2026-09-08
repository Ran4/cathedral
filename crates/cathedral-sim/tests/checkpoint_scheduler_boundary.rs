//! Independent scheduler boundaries through ordinary public APIs. Candidates
//! remain read-only; restored continuation is tested inside the owning module.
mod prompt_support;

use cathedral_sim::{
    ActorId, Capabilities, Engine, EngineConfig, FakeCognition, NullSight, NullTranscription,
    NullTts, Presence, TtsBackendKind, Vec3, World, WorldSeed,
    attention::IdleGate,
    checkpoint::{Admitted, CheckpointBudget, Cohort, MAX_RESIDENT_BYTES},
    engine::scheduler_checkpoint::EngineSchedulerDtoV1,
    ids::RequestId,
    scheduler::{
        NpcScheduler, SchedulerEvent,
        checkpoint::{NpcSchedulerDtoV1, SchedulerCheckpointContext},
    },
    timeline::LogicalTime,
    traits::{Cognition, CognitionBusy, CognitionError, Completion},
};
use std::collections::BTreeMap;

fn at(seconds: f64) -> LogicalTime {
    LogicalTime::new(seconds).unwrap()
}
fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

#[derive(Default)]
struct Capture {
    busy: bool,
    prompts: Vec<(String, Option<u32>)>,
}
impl Cognition for Capture {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        panic!("scheduler submissions must retain their output budget")
    }
    fn request_with_budget(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.prompts.push((prompt, budget));
        if self.busy {
            Err(CognitionBusy)
        } else {
            Ok(RequestId(self.prompts.len() as u64 - 1))
        }
    }
}

fn poll(
    s: &mut NpcScheduler,
    w: &mut World,
    c: &mut Capture,
    now: f64,
    done: Vec<Completion>,
    held: bool,
) -> Vec<SchedulerEvent> {
    s.poll(
        now,
        w,
        &mut Vec::new(),
        &mut done.into_iter().collect(),
        held,
        IdleGate::All,
        c,
        &prompt_support::prompt_env(),
    )
}
fn save(s: &NpcScheduler, w: &World, now: f64, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
    s.export_checkpoint(
        SchedulerCheckpointContext::from_world(w, at(now)),
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn round_trip(s: &NpcScheduler, w: &World, now: f64) -> serde_json::Value {
    let budget = CheckpointBudget::default();
    let saved = save(s, w, now, &budget);
    let context = SchedulerCheckpointContext::from_world(w, at(now));
    let candidate = NpcSchedulerDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    let restored = candidate.value().scheduler();
    assert_eq!(restored.turn_order(), s.turn_order());
    assert_eq!(restored.running(), s.running());
    assert_eq!(restored.priority_actor_id(), s.priority_actor_id());
    assert_eq!(restored.in_flight_actor_id(), s.in_flight_actor_id());
    assert_eq!(restored.has_held_result(), s.has_held_result());
    assert_eq!(
        restored.player_reaction_pending(),
        s.player_reaction_pending()
    );
    assert_eq!(
        save(restored, w, now, &CheckpointBudget::default()).value(),
        saved.value()
    );
    let wire = serde_json::from_slice(saved.value()).unwrap();
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
    wire
}

#[test]
fn weighted_cursor_and_both_lanes_preserve_a_followup_for_the_flight_actor() {
    let mut world = prompt_support::seed_world();
    let order = vec![
        actor("sv3n1"),
        actor("sv3n1"),
        actor("cb947"),
        actor("k0fb1"),
    ];
    let mut scheduler = NpcScheduler::new(order.clone(), 0.0, 10.0, 0.0);
    let mut capture = Capture::default();
    scheduler.start(0.0);
    poll(&mut scheduler, &mut world, &mut capture, 0.0, vec![], false);
    poll(
        &mut scheduler,
        &mut world,
        &mut capture,
        0.1,
        vec![Completion {
            request_id: RequestId(0),
            result: Ok("wait {}".into()),
            duration_seconds: 0.1,
        }],
        false,
    );
    world.command_ledger.drain_updates();
    assert_eq!(capture.prompts.len(), 2);
    assert!(scheduler.prioritize(&world, &actor("cb947"), false, 0.2));
    assert!(scheduler.prioritize(&world, &actor("k0fb1"), false, 0.2));
    assert!(scheduler.prioritize_player_reaction(&world, &actor("k0fb1"), 0.2));
    assert!(scheduler.prioritize_player_reaction(&world, &actor("sv3n1"), 0.2));
    assert!(scheduler.prioritize(&world, &actor("sv3n1"), false, 0.2));
    let wire = round_trip(&scheduler, &world, 0.2);
    let state = &wire["scheduler"];
    assert_eq!(
        state["order"],
        serde_json::json!(["sv3n1", "sv3n1", "cb947", "k0fb1"])
    );
    assert_eq!(state["round_robin_index"], 2);
    assert_eq!(state["priority_handoffs"], serde_json::json!(["cb947"]));
    assert_eq!(
        state["player_reactions"],
        serde_json::json!(["k0fb1", "sv3n1"])
    );
    assert_eq!(state["in_flight"]["actor_id"], "sv3n1");
    assert_eq!(state["in_flight"]["lane"], "idle");
    assert_eq!(state["submitted"], "sv3n1");
    assert_eq!(capture.prompts.len(), 2);
}

#[test]
fn submitted_input_stays_exact_after_late_news_and_departure() {
    let mut world = prompt_support::seed_world();
    let sven = actor("sv3n1");
    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .notify_percept("The original event, before submission.");
    let mut scheduler = NpcScheduler::new(vec![sven.clone()], 0.0, 10.0, 0.0);
    scheduler.prioritize_player_reaction(&world, &sven, 0.0);
    scheduler.start(0.0);
    let mut capture = Capture::default();
    poll(&mut scheduler, &mut world, &mut capture, 0.0, vec![], false);
    let character = world.characters.get_mut(&sven).unwrap();
    character.notify_percept("A late event which the submitted prompt never saw.");
    character.state.goal = "A newly changed goal after submission".into();
    let wire = round_trip(&scheduler, &world, 0.1);
    let flight = &wire["scheduler"]["in_flight"];
    assert_eq!(flight["prompt"], capture.prompts[0].0);
    assert_eq!(
        capture.prompts[0].1, None,
        "the legacy demo cast has no lore budget"
    );
    assert_eq!(flight["request_id"], 0, "opaque request zero is valid");
    assert_eq!(flight["lane"], "player_reaction");
    for field in ["drained_events", "presented"] {
        assert!(
            flight[field]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "The original event, before submission.")
        );
        assert!(!flight[field].to_string().contains("A late event"));
    }
    assert!(
        !flight["prompt"]
            .as_str()
            .unwrap()
            .contains("newly changed goal")
    );
    assert!(
        world.characters[&sven]
            .inbox()
            .iter()
            .any(|s| s.contains("A late event"))
    );
    world
        .transition_presence(&[sven], Presence::BeyondTheWalls, &BTreeMap::new())
        .unwrap();
    world.command_ledger.drain_updates();
    let historical = round_trip(&scheduler, &world, 0.1);
    assert_eq!(
        historical, wire,
        "ordinary departure processing owns the stale flight"
    );
    assert_eq!(capture.prompts.len(), 1);
}

#[test]
fn floor_held_success_error_and_oversized_raw_answer_are_not_reinterpreted() {
    for result in [
        Ok("wait {}".to_owned()),
        Err(CognitionError::detailed(
            "Unavailable",
            "provider detail differs from kind",
        )),
        Ok("x".repeat(100_001)),
    ] {
        let expected_failure = match &result {
            Ok(text) => text.chars().count() > 100_000,
            Err(_) => true,
        };
        let mut world = prompt_support::seed_world();
        let mut scheduler = NpcScheduler::new(vec![actor("sv3n1")], 0.0, 10.0, 0.0);
        scheduler.start(0.0);
        let mut capture = Capture::default();
        poll(&mut scheduler, &mut world, &mut capture, 0.0, vec![], false);
        scheduler.close();
        let events = poll(
            &mut scheduler,
            &mut world,
            &mut capture,
            0.1,
            vec![Completion {
                request_id: RequestId(0),
                result: result.clone(),
                duration_seconds: 0.25,
            }],
            true,
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
        );
        let wire = round_trip(&scheduler, &world, 0.1);
        assert_eq!(wire["scheduler"]["running"], false);
        let held = &wire["scheduler"]["held_result"];
        assert_eq!(held["request_id"], 0);
        assert_eq!(held["duration_seconds"], 0.25);
        match &result {
            Ok(text) => assert_eq!(held["result"]["ok"], *text),
            Err(error) => {
                assert_eq!(held["result"]["err"]["kind"], error.kind());
                assert_eq!(held["result"]["err"]["detail"], error.detail());
            }
        }
        // This is the original owner's ordinary continuation. The public
        // candidate deliberately has no partial-install or mutable escape API.
        let applied = poll(&mut scheduler, &mut world, &mut capture, 0.2, vec![], false);
        assert_eq!(
            applied
                .iter()
                .filter(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
                .count(),
            1
        );
        let repeated = poll(&mut scheduler, &mut world, &mut capture, 0.3, vec![], false);
        assert!(
            !repeated
                .iter()
                .any(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
        );
        world.command_ledger.drain_updates();
        let after = round_trip(&scheduler, &world, 0.3);
        assert_eq!(
            after["scheduler"]["provider_failures"],
            u32::from(expected_failure)
        );
        assert!(after["scheduler"]["held_result"].is_null());
        assert_eq!(capture.prompts.len(), 1);
    }
}

#[test]
fn busy_retry_preserves_its_root_and_rejects_an_unrelated_ledger() {
    let mut world = prompt_support::seed_world();
    let sven = actor("sv3n1");
    let mut scheduler = NpcScheduler::new(vec![sven.clone()], 0.0, 10.0, 0.0);
    scheduler.prioritize_player_reaction(&world, &sven, 0.0);
    scheduler.start(0.0);
    let mut capture = Capture {
        busy: true,
        ..Capture::default()
    };
    poll(&mut scheduler, &mut world, &mut capture, 0.0, vec![], false);
    let original = round_trip(&scheduler, &world, 0.0);
    assert!(!original["scheduler"]["retry_work"]["sv3n1"]["semantic"].is_null());
    assert_eq!(
        original["scheduler"]["player_reactions"],
        serde_json::json!(["sv3n1"])
    );
    assert!(original["scheduler"]["in_flight"].is_null());
    world
        .transition_presence(&[sven], Presence::BeyondTheWalls, &BTreeMap::new())
        .unwrap();
    world.command_ledger.drain_updates();
    assert_eq!(round_trip(&scheduler, &world, 0.0), original);
    let bytes = serde_json::to_vec(&original).unwrap();
    let unrelated = prompt_support::seed_world();
    let budget = CheckpointBudget::default();
    assert!(
        NpcSchedulerDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            SchedulerCheckpointContext::from_world(&unrelated, at(0.0))
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(capture.prompts.len(), 1);
}

#[test]
fn strict_records_and_raw_padding_keep_admission_until_candidate_drop() {
    let mut world = prompt_support::seed_world();
    let mut scheduler = NpcScheduler::new(vec![actor("sv3n1")], 0.0, 10.0, 0.0);
    scheduler.start(0.0);
    poll(
        &mut scheduler,
        &mut world,
        &mut Capture::default(),
        0.0,
        vec![],
        false,
    );
    let original = round_trip(&scheduler, &world, 0.0);
    let budget = CheckpointBudget::default();
    let context = SchedulerCheckpointContext::from_world(&world, at(0.0));
    for case in 0..10 {
        let mut value = original.clone();
        let state = &mut value["scheduler"];
        match case {
            0 => {
                state.as_object_mut().unwrap().remove("in_flight").unwrap();
            }
            1 => {
                state
                    .as_object_mut()
                    .unwrap()
                    .remove("held_result")
                    .unwrap();
            }
            2 => {
                state.as_object_mut().unwrap().remove("submitted").unwrap();
            }
            3 => state["unrecognized_authority"] = true.into(),
            4 => state["round_robin_index"] = 1.into(),
            5 => {
                state["player_reactions"] = serde_json::json!(["cb947"]);
                state["priority_handoffs"] = serde_json::json!(["cb947"]);
            }
            6 => {
                state["held_result"] = serde_json::json!({"request_id": 99, "result": {"ok": "wait {}"}, "duration_seconds": 0.1})
            }
            7 => {
                state["retry_work"] = serde_json::json!({"cb947": {"semantic": state["in_flight"]["semantic"], "presence_epoch": 0}})
            }
            8 => state["provider_failures"] = (i32::MAX as u32).into(),
            _ => state["in_flight"]["lane"] = "invented_lane".into(),
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        let Err(error) = NpcSchedulerDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context,
        ) else {
            panic!("malformed case {case} admitted")
        };
        let expected = match case {
            0..=2 => "missing field",
            3 => "unknown field",
            4 => "cursor",
            5 => "duplicate scheduler lane",
            6 => "held scheduler request",
            7 => "duplicate scheduler semantic",
            8 => "failure counter",
            _ => "unknown variant",
        };
        assert!(error.reason.contains(expected), "case {case}: {error:?}");
        assert_eq!(budget.retained_bytes(), 0);
    }
    let padding = 1024 * 1024;
    let mut bytes = vec![b' '; padding];
    bytes.extend_from_slice(&serde_json::to_vec(&original).unwrap());
    let decoded = NpcSchedulerDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained >= canonical_peak + 3 * padding);
    let candidate = decoded.into_candidate(context).unwrap();
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(
        candidate.value().scheduler().in_flight_actor_id(),
        Some(&actor("sv3n1"))
    );
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    let initial = bytes.len() + 4096;
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - initial)
        .unwrap();
    assert!(
        NpcSchedulerDtoV1::decode(
            &bytes,
            budget.reserve(Cohort::LoadCandidate, initial).unwrap(),
            context
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES - initial);
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn pacing_and_original_engine_configuration_are_independent_bit_exact_records() {
    let world = prompt_support::seed_world();
    let scheduler = NpcScheduler::new(vec![], f64::INFINITY, 1.0, f64::MAX);
    let wire = round_trip(&scheduler, &world, -0.0);
    assert_eq!(wire["scheduler"]["minimum_delay_seconds"], "never");
    assert_eq!(wire["scheduler"]["maximum_backoff_seconds"], "never");
    assert_eq!(
        wire["scheduler"]["next_turn_at"]["at"].as_f64(),
        Some(f64::MAX)
    );
    let budget = CheckpointBudget::default();
    let bytes = serde_json::to_vec(&wire).unwrap();
    assert!(
        NpcSchedulerDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            SchedulerCheckpointContext::from_world(&world, at(0.0))
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);

    let delay_bits = 0x7ff8_0000_0000_0179;
    let engine = Engine::new(
        EngineConfig {
            turn_delay_seconds: f64::from_bits(delay_bits),
            maximum_backoff_seconds: f64::NEG_INFINITY,
            ..EngineConfig::default()
        },
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
    let saved = engine
        .export_scheduler_checkpoint(at(0.0), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let context = engine.scheduler_checkpoint_context(at(0.0));
    let candidate = EngineSchedulerDtoV1::decode(
        saved.value(),
        budget
            .reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().turn_delay_seconds().to_bits(), delay_bits);
    assert_eq!(
        candidate.value().maximum_backoff_seconds().to_bits(),
        f64::NEG_INFINITY.to_bits()
    );
    assert_eq!(candidate.value().player_id(), &actor("player"));
    let normalized = round_trip(candidate.value().scheduler(), engine.world(), 0.0);
    assert_eq!(normalized["scheduler"]["minimum_delay_seconds"]["at"], 0.0);
    assert_eq!(
        normalized["scheduler"]["maximum_backoff_seconds"]["at"],
        1.0
    );
    drop(candidate);
    let charge = budget.retained_bytes();
    let mut wrong: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    wrong["player_id"] = "sv3n1".into();
    let bytes = serde_json::to_vec(&wrong).unwrap();
    assert!(
        EngineSchedulerDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), charge);
    drop(saved);
    assert_eq!(budget.retained_bytes(), 0);
}
