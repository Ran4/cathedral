//! Tests for the daily round (M4) and the water round it subsumes (M3).

use super::*;
use crate::{
    Office, WorldClock,
    character::{CharacterSheet, Control},
    event::EventType,
    lore::{LoreProfile, PlanningWard},
    sounds::SoundCatalog,
};
use std::collections::BTreeSet;

const NAV_JSON: &str = include_str!("../../../../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../../../../assets/world/navigation.bin");
const CATALOG: &str = include_str!("../../../../assets/sounds/catalog.toml");

fn nav() -> NavData {
    NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed nav loads")
}

/// One game day per real hour, opening on `office` — the shipped default clock.
fn clock_at(office: Office) -> WorldClock {
    WorldClock::new(3600.0, office, 0, 0.05)
}

fn player() -> ActorId {
    ActorId::from_raw("player")
}

/// A character at `position` with an optional occupation and significance.
fn person(id: &str, position: Vec3, occupation: Option<&str>, significance: Significance) -> Character {
    let lore = occupation.map(|occupation_id| LoreProfile {
        significance,
        planning_ward: PlanningWard::Fabric,
        age: 30,
        gender: "f".into(),
        occupation_id: Some(occupation_id.into()),
        occupation_display: None,
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Fabric".into(),
        father: None,
        mother: None,
        children: Vec::new(),
        circumstances: Vec::new(),
        conditions: Vec::new(),
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
    });
    Character::from_sheet(CharacterSheet {
        id: ActorId::from_raw(id),
        name: id.to_uppercase(),
        control: Control::Llm,
        back_story: String::new(),
        location_description: String::new(),
        appearance_key: id.into(),
        voice_key: None,
        position_m: position,
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore,
    })
}

fn base_world() -> World {
    let mut world = World::new();
    world.sound_catalog = SoundCatalog::from_toml_str(CATALOG).expect("catalog loads");
    world
}

/// One engine-style beat: walk the movers a slice, then run the round.
fn beat(round: &mut Round, world: &mut World, nav: &NavData, clock: &WorldClock, now: f64, dt: f64) {
    world.step_movement(dt, nav);
    tick(round, world, nav, clock, now, &player(), None);
}

// --------------------------------------------------------------------------- //
// The embedded content
// --------------------------------------------------------------------------- //

/// Both authored files parse, cover the whole occupation set, and every place a
/// leg names resolves against the committed nav graph — no dangling destination.
#[test]
fn the_round_content_parses_and_every_destination_resolves() {
    let rounds: RoundsDoc = serde_json::from_str(ROUNDS_JSON).expect("rounds.json parses");
    let homes: HomesDoc = serde_json::from_str(HOMES_JSON).expect("homes.json parses");
    let nav = nav();
    let resolver = PlaceResolver::new(&nav);

    assert_eq!(homes.homes.len(), 400, "the committed bake houses 400 of the cast");
    assert_eq!(rounds.workplaces.len(), 65, "every occupation has a workplace list");
    assert_eq!(rounds.occupations.len(), 65, "every occupation has an archetype");

    // Every non-keyword destination resolves to a nav place or site.
    let check = |name: &str, whence: &str| {
        if name != "home" && name != "workplace" {
            assert!(resolver.resolve(name).is_some(), "{whence}: `{name}` does not resolve");
        }
    };
    for (occupation, places) in &rounds.workplaces {
        for place in places {
            check(place, occupation);
        }
    }
    for (name, template) in &rounds.archetypes {
        for leg in &template.legs {
            check(&leg.at, name);
        }
    }
    for (id, route) in &rounds.routes {
        for leg in &route.legs {
            check(&leg.at, id);
        }
    }
}

// --------------------------------------------------------------------------- //
// active_leg — the schedule
// --------------------------------------------------------------------------- //

fn leg(from: Office, label: &str, only_on: Option<Vec<Weekday>>) -> RoundLeg {
    RoundLeg {
        from,
        at: Vec3::new(0.0, WALK_Y, 0.0),
        label: label.into(),
        doing: Arrival::Work,
        only_on,
        is_home: label == "home",
    }
}

