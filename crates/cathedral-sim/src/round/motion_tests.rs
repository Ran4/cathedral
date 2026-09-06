use super::*;
use crate::round::motion::{MotionCause, MotionProbe, positions};

#[test]
fn motion_counts_displacement_at_a_post_and_does_not_confuse_a_pending_path_with_walking() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let at = nav.node_point(nav.place("The Wickmarket").unwrap().node);
    let id = ActorId::from_raw("x00000");
    let mut actor = person(id.as_str(), at, Some("mason"), Significance::Ambient);
    actor.sheet.lore.as_mut().unwrap().generated = true;
    let mut world = base_world();
    world.characters.insert(id.clone(), actor);
    let mut round = Round::default();
    let mut enrolled = weather_person(at);
    enrolled.home = Some(at);
    enrolled.legs.push(RoundLeg {
        from: Office::Dayspring,
        at,
        label: "The Wickmarket".into(),
        doing: Arrival::Work,
        only_on: None,
        is_home: false,
    });
    enrolled.motion_cause = MotionCause::Wander;
    round.people.insert(id.clone(), enrolled);
    let mut probe = MotionProbe::new(&world);

    // A stale positive speed, a path and a new facing do not prove movement.
    set_route(&mut world, &id, vec![at + Vec3::X]);
    world
        .characters
        .get_mut(&id)
        .unwrap()
        .state
        .movement
        .as_mut()
        .unwrap()
        .speed = 1.4;
    let before = positions(&world, &round);
    world.characters.get_mut(&id).unwrap().state.facing_yaw = 1.0;
    let still = probe.sample(&world, &round, &clock, 0.0, &before);
    assert_eq!(still.generated.stationary, 1);
    assert_eq!(still.generated.stationary_with_path, 1);
    assert_eq!(still.generated.resting_at_home_proxy, 1);
    assert_eq!(
        still.generated.present, 1,
        "home proxy stays in the denominator"
    );
    assert_eq!(still.generated.indoors, None);

    let before = positions(&world, &round);
    world.step_movement(0.05, &nav, None);
    let moving = probe.sample(&world, &round, &clock, 0.05, &before);
    assert_eq!(round.census(&world, &clock, 0.05).at_post, 1);
    assert_eq!(moving.generated.optional_walking, 1);
    assert_eq!(moving.generated.stationary, 0);
    assert_eq!(moving.authored.total, 0);
    assert_eq!(moving.generated.walking_by_cause[&MotionCause::Wander], 1);
}

#[test]
fn motion_uses_committed_cause_even_when_the_clock_changes_and_custody_has_no_path() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let at = nav.node_point(nav.place("The Wickmarket").unwrap().node);
    let target = nav.node_point(nav.place("The Needle").unwrap().node);
    let id = ActorId::from_raw("probe");
    let mut world = base_world();
    world
        .characters
        .insert(id.clone(), person("probe", at, None, Significance::Ambient));
    let mut round = Round::default();
    let mut enrolled = weather_person(at);
    enrolled.home = Some(target);
    round.people.insert(id.clone(), enrolled);
    let mut probe = MotionProbe::new(&world);
    for (decision, cause) in [
        (Decision::Wander(target), MotionCause::Wander),
        (Decision::Travel(target), MotionCause::Domestic),
        (Decision::TravelIntent(target), MotionCause::ExplicitIntent),
    ] {
        apply_decision(&mut round, &mut world, &nav, &id, decision);
        assert!(world.characters[&id].is_walking());
        assert_eq!(round.motion_cause(&world, &id), cause);
        let before = positions(&world, &round);
        world.step_movement(0.05, &nav, None);
        // The next ladder decision can replace/finish the route in this same
        // poll. Attribute the observed stride to the route which took it.
        round.people.get_mut(&id).unwrap().motion_cause = MotionCause::Other;
        let counted = probe.sample(&world, &round, &clock, 1000.0, &before);
        assert_eq!(counted.authored.walking_by_cause[&cause], 1);
    }
    // A custody follower is positioned by the real escort pass without
    // Movement; an unmoving follower is still stationary.
    let officer = ActorId::from_raw("officer");
    world.characters.insert(
        officer.clone(),
        person("officer", at, None, Significance::Minor),
    );
    world.custody.seize(
        id.clone(),
        officer.clone(),
        None,
        crate::custody::Station {
            place_id: PlaceId::from_raw("pl_probe"),
            name: "Probe station".into(),
            point: target,
            stone_house: false,
        },
        1000.0,
    );
    crate::custody::follow_escorts(&mut world, 1000.0);
    assert!(world.characters[&id].state.movement.is_none());
    let before = positions(&world, &round);
    assert_eq!(
        probe
            .sample(&world, &round, &clock, 1001.0, &before)
            .authored
            .stationary,
        2
    );
    world
        .characters
        .get_mut(&officer)
        .unwrap()
        .state
        .position_m
        .x += 0.1;
    crate::custody::follow_escorts(&mut world, 1001.05);
    assert_eq!(
        probe
            .sample(&world, &round, &clock, 1001.05, &before)
            .authored
            .emergency_custody_walking,
        2
    );
}

#[test]
fn motion_reports_actual_queue_membership_without_removing_waiters_from_present() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let at = nav.node_point(nav.place("The Wickmarket").unwrap().node);
    let id = ActorId::from_raw("probe");
    let mut world = base_world();
    world.characters.insert(
        id.clone(),
        person("probe", at, Some("mason"), Significance::Ambient),
    );
    let mut round = Round::default();
    round.seed(&mut world, &nav, 0.0, &clock);
    assert!(!round.sources.is_empty() && !round.stalls.is_empty());
    // The source vectors are authoritative even before the person's cached
    // phase changes. Deliberately exercise the diagnostic against that seam.
    round.sources[0].queue.push(id.clone());
    round.stalls[0].queue.push(id);
    let mut probe = MotionProbe::new(&world);
    let census = probe.sample(&world, &round, &clock, 0.05, &positions(&world, &round));
    assert_eq!(census.authored.present, 1);
    assert_eq!(census.authored.stationary, 1);
    assert_eq!(census.authored.water_queue, 1);
    assert_eq!(census.authored.food_queue, 1);
    assert_eq!(census.authored.queues.len(), 2);
}

#[test]
fn motion_keeps_baseline_cells_time_away_and_absence_separate() {
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("probe");
    let at = Vec3::new(-0.1, 1.0, 0.0);
    let mut world = base_world();
    world
        .characters
        .insert(id.clone(), person("probe", at, None, Significance::Ambient));
    let round = Round::default();
    let mut probe = MotionProbe::new(&world);
    let before = positions(&world, &round);
    let first = probe.sample(&world, &round, &clock, 0.0, &before);
    assert_eq!(first.authored.occupied_cells["-1:0"], 1);
    world.characters.get_mut(&id).unwrap().state.position_m.x += 40.0;
    let before = positions(&world, &round);
    let away = probe.sample(&world, &round, &clock, 30.0, &before);
    assert_eq!(
        away.authored.stationary, 1,
        "stationary can be far from initial placement"
    );
    assert_eq!(away.authored.away_from_initial_15m, 1);
    assert_eq!(away.authored.sampled_away_seconds, 30.0);
    assert_eq!(away.authored.in_initial_cell, 0);
    world.characters.get_mut(&id).unwrap().state.presence = crate::Presence::BeyondTheWalls;
    let gone = probe.sample(&world, &round, &clock, 60.0, &before);
    assert_eq!(gone.authored.absent, 1);
    assert_eq!(gone.authored.present, 0);
    assert!(gone.authored.occupied_cells.is_empty());
}
