use super::*;

fn nav() -> &'static NavData {
    static NAV: std::sync::OnceLock<NavData> = std::sync::OnceLock::new();
    NAV.get_or_init(|| {
        NavData::from_parts(
            include_str!("../../../../../assets/world/navigation.json"),
            include_bytes!("../../../../../assets/world/navigation.bin"),
        )
        .unwrap()
    })
}
fn fixture(count: usize, office: Office) -> (World, Round, WorldClock) {
    let mut world = World::new();
    for s in crate::crowd::generate_ambient(nav(), count, 0, &[], &[])
        .unwrap()
        .sheets
    {
        world.add_character(Character::from_sheet(s));
    }
    let clock = WorldClock::new(3600.0, office, 2, 0.0);
    let mut round = Round::new();
    round.seed(&mut world, nav(), 0.0, &clock);
    (world, round, clock)
}
fn slice(
    world: &mut World,
    round: &mut Round,
    clock: &WorldClock,
    now: f64,
    held: &BTreeSet<ActorId>,
) {
    world.step_movement(0.05, nav(), None);
    let mut nudges = Vec::new();
    super::super::decay_needs(round, world, nav(), clock, now);
    super::tick(round, world, nav(), clock, now, held, &mut nudges);
}

fn full_tick(
    world: &mut World,
    round: &mut Round,
    clock: &WorldClock,
    now: f64,
    held: &BTreeSet<ActorId>,
) {
    super::super::tick(
        round,
        world,
        nav(),
        clock,
        now,
        &ActorId::from_raw("player"),
        held,
    );
}

#[test]
fn fresh_explicit_travel_owns_replaced_feet_then_returns_once_without_teleport() {
    let (mut world, mut round, clock) = fixture(1, Office::Dayspring);
    let id = round.residents.order[0].clone();
    let mut residents = std::mem::take(&mut round.residents);
    let mut r = residents.people.remove(&id).unwrap();
    let patch = &nav().resident_places().patches[r.patch];
    let target = (r.preferred_spot + 1) % patch.spots.len();
    assert!(start_move(
        &mut round,
        &mut world,
        nav(),
        &mut residents,
        &id,
        &mut r,
        target,
        true
    ));
    residents.people.insert(id.clone(), r);
    round.residents = residents;
    let origin = world.characters[&id].position_m();
    let away = (0..nav().node_count())
        .map(|n| nav().node_point(n))
        .find(|p| {
            let d = p.distance(origin);
            (60.0..=90.0).contains(&d)
                && nav()
                    .connected_route(origin, *p, lane_fraction(&id))
                    .is_some_and(|r| r.length_m < 150.0)
        })
        .unwrap();
    world.characters.get_mut(&id).unwrap().state.intent = Some(crate::TravelIntent {
        target: IntentTarget::Place {
            place_id: crate::PlaceId::from_raw("fixture"),
            name: "the test visit".into(),
            point: away,
        },
        budget_seconds: 400.0,
        deadline: None,
    });
    full_tick(&mut world, &mut round, &clock, 0.05, &BTreeSet::new());
    assert!(
        world.characters[&id].is_walking(),
        "cleaning the resident claim must retain new explicit feet"
    );
    assert!(round.people[&id].travel_for_intent);
    assert!(!round.residents.optional.contains(&id));
    assert_eq!(round.resident_reservations().destination_count(), 0);
    let mut reached_visit = false;
    let mut return_starts = 0;
    let mut returning = false;
    let mut arrival_at = None;
    for step in 2..=14_000 {
        let now = step as f64 * 0.05;
        let previous = world.characters[&id].position_m();
        world.step_movement(0.05, nav(), None);
        let current = world.characters[&id].position_m();
        assert!(
            previous.distance(current) <= crate::WALK_SPEED_MPS * 0.05 + 1e-6,
            "no teleport"
        );
        assert!(nav().segment_walkable_exact(previous, current));
        reached_visit |= current.distance(away) <= PLACE_ARRIVE_RADIUS_M;
        full_tick(&mut world, &mut round, &clock, now, &BTreeSet::new());
        let r = &round.residents.people[&id];
        let is_return = r.phase == ResidentPhase::Returning;
        if is_return && !returning {
            return_starts += 1;
        }
        returning = is_return;
        if reached_visit
            && return_starts > 0
            && r.spot.is_some()
            && !world.characters[&id].is_walking()
        {
            let arrived = *arrival_at.get_or_insert(now);
            if now - arrived > 30.0 {
                break;
            }
        }
    }
    assert!(reached_visit && arrival_at.is_some());
    assert_eq!(return_starts, 1);
}