#[test]
fn active_leg_advances_with_the_office_and_carries_over_at_night() {
    let legs = vec![
        leg(Office::Kindling, "oven", None),
        leg(Office::Dayspring, "shop", None),
        leg(Office::Lamplight, "home", None),
    ];
    // The office selects the last-begun leg.
    assert_eq!(active_leg(&legs, Office::Kindling, Weekday::Bellday).unwrap().label, "oven");
    assert_eq!(active_leg(&legs, Office::HighWick, Weekday::Bellday).unwrap().label, "shop");
    assert_eq!(active_leg(&legs, Office::Snuffing, Weekday::Bellday).unwrap().label, "home");
    // Deep night before the first leg carries over the day's tail (home).
    assert_eq!(active_leg(&legs, Office::Watch, Weekday::Bellday).unwrap().label, "home");
}

#[test]
fn a_market_day_leg_wins_only_on_its_day() {
    // The generic post first, then the market-square leg for the same office.
    let legs = vec![
        leg(Office::Dayspring, "workshop", None),
        leg(Office::Dayspring, "square", Some(vec![Weekday::Highmarket])),
    ];
    assert_eq!(
        active_leg(&legs, Office::Dayspring, Weekday::Highmarket).unwrap().label,
        "square",
        "on the market day the crowd moves to the square"
    );
    assert_eq!(
        active_leg(&legs, Office::Dayspring, Weekday::Fourth).unwrap().label,
        "workshop",
        "on an ordinary day the market leg is filtered out"
    );
}

// --------------------------------------------------------------------------- //
// The round rung and curfew — using a real housed, routed major
// --------------------------------------------------------------------------- //

