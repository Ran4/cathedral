use super::*;
use crate::custody::Station;
use crate::{Character, Vec3, WALK_Y, WorldSeed, apply_action};
use serde_json::json;

fn id(value: &str) -> ActorId {
    ActorId::from_raw(value)
}
fn boundary() -> Boundary {
    Boundary {
        generation: RuntimeGeneration::INITIAL,
        at: LogicalTime::new(1.).unwrap(),
    }
}
fn fixture() -> World {
    let mut world = World::new();
    let template = WorldSeed::from_json_str(include_str!("../../tests/fixtures/demo_seed.json"))
        .unwrap()
        .characters[0]
        .clone();
    for (name, x) in [("actor", 0.), ("prisoner", 1.), ("outsider", 2.)] {
        let mut sheet = template.clone();
        sheet.id = id(name);
        sheet.position_m = Vec3::new(x, WALK_Y, 0.);
        sheet.lore = None;
        world.add_character(Character::from_sheet(sheet));
    }
    world.nav = Some(std::sync::Arc::new(crate::nav::local_tests::surface(&[])));
    world
}
fn hold(world: &mut World) {
    world.custody.seed_inmate(
        id("prisoner"),
        Station {
            place_id: crate::PlaceId::from_raw("station"),
            name: "A station".into(),
            point: Vec3::ZERO,
            stone_house: false,
        },
    );
}

#[test]
fn direct_action_and_actor_route_share_current_custody_denial() {
    let mut world = fixture();
    hold(&mut world);
    let receipt = Receipt::capture(
        &world,
        Request::VoluntaryTravel(&id("prisoner")),
        boundary(),
    )
    .unwrap();
    assert_eq!(receipt.decision(), Err(Denial::InCustody));
    assert!(
        receipt
            .denial_clue()
            .unwrap()
            .contains("speak to whoever holds you")
    );
    assert!(matches!(
        receipt.route_nodes(&world, boundary(), &[], 0, 0, 0),
        Err(Error::Denied(Denial::InCustody))
    ));
    let revision = world.world_revision;
    let error = apply_action(
        &mut world,
        &id("prisoner"),
        "go_to",
        &json!({"place_id":"imaginary"}),
    )
    .unwrap_err();
    assert_eq!(error.code, ActionErrorCode::InCustody);
    assert_eq!(error.message, receipt.denial_clue().unwrap());
    assert_eq!(world.world_revision, revision);
    assert!(world.characters[&id("prisoner")].state.intent.is_none());
    world.custody.release(&id("prisoner"));
    assert_eq!(
        receipt.require_current(&world, boundary()),
        Err(Error::Stale)
    );
    assert_eq!(
        receipt.decision(),
        Err(Denial::InCustody),
        "old denial is not rewritten"
    );
    assert!(
        Receipt::capture(
            &world,
            Request::VoluntaryTravel(&id("prisoner")),
            boundary()
        )
        .unwrap()
        .decision()
        .is_ok()
    );
}

#[test]
fn keeper_authority_is_scoped_revocable_and_rechecked_by_direct_release() {
    let mut world = fixture();
    hold(&mut world);
    world.custody.grab(&id("prisoner"), id("actor"));
    let receipt = Receipt::capture(
        &world,
        Request::ReleaseCustody {
            actor: &id("actor"),
            prisoner: &id("prisoner"),
        },
        boundary(),
    )
    .unwrap();
    assert!(receipt.require_current(&world, boundary()).is_ok());
    assert!(matches!(
        receipt.route_nodes(&world, boundary(), &[], 0, 0, 0),
        Err(Error::WrongOperation)
    ));
    let revision = world.world_revision;
    world.custody.let_go(&id("prisoner"), &id("actor"));
    assert_eq!(
        world.world_revision, revision,
        "hot authority can change without public revision"
    );
    assert_eq!(
        receipt.require_current(&world, boundary()),
        Err(Error::Stale)
    );
    let before = world.custody.clone();
    let error = apply_action(
        &mut world,
        &id("actor"),
        "release",
        &json!({"person":"prisoner"}),
    )
    .unwrap_err();
    assert_eq!(error.message, Denial::NotKeeper.clue());
    assert_eq!(world.custody, before);
    world.custody.get_mut(&id("prisoner")).unwrap().officer = Some(id("actor"));
    assert!(
        apply_action(
            &mut world,
            &id("actor"),
            "release",
            &json!({"person":"prisoner"})
        )
        .is_ok()
    );
    assert!(!world.custody.holds(&id("prisoner")));
    assert!(
        receipt.decision().is_ok(),
        "historical check is retained but cannot execute"
    );
}