#[test]
fn conversation_releases_target_immediately_and_settled_talkers_keep_meals() {
    let (mut world, mut round, clock) = fixture(12, Office::Dayspring);
    let held: BTreeSet<_> = round.residents.order.iter().cloned().collect();
    for id in &held {
        super::super::interrupt_for_conversation(&mut round, &mut world, id);
    }
    for step in 1..=144_000 {
        let now = step as f64 * 0.05;
        slice(&mut world, &mut round, &clock, now, &held);
        if now > 60.0 {
            assert!(
                world
                    .characters
                    .values()
                    .all(|c| c.needs().hunger > HUNGER_FAMISHED)
            );
        }
        assert!(world.characters.values().all(|c| !c.is_walking()));
    }
    assert_eq!(round.resident_reservations().occupied_count(), held.len());
}

#[test]
fn retained_origin_can_be_recovered_when_every_other_spot_is_taken() {
    let (mut world, mut round, clock) = fixture(1, Office::Dayspring);
    let id = round.residents.order[0].clone();
    let r = &round.residents.people[&id];
    let patch = &nav().resident_places().patches[r.patch];
    let original = r.preferred_spot;
    let position = patch.spots[original].position();
    for (s, spot) in patch
        .spots
        .iter()
        .enumerate()
        .filter(|(s, _)| *s != original)
    {
        assert!(round.residents.reservations.occupy(
            &ActorId::from_raw(format!("block{s}")),
            &spot.id,
            spot.position()
        ));
    }
    let displaced = [
        Vec3::new(0.5, 0., 0.),
        Vec3::new(-0.5, 0., 0.),
        Vec3::new(0., 0., 0.5),
        Vec3::new(0., 0., -0.5),
    ]
    .into_iter()
    .map(|d| position + d)
    .find(|p| nav().segment_walkable_exact(position, *p))
    .unwrap();
    world.characters.get_mut(&id).unwrap().state.position_m = displaced;
    round.residents.people.get_mut(&id).unwrap().dwell_until = 0.0;
    for step in 1..=100 {
        slice(
            &mut world,
            &mut round,
            &clock,
            step as f64 * 0.05,
            &BTreeSet::new(),
        );
    }
    assert_eq!(round.residents.people[&id].spot, Some(original));
    assert!(patch.spots[original].contains(world.characters[&id].position_m()));
    assert_eq!(
        round
            .resident_reservations()
            .owner(&patch.spots[original].id),
        Some(&id)
    );
}

#[test]
fn weather_uses_distinct_local_claims_and_never_calls_the_global_shelter_ladder() {
    let (mut world, mut round, clock) = fixture(1000, Office::Dayspring);
    world.shelters = std::sync::Arc::new(
        crate::ShelterMap::from_json_str(include_str!("../../../../../assets/world/shelters.json"))
            .unwrap(),
    );
    super::seed(&mut round, &mut world, nav(), clock.at(0.0), 0.0);
    world.current_weather = Some(WeatherSample {
        kind: WeatherKind::Downpour,
        precipitation: 0.9,
        ..WeatherSample::CLEAR
    });
    let mut seekers = BTreeSet::new();
    for step in 1..=1200 {
        slice(
            &mut world,
            &mut round,
            &clock,
            step as f64 * 0.05,
            &BTreeSet::new(),
        );
        let mut targets = BTreeSet::new();
        for (id, r) in &round.residents.people {
            if let Some(w) = &r.weather {
                seekers.insert(id.clone());
                let spot = &nav().resident_places().shelter_spots[w.slot].spot;
                assert!(targets.insert(w.slot));
                assert_eq!(round.resident_reservations().owner(&spot.id), Some(id));
                assert!(
                    nav().resident_places().patches[r.patch].spots[0]
                        .position()
                        .distance(spot.position())
                        <= 30.0
                );
                if w.arrived {
                    assert!(
                        world
                            .shelters
                            .is_sheltered(world.characters[id].position_m())
                    );
                    assert!(world.is_present(id));
                }
            }
        }
        assert!(round.weather_shelter_intents.is_empty());
    }
    assert!(
        seekers.len() >= 2,
        "fixture needs independently reserved shelter trips"
    );
    println!("local weather seekers after 60s: {}", seekers.len());
    world.current_weather = Some(WeatherSample::CLEAR);
    for step in 1201..=3600 {
        slice(
            &mut world,
            &mut round,
            &clock,
            step as f64 * 0.05,
            &BTreeSet::new(),
        );
    }
    assert!(round.residents.people.values().all(|r| r.weather.is_none()));
}

