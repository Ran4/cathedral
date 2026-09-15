use super::*;
use crate::night::checkpoint::tests::{env, world_with_cast};
struct Recorded {
    attempts: usize,
    busy: bool,
}
impl Cognition for Recorded {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, crate::CognitionBusy> {
        self.attempts += 1;
        if self.busy {
            Err(crate::CognitionBusy)
        } else {
            Ok(RequestId(self.attempts as u64))
        }
    }
}
fn prepared() -> (NpcScheduler, World, Recorded) {
    let mut world = world_with_cast();
    let mut n = NpcScheduler::new(vec![ActorId::from_raw("mjr01")], 0.0, 60.0, 0.0);
    n.start(0.0);
    let mut service = Recorded {
        attempts: 0,
        busy: false,
    };
    n.poll(
        0.0,
        &mut world,
        &mut vec![],
        &mut vec![],
        false,
        IdleGate::All,
        &mut service,
        &env(),
    );
    assert!(n.in_flight.is_some());
    n.prepare_continuation(&mut world);
    (n, world, service)
}

#[test]
fn archive_scheduler_backpressure_retains_held_and_same_poll_exchange_until_last_clone() {
    use crate::prompt_archive::test_support::Probe;
    let mut world = world_with_cast();
    let actor = ActorId::from_raw("mjr01");
    let mut scheduler = NpcScheduler::new(vec![actor.clone()], 0.0, 60.0, 0.0);
    scheduler.start(0.0);
    let mut service = Probe::new(1);
    scheduler.poll(
        0.0,
        &mut world,
        &mut vec![],
        &mut vec![],
        false,
        IdleGate::All,
        &mut service,
        &env(),
    );
    assert_eq!(service.calls.lock().unwrap().requests.len(), 1);
    let id = scheduler.in_flight.as_ref().unwrap().request_id;
    let semantic = scheduler.in_flight.as_ref().unwrap().semantic;
    let done = Completion {
        request_id: id,
        result: Ok("wait {}".into()),
        duration_seconds: 0.25,
    };
    let held = scheduler.poll(
        0.1,
        &mut world,
        &mut vec![],
        &mut vec![done],
        true,
        IdleGate::All,
        &mut service,
        &env(),
    );
    assert!(
        !held
            .iter()
            .any(|e| matches!(e, SchedulerEvent::PromptExchange { .. }))
    );
    assert_eq!(service.retained(), 1);
    assert!(scheduler.held_result.is_some());
    assert!(world.command_ledger.is_protected(semantic));
    let events = scheduler.poll(
        0.2,
        &mut world,
        &mut vec![],
        &mut vec![],
        false,
        IdleGate::All,
        &mut service,
        &env(),
    );
    let exchange = events
        .iter()
        .find_map(|e| match e {
            SchedulerEvent::PromptExchange { exchange } => Some(exchange.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(exchange.answer.as_deref(), Some("wait {}"));
    assert_eq!(
        service.calls.lock().unwrap().requests.len(),
        1,
        "same-poll next request must wait for the archive"
    );
    assert!(scheduler.in_flight.is_none());
    assert_eq!(service.retained(), 1);
    drop(events);
    assert_eq!(
        service.retained(),
        1,
        "the last event clone owns the actual strings"
    );
    drop(exchange);
    assert_eq!(service.retained(), 0);
    scheduler.poll(
        2.0,
        &mut world,
        &mut vec![],
        &mut vec![],
        false,
        IdleGate::All,
        &mut service,
        &env(),
    );
    assert_eq!(service.calls.lock().unwrap().requests.len(), 2);
    assert_eq!(service.retained(), 1);
    drop(scheduler);
    assert_eq!(service.retained(), 0);
}

#[test]
fn archive_scheduler_refused_resumed_request_preserves_exact_obligation_and_inputs() {
    use crate::prompt_archive::test_support::Probe;
    let (mut scheduler, mut world, _) = prepared();
    let original = scheduler.load_retries[0].clone();
    let mut service = Probe::new(0);
    assert!(scheduler.submit_load_retry(1.0, &mut world, IdleGate::All, &mut service, &mut vec![]));
    assert!(service.calls.lock().unwrap().requests.is_empty());
    assert_eq!(scheduler.load_retries[0].flight, original.flight);
    assert!(world.command_ledger.is_protected(original.flight.semantic));
    assert_eq!(service.calls.lock().unwrap().attempts, 0);
    service.limit = 1;
    service.calls.lock().unwrap().refused = true;
    scheduler.submit_load_retry(1.5, &mut world, IdleGate::All, &mut service, &mut vec![]);
    assert_eq!(
        service.retained(),
        0,
        "provider refusal releases the temporary archive permit"
    );
    assert!(service.calls.lock().unwrap().requests.is_empty());
    assert_eq!(scheduler.load_retries[0].flight, original.flight);
    assert!(world.command_ledger.is_protected(original.flight.semantic));
    assert_eq!(service.calls.lock().unwrap().attempts, 1);
    service.calls.lock().unwrap().refused = false;
    scheduler.submit_load_retry(2.0, &mut world, IdleGate::All, &mut service, &mut vec![]);
    assert_eq!(
        service.calls.lock().unwrap().requests[0].1,
        original.flight.prompt
    );
    assert_eq!(
        scheduler.in_flight.as_ref().unwrap().semantic,
        original.flight.semantic
    );
    assert_eq!(service.retained(), 1);
}

#[test]
fn archive_scheduler_stale_actor_and_failed_results_keep_their_admitted_archive() {
    use crate::prompt_archive::test_support::Probe;
    for fail in [false, true] {
        let mut world = world_with_cast();
        let actor = ActorId::from_raw("mjr01");
        let mut scheduler = NpcScheduler::new(vec![actor.clone()], 0.0, 60.0, 0.0);
        scheduler.start(0.0);
        let mut service = Probe::new(1);
        scheduler.poll(
            0.0,
            &mut world,
            &mut vec![],
            &mut vec![],
            false,
            IdleGate::All,
            &mut service,
            &env(),
        );
        let id = scheduler.in_flight.as_ref().unwrap().request_id;
        if !fail {
            world
                .characters
                .get_mut(&actor)
                .unwrap()
                .state
                .presence_epoch += 1;
        }
        let result = if fail {
            Err(crate::CognitionError::detailed(
                "offline",
                "exact archive detail",
            ))
        } else {
            Ok("wait {}".into())
        };
        let events = scheduler.poll(
            0.1,
            &mut world,
            &mut vec![],
            &mut vec![Completion {
                request_id: id,
                result,
                duration_seconds: 0.5,
            }],
            false,
            IdleGate::Suppressed,
            &mut service,
            &env(),
        );
        let exchange = events
            .iter()
            .find_map(|e| match e {
                SchedulerEvent::PromptExchange { exchange } => Some(exchange),
                _ => None,
            })
            .unwrap();
        assert!(exchange.admission().enabled());
        assert_eq!(
            exchange.error.as_deref(),
            fail.then_some("exact archive detail")
        );
        assert_eq!(service.retained(), 1);
        drop(events);
        assert_eq!(service.retained(), 0);
    }
}
#[test]
fn continuation_retry_capacity_preserves_every_root_and_backpressures_new_protected_work() {
    let (mut n, mut world, mut service) = prepared();
    let original = n.load_retries[0].clone();
    for _ in 1..MAX_LOAD_RETRIES {
        let mut copy = original.clone();
        copy.flight.semantic = world
            .command_ledger
            .reserve_operation(receipts::TURN_PRODUCER)
            .unwrap();
        n.load_retries.push_back(copy);
    }
    let roots: Vec<_> = n.load_retries.iter().map(|r| r.flight.semantic).collect();
    n.player_reactions.push_back(ActorId::from_raw("mjr01"));
    let before = service.attempts;
    assert!(n.submit_load_retry(
        1.0,
        &mut world,
        IdleGate::Suppressed,
        &mut service,
        &mut vec![]
    ));
    assert_eq!(service.attempts, before);
    assert!(n.in_flight.is_none());
    assert_eq!(n.load_retries.len(), 64);
    assert_eq!(n.player_reactions.len(), 1);
    assert!(n.continuation_preflight().is_ok());
    assert!(n.submit_load_retry(2.0, &mut world, IdleGate::All, &mut service, &mut vec![]));
    assert_eq!(n.load_retries.len(), 63);
    assert_eq!(n.in_flight.as_ref().unwrap().semantic, roots[0]);
    assert_eq!(n.player_reactions.len(), 1);
    assert!(n.continuation_preflight().is_ok());
    n.prepare_continuation(&mut world);
    assert_eq!(n.load_retries.len(), 64);
    assert_eq!(
        n.load_retries
            .iter()
            .map(|r| r.flight.semantic)
            .collect::<Vec<_>>(),
        roots
    );
    assert!(
        roots
            .iter()
            .all(|root| world.command_ledger.is_protected(*root))
    );
    assert_eq!(n.provider_failures, 0);
    // A malformed/older65-obligation candidate is refused before preparation.
    let mut extra = original;
    extra.flight.semantic = world
        .command_ledger
        .reserve_operation(receipts::TURN_PRODUCER)
        .unwrap();
    n.in_flight = Some(extra.flight);
    assert!(n.continuation_preflight().is_err());
    assert_eq!(n.load_retries.len(), 64);
}

#[test]
fn continuation_real_retry_failure_keeps_exact_input_and_newer_arrivals_separate() {
    let (mut n, mut world, mut service) = prepared();
    let original = n.load_retries[0].clone();
    let actor = original.flight.actor_id.clone();
    n.submit_load_retry(1.0, &mut world, IdleGate::All, &mut service, &mut vec![]);
    world
        .characters
        .get_mut(&actor)
        .unwrap()
        .state
        .inbox
        .push("newer arrival".into());
    let request_id = n.in_flight.as_ref().unwrap().request_id;
    n.poll(
        2.0,
        &mut world,
        &mut vec![],
        &mut vec![Completion {
            request_id,
            result: Err(CognitionError::detailed(
                "RealFailure",
                "real provider detail",
            )),
            duration_seconds: 0.3,
        }],
        false,
        IdleGate::All,
        &mut service,
        &env(),
    );
    assert_eq!(n.provider_failures, 1);
    assert!(n.in_flight.is_none());
    assert_eq!(n.load_retries.len(), 1);
    let saved = &n.load_retries[0].flight;
    assert_eq!(saved.prompt, original.flight.prompt);
    assert_eq!(saved.drained_events, original.flight.drained_events);
    assert_eq!(saved.presented, original.flight.presented);
    assert_eq!(saved.semantic, original.flight.semantic);
    assert!(n.retry_work.is_empty());
    assert_eq!(
        world.characters[&actor].state.inbox,
        vec![
            "newer arrival".to_string(),
            SYSTEM_PROVIDER_FAILED.to_string()
        ]
    );
    assert!(world.command_ledger.is_protected(saved.semantic));
}