#[test]
fn route_tickets_require_permission_boundary_and_exact_geometry() {
    let mut world = fixture();
    let receipt =
        Receipt::capture(&world, Request::VoluntaryTravel(&id("actor")), boundary()).unwrap();
    let route = receipt
        .route_nodes(&world, boundary(), &[], 3, 0, 0)
        .unwrap();
    assert!(route.route(&world, boundary(), &[], 3).is_ok());
    assert!(matches!(
        route.route(&world, boundary(), &[], 4),
        Err(Error::Geometry(QueryError::Stale))
    ));
    let closure = [Closure {
        id: 1,
        min: [100., 100.],
        max: [101., 101.],
    }];
    assert!(matches!(
        route.route(&world, boundary(), &closure, 3),
        Err(Error::Geometry(QueryError::Stale))
    ));
    let mut later = boundary();
    later.at = LogicalTime::new(1.01).unwrap();
    assert_eq!(receipt.require_current(&world, later), Err(Error::Stale));
    let mut generation = boundary();
    generation.generation = RuntimeGeneration(1);
    assert_eq!(
        receipt.require_current(&world, generation),
        Err(Error::Stale)
    );
    world
        .characters
        .get_mut("actor")
        .unwrap()
        .state
        .position_m
        .x += 0.1;
    assert!(matches!(
        route.route(&world, boundary(), &[], 3),
        Err(Error::Stale)
    ));
    world
        .characters
        .get_mut("actor")
        .unwrap()
        .state
        .position_m
        .x -= 0.1;
    world
        .characters
        .get_mut("actor")
        .unwrap()
        .state
        .leaving_city = true;
    assert_eq!(
        receipt.require_current(&world, boundary()),
        Err(Error::Stale)
    );
    assert_eq!(
        voluntary_travel(&world, &id("actor")),
        Err(Denial::LeavingCity)
    );
}

#[test]
fn bounded_holder_inspection_and_malformed_ids_refuse_without_mutation() {
    let mut world = fixture();
    hold(&mut world);
    for index in 0..=MAX_AUTHORITY_HOLDERS {
        let mut sheet = world.characters[&id("actor")].sheet.clone();
        sheet.id = id(&format!("holder_{index}"));
        let holder = sheet.id.clone();
        world.add_character(Character::from_sheet(sheet));
        assert!(world.custody.grab(&id("prisoner"), holder));
    }
    world.custody.get_mut(&id("prisoner")).unwrap().officer = Some(id("actor"));
    assert_eq!(
        release_custody(&world, &id("actor"), &id("prisoner")),
        Ok(())
    );
    assert!(matches!(
        Receipt::capture(
            &world,
            Request::ReleaseCustody {
                actor: &id("actor"),
                prisoner: &id("prisoner")
            },
            boundary()
        ),
        Err(Denial::Capacity)
    ));
    assert!(
        apply_action(
            &mut world,
            &id("actor"),
            "release",
            &json!({"person":"prisoner"})
        )
        .is_ok()
    );
    assert!(!world.custody.holds(&id("prisoner")));
    assert_eq!(
        voluntary_travel(&world, &id(&"x".repeat(ID_BYTES + 1))),
        Err(Denial::Capacity)
    );
    assert_eq!(voluntary_travel(&world, &id("")), Err(Denial::Invalid));
}

#[test]
fn committed_operation_owns_the_same_voluntary_capability() {
    use crate::operations::{
        self, AdapterDeclaration, FixtureDeclaration, OperationConfig, OperationKernel,
    };
    let mut world = fixture();
    let before =
        Receipt::capture(&world, Request::VoluntaryTravel(&id("actor")), boundary()).unwrap();
    world.operations = OperationKernel::from_config(&OperationConfig {
        fixtures: vec![FixtureDeclaration {
            id: "bench".into(),
            adapter: AdapterDeclaration::default(),
            position: [0., WALK_Y, 0.],
        }],
    })
    .unwrap();
    let mut round = crate::round::Round::new();
    let clock = crate::WorldClock::new(3600., crate::Office::Dayspring, 0, 0.05);
    let command = crate::receipts::OperationId {
        producer: 0,
        sequence: 1,
    }
    .command(0);
    let request = operations::Request::Start {
        actor: id("actor"),
        resource: "bench".into(),
        adapter: Default::default(),
        work_seconds: 10.,
        recovery_seconds: 20.,
        retries: 0,
    };
    let crate::receipts::Admission::New(ticket) = world
        .command_ledger
        .begin(command, &json!({"request":request}))
    else {
        panic!("fresh operation command admission");
    };
    let outcome = operations::command(
        &mut world,
        &mut round,
        &clock,
        boundary().at,
        command,
        request,
    );
    world
        .command_ledger
        .finish(ticket, boundary().at.seconds(), outcome.clone(), Vec::new());
    assert!(
        world.operations.actor_owner(&id("actor")).is_some(),
        "{outcome:?}"
    );
    assert_eq!(
        voluntary_travel(&world, &id("actor")),
        Err(Denial::CommittedWork)
    );
    assert_eq!(
        before.require_current(&world, boundary()),
        Err(Error::Stale)
    );
    let denied =
        Receipt::capture(&world, Request::VoluntaryTravel(&id("actor")), boundary()).unwrap();
    assert!(matches!(
        denied.route_nodes(&world, boundary(), &[], 0, 0, 0),
        Err(Error::Denied(Denial::CommittedWork))
    ));
    assert_eq!(
        apply_action(&mut world, &id("actor"), "go_to", &json!({}))
            .unwrap_err()
            .message,
        Denial::CommittedWork.clue()
    );
}