#[test]
fn absence_removal_custody_and_release_do_not_leak_claims_or_erase_new_owner_feet() {
    let (mut world, mut round, clock) = fixture(1, Office::Dayspring);
    let id = round.residents.order[0].clone();
    let mut residents = std::mem::take(&mut round.residents);
    let mut r = residents.people.remove(&id).unwrap();
    let patch = &nav().resident_places().patches[r.patch];
    let target = (r.preferred_spot + 1) % patch.spots.len();
    assert!(start_move(
        &mut round,
        &mut world,
        nav(),
        &mut residents,
        &id,
        &mut r,
        target,
        true
    ));
    residents.people.insert(id.clone(), r);
    round.residents = residents;
    super::super::interrupt_for_conversation(&mut round, &mut world, &id);
    assert_eq!(round.resident_reservations().destination_count(), 0);
    assert!(!world.characters[&id].is_walking());
    world.characters.get_mut(&id).unwrap().state.leaving_city = true;
    full_tick(&mut world, &mut round, &clock, 1.0, &BTreeSet::new());
    assert!(round.resident_reservations().claims(&id).is_none());
    world.characters.get_mut(&id).unwrap().state.leaving_city = false;
    full_tick(&mut world, &mut round, &clock, 1.05, &BTreeSet::new());
    assert!(round.resident_reservations().claims(&id).is_some());
    world.custody.seed_inmate(
        id.clone(),
        crate::custody::Station {
            place_id: crate::PlaceId::from_raw("fixture_station"),
            name: "the test station".into(),
            point: world.characters[&id].position_m(),
            stone_house: true,
        },
    );
    let escort_path = nav()
        .local_route(
            world.characters[&id].position_m(),
            patch.spots[target].position(),
            LocalPathBudget::VISIT,
        )
        .unwrap()
        .into_movement(0.0);
    world.characters.get_mut(&id).unwrap().state.movement = Some(escort_path.clone());
    full_tick(&mut world, &mut round, &clock, 2.0, &BTreeSet::new());
    assert!(round.resident_reservations().claims(&id).is_none());
    assert_eq!(
        world.characters[&id].state.movement.as_ref(),
        Some(&escort_path)
    );
    world.custody.release(&id);
    world.characters.get_mut(&id).unwrap().state.movement = None;
    full_tick(&mut world, &mut round, &clock, 2.05, &BTreeSet::new());
    assert!(round.resident_reservations().claims(&id).is_some());
    world.characters.remove(&id);
    full_tick(&mut world, &mut round, &clock, 3.0, &BTreeSet::new());
    assert!(round.resident_reservations().claims(&id).is_none());
    assert_eq!(round.resident_count(), 0);
}