/// Seed a world holding one real major (`b4hst`, Hamel Stott the mason — housed,
/// with an authored route: Coswald's Yard by day, home to sleep), placed at a
/// neutral spot away from both. Returns the round, world, nav, and their id.
fn seed_hamel() -> (Round, World, NavData, ActorId) {
    let nav = nav();
    let id = ActorId::from_raw("b4hst");
    let mut world = base_world();
    // The forecourt — away from Coswald's Yard and away from his home.
    world.add_character(person("b4hst", Vec3::new(0.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    (round, world, nav, id)
}

#[test]
fn a_housed_major_is_enrolled_with_a_home_and_an_authored_route() {
    let (round, _world, _nav, id) = seed_hamel();
    let person = round.people.get(&id).expect("Hamel is enrolled");
    let home = person.home.expect("Hamel is housed");
    assert!(home.x.is_finite());
    // His route is the authored one: a Coswald's Yard leg at the Kindling.
    let kindling = person
        .legs
        .iter()
        .find(|leg| leg.from == Office::Kindling)
        .expect("the route has a Kindling leg");
    assert_eq!(kindling.label, "Coswald's Yard");
}

#[test]
fn the_round_rung_walks_a_worker_to_their_post() {
    let (round, world, nav, id) = seed_hamel();
    let coswalds = round
        .people[&id]
        .legs
        .iter()
        .find(|leg| leg.from == Office::Kindling)
        .expect("Kindling leg")
        .at;
    // At the working morning, away from his post, the round rung sends him there.
    match decide(&round, &world, &nav, &id, 0, Office::Kindling, Weekday::Bellday) {
        Decision::Travel(target) => assert!(
            target.distance(coswalds) < 1.0,
            "he sets off for Coswald's Yard, not {target:?}"
        ),
        other => panic!("expected Travel to the yard, got {other:?}"),
    }
}

/// An exchange with the player holds the round: the partner neither sets off
/// on an errand nor keeps walking one already begun, and the round resumes on
/// its own once the exchange goes cold.
#[test]
fn a_conversation_with_the_player_pins_the_round() {
    let nav = nav();
    let clock = clock_at(Office::Kindling);
    let id = ActorId::from_raw("b4hst");
    let mut world = base_world();
    world.add_character(person("b4hst", Vec3::new(0.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    // In conversation: his cadence comes due and he stays put.
    let due = round.people[&id].next_decision + 1.0;
    tick(&mut round, &mut world, &nav, &clock, due, &player(), Some(&id));
    assert!(
        !world.characters[&id].is_walking(),
        "nobody sets off for their post mid-conversation"
    );

    // The exchange goes cold: the same cadence now sends him to his post.
    tick(&mut round, &mut world, &nav, &clock, due, &player(), None);
    assert!(
        world.characters[&id].is_walking(),
        "the round resumes once the conversation lapses"
    );
    assert_eq!(round.people[&id].phase, Phase::Travelling);

    // Addressed mid-stride: he stops on the spot and is the ladder's again.
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(!world.characters[&id].is_walking(), "a walker stops to talk");
    assert_eq!(round.people[&id].phase, Phase::Idle);
}

#[test]
fn curfew_sends_the_housed_home_at_the_snuffing() {
    let (round, world, nav, id) = seed_hamel();
    let home = round.people[&id].home.expect("housed");
    match decide(&round, &world, &nav, &id, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::Travel(target) => assert!(
            target.distance(home) < 1.0,
            "at the Snuffing he goes home, not to {target:?}"
        ),
        other => panic!("expected Travel home, got {other:?}"),
    }
}

#[test]
fn a_night_trade_is_not_sent_home_by_curfew() {
    // A tavern worker (curfew-exempt) keeps their post at the Snuffing.
    let nav = nav();
    let id = ActorId::from_raw("tapster_x");
    let mut world = base_world();
    world.add_character(person("tapster_x", Vec3::new(0.0, WALK_Y, 95.0), Some("tavern_worker"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    assert!(round.people[&id].curfew_exempt, "the tavern archetype is curfew-exempt");
    // At the Snuffing the tavern's Snuffing leg is active — they work, not sleep.
    match decide(&round, &world, &nav, &id, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::Travel(_) | Decision::Stay | Decision::Wander(_) => {}
        Decision::ApproachWell => panic!("a tavern worker draws no water here"),
    }
    // And their active Snuffing leg is a work post (the Hungry Ox / Bellstand).
    let leg = active_leg(&round.people[&id].legs, Office::Snuffing, Weekday::Bellday)
        .expect("the tavern works at the Snuffing");
    assert!(!leg.is_home, "the tavern stays open past curfew");
}

#[test]
fn the_anchoress_is_never_marched_home_at_curfew() {
    // Dame Aldith (aq7ld) is bricked into her cell; her route override must carry
    // curfew_exempt so the curfew rung never walks her out to her nominal home.
    let nav = nav();
    let id = ActorId::from_raw("aq7ld");
    let mut world = base_world();
    world.add_character(person("aq7ld", Vec3::new(0.0, WALK_Y, 95.0), Some("anchoress"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    let person = round.people.get(&id).expect("the anchoress is enrolled");
    assert!(person.curfew_exempt, "her route override keeps curfew_exempt");
    let home = person.home;
    // At the Snuffing she is never sent to that home — the curfew rung is skipped.
    if let Decision::Travel(target) = decide(&round, &world, &nav, &id, 0, Office::Snuffing, Weekday::Bellday) {
        assert!(home.is_none_or(|home| target.distance(home) > 1.0), "she must not be walked home");
    }
}

/// Rung 2 precedes rung 5: a *parched* drawer goes to the well even at the
/// Snuffing, rather than being sent to bed thirsty (`03_the_ladder.md` §4).
#[test]
fn a_parched_drawer_goes_to_the_well_even_at_curfew() {
    let nav = nav();
    let ford = nav.place("Ford Well").expect("Ford Well").node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    world.add_character(person("keeper", curb, Some("mason"), Significance::Ambient));
    // A housed servant (a real homes.json id) who is also a water drawer.
    let servant = ActorId::from_raw("a2gpk");
    world.add_character(person("a2gpk", Vec3::new(89.0, WALK_Y, 36.0), Some("domestic_servant"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let person = round.people.get(&servant).expect("enrolled");
    assert!(person.home.is_some(), "a2gpk is housed");
    assert!(person.source.is_some(), "and bound to a staffed well");

    world.characters.get_mut(&servant).unwrap().state.needs.thirst = 0.0;
    // Deep in curfew, parched wins over going home.
    match decide(&round, &world, &nav, &servant, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::ApproachWell => {}
        other => panic!("a parched drawer heads for the well even at curfew, got {other:?}"),
    }
}

// --------------------------------------------------------------------------- //
// The census
// --------------------------------------------------------------------------- //

#[test]
fn the_census_counts_workers_at_their_post() {
    let nav = nav();
    let clock = clock_at(Office::Kindling);
    // Where a mason's day begins — resolve the workplace the way seed does.
    let coswalds = nav.place("Coswald's Yard").expect("Coswald's Yard is a nav place").node;
    let post = nav.node_point(coswalds);

    let mut world = base_world();
    for n in 0..3 {
        // Majors, so they are enrolled rather than pinned as well-keepers.
        world.add_character(person(&format!("mason{n}"), post, Some("mason"), Significance::Major));
    }
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let census = round.census(&world, &clock, 0.0);
    assert_eq!(census.total, 3);
    assert_eq!(census.at_post, 3, "all three stand at their post at the Kindling");
    assert_eq!(census.by_place.get("Coswald's Yard").copied(), Some(3));
    assert_eq!(census.walking, 0);
}

// --------------------------------------------------------------------------- //
// The water round (M3), preserved
// --------------------------------------------------------------------------- //

/// The vertical slice: a parched servant walks to the well, queues, draws (thirst
/// refilled, the windlass heard as a nudge-free world sound), then heads home.
#[test]
fn a_parched_servant_walks_to_the_well_draws_and_goes_home() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let ford = nav.place("Ford Well").expect("Ford Well is a nav place").node;
    let curb = nav.node_point(ford);
    let hop = nav.adjacency()[ford].first().expect("the well node has a neighbour").to;
    let home = nav.node_point(hop);

    let mut world = base_world();
    world.add_character(person("keeper", curb, Some("mason"), Significance::Ambient));
    world.add_character(person("servant", home, Some(HOUSEHOLD_OCCUPATIONS[0]), Significance::Ambient));

    let mut round = Round::new();
    let diagnostics = round.seed(&mut world, &nav, 0.0, &clock);
    assert!(
        round.sources().iter().any(|source| source.name == "Ford Well" && source.keeper.is_some()),
        "Ford Well was staffed: {diagnostics:?}"
    );
    assert_eq!(world.characters[&ActorId::from_raw("keeper")].position_m(), curb);

    let servant = ActorId::from_raw("servant");
    world.characters.get_mut(&servant).unwrap().state.needs.thirst = 0.0;

    let dt = 0.2;
    let mut now = 0.0;
    let mut drew = false;
    let mut drew_at_max = false;
    let mut windlass_events = 0;
    let mut remembered = false;
    let mut went_home = false;

    for _ in 0..3000 {
        now += dt;
        beat(&mut round, &mut world, &nav, &clock, now, dt);
        for event in world.drain_events() {
            if event.event_type == EventType::Sound && event.sound_id.as_deref() == Some("draw_water") {
                windlass_events += 1;
                assert!(event.actor_id.is_none(), "the windlass is a world sound, never attributed");
                assert!(event.witness_ids.is_empty(), "a world sound has no witnesses to nudge");
            }
        }
        if round.is_drawing_at("Ford Well") {
            drew = true;
        }
        if world.characters[&servant].recent_history().iter().any(|line| line.contains("drew water")) {
            remembered = true;
        }
        if drew && world.characters[&servant].needs().thirst >= THIRST_MAX - 1.0 {
            drew_at_max = true;
        }
        // Home again: back within a stride of where they started, after drawing.
        // (The daily-round rung may set off again afterwards; reaching home is
        // what proves the water errand walked them back.)
        if drew_at_max
            && world.characters[&servant].position_m().distance(home) < 2.0
            && now > 5.0
        {
            went_home = true;
            break;
        }
    }

    assert!(drew, "the servant reached the front of the queue and drew");
    assert!(windlass_events > 0, "the well's windlass was emitted as a world sound");
    assert!(remembered, "the drawer remembers drawing, so they can be asked about it");
    assert!(drew_at_max, "the draw refilled the servant's thirst");
    assert!(went_home, "the servant walked home again after drawing");
}

/// Household vessels go before trade vessels in an ordinary queue, arrival order
/// preserved within each class (`lore/wells_and_water.md`).
#[test]
fn household_vessels_queue_ahead_of_trade_vessels() {
    let mut round = Round::default();
    round.sources.push(WaterSource {
        name: "Test Well".into(),
        draw_point: Vec3::new(0.0, WALK_Y, 0.0),
        draw_sound: "draw_water",
        keeper: Some(ActorId::from_raw("k")),
        queue: Vec::new(),
        serving: None,
        keeper_next_sound: 0.0,
    });
    let townsperson = |household: bool| Townsperson {
        home: None,
        base: Vec3::ZERO,
        legs: Vec::new(),
        leash_m: DEFAULT_ROUND_LEASH_M,
        curfew_exempt: false,
        source: Some(0),
        is_household: household,
        phase: Phase::Idle,
        next_decision: 0.0,
        epoch: 0,
    };
    for (id, household) in [("trade_a", false), ("house_a", true), ("trade_b", false), ("house_b", true)] {
        round.people.insert(ActorId::from_raw(id), townsperson(household));
        enqueue(&mut round, 0, ActorId::from_raw(id));
    }
    let order: Vec<&str> = round.sources[0].queue.iter().map(ActorId::as_str).collect();
    assert_eq!(order, ["house_a", "house_b", "trade_a", "trade_b"]);
}

/// Thirst falls by the game clock, and the debug time-scale speeds it up.
#[test]
fn thirst_decays_by_the_game_clock() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let ford = nav.place("Ford Well").unwrap().node;
    let mut world = base_world();
    world.add_character(person("keeper", nav.node_point(ford), Some("mason"), Significance::Ambient));
    world.add_character(person(
        "servant",
        nav.node_point(nav.adjacency()[ford][0].to),
        Some(HOUSEHOLD_OCCUPATIONS[0]),
        Significance::Ambient,
    ));

    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let servant = ActorId::from_raw("servant");
    world.characters.get_mut(&servant).unwrap().state.needs.thirst = THIRST_MAX;

    let one_game_hour = 3600.0 / 24.0;
    tick(&mut round, &mut world, &nav, &clock, one_game_hour, &player(), None);
    let expected = THIRST_MAX - 3600.0 * crate::THIRST_DECAY_PER_GAME_SECOND;
    let thirst = world.characters[&servant].needs().thirst;
    assert!((thirst - expected).abs() < 1.0, "thirst {thirst} decayed to ~{expected} over one game hour");
}

/// A well keeper stands at the curb and never walks off on a round of their own.
#[test]
fn a_keeper_never_walks() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let ford = nav.place("Ford Well").unwrap().node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    // A lone ambient at the curb becomes Ford Well's keeper (pinned, not enrolled).
    world.add_character(person("stranger", curb, Some("mason"), Significance::Ambient));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    assert!(
        round.sources().iter().any(|source| source.keeper.as_ref().map(ActorId::as_str) == Some("stranger")),
        "the ambient at the curb keeps the well"
    );
    for step in 1..80 {
        beat(&mut round, &mut world, &nav, &clock, step as f64 * 0.5, 0.5);
    }
    assert!(
        world.characters[&ActorId::from_raw("stranger")].state.movement.is_none(),
        "a keeper never walks"
    );
    assert!(
        round.sources().iter().all(|source| source.queue.is_empty()),
        "no drawer, no queue"
    );
}

/// The deterministic decision hash gives the same city every run.
#[test]
fn the_decision_hash_is_stable() {
    let id = ActorId::from_raw("servant");
    assert_eq!(hash01("round_decision", &id, 3), hash01("round_decision", &id, 3));
    assert_ne!(hash01("round_decision", &id, 3), hash01("round_decision", &id, 4));
    for epoch in 0..64 {
        let jitter = decision_jitter(&id, epoch);
        assert!((LADDER_DECISION_MIN_SECONDS..=LADDER_DECISION_MAX_SECONDS).contains(&jitter));
    }
}
