use super::*;
use crate::event::DomainEvent;

fn boundary() -> ObservationBoundary {
    ObservationBoundary {
        generation: RuntimeGeneration::INITIAL,
        at: LogicalTime::new(2.).unwrap(),
    }
}

fn fixture() -> World {
    let mut world = World::new();
    world.nav = Some(std::sync::Arc::new(crate::nav::local_tests::surface(&[])));
    world.add_character(crate::perception::tests::character(
        "observer",
        Vec3::new(0., WALK_Y, 5.),
        0.,
    ));
    world.add_character(crate::perception::tests::character(
        "private_subject",
        Vec3::new(0., WALK_Y, 0.),
        0.,
    ));
    world.emit(DomainEvent::gesture(
        ActorId::from_raw("private_subject"),
        None,
        "wave",
        Vec3::new(0., WALK_Y, 0.),
        vec![],
    ));
    world
}
fn coverage() -> Volume {
    Volume {
        min: Vec3::new(-30., 0., -30.),
        max: Vec3::new(30., 5., 30.),
    }
}
fn capture(world: &World, geometry: &ObservationGeometry<'_>) -> ObservationReceipt {
    *ObservationSnapshot::capture_latest(
        world,
        geometry,
        &[&ActorId::from_raw("observer")],
        boundary(),
    )
    .unwrap()
    .receipts()
    .next()
    .unwrap()
}

#[test]
fn sampled_receipt_retains_only_supported_description() {
    let mut world = fixture();
    let geometry = ObservationGeometry::complete_fixture(coverage(), &[], 1).unwrap();
    let receipt = capture(&world, &geometry);
    assert_eq!(receipt.support(), VisualSupport::Visible);
    assert_eq!(receipt.visible_description(), Some("a stranger"));
    assert_eq!(receipt.source_event(), 1);
    assert_eq!(receipt.observed_at(), 2.);
    assert!(!format!("{receipt:?}").contains("private_subject"));
    world
        .characters
        .get_mut("observer")
        .unwrap()
        .state
        .knows
        .insert(ActorId::from_raw("private_subject"));
    assert_eq!(
        receipt.require_current(&world, &geometry, boundary()),
        Err(ObservationError::Stale)
    );
    assert_eq!(
        receipt.visible_description(),
        Some("a stranger"),
        "later identification never rewrites past support"
    );
    assert_eq!(
        capture(&world, &geometry).visible_description(),
        Some("PRIVATE_SUBJECT")
    );
}

#[test]
fn walls_turning_away_missing_coverage_and_other_floor_do_not_identify() {
    let mut world = fixture();
    let wall = Volume {
        min: Vec3::new(-1., 0., 2.),
        max: Vec3::new(1., 3., 3.),
    };
    for walls in [&[wall][..], &[wall, wall][..]] {
        let geometry = ObservationGeometry::complete_fixture(coverage(), walls, 1).unwrap();
        let receipt = capture(&world, &geometry);
        assert_eq!(receipt.support(), VisualSupport::Occluded);
        assert_eq!(receipt.visible_description(), None);
    }
    assert_eq!(
        capture(&world, &ObservationGeometry::unavailable(1)).support(),
        VisualSupport::Unavailable
    );
    let geometry = ObservationGeometry::complete_fixture(coverage(), &[], 1).unwrap();
    world
        .characters
        .get_mut("observer")
        .unwrap()
        .state
        .facing_yaw = std::f64::consts::PI;
    assert_eq!(
        capture(&world, &geometry).support(),
        VisualSupport::OutsideView
    );
    world
        .characters
        .get_mut("observer")
        .unwrap()
        .state
        .position_m
        .y = 3.91;
    assert_eq!(
        capture(&world, &geometry).support(),
        VisualSupport::Unavailable
    );
}