#[test]
fn resident_projection_is_truthful_without_advertising_internal_spots() {
    let (world, round, _) = fixture(1, Office::Dayspring);
    let id = &round.residents.order[0];
    let status = world.characters[id].state.resident.as_ref().unwrap();
    assert_eq!(status.phase, ResidentPhase::Lingering);
    assert!(!status.sheltered);
    assert!(!status.description().contains(&status.patch));
    let snapshot = world.public_snapshot(id);
    assert_eq!(snapshot.actors[0].resident.as_ref(), Some(status));
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(value["actors"][0]["resident"]["phase"], "lingering");
    assert!(
        !world.characters[id]
            .state
            .daily_round
            .iter()
            .any(|line| line.contains(&status.patch))
    );
}

#[test]
fn local_phase_and_cause_updates_leave_the_cold_world_revision_alone() {
    let (mut world, mut round, _) = fixture(2, Office::Dayspring);
    let ids = round.residents.order.clone();
    assert_eq!(round.drain_resident_updates(&world).len(), 2);
    let revision = world.world_revision;
    let position = world.characters[&ids[0]].position_m();
    round.residents.people.get_mut(&ids[0]).unwrap().phase = ResidentPhase::Interrupted;
    project(&mut round, &mut world, nav(), 1.0);
    let updates = round.drain_resident_updates(&world);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, ids[0]);
    assert_eq!(updates[0].1.phase, ResidentPhase::Interrupted);
    assert_eq!(world.characters[&ids[0]].position_m(), position);
    assert_eq!(world.world_revision, revision);
    project(&mut round, &mut world, nav(), 2.0);
    assert!(
        round.drain_resident_updates(&world).is_empty(),
        "countdown is not a new state"
    );
    world.custody.seed_inmate(
        ids[0].clone(),
        crate::custody::Station {
            place_id: crate::PlaceId::from_raw("fixture_station"),
            name: "test station".into(),
            point: position,
            stone_house: true,
        },
    );
    project(&mut round, &mut world, nav(), 3.0);
    let updates = round.drain_resident_updates(&world);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].1.phase, ResidentPhase::Interrupted);
    assert_eq!(updates[0].1.movement_cause, motion::MotionCause::Custody);
    assert_eq!(world.world_revision, revision);
    let r = round.residents.people.get_mut(&ids[1]).unwrap();
    r.dwell_until += 90.0;
    project(&mut round, &mut world, nav(), 4.0);
    assert_eq!(round.drain_resident_updates(&world)[0].0, ids[1]);
    assert_eq!(world.world_revision, revision);
}

#[test]
fn removing_a_nonmoving_patchmate_preserves_the_active_departure_claim() {
    let (mut world, mut round, clock) = fixture(1, Office::Dayspring);
    let id = round.residents.order[0].clone();
    let mut residents = std::mem::take(&mut round.residents);
    let mut r = residents.people.remove(&id).unwrap();
    let patch = r.patch;
    let target = (r.preferred_spot + 1) % nav().resident_places().patches[patch].spots.len();
    assert!(start_move(
        &mut round,
        &mut world,
        nav(),
        &mut residents,
        &id,
        &mut r,
        target,
        true
    ));
    // This is the record left by a removed, nonmoving neighbour. It never
    // acquired the optional departure flag owned by the first resident.
    let gone = ActorId::from_raw("removed");
    let mut neighbour = r.clone();
    neighbour.target = None;
    residents.people.insert(gone.clone(), neighbour);
    residents.order.push(gone);
    residents.people.insert(id.clone(), r);
    round.residents = residents;
    full_tick(&mut world, &mut round, &clock, 0.05, &BTreeSet::new());
    assert!(round.residents.optional.contains(&id));
    assert!(round.residents.departing.contains(&patch));
}

