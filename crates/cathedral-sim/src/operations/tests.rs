use super::*;
use crate::receipts::{Admission, PROTECTED_CAPACITY};
use crate::{Character, WorldSeed};
use serde_json::json;

fn actor_key(n: usize) -> String {
    format!("actor_{n:058}")
}
fn resource_key(n: usize) -> String {
    format!("fixture_{n:056}")
}

fn setup(count: usize) -> (World, Round, WorldClock, OperationConfig) {
    let mut world = World::new();
    let template = WorldSeed::from_json_str(include_str!("../../tests/fixtures/demo_seed.json"))
        .unwrap()
        .characters[0]
        .clone();
    let mut config = OperationConfig::default();
    for n in 0..count {
        let mut sheet = template.clone();
        sheet.id = ActorId::from_raw(actor_key(n));
        sheet.position_m = Vec3::ZERO;
        world.add_character(Character::from_sheet(sheet));
        if n < MAX_FIXTURES {
            config.fixtures.push(FixtureDeclaration {
                id: resource_key(n),
                adapter: Default::default(),
                position: [0.0; 3],
            });
        }
    }
    world.operations = OperationKernel::from_config(&config).unwrap();
    (
        world,
        Round::new(),
        WorldClock::new(3600.0, crate::Office::Dayspring, 0, 0.05),
        config,
    )
}
fn request(n: usize) -> Request {
    Request::Start {
        actor: ActorId::from_raw(actor_key(n)),
        resource: resource_key(n.min(MAX_FIXTURES - 1)),
        adapter: Default::default(),
        work_seconds: 1.0,
        recovery_seconds: 5.0,
        retries: 2,
    }
}
fn admit(
    world: &mut World,
    round: &mut Round,
    clock: &WorldClock,
    id: CommandId,
    request: Request,
) -> Outcome {
    let Admission::New(ticket) = world.command_ledger.begin(id, &json!({"request":request})) else {
        panic!("new ticket");
    };
    let outcome = command(
        world,
        round,
        clock,
        crate::timeline::LogicalTime::new(0.0).unwrap(),
        id,
        request,
    );
    world
        .command_ledger
        .finish(ticket, 0.0, outcome.clone(), Vec::new());
    outcome
}
fn id(n: u64) -> CommandId {
    OperationId {
        producer: 0,
        sequence: n,
    }
    .command(0)
}

#[test]
fn full_instance_window_is_bounded_even_with_one_protected_root() {
    let (mut world, mut round, clock, _) = setup(MAX_INSTANCES + 1);
    let root = OperationId {
        producer: 0,
        sequence: 1,
    };
    for n in 0..MAX_INSTANCES {
        assert_eq!(
            admit(
                &mut world,
                &mut round,
                &clock,
                root.command(n as u16),
                request(n)
            )
            .state,
            ReceiptState::Accepted
        );
    }
    assert_eq!(world.operations.active_count(), MAX_INSTANCES);
    let before = world.operations.clone();
    assert_eq!(
        admit(
            &mut world,
            &mut round,
            &clock,
            id(2),
            request(MAX_INSTANCES)
        )
        .code,
        "operation_capacity"
    );
    assert_eq!(world.operations, before);
    let encoded = serde_json::to_vec(&world.operations.active.values().collect::<Vec<_>>())
        .unwrap()
        .len();
    let allocated = world.operations.allocated_upper_bound();
    eprintln!(
        "operation_window instances={} roots=1 active_encoded_bytes={encoded} conservative_heap_bytes={allocated}",
        MAX_INSTANCES
    );
    assert!(encoded < 512 * 1024);
    assert!(allocated < MAX_KERNEL_ALLOCATED_BYTES);
    for n in 0..MAX_INSTANCES {
        finish(
            &mut world,
            InstanceId(root.command(n as u16)),
            crate::timeline::LogicalTime::new(0.1).unwrap(),
            ReceiptState::Interrupted,
            "test_cancel",
        );
        assert_eq!(
            world.command_ledger.is_protected(root),
            n + 1 < MAX_INSTANCES
        );
    }
    assert_eq!(world.operations.active_count(), 0);
    assert!(
        world.operations.actor_claims.is_empty() && world.operations.resource_claims.is_empty()
    );
}

#[test]
fn shared_root_capacity_refuses_before_any_domain_claim_or_route_mutation() {
    let (mut world, mut round, clock, _) = setup(1);
    for _ in 0..PROTECTED_CAPACITY {
        world.command_ledger.reserve_operation(1).unwrap();
    }
    let before = world.characters.clone();
    let kernel = world.operations.clone();
    let outcome = admit(&mut world, &mut round, &clock, id(1), request(0));
    assert_eq!(outcome.code, "active_capacity");
    assert_eq!(world.characters, before);
    assert_eq!(world.operations, kernel);
}

#[test]
fn retained_identifiers_discard_arbitrary_caller_spare_capacity() {
    let (mut world, mut round, clock, _) = setup(1);
    let mut actor = String::with_capacity(2 * 1024 * 1024);
    actor.push_str(&actor_key(0));
    let mut resource = String::with_capacity(2 * 1024 * 1024);
    resource.push_str(&resource_key(0));
    let mut req = request(0);
    if let Request::Start {
        actor: a,
        resource: r,
        ..
    } = &mut req
    {
        *a = ActorId::from_raw(actor);
        *r = resource;
    }
    assert_eq!(
        admit(&mut world, &mut round, &clock, id(1), req).state,
        ReceiptState::Accepted
    );
    let op = world.operations.active(InstanceId(id(1))).unwrap();
    assert!(op.actor.allocated_bytes() <= 64 && op.resource.capacity() <= 64);
    assert!(
        world
            .operations
            .actor_claims
            .keys()
            .all(|s| s.allocated_bytes() <= 64)
    );
    assert!(
        world
            .operations
            .resource_claims
            .keys()
            .all(|s| s.capacity() <= 64)
    );
}

#[test]
fn corrupt_continuations_are_rejected_without_using_unknown_adapters_or_claims() {
    let (mut world, mut round, clock, config) = setup(1);
    admit(&mut world, &mut round, &clock, id(1), request(0));
    assert!(
        world
            .operations
            .validate_continuation(
                &config,
                &world,
                crate::timeline::LogicalTime::new(0.0).unwrap()
            )
            .is_ok()
    );
    let original = world.operations.clone();
    for case in 0..10 {
        let mut candidate = original.clone();
        let op = candidate.active.get_mut(&InstanceId(id(1))).unwrap();
        match case {
            0 => op.adapter.name = "missing".into(),
            1 => op.adapter.version = 2,
            2 => op.retries_spent = 3,
            3 => op.recovery_deadline = ExclusiveDeadline(LogicalTime::new(0.0).unwrap()),
            4 => op.resource = "missing".into(),
            5 => op.presence_epoch += 1,
            6 => op.step.index = 1,
            7 => {
                candidate.actor_claims.clear();
            }
            8 => {
                candidate.resource_claims.clear();
            }
            _ => {
                let mut huge = String::with_capacity(MAX_KERNEL_ALLOCATED_BYTES + 1);
                huge.push_str(op.actor.as_str());
                op.actor = ActorId::from_raw(huge);
            }
        }
        assert!(
            candidate
                .validate_continuation(
                    &config,
                    &world,
                    crate::timeline::LogicalTime::new(0.0).unwrap()
                )
                .is_err(),
            "case {case}"
        );
        assert_eq!(world.operations, original);
    }
}
