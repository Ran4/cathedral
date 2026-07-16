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
/// Day 0 is a **Bellday**.
fn clock_at(office: Office) -> WorldClock {
    WorldClock::new(3600.0, office, 0, 0.05)
}

/// As [`clock_at`], but on absolute day `day` — day 1 is an ordinary Second,
/// day 2 a Highmarket, day 5 a Lowmarket ([`Weekday::of_day`]).
fn clock_on(office: Office, day: i64) -> WorldClock {
    WorldClock::new(3600.0, office, day, 0.05)
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
    tick(round, world, nav, clock, now, &player(), &BTreeSet::new());
}

/// A warm-exchange set of one, for driving [`tick`] mid-conversation.
fn warm(id: &ActorId) -> BTreeSet<ActorId> {
    BTreeSet::from([id.clone()])
}

/// [`decide`] without the pressure line, for the rung tests that only care
/// where the ladder points.
fn decide_only(
    round: &Round,
    world: &World,
    nav: &NavData,
    id: &ActorId,
    epoch: u64,
    office: Office,
    weekday: Weekday,
) -> Decision {
    decide(round, world, nav, id, epoch, office, weekday).0
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

    // `homes.json` is baked by `scripts/bake_homes.py`: every sheet under
    // `lore/characters` is bound to a residential door except the player and
    // anyone whose circumstances say they have no such bed (the bake script's
    // skip set). Deriving the expected ids from the lore instead of pinning a
    // count makes a stale bake fail here with the ids that drifted.
    let bedless = ["pauper", "unhoused", "insecure_lodging", "enclosed_religious"];
    let characters_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lore/characters");
    let mut expected_housed = BTreeSet::new();
    let mut stack = vec![characters_dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("lore/characters is readable") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let sheet: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&path).expect("character sheet is readable"),
            )
            .expect("character sheet parses");
            let id = sheet["id"].as_str().expect("character sheet has an id");
            let has_bed = !sheet["circumstances"]
                .as_array()
                .is_some_and(|c| c.iter().any(|c| bedless.contains(&c.as_str().unwrap_or(""))));
            if id != "player" && has_bed {
                expected_housed.insert(id.to_owned());
            }
        }
    }
    let housed: BTreeSet<String> = homes.homes.keys().cloned().collect();
    let unhoused: Vec<&String> = expected_housed.difference(&housed).collect();
    let overhoused: Vec<&String> = housed.difference(&expected_housed).collect();
    assert!(
        unhoused.is_empty() && overhoused.is_empty(),
        "homes.json is out of step with lore/characters — re-run scripts/bake_homes.py \
         (missing a home: {unhoused:?}; housed but bedless: {overhoused:?})"
    );
    assert!(
        !homes.homes.contains_key("aq7ld"),
        "Dame Aldith is bricked into her cell, never bound to a residential door"
    );
    assert_eq!(rounds.workplaces.len(), 65, "every occupation has a workplace list");
    assert_eq!(rounds.occupations.len(), 65, "every occupation has an archetype");

    // All twenty dramatis personae `route:` lines are transcribed, joined to the
    // right 5-char sheet ids — `04_the_round.md` §2(a): "assert that all twenty
    // resolve — a silent mis-join would give the Praelucent a fuller's day."
    let expected_majors: BTreeSet<&str> = [
        "ak3vd", "a9prs", "b4hst", "cj9sp", "dv8ll", "fg2sh", "cf2rr", "fl5cp", "fc9rn",
        "amt4p", "hj6br", "em3rl", "he3nd", "aq7ld", "ax5nf", "gw4ld", "az2sm", "gr8tp",
        "et7rd", "cg6ud",
    ]
    .into_iter()
    .collect();
    let authored: BTreeSet<&str> = rounds.routes.keys().map(String::as_str).collect();
    assert_eq!(
        authored, expected_majors,
        "exactly the twenty dramatis personae carry authored route overrides"
    );

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
// Market days and Bellday — the week moves the crowd (`04_the_round.md` §5)
// --------------------------------------------------------------------------- //