#[test]
fn lightning_and_curfew_preempt_resident_feet_without_optional_recall() {
    let (mut world, mut round, clock) = fixture(1, Office::Dayspring);
    let id = round.residents.order[0].clone();
    let mut residents = std::mem::take(&mut round.residents);
    let mut r = residents.people.remove(&id).unwrap();
    let target = (r.preferred_spot + 1) % nav().resident_places().patches[r.patch].spots.len();
    assert!(start_move(
        &mut round,
        &mut world,
        nav(),
        &mut residents,
        &id,
        &mut r,
        target,
        true
    ));
    residents.people.insert(id.clone(), r);
    round.residents = residents;
    round.lightning_reflex_until.insert(id.clone(), 10.0);
    full_tick(&mut world, &mut round, &clock, 0.05, &BTreeSet::new());
    assert!(!world.characters[&id].is_walking());
    assert_eq!(round.resident_reservations().destination_count(), 0);
    assert!(round.residents.optional.is_empty());
    round.lightning_reflex_until.clear();
    // This housed resident's explicit errand is interrupted by curfew even
    // while a conversation holds the ordinary routine.
    assert!(round.people[&id].home.is_some());
    world.characters.get_mut(&id).unwrap().state.intent = Some(crate::TravelIntent {
        target: IntentTarget::Place {
            place_id: crate::PlaceId::from_raw("fixture"),
            name: "a visit".into(),
            point: nav().node_point(nav().forecourt()),
        },
        budget_seconds: 500.,
        deadline: Some(500.),
    });
    let night = WorldClock::new(3600., Office::Watch, 2, 0.0);
    full_tick(
        &mut world,
        &mut round,
        &night,
        1.0,
        &BTreeSet::from([id.clone()]),
    );
    assert!(world.characters[&id].state.intent.is_none());
    assert!(world.is_present(&id));
}

#[test]
fn residents_have_no_jobs_queues_or_legs_and_keep_their_frontage_claims() {
    let (mut world, mut round, clock) = fixture(200, Office::Dayspring);
    assert_eq!(round.resident_count(), 200);
    assert_eq!(round.resident_reservations().occupied_count(), 200);
    assert!(
        round
            .people
            .values()
            .all(|p| p.legs.is_empty() && p.source.is_none() && p.leash_m == 0.0)
    );
    assert!(
        world
            .characters
            .values()
            .all(|c| !c.state.daily_round.is_empty())
    );
    assert_eq!(round.reroll_ambient_evenings(&mut world, 3), 0);
    let mut movers = BTreeSet::new();
    let mut peak = 0;
    for step in 1..=12_000 {
        let now = step as f64 * 0.05;
        world.step_movement(0.05, nav(), None);
        super::super::tick(
            &mut round,
            &mut world,
            nav(),
            &clock,
            now,
            &ActorId::from_raw("player"),
            &BTreeSet::new(),
        );
        let active = round.residents.optional.len();
        peak = peak.max(active);
        assert!(active <= 20);
        assert_eq!(
            round.residents.departing.len(),
            active,
            "one optional departure per patch"
        );
        movers.extend(round.residents.optional.iter().cloned());
        for id in &round.residents.optional {
            let r = &round.residents.people[id];
            let p = &nav().resident_places().patches[r.patch];
            let target = &p.spots[r.target.unwrap()];
            let c = &world.characters[id];
            assert!(c.state.movement.as_ref().unwrap().exact_local);
            assert_eq!(round.resident_reservations().owner(&target.id), Some(id));
        }
    }
    println!(
        "resident600s count=200 unique_optional_walkers={} peak_optional={peak}",
        movers.len()
    );
    assert!(
        movers.len() > 120,
        "optional admission must rotate through the population"
    );
    assert!(peak > 0);
    assert!(
        round
            .people
            .values()
            .all(|p| p.food.is_none() && p.source.is_none())
    );
    assert!(round.sources.iter().all(|s| s.queue.is_empty()));
    for (id, r) in &round.residents.people {
        let p = &nav().resident_places().patches[r.patch];
        assert!(
            world.characters[id]
                .position_m()
                .distance(p.spots[r.preferred_spot].position())
                <= 8.2
        );
    }
}

