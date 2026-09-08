use super::*;
use crate::operations::{self, AdapterDeclaration, FixtureDeclaration, OperationConfig, Request};
use crate::receipts::{Admission, CommandId, OperationId, Outcome, ReceiptState};
fn command_id(n: u64) -> CommandId {
    OperationId {
        producer: 0,
        sequence: n,
    }
    .command(0)
}
fn fixture(world: &mut World, actor: &ActorId) {
    let position = world.characters[actor].position_m().to_array();
    world.operations = operations::OperationKernel::from_config(&OperationConfig {
        fixtures: vec![FixtureDeclaration {
            id: "work".into(),
            adapter: AdapterDeclaration::default(),
            position,
        }],
    })
    .unwrap();
}
fn start(
    round: &mut Round,
    world: &mut World,
    clock: &WorldClock,
    actor: &ActorId,
    work: f64,
) -> Outcome {
    let req = Request::Start {
        actor: actor.clone(),
        resource: "work".into(),
        adapter: Default::default(),
        work_seconds: work,
        recovery_seconds: 30.0,
        retries: 2,
    };
    let Admission::New(ticket) = world
        .command_ledger
        .begin(command_id(1), &serde_json::json!({"request":req}))
    else {
        panic!("new start");
    };
    let outcome = operations::command(
        world,
        round,
        clock,
        crate::timeline::LogicalTime::new(0.0).unwrap(),
        command_id(1),
        req,
    );
    world
        .command_ledger
        .finish(ticket, 0.0, outcome.clone(), Vec::new());
    outcome
}
#[test]
fn operation_holds_real_round_motion_and_conversation_until_completion_then_resumes() {
    let (mut round, mut world, nav, id) = seed_hamel();
    let clock = clock_at(Office::Kindling);
    world.characters.get_mut(&id).unwrap().state.position_m =
        nav.node_point(nav.place("Seraph statue").unwrap().node);
    round.people.get_mut(&id).unwrap().next_decision = 0.0;
    fixture(&mut world, &id);
    let mut baseline_world = world.clone();
    let mut baseline_round = round.clone();
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 0.5).state,
        ReceiptState::Accepted
    );
    let origin = world.characters[&id].position_m();
    for step in 1..10 {
        let now = step as f64 * 0.05;
        tick(
            &mut round,
            &mut world,
            &nav,
            &clock,
            now,
            &player(),
            &BTreeSet::new(),
        );
        interrupt_for_conversation(&mut round, &mut world, &id);
        world.step_movement(0.05, &nav, None);
        operations::poll(
            &mut world,
            &round,
            &clock,
            crate::timeline::LogicalTime::new(now).unwrap(),
        );
        tick(
            &mut baseline_round,
            &mut baseline_world,
            &nav,
            &clock,
            now,
            &player(),
            &BTreeSet::new(),
        );
        baseline_world.step_movement(0.05, &nav, None);
        assert_eq!(world.characters[&id].position_m(), origin);
        assert!(world.characters[&id].state.movement.is_none());
    }
    assert!(
        baseline_world.characters[&id].position_m().distance(origin) > 0.0,
        "the unclaimed ordinary route really moves"
    );
    operations::poll(
        &mut world,
        &round,
        &clock,
        crate::timeline::LogicalTime::new(0.5).unwrap(),
    );
    assert_eq!(
        world
            .command_ledger
            .get(command_id(1))
            .unwrap()
            .outcome
            .state,
        ReceiptState::Completed
    );
    for step in 11..=30 {
        tick(
            &mut round,
            &mut world,
            &nav,
            &clock,
            step as f64 * 0.05,
            &player(),
            &BTreeSet::new(),
        );
        world.step_movement(0.05, &nav, None);
    }
    assert!(
        world.characters[&id].position_m().distance(origin) > 0.0,
        "resume: {:?} movement {:?}",
        round.people[&id],
        world.characters[&id].state.movement
    );
}
#[test]
fn atomic_draw_refuses_before_receipt_binding_claim_or_queue_mutation() {
    let (mut round, mut world, _nav, id) = seed_hamel();
    let clock = clock_at(Office::Dayspring);
    fixture(&mut world, &id);
    round.people.get_mut(&id).unwrap().phase = Phase::Drawing;
    let before = round.clone();
    let actor = world.characters[&id].clone();
    let kernel = world.operations.clone();
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 1.0).code,
        "atomic_service"
    );
    assert_eq!(round, before);
    assert_eq!(world.characters[&id], actor);
    assert_eq!(world.operations, kernel);
    assert!(!world.command_ledger.is_protected(command_id(1).operation));
}
#[test]
fn work_supersedes_the_actual_accepted_travel_receipt() {
    let (mut round, mut world, nav, id) = seed_hamel();
    let clock = clock_at(Office::Dayspring);
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    world.characters.get_mut(&id).unwrap().state.position_m =
        nav.node_point(nav.place("Seraph statue").unwrap().node);
    fixture(&mut world, &id);
    let place = world.characters[&id]
        .state
        .places_known
        .iter()
        .next()
        .unwrap()
        .clone();
    let travel = command_id(2);
    let args = serde_json::json!({"place_id":place});
    let Admission::New(ticket) = world.command_ledger.begin(travel, &args) else {
        panic!("new travel");
    };
    world.command_ledger.protect(travel.operation).unwrap();
    let (result, receipt) =
        crate::receipts::commit_actor_action(&mut world, 0.0, ticket, &id, "go_to", &args);
    result.unwrap();
    assert_eq!(receipt.outcome.state, ReceiptState::Accepted);
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 1.0).state,
        ReceiptState::Accepted
    );
    assert_eq!(
        world.command_ledger.get(travel).unwrap().outcome.state,
        ReceiptState::Superseded
    );
    assert!(!world.travel_actions.contains_key(&id));
    assert!(world.characters[&id].state.intent.is_none());
    assert!(!world.command_ledger.is_protected(travel.operation));
    assert!(crate::apply_action(&mut world, &id, "go_to", &args).is_err());
    assert!(crate::apply_action(&mut world, &id, "stop", &serde_json::json!({})).is_err());
}
#[test]
fn urgent_curfew_and_critical_need_interrupt_before_fixture_completion() {
    for curfew in [false, true] {
        let (mut round, mut world, nav, id) = seed_hamel();
        let clock = clock_at(Office::Dayspring);
        fixture(&mut world, &id);
        assert_eq!(
            start(&mut round, &mut world, &clock, &id, 0.1).state,
            ReceiptState::Accepted
        );
        let clock = if curfew {
            clock_at(Office::Snuffing)
        } else {
            clock
        };
        if !curfew {
            world.characters.get_mut(&id).unwrap().state.needs.hunger = 0.0;
        }
        tick(
            &mut round,
            &mut world,
            &nav,
            &clock,
            0.1,
            &player(),
            &BTreeSet::new(),
        );
        operations::poll(
            &mut world,
            &round,
            &clock,
            crate::timeline::LogicalTime::new(0.1).unwrap(),
        );
        assert_eq!(world.operations.active_count(), 0);
        assert_eq!(world.operations.fixture("work").unwrap().completed_units, 0);
        let receipt = world.command_ledger.get(command_id(1)).unwrap();
        assert_eq!(receipt.outcome.state, ReceiptState::Interrupted);
        assert_eq!(
            receipt.outcome.code,
            if curfew {
                "operation_curfew"
            } else {
                "operation_critical_need"
            }
        );
    }
}
#[test]
fn refreshing_needs_for_work_does_not_decay_other_actors_twice_at_round_tick() {
    let (mut round, mut world, nav, id) = seed_hamel();
    let clock = clock_at(Office::Dayspring);
    let mut other = world.characters[&id].clone();
    other.sheet.id = ActorId::from_raw("unclaimed");
    let other_id = other.id().clone();
    world.add_character(other);
    round
        .people
        .insert(other_id.clone(), round.people[&id].clone());
    fixture(&mut world, &id);
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 1.0).state,
        ReceiptState::Accepted
    );
    let before = world.characters[&other_id].needs().hunger;
    round.refresh_operation_needs(&mut world, &nav, &clock, 0.025);
    round.refresh_operation_needs(&mut world, &nav, &clock, 0.05);
    let once = world.characters[&other_id].needs().hunger;
    assert!(once < before);
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        0.05,
        &player(),
        &BTreeSet::new(),
    );
    assert_eq!(world.characters[&other_id].needs().hunger, once);
    let expected = before
        - (clock.game_days(0.05) - clock.game_days(0.0)) * 86400.0 * HUNGER_DECAY_PER_GAME_SECOND;
    assert!((once - expected).abs() < 1e-12);
}

