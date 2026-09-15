//! Coordinator-owned behavior tests. The sole shared adoption helper roundtrips
//! the real pending-owner codecs and calls production preparation; assertions
//! and service recordings here are independent. Whole-host adoption is M3.
use super::*;
use crate::{
    RequestId, RuntimeGeneration,
    attention::IdleGate,
    checkpoint::{CheckpointBudget, Cohort, complete::CheckpointCategory as Category},
    timeline::LogicalTime,
};
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
struct Accepted {
    method: &'static str,
    prompt: String,
    budget: Option<u32>,
    id: RequestId,
}
#[derive(Default)]
struct Calls {
    accepted: Vec<Accepted>,
    attempted: usize,
    busy: bool,
    next: u64,
}
struct Recorded(Rc<RefCell<Calls>>);
impl Recorded {
    fn accept(
        &mut self,
        method: &'static str,
        prompt: String,
        budget: Option<u32>,
    ) -> std::result::Result<RequestId, crate::CognitionBusy> {
        let mut calls = self.0.borrow_mut();
        calls.attempted += 1;
        if calls.busy {
            return Err(crate::CognitionBusy);
        }
        calls.next += 1;
        let id = RequestId(calls.next);
        calls.accepted.push(Accepted {
            method,
            prompt,
            budget,
            id,
        });
        Ok(id)
    }
}
impl Cognition for Recorded {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, crate::CognitionBusy> {
        panic!("original method lost")
    }
    fn request_with_budget(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> std::result::Result<RequestId, crate::CognitionBusy> {
        self.accept("scheduler", prompt, budget)
    }
    fn request_night(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> std::result::Result<RequestId, crate::CognitionBusy> {
        self.accept("night", prompt, budget)
    }
}
fn fixture() -> (Engine, Rc<RefCell<Calls>>) {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut engine = continuation::demo_engine(
        EngineConfig {
            fake_mode: true,
            turn_delay_seconds: 0.0,
            maximum_backoff_seconds: 60.0,
            idle_mode: IdleCognitionMode::All,
            idle_requires_news: true,
            knowledge_enabled: true,
            tts_selected: TtsBackendKind::Off,
            ..Default::default()
        },
        Box::new(Recorded(Rc::clone(&calls))),
    );
    engine.scheduler = NpcScheduler::new(vec![id("sv3n1"), id("cb947")], 0.0, 60.0, 0.0);
    engine.scheduler.start(0.0);
    for actor in engine.world.characters.values_mut() {
        actor.state.inbox.clear();
        actor.state.pending_history.clear();
    }
    (engine, calls)
}
fn id(value: &str) -> ActorId {
    ActorId::from_raw(value)
}
fn at(value: f64) -> LogicalTime {
    LogicalTime::new(value).unwrap()
}
fn snapshot(engine: &Engine, now: f64, category: Category) -> Value {
    let budget = CheckpointBudget::default();
    let mut reservation = budget
        .reserve(Cohort::SavePayload, 64 * 1024 * 1024)
        .unwrap();
    let mut bytes = Vec::new();
    engine
        .complete_write_category(category, at(now), &mut bytes, &mut reservation)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn scheduler_base(value: &Value) -> &Value {
    if value["version"] == 2 {
        &value["base"]["scheduler"]
    } else {
        &value["scheduler"]
    }
}
fn scheduler_step(
    engine: &mut Engine,
    now: f64,
    gate: IdleGate<'_>,
    completions: Vec<Completion>,
    floor_busy: bool,
) -> (Vec<SchedulerEvent>, Option<ActorId>) {
    let events = engine.scheduler.poll(
        now,
        &mut engine.world,
        &mut engine.transcript,
        &mut completions.into_iter().collect(),
        floor_busy,
        gate,
        engine.cognition.as_mut(),
        &engine.env,
    );
    let submitted = engine.scheduler.take_submitted();
    engine.world.command_ledger.drain_updates();
    (events, submitted)
}
fn done(request: RequestId, reply: &str) -> Completion {
    Completion {
        request_id: request,
        result: Ok(reply.into()),
        duration_seconds: 0.25,
    }
}
fn last(calls: &Rc<RefCell<Calls>>) -> Accepted {
    calls.borrow().accepted.last().unwrap().clone()
}
fn same_input(actual: &Accepted, expected: &Accepted) {
    assert_eq!(actual.method, expected.method);
    assert_eq!(actual.prompt, expected.prompt);
    assert_eq!(actual.budget, expected.budget);
}

#[test]
fn deferred_empty_idle_and_new_protected_work_survive_second_load_without_rerender() {
    let (mut engine, calls) = fixture();
    let a = id("sv3n1");
    let b = id("cb947");
    assert!(engine.world.characters[&a].state.inbox.is_empty());
    engine
        .world
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .recent_history
        .push("earlier archived observation".into());
    engine
        .world
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .pending_history
        .push("old presented observation".into());
    assert_eq!(
        scheduler_step(&mut engine, 0.0, IdleGate::All, vec![], false).1,
        Some(a.clone())
    );
    let original = last(&calls);
    assert!(original.prompt.contains("earlier archived observation"));
    let submitted_state = snapshot(&engine, 0.0, Category::Scheduler);
    let flight = &scheduler_base(&submitted_state)["in_flight"];
    assert!(flight["drained_events"].as_array().unwrap().is_empty());
    assert_eq!(
        flight["presented"],
        serde_json::json!(["old presented observation"])
    );
    let cursor =
        scheduler_base(&snapshot(&engine, 0.0, Category::Scheduler))["round_robin_index"].clone();
    engine = continuation::adopt_for_test(engine, at(0.0), RuntimeGeneration(2), &[]);
    assert_eq!(engine.scheduler.load_retry_count(), 1);
    assert!(engine.scheduler.in_flight_actor_id().is_none());
    engine
        .world
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .inbox
        .push("arrived after saved prompt".into());
    engine
        .world
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .pending_history
        .push("later observation".into());
    let empty_stage = BTreeSet::new();
    scheduler_step(&mut engine, 0.1, IdleGate::Suppressed, vec![], false);
    assert_eq!(
        calls.borrow().accepted.len(),
        1,
        "composition must defer old background work"
    );
    engine
        .world
        .characters
        .get_mut(&b)
        .unwrap()
        .state
        .inbox
        .push("new protected player question".into());
    assert!(
        engine
            .scheduler
            .prioritize_player_reaction(&engine.world, &b, 0.2)
    );
    scheduler_step(
        &mut engine,
        0.2,
        IdleGate::Stage(&empty_stage),
        vec![],
        false,
    );
    let protected = last(&calls);
    assert!(protected.prompt.contains("new protected player question"));
    assert_eq!(engine.scheduler.in_flight_actor_id(), Some(&b));
    let mixed = snapshot(&engine, 0.2, Category::Scheduler);
    assert_eq!(
        mixed["continuation"]["load_retries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!scheduler_base(&mixed)["in_flight"].is_null());
    assert_eq!(scheduler_base(&mixed)["round_robin_index"], cursor);
    engine = continuation::adopt_for_test(engine, at(0.2), RuntimeGeneration(3), &[]);
    assert_eq!(engine.scheduler.load_retry_count(), 2);
    let inboxes: Vec<_> = [&a, &b]
        .map(|actor| engine.world.characters[actor].state.inbox.clone())
        .into();
    calls.borrow_mut().busy = true;
    let accepted_before = calls.borrow().accepted.len();
    let events = scheduler_step(
        &mut engine,
        0.3,
        IdleGate::Stage(&empty_stage),
        vec![],
        false,
    )
    .0;
    assert!(events.is_empty(), "Busy invented a failure/publication");
    assert_eq!(calls.borrow().accepted.len(), accepted_before);
    assert_eq!(engine.scheduler.load_retry_count(), 2);
    assert_eq!(
        [&a, &b]
            .map(|actor| engine.world.characters[actor].state.inbox.clone())
            .to_vec(),
        inboxes
    );
    assert_eq!(
        scheduler_base(&snapshot(&engine, 0.3, Category::Scheduler))["provider_failures"],
        0
    );
    calls.borrow_mut().busy = false;
    assert!(
        scheduler_step(
            &mut engine,
            1.4,
            IdleGate::Stage(&empty_stage),
            vec![],
            false
        )
        .1
        .is_none()
    );
    let new_protected = last(&calls);
    same_input(&new_protected, &protected);
    assert_ne!(new_protected.id, protected.id);
    assert_eq!(engine.scheduler.in_flight_actor_id(), Some(&b));
    assert!(
        scheduler_step(
            &mut engine,
            1.5,
            IdleGate::Stage(&empty_stage),
            vec![done(new_protected.id, "wait {}")],
            false
        )
        .1
        .is_none()
    );
    let retried = last(&calls);
    same_input(&retried, &original);
    assert_ne!(retried.id, original.id);
    assert_eq!(engine.scheduler.in_flight_actor_id(), Some(&a));
    assert_eq!(
        engine.world.characters[&a].state.inbox,
        ["arrived after saved prompt"]
    );
    assert_eq!(
        engine.world.characters[&a].state.pending_history,
        ["later observation"]
    );
    assert_eq!(
        scheduler_base(&snapshot(&engine, 1.5, Category::Scheduler))["round_robin_index"],
        cursor
    );
    engine.scheduler.close();
    let events = scheduler_step(
        &mut engine,
        1.6,
        IdleGate::Stage(&empty_stage),
        vec![done(retried.id, "wait {}")],
        false,
    )
    .0;
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
            .count(),
        1
    );
    assert_eq!(engine.scheduler.load_retry_count(), 0);
    assert_eq!(calls.borrow().accepted.len(), 4);
    assert_eq!(
        engine.world.characters[&a]
            .recent_history()
            .iter()
            .filter(|line| line.as_str() == "old presented observation")
            .count(),
        1,
        "the original presented receipt must graduate exactly once"
    );
    assert_eq!(
        engine.world.characters[&a].state.inbox,
        ["arrived after saved prompt"]
    );
    assert_eq!(
        engine.world.characters[&a].state.pending_history,
        ["later observation"]
    );
}

#[test]
fn resumed_prompt_uses_original_occasion_and_preserves_newer_same_actor_and_global_context() {
    use crate::knowledge::{Topic, mint::mint_claim};
    let (mut engine, calls) = fixture();
    let a = id("sv3n1");
    let b = id("cb947");
    let c = id("k0fb1");
    let days = engine.clock.at(0.0).game_days();
    let old_key = mint_claim(
        &mut engine.world,
        &a,
        Topic::Talk,
        "An old story about a copper latch".into(),
        vec![b.clone()],
        None,
        Some(days),
    )
    .unwrap();
    let question = format!("Tell me about {}", engine.world.characters[&b].name());
    engine
        .world
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .inbox
        .push(question);
    engine
        .world
        .knowledge
        .note_occasion(&a, Some(b.clone()), Some(c.clone()), days);
    scheduler_step(&mut engine, 0.0, IdleGate::All, vec![], false);
    let original_input = last(&calls);
    let original_occasion = engine.world.knowledge.occasion(&a).unwrap().clone();
    assert!(
        original_occasion.offered,
        "fixture did not offer the original occasion"
    );
    let original_keys = engine.world.knowledge.take_seated(&a);
    assert!(
        original_keys.contains(&old_key),
        "fixture did not seat the original fact"
    );
    engine
        .world
        .knowledge
        .note_seated(&a, original_keys.clone());
    engine = continuation::adopt_for_test(engine, at(0.0), RuntimeGeneration(2), &[]);
    assert!(engine.world.knowledge.occasion(&a).is_none());
    let original_context =
        snapshot(&engine, 0.0, Category::Scheduler)["continuation"]["load_retries"][0]["context"]
            .clone();
    let new_key = mint_claim(
        &mut engine.world,
        &a,
        Topic::Bread,
        "A new story about the price of flour".into(),
        vec![c.clone()],
        None,
        Some(days),
    )
    .unwrap();
    engine.world.knowledge.note_occasion(
        &a,
        Some(c.clone()),
        Some(b.clone()),
        engine.clock.at(0.1).game_days(),
    );
    let question = format!(
        "A later player question about {}",
        engine.world.characters[&c].name()
    );
    engine
        .world
        .characters
        .get_mut(&a)
        .unwrap()
        .state
        .inbox
        .push(question);
    assert!(
        engine
            .scheduler
            .prioritize_player_reaction(&engine.world, &a, 0.1)
    );
    let empty = BTreeSet::new();
    scheduler_step(&mut engine, 0.1, IdleGate::Stage(&empty), vec![], false);
    let new_input = last(&calls);
    assert_ne!(new_input.prompt, original_input.prompt);
    let mut newer_occasion = engine.world.knowledge.occasion(&a).unwrap().clone();
    assert!(newer_occasion.offered);
    assert_eq!(newer_occasion.subject, Some(c.clone()));
    let newer_keys = engine.world.knowledge.take_seated(&a);
    assert!(
        newer_keys.contains(&new_key),
        "fixture did not seat the newer fact"
    );
    engine.world.knowledge.note_seated(&a, newer_keys);
    let mixed = snapshot(&engine, 0.1, Category::Scheduler);
    assert_eq!(
        mixed["continuation"]["load_retries"][0]["context"],
        original_context
    );
    engine = continuation::adopt_for_test(engine, at(0.1), RuntimeGeneration(3), &[]);
    let twice = snapshot(&engine, 0.1, Category::Scheduler);
    assert_eq!(
        twice["continuation"]["load_retries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        twice["continuation"]["load_retries"][0]["context"],
        original_context
    );
    scheduler_step(&mut engine, 0.2, IdleGate::Stage(&empty), vec![], false);
    let retried_new = last(&calls);
    same_input(&retried_new, &new_input);
    scheduler_step(
        &mut engine,
        0.3,
        IdleGate::Stage(&empty),
        vec![done(retried_new.id, "wait {}")],
        false,
    );
    let retried_old = last(&calls);
    same_input(&retried_old, &original_input);
    newer_occasion.offered = false;
    assert_eq!(engine.world.knowledge.occasion(&a), Some(&newer_occasion));
    engine.world.knowledge.note_seated(&b, vec![new_key]);
    engine.scheduler.close();
    let prior_facts = engine.world.knowledge.len();
    let result = scheduler_step(&mut engine, 0.4, IdleGate::Stage(&empty), vec![done(retried_old.id,
        "raise_word {\"said\":\"The granary has a hidden passage\",\"topic\":\"omen\"}\nsay {\"text\":\"I heard an old story.\"}")], false).0;
    assert!(
        result
            .iter()
            .any(|event| matches!(event, SchedulerEvent::ActionReceipt(r)
        if r.id.step == 1 && r.outcome.state == crate::receipts::ReceiptState::Completed)),
        "old occasion did not authorize its original action"
    );
    assert_eq!(engine.world.knowledge.len(), prior_facts + 1);
    let minted = engine.world.knowledge.facts().last().unwrap().1;
    assert_eq!(
        minted.subject,
        [b.clone()],
        "completion used the newer occasion's subject"
    );
    assert_eq!(engine.world.knowledge.occasion(&a), Some(&newer_occasion));
    assert_eq!(
        engine.world.knowledge.take_seated(&b),
        [new_key],
        "resumed completion erased another actor's global seated receipt"
    );
    assert!(!engine.scheduler.resumed());
}

#[test]
fn held_success_failure_and_oversized_replies_apply_once_without_a_new_submission() {
    for mode in 0..3 {
        let (mut engine, calls) = fixture();
        let a = id("sv3n1");
        engine
            .world
            .characters
            .get_mut(&a)
            .unwrap()
            .state
            .inbox
            .push("before submission".into());
        scheduler_step(&mut engine, 0.0, IdleGate::All, vec![], false);
        let request = last(&calls).id;
        let completion = Completion {
            request_id: request,
            result: match mode {
                0 => Ok("remember {\"memory\":\"M2c remembered exactly once\"}".into()),
                1 => Err(crate::CognitionError::detailed(
                    "RecordedFailure",
                    "exact recorded provider detail",
                )),
                _ => Ok("x".repeat(crate::MAX_LLM_REPLY_CHARS + 1)),
            },
            duration_seconds: 2.125,
        };
        let events = scheduler_step(
            &mut engine,
            0.1,
            IdleGate::Suppressed,
            vec![completion.clone()],
            true,
        )
        .0;
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, SchedulerEvent::PromptExchange { .. }))
        );
        assert!(engine.scheduler.has_held_result());
        engine.scheduler.close();
        engine
            .world
            .characters
            .get_mut(&a)
            .unwrap()
            .state
            .inbox
            .push("after held result".into());
        engine = continuation::adopt_for_test(engine, at(0.1), RuntimeGeneration(2), &[]);
        assert!(engine.scheduler.has_held_result());
        assert_eq!(engine.scheduler.load_retry_count(), 0);
        assert_eq!(calls.borrow().accepted.len(), 1);
        assert_eq!(
            engine.world.characters[&a].state.inbox,
            ["after held result"]
        );
        scheduler_step(&mut engine, 0.2, IdleGate::Suppressed, vec![], true);
        assert!(engine.scheduler.has_held_result());
        let applied = scheduler_step(&mut engine, 0.3, IdleGate::Suppressed, vec![], false).0;
        let exchanges: Vec<_> = applied
            .iter()
            .filter(|event| matches!(event, SchedulerEvent::PromptExchange { .. }))
            .collect();
        assert_eq!(exchanges.len(), 1);
        assert!(!engine.scheduler.has_held_result());
        assert!(engine.scheduler.in_flight_actor_id().is_none());
        assert_eq!(calls.borrow().accepted.len(), 1);
        if let SchedulerEvent::PromptExchange { exchange } = exchanges[0] {
            let answer = &exchange.answer;
            let error = &exchange.error;
            let duration_seconds = &exchange.duration_seconds;
            assert_eq!(*duration_seconds, 2.125);
            if mode == 0 {
                assert_eq!(answer.as_ref(), completion.result.as_ref().ok());
            } else {
                assert!(error.is_some());
            }
        }
        if mode == 0 {
            assert_eq!(
                engine.world.characters[&a]
                    .state
                    .memories
                    .iter()
                    .filter(|m| m.as_str() == "M2c remembered exactly once")
                    .count(),
                1
            );
            assert_eq!(
                engine.world.characters[&a].state.inbox,
                ["after held result"]
            );
        } else {
            assert_eq!(
                &engine.world.characters[&a].state.inbox[..2],
                ["before submission", "after held result"]
            );
        }
        let before = snapshot(&engine, 0.3, Category::Scheduler);
        let inbox = engine.world.characters[&a].state.inbox.clone();
        let duplicate = scheduler_step(
            &mut engine,
            0.3,
            IdleGate::Suppressed,
            vec![completion],
            false,
        )
        .0;
        assert!(
            !duplicate
                .iter()
                .any(|event| matches!(event, SchedulerEvent::PromptExchange { .. }))
        );
        assert_eq!(snapshot(&engine, 0.3, Category::Scheduler), before);
        assert_eq!(engine.world.characters[&a].state.inbox, inbox);
        assert_eq!(calls.borrow().accepted.len(), 1);
    }
}

fn night_fixture() -> (Engine, Rc<RefCell<Calls>>) {
    let (mut engine, calls) = fixture();
    engine.world = crate::night::checkpoint::tests::world_with_cast();
    engine.clock = WorldClock::new(1.0, Office::Waning, 0, 0.05);
    engine.scheduler = NpcScheduler::new(vec![id("mjr01")], 0.0, 60.0, 0.0);
    engine.scheduler.start(0.0);
    engine.night = NightOffice::new(
        NightOfficeConfig {
            enabled: true,
            majors: true,
            wards: false,
            ambients: false,
        },
        0.0,
        &engine.clock,
    );
    engine.night.seed(&engine.world, &engine.round);
    engine.night.ring(
        0.3,
        &mut engine.world,
        &mut engine.round,
        &engine.clock,
        &mut vec![],
    );
    engine.night.poll(
        0.3,
        &mut engine.world,
        &engine.clock,
        &mut vec![],
        crate::night::checkpoint::tests::open(),
        engine.cognition.as_mut(),
        &engine.env,
    );
    engine.world.command_ledger.drain_updates();
    assert_eq!(calls.borrow().accepted.len(), 1);
    assert_eq!(last(&calls).method, "night");
    assert!(engine.night.in_flight_subject().is_some());
    assert_eq!(
        engine.night.owed(),
        0,
        "fixture must have no ordinary queued Night row"
    );
    (engine, calls)
}

#[test]
fn inactive_night_retry_cannot_consume_new_scheduler_completion_with_reused_numeric_id() {
    let (mut engine, calls) = night_fixture();
    let original = last(&calls);
    engine = continuation::adopt_for_test(engine, at(0.3), RuntimeGeneration(2), &[]);
    assert!(engine.night.load_retry_pending());
    assert!(
        engine.night.could_submit(0.31, &[]),
        "empty ordinary queue hid a pending retry from the lazy gate"
    );
    calls.borrow_mut().next = 0;
    let actor = id("mjr01");
    engine
        .world
        .characters
        .get_mut(&actor)
        .unwrap()
        .state
        .inbox
        .push("new protected question".into());
    engine
        .scheduler
        .prioritize_player_reaction(&engine.world, &actor, 0.31);
    scheduler_step(&mut engine, 0.31, IdleGate::Suppressed, vec![], false);
    let scheduler_request = last(&calls);
    assert_eq!(scheduler_request.method, "scheduler");
    assert_eq!(scheduler_request.id, original.id);
    let mut completions = vec![done(
        scheduler_request.id,
        "remember {\"memory\":\"new scheduler reply survived\"}",
    )];
    let mut blocked = crate::night::checkpoint::tests::open();
    blocked.player_reaction = true;
    let night_events = engine.night.poll(
        0.32,
        &mut engine.world,
        &engine.clock,
        &mut completions,
        blocked,
        engine.cognition.as_mut(),
        &engine.env,
    );
    assert_eq!(
        completions.len(),
        1,
        "inactive Night flight stole the scheduler result"
    );
    assert!(
        !night_events
            .iter()
            .any(|event| matches!(event, SchedulerEvent::PromptExchange { .. }))
    );
    assert!(engine.night.load_retry_pending());
    engine.scheduler.close();
    scheduler_step(&mut engine, 0.32, IdleGate::Suppressed, completions, false);
    assert!(
        engine.world.characters[&actor]
            .state
            .memories
            .iter()
            .any(|m| m == "new scheduler reply survived")
    );
    engine.night.poll(
        0.33,
        &mut engine.world,
        &engine.clock,
        &mut vec![],
        crate::night::checkpoint::tests::open(),
        engine.cognition.as_mut(),
        &engine.env,
    );
    let retried = last(&calls);
    same_input(&retried, &original);
    assert_ne!(retried.id, original.id);
    assert!(!engine.night.load_retry_pending());
}

#[test]
fn restored_night_work_expires_with_its_saved_day_or_person_incarnation() {
    for departed in [false, true] {
        let (engine, calls) = night_fixture();
        let roots: Vec<_> = engine.night.complete_roots().collect();
        assert_eq!(roots.len(), 1);
        let mut engine = continuation::adopt_for_test(engine, at(0.3), RuntimeGeneration(2), &[]);
        assert!(engine.world.command_ledger.is_protected(roots[0]));
        if departed {
            engine
                .world
                .characters
                .get_mut(&id("mjr01"))
                .unwrap()
                .state
                .presence_epoch += 1;
        }
        let now = if departed { 0.31 } else { 1.31 };
        engine.night.poll(
            now,
            &mut engine.world,
            &engine.clock,
            &mut vec![],
            crate::night::checkpoint::tests::open(),
            engine.cognition.as_mut(),
            &engine.env,
        );
        assert_eq!(calls.borrow().accepted.len(), 1);
        assert!(!engine.night.load_retry_pending());
        assert!(engine.night.in_flight_subject().is_none());
        assert!(!engine.world.command_ledger.is_protected(roots[0]));
    }
}

#[test]
fn old_and_unwrapped_callbacks_are_inert_after_preparation_even_with_reused_ids() {
    use crate::{RealtimeResult, TranscriptionJobId, TranscriptionOutcome, TtsOutcome};
    use std::sync::Arc;
    let (mut engine, calls) = fixture();
    scheduler_step(&mut engine, 0.0, IdleGate::All, vec![], false);
    let old_request = last(&calls).id;
    engine = continuation::adopt_for_test(engine, at(0.0), RuntimeGeneration(2), &[]);
    calls.borrow_mut().next = 0;
    scheduler_step(&mut engine, 0.1, IdleGate::All, vec![], false);
    let current_request = last(&calls).id;
    assert_eq!(current_request, old_request);
    let event = SpeechEventId("reused-speech".into());
    engine
        .floor
        .acquire(0.1, &event, "currently readable speech", true);
    engine.floor.bump_player_hold(0.1, 2.0);
    let mut completions = Vec::new();
    let mut output = Vec::new();
    engine.apply_command(
        0.1,
        EngineCommand::InGeneration {
            generation: RuntimeGeneration(2),
            command: Box::new(EngineCommand::PlayerAudioBegin {
                wav_basename: "reused.wav".into(),
                sample_rate: 48_000,
            }),
        },
        &mut completions,
        &mut output,
    );
    engine.world.command_ledger.drain_updates();
    output.clear();
    let before: Vec<_> = [
        Category::Ledger,
        Category::Backbone,
        Category::Scheduler,
        Category::Speech,
        Category::Continuity,
    ]
    .map(|category| (category, snapshot(&engine, 0.1, category)))
    .into();
    let calls_before = calls.borrow().attempted;
    let stale = vec![
        EngineCommand::LlmCompletion(done(
            current_request,
            "remember {\"memory\":\"stale cognition\"}",
        )),
        EngineCommand::Transcription(TranscriptionOutcome::Done {
            job: TranscriptionJobId(1),
            result: Ok("stale batch text".into()),
        }),
        EngineCommand::Transcription(TranscriptionOutcome::Realtime(RealtimeResult::Transcript {
            key: "reused.wav".into(),
            text: "stale realtime text".into(),
        })),
        EngineCommand::Transcription(TranscriptionOutcome::Realtime(RealtimeResult::Failure {
            key: None,
            reason: "stale failure".into(),
        })),
        EngineCommand::Tts(TtsOutcome::Chunk {
            event_id: event.clone(),
            seq: 0,
            sample_rate: 48_000,
            samples: Arc::from([1i16, 2]),
        }),
        EngineCommand::Tts(TtsOutcome::StreamEnd {
            event_id: event.clone(),
            chunk_count: 1,
            first_chunk_ms: 10,
        }),
        EngineCommand::Tts(TtsOutcome::Done {
            event_id: event.clone(),
            result: Ok(Arc::from([1u8, 2])),
        }),
        EngineCommand::BackendStatus(crate::StatusEvent::llm(
            "degraded",
            None,
            Some("stale backend".into()),
        )),
        EngineCommand::SpeechPresented {
            event_id: event.clone(),
        },
        EngineCommand::PlayerAudioBegin {
            wav_basename: "old.wav".into(),
            sample_rate: 48_000,
        },
        EngineCommand::PlayerAudioChunk {
            wav_basename: "reused.wav".into(),
            seq: 0,
            samples: Arc::from([1i16, 2]),
        },
        EngineCommand::PlayerAudioEnd {
            wav_basename: "reused.wav".into(),
            chunk_count: 1,
            silent: false,
        },
        EngineCommand::PlayerAudioAbort {
            wav_basename: "reused.wav".into(),
        },
        EngineCommand::PlayerUtteranceStarted {
            wav_basename: "old.wav".into(),
        },
    ];
    for command in stale {
        engine.apply_command(
            0.1,
            EngineCommand::InGeneration {
                generation: RuntimeGeneration::INITIAL,
                command: Box::new(command.clone()),
            },
            &mut completions,
            &mut output,
        );
        engine.apply_command(0.1, command, &mut completions, &mut output);
        assert!(completions.is_empty());
        assert!(output.is_empty());
    }
    assert_eq!(calls.borrow().attempted, calls_before);
    assert!(engine.floor.is_awaiting(&event));
    for (category, expected) in before {
        assert_eq!(
            snapshot(&engine, 0.1, category),
            expected,
            "stale callback changed {category:?}"
        );
    }
    engine.scheduler.close();
    engine.apply_command(
        0.1,
        EngineCommand::InGeneration {
            generation: RuntimeGeneration(2),
            command: Box::new(EngineCommand::LlmCompletion(done(
                current_request,
                "remember {\"memory\":\"current generation applied\"}",
            ))),
        },
        &mut completions,
        &mut output,
    );
    assert_eq!(
        completions.len(),
        1,
        "positive control: current generation was also discarded"
    );
    scheduler_step(&mut engine, 0.1, IdleGate::Suppressed, completions, false);
    assert!(
        engine.world.characters[&id("sv3n1")]
            .state
            .memories
            .iter()
            .any(|memory| memory == "current generation applied")
    );
    assert_eq!(calls.borrow().attempted, calls_before);
}