/// The lore names FOUR market squares: Highmarket "chiefly at the Wickmarket
/// and Coswald's Yard", Lowmarket "at the Tallage and Maren's Green"
/// (`lore/core_lore/trade_and_daily_life.md`). The two market-trader archetypes
/// split the trades across the pairs.
#[test]
fn market_day_legs_split_the_traders_across_the_four_squares() {
    let nav = nav();
    let mut world = base_world();
    // A chartered-goods trader (`market_trader`) and a provisions trader
    // (`market_trader_green`) — Majors, so neither is drafted as a well keeper.
    // Spawned beside their own trade grounds, so the workplace binding (nearest
    // candidate to base) picks the Reach and the Moorings, not a market square.
    world.add_character(person("draper_a", Vec3::new(120.0, WALK_Y, 260.0), Some("draper"), Significance::Major));
    world.add_character(person("fish_a", Vec3::new(-366.0, WALK_Y, -406.0), Some("fish_trader"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let leg_label = |id: &str, weekday: Weekday| {
        active_leg(&round.people[&ActorId::from_raw(id)].legs, Office::HighWick, weekday)
            .expect("a trader has a daytime leg")
            .label
            .clone()
    };
    // The Wickmarket/Tallage pair.
    assert_eq!(leg_label("draper_a", Weekday::Highmarket), "The Wickmarket");
    assert_eq!(leg_label("draper_a", Weekday::Lowmarket), "The Tallage");
    // The Coswald's Yard/Maren's Green pair the bug report found missing.
    assert_eq!(leg_label("fish_a", Weekday::Highmarket), "Coswald's Yard");
    assert_eq!(leg_label("fish_a", Weekday::Lowmarket), "Maren's Green");
    // On an ordinary day both keep their own workplace, not a market square.
    for id in ["draper_a", "fish_a"] {
        let ordinary = leg_label(id, Weekday::Fourth);
        for square in ["The Wickmarket", "The Tallage", "Coswald's Yard", "Maren's Green"] {
            assert_ne!(ordinary, square, "{id} works their own post on a Fourth");
        }
    }
}

/// Census-style: on each market day the right squares rise above their
/// ordinary-day baseline — including Coswald's Yard and Maren's Green, which
/// the bug report measured stuck at baseline. Four traders stand in the four
/// squares; only on the square's market day does the census count them there.
#[test]
fn each_market_day_raises_its_squares_above_the_ordinary_baseline() {
    let nav = nav();
    let square = |name: &str| nav.node_point(nav.place(name).expect("a market square is a nav place").node);
    let wickmarket = square("The Wickmarket");
    let tallage = square("The Tallage");
    let coswalds = square("Coswald's Yard");
    let marens = square("Maren's Green");

    let mut world = base_world();
    // Occupations chosen so nobody's *workplace* is the square they stand in —
    // the ordinary-day baseline for all four squares is then exactly zero.
    world.add_character(person("draper_hm", wickmarket, Some("draper"), Significance::Major));
    world.add_character(person("baker_lm", tallage, Some("baker"), Significance::Major));
    world.add_character(person("fish_hm", coswalds, Some("fish_trader"), Significance::Major));
    world.add_character(person("butcher_lm", marens, Some("butcher"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let occupancy = |round: &Round, day: i64| {
        let clock = clock_on(Office::HighWick, day);
        let census = round.census(&world, &clock, 0.0);
        [
            census.by_place.get("The Wickmarket").copied().unwrap_or(0),
            census.by_place.get("The Tallage").copied().unwrap_or(0),
            census.by_place.get("Coswald's Yard").copied().unwrap_or(0),
            census.by_place.get("Maren's Green").copied().unwrap_or(0),
        ]
    };

    // Day 3, a Fourth: nobody's leg points at a square — the baseline.
    assert_eq!(occupancy(&round, 3), [0, 0, 0, 0], "ordinary-day baseline");
    // Day 2, Highmarket: BOTH of its squares rise above the baseline.
    assert_eq!(occupancy(&round, 2), [1, 0, 1, 0], "Highmarket fills the Wickmarket AND Coswald's Yard");
    // Day 5, Lowmarket: BOTH of its squares rise above the baseline.
    assert_eq!(occupancy(&round, 5), [0, 1, 0, 1], "Lowmarket fills the Tallage AND Maren's Green");
}

/// Bellday closes the trades and fills the nave (`04_the_round.md` §5): the
/// generic trades lie in at the Kindling instead of opening the workshop, pray
/// at The Lanthorn from Dayspring through the Waning, and the census sees the
/// nave fill. The night trades keep their counters and the wharf keeps its
/// before-dawn work (Wyn Alder's canon: "the Moorings yard before dawn; ...
/// the nave on Bellday").
#[test]
fn bellday_closes_the_trades_and_fills_the_nave() {
    let nav = nav();
    let lanthorn = nav.node_point(nav.place("The Lanthorn").expect("the nave is a nav place").node);
    let mut world = base_world();
    // `a2gpk` is a real homes.json id, so the housed Kindling lie-in resolves.
    world.add_character(person("a2gpk", lanthorn, Some("baker"), Significance::Major));
    world.add_character(person("mason_b", lanthorn, Some("mason"), Significance::Major));
    world.add_character(person("boat_b", Vec3::new(0.0, WALK_Y, 95.0), Some("boatworker"), Significance::Major));
    world.add_character(person("tap_b", Vec3::new(0.0, WALK_Y, 95.0), Some("tavern_worker"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let leg = |id: &str, office: Office| {
        active_leg(&round.people[&ActorId::from_raw(id)].legs, office, Weekday::Bellday)
            .expect("a Bellday leg is active")
            .clone()
    };
    // The housed trader does not open the workshop before light: home, idle.
    let lie_in = leg("a2gpk", Office::Kindling);
    assert!(lie_in.is_home, "on Bellday the workshop stays shut at the Kindling");
    assert_eq!(lie_in.doing, Arrival::Idle);
    // From Dayspring through the Waning, trader and day worker fill the nave.
    for office in [Office::Dayspring, Office::HighWick, Office::Waning] {
        for id in ["a2gpk", "mason_b"] {
            let leg = leg(id, office);
            assert_eq!(leg.label, "The Lanthorn", "{id} prays in the nave at {office:?}");
            assert_eq!(leg.doing, Arrival::Pray);
        }
    }
    // The wharf works before dawn as ever, then joins the nave at Dayspring.
    assert_ne!(leg("boat_b", Office::Kindling).label, "The Lanthorn");
    assert!(!leg("boat_b", Office::Kindling).is_home, "the moorings open before light even on Bellday");
    assert_eq!(leg("boat_b", Office::Dayspring).label, "The Lanthorn");
    // A night trade keeps its counter: no Bellday leg drags the tavern to the nave.
    assert_ne!(leg("tap_b", Office::Dayspring).label, "The Lanthorn");
    // And on an ordinary day the same trader is at their own post, not the nave.
    assert_ne!(
        active_leg(&round.people[&ActorId::from_raw("a2gpk")].legs, Office::Dayspring, Weekday::Second)
            .expect("an ordinary Dayspring post")
            .label,
        "The Lanthorn"
    );

    // Census: the two standing in the nave count there on Bellday (day 0)...
    let bellday = round.census(&world, &clock_on(Office::HighWick, 0), 0.0);
    assert_eq!(bellday.by_place.get("The Lanthorn").copied(), Some(2), "the nave fills on Bellday");
    assert_eq!(bellday.by_place.get("The Wickmarket").copied(), None, "the generic workshop stays closed");
    // ...and on an ordinary day the same spot censuses as nobody's post.
    let ordinary = round.census(&world, &clock_on(Office::HighWick, 1), 0.0);
    assert_eq!(ordinary.by_place.get("The Lanthorn").copied(), None);
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
    match decide_only(&round, &world, &nav, &id, 0, Office::Kindling, Weekday::Bellday) {
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
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &warm(&id));
    assert!(
        !world.characters[&id].is_walking(),
        "nobody sets off for their post mid-conversation"
    );

    // The exchange goes cold: the same cadence now sends him to his post.
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &BTreeSet::new());
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

/// The same courtesy between two NPCs: while their exchange is warm the ladder
/// walks neither of them off to their post, and when it lapses both errands
/// resume on their own.
#[test]
fn a_warm_npc_exchange_holds_both_parties_until_it_lapses() {
    let nav = nav();
    let clock = clock_at(Office::Kindling);
    let a = ActorId::from_raw("mason_a");
    let b = ActorId::from_raw("mason_b");
    let mut world = base_world();
    // Two masons at the forecourt, away from Coswald's Yard — Majors, so
    // neither is drafted as a well keeper. Both owe the round rung a walk.
    world.add_character(person("mason_a", Vec3::new(0.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    world.add_character(person("mason_b", Vec3::new(1.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let hold: BTreeSet<ActorId> = BTreeSet::from([a.clone(), b.clone()]);
    let due = round.people[&a].next_decision.max(round.people[&b].next_decision) + 1.0;

    // Both cadences come due mid-exchange: neither sets off.
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &hold);
    assert!(!world.characters[&a].is_walking(), "the speaker's errand waits too");
    assert!(!world.characters[&b].is_walking(), "and the listener's");

    // The exchange goes cold: the same cadences send both to their post.
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &BTreeSet::new());
    assert!(world.characters[&a].is_walking(), "the round resumes for one");
    assert!(world.characters[&b].is_walking(), "and for the other");
}

/// Rung 5 outranks the chat — but not mid-sentence: the first pressing decision
/// under a hold lands as a `system:` percept (the one excuse-yourself turn),
/// the next one walks the body regardless, and a parting line no longer stops
/// the released walker.
#[test]
fn curfew_breaks_a_warm_hold_after_one_excuse_turn() {
    let nav = nav();
    let clock = clock_at(Office::Snuffing);
    let id = ActorId::from_raw("b4hst");
    let mut world = base_world();
    world.add_character(person("b4hst", Vec3::new(0.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let hold = warm(&id);

    // The first due cadence at the Snuffing: no marching off mid-sentence —
    // the pressure is injected on the self-correction channel instead.
    let due = round.people[&id].next_decision + 1.0;
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &hold);
    assert!(!world.characters[&id].is_walking(), "the excuse turn comes before the walk");
    assert!(
        world.characters[&id]
            .inbox()
            .iter()
            .any(|line| line.starts_with("system:") && line.contains("night is falling")),
        "the curfew pressure lands as a system: percept"
    );

    // The next cadence releases the hold regardless of what was said.
    let again = round.people[&id].next_decision + 1.0;
    tick(&mut round, &mut world, &nav, &clock, again, &player(), &hold);
    assert!(world.characters[&id].is_walking(), "urgency beats chat after the one excuse turn");
    assert_eq!(round.people[&id].phase, Phase::Travelling);

    // A parting line must not stop the released walker on the doorstep of night.
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(world.characters[&id].is_walking(), "the excused walker keeps going");
    assert_eq!(round.people[&id].phase, Phase::Travelling);

    // Once the exchange lapses, the excuse is handed back for the next one.
    tick(&mut round, &mut world, &nav, &clock, again, &player(), &BTreeSet::new());
    assert!(!round.people[&id].excused, "a lapsed hold resets the excuse");
}

/// Only curfew (5) and parched (2) press hard enough to break a conversation
/// hold; merely thirsty (6) — and everything below — defers to the exchange.
#[test]
fn only_the_pressing_rungs_carry_a_pressure_line() {
    let (round, mut world, nav, servant) = seed_parched_servant();
    // Parched at the Kindling: pressing.
    let (decision, pressure) =
        decide(&round, &world, &nav, &servant, 0, Office::Kindling, Weekday::Bellday);
    assert!(matches!(decision, Decision::ApproachWell));
    assert_eq!(pressure, Some(PARCHED_PRESSURE));
    // Housed at the Snuffing: pressing.
    let (decision, pressure) =
        decide(&round, &world, &nav, &servant, 0, Office::Snuffing, Weekday::Bellday);
    assert!(matches!(decision, Decision::Travel(_)));
    assert_eq!(pressure, Some(CURFEW_PRESSURE));
    // Merely thirsty: the well can wait out the conversation.
    world.characters.get_mut(&servant).unwrap().state.needs.thirst = THIRST_PARCHED + 1.0;
    let (decision, pressure) =
        decide(&round, &world, &nav, &servant, 0, Office::Kindling, Weekday::Bellday);
    assert!(matches!(decision, Decision::ApproachWell));
    assert_eq!(pressure, None, "rung 6 defers to a conversation");
}

#[test]
fn curfew_sends_the_housed_home_at_the_snuffing() {
    let (round, world, nav, id) = seed_hamel();
    let home = round.people[&id].home.expect("housed");
    match decide_only(&round, &world, &nav, &id, 0, Office::Snuffing, Weekday::Bellday) {
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
    match decide_only(&round, &world, &nav, &id, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::Travel(_) | Decision::Stay | Decision::Wander(_) => {}
        Decision::ApproachWell => panic!("a tavern worker draws no water here"),
        Decision::TravelIntent(_) => panic!("no go_to intent was issued"),
    }
    // And their active Snuffing leg is a work post (the Hungry Ox / Bellstand).
    let leg = active_leg(&round.people[&id].legs, Office::Snuffing, Weekday::Bellday)
        .expect("the tavern works at the Snuffing");
    assert!(!leg.is_home, "the tavern stays open past curfew");
}

#[test]
fn the_anchoress_is_never_marched_home_at_curfew() {
    // Dame Aldith (aq7ld) is bricked into her cell; her route override must carry
    // curfew_exempt, and the bake must give her no house — so the curfew rung has
    // nowhere to march her (`04_the_round.md` §1: zero legs, `route: none`).
    let nav = nav();
    let id = ActorId::from_raw("aq7ld");
    let mut world = base_world();
    world.add_character(person("aq7ld", Vec3::new(0.0, WALK_Y, 95.0), Some("anchoress"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    let person = round.people.get(&id).expect("the anchoress is enrolled");
    assert!(person.curfew_exempt, "her route override keeps curfew_exempt");
    assert!(person.home.is_none(), "the anchorhold is a cell, not a homes.json house");
    assert!(person.legs.is_empty(), "her Round has zero legs (`04_the_round.md` §1)");
    assert!(person.source.is_none(), "she is no water drawer; thirst never moves her");
    // At the Snuffing the ladder leaves her exactly where she stands.
    match decide_only(&round, &world, &nav, &id, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::Stay => {}
        other => panic!("the anchoress stands at curfew, got {other:?}"),
    }
}

/// Dame Aldith is immovable: bricked into the Ilvane anchorhold, `route: none`
/// (`04_the_round.md` §1 — "Her Round has zero legs and it works"). Across a
/// full simulated day-night cycle — every office, curfew included — no rung of
/// the ladder may move her a single step off her authored spawn: no leg, no
/// curfew march, no thirst errand, no social drift, no wander (her leash is 0).
#[test]
fn the_anchoress_never_moves_through_a_full_day() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("aq7ld");
    // Her authored spawn (lore/characters/anchoress/aq7ld_aldith.json): the squint.
    let spawn = Vec3::new(194.5, WALK_Y, -92.0);
    let mut world = base_world();
    world.add_character(person("aq7ld", spawn, Some("anchoress"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    // One game day per real hour: 1800 beats of 2 s span the whole cycle —
    // Dayspring through the Snuffing and the Watch and round to morning.
    let mut now = 0.0;
    for _ in 0..1800 {
        now += 2.0;
        beat(&mut round, &mut world, &nav, &clock, now, 2.0);
        let position = world.characters[&id].position_m();
        assert_eq!(
            position.distance(spawn),
            0.0,
            "the anchoress moved: at {:?} she stands at {position:?}, not her squint {spawn:?}",
            clock.at(now).office
        );
    }
    assert_eq!(round.people[&id].phase, Phase::Idle, "she never even sets off");
}

/// Curfew preempts a journey already under way: Hamel sets off for the masons'
/// lodge at the Waning, night falls mid-walk, and within one cadence the walk is
/// re-aimed home instead of him finishing the obsolete leg first
/// (`04_the_round.md` §5: the higher rungs all preempt the round). This drives
/// the full `tick` loop — a direct `decide` call on an idle actor cannot catch
/// the "committed to an errand" skip this guards against.
#[test]
fn curfew_preempts_a_journey_already_under_way() {
    let nav = nav();
    // One game day per real hour, opening at the Waning (15:00): the Snuffing
    // (21:00) rings 900 real seconds in.
    let clock = clock_at(Office::Waning);
    let id = ActorId::from_raw("b4hst");
    let mut world = base_world();
    world.add_character(person("b4hst", Vec3::new(0.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let home = round.people[&id].home.expect("housed");
    let lodge = round.people[&id]
        .legs
        .iter()
        .find(|leg| leg.from == Office::Waning)
        .expect("the authored route has a Waning leg (the masons' lodge)")
        .at;

    // His cadence comes due while it is still the Waning: he sets off for the lodge.
    let mut now = round.people[&id].next_decision + 0.1;
    tick(&mut round, &mut world, &nav, &clock, now, &player(), &BTreeSet::new());
    assert_eq!(round.people[&id].phase, Phase::Travelling);
    assert_eq!(round.people[&id].travel_target, Some(lodge), "he is bound for the lodge");

    // A few strides along: genuinely mid-journey, nowhere near either anchor.
    for _ in 0..10 {
        world.step_movement(0.2, &nav);
    }
    assert!(world.characters[&id].is_walking(), "still on the way to the lodge");
    assert!(world.characters[&id].position_m().distance(lodge) > ROUND_ARRIVE_RADIUS_M);

    // The Snuffing has rung. Within one decision cadence the traveller must
    // divert home — not walk the rest of the way to the lodge first.
    now = 905.0;
    let mut diverted = false;
    for _ in 0..40 {
        now += 0.2;
        beat(&mut round, &mut world, &nav, &clock, now, 0.2);
        if round.people[&id].travel_target == Some(home) {
            diverted = true;
            break;
        }
        assert!(
            world.characters[&id].position_m().distance(lodge) > ROUND_ARRIVE_RADIUS_M,
            "he must not finish the obsolete lodge journey before turning home"
        );
    }
    assert!(diverted, "the traveller turned home at curfew instead of finishing the lodge leg");
    assert_eq!(round.people[&id].phase, Phase::Travelling);
    assert!(world.characters[&id].is_walking(), "he walks home rather than standing in the street");
}

/// Seed a housed, well-bound servant (a real `homes.json` id) plus a keeper at
/// the Ford Well curb, and return everything a curfew-vs-parched test needs.
fn seed_parched_servant() -> (Round, World, NavData, ActorId) {
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
    (round, world, nav, servant)
}

/// Rung 5 precedes rung 2: at curfew a *parched* housed drawer is sent home —
/// the well waits until morning (`07_milestones.md` M4: curfew → parched →
/// thirsty; "curfew empties the streets").
#[test]
fn a_parched_housed_drawer_is_still_sent_home_at_curfew() {
    let (round, world, nav, servant) = seed_parched_servant();
    let home = round.people[&servant].home.expect("housed");
    // Deep in curfew, home wins over the well; at the door, they stay in.
    match decide_only(&round, &world, &nav, &servant, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::Travel(target) => assert!(
            target.distance(home) < 1.0,
            "at the Snuffing a parched drawer heads home, not to {target:?}"
        ),
        Decision::Stay => panic!("away from home, they must walk there, not stand"),
        other => panic!("expected Travel home, got {other:?}"),
    }
    // And deeper still, at the Watch, the same.
    match decide_only(&round, &world, &nav, &servant, 0, Office::Watch, Weekday::Bellday) {
        Decision::Travel(_) => {}
        other => panic!("the Watch is still curfew; expected Travel home, got {other:?}"),
    }
}

/// Once the curfew lifts at the Kindling, the night's thirst sends the drawer
/// straight to the well.
#[test]
fn a_parched_drawer_heads_for_the_well_once_curfew_lifts() {
    let (round, world, nav, servant) = seed_parched_servant();
    match decide_only(&round, &world, &nav, &servant, 0, Office::Kindling, Weekday::Bellday) {
        Decision::ApproachWell => {}
        other => panic!("at the Kindling the parched drawer goes to the well, got {other:?}"),
    }
}

/// The curfew rung needs a home: a parched *homeless* drawer still draws at
/// night rather than dying of thirst in a doorway.
#[test]
fn a_parched_homeless_drawer_still_draws_at_night() {
    let nav = nav();
    let ford = nav.place("Ford Well").expect("Ford Well").node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    world.add_character(person("keeper", curb, Some("mason"), Significance::Ambient));
    // Not a homes.json id, so no home is bound — the curfew rung is skipped.
    let servant = ActorId::from_raw("servant_x");
    world.add_character(person(
        "servant_x",
        nav.node_point(nav.adjacency()[ford][0].to),
        Some(HOUSEHOLD_OCCUPATIONS[0]),
        Significance::Ambient,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let person = round.people.get(&servant).expect("enrolled");
    assert!(person.home.is_none(), "servant_x has no baked home");
    assert!(person.source.is_some(), "but is bound to a staffed well");

    world.characters.get_mut(&servant).unwrap().state.needs.thirst = 0.0;
    match decide_only(&round, &world, &nav, &servant, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::ApproachWell => {}
        other => panic!("a homeless parched drawer still draws at night, got {other:?}"),
    }
}

// --------------------------------------------------------------------------- //
// The census
// --------------------------------------------------------------------------- //

#[test]
fn the_census_counts_workers_at_their_post() {
    let nav = nav();
    // An ordinary working morning (day 1) — on Bellday the workshops are shut.
    let clock = clock_on(Office::Kindling, 1);
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
        travel_target: None,
        travel_for_intent: false,
        next_decision: 0.0,
        epoch: 0,
        excused: false,
    };
    for (id, household) in [("trade_a", false), ("house_a", true), ("trade_b", false), ("house_b", true)] {
        round.people.insert(ActorId::from_raw(id), townsperson(household));
        enqueue(&mut round, 0, ActorId::from_raw(id));
    }
    let order: Vec<&str> = round.sources[0].queue.iter().map(ActorId::as_str).collect();
    assert_eq!(order, ["house_a", "house_b", "trade_a", "trade_b"]);
}

/// A full vessel is delivered by kind: a household vessel goes home whatever
/// the round says, a trade vessel goes to the workshop the current leg names,
/// and at night the non-exempt deliver homeward so the committed return leg
/// never fights the curfew rung.
#[test]
fn a_full_vessel_is_delivered_by_kind() {
    let home = Vec3::new(0.0, WALK_Y, 0.0);
    let shop = Vec3::new(50.0, WALK_Y, 0.0);
    let drawer = |household: bool| Townsperson {
        home: Some(home),
        base: home,
        legs: vec![RoundLeg {
            from: Office::Kindling,
            at: shop,
            label: "shop".into(),
            doing: Arrival::Work,
            only_on: None,
            is_home: false,
        }],
        leash_m: DEFAULT_ROUND_LEASH_M,
        curfew_exempt: false,
        source: Some(0),
        is_household: household,
        phase: Phase::Drawing,
        travel_target: None,
        travel_for_intent: false,
        next_decision: 0.0,
        epoch: 0,
        excused: false,
    };
    // Household: water for the home, even while the round leg says the shop.
    assert_eq!(delivery_point(&drawer(true), Office::Dayspring, Weekday::Bellday), home);
    // Trade: water for the workshop the current leg names.
    assert_eq!(delivery_point(&drawer(false), Office::Dayspring, Weekday::Bellday), shop);
    // At night the non-exempt carry it home; the curfew rung agrees on arrival.
    assert_eq!(delivery_point(&drawer(false), Office::Snuffing, Weekday::Bellday), home);
}

/// A trade-vessel drawer carries the water back to their workshop, not home:
/// the return leg after a finished draw is routed to the active round leg's
/// anchor (`04_the_round.md` §8: the thirsty man "arrives late" at the
/// tenter-yards — he does not walk home first).
#[test]
fn a_trade_drawer_returns_to_their_workshop_not_home() {
    let nav = nav();
    // An ordinary working day (day 1) — on Bellday the "workshop" is the nave.
    let clock = clock_on(Office::Dayspring, 1);
    let ford = nav.place("Ford Well").expect("Ford Well").node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    world.add_character(person("keeper", curb, Some("mason"), Significance::Ambient));
    // A housed trade drawer (`a2gpk` is a real homes.json id), one hop from the curb.
    let fuller = ActorId::from_raw("a2gpk");
    world.add_character(person(
        "a2gpk",
        nav.node_point(nav.adjacency()[ford][0].to),
        Some(TRADE_OCCUPATIONS[0]),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let (home, workshop) = {
        let person = round.people.get(&fuller).expect("enrolled");
        assert!(!person.is_household, "a cloth worker draws with a trade vessel");
        assert!(person.source.is_some(), "and is bound to the staffed well");
        let home = person.home.expect("a2gpk is housed");
        let workshop = active_leg(&person.legs, Office::Dayspring, Weekday::Second)
            .expect("a day worker has a Dayspring post")
            .at;
        (home, workshop)
    };
    assert!(
        workshop.distance(home) > 2.0 * ROUND_ARRIVE_RADIUS_M,
        "home and workshop must be distinct places for this test to mean anything"
    );

    world.characters.get_mut(&fuller).unwrap().state.needs.thirst = 0.0;

    let dt = 0.2;
    let mut now = 0.0;
    let mut return_path_end = None;
    for _ in 0..3000 {
        now += dt;
        beat(&mut round, &mut world, &nav, &clock, now, dt);
        if round.people[&fuller].phase == Phase::Returning {
            return_path_end = world.characters[&fuller]
                .state
                .movement
                .as_ref()
                .and_then(|movement| movement.path.last().copied());
            break;
        }
    }
    let end = return_path_end.expect("the drawer finished a draw and set off with the vessel");
    assert!(
        world.characters[&fuller].needs().thirst >= THIRST_MAX - 1.0,
        "the draw refilled their thirst before the return leg"
    );
    assert!(
        end.distance(workshop) < 1.0,
        "the full trade vessel is carried to the workshop, not to {end:?}"
    );
    assert!(end.distance(home) > 1.0, "and certainly not home first");
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
    tick(&mut round, &mut world, &nav, &clock, one_game_hour, &player(), &BTreeSet::new());
    let expected = THIRST_MAX - 3600.0 * crate::THIRST_DECAY_PER_GAME_SECOND;
    let thirst = world.characters[&servant].needs().thirst;
    assert!((thirst - expected).abs() < 1.0, "thirst {thirst} decayed to ~{expected} over one game hour");
}

/// A well keeper is enrolled like anyone else, with the well as their one post:
/// through the working day they hold the curb (the wander leash is a stride or
/// two), so the well stays kept and the queue always has someone to form on.
#[test]
fn a_keeper_is_enrolled_and_holds_their_curb_by_day() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let ford = nav.place("Ford Well").unwrap().node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    // A lone ambient at the curb becomes Ford Well's keeper.
    world.add_character(person("stranger", curb, Some("mason"), Significance::Ambient));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let keeper = ActorId::from_raw("stranger");
    assert!(
        round.sources().iter().any(|source| source.keeper.as_ref() == Some(&keeper)),
        "the ambient at the curb keeps the well"
    );
    let person = round.people.get(&keeper).expect("the keeper is enrolled in the round");
    assert!(person.source.is_none(), "a keeper works the curb, never queues at it");
    let leg = active_leg(&person.legs, Office::Dayspring, Weekday::Bellday)
        .expect("the keeper's day is their well");
    assert_eq!(leg.label, "Ford Well");
    for step in 1..80 {
        beat(&mut round, &mut world, &nav, &clock, step as f64 * 0.5, 0.5);
    }
    assert!(
        world.characters[&keeper].position_m().distance(curb) <= CENSUS_POST_RADIUS_M,
        "through the working day the keeper stays at the curb"
    );
    assert!(
        round.sources().iter().all(|source| source.queue.is_empty()),
        "no drawer, no queue"
    );
}

/// A housed keeper mans the well by day and is sent home by the curfew rung at
/// the Snuffing like any other housed townsperson; at the Kindling the round
/// rung walks them back to their curb.
#[test]
fn a_housed_keeper_goes_home_at_curfew_and_returns_to_the_well_by_day() {
    let nav = nav();
    let ford = nav.place("Ford Well").unwrap().node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    // `a2gpk` is a real homes.json id: this keeper has a bed to go to.
    let keeper = ActorId::from_raw("a2gpk");
    world.add_character(person("a2gpk", curb, Some("mason"), Significance::Ambient));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    assert!(
        round.sources().iter().any(|source| source.keeper.as_ref() == Some(&keeper)),
        "the ambient at the curb keeps the well"
    );
    let home = round.people[&keeper].home.expect("a2gpk is housed");
    assert!(!round.people[&keeper].curfew_exempt, "keeping a well is not a night trade");

    // At the Snuffing the curfew rung sends the keeper home.
    match decide_only(&round, &world, &nav, &keeper, 0, Office::Snuffing, Weekday::Bellday) {
        Decision::Travel(target) => assert!(
            target.distance(home) < 1.0,
            "at the Snuffing the keeper heads home, not to {target:?}"
        ),
        other => panic!("expected Travel home, got {other:?}"),
    }

    // Morning: from their own doorstep, the round rung walks them back to the curb.
    world.characters.get_mut(&keeper).unwrap().state.position_m = home;
    match decide_only(&round, &world, &nav, &keeper, 0, Office::Kindling, Weekday::Bellday) {
        Decision::Travel(target) => assert!(
            target.distance(curb) < 1.0,
            "at the Kindling the keeper returns to the well, not to {target:?}"
        ),
        other => panic!("expected Travel to the curb, got {other:?}"),
    }
}

/// Nobody is left out of the round: the well keepers and the erstwhile M2 pacer
/// (`p0012`) are enrolled alongside everyone else, so the enrolment — and the
/// census built from it — covers the whole LLM cast (`07_milestones.md` M4:
/// "enrols the whole LLM cast").
#[test]
fn the_whole_cast_is_enrolled_including_keepers_and_the_old_pacer() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let ford = nav.place("Ford Well").unwrap().node;
    let curb = nav.node_point(ford);
    let mut world = base_world();
    world.add_character(person("stranger", curb, Some("mason"), Significance::Ambient));
    world.add_character(person("p0012", Vec3::new(42.5, WALK_Y, 142.5), Some("market_seller"), Significance::Minor));
    world.add_character(person("mason_a", Vec3::new(0.0, WALK_Y, 95.0), Some("mason"), Significance::Major));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    assert_eq!(round.enrolled(), 3, "keeper, pacer and worker are all enrolled");
    assert_eq!(round.census(&world, &clock, 0.0).total, 3, "and the census counts all three");

    // The pacer follows the ordinary market-trader round, not a scripted ping-pong.
    let pacer = ActorId::from_raw("p0012");
    assert!(
        world.characters[&pacer].state.movement.as_ref().is_none_or(|movement| movement.patrol.is_none()),
        "no permanent patrol is scripted onto p0012"
    );
    assert!(
        active_leg(&round.people[&pacer].legs, Office::Dayspring, Weekday::Second).is_some(),
        "p0012 has an ordinary working day"
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

// --------------------------------------------------------------------------- //
// M5 — go_to / stop / tell_way and the wayfinding registry
// --------------------------------------------------------------------------- //

use crate::{actions::apply_action, character::TravelIntent, error::ActionErrorCode};
use serde_json::json;

/// [`tick`], collecting the priority nudges the engine would forward.
fn tick_collect(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
) -> Vec<ActorId> {
    tick(round, world, nav, clock, now, &player(), &BTreeSet::new())
}

/// Walk the sim forward in half-second beats until `stop` says done, collecting
/// every nudge along the way. Panics past `max_beats` so a stuck walker fails
/// loudly.
fn beats_until(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    start: f64,
    max_beats: usize,
    mut done: impl FnMut(&World) -> bool,
) -> (f64, Vec<ActorId>) {
    let mut nudges = Vec::new();
    let mut now = start;
    for _ in 0..max_beats {
        now += 0.5;
        world.step_movement(0.5, nav);
        nudges.extend(tick_collect(round, world, nav, clock, now));
        if done(world) {
            return (now, nudges);
        }
    }
    panic!("still not done after {max_beats} beats");
}

/// Seeding hands everyone the coarse handles — the major squares and the eight
/// wards, the "legal first step" of every journey — plus the places of their
/// own ward and the homes of the people they know.
#[test]
fn seeding_hands_out_the_wayfinding_whitelist() {
    let nav = nav();
    let mut world = base_world();
    // a2gpk is housed by the real bake; the friend knows them.
    world.add_character(person("a2gpk", Vec3::new(0.0, WALK_Y, 95.0), Some("domestic_servant"), Significance::Minor));
    let mut friend = person("frnd1", Vec3::new(2.0, WALK_Y, 95.0), Some("mason"), Significance::Minor);
    friend.state.knows.insert(ActorId::from_raw("a2gpk"));
    world.add_character(friend);
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    assert!(!world.places.is_empty(), "the registry reached the world");
    let coarse: Vec<&str> = world.places.coarse().map(|entry| entry.name.as_str()).collect();
    assert!(coarse.contains(&"The Gradine") && coarse.contains(&"Reed Ward"), "{coarse:?}");

    let servant = &world.characters[&ActorId::from_raw("a2gpk")];
    for entry in world.places.coarse() {
        assert!(
            servant.state.places_known.contains(&entry.id),
            "everyone holds the coarse handle for {}",
            entry.name
        );
    }
    let home = world.places.home_of(&ActorId::from_raw("a2gpk")).expect("a2gpk is housed");
    assert_eq!(home.name, "A2GPK's house");
    assert!(servant.state.places_known.contains(&home.id), "you know your own house");
    // The Fabric-ward test lore puts both in Fabric Ward: its places are theirs.
    let ford_well = world.places.named("Ford Well").expect("Ford Well is baked");
    assert_eq!(ford_well.ward.as_deref(), Some("fabric"));
    assert!(servant.state.places_known.contains(&ford_well.id), "own-ward places are known");

    let friend = &world.characters[&ActorId::from_raw("frnd1")];
    assert!(
        friend.state.places_known.contains(&home.id),
        "knowing somebody means knowing the way to their door"
    );
    assert!(
        !servant.state.places_known.contains(
            &world.places.home_of(&ActorId::from_raw("frnd1")).map(|entry| entry.id.clone())
                .unwrap_or_else(|| PlaceId::from_raw("pl_none"))
        ),
        "the servant does not know the stranger's door"
    );
}

/// The full errand: `go_to` sets the intent, the ladder walks it, arrival is a
/// percept, and the walker is handed the same priority nudge an addressed say
/// gets.
#[test]
fn go_to_walks_there_and_arrival_is_a_percept_and_a_nudge() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("walkr");
    let start = nav.node_point(nav.place("Seraph statue").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("walkr", start, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let target = world.places.named("The Gradine").expect("baked").id.clone();
    let line = apply_action(&mut world, &id, "go_to", &json!({"place_id": target.as_str()})).unwrap();
    assert_eq!(line, "WALKR sets off for The Gradine");
    assert!(
        world.characters[&id].recent_history().iter().any(|line| line == "You set off for The Gradine."),
        "the walker remembers their own errand"
    );

    let gradine = world.places.named("The Gradine").unwrap().point;
    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.0, 2000, |world| {
        world.characters[&id].state.intent.is_none()
    });
    let walker = &world.characters[&id];
    assert!(
        walker.position_m().distance(gradine) <= PLACE_ARRIVE_RADIUS_M + 0.5,
        "the walker stands at the Gradine, not {:?}",
        walker.position_m()
    );
    assert!(
        walker.inbox().iter().any(|line| line == "You have arrived at The Gradine."),
        "arrival is a percept: {:?}",
        walker.inbox()
    );
    assert_eq!(nudges, std::slice::from_ref(&id), "arrival granted exactly one priority nudge");
}

/// A second `go_to` replaces the first silently, and `stop {}` abandons the
/// errand — halting the walk without any percept.
#[test]
fn a_second_go_to_replaces_and_stop_halts_the_walk() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("walkr");
    let start = nav.node_point(nav.place("Seraph statue").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("walkr", start, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let gradine = world.places.named("The Gradine").unwrap().id.clone();
    let lanthorn = world.places.named("The Lanthorn").unwrap().id.clone();
    apply_action(&mut world, &id, "go_to", &json!({"place_id": gradine.as_str()})).unwrap();
    apply_action(&mut world, &id, "go_to", &json!({"place_id": lanthorn.as_str()})).unwrap();
    match &world.characters[&id].state.intent {
        Some(TravelIntent { target: IntentTarget::Place { place_id, .. }, .. }) => {
            assert_eq!(place_id, &lanthorn, "the second go_to replaced the first")
        }
        other => panic!("expected a place intent, got {other:?}"),
    }

    // Let the ladder set the walk, then stop: the walk halts, no percept lands.
    let mut now = 0.0;
    for _ in 0..20 {
        now += 0.5;
        world.step_movement(0.5, &nav);
        tick_collect(&mut round, &mut world, &nav, &clock, now);
        if world.characters[&id].is_walking() {
            break;
        }
    }
    assert!(world.characters[&id].is_walking(), "the errand walk is under way");
    let inbox_before = world.characters[&id].inbox().len();
    apply_action(&mut world, &id, "stop", &json!({})).unwrap();
    now += 0.5;
    tick_collect(&mut round, &mut world, &nav, &clock, now);
    assert!(world.characters[&id].state.intent.is_none());
    assert!(!world.characters[&id].is_walking(), "stop halted the intent walk");
    assert_eq!(
        world.characters[&id].inbox().len(),
        inbox_before,
        "stop is self-initiated: no percept"
    );
}

/// An intent expires on its route-derived budget, and the lapse is a percept
/// plus the nudge — the honest ending an off-stage errand needs.
#[test]
fn an_expired_intent_lapses_with_a_percept_and_a_nudge() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("walkr");
    let start = nav.node_point(nav.place("Seraph statue").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("walkr", start, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let tallage = world.places.named("The Tallage").unwrap().id.clone();
    apply_action(&mut world, &id, "go_to", &json!({"place_id": tallage.as_str()})).unwrap();
    // Shrink the budget so the errand cannot possibly finish.
    world.characters.get_mut(&id).unwrap().state.intent.as_mut().unwrap().budget_seconds = 2.0;

    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.0, 40, |world| {
        world.characters[&id].state.intent.is_none()
    });
    assert!(
        world.characters[&id]
            .inbox()
            .iter()
            .any(|line| line == "Your errand to The Tallage lapsed before you arrived."),
        "lapse is a percept: {:?}",
        world.characters[&id].inbox()
    );
    assert_eq!(nudges, std::slice::from_ref(&id), "the lapse granted the nudge");
    assert!(!world.characters[&id].is_walking(), "the lapsed walk was halted");
}

/// Curfew — a pressing rung — preempts a live errand, and the preemption names
/// its cause: "The curfew turned you back…".
#[test]
fn curfew_preempts_an_errand_with_a_cause_percept() {
    let nav = nav();
    let id = ActorId::from_raw("a2gpk"); // housed by the real bake
    let start = nav.node_point(nav.place("Seraph statue").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("a2gpk", start, Some("domestic_servant"), Significance::Minor));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    assert!(round.people[&id].home.is_some(), "the bake houses a2gpk");

    let tallage = world.places.named("The Tallage").unwrap().id.clone();
    apply_action(&mut world, &id, "go_to", &json!({"place_id": tallage.as_str()})).unwrap();

    // The same wall clock, but read at the Snuffing: the curfew rung fires.
    let night = clock_at(Office::Snuffing);
    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &night, 0.0, 40, |world| {
        world.characters[&id].state.intent.is_none()
    });
    assert!(
        world.characters[&id]
            .inbox()
            .iter()
            .any(|line| line == "The curfew turned you back before you reached The Tallage."),
        "the preemption says why: {:?}",
        world.characters[&id].inbox()
    );
    assert!(nudges.contains(&id), "the preempted errand still granted its nudge");
}

/// `go_to {"person"}`: the follow tracks a visible target to conversation
/// distance; catching up is a percept and a nudge.
#[test]
fn a_followed_person_is_caught_at_conversation_distance() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let follower = ActorId::from_raw("follw");
    let target = ActorId::from_raw("targt");
    let gradine_node = nav.place("The Gradine").expect("baked").node;
    let start = nav.node_point(gradine_node);
    // The target stands a dozen metres up the street — on the walkable midline,
    // where a real townsperson (who only ever walks the graph) stands.
    let up_the_street = nav.node_point(nav.adjacency()[gradine_node][0].to);
    let target_at = start + (up_the_street - start).normalize() * 12.0;
    assert!(nav.is_walkable(target_at.x, target_at.z), "the street midline is walkable");
    let mut world = base_world();
    world.add_character(person("follw", start, None, Significance::Major));
    world.add_character(person("targt", target_at, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    apply_action(&mut world, &follower, "go_to", &json!({"person": "targt"})).unwrap();
    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.0, 200, |world| {
        world.characters[&follower].state.intent.is_none()
    });
    let gap = world.characters[&follower]
        .position_m()
        .distance(world.characters[&target].position_m());
    assert!(gap <= PERSON_ARRIVE_RADIUS_M + 0.5, "closed to conversation distance, gap {gap}");
    assert!(
        world.characters[&follower]
            .inbox()
            .iter()
            .any(|line| line == "You have caught up with a stranger (id targt)."),
        "{:?}",
        world.characters[&follower].inbox()
    );
    assert_eq!(nudges, std::slice::from_ref(&follower));
}

/// Losing sight of a followed target is a percept, and the intent degrades to
/// their last-seen position — a hoarded id is not a tracking device.
#[test]
fn losing_sight_degrades_the_follow_to_the_last_seen_spot() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let follower = ActorId::from_raw("follw");
    let target = ActorId::from_raw("targt");
    let start = nav.node_point(nav.place("The Gradine").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("follw", start, None, Significance::Major));
    let target_at = Vec3::new(start.x + 15.0, WALK_Y, start.z);
    world.add_character(person("targt", target_at, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    apply_action(&mut world, &follower, "go_to", &json!({"person": "targt"})).unwrap();
    // One tick to stamp the follow, then the target vanishes across the city.
    tick_collect(&mut round, &mut world, &nav, &clock, 0.5);
    world.characters.get_mut(&target).unwrap().state.position_m = Vec3::new(start.x + 300.0, WALK_Y, start.z);

    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.5, 200, |world| {
        world.characters[&follower].state.intent.is_none()
    });
    let inbox = world.characters[&follower].inbox();
    assert!(
        inbox.iter().any(|line| line == "You have lost sight of a stranger (id targt)."),
        "{inbox:?}"
    );
    assert!(
        inbox
            .iter()
            .any(|line| line == "You reach the spot where you last saw a stranger (id targt), but they are gone."),
        "{inbox:?}"
    );
    // The follower ended near where the target was last seen, not where they went.
    assert!(
        world.characters[&follower].position_m().distance(target_at) <= PLACE_ARRIVE_RADIUS_M + 0.5
    );
    assert_eq!(nudges, std::slice::from_ref(&follower));
}

/// A conversation never pins a `go_to`: leaving is the character's own
/// decision, made in the same reply as the goodbye — the walk starts within
/// the ladder cadence even while the exchange is warm (a 30 s statue after
/// "meet me at the Gradine" reads as broken, not polite). A fresh addressed
/// line still stops them for the answer, and the errand resumes on its own.
#[test]
fn a_conversation_does_not_pin_a_self_willed_errand() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("walkr");
    let start = nav.node_point(nav.place("Seraph statue").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("walkr", start, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let gradine = world.places.named("The Gradine").unwrap().id.clone();
    apply_action(&mut world, &id, "go_to", &json!({"place_id": gradine.as_str()})).unwrap();

    // Held in a warm exchange the whole time: the errand walks anyway, within
    // the ladder's jittered 1-6 s cadence — never the exchange's 30 s memory.
    let mut now = 0.0;
    while !world.characters[&id].is_walking() {
        now += 0.5;
        assert!(
            now <= LADDER_DECISION_MAX_SECONDS + 1.0,
            "the errand waited out the conversation instead of walking"
        );
        world.step_movement(0.5, &nav);
        tick(&mut round, &mut world, &nav, &clock, now, &player(), &warm(&id));
    }
    assert!(world.characters[&id].state.intent.is_some());

    // A fresh addressed line stops them mid-stride to answer...
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(!world.characters[&id].is_walking(), "they stop for the answer");
    assert!(world.characters[&id].state.intent.is_some(), "without dropping the errand");

    // ...and, still mid-conversation, the walk resumes on the next cadence.
    let resumed_by = now + LADDER_DECISION_MAX_SECONDS + 1.0;
    while !world.characters[&id].is_walking() {
        now += 0.5;
        assert!(now <= resumed_by, "the interrupted errand never resumed");
        world.step_movement(0.5, &nav);
        tick(&mut round, &mut world, &nav, &clock, now, &player(), &warm(&id));
    }
}

/// `tell_way` writes the handle into the receiver's set — the sheet is the
/// memory — and the whole ask-the-way exchange works end-to-end.
#[test]
fn tell_way_transfers_the_handle_and_go_to_accepts_it() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let teller = ActorId::from_raw("tellr");
    let asker = ActorId::from_raw("askr1");
    let start = nav.node_point(nav.place("The Gradine").expect("baked").node);
    let mut world = base_world();
    // The mason knows the masons' lodge (a Wallwright place); the fabric-ward
    // asker does not.
    let mut teller_character = person("tellr", start, None, Significance::Major);
    teller_character.sheet.lore = person("x", Vec3::ZERO, Some("mason"), Significance::Major).sheet.lore.clone();
    teller_character.sheet.lore.as_mut().unwrap().planning_ward = PlanningWard::Wallwright;
    world.add_character(teller_character);
    world.add_character(person("askr1", Vec3::new(start.x + 3.0, WALK_Y, start.z), None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let lodge = world.places.named("The masons' lodge").expect("baked").id.clone();
    assert!(world.characters[&teller].state.places_known.contains(&lodge));
    assert!(!world.characters[&asker].state.places_known.contains(&lodge));

    // The asker cannot walk there yet: the id is not theirs to use.
    let error = apply_action(&mut world, &asker, "go_to", &json!({"place_id": lodge.as_str()})).unwrap_err();
    assert_eq!(error.code, ActionErrorCode::UnknownPlace);

    // The teller shares the way; the asker gets one inbox line and the handle.
    let line = apply_action(
        &mut world,
        &teller,
        "tell_way",
        &json!({"person": "askr1", "place_id": lodge.as_str()}),
    )
    .unwrap();
    assert_eq!(line, "TELLR tells ASKR1 the way to The masons' lodge");
    assert!(world.characters[&asker].state.places_known.contains(&lodge));
    assert!(
        world.characters[&asker]
            .inbox()
            .iter()
            .any(|line| line == "A stranger (id tellr) told you the way to The masons' lodge."),
        "{:?}",
        world.characters[&asker].inbox()
    );
    assert!(
        world.characters[&teller]
            .recent_history()
            .iter()
            .any(|line| line == "You told a stranger (id askr1) the way to The masons' lodge."),
    );

    // And now the errand is legal.
    apply_action(&mut world, &asker, "go_to", &json!({"place_id": lodge.as_str()})).unwrap();
    assert!(world.characters[&asker].state.intent.is_some());
}

/// The verb validation walls: sight-gating on person targets, earshot on
/// tell_way, and the whitelist on both — the errors the model self-corrects on.
#[test]
fn go_to_and_tell_way_validation_walls() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let id = ActorId::from_raw("walkr");
    let start = nav.node_point(nav.place("The Gradine").expect("baked").node);
    let mut world = base_world();
    world.add_character(person("walkr", start, None, Significance::Major));
    world.add_character(person("farbd", Vec3::new(start.x + 50.0, WALK_Y, start.z), None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    for (args, code) in [
        (json!({}), ActionErrorCode::InvalidArguments),
        (json!({"place_id": "pl_x", "person": "farbd"}), ActionErrorCode::InvalidArguments),
        (json!({"place_id": "pl_no_such"}), ActionErrorCode::UnknownPlace),
        (json!({"person": "ghost"}), ActionErrorCode::UnknownTarget),
        (json!({"person": "walkr"}), ActionErrorCode::SelfTarget),
        // 50 m away: out of you_see, so not a legal follow target.
        (json!({"person": "farbd"}), ActionErrorCode::OutOfRange),
    ] {
        let error = apply_action(&mut world, &id, "go_to", &args).unwrap_err();
        assert_eq!(error.code, code, "go_to {args}");
    }
    assert!(world.characters[&id].state.intent.is_none(), "no failed go_to left an intent");

    let gradine = world.places.named("The Gradine").unwrap().id.clone();
    for (args, code) in [
        // The teller holds the id, but the target is beyond earshot.
        (json!({"person": "farbd", "place_id": gradine.as_str()}), ActionErrorCode::OutOfRange),
        // A handle the teller does not hold cannot be shared.
        (json!({"person": "farbd", "place_id": "pl_no_such"}), ActionErrorCode::UnknownPlace),
        (json!({"person": "walkr", "place_id": gradine.as_str()}), ActionErrorCode::SelfTarget),
    ] {
        let error = apply_action(&mut world, &id, "tell_way", &args).unwrap_err();
        assert_eq!(error.code, code, "tell_way {args}");
    }
}