#[test]
fn operation_yields_to_actual_road_return_and_lightning_writers() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let id = ActorId::from_raw("rbrde");
    let party = PartyId::from_raw("brede_wool_gate");
    assert!(world.is_present(&id));
    fixture(&mut world, &id);
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 1.0).state,
        ReceiptState::Accepted
    );
    let at = world.characters[&id].position_m();
    round.tick_road_parties(&mut world, &nav, clock.at(0.05), 0.05, &BTreeSet::new());
    world.step_movement(0.05, &nav, None);
    assert_eq!(world.characters[&id].position_m(), at);
    assert!(world.characters[&id].state.movement.is_none());
    round.begin_road_return(&mut world, &party, 2, 0.1, &mut Vec::new());
    assert_eq!(
        world
            .command_ledger
            .get(command_id(1))
            .unwrap()
            .outcome
            .code,
        "operation_road_return"
    );
    assert_eq!(world.operations.active_count(), 0);
    assert!(world.characters[&id].state.leaving_city);
    assert_eq!(
        round.road_parties[&party].state.phase,
        PartyPhase::Returning
    );

    let (mut round, mut world, _nav, id) = seed_hamel();
    let clock = clock_at(Office::Dayspring);
    fixture(&mut world, &id);
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 0.1).state,
        ReceiptState::Accepted
    );
    round.note_lightning(
        &world,
        &crate::weather::LightningStrike {
            id: 1,
            game_instant_days: clock.game_days(0.1),
            origin_m: world.characters[&id].position_m().to_array(),
            strength: 1.0,
        },
        0.1,
    );
    operations::poll(
        &mut world,
        &round,
        &clock,
        crate::timeline::LogicalTime::new(0.1).unwrap(),
    );
    assert_eq!(
        world
            .command_ledger
            .get(command_id(1))
            .unwrap()
            .outcome
            .code,
        "operation_danger"
    );
    assert_eq!(world.operations.fixture("work").unwrap().completed_units, 0);
}
#[test]
fn operation_pauses_existing_production_without_consuming_reservations_and_resumes_after_release() {
    let (mut world, mut round, id, clock) = active_production_fixture(240);
    fixture(&mut world, &id);
    let original = world.active_transform_job(&id).unwrap().clone();
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 1.0).state,
        ReceiptState::Accepted
    );
    for step in 1..=10 {
        let now = step as f64 * 0.05;
        round.tick_production(&mut world, &clock, now, &BTreeSet::new());
        operations::poll(
            &mut world,
            &round,
            &clock,
            crate::timeline::LogicalTime::new(now).unwrap(),
        );
    }
    assert_eq!(world.active_transform_job(&id), Some(&original));
    assert!(
        original
            .inputs
            .iter()
            .all(|input| world.items.contains_key(&input.item_id))
    );
    let outcome = operations::command(
        &mut world,
        &mut round,
        &clock,
        crate::timeline::LogicalTime::new(0.5).unwrap(),
        command_id(2),
        Request::Cancel {
            instance: operations::InstanceId(command_id(1)),
        },
    );
    assert!(outcome.succeeded());
    round.tick_production(&mut world, &clock, 0.55, &BTreeSet::new());
    assert_eq!(
        world
            .active_transform_job(&id)
            .unwrap()
            .progress_work_minutes,
        0.0
    );
    round.tick_production(&mut world, &clock, 0.6, &BTreeSet::new());
    let resumed = world.active_transform_job(&id).unwrap();
    assert!(resumed.progress_work_minutes > 0.0);
    assert_eq!(resumed.inputs, original.inputs);
    assert_eq!(world.operations.fixture("work").unwrap().completed_units, 0);
}

#[test]
fn operation_cannot_finish_between_round_ticks_before_owed_critical_decay() {
    let (mut round, mut world, nav, id) = seed_hamel();
    let clock = clock_at(Office::Dayspring);
    fixture(&mut world, &id);
    let decay =
        (clock.game_days(0.025) - clock.game_days(0.0)) * 86400.0 * HUNGER_DECAY_PER_GAME_SECOND;
    world.characters.get_mut(&id).unwrap().state.needs.hunger = HUNGER_FAMISHED + decay / 2.0;
    assert_eq!(
        start(&mut round, &mut world, &clock, &id, 0.02).state,
        ReceiptState::Accepted
    );
    // Ordinary Round's next 20Hz evaluation is at .05. Engine's active-duty
    // preparation must account for this earlier accepted .025 boundary.
    round.refresh_operation_needs(&mut world, &nav, &clock, 0.025);
    operations::poll(
        &mut world,
        &round,
        &clock,
        crate::timeline::LogicalTime::new(0.025).unwrap(),
    );
    assert_eq!(
        world
            .command_ledger
            .get(command_id(1))
            .unwrap()
            .outcome
            .code,
        "operation_critical_need"
    );
    assert_eq!(world.operations.fixture("work").unwrap().completed_units, 0);
}