#[test]
fn support_uses_game_time_does_not_mint_food_and_recovers_after_a_missed_meal() {
    let (mut world, mut round, clock) = fixture(24, Office::Dayspring);
    let initial_items = world.items.clone();
    let mut minimum_after_settling = HUNGER_MAX;
    for step in 1..=144_000 {
        let now = step as f64 * 0.05;
        slice(&mut world, &mut round, &clock, now, &BTreeSet::new());
        if now > 60.0 {
            for c in world.characters.values() {
                minimum_after_settling = minimum_after_settling.min(c.needs().hunger);
            }
        }
    }
    println!("resident_two_days min_hunger={minimum_after_settling:.3}");
    assert!(minimum_after_settling > HUNGER_FAMISHED);
    assert_eq!(
        world.items, initial_items,
        "local support changes no stock, inventory or purse"
    );
    let id = round.residents.order[0].clone();
    let r = round.residents.people.get_mut(&id).unwrap();
    let original = world.characters[&id].position_m();
    world.characters.get_mut(&id).unwrap().state.position_m = original + Vec3::new(50.0, 0.0, 0.0);
    world.characters.get_mut(&id).unwrap().state.needs.hunger = 10.0;
    r.support_was_eligible = false;
    r.phase = ResidentPhase::Interrupted;
    let c = &world.characters[&id];
    assert_eq!(support_gain(r, c, nav(), 4.5, 4.51), 0.0);
    assert!(r.missed_meal);
    world.characters.get_mut(&id).unwrap().state.position_m = original;
    // Use a known settled occupied spot after the independent two-day run.
    let p = &nav().resident_places().patches[r.patch];
    r.spot = Some(r.preferred_spot);
    r.phase = ResidentPhase::Lingering;
    world.characters.get_mut(&id).unwrap().state.position_m = p.spots[r.preferred_spot].position();
    world.characters.get_mut(&id).unwrap().state.movement = None;
    let c = &world.characters[&id];
    assert_eq!(support_gain(r, c, nav(), 4.95, 4.951), 0.0);
    let gain = support_gain(r, c, nav(), 4.951, 4.952);
    assert!(gain > 0.0 && gain < 100.0);
    assert_eq!(support_gain(r, c, nav(), 4.952, 4.952), 0.0);
}

#[test]
fn meal_accounting_has_no_phantom_breakfast_at_a_dayspring_session_start() {
    for day in [0.0, 2.0, 3.0, 20.0, 10_000.0] {
        let start = day + 7.0 / 24.0;
        assert_eq!(meal_overlap(start, start + 0.05 / 3600.0), 0.0);
        let breakfast = meal_overlap(day + 5.0 / 24.0, day + 7.0 / 24.0);
        assert!((breakfast - 2.0 / 24.0).abs() < 1e-10);
    }
    assert!((meal_overlap(2.0, 4.0) - 22.0 / 24.0).abs() < 1e-12);
}

#[test]
fn conversation_interruption_releases_the_optional_target() {
    let (mut world, mut round, clock) = fixture(32, Office::Dayspring);
    for step in 1..=4000 {
        slice(
            &mut world,
            &mut round,
            &clock,
            step as f64 * 0.05,
            &BTreeSet::new(),
        );
        if let Some(id) = round.residents.optional.iter().next().cloned() {
            let r = &round.residents.people[&id];
            let target = r.target.unwrap();
            let target_id = nav().resident_places().patches[r.patch].spots[target]
                .id
                .clone();
            let held = BTreeSet::from([id.clone()]);
            slice(
                &mut world,
                &mut round,
                &clock,
                step as f64 * 0.05 + 0.05,
                &held,
            );
            assert!(!world.characters[&id].is_walking());
            assert_eq!(round.resident_reservations().owner(&target_id), None);
            assert!(!round.residents.optional.contains(&id));
            assert_eq!(
                round.residents.people[&id].phase,
                ResidentPhase::Interrupted
            );
            return;
        }
    }
    panic!("a local move should have been admitted");
}

