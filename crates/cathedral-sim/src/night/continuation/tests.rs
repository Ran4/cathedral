use super::*;
use crate::night::checkpoint::tests::{Recorded, all_tiers, clock, env, open, world_with_cast};

fn pending() -> (NightOffice, World, WorldClock, Recorded) {
    let mut world = world_with_cast();
    let clock = clock();
    let mut night = NightOffice::new(all_tiers(), 0.0, &clock);
    night.seed(&world, &Round::new());
    night.enqueue(Subject::Person(ActorId::from_raw("mjr01")), 0);
    let mut recorded = Recorded::new();
    night.poll(
        1.0,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut recorded,
        &env(),
    );
    assert!(night.in_flight.is_some());
    (night, world, clock, recorded)
}

#[test]
fn archive_night_refusal_preserves_queued_and_resumed_semantics() {
    use crate::prompt_archive::test_support::Probe;
    let mut world = world_with_cast();
    let clock = clock();
    let mut night = NightOffice::new(all_tiers(), 0.0, &clock);
    night.seed(&world, &Round::new());
    night.enqueue(Subject::Person(ActorId::from_raw("mjr01")), 0);
    let mut service = Probe::new(0);
    night.poll(
        1.0,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    assert!(service.calls.lock().unwrap().requests.is_empty());
    assert_eq!(night.queue.len(), 1);
    let semantic = night.queue[0].semantic.unwrap();
    assert_eq!(service.calls.lock().unwrap().attempts, 0);
    service.limit = 1;
    service.calls.lock().unwrap().refused = true;
    let now = night.next_attempt_at;
    night.poll(
        now,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    assert_eq!(
        service.retained(),
        0,
        "provider refusal releases the temporary archive permit"
    );
    assert_eq!(night.queue[0].semantic, Some(semantic));
    assert!(service.calls.lock().unwrap().requests.is_empty());
    assert_eq!(service.calls.lock().unwrap().attempts, 1);
    service.calls.lock().unwrap().refused = false;
    let now = night.next_attempt_at;
    night.poll(
        now,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    assert_eq!(night.in_flight.as_ref().unwrap().semantic, semantic);
    assert_eq!(service.retained(), 1);
    let original = night.in_flight.clone().unwrap();
    night.prepare_continuation(&mut world);
    // Reconstructed candidates have no execution archive permit. Here we
    // dispose that prior-generation entitlement explicitly to model the codec.
    night.archive = None;
    service.limit = 0;
    let now = night.next_attempt_at;
    night.poll(
        now,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    assert_eq!(night.in_flight.as_ref(), Some(&original));
    assert!(night.load_retry_pending);
    assert_eq!(service.calls.lock().unwrap().requests.len(), 1);
    assert_eq!(service.calls.lock().unwrap().attempts, 2);
    service.limit = 1;
    service.calls.lock().unwrap().refused = true;
    let now = night.next_attempt_at;
    night.poll(
        now,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    assert_eq!(service.retained(), 0);
    assert_eq!(night.in_flight.as_ref(), Some(&original));
    assert!(night.load_retry_pending);
    assert_eq!(service.calls.lock().unwrap().requests.len(), 1);
    assert_eq!(service.calls.lock().unwrap().attempts, 3);
    service.calls.lock().unwrap().refused = false;
    let now = night.next_attempt_at;
    night.poll(
        now,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    let calls = service.calls.lock().unwrap();
    assert_eq!(calls.requests.len(), 2);
    assert!(
        calls
            .requests
            .iter()
            .all(|(night, prompt)| *night && prompt == &original.prompt)
    );
    assert_eq!(night.in_flight.as_ref().unwrap().semantic, semantic);
}

#[test]
fn archive_night_held_result_owns_its_permit_until_the_actual_exchange_dies() {
    use crate::prompt_archive::test_support::Probe;
    let (mut night, mut world, clock, _) = pending();
    let mut service = Probe::new(1);
    let id = night.in_flight.as_ref().unwrap().request_id;
    night.held_result = Some(Completion {
        request_id: id,
        result: Err(crate::CognitionError::detailed("saved", "saved detail")),
        duration_seconds: 0.5,
    });
    let (prompt, labels) = night.held_archive_requirements(&world).unwrap();
    night.bind_held_archive(Some(
        service.reserve_prompt_archive(prompt, labels).unwrap(),
    ));
    let events = night.poll(
        2.0,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut service,
        &env(),
    );
    assert!(
        service.calls.lock().unwrap().requests.is_empty(),
        "held restoration cannot submit a provider request"
    );
    let exchange = events
        .iter()
        .find_map(|e| match e {
            SchedulerEvent::PromptExchange { exchange } => Some(exchange.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(exchange.error.as_deref(), Some("saved detail"));
    assert_eq!(service.retained(), 1);
    drop(events);
    assert_eq!(service.retained(), 1);
    drop(exchange);
    assert_eq!(service.retained(), 0);
}

#[test]
fn continuation_night_exact_busy_retry_yields_and_does_not_harvest_another_lanes_reused_id() {
    let (mut night, mut world, clock, mut recorded) = pending();
    let old = night.in_flight.clone().unwrap();
    night.prepare_continuation(&mut world);
    assert!(night.load_retry_pending);
    let now = night.next_attempt_at;
    assert!(night.queue.is_empty());
    let done = Completion {
        request_id: old.request_id,
        result: Ok("remember {\"memory\":\"wrong lane\"}".into()),
        duration_seconds: 0.1,
    };
    let mut completions = vec![done.clone()];
    assert!(
        night.could_submit(now, &completions),
        "empty ordinary queue still needs lazy gate"
    );
    night.poll(
        now,
        &mut world,
        &clock,
        &mut completions,
        NightGate {
            player_reaction: true,
            ..open()
        },
        &mut recorded,
        &env(),
    );
    assert_eq!(completions, vec![done]);
    assert_eq!(recorded.prompts.len(), 1);
    assert_eq!(night.in_flight.as_ref(), Some(&old));
    assert_eq!(night.totals(), (0, 0));
    recorded.busy = true;
    night.poll(
        now,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut recorded,
        &env(),
    );
    assert_eq!(night.in_flight.as_ref(), Some(&old));
    assert_eq!(night.next_attempt_at, now + RETRY_SECONDS);
    assert!(world.command_ledger.is_protected(old.semantic));
    recorded.busy = false;
    night.poll(
        now + RETRY_SECONDS,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut recorded,
        &env(),
    );
    assert!(!night.load_retry_pending);
    assert_eq!(recorded.prompts, vec![old.prompt.clone(), old.prompt]);
    assert_eq!(night.in_flight.as_ref().unwrap().semantic, old.semantic);
    assert_ne!(night.in_flight.as_ref().unwrap().request_id, old.request_id);
    assert_eq!(night.last_reflected[&old.subject], old.owed_day);
}

#[test]
fn continuation_night_expired_day_and_reentered_person_spend_saved_duty_without_submission() {
    for expired in [false, true] {
        let (mut night, mut world, clock, mut recorded) = pending();
        let old = night.in_flight.clone().unwrap();
        night.prepare_continuation(&mut world);
        if !expired {
            world
                .characters
                .get_mut(&ActorId::from_raw("mjr01"))
                .unwrap()
                .state
                .presence_epoch += 2;
        }
        let now = if expired {
            3601.0
        } else {
            night.next_attempt_at
        };
        night.poll(
            now,
            &mut world,
            &clock,
            &mut vec![],
            open(),
            &mut recorded,
            &env(),
        );
        assert_eq!(recorded.prompts.len(), 1);
        assert!(night.in_flight.is_none());
        assert!(!night.load_retry_pending);
        assert_eq!(night.totals(), (0, 1));
        assert!(!world.command_ledger.is_protected(old.semantic));
        night.enqueue(old.subject.clone(), old.owed_day);
        assert!(
            night.queue.is_empty(),
            "queue-time stamp keeps dropped duty spent"
        );
        night.poll(
            now + 1.0,
            &mut world,
            &clock,
            &mut vec![],
            open(),
            &mut recorded,
            &env(),
        );
        assert_eq!(night.totals(), (0, 1));
    }
}

#[test]
fn continuation_night_held_success_error_and_oversized_result_apply_once_without_request() {
    for result in [
        Ok("remember {\"memory\":\"held Night\"}".into()),
        Err(crate::CognitionError::detailed(
            "SavedError",
            "saved exact detail",
        )),
        Ok("x".repeat(crate::MAX_LLM_REPLY_CHARS + 1)),
    ] {
        let (mut night, mut world, clock, mut recorded) = pending();
        let id = night.in_flight.as_ref().unwrap().request_id;
        night.held_result = Some(Completion {
            request_id: id,
            result: result.clone(),
            duration_seconds: 0.25,
        });
        night.prepare_continuation(&mut world);
        assert!(!night.load_retry_pending);
        let events = night.poll(
            2.0,
            &mut world,
            &clock,
            &mut vec![],
            open(),
            &mut recorded,
            &env(),
        );
        world.command_ledger.drain_updates();
        assert_eq!(recorded.prompts.len(), 1);
        assert!(night.in_flight.is_none());
        assert!(night.held_result.is_none());
        if result.as_ref().is_ok_and(|s| s.starts_with("remember")) {
            assert_eq!(night.totals(), (1, 0));
            assert!(
                world.characters[&ActorId::from_raw("mjr01")]
                    .memories()
                    .iter()
                    .any(|s| s == "held Night")
            );
        } else {
            assert_eq!(night.totals(), (0, 1));
        }
        assert!(!events.is_empty());
        assert!(
            night
                .poll(
                    3.0,
                    &mut world,
                    &clock,
                    &mut vec![],
                    open(),
                    &mut recorded,
                    &env()
                )
                .is_empty()
        );
        assert_eq!(recorded.prompts.len(), 1);
    }
}

#[test]
fn continuation_night_queue_time_incarnation_survives_busy_and_legacy_migration_is_explicit() {
    let mut world = world_with_cast();
    let clock = clock();
    let actor = ActorId::from_raw("mjr01");
    let mut night = NightOffice::new(
        NightOfficeConfig {
            enabled: true,
            majors: true,
            wards: false,
            ambients: false,
        },
        0.0,
        &clock,
    );
    night.seed(&world, &Round::new());
    night.bedtimes.insert(actor.clone(), Office::Lamplight);
    night.ring(601.0, &mut world, &mut Round::new(), &clock, &mut vec![]);
    assert_eq!(night.queue.len(), 1);
    assert_eq!(night.queue[0].queued_presence_epoch, Some(0));
    assert!(night.require_legacy().is_err());
    let mut recorded = Recorded::new();
    recorded.busy = true;
    night.poll(
        601.0,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut recorded,
        &env(),
    );
    let root = night.queue[0].semantic.unwrap();
    assert_eq!(night.queue[0].presence_epoch, Some(0));
    world
        .characters
        .get_mut(&actor)
        .unwrap()
        .state
        .presence_epoch = 2;
    night.poll(
        606.0,
        &mut world,
        &clock,
        &mut vec![],
        open(),
        &mut recorded,
        &env(),
    );
    assert!(night.queue.is_empty());
    assert_eq!(night.totals(), (0, 1));
    assert!(!world.command_ledger.is_protected(root));
    assert!(recorded.prompts.is_empty());

    let mut legacy = NightOffice::new(all_tiers(), 0.0, &clock);
    legacy.enqueue(Subject::Person(actor), 0);
    assert!(legacy.require_legacy().is_ok());
    legacy.prepare_continuation(&mut world);
    assert_eq!(
        legacy.queue[0].queued_presence_epoch,
        Some(2),
        "V1 migration can only anchor saved current lifetime"
    );
    assert!(legacy.require_legacy().is_err());
}

#[test]
fn continuation_night_missing_person_migration_preserves_the_post_preparation_counter_bound() {
    let mut world = world_with_cast();
    let mut night = NightOffice::new(all_tiers(), 0.0, &clock());
    night.enqueue(
        Subject::Person(ActorId::from_raw("historical-departed-person")),
        0,
    );
    let limit = u64::MAX - checkpoint::COUNTER_DROP_HEADROOM;
    night.dropped = limit;
    assert!(night.continuation_preflight(&world).is_err());
    assert_eq!(night.queue.len(), 1);
    assert_eq!(night.dropped, limit);
    night.dropped = limit - 1;
    night.continuation_preflight(&world).unwrap();
    night.prepare_continuation(&mut world);
    assert_eq!(night.dropped, limit);
    assert!(night.queue.is_empty());
    night.continuation_preflight(&world).unwrap();
}
