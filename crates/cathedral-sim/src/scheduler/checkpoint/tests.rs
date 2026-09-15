use super::*;
use crate::{
    checkpoint::{CheckpointBudget, Cohort},
    traits::CognitionBusy,
};
use serde_json::json;
use std::collections::BTreeSet;
mod cognition_inputs;
fn at(t: f64) -> LogicalTime {
    LogicalTime::new(t).unwrap()
}
fn world() -> World {
    crate::night::checkpoint::tests::world_with_cast()
}
fn env() -> PromptEnv {
    crate::night::checkpoint::tests::env()
}
fn actor(s: &str) -> ActorId {
    ActorId::from_raw(s)
}
fn reservation() -> Reservation {
    CheckpointBudget::default()
        .reserve(Cohort::SavePayload, 4096)
        .unwrap()
}
fn bytes(n: &NpcScheduler, w: &World, t: f64) -> Vec<u8> {
    n.export_checkpoint(
        SchedulerCheckpointContext::from_world(w, at(t)),
        reservation(),
    )
    .unwrap()
    .encode()
    .unwrap()
    .value()
    .clone()
}
fn decode(raw: &[u8], w: &World, t: f64) -> Result<Admitted<NpcSchedulerDtoV1>> {
    NpcSchedulerDtoV1::decode(
        raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        SchedulerCheckpointContext::from_world(w, at(t)),
    )
}
fn restored(n: &NpcScheduler, w: &World, t: f64) -> NpcScheduler {
    let raw = bytes(n, w, t);
    let c = SchedulerCheckpointContext::from_world(w, at(t));
    let saved = decode(&raw, w, t).unwrap().into_candidate(c).unwrap();
    // Every covered field deliberately diverges, including actual pending
    // records, before the private install; no public partial-install API.
    let mut other = NpcScheduler {
        order: vec![actor("scrambled")],
        minimum_delay_seconds: 77.0,
        maximum_backoff_seconds: 78.0,
        round_robin_index: 0,
        priority_handoffs: VecDeque::from([actor("scramble_h")]),
        player_reactions: VecDeque::from([actor("scramble_p")]),
        in_flight: None,
        load_retries: VecDeque::new(),
        resumed_context: None,
        retry_work: BTreeMap::new(),
        held_result: None,
        next_turn_at: 99.0,
        provider_failures: 99,
        running: !n.running,
        submitted: Some(actor("scramble_s")),
    };
    assert_ne!(bytes(&other, w, t), raw);
    other = copy(&saved.value().data.scheduler);
    assert_eq!(
        bytes(&other, w, t),
        raw,
        "immediate canonical equality before any continuation"
    );
    other
}
#[derive(Default)]
struct Service {
    next: u64,
    busy: bool,
    prompts: Vec<String>,
}
impl Cognition for Service {
    fn request(&mut self, prompt: String) -> std::result::Result<RequestId, CognitionBusy> {
        self.prompts.push(prompt);
        if self.busy {
            Err(CognitionBusy)
        } else {
            self.next += 1;
            Ok(RequestId(self.next))
        }
    }
}
fn poll(
    n: &mut NpcScheduler,
    w: &mut World,
    t: f64,
    done: Vec<Completion>,
    floor: bool,
    gate: IdleGate<'_>,
    s: &mut Service,
) -> Vec<SchedulerEvent> {
    n.poll(
        t,
        w,
        &mut Vec::new(),
        &mut done.clone(),
        floor,
        gate,
        s,
        &env(),
    )
}
fn flight(lane: TurnLane) -> (World, NpcScheduler, Service) {
    let mut w = world();
    let a = actor("mjr01");
    w.characters.get_mut(&a).unwrap().state.inbox = vec!["old event".into()];
    w.characters.get_mut(&a).unwrap().state.pending_history = vec!["old percept".into()];
    let mut n = NpcScheduler::new(vec![a.clone(), a.clone(), actor("mnr01")], 2.0, 60.0, 0.0);
    n.start(0.0);
    match lane {
        TurnLane::PlayerReaction => {
            n.prioritize_player_reaction(&w, &a, 0.0);
        }
        TurnLane::Handoff => {
            n.prioritize(&w, &a, true, 0.0);
        }
        TurnLane::Idle => {}
    }
    let mut s = Service::default();
    poll(&mut n, &mut w, 0.0, vec![], false, IdleGate::All, &mut s);
    assert_eq!(n.in_flight.as_ref().unwrap().lane, lane);
    assert_eq!(n.in_flight.as_ref().unwrap().prompt, s.prompts[0]);
    assert_eq!(n.in_flight.as_ref().unwrap().drained_events, ["old event"]);
    assert_eq!(n.in_flight.as_ref().unwrap().presented, ["old percept"]);
    w.characters
        .get_mut(&a)
        .unwrap()
        .state
        .inbox
        .push("late event".into());
    w.characters
        .get_mut(&a)
        .unwrap()
        .state
        .pending_history
        .push("late percept".into());
    (w, n, s)
}
#[test]
fn checkpoint_scheduler_weighted_fairness_fifo_suppression_and_historical_slots() {
    let w = world();
    let mut a = NpcScheduler::new(
        vec![
            actor("gone"),
            actor("mjr01"),
            actor("mjr01"),
            actor("mnr01"),
        ],
        0.0,
        60.0,
        0.0,
    );
    a.round_robin_index = 2;
    a.prioritize(&w, &actor("mjr01"), false, 0.0);
    a.prioritize(&w, &actor("mnr01"), false, 0.0);
    a.prioritize_player_reaction(&w, &actor("mjr01"), 0.0);
    a.prioritize_player_reaction(&w, &actor("amb01"), 0.0);
    let mut b = restored(&a, &w, 0.0);
    let empty = BTreeSet::new();
    let mut actual = Vec::new();
    for gate in [
        IdleGate::Suppressed,
        IdleGate::Suppressed,
        IdleGate::Suppressed,
        IdleGate::Stage(&empty),
        IdleGate::Stage(&empty),
        IdleGate::All,
        IdleGate::All,
        IdleGate::All,
        IdleGate::All,
    ] {
        let left = a.select_next_actor(gate);
        let right = b.select_next_actor(gate);
        assert_eq!(left, right);
        actual.push(left);
        assert_eq!(bytes(&a, &w, 0.0), bytes(&b, &w, 0.0));
    }
    assert_eq!(
        actual,
        vec![
            Some((actor("mjr01"), TurnLane::PlayerReaction)),
            Some((actor("amb01"), TurnLane::PlayerReaction)),
            None,
            Some((actor("mnr01"), TurnLane::Handoff)),
            None,
            Some((actor("mjr01"), TurnLane::Idle)),
            Some((actor("mnr01"), TurnLane::Idle)),
            Some((actor("gone"), TurnLane::Idle)),
            Some((actor("mjr01"), TurnLane::Idle))
        ]
    );
}
#[test]
fn checkpoint_scheduler_pending_input_and_new_queue_while_flying() {
    for lane in [TurnLane::Idle, TurnLane::Handoff, TurnLane::PlayerReaction] {
        let (mut w, mut a, mut service) = flight(lane);
        a.prioritize_player_reaction(&w, &actor("mjr01"), 0.1);
        a.prioritize(&w, &actor("mnr01"), false, 0.1);
        let mut other = restored(&a, &w, 0.1);
        let mut w2 = w.clone();
        let mut service2 = Service {
            next: service.next,
            ..Default::default()
        };
        assert_eq!(a.take_submitted(), other.take_submitted());
        assert!(a.take_submitted().is_none());
        a.close();
        other.close();
        let done = Completion {
            request_id: RequestId(1),
            result: Ok("remember {\"memory\":\"Exact obligation\"}".into()),
            duration_seconds: -0.0,
        };
        let x = poll(
            &mut a,
            &mut w,
            0.2,
            vec![done.clone()],
            false,
            IdleGate::All,
            &mut service,
        );
        let y = poll(
            &mut other,
            &mut w2,
            0.2,
            vec![done],
            false,
            IdleGate::All,
            &mut service2,
        );
        assert_eq!(x, y);
        assert_eq!(w, w2);
        assert_eq!(bytes(&a, &w, 0.2), bytes(&other, &w2, 0.2));
        let ch = &w.characters[&actor("mjr01")];
        assert_eq!(ch.inbox(), ["late event"]);
        assert_eq!(ch.state.pending_history, ["late percept"]);
        assert!(ch.state.recent_history.iter().any(|x| x == "old percept"));
        assert_eq!(a.priority_actor_id(), Some(&actor("mjr01")));
        assert_eq!(service.prompts.len(), 1);
        assert!(service2.prompts.is_empty());
    }
}
#[test]
fn checkpoint_scheduler_closed_held_success_error_and_oversized_once() {
    for result in [
        Ok("remember {\"memory\":\"Held once\"}".into()),
        Err(CognitionError::detailed(
            "TimeoutError",
            "exact detail\nsecond line",
        )),
        Ok("x".repeat(crate::MAX_LLM_REPLY_CHARS + 1)),
    ] {
        let is_ok = result
            .as_ref()
            .is_ok_and(|x| receipts::provider_reply_within_limit(x));
        let (mut w, mut a, mut service) = flight(TurnLane::PlayerReaction);
        let done = Completion {
            request_id: RequestId(1),
            result,
            duration_seconds: 0.123,
        };
        let empty = poll(
            &mut a,
            &mut w,
            0.1,
            vec![done.clone()],
            true,
            IdleGate::All,
            &mut service,
        );
        assert!(empty.is_empty());
        a.close();
        let mut b = restored(&a, &w, 0.1);
        let mut w2 = w.clone();
        let mut s2 = Service::default();
        let x = poll(
            &mut a,
            &mut w,
            0.2,
            vec![],
            false,
            IdleGate::Suppressed,
            &mut service,
        );
        let y = poll(
            &mut b,
            &mut w2,
            0.2,
            vec![],
            false,
            IdleGate::Suppressed,
            &mut s2,
        );
        assert_eq!(x, y);
        assert_eq!(w, w2);
        assert_eq!(bytes(&a, &w, 0.2), bytes(&b, &w2, 0.2));
        assert_eq!(
            x.iter()
                .filter(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
                .count(),
            1
        );
        assert!(!a.running());
        assert!(!a.has_held_result());
        assert!(a.in_flight.is_none());
        let ch = &w.characters[&actor("mjr01")];
        if is_ok {
            assert!(a.retry_work.is_empty());
            assert_eq!(ch.inbox(), ["late event"]);
        } else {
            assert_eq!(a.provider_failures, 1);
            assert_eq!(a.retry_work.len(), 1);
            assert_eq!(
                ch.inbox(),
                ["old event", "late event", SYSTEM_PROVIDER_FAILED]
            );
            assert_eq!(ch.state.pending_history, ["old percept", "late percept"]);
        }
        let snapshot = w.clone();
        let before = bytes(&a, &w, 0.2);
        let duplicate = poll(
            &mut a,
            &mut w,
            0.3,
            vec![done],
            false,
            IdleGate::All,
            &mut service,
        );
        assert!(
            !duplicate
                .iter()
                .any(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
        );
        assert_eq!(w, snapshot);
        assert_eq!(bytes(&a, &w, 0.2), before);
        assert_eq!(service.prompts.len(), 1);
        assert!(s2.prompts.is_empty());
    }
}
#[test]
fn checkpoint_scheduler_busy_and_failure_lane_courtesy_backoff() {
    for lane in [TurnLane::Idle, TurnLane::Handoff, TurnLane::PlayerReaction] {
        let (mut w, mut a, mut service) = flight(lane);
        // A newer protected queue entry must absorb handoff failure courtesy.
        if lane == TurnLane::Handoff {
            a.prioritize_player_reaction(&w, &actor("mjr01"), 0.1);
        }
        a.close();
        poll(
            &mut a,
            &mut w,
            0.1,
            vec![Completion {
                request_id: RequestId(1),
                result: Err(CognitionError::new("TimeoutError")),
                duration_seconds: 0.1,
            }],
            false,
            IdleGate::All,
            &mut service,
        );
        let root = a.retry_work[&actor("mjr01")].semantic;
        let mut b = restored(&a, &w, 0.1);
        let mut w2 = w.clone();
        let mut s2 = Service {
            next: service.next,
            busy: true,
            ..Default::default()
        };
        service.busy = true;
        a.start(0.2);
        b.start(0.2);
        assert_eq!(
            poll(
                &mut a,
                &mut w,
                0.2,
                vec![],
                false,
                IdleGate::All,
                &mut service
            ),
            poll(&mut b, &mut w2, 0.2, vec![], false, IdleGate::All, &mut s2)
        );
        assert_eq!(w, w2);
        assert_eq!(a.retry_work[&actor("mjr01")].semantic, root);
        assert_eq!(a.provider_failures, 1);
        assert_eq!(bytes(&a, &w, 0.2), bytes(&b, &w2, 0.2));
        assert_eq!(
            a.player_reactions.len(),
            usize::from(lane != TurnLane::Idle)
        );
        assert!(a.priority_handoffs.is_empty());
        service.busy = false;
        s2.busy = false;
        assert_eq!(
            poll(
                &mut a,
                &mut w,
                10.0,
                vec![],
                false,
                IdleGate::All,
                &mut service
            ),
            poll(&mut b, &mut w2, 10.0, vec![], false, IdleGate::All, &mut s2)
        );
        if lane == TurnLane::Idle {
            assert_eq!(
                a.in_flight_actor_id(),
                Some(&actor("mnr01")),
                "Busy consumed the second weighted major slot"
            );
            assert_ne!(a.in_flight.as_ref().unwrap().semantic, root);
            assert_eq!(
                a.retry_work[&actor("mjr01")].semantic,
                root,
                "idle retry waits for its next rotation"
            );
        } else {
            assert_eq!(a.in_flight.as_ref().unwrap().semantic, root);
        }
        assert_eq!(w, w2);
        assert_eq!(bytes(&a, &w, 10.0), bytes(&b, &w2, 10.0));
    }
}
#[test]
fn checkpoint_scheduler_stale_epoch_and_historical_retry_retire_only_on_poll() {
    let (mut w, mut a, mut service) = flight(TurnLane::Idle);
    let stale = actor("gone");
    let root = w.command_ledger.reserve_operation(TURN_PRODUCER).unwrap();
    a.retry_work.insert(
        stale,
        RetryWork {
            semantic: root,
            presence_epoch: u64::MAX,
        },
    );
    w.characters
        .get_mut(&actor("mjr01"))
        .unwrap()
        .state
        .presence_epoch = 1;
    a.held_result = Some(Completion {
        request_id: RequestId(1),
        result: Ok("remember {\"memory\":\"Wrong visit\"}".into()),
        duration_seconds: 1.0,
    });
    a.close();
    let mut b = restored(&a, &w, 1.0);
    assert_eq!(b.retry_work.len(), 1);
    assert!(b.has_held_result());
    let mut w2 = w.clone();
    let mut s2 = Service::default();
    let before = w.characters[&actor("mjr01")].clone();
    assert_eq!(
        poll(
            &mut a,
            &mut w,
            1.1,
            vec![],
            false,
            IdleGate::All,
            &mut service
        ),
        poll(&mut b, &mut w2, 1.1, vec![], false, IdleGate::All, &mut s2)
    );
    assert_eq!(w, w2);
    assert_eq!(w.characters[&actor("mjr01")], before);
    assert!(a.retry_work.is_empty());
    assert!(!w.command_ledger.is_protected(root));
}
#[test]
fn checkpoint_scheduler_known_committed_root_still_admits_replay_guard() {
    let (mut w, mut a, mut service) = flight(TurnLane::Idle);
    let f = a.in_flight.clone().unwrap();
    a.close();
    let done = Completion {
        request_id: f.request_id,
        result: Ok("remember {\"memory\":\"Replay guard\"}".into()),
        duration_seconds: 1.0,
    };
    poll(
        &mut a,
        &mut w,
        1.0,
        vec![done.clone()],
        false,
        IdleGate::All,
        &mut service,
    );
    a.in_flight = Some(f);
    a.held_result = Some(done);
    let mut b = restored(&a, &w, 1.0);
    let before = w.clone();
    let out = poll(
        &mut b,
        &mut w,
        1.1,
        vec![],
        false,
        IdleGate::All,
        &mut Service::default(),
    );
    assert_eq!(w, before);
    assert!(matches!(out.as_slice(), [SchedulerEvent::ActionReceipt(_)]));
}
#[test]
fn checkpoint_scheduler_pacing_raw_bits_and_local_failure_headroom() {
    let w = world();
    for pace in [-0.0, 0.0, 1e300, f64::MAX, f64::INFINITY] {
        let mut n = NpcScheduler::new(vec![], pace, f64::INFINITY, pace);
        n.provider_failures = MAX_PROVIDER_FAILURES;
        let mut b = restored(&n, &w, 0.0);
        assert_eq!(
            b.minimum_delay_seconds.to_bits(),
            n.minimum_delay_seconds.to_bits()
        );
        assert_eq!(b.next_turn_at.to_bits(), pace.to_bits());
        b.backoff_after_failure();
        assert_eq!(b.provider_failures, i32::MAX as u32);
        assert!(
            b.export_checkpoint(
                SchedulerCheckpointContext::from_world(&w, at(0.0)),
                reservation()
            )
            .is_err()
        );
    }
    let n = NpcScheduler::new(vec![], 0.0, 60.0, 0.0);
    let raw = bytes(&n, &w, 0.0);
    assert!(decode(&raw, &w, -0.0).is_err(), "bitwise boundary equality");
}
#[test]
fn checkpoint_scheduler_strict_records_and_binding_refuse_malformed() {
    let (w, mut n, _) = flight(TurnLane::Idle);
    n.held_result = Some(Completion {
        request_id: RequestId(1),
        result: Err(CognitionError::new("Exact")),
        duration_seconds: 0.0,
    });
    let base: serde_json::Value = serde_json::from_slice(&bytes(&n, &w, 0.0)).unwrap();
    for (pointer, value) in [
        ("/version", json!(2)),
        ("/scheduler/round_robin_index", json!(3)),
        ("/scheduler/provider_failures", json!(i32::MAX)),
        ("/scheduler/maximum_backoff_seconds", json!({"at":0.5})),
        ("/scheduler/next_turn_at", json!({"at":-1})),
        (
            "/scheduler/in_flight/semantic/producer",
            json!(receipts::NIGHT_PRODUCER),
        ),
        ("/scheduler/in_flight/semantic/sequence", json!(0)),
        ("/scheduler/held_result/request_id", json!(2)),
        ("/scheduler/held_result/duration_seconds", json!(-1)),
        ("/scheduler/order", json!(["bad id"])),
        ("/scheduler/player_reactions", json!(["mjr01", "mjr01"])),
        (
            "/scheduler/in_flight/prompt",
            json!("x".repeat(MAX_PROMPT_BYTES + 1)),
        ),
        (
            "/scheduler/in_flight/drained_events",
            json!(vec![""; MAX_INPUT_ROWS + 1]),
        ),
        (
            "/scheduler/in_flight/presented",
            json!(["x".repeat(65_537)]),
        ),
        (
            "/scheduler/held_result/result",
            json!({"ok":"x".repeat(MAX_HELD_REPLY_BYTES+1)}),
        ),
    ] {
        let mut x = base.clone();
        *x.pointer_mut(pointer).unwrap() = value;
        assert!(
            decode(&serde_json::to_vec(&x).unwrap(), &w, 0.0).is_err(),
            "{pointer}"
        );
    }
    for path in [
        "/scheduler",
        "/scheduler/in_flight",
        "/scheduler/in_flight/semantic",
        "/scheduler/held_result",
        "/scheduler/held_result/result/err",
    ] {
        let object = base.pointer(path).unwrap().as_object().unwrap();
        for key in object.keys() {
            let mut x = base.clone();
            x.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                decode(&serde_json::to_vec(&x).unwrap(), &w, 0.0).is_err(),
                "missing {path}/{key}"
            );
        }
        let mut x = base.clone();
        x.pointer_mut(path).unwrap()["unknown"] = json!(null);
        assert!(decode(&serde_json::to_vec(&x).unwrap(), &w, 0.0).is_err());
    }
    let mut x = base.clone();
    x["scheduler"]["player_reactions"] = json!(["mjr01"]);
    x["scheduler"]["priority_handoffs"] = json!(["mjr01"]);
    assert!(decode(&serde_json::to_vec(&x).unwrap(), &w, 0.0).is_err());
    let raw = String::from_utf8(bytes(&n, &w, 0.0)).unwrap().replacen(
        "\"running\":true",
        "\"running\":true,\"running\":true",
        1,
    );
    assert!(decode(raw.as_bytes(), &w, 0.0).is_err());
}
#[test]
fn checkpoint_scheduler_maximum_collections_and_duplicate_roots() {
    let mut w = world();
    let mut n = NpcScheduler::new(vec![actor("mjr01"); MAX_ORDER_SLOTS], 0.0, 60.0, 0.0);
    n.round_robin_index = MAX_ORDER_SLOTS - 1;
    n.priority_handoffs = (0..MAX_LANE_ACTORS)
        .map(|i| actor(&format!("h{i}")))
        .collect();
    n.player_reactions = (0..MAX_LANE_ACTORS)
        .map(|i| actor(&format!("p{i}")))
        .collect();
    for i in 0..MAX_RETRY_WORK {
        let semantic = w.command_ledger.reserve_operation(TURN_PRODUCER).unwrap();
        n.retry_work.insert(
            actor(&format!("r{i}")),
            RetryWork {
                semantic,
                presence_epoch: u64::MAX,
            },
        );
    }
    let raw = bytes(&n, &w, 0.0);
    let mut b = restored(&n, &w, 0.0);
    assert_eq!(b.order.len(), MAX_ORDER_SLOTS);
    assert_eq!(b.retry_work.len(), MAX_RETRY_WORK);
    for field in ["order", "priority_handoffs", "player_reactions"] {
        let mut v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        v["scheduler"][field]
            .as_array_mut()
            .unwrap()
            .push(json!("one_more"));
        assert!(
            decode(&serde_json::to_vec(&v).unwrap(), &w, 0.0).is_err(),
            "{field}"
        );
    }
    let work = *b.retry_work.values().next().unwrap();
    b.retry_work.insert(actor("too_many"), work);
    assert!(
        b.export_checkpoint(
            SchedulerCheckpointContext::from_world(&w, at(0.0)),
            reservation()
        )
        .is_err()
    );
    b.retry_work.remove(&actor("too_many"));
    *b.retry_work.values_mut().last().unwrap() = work;
    assert!(
        b.export_checkpoint(
            SchedulerCheckpointContext::from_world(&w, at(0.0)),
            reservation()
        )
        .is_err()
    );
    let mut v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let row = v["scheduler"]["retry_work"]["r0"].clone();
    v["scheduler"]["retry_work"]["extra"] = row.clone();
    assert!(decode(&serde_json::to_vec(&v).unwrap(), &w, 0.0).is_err());
    let text =
        serde_json::to_string(&serde_json::from_slice::<serde_json::Value>(&raw).unwrap()).unwrap();
    let entry = format!("\"r0\":{}", serde_json::to_string(&row).unwrap());
    let duplicate = text.replacen(&entry, &format!("{entry},{entry}"), 1);
    assert_ne!(
        text, duplicate,
        "mutation must actually insert duplicate key"
    );
    let text = duplicate;
    assert!(decode(text.as_bytes(), &w, 0.0).is_err());
}
#[test]
fn checkpoint_scheduler_maximum_input_strings_and_raw_admission_retained() {
    let (w, mut n, _) = flight(TurnLane::Idle);
    let f = n.in_flight.as_mut().unwrap();
    f.request_id = RequestId(u64::MAX);
    f.presence_epoch = u64::MAX;
    f.prompt = "p".repeat(MAX_PROMPT_BYTES);
    f.drained_events = vec!["d".repeat(65_536); MAX_INPUT_ROWS];
    f.presented = vec!["é".repeat(32_768); MAX_INPUT_ROWS];
    n.held_result = Some(Completion {
        request_id: RequestId(u64::MAX),
        result: Ok("x".repeat(MAX_HELD_REPLY_BYTES)),
        duration_seconds: -0.0,
    });
    let raw = bytes(&n, &w, 0.0);
    let b = restored(&n, &w, 0.0);
    assert_eq!(b.in_flight.as_ref().unwrap().prompt.len(), MAX_PROMPT_BYTES);
    assert_eq!(
        b.held_result.as_ref().unwrap().duration_seconds.to_bits(),
        (-0.0f64).to_bits()
    );
    let c = SchedulerCheckpointContext::from_world(&w, at(0.0));
    let budget = CheckpointBudget::default();
    let mut padded = vec![b' '; 200_000];
    padded.extend_from_slice(&raw);
    let expected = aggregate::inspect(&padded).unwrap().peak_bytes + VALIDATION_WORKING_BYTES;
    let d = NpcSchedulerDtoV1::decode(
        &padded,
        budget
            .reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap();
    assert!(d.reserved_bytes() >= expected);
    let candidate = d.into_candidate(c).unwrap();
    assert!(candidate.reserved_bytes() >= expected);
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    let blocked = budget
        .reserve(
            Cohort::Running,
            crate::checkpoint::MAX_RESIDENT_BYTES - expected + 1,
        )
        .unwrap();
    assert!(
        NpcSchedulerDtoV1::decode(
            &padded,
            budget
                .reserve(Cohort::LoadCandidate, padded.len() + 4096)
                .unwrap(),
            c
        )
        .is_err()
    );
    drop(blocked);
    assert_eq!(budget.retained_bytes(), 0);
    let escaped =
        String::from_utf8(raw)
            .unwrap()
            .replacen("pppp", "\\u0070\\u0070\\u0070\\u0070", 1);
    let d = decode(escaped.as_bytes(), &w, 0.0).unwrap();
    assert!(
        d.reserved_bytes()
            >= aggregate::inspect(escaped.as_bytes()).unwrap().peak_bytes
                + VALIDATION_WORKING_BYTES
    );
    assert!(
        decode(
            &vec![b' '; crate::checkpoint::POPULATED_PAYLOAD_BYTES + 1],
            &w,
            0.0
        )
        .is_err()
    );
    assert!(
        decode(
            &format!("{}0{}", "[".repeat(65), "]".repeat(65)).into_bytes(),
            &w,
            0.0
        )
        .is_err()
    );
}
#[test]
fn checkpoint_scheduler_layout_bound_for_sparse_and_raw_shapes() {
    use std::mem::size_of as sz;
    let node = |key: usize, value: usize| 16 + 12 * sz::<usize>() + 11 * (key + value);
    let ledger = 256 * node(sz::<OperationId>(), 0)
        + 4352 * (node(sz::<receipts::CommandId>(), 0) + node(sz::<u64>(), 0));
    let lanes = 2 * MAX_LANE_ACTORS * sz::<&ActorId>();
    let roots = (MAX_RETRY_WORK + 1) * sz::<OperationId>();
    assert!(ledger + 65_536 < VALIDATION_WORKING_BYTES);
    assert!(lanes + roots + 65_536 < VALIDATION_WORKING_BYTES);
    // Even one sparse node per retry row fits its key+object+field charges.
    assert!(node(sz::<ActorId>(), sz::<RetryWork>()) < 64 + 512 + 3 * 64 + 512);
    assert!(2 * sz::<ActorId>() < 64);
    assert!(2 * sz::<String>() < 64);
    assert!(sz::<NpcScheduler>() < 512 + 13 * 64);
    assert!(sz::<InFlight>() < 512 + 8 * 64);
    println!(
        "scheduler_layout owner={} flight={} retry={} completion={} actor={} operation={} command={} retry_node={} dto={} candidate={} context={} ledger_scratch={} lane_scratch={} root_scratch={}",
        sz::<NpcScheduler>(),
        sz::<InFlight>(),
        sz::<RetryWork>(),
        sz::<Completion>(),
        sz::<ActorId>(),
        sz::<OperationId>(),
        sz::<receipts::CommandId>(),
        node(sz::<ActorId>(), sz::<RetryWork>()),
        sz::<NpcSchedulerDtoV1>(),
        sz::<NpcSchedulerCandidate>(),
        sz::<SchedulerCheckpointContext<'_>>(),
        ledger,
        lanes,
        roots
    );
}

#[test]
fn checkpoint_scheduler_unadopted_backbone_and_ledger_context() {
    let (w, n, _) = flight(TurnLane::Idle);
    let raw = bytes(&n, &w, 2.0);
    let bb = CheckpointBudget::default();
    let body = w
        .export_backbone_checkpoint(bb.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let backbone = crate::world::WorldBackboneDtoV1::decode(
        body.value(),
        bb.reserve(Cohort::LoadCandidate, body.value().len() + 4096)
            .unwrap(),
        &w.item_catalog,
        &w.command_ledger,
    )
    .unwrap()
    .into_candidate(&w.item_catalog, &w.command_ledger)
    .unwrap();
    let ledger = w
        .command_ledger
        .checkpoint_v1(
            at(2.0),
            CheckpointBudget::default()
                .reserve(
                    Cohort::SavePayload,
                    crate::receipts::CommandLedgerDtoV1::WORKING_BYTES,
                )
                .unwrap(),
        )
        .unwrap();
    let c = SchedulerCheckpointContext::from_backbone(backbone.value(), at(2.0), ledger.value());
    let b = CheckpointBudget::default();
    let d = NpcSchedulerDtoV1::decode(
        &raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert_eq!(bytes(d.value().scheduler(), &w, 2.0), raw);
    let empty = World::new()
        .command_ledger
        .checkpoint_v1(
            at(2.0),
            CheckpointBudget::default()
                .reserve(
                    Cohort::SavePayload,
                    crate::receipts::CommandLedgerDtoV1::WORKING_BYTES,
                )
                .unwrap(),
        )
        .unwrap();
    drop(d);
    let wrong = SchedulerCheckpointContext::from_backbone(backbone.value(), at(2.0), empty.value());
    assert!(
        NpcSchedulerDtoV1::decode(
            &raw,
            b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
            wrong
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
}
fn fixtures() -> Vec<(&'static str, World, NpcScheduler)> {
    let w = world();
    let mut empty = NpcScheduler::new(vec![], 0.0, f64::INFINITY, 0.0);
    empty.next_turn_at = f64::INFINITY;
    let (w2, pending, _) = flight(TurnLane::Idle);
    let (w3, mut held, _) = flight(TurnLane::PlayerReaction);
    held.close();
    held.prioritize_player_reaction(&w3, &actor("mjr01"), 0.0);
    held.held_result = Some(Completion {
        request_id: RequestId(1),
        result: Err(CognitionError::detailed(
            "TimeoutError",
            "saved terminal\nexact diagnostic",
        )),
        duration_seconds: 0.123,
    });
    vec![
        ("empty-v1.json", w, empty),
        ("pending-idle-v1.json", w2, pending),
        ("held-error-v1.json", w3, held),
    ]
}
#[test]
fn checkpoint_scheduler_supported_component_fixtures() {
    for (name, w, n) in fixtures() {
        let raw = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/checkpoint_scheduler")
                .join(name),
        )
        .unwrap();
        let c = SchedulerCheckpointContext::from_world(&w, at(0.0));
        let d = decode(&raw, &w, 0.0).unwrap().into_candidate(c).unwrap();
        assert_eq!(bytes(d.value().scheduler(), &w, 0.0), raw);
        assert_eq!(bytes(&n, &w, 0.0), raw);
    }
}
#[test]
fn checkpoint_scheduler_receipt_capacity_held_restore_then_release_once() {
    use crate::receipts::{Admission, LEGACY_PRODUCER, MAX_STEPS, RECENT_CAPACITY};
    let (mut w, mut n, mut service) = flight(TurnLane::Idle);
    n.close();
    let blocker = w
        .command_ledger
        .reserve_operation(receipts::NIGHT_PRODUCER)
        .unwrap();
    for step in 0..=MAX_STEPS {
        let Admission::New(ticket) = w.command_ledger.begin(blocker.command(step), &json!(step))
        else {
            panic!("fill protected receipt window")
        };
        w.command_ledger
            .finish(ticket, 0.0, Outcome::completed("held"), vec![]);
    }
    for _ in 0..RECENT_CAPACITY - 1 {
        let id = w.command_ledger.issue(LEGACY_PRODUCER).unwrap().command(0);
        let Admission::New(ticket) = w.command_ledger.begin(id, &json!(null)) else {
            panic!("fill recent receipt window")
        };
        w.command_ledger
            .finish(ticket, 0.0, Outcome::completed("traffic"), vec![]);
    }
    let done = Completion {
        request_id: RequestId(1),
        result: Ok("remember {\"memory\":\"Capacity restored once\"}".into()),
        duration_seconds: 0.123,
    };
    let deferred = poll(
        &mut n,
        &mut w,
        0.1,
        vec![done],
        false,
        IdleGate::All,
        &mut service,
    );
    assert!(n.has_held_result());
    assert!(deferred.iter().any(|e| matches!(
        e,
        SchedulerEvent::CommandAdmissionRefused {
            retryable: true,
            ..
        }
    )));
    let mut other = restored(&n, &w, 0.1);
    let mut w2 = w.clone();
    let mut s2 = Service::default();
    w.command_ledger.unprotect(blocker);
    w2.command_ledger.unprotect(blocker);
    let out = poll(
        &mut n,
        &mut w,
        0.2,
        vec![],
        false,
        IdleGate::All,
        &mut service,
    );
    assert_eq!(
        out,
        poll(
            &mut other,
            &mut w2,
            0.2,
            vec![],
            false,
            IdleGate::All,
            &mut s2
        )
    );
    assert_eq!(w, w2);
    assert_eq!(bytes(&n, &w, 0.2), bytes(&other, &w2, 0.2));
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
            .count(),
        1
    );
    assert!(
        w.characters[&actor("mjr01")]
            .memories()
            .iter()
            .any(|m| m == "Capacity restored once")
    );
    assert!(s2.prompts.is_empty());
    assert_eq!(service.prompts.len(), 1);
    let before = w.clone();
    assert!(
        poll(
            &mut other,
            &mut w2,
            0.3,
            vec![],
            false,
            IdleGate::All,
            &mut s2
        )
        .is_empty()
    );
    assert_eq!(w2, before);
}
#[test]
#[ignore = "explicit new scheduler component fixture generation"]
fn regenerate_checkpoint_scheduler_fixtures() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_scheduler");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, w, n) in fixtures() {
        std::fs::write(dir.join(name), bytes(&n, &w, 0.0)).unwrap();
    }
}

#[test]
fn checkpoint_scheduler_raw_expansion_refuses_before_typed_parse() {
    let w = world();
    let raw = format!(
        "[{}null]",
        "null,".repeat(crate::checkpoint::aggregate::MAX_EXPANDED_BYTES / 64)
    );
    let error = decode(raw.as_bytes(), &w, 0.0).unwrap_err();
    assert!(error.to_string().contains("expanded allocation limit"));
}