#[test]
fn arrival_and_failed_routes_each_start_one_dwell_without_recall_or_catchup() {
    for fail in [false, true] {
        let (mut world, mut round, clock) = fixture(32, Office::Dayspring);
        let mut tracked = None;
        for step in 1..=5000 {
            let now = step as f64 * 0.05;
            slice(&mut world, &mut round, &clock, now, &BTreeSet::new());
            if tracked.is_none() {
                if let Some(id) = round.residents.optional.iter().next().cloned() {
                    let r = &round.residents.people[&id];
                    tracked = Some((id, r.target.unwrap()));
                }
                continue;
            }
            let (id, target) = tracked.as_ref().unwrap();
            if fail {
                world.characters.get_mut(id).unwrap().state.movement = None;
            }
            if round.residents.optional.contains(id) {
                continue;
            }
            let r = &round.residents.people[id];
            assert!(r.target.is_none());
            let patch = &nav().resident_places().patches[r.patch];
            let spot = &patch.spots[*target];
            if fail {
                assert!(!spot.contains(world.characters[id].position_m()));
                assert_ne!(round.resident_reservations().owner(&spot.id), Some(id));
            } else {
                assert_eq!(r.spot, Some(*target));
                assert!(spot.contains(world.characters[id].position_m()));
                assert_eq!(round.resident_reservations().owner(&spot.id), Some(id));
            }
            assert!((DWELL_MIN_SECONDS..=DWELL_MAX_SECONDS).contains(&(r.dwell_until - now)));
            let epoch = r.epoch;
            let position = world.characters[id].position_m();
            // A sparse poll just before the deadline cannot accumulate earlier
            // wander opportunities or invoke the old six-metre post recall.
            let before_deadline = r.dwell_until - 0.01;
            super::tick(
                &mut round,
                &mut world,
                nav(),
                &clock,
                before_deadline,
                &BTreeSet::new(),
                &mut Vec::new(),
            );
            assert_eq!(round.residents.people[id].epoch, epoch);
            assert_eq!(world.characters[id].position_m(), position);
            assert!(!world.characters[id].is_walking());
            break;
        }
        assert!(tracked.is_some());
        let (id, _) = tracked.unwrap();
        assert!(
            !round.residents.optional.contains(&id),
            "walk should finish inside the fixture"
        );
    }
}

#[test]
fn home_rest_proxy_is_an_outdoor_reserved_spot_and_dawn_staggers() {
    let (mut world, mut round, clock) = fixture(100, Office::Snuffing);
    for id in &round.residents.order {
        let status = round.resident_status(nav(), &world, id, 0.0).unwrap();
        assert_eq!(status.phase, ResidentPhase::Resting);
        assert!(status.resting_at_household_frontage || status.resting_without_home);
        assert!(world.is_present(id));
        if let Some(home) = round.people[id].home {
            assert!(world.characters[id].position_m().distance(home) >= 2.0);
        }
    }
    for step in 1..=4800 {
        slice(
            &mut world,
            &mut round,
            &clock,
            step as f64 * 0.05,
            &BTreeSet::new(),
        );
    }
    assert!(round.residents.optional.is_empty());
    let time = WorldTime {
        day: 3,
        fraction: 6.0 / 24.0,
        office: Office::Kindling,
        weekday: Weekday::of_day(3),
    };
    let retired = round
        .residents
        .order
        .iter()
        .filter(|id| retiring(id, time))
        .count();
    assert!((25..75).contains(&retired));
}

#[test]
fn explicit_worker_uses_the_named_workplace_and_ordinary_archetype() {
    let rounds: serde_json::Value = serde_json::from_str(ROUNDS_JSON).unwrap();
    let workplace = rounds["workplaces"]["cargo_worker"][1]
        .as_str()
        .unwrap()
        .to_string();
    let worker = crate::crowd::WorkerOverride {
        index: 0,
        occupation: "cargo_worker".into(),
        workplace: workplace.clone(),
    };
    let crowd = crate::crowd::generate_ambient(nav(), 4, 0, &[], &[worker]).unwrap();
    let mut world = World::new();
    for s in crowd.sheets {
        world.add_character(Character::from_sheet(s));
    }
    let mut round = Round::new();
    let clock = WorldClock::new(3600.0, Office::Dayspring, 2, 0.0);
    round.seed(&mut world, nav(), 0.0, &clock);
    let id = ActorId::from_raw("x00000");
    assert!(!round.residents.people.contains_key(&id));
    assert!(
        round.people[&id]
            .legs
            .iter()
            .any(|l| l.doing == Arrival::Work && l.label == workplace)
    );
    assert!(round.people[&id].legs.iter().any(|l| l.is_home));
}