#[test]
fn movement_facing_geometry_navigation_and_revision_fence_fresh_use() {
    let mut world = fixture();
    let geometry = ObservationGeometry::complete_fixture(coverage(), &[], 1).unwrap();
    let receipt = capture(&world, &geometry);
    let revision = world.world_revision;
    world
        .characters
        .get_mut("private_subject")
        .unwrap()
        .state
        .position_m
        .x = 2.;
    assert_eq!(world.world_revision, revision);
    assert_eq!(
        receipt.require_current(&world, &geometry, boundary()),
        Err(ObservationError::Stale)
    );
    world
        .characters
        .get_mut("private_subject")
        .unwrap()
        .state
        .position_m
        .x = 0.;
    world
        .characters
        .get_mut("observer")
        .unwrap()
        .state
        .facing_yaw = 1.;
    assert_eq!(
        receipt.require_current(&world, &geometry, boundary()),
        Err(ObservationError::Stale)
    );
    world
        .characters
        .get_mut("observer")
        .unwrap()
        .state
        .facing_yaw = 0.;
    assert!(
        receipt
            .require_current(&world, &geometry, boundary())
            .is_ok()
    );
    assert_eq!(
        receipt.require_current(
            &world,
            &ObservationGeometry::complete_fixture(coverage(), &[], 2).unwrap(),
            boundary()
        ),
        Err(ObservationError::Stale)
    );
    let wall = [Volume {
        min: Vec3::new(-1., 0., 2.),
        max: Vec3::new(1., 3., 3.),
    }];
    assert_eq!(
        receipt.require_current(
            &world,
            &ObservationGeometry::complete_fixture(coverage(), &wall, 1).unwrap(),
            boundary()
        ),
        Err(ObservationError::Stale)
    );
    world.nav = Some(std::sync::Arc::new(crate::nav::local_tests::surface(&[(
        1, 1,
    )])));
    assert_eq!(
        receipt.require_current(&world, &geometry, boundary()),
        Err(ObservationError::Stale)
    );
}

#[test]
fn bounded_candidates_and_latest_owed_event_are_required_without_mutation() {
    let mut world = fixture();
    let geometry = ObservationGeometry::complete_fixture(coverage(), &[], 1).unwrap();
    let id = ActorId::from_raw("observer");
    let time = boundary();
    assert!(matches!(
        ObservationSnapshot::capture_latest(&world, &geometry, &[&id; MAX_OBSERVERS + 1], time),
        Err(ObservationError::Capacity)
    ));
    assert!(matches!(
        ObservationSnapshot::capture_latest(&world, &geometry, &[&id, &id], time),
        Err(ObservationError::Invalid)
    ));
    assert!(matches!(
        ObservationGeometry::complete_fixture(coverage(), &[coverage(); MAX_OCCLUDERS + 1], 1),
        Err(ObservationError::Capacity)
    ));
    let revision = world.world_revision;
    let receipt = capture(&world, &geometry);
    assert_eq!(world.world_revision, revision);
    assert_eq!(world.event_sequence, 1);
    let events = world.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(receipt.source_event(), events[0].sequence);
    assert!(matches!(
        ObservationSnapshot::capture_latest(&world, &geometry, &[&id], time),
        Err(ObservationError::MissingEvent)
    ));
    assert_eq!(receipt.visible_description(), Some("a stranger"));
}

#[test]
fn generation_quiet_time_and_presence_epoch_refuse_fresh_use() {
    let mut world = fixture();
    let geometry = ObservationGeometry::complete_fixture(coverage(), &[], 1).unwrap();
    let receipt = capture(&world, &geometry);
    let mut later = boundary();
    later.at = LogicalTime::new(2.01).unwrap();
    assert_eq!(
        receipt.require_current(&world, &geometry, later),
        Err(ObservationError::Stale)
    );
    let mut replacement = boundary();
    replacement.generation = RuntimeGeneration(1);
    assert_eq!(
        receipt.require_current(&world, &geometry, replacement),
        Err(ObservationError::Stale)
    );
    world
        .characters
        .get_mut("observer")
        .unwrap()
        .state
        .presence_epoch += 1;
    assert_eq!(
        receipt.require_current(&world, &geometry, boundary()),
        Err(ObservationError::Stale)
    );
    assert_eq!(receipt.observed_at(), 2.);
    assert_eq!(receipt.generation(), RuntimeGeneration::INITIAL);
    assert_eq!(receipt.visible_description(), Some("a stranger"));
}
