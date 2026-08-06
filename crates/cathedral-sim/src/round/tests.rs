//! Tests for the daily round (M4) and the water round it subsumes (M3).

use super::*;
use crate::{
    Offer, Office, WorldClock,
    character::{CharacterSheet, Control},
    event::EventType,
    lore::{LoreProfile, PlanningWard},
    sounds::SoundCatalog,
};
use std::collections::{BTreeMap, BTreeSet};

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
fn person(
    id: &str,
    position: Vec3,
    occupation: Option<&str>,
    significance: Significance,
) -> Character {
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
        home: None,
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
    });
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(id),
        name: id.to_uppercase(),
        control: Control::Llm,
        back_story: String::new(),
        location_description: String::new(),
        appearance: Default::default(),
        voice_key: None,
        position_m: position,
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore,
        presence: crate::Presence::InCity,
        presence_epoch: 0,
        economic_class: crate::EconomicClass::Resident,
    })
}

fn base_world() -> World {
    let mut world = World::new();
    world.sound_catalog = SoundCatalog::from_toml_str(CATALOG).expect("catalog loads");
    world
}

fn stock(kind: &str, quantity: u32) -> StockSpec {
    StockSpec {
        kind: kind.into(),
        metadata: BTreeMap::new(),
        quantity,
    }
}

/// Test-only boundary helper. Production code never resets purses mid-round;
/// this uses the same commitment-aware inventory primitive as road staging and
/// settlement instead of reaching into item quantities directly.
fn set_wallet(world: &mut World, actor: &ActorId, amount: u32) {
    world
        .settle_wallet_exact(actor, amount, &format!("test_wallet:{actor}:{amount}"))
        .expect("test purse is uncommitted");
}

/// One engine-style beat: walk the movers a slice, then run the round.
fn beat(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
    dt: f64,
) {
    world.step_movement(dt, nav, None);
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
    let road_members: BTreeSet<String> = rounds
        .road_parties
        .iter()
        .flat_map(|party| std::iter::once(&party.leader).chain(party.members.iter()))
        .map(ToString::to_string)
        .collect();

    // `homes.json` is baked by `scripts/bake_homes.py`: every sheet under
    // `lore/characters` is bound to a residential door except the player and
    // anyone whose circumstances say they have no such bed (the bake script's
    // skip set). Deriving the expected ids from the lore instead of pinning a
    // count makes a stale bake fail here with the ids that drifted.
    let bedless_circumstances = [
        "pauper",
        "unhoused",
        "insecure_lodging",
        "enclosed_religious",
    ];
    let characters_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lore/characters");
    let mut expected_housed = BTreeSet::new();
    let mut expected_bedless = BTreeSet::new();
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
            let has_bed = !sheet["circumstances"].as_array().is_some_and(|c| {
                c.iter()
                    .any(|c| bedless_circumstances.contains(&c.as_str().unwrap_or("")))
            });
            if id == "player" || road_members.contains(id) {
                continue;
            }
            if has_bed {
                expected_housed.insert(id.to_owned());
            } else {
                expected_bedless.insert(id.to_owned());
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

    // The prompt side of the same bake: every housed entry speaks its home,
    // every bedless character gets the explicit no-fixed-bed framing — so the
    // `**you**` line's `Home:` is never an id, a coordinate, or silence
    // (`features/npc_knows_where_it_lives__inject_home_into_prompt.md`).
    let baked_bedless: BTreeSet<String> = homes.bedless.keys().cloned().collect();
    assert_eq!(
        baked_bedless, expected_bedless,
        "homes.json bedless is out of step with lore/characters — re-run scripts/bake_homes.py"
    );
    for (id, entry) in &homes.homes {
        assert!(
            entry
                .place_description
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty()),
            "{id}'s home has no place_description — re-run scripts/bake_homes.py"
        );
    }
    for (id, entry) in &homes.bedless {
        assert!(
            !entry.place_description.trim().is_empty(),
            "{id}'s bedless entry has an empty place_description"
        );
    }
    assert!(
        homes.bedless["aq7ld"]
            .place_description
            .contains("anchorhold cell"),
        "the anchoress is bedless but not homeless: her framing is the cell"
    );
    assert_eq!(
        rounds.workplaces.len(),
        65,
        "every occupation has a workplace list"
    );
    assert_eq!(
        rounds.occupations.len(),
        65,
        "every occupation has an archetype"
    );

    // The original twenty dramatis personae plus the five M5 supply-chain
    // residents carry explicit routes, joined to the right 5-char sheet ids —
    // and, since law_and_order.md M2, the staffed law: three sergeant beats,
    // the notary's counter, and the five gate keepers.
    let expected_majors: BTreeSet<&str> = [
        "ak3vd", "a9prs", "b4hst", "cj9sp", "dv8ll", "fg2sh", "cf2rr", "fl5cp", "fc9rn", "amt4p",
        "hj6br", "em3rl", "he3nd", "aq7ld", "ax5nf", "gw4ld", "az2sm", "gr8tp", "et7rd", "cg6ud",
        "danqn", "davqn", "e1skl", "e7mil", "p008s", "p009x", "p009z", "p00a3", "fo6gl", "hrnsk",
        "p00a7", "p00a8", "p00ad", "p00ah",
        // M5b: the Stone House's keeper and its two guards. Narrowing three
        // postings by name rather than widening `workplaces`, which binds the
        // *nearest* candidate and would have gaoled the debt officer too.
        "p009w", "p009y", "p00a2",
    ]
    .into_iter()
    .collect();
    let authored: BTreeSet<&str> = rounds.routes.keys().map(String::as_str).collect();
    assert_eq!(
        authored, expected_majors,
        "exactly the authored principals carry route overrides"
    );

    // Every non-keyword destination resolves to a nav place/site or to the
    // one home-anchored worksite constructed by `Round::seed`.
    let check = |name: &str, whence: &str| {
        if name != "home" && name != "workplace" {
            let resolves = resolver.resolve(name).is_some()
                || (name == "Ansel Quern's common oven" && homes.homes.contains_key("danqn"));
            assert!(resolves, "{whence}: `{name}` does not resolve");
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

/// The three committed assets that pin **bare nav node indices** still resolve
/// to the world points they were baked against.
///
/// A change to the city's colliders runs the whole chain, in this order:
///
/// ```sh
/// cargo test -p cathedralbevy export_collision_footprints -- --ignored
/// uv run scripts/bake_navigation.py    # welds — and renumbers — the graph
/// uv run scripts/bake_places.py
/// uv run scripts/bake_homes.py
/// # then re-point assets/world/shelters.json by hand: it has no script
/// ```
///
/// The second step is the one that rots everything downstream. Re-welding the
/// street graph renumbers it wholesale — a 2026-07 re-bake took 472 street
/// nodes to 457, moving every index from 20 up — and `places.json`'s `node`,
/// `homes.json`'s `door_node` and `shelters.json`'s `route_node` are all bare
/// `usize` indices into that list. A stale one is still a *valid* index, so
/// nothing refuses to load: it simply means somewhere else, by up to 900 m.
/// The last time half this chain was run, the only evidence was four oblique
/// failures elsewhere in the suite ("lanthorn_nave has no walkable spread
/// point", a shelter binding a hearth across the ward, an inmate standing on
/// the open street graph) — none of which named the asset or the command.
/// Hence one test that does both.
#[test]
fn every_baked_nav_pin_still_resolves_to_the_point_it_was_baked_against() {
    let nav = nav();

    // ----------------------------------------------------------------- //
    // places.json — the wayfinding registry
    // ----------------------------------------------------------------- //
    // A place's `node` is a straight copy of the graph's own place of that
    // name (`bake_places.py`), so it must still resolve to that same point.
    // The metre of slack is a courtesy to a future bake that merely re-orders
    // coincident nodes; a renumbering throws the point across the city.
    const PLACE_TOLERANCE_M: f64 = 1.0;
    let places: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/world/places.json"))
            .expect("places.json parses");
    let baked = places["places"].as_array().expect("places.json has places");
    let baked_names: BTreeSet<&str> = baked
        .iter()
        .map(|place| place["name"].as_str().expect("a place has a name"))
        .collect();
    let graph_names: BTreeSet<&str> = nav.places().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        baked_names, graph_names,
        "places.json no longer names the same places as navigation.json — \
         re-run `uv run scripts/bake_places.py`"
    );
    for place in baked {
        let name = place["name"].as_str().expect("a place has a name");
        let node = place["node"].as_u64().expect("a place pins a node") as usize;
        assert!(
            node < nav.node_count(),
            "places.json pins node {node} for {name}, past the end of a \
             {}-node graph — re-run `uv run scripts/bake_places.py`",
            nav.node_count()
        );
        let pinned = nav.node_point(node);
        let in_the_graph = nav.node_point(nav.place(name).expect("named above").node);
        let drift = pinned.distance(in_the_graph);
        assert!(
            drift <= PLACE_TOLERANCE_M,
            "places.json is out of step with navigation.json — re-run \
             `uv run scripts/bake_places.py` ({name} pins node {node} at \
             {pinned:?}, but the graph puts {name} at {in_the_graph:?}, \
             {drift:.0} m away)"
        );
    }

    // The eight ward anchors have no name in the graph to check against: the
    // bake plants each one at the nav node nearest the centroid of its ward's
    // buildings. The houses `homes.json` bound in the same ward are a large
    // sample of exactly those buildings — and its `ward` is the building's own
    // district string, which is the anchor's `name` — so a live anchor stands
    // among them: 12 m to 30 m today, the loosest being the Bell and Sluice
    // Wards' long sprawl and the Cinder Ward's mere seven houses. Coarse on
    // purpose, and it needs no third file: a renumbered anchor lands in
    // another ward entirely.
    const WARD_ANCHOR_TOLERANCE_M: f64 = 80.0;
    let homes_json: serde_json::Value =
        serde_json::from_str(HOMES_JSON).expect("homes.json parses");
    let homes = homes_json["homes"]
        .as_object()
        .expect("homes.json has a homes map");
    for ward in places["wards"].as_array().expect("places.json has wards") {
        let name = ward["name"].as_str().expect("a ward has a name");
        let node = ward["node"].as_u64().expect("a ward pins a node") as usize;
        assert!(
            node < nav.node_count(),
            "places.json pins node {node} for {name}, past the end of a \
             {}-node graph — re-run `uv run scripts/bake_places.py`",
            nav.node_count()
        );
        let houses: Vec<&serde_json::Value> = homes
            .values()
            .filter(|home| home["ward"].as_str() == Some(name))
            .collect();
        assert!(!houses.is_empty(), "{name} houses somebody");
        let count = houses.len() as f64;
        let centre = Vec3::new(
            houses
                .iter()
                .map(|h| h["point"][0].as_f64().unwrap())
                .sum::<f64>()
                / count,
            WALK_Y,
            houses
                .iter()
                .map(|h| h["point"][1].as_f64().unwrap())
                .sum::<f64>()
                / count,
        );
        let drift = nav.node_point(node).distance(centre);
        assert!(
            drift <= WARD_ANCHOR_TOLERANCE_M,
            "places.json's {name} anchor is out of step with navigation.json — \
             re-run `uv run scripts/bake_places.py` (node {node} stands \
             {drift:.0} m from the centre of the ward's own baked houses, \
             tolerance {WARD_ANCHOR_TOLERANCE_M} m)"
        );
    }

    // ----------------------------------------------------------------- //
    // homes.json — the home-binding
    // ----------------------------------------------------------------- //
    // `point` is the door node's own coordinate at bake time, rounded to four
    // decimals, so the two must still agree to well under a metre. The sim
    // never reads `door_node` (the round walks to `point`), which is precisely
    // why it rots unwatched — and it is the one witness that the whole entry,
    // building and all, was baked against *this* graph.
    const HOME_TOLERANCE_M: f64 = 0.5;
    for (id, home) in homes {
        let building = home["building"].as_str().expect("a home names a building");
        let node = home["door_node"].as_u64().expect("a home pins a door node") as usize;
        assert!(
            node < nav.node_count(),
            "homes.json pins door node {node} for {id}, past the end of a \
             {}-node graph — re-run `uv run scripts/bake_homes.py`",
            nav.node_count()
        );
        let door = nav.door(building).unwrap_or_else(|| {
            panic!(
                "homes.json is out of step with navigation.json — re-run \
                 `uv run scripts/bake_homes.py` ({id}'s building {building} \
                 has no baked door in the graph at all)"
            )
        });
        assert_eq!(
            door.node, node,
            "homes.json is out of step with navigation.json — re-run \
             `uv run scripts/bake_homes.py` ({id}'s building {building} is \
             doored on node {} now, not the pinned {node})",
            door.node
        );
        let baked_point = Vec3::new(
            home["point"][0].as_f64().expect("a home point is a number"),
            WALK_Y,
            home["point"][1].as_f64().expect("a home point is a number"),
        );
        let drift = nav.node_point(node).distance(baked_point);
        assert!(
            drift <= HOME_TOLERANCE_M,
            "homes.json is out of step with navigation.json — re-run \
             `uv run scripts/bake_homes.py` ({id}'s door node {node} resolves \
             to {:?}, {drift:.0} m from the baked point {baked_point:?})",
            nav.node_point(node)
        );
    }

    // ----------------------------------------------------------------- //
    // shelters.json — hand-authored, and the only pin with no script
    // ----------------------------------------------------------------- //
    // `route_node` is where the weather ladder routes a soaked NPC before the
    // final stride into the covered polygon, so it belongs *at* the shelter.
    // Measured against the polygon rather than its centre, because
    // `lanthorn_nave` sits 44.6 m off centre on purpose: the collision export
    // subtracts the cathedral footprint wholesale (CathedralPlugin builds that
    // interior, so none of it reaches the bake), the nave therefore owns no
    // street node of its own, and its pin is the apron inside the west wall —
    // long-standing, intended, and still under 92 m of roof, so still 0 m from
    // the polygon. The worst honest gap today is 7.3 m, the simples awning on
    // Maren's Green, whose node is out in the square.
    const SHELTER_TOLERANCE_M: f64 = 12.0;
    let shelters =
        crate::ShelterMap::from_json_str(include_str!("../../../../assets/world/shelters.json"))
            .expect("shelters.json loads");
    for shelter in shelters.shelters() {
        assert!(
            shelter.route_node < nav.node_count(),
            "shelters.json is hand-authored — there is no bake script for it. \
             `{}` pins route node {}, past the end of a {}-node graph; \
             re-point it by hand against assets/world/navigation.json",
            shelter.id,
            shelter.route_node,
            nav.node_count()
        );
        let pinned = nav.node_point(shelter.route_node);
        let drift = if shelter.contains(pinned) {
            0.0
        } else {
            distance_to_polygon_xz(pinned, &shelter.polygon_xz)
        };
        assert!(
            drift <= SHELTER_TOLERANCE_M,
            "shelters.json is out of step with navigation.json, and it is \
             hand-authored — there is no bake script for it: re-point its \
             `route_node` values by hand against assets/world/navigation.json \
             (`{}` pins node {} at {pinned:?}, {drift:.0} m outside its own \
             polygon, tolerance {SHELTER_TOLERANCE_M} m)",
            shelter.id,
            shelter.route_node
        );
    }
}

/// Metres from `point` to the nearest edge of a closed XZ polygon. Only ever
/// asked about a point already known to be outside it, so it needs no winding
/// test of its own.
fn distance_to_polygon_xz(point: Vec3, polygon: &[[f64; 2]]) -> f64 {
    let mut nearest = f64::INFINITY;
    for index in 0..polygon.len() {
        let [ax, az] = polygon[index];
        let [bx, bz] = polygon[(index + 1) % polygon.len()];
        let (dx, dz) = (bx - ax, bz - az);
        let length_squared = dx * dx + dz * dz;
        let along = if length_squared == 0.0 {
            0.0
        } else {
            (((point.x - ax) * dx + (point.z - az) * dz) / length_squared).clamp(0.0, 1.0)
        };
        nearest = nearest.min(f64::hypot(
            point.x - (ax + along * dx),
            point.z - (az + along * dz),
        ));
    }
    nearest
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
    assert_eq!(
        active_leg(&legs, Office::Kindling, Weekday::Bellday)
            .unwrap()
            .label,
        "oven"
    );
    assert_eq!(
        active_leg(&legs, Office::HighWick, Weekday::Bellday)
            .unwrap()
            .label,
        "shop"
    );
    assert_eq!(
        active_leg(&legs, Office::Snuffing, Weekday::Bellday)
            .unwrap()
            .label,
        "home"
    );
    // Deep night before the first leg carries over the day's tail (home).
    assert_eq!(
        active_leg(&legs, Office::Watch, Weekday::Bellday)
            .unwrap()
            .label,
        "home"
    );
}

#[test]
fn a_market_day_leg_wins_only_on_its_day() {
    // The generic post first, then the market-square leg for the same office.
    let legs = vec![
        leg(Office::Dayspring, "workshop", None),
        leg(Office::Dayspring, "square", Some(vec![Weekday::Highmarket])),
    ];
    assert_eq!(
        active_leg(&legs, Office::Dayspring, Weekday::Highmarket)
            .unwrap()
            .label,
        "square",
        "on the market day the crowd moves to the square"
    );
    assert_eq!(
        active_leg(&legs, Office::Dayspring, Weekday::Fourth)
            .unwrap()
            .label,
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
    world.add_character(person(
        "draper_a",
        Vec3::new(120.0, WALK_Y, 260.0),
        Some("draper"),
        Significance::Major,
    ));
    world.add_character(person(
        "fish_a",
        Vec3::new(-366.0, WALK_Y, -406.0),
        Some("fish_trader"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let leg_label = |id: &str, weekday: Weekday| {
        active_leg(
            &round.people[&ActorId::from_raw(id)].legs,
            Office::HighWick,
            weekday,
        )
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
        for square in [
            "The Wickmarket",
            "The Tallage",
            "Coswald's Yard",
            "Maren's Green",
        ] {
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
    let square = |name: &str| {
        nav.node_point(
            nav.place(name)
                .expect("a market square is a nav place")
                .node,
        )
    };
    let wickmarket = square("The Wickmarket");
    let tallage = square("The Tallage");
    let coswalds = square("Coswald's Yard");
    let marens = square("Maren's Green");

    let mut world = base_world();
    // Occupations chosen so nobody's *workplace* is the square they stand in —
    // the ordinary-day baseline for all four squares is then exactly zero.
    world.add_character(person(
        "draper_hm",
        wickmarket,
        Some("draper"),
        Significance::Major,
    ));
    world.add_character(person(
        "baker_lm",
        tallage,
        Some("baker"),
        Significance::Major,
    ));
    world.add_character(person(
        "fish_hm",
        coswalds,
        Some("fish_trader"),
        Significance::Major,
    ));
    world.add_character(person(
        "butcher_lm",
        marens,
        Some("butcher"),
        Significance::Major,
    ));
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
    assert_eq!(
        occupancy(&round, 2),
        [1, 0, 1, 0],
        "Highmarket fills the Wickmarket AND Coswald's Yard"
    );
    // Day 5, Lowmarket: BOTH of its squares rise above the baseline.
    assert_eq!(
        occupancy(&round, 5),
        [0, 1, 0, 1],
        "Lowmarket fills the Tallage AND Maren's Green"
    );
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
    let lanthorn = nav.node_point(
        nav.place("The Lanthorn")
            .expect("the nave is a nav place")
            .node,
    );
    let mut world = base_world();
    // `a2gpk` is a real homes.json id, so the housed Kindling lie-in resolves.
    world.add_character(person(
        "a2gpk",
        lanthorn,
        Some("baker"),
        Significance::Major,
    ));
    world.add_character(person(
        "mason_b",
        lanthorn,
        Some("mason"),
        Significance::Major,
    ));
    world.add_character(person(
        "boat_b",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("boatworker"),
        Significance::Major,
    ));
    world.add_character(person(
        "tap_b",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("tavern_worker"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let leg = |id: &str, office: Office| {
        active_leg(
            &round.people[&ActorId::from_raw(id)].legs,
            office,
            Weekday::Bellday,
        )
        .expect("a Bellday leg is active")
        .clone()
    };
    // The housed trader does not open the workshop before light: home, idle.
    let lie_in = leg("a2gpk", Office::Kindling);
    assert!(
        lie_in.is_home,
        "on Bellday the workshop stays shut at the Kindling"
    );
    assert_eq!(lie_in.doing, Arrival::Idle);
    // From Dayspring through the Waning, trader and day worker fill the nave.
    for office in [Office::Dayspring, Office::HighWick, Office::Waning] {
        for id in ["a2gpk", "mason_b"] {
            let leg = leg(id, office);
            assert_eq!(
                leg.label, "The Lanthorn",
                "{id} prays in the nave at {office:?}"
            );
            assert_eq!(leg.doing, Arrival::Pray);
        }
    }
    // The wharf works before dawn as ever, then joins the nave at Dayspring.
    assert_ne!(leg("boat_b", Office::Kindling).label, "The Lanthorn");
    assert!(
        !leg("boat_b", Office::Kindling).is_home,
        "the moorings open before light even on Bellday"
    );
    assert_eq!(leg("boat_b", Office::Dayspring).label, "The Lanthorn");
    // A night trade keeps its counter: no Bellday leg drags the tavern to the nave.
    assert_ne!(leg("tap_b", Office::Dayspring).label, "The Lanthorn");
    // And on an ordinary day the same trader is at their own post, not the nave.
    assert_ne!(
        active_leg(
            &round.people[&ActorId::from_raw("a2gpk")].legs,
            Office::Dayspring,
            Weekday::Second
        )
        .expect("an ordinary Dayspring post")
        .label,
        "The Lanthorn"
    );

    // Census: the two standing in the nave count there on Bellday (day 0)...
    let bellday = round.census(&world, &clock_on(Office::HighWick, 0), 0.0);
    assert_eq!(
        bellday.by_place.get("The Lanthorn").copied(),
        Some(2),
        "the nave fills on Bellday"
    );
    assert_eq!(
        bellday.by_place.get("The Wickmarket").copied(),
        None,
        "the generic workshop stays closed"
    );
    // ...and on an ordinary day the same spot censuses as nobody's post.
    let ordinary = round.census(&world, &clock_on(Office::HighWick, 1), 0.0);
    assert_eq!(ordinary.by_place.get("The Lanthorn").copied(), None);
}

/// Seed writes each person's timetable into their own character state — the
/// sheet's `your_round` lines — so "where will you be tomorrow?" is answered
/// from the sheet rather than improvised.
#[test]
fn seed_writes_the_daily_round_onto_the_character_state() {
    let nav = nav();
    let lanthorn = nav.node_point(
        nav.place("The Lanthorn")
            .expect("the nave is a nav place")
            .node,
    );
    let mut world = base_world();
    world.add_character(person(
        "mason_b",
        lanthorn,
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let lines = &world.characters[&ActorId::from_raw("mason_b")]
        .state
        .daily_round;
    assert!(
        !lines.is_empty(),
        "an enrolled townsperson knows their round"
    );
    assert!(
        lines.iter().any(
            |line| line.contains("prayers at The Lanthorn") && line.contains("on Bellday only")
        ),
        "the Bellday service is a marked leg: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("at Dayspring:") && line.contains("work at ")),
        "the ordinary working leg reads as work: {lines:?}"
    );
}

// --------------------------------------------------------------------------- //
// The Night Office's seams into the round (movement M6)
// --------------------------------------------------------------------------- //

/// Forty ids the home bake really housed and `rounds.json` gives no route
/// override, so the occupation template's `lamplight: home, sleep` leg is the
/// one the ambient evening roll finds.
const HOUSED_AMBIENT_IDS: [&str; 40] = [
    "a2gpk", "a3crk", "a4anh", "a5sbp", "a6avh", "a7pcr", "a8ewf", "a9rnh", "ar5tl", "b0nll",
    "b1sbb", "b3glc", "b5ewk", "b6clm", "b9stt", "ba8hf", "bc6tf", "bd7hb", "bn1id", "bn2hm",
    "bn3sg", "bn4cp", "bn5jk", "bn6gb", "bn7an", "bn8jm", "bn9et", "bnawr", "bnbcr", "bndhk",
    "bnpro", "bnrse", "br2sk", "brn5o", "bt4hb", "c2nsl", "c3wnk", "c5tbo", "c6pkl", "c7kbd",
];

/// A day worker's bedtime is the office of their sleep leg — which is what
/// staggers the Night Office across the night without a scheduler of its own.
#[test]
fn bedtime_is_the_office_of_the_earliest_sleep_leg() {
    let nav = nav();
    let mut world = base_world();
    let mason = ActorId::from_raw("b4hst");
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    assert!(round.is_enrolled(&mason));
    assert_eq!(
        round.bedtime(&mason),
        Some(Office::Lamplight),
        "the authored mason's round beds him at dusk"
    );
    // Nobody the round never enrolled has a bedtime to read.
    assert_eq!(round.bedtime(&ActorId::from_raw("nobody")), None);
}

/// `set_round` moves one leg and nothing else: the anchor and the sheet line
/// change, the office, the weekday and the rest of the day stand — and the
/// walker really goes there.
#[test]
fn a_set_round_edit_moves_one_leg_and_rewrites_only_its_sheet_line() {
    let (mut round, mut world, nav, mason) = seed_hamel();
    let clock = clock_at(Office::Dayspring);
    let before = world.characters[&mason].state.daily_round.clone();
    assert!(before.len() >= 2, "the mason keeps a multi-leg day");

    // Point his working leg at the nave, a place he necessarily knows (it is a
    // coarse destination everyone holds).
    let lanthorn = world
        .places
        .named("The Lanthorn")
        .expect("the nave is registered")
        .id
        .clone();
    world
        .characters
        .get_mut(&mason)
        .unwrap()
        .state
        .places_known
        .insert(lanthorn.clone());
    let line = crate::apply_action(
        &mut world,
        &mason,
        "set_round",
        &serde_json::json!({"leg": 1, "place_id": lanthorn.as_str()}),
    )
    .expect("a known place and a leg on the sheet");
    assert!(line.contains("The Lanthorn"), "{line}");

    // The verb records; the round carries out.
    assert!(world.characters[&mason].state.round_edit.is_some());
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        1.0,
        &player(),
        &BTreeSet::new(),
    );
    assert!(
        world.characters[&mason].state.round_edit.is_none(),
        "the edit is consumed, not re-applied every tick"
    );

    let after = &world.characters[&mason].state.daily_round;
    assert_eq!(after.len(), before.len(), "no leg was added or removed");
    // `leg: 1` on the sheet is index 0 in the round — the verb subtracts.
    assert_eq!(after[1], before[1], "the other legs are untouched");
    assert!(
        after[0].contains("The Lanthorn"),
        "leg 1 moved: {:?}",
        after[0]
    );
    assert!(
        after[0].starts_with(&before[0][..before[0].find(':').unwrap()]),
        "the office and weekday of the leg stand: {:?} -> {:?}",
        before[0],
        after[0]
    );
}

/// An edit naming a leg the round no longer has, or a place the registry does
/// not hold, is dropped rather than applied — and the verb refuses both before
/// they can get that far.
#[test]
fn set_round_refuses_an_unknown_place_and_a_leg_off_the_end() {
    let (_round, mut world, _nav, mason) = seed_hamel();
    let legs = world.characters[&mason].state.daily_round.len();

    let unknown = crate::apply_action(
        &mut world,
        &mason,
        "set_round",
        &serde_json::json!({"leg": 1, "place_id": "pl_zzzz"}),
    )
    .unwrap_err();
    assert_eq!(unknown.code, crate::ActionErrorCode::UnknownPlace);

    let lanthorn = world.places.named("The Lanthorn").unwrap().id.clone();
    world
        .characters
        .get_mut(&mason)
        .unwrap()
        .state
        .places_known
        .insert(lanthorn.clone());
    let off_the_end = crate::apply_action(
        &mut world,
        &mason,
        "set_round",
        &serde_json::json!({"leg": legs + 1, "place_id": lanthorn.as_str()}),
    )
    .unwrap_err();
    assert_eq!(off_the_end.code, crate::ActionErrorCode::InvalidArguments);
    assert!(world.characters[&mason].state.round_edit.is_none());
}

/// The ambient cast's Night Office is a code roll and nothing else: a share of
/// them take tomorrow's evening at a tavern hearth, the rest stay home, and a
/// night that comes up "home" really puts back the evening the seed authored.
#[test]
fn the_ambient_roll_moves_some_evenings_to_a_tavern_and_restores_the_rest() {
    let nav = nav();
    let mut world = base_world();
    // Forty ambient day-workers, and **housed** ones: a bedless actor has no
    // `home` leg for the evening roll to move, which is the design, so an
    // invented id would make this test pass by proving nothing. These are real
    // baked homes (`assets/world/homes.json`) with no authored route override.
    let ids: Vec<ActorId> = HOUSED_AMBIENT_IDS
        .iter()
        .enumerate()
        .map(|(index, id)| {
            world.add_character(person(
                id,
                Vec3::new(index as f64, WALK_Y, 95.0),
                Some("mason"),
                Significance::Ambient,
            ));
            ActorId::from_raw(*id)
        })
        .collect();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    let seeded: Vec<Vec<String>> = ids
        .iter()
        .map(|id| world.characters[id].state.daily_round.clone())
        .collect();

    let moved = round.reroll_ambient_evenings(&mut world, 0);
    assert!(moved > 0, "some evening moved on the first night");
    assert!(moved < ids.len(), "and most people stayed home");
    // Each tavern-goer paired with the evening the seed authored for *them*.
    let tavern_goers: Vec<(&ActorId, &Vec<String>)> = ids
        .iter()
        .zip(&seeded)
        .filter(|(id, before)| world.characters[*id].state.daily_round != **before)
        .collect();
    assert_eq!(tavern_goers.len(), moved);
    let (first, first_seed) = tavern_goers[0];
    assert!(
        world.characters[first]
            .state
            .daily_round
            .iter()
            .any(|line| line.contains("your ease at The ")),
        "a moved evening reads as ease at a hearth, never as a bed: {:?}",
        world.characters[first].state.daily_round
    );

    // The same night twice is the same night: idempotent, not cumulative.
    let again = round.reroll_ambient_evenings(&mut world, 0);
    assert_eq!(again, moved);

    // Thirty nights on, the first night's tavern-goer has slept at their own
    // hearth again — the roll is a nightly choice, not a one-way drift.
    let mut ever_home = 0usize;
    for day in 1..30 {
        round.reroll_ambient_evenings(&mut world, day);
        if world.characters[first].state.daily_round == *first_seed {
            ever_home += 1;
        }
    }
    assert!(
        ever_home > 0,
        "the first night's tavern-goer never went home again"
    );
}

/// The roll never touches anybody the Night Office reflects for itself, and
/// never a night trade — they have no evening to move.
#[test]
fn the_ambient_roll_leaves_the_majors_and_the_night_trades_alone() {
    let nav = nav();
    let mut world = base_world();
    // Both housed, so neither is skipped for want of a bed — the roll has to
    // pass them over for the reason under test, not by accident.
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    world.add_character(person(
        "c8ghd",
        Vec3::new(4.0, WALK_Y, 95.0),
        Some("watchman_and_keeper"),
        Significance::Ambient,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    let before: BTreeMap<ActorId, Vec<String>> = world
        .characters
        .iter()
        .map(|(id, actor)| (id.clone(), actor.state.daily_round.clone()))
        .collect();

    for day in 0..40 {
        round.reroll_ambient_evenings(&mut world, day);
        for (id, lines) in &before {
            assert_eq!(
                &world.characters[id].state.daily_round, lines,
                "{id} was moved on day {day}"
            );
        }
    }
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
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
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
    let coswalds = round.people[&id]
        .legs
        .iter()
        .find(|leg| leg.from == Office::Kindling)
        .expect("Kindling leg")
        .at;
    // At the working morning, away from his post, the round rung sends him there.
    match decide_only(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Kindling,
        Weekday::Bellday,
    ) {
        Decision::Travel(target) => assert!(
            target.distance(coswalds) < 1.0,
            "he sets off for Coswald's Yard, not {target:?}"
        ),
        other => panic!("expected Travel to the yard, got {other:?}"),
    }
}

/// law_and_order.md M2 — the staffed law. A bench sergeant walks an authored
/// beat through the squares, a gate keeper stands his gate from Dayspring,
/// Odo Trask keeps the toll-house counter instead of praying across town, and
/// the routeless rest of the watch anchors on the watch-bell tower itself.
/// Together with the other two beats and four gates this is the "~8 reporting
/// points instead of one cross-town pilgrimage" the feature asks for.
#[test]
fn the_law_cast_is_stationed_where_people_are() {
    let nav = nav();
    let mut world = base_world();
    for (id, occupation) in [
        ("p009x", "bailiff_and_gaoler"),  // Havise Ashe, bench sergeant
        ("hrnsk", "watchman_and_keeper"), // Renn Skell, gate guard
        ("fo6gl", "court_officer"),       // Odo Trask, notary
        ("p009w", "bailiff_and_gaoler"),  // Ede Clove, Stone keeper (M5b)
        ("p009y", "bailiff_and_gaoler"),  // Tobin Marle, prison guard (M5b)
        ("p00a2", "bailiff_and_gaoler"),  // Ewart Rasp, prison guard (M5b)
        ("p00a1", "bailiff_and_gaoler"),  // Segwin Vell, court usher — routeless
    ] {
        world.add_character(person(
            id,
            Vec3::new(0.0, WALK_Y, 95.0),
            Some(occupation),
            Significance::Minor,
        ));
    }
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    // The sergeant's beat moves with the offices — market at first light, the
    // Gradine at noon — and keeps the exemption a night cry needs.
    let ashe = &round.people[&ActorId::from_raw("p009x")];
    let leg_at = |person: &Townsperson, office: Office| {
        person
            .legs
            .iter()
            .find(|leg| leg.from == office && leg.only_on.is_none())
            .unwrap_or_else(|| panic!("no ordinary leg at {office:?}"))
            .clone()
    };
    assert_eq!(leg_at(ashe, Office::Dayspring).label, "The Wickmarket");
    assert_eq!(leg_at(ashe, Office::HighWick).label, "The Gradine");
    assert!(ashe.curfew_exempt, "the bench answers night cries");

    // The gate keeper *stands* his gate — the road parties' posted verb.
    let skell = &round.people[&ActorId::from_raw("hrnsk")];
    let gate = leg_at(skell, Office::Dayspring);
    assert_eq!(gate.label, "The Harne Gate");
    assert_eq!(gate.doing, Arrival::Stand);

    // The notary's counter is the toll-house, not the cleric archetype's
    // chapter house.
    let odo = &round.people[&ActorId::from_raw("fo6gl")];
    assert_eq!(leg_at(odo, Office::Dayspring).label, "Tallage toll-house");

    // The Stone keeper keeps the Stone House (M5b). The gaol's confinement is a
    // person standing at a threshold, so a keeper who wandered the ward would be
    // an open door — and she is posted there all day, not merely at the Waning
    // the night-watch archetype used to give her.
    let clove = &round.people[&ActorId::from_raw("p009w")];
    for office in [
        Office::Dayspring,
        Office::HighWick,
        Office::Waning,
        Office::Lamplight,
    ] {
        assert_eq!(
            leg_at(clove, office).label,
            "The Stone House",
            "the keeper is at her threshold at {office:?}"
        );
    }
    assert!(clove.curfew_exempt, "somebody has to be awake with them");

    // The two prison guards stand it in turn, which is also what makes a second
    // pair of hands available when a prisoner pulls (M4d: two holders is much
    // worse to pull against).
    for (id, office) in [("p009y", Office::Dayspring), ("p00a2", Office::HighWick)] {
        let guard = &round.people[&ActorId::from_raw(id)];
        let posted = leg_at(guard, office);
        assert_eq!(posted.label, "The Stone House");
        assert_eq!(posted.doing, Arrival::Stand);
    }

    // …and the routeless rest of the bench still anchors on the tower next door,
    // because M5b narrowed exactly three postings and left `workplaces` alone:
    // `build_legs` binds the nearest candidate, so adding the gaol there would
    // have quietly pulled the debt officer and the court usher inside as well.
    let vell = &round.people[&ActorId::from_raw("p00a1")];
    assert_eq!(
        leg_at(vell, Office::Waning).label,
        "Bellstand watch-bell tower"
    );
}

/// A character the city is already holding (`law_and_order.md` M5b) — the
/// `prisoner` circumstance the eight authored inmates carry, and nothing else.
fn prisoner(id: &str, position: Vec3) -> Character {
    let mut character = person(
        id,
        position,
        Some("domestic_servant"),
        Significance::Ambient,
    );
    character
        .sheet
        .lore
        .as_mut()
        .expect("the fixture gives them a profile")
        .circumstances = vec![crate::custody::PRISONER_CIRCUMSTANCE.to_string()];
    character
}

/// M5b: the eight the lore already holds go into the room the lore already
/// named. Their sheets say they are *"now held … Stone House rations and food
/// carried in by kin are your present support"*, and until M5a built the place
/// they were spawned walking free across the whole city — a live
/// world-consistency bug, and also the gaol's entire population, already
/// written.
///
/// The `authored` flag is the point of the whole record: the eight arrive
/// `Committed` with no arresting officer and no notice, and they do not count
/// against [`crate::custody::CUSTODY_MAX_ARRESTS`], because that cap exists to
/// stop a bad-tempered sergeant emptying the Wickmarket, not to evict the lore.
#[test]
fn the_confined_are_seeded_into_the_stone_house() {
    let nav = nav();
    let mut world = base_world();
    // Scattered exactly as they are authored — Bell-and-Sluice, the Cloth Ward,
    // the far north-east — because the seeding is what gathers them.
    // All eight, so the ring placement is exercised at its real width — see the
    // roam assertion below for why that matters.
    for (id, at) in [
        ("p0055", Vec3::new(-17.0, WALK_Y, -249.9)),
        ("p0056", Vec3::new(-12.3, WALK_Y, -173.3)),
        ("p0057", Vec3::new(-5.3, WALK_Y, -110.3)),
        ("p0059", Vec3::new(-0.9, WALK_Y, -317.5)),
        ("p005a", Vec3::new(9.3, WALK_Y, -145.3)),
        ("p005c", Vec3::new(232.4, WALK_Y, 336.8)),
        ("p005f", Vec3::new(15.8, WALK_Y, -411.3)),
        ("p00b0", Vec3::new(337.8, WALK_Y, -103.3)),
    ] {
        world.add_character(prisoner(id, at));
    }
    // A free neighbour, to prove the circumstance is what is read and not the
    // occupation or the ward.
    world.add_character(person(
        "free1",
        Vec3::new(-12.0, WALK_Y, -173.0),
        Some("domestic_servant"),
        Significance::Ambient,
    ));

    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let held: Vec<&str> = world.custody.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        held,
        [
            "p0055", "p0056", "p0057", "p0059", "p005a", "p005c", "p005f", "p00b0"
        ]
    );
    assert!(
        !world.custody.holds(&ActorId::from_raw("free1")),
        "a housemaid who is not a prisoner is not gaoled for keeping the same trade"
    );

    let stone_house = world
        .places
        .named(crate::custody::STONE_HOUSE_PLACE_NAME)
        .expect("M5a put the Stone House in the registry")
        .clone();
    for id in [
        "p0055", "p0056", "p0057", "p0059", "p005a", "p005c", "p005f", "p00b0",
    ] {
        let id = ActorId::from_raw(id);
        let record = world.custody.get(&id).expect("held");
        assert!(
            world.custody.is_confined(&id),
            "{id} is committed, not merely in charge"
        );
        assert!(record.authored, "{id} was here before the run began");
        assert!(
            record.station.stone_house,
            "{id} is in the gaol, not at a gate arch"
        );
        assert_eq!(record.station.place_id, stone_house.id);
        assert!(record.officer.is_none(), "nobody arrested {id}");
        assert!(record.notice_id.is_none(), "no word of ours put {id} here");

        // And they are standing in it, on ground they could walk out of if they
        // were ever let out — never inside a wall.
        let character = &world.characters[&id];
        let at = character.position_m();
        assert!(
            nav.is_walkable(at.x, at.z),
            "{id} stands on real graph at {at:?}"
        );
        // Well inside the roam, and that is load-bearing rather than tidy: a
        // prisoner seeded past `COMMITTED_ROAM_M` is judged to have walked out
        // on the very first poll, so a wide enough ring would have let the
        // lore's own inmates out of the gaol the instant the game started.
        let from_the_door = f64::hypot(at.x - stone_house.point.x, at.z - stone_house.point.z);
        assert!(
            from_the_door <= crate::custody::COMMITTED_ROAM_M * 0.5,
            "{id} is in the room and staying in it: {from_the_door} m"
        );
        assert!(character.state.movement.is_none(), "{id} is not mid-walk");
    }

    // The cap is untouched: the standing population is not an arrest.
    assert_eq!(world.custody.arrest_count(), 0);
    assert!(world.custody.has_room());
}

/// M5b, and the spec's own test: *"a confined NPC with `thirst` under the well
/// rung does not path to a cistern, does not take a round leg, and is not
/// curfew-routed."* One guard at rung 0 of [`decide`] covers all three, because
/// all three are decided in that one function — an inmate who set off for the
/// nearest cistern would walk straight through the gaol wall.
#[test]
fn a_confined_inmate_stays_put_through_thirst_a_leg_and_the_curfew() {
    let nav = nav();
    let mut world = base_world();
    world.add_character(prisoner("p0056", Vec3::new(-12.3, WALK_Y, -173.3)));
    let held = ActorId::from_raw("p0056");

    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    assert!(
        world.custody.holds(&held),
        "M5a's place resolved and they are in it"
    );
    let cell = world.characters[&held].position_m();
    // They are enrolled like anybody else — the legs are waiting for the day
    // they are let out; the guard is what keeps them off their feet meanwhile.
    assert!(!round.people[&held].legs.is_empty());

    let mut nudges = Vec::new();
    for (what, office) in [
        ("parched", Office::Dayspring),
        ("a round leg", Office::HighWick),
        ("the curfew", Office::Snuffing),
    ] {
        world.characters.get_mut(&held).unwrap().state.needs.thirst = THIRST_PARCHED - 1.0;
        round.people.get_mut(&held).unwrap().next_decision = 0.0;
        round.people.get_mut(&held).unwrap().phase = Phase::Idle;
        run_ladder(
            &mut round,
            &mut world,
            &nav,
            &clock_at(office),
            0.0,
            &BTreeSet::new(),
            &mut nudges,
        );
        let character = &world.characters[&held];
        assert!(
            character.state.movement.is_none(),
            "{what} did not put a route under a confined body"
        );
        assert!(
            character.state.intent.is_none(),
            "{what} laid no errand either"
        );
        assert_eq!(character.position_m(), cell, "{what} did not move them");
    }
}

/// M5b: **Stone House rations.** Somebody the law is holding cannot walk to a
/// cistern or a stall — rung 0 of `decide` sees to that, and `go_to` refuses
/// them — so without an exemption in `decay_needs` the eight authored inmates
/// decay to nothing within minutes of every run starting and stay there, dying
/// of thirst against a wall for the rest of the session.
///
/// The lore answered this before the code existed: their own sheets say *"Stone
/// House rations and food carried in by kin are your present support"*, and
/// M5's design turns on families bringing bread and a blanket to the grate. A
/// keeper who let their prisoners die would not be a keeper, and being held is
/// meant to be a conversation rather than a slow death.
#[test]
fn the_confined_are_fed_and_watered_because_they_cannot_go_and_get_it() {
    let nav = nav();
    let mut world = base_world();
    world.add_character(prisoner("p0056", Vec3::new(-12.3, WALK_Y, -173.3)));
    // A free neighbour on the same trade, as the control: the exemption must be
    // custody, not occupation.
    world.add_character(person(
        "free1",
        Vec3::new(-12.0, WALK_Y, -173.0),
        Some("domestic_servant"),
        Significance::Ambient,
    ));
    let held = ActorId::from_raw("p0056");
    let free = ActorId::from_raw("free1");

    let mut round = Round::new();
    let clock = clock_at(Office::Dayspring);
    round.seed(&mut world, &nav, 0.0, &clock);
    assert!(
        world.custody.holds(&held),
        "M5a's place resolved and they are in it"
    );

    for who in [&held, &free] {
        let needs = &mut world.characters.get_mut(who).unwrap().state.needs;
        needs.thirst = 60.0;
        needs.hunger = 60.0;
    }
    // A third of a game day of decay — far past THIRST_PARCHED for anyone the
    // clock is allowed to touch.
    decay_needs(&mut round, &mut world, &clock, 1200.0);

    let kept = &world.characters[&held].state.needs;
    assert_eq!(kept.thirst, 60.0, "the keeper brings water");
    assert!(kept.hunger >= 60.0, "and rations: {}", kept.hunger);

    // The control: the same trade, the same street, not held — and hunger, which
    // every enrolled body loses, really does drain out of them. So the exemption
    // is custody and not the occupation.
    let outside = &world.characters[&free].state.needs;
    assert!(
        outside.hunger < 60.0,
        "an unheld neighbour still gets hungry: {}",
        outside.hunger
    );
    assert!(
        world.characters[&held].state.needs.hunger > outside.hunger,
        "and the difference between them is the ration"
    );
    // Thirst only drains for someone the round bound a water source to, which a
    // hand-built fixture may not be; assert the exemption where it is testable.
    if round.people[&free].draws_water() {
        assert!(outside.thirst < 60.0, "{}", outside.thirst);
    }
}

/// Session 514: a famished Bencher walking the player to the Stone House was
/// marched off to the Hungry Ox by the famished rung — "Rasmus, hunger has
/// turned me back" — and the 20 m poll freed the prisoner behind him.
/// Delivering a prisoner outranks the body (rung 0's other side): while the
/// escort has somebody merely in charge, the pressing rungs wait and the
/// station `go_to` keeps the feet, and the moment the prisoner is committed
/// the body presses again.
#[test]
fn an_escort_is_not_marched_to_food_mid_delivery() {
    let nav = nav();
    let mut world = base_world();
    // A real housed major (the guard reads custody, not occupation — and the
    // famished control below needs his non-exempt hearth to press him home).
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    world.add_character(person(
        "taken1",
        Vec3::new(1.0, WALK_Y, 95.0),
        Some("domestic_servant"),
        Significance::Ambient,
    ));
    let officer = ActorId::from_raw("b4hst");
    let taken = ActorId::from_raw("taken1");
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    // Famished at a meal office, free: the ladder really would divert him —
    // the control that makes the guarded assertions below mean anything.
    world
        .characters
        .get_mut(&officer)
        .unwrap()
        .state
        .needs
        .hunger = HUNGER_FAMISHED / 2.0;
    let (_, pressure) = decide(
        &round,
        &world,
        &nav,
        &officer,
        0,
        Office::HighWick,
        Weekday::Bellday,
    );
    assert_eq!(
        pressure,
        Some(FAMISHED_PRESSURE),
        "a free famished officer is pressed toward food"
    );

    // Taken in charge and aimed at the gaol, exactly as the `seize` verb
    // leaves the officer: the delivery now outranks the hunger.
    let station = crate::custody::stone_house(&world.places).expect("M5a built the gaol");
    let station_point = station.point;
    world
        .custody
        .seize(taken.clone(), officer.clone(), Some(1), station, 0.0);
    world.characters.get_mut(&officer).unwrap().state.intent = Some(TravelIntent {
        target: IntentTarget::Place {
            place_id: PlaceId::from_raw("pl_gaol"),
            name: crate::custody::STONE_HOUSE_PLACE_NAME.into(),
            point: station_point,
        },
        budget_seconds: 600.0,
        deadline: Some(600.0),
    });
    let (decision, pressure) = decide(
        &round,
        &world,
        &nav,
        &officer,
        0,
        Office::HighWick,
        Weekday::Bellday,
    );
    assert_eq!(pressure, None, "no pressure line marches an escort off");
    match decision {
        Decision::TravelIntent(target) => assert!(
            target.distance(station_point) < 1.0,
            "he keeps walking to the Stone House, not to {target:?}"
        ),
        other => panic!("expected the station go_to to keep the feet, got {other:?}"),
    }

    // Committed: the keeper at the threshold is free again, and the same
    // hunger presses at once.
    assert!(world.custody.commit(&taken, 0.0).is_some());
    world.characters.get_mut(&officer).unwrap().state.intent = None;
    let (_, pressure) = decide(
        &round,
        &world,
        &nav,
        &officer,
        0,
        Office::HighWick,
        Weekday::Bellday,
    );
    assert_eq!(
        pressure,
        Some(FAMISHED_PRESSURE),
        "the delivery done, the body gets its say"
    );
}

/// Session 514's other half: the escort gate held the needs rungs, but once
/// the station intent died — its budget burned through conversation holds, or
/// the closing chase's Person-follow ending at the grab — the round leg, the
/// social pull, the wander and the weather shelters were all still willing to
/// walk the officer away, dragging an NPC prisoner across town or silently
/// freeing a player one at the 20 m poll. An escort with no live `go_to`
/// stands where they are until the custody poll re-aims them.
#[test]
fn an_escort_with_no_errand_stands_rather_than_walk_their_round() {
    let nav = nav();
    let mut world = base_world();
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    world.add_character(person(
        "taken1",
        Vec3::new(1.0, WALK_Y, 95.0),
        Some("domestic_servant"),
        Significance::Ambient,
    ));
    let officer = ActorId::from_raw("b4hst");
    let taken = ActorId::from_raw("taken1");
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    // Control: free at the working morning, away from his post, the round rung
    // really does walk him — the guarded assertions below mean something.
    match decide_only(
        &round,
        &world,
        &nav,
        &officer,
        0,
        Office::Kindling,
        Weekday::Bellday,
    ) {
        Decision::Travel(_) => {}
        other => panic!("control: the round leg walks a free mason, got {other:?}"),
    }

    // Taken in charge, and with no intent at all — the post-expiry, post-grab
    // state nothing used to heal.
    let station = crate::custody::stone_house(&world.places).expect("M5a built the gaol");
    world
        .custody
        .seize(taken.clone(), officer.clone(), Some(1), station, 0.0);
    match decide_only(
        &round,
        &world,
        &nav,
        &officer,
        0,
        Office::Kindling,
        Weekday::Bellday,
    ) {
        Decision::Stay => {}
        other => panic!("an escort with no errand stands, got {other:?}"),
    }

    // A stored weather-shelter claim may not move him either.
    round.weather_shelter_intents.insert(
        officer.clone(),
        WeatherShelterIntent {
            shelter: 0,
            target: Vec3::new(10.0, WALK_Y, 95.0),
            release_threshold: 0.08,
            below_since_days: None,
            release_after_days: 1.0,
        },
    );
    match decide_only(
        &round,
        &world,
        &nav,
        &officer,
        0,
        Office::Kindling,
        Weekday::Bellday,
    ) {
        Decision::Stay => {}
        other => panic!("a sheltering escort stands too, got {other:?}"),
    }
}

/// An officer seized mid-errand must not finish the errand first: the
/// committed-state skips at the top of `run_ladder` would otherwise carry a
/// live well or stall errand to completion — the whole walk-queue-draw-deliver
/// arc — before `decide` ever saw the fresh station intent, and `tick_intents`
/// never lays a walk from a well phase. The escort sweep abandons the errand
/// through the same release paths a closing stall uses: no leaked queue slot,
/// no leaked `serving` entry, and the shelter claim dropped with it.
#[test]
fn a_seizure_abandons_the_officers_well_and_stall_errands() {
    let nav = nav();
    let clock = clock_at(Office::Dayspring);
    let mut world = base_world();
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    world.add_character(person(
        "taken1",
        Vec3::new(1.0, WALK_Y, 95.0),
        Some("domestic_servant"),
        Significance::Ambient,
    ));
    let officer = ActorId::from_raw("b4hst");
    let taken = ActorId::from_raw("taken1");
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    assert!(!round.sources.is_empty(), "the seeded city has wells");
    assert!(!round.stalls.is_empty(), "the seeded city has stalls");

    // Mid-well-errand — standing in the queue at the curb — with a claimed
    // shelter from earlier drizzle beside it.
    round.sources[0].queue.push(officer.clone());
    round.people.get_mut(&officer).expect("enrolled").phase = Phase::Queued;
    round.weather_shelter_intents.insert(
        officer.clone(),
        WeatherShelterIntent {
            shelter: 0,
            target: Vec3::new(10.0, WALK_Y, 95.0),
            release_threshold: 0.08,
            below_since_days: None,
            release_after_days: 1.0,
        },
    );
    let station = crate::custody::stone_house(&world.places).expect("M5a built the gaol");
    world
        .custody
        .seize(taken.clone(), officer.clone(), Some(1), station, 0.0);

    let mut nudges = Vec::new();
    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        0.0,
        &BTreeSet::new(),
        &mut nudges,
    );
    assert!(
        !round.sources[0].queue.contains(&officer),
        "the queue slot is given back, not leaked"
    );
    assert_eq!(
        round.people[&officer].phase,
        Phase::Idle,
        "the well errand is abandoned"
    );
    assert!(
        !round.weather_shelter_intents.contains_key(&officer),
        "the shelter claim is dropped"
    );

    // Mid-stall-errand too: queued to buy when the seizure lands.
    round.stalls[0].queue.push(officer.clone());
    round.people.get_mut(&officer).expect("enrolled").food = Some(FoodErrand {
        stall: 0,
        phase: FoodPhase::Queued,
    });
    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        1.0,
        &BTreeSet::new(),
        &mut nudges,
    );
    assert!(
        round.people[&officer].food.is_none(),
        "the stall errand is abandoned"
    );
    assert!(
        !round.stalls[0].queue.contains(&officer),
        "the stall queue slot is given back, not leaked"
    );
}

/// The prisoner's side of the same seizure. `abandon_bodily_errands` is only
/// ever run for the escort, so a buyer taken in charge kept their place at the
/// pitch while `custody::follow_escorts` walked their body across the city —
/// and the head of a queue is served unconditionally, so `service_stalls` paid
/// a spark to a vendor hundreds of metres behind them, handed back a loaf, and
/// sat them down to eat it on the march to the Stone House. The place goes with
/// the errand, so the neighbour behind them is served now rather than waiting
/// out a committal for a turn that can never come.
#[test]
fn the_law_takes_a_buyer_out_of_the_stall_queue() {
    let (mut world, mut round, _vendor, buyer, stock_id) = bread_stall_world();
    let clock = clock_at(Office::HighWick);
    let pitch = round.stalls[0].pitch;

    // A second hungry neighbour, queued behind the one the law wants.
    let next = ActorId::from_raw("nextup");
    world.add_character(person(
        "nextup",
        Vec3::new(2.0, WALK_Y, 1.0),
        None,
        Significance::Minor,
    ));
    let next_wallet = ItemId::from_raw("w_nextup");
    world.add_item(Item::stack(next_wallet.clone(), "spark", 5));
    world
        .characters
        .get_mut(&next)
        .unwrap()
        .state
        .holds
        .push(next_wallet);
    world.assert_invariants();
    round.stalls[0].queue.push(next.clone());
    for id in [&buyer, &next] {
        round.people.insert(id.clone(), weather_person(pitch));
        round.people.get_mut(id).unwrap().food = Some(FoodErrand {
            stall: 0,
            phase: FoodPhase::Queued,
        });
    }

    let station = crate::custody::Station {
        place_id: PlaceId::from_raw("pl_test"),
        name: crate::custody::STONE_HOUSE_PLACE_NAME.into(),
        point: Vec3::new(200.0, WALK_Y, 0.0),
        stone_house: true,
    };
    world.custody.seize(
        buyer.clone(),
        ActorId::from_raw("srgnt"),
        Some(1),
        station,
        0.0,
    );

    service_stalls(&mut round, &mut world, &clock, 0.0, &player());
    assert!(
        !round.stalls[0].queue.contains(&buyer),
        "the law's prisoner loses their place, not merely their turn"
    );
    assert!(
        round.people[&buyer].food.is_none(),
        "and the errand with it, so the ladder has them back the moment they are released"
    );
    assert_eq!(
        round.stalls[0].serving.as_ref().map(|(id, _)| id.clone()),
        Some(next.clone()),
        "the neighbour behind them is served instead of queueing behind a committal"
    );

    // Past the purchase timer: nothing of the prisoner's ever settles.
    service_stalls(
        &mut round,
        &mut world,
        &clock,
        PURCHASE_SECONDS + 0.1,
        &player(),
    );
    assert_eq!(
        world.wallet_sparks(&buyer),
        5,
        "no spark crossed the city to a vendor the prisoner has been marched away from"
    );
    assert_eq!(
        world.items[&stock_id].quantity, 2,
        "the single loaf off the board is the neighbour's"
    );
    assert_eq!(world.wallet_sparks(&next), 3, "the neighbour paid for it");
    world.assert_invariants();
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
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    // In conversation: his cadence comes due and he stays put.
    let due = round.people[&id].next_decision + 1.0;
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        due,
        &player(),
        &warm(&id),
    );
    assert!(
        !world.characters[&id].is_walking(),
        "nobody sets off for their post mid-conversation"
    );

    // The exchange goes cold: the same cadence now sends him to his post.
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        due,
        &player(),
        &BTreeSet::new(),
    );
    assert!(
        world.characters[&id].is_walking(),
        "the round resumes once the conversation lapses"
    );
    assert_eq!(round.people[&id].phase, Phase::Travelling);

    // Addressed mid-stride: he stops on the spot and is the ladder's again.
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(
        !world.characters[&id].is_walking(),
        "a walker stops to talk"
    );
    assert_eq!(round.people[&id].phase, Phase::Idle);
}

/// A stall errand is the one walk the water phases cannot see — `ApproachStall`
/// leaves the phase standing at Idle — so the interrupt has to ask the food
/// errand too. Addressed halfway across the square, the buyer stops where they
/// stand instead of joining the market queue with an answer still owed.
#[test]
fn a_conversation_stops_a_walk_to_a_food_stall() {
    let nav = nav();
    let clock = clock_at(Office::HighWick);
    let id = ActorId::from_raw("b4hst");
    let mut world = base_world();
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    assert!(!round.stalls.is_empty(), "the seeded city has stalls");

    // The hunger rung's own act: the walk is laid, the water phase left standing.
    apply_decision(
        &mut round,
        &mut world,
        &nav,
        &id,
        Decision::ApproachStall(0),
    );
    assert!(
        world.characters[&id].is_walking(),
        "the buyer set off for the pitch"
    );
    assert_eq!(round.people[&id].phase, Phase::Idle);
    assert!(matches!(
        round.people[&id].food.as_ref().map(|errand| &errand.phase),
        Some(FoodPhase::Approaching)
    ));

    // Addressed mid-square: the buyer stops on the spot...
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(
        !world.characters[&id].is_walking(),
        "a buyer stops to talk instead of walking on to the stall"
    );

    // ...and, stopped short of the pitch, the errand is handed back to the ladder
    // rather than pinning them out of the ladder's reach for a queue slot they
    // never reached.
    resolve_food_arrivals(&mut round, &mut world);
    assert!(
        round.people[&id].food.is_none(),
        "the abandoned stall errand does not outlive the interrupt"
    );
    assert!(
        !round.stalls[0].queue.contains(&id),
        "and they never join the queue they never walked to"
    );
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
    world.add_character(person(
        "mason_a",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    world.add_character(person(
        "mason_b",
        Vec3::new(1.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let hold: BTreeSet<ActorId> = BTreeSet::from([a.clone(), b.clone()]);
    let due = round.people[&a]
        .next_decision
        .max(round.people[&b].next_decision)
        + 1.0;

    // Both cadences come due mid-exchange: neither sets off.
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &hold);
    assert!(
        !world.characters[&a].is_walking(),
        "the speaker's errand waits too"
    );
    assert!(!world.characters[&b].is_walking(), "and the listener's");

    // The exchange goes cold: the same cadences send both to their post.
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        due,
        &player(),
        &BTreeSet::new(),
    );
    assert!(
        world.characters[&a].is_walking(),
        "the round resumes for one"
    );
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
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let hold = warm(&id);

    // The first due cadence at the Snuffing: no marching off mid-sentence —
    // the pressure is injected on the self-correction channel instead.
    let due = round.people[&id].next_decision + 1.0;
    tick(&mut round, &mut world, &nav, &clock, due, &player(), &hold);
    assert!(
        !world.characters[&id].is_walking(),
        "the excuse turn comes before the walk"
    );
    assert!(
        world.characters[&id]
            .inbox()
            .iter()
            .any(|line| line.starts_with("system:") && line.contains("night is falling")),
        "the curfew pressure lands as a system: percept"
    );

    // The next cadence releases the hold regardless of what was said.
    let again = round.people[&id].next_decision + 1.0;
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        again,
        &player(),
        &hold,
    );
    assert!(
        world.characters[&id].is_walking(),
        "urgency beats chat after the one excuse turn"
    );
    assert_eq!(round.people[&id].phase, Phase::Travelling);

    // A parting line must not stop the released walker on the doorstep of night.
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(
        world.characters[&id].is_walking(),
        "the excused walker keeps going"
    );
    assert_eq!(round.people[&id].phase, Phase::Travelling);

    // Once the exchange lapses, the excuse is handed back for the next one.
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        again,
        &player(),
        &BTreeSet::new(),
    );
    assert!(
        !round.people[&id].excused,
        "a lapsed hold resets the excuse"
    );
}

/// Only curfew (5) and parched (2) press hard enough to break a conversation
/// hold; merely thirsty (6) — and everything below — defers to the exchange.
#[test]
fn only_the_pressing_rungs_carry_a_pressure_line() {
    let (round, mut world, nav, servant) = seed_parched_servant();
    // Parched at the Kindling: pressing.
    let (decision, pressure) = decide(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Kindling,
        Weekday::Bellday,
    );
    assert!(matches!(decision, Decision::ApproachWell));
    assert_eq!(pressure, Some(PARCHED_PRESSURE));
    // Housed at the Snuffing: pressing.
    let (decision, pressure) = decide(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    );
    assert!(matches!(decision, Decision::Travel(_)));
    assert_eq!(pressure, Some(CURFEW_PRESSURE));
    // Merely thirsty: the well can wait out the conversation.
    world
        .characters
        .get_mut(&servant)
        .unwrap()
        .state
        .needs
        .thirst = THIRST_PARCHED + 1.0;
    let (decision, pressure) = decide(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Kindling,
        Weekday::Bellday,
    );
    assert!(matches!(decision, Decision::ApproachWell));
    assert_eq!(pressure, None, "rung 6 defers to a conversation");
}

#[test]
fn curfew_sends_the_housed_home_at_the_snuffing() {
    let (round, world, nav, id) = seed_hamel();
    let home = round.people[&id].home.expect("housed");
    match decide_only(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
        Decision::Travel(target) => assert!(
            target.distance(home) < 1.0,
            "at the Snuffing he goes home, not to {target:?}"
        ),
        other => panic!("expected Travel home, got {other:?}"),
    }
}

/// Rung 3's invariant — *a famished actor with food in hand always eats it, at
/// any hour* (`03_hunger.md` §3) — survives the curfew rung standing above it.
/// The night has no hearth (the refill is gated on `is_meal_office`), so a mason
/// who carried supper home and crossed famished in the small hours must be able
/// to eat the loaf at his own door instead of holding it until the Kindling. The
/// curfew keeps everything it is for: the same famished holder out in the street
/// is still walked home first, with the excuse-yourself pressure, and the meal is
/// a standing act that lays no route.
#[test]
fn a_famished_holder_eats_at_his_own_door_through_the_curfew() {
    let (mut round, mut world, nav, id) = seed_hamel();
    let home = round.people[&id].home.expect("housed");
    assert!(
        !round.people[&id].curfew_exempt,
        "a mason keeps no night post"
    );
    let loaf = ItemId::from_raw("supper_loaf");
    world.add_item(Item::stack(loaf.clone(), "loaf", 1));
    {
        let character = world.characters.get_mut(&id).expect("enrolled");
        character.state.holds.push(loaf.clone());
        character.state.needs.hunger = HUNGER_FAMISHED / 2.0;
    }

    // Out in the street at the Snuffing, the curfew still owns him — the loaf
    // does not buy him the right to stand about eating in the watch's way.
    let (street, pressure) = decide(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    );
    assert!(
        matches!(street, Decision::Travel(target) if target.distance(home) < 1.0),
        "the famished holder is still sent home at curfew, got {street:?}"
    );
    assert_eq!(
        pressure,
        Some(CURFEW_PRESSURE),
        "and still gets his one turn to excuse himself"
    );

    // At his door, though, standing still is all the curfew asks — so he eats.
    world
        .characters
        .get_mut(&id)
        .expect("enrolled")
        .state
        .position_m = home;
    match decide_only(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
        Decision::EatHeld(item) => assert_eq!(item, loaf, "he eats the loaf he carried home"),
        other => panic!("famished at his own door, he eats what he holds, got {other:?}"),
    }
    apply_decision(&mut round, &mut world, &nav, &id, Decision::EatHeld(loaf));
    assert!(
        world.characters[&id].needs().hunger > HUNGER_FAMISHED,
        "the supper feeds him"
    );
    assert!(
        !world.characters[&id].is_walking(),
        "and it never took a step out of the door"
    );

    // Empty-handed and famished again, the same door at the same hour is the
    // plain curfew Stay — the rung below only ever borrowed the one branch.
    world
        .characters
        .get_mut(&id)
        .expect("enrolled")
        .state
        .needs
        .hunger = HUNGER_FAMISHED / 2.0;
    match decide_only(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
        Decision::Stay => {}
        other => panic!("with nothing to eat he stays in for the night, got {other:?}"),
    }
}

#[test]
fn a_night_trade_is_not_sent_home_by_curfew() {
    // A tavern worker (curfew-exempt) keeps their post at the Snuffing.
    let nav = nav();
    let id = ActorId::from_raw("tapster_x");
    let mut world = base_world();
    world.add_character(person(
        "tapster_x",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("tavern_worker"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    assert!(
        round.people[&id].curfew_exempt,
        "the tavern archetype is curfew-exempt"
    );
    // At the Snuffing the tavern's Snuffing leg is active — they work, not sleep.
    match decide_only(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
        Decision::Travel(_) | Decision::Stay | Decision::Wander(_) => {}
        Decision::ApproachWell => panic!("a tavern worker draws no water here"),
        Decision::ApproachStall(_) => panic!("no food stall is in reach of this tavern worker"),
        Decision::TravelIntent(_) => panic!("no go_to intent was issued"),
        Decision::SeekShelter(_) => panic!("clear weather creates no shelter intent"),
        Decision::WeatherPause => panic!("no lightning was observed"),
        Decision::EatHeld(_) => panic!("a well-fed tavern worker holds no food to eat"),
        Decision::WalkToLamp(_) | Decision::LightLamp(_) => {
            panic!("a tavern worker carries no taper")
        }
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
    world.add_character(person(
        "aq7ld",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("anchoress"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    let person = round.people.get(&id).expect("the anchoress is enrolled");
    assert!(
        person.curfew_exempt,
        "her route override keeps curfew_exempt"
    );
    assert!(
        person.home.is_none(),
        "the anchorhold is a cell, not a homes.json house"
    );
    assert!(
        person.legs.is_empty(),
        "her Round has zero legs (`04_the_round.md` §1)"
    );
    assert!(
        person.source.is_none(),
        "she is no water drawer; thirst never moves her"
    );
    // At the Snuffing the ladder leaves her exactly where she stands.
    match decide_only(
        &round,
        &world,
        &nav,
        &id,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
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
    world.add_character(person(
        "aq7ld",
        spawn,
        Some("anchoress"),
        Significance::Major,
    ));
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
    assert_eq!(
        round.people[&id].phase,
        Phase::Idle,
        "she never even sets off"
    );
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
    world.add_character(person(
        "b4hst",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
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
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        now,
        &player(),
        &BTreeSet::new(),
    );
    assert_eq!(round.people[&id].phase, Phase::Travelling);
    assert_eq!(
        round.people[&id].travel_target,
        Some(lodge),
        "he is bound for the lodge"
    );

    // A few strides along: genuinely mid-journey, nowhere near either anchor.
    for _ in 0..10 {
        world.step_movement(0.2, &nav, None);
    }
    assert!(
        world.characters[&id].is_walking(),
        "still on the way to the lodge"
    );
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
    assert!(
        diverted,
        "the traveller turned home at curfew instead of finishing the lodge leg"
    );
    assert_eq!(round.people[&id].phase, Phase::Travelling);
    assert!(
        world.characters[&id].is_walking(),
        "he walks home rather than standing in the street"
    );
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
    world.add_character(person(
        "a2gpk",
        Vec3::new(89.0, WALK_Y, 36.0),
        Some("domestic_servant"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    let person = round.people.get(&servant).expect("enrolled");
    assert!(person.home.is_some(), "a2gpk is housed");
    assert!(person.source.is_some(), "and bound to a staffed well");

    world
        .characters
        .get_mut(&servant)
        .unwrap()
        .state
        .needs
        .thirst = 0.0;
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
    match decide_only(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
        Decision::Travel(target) => assert!(
            target.distance(home) < 1.0,
            "at the Snuffing a parched drawer heads home, not to {target:?}"
        ),
        Decision::Stay => panic!("away from home, they must walk there, not stand"),
        other => panic!("expected Travel home, got {other:?}"),
    }
    // And deeper still, at the Watch, the same.
    match decide_only(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Watch,
        Weekday::Bellday,
    ) {
        Decision::Travel(_) => {}
        other => panic!("the Watch is still curfew; expected Travel home, got {other:?}"),
    }
}

/// Once the curfew lifts at the Kindling, the night's thirst sends the drawer
/// straight to the well.
#[test]
fn a_parched_drawer_heads_for_the_well_once_curfew_lifts() {
    let (round, world, nav, servant) = seed_parched_servant();
    match decide_only(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Kindling,
        Weekday::Bellday,
    ) {
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

    world
        .characters
        .get_mut(&servant)
        .unwrap()
        .state
        .needs
        .thirst = 0.0;
    match decide_only(
        &round,
        &world,
        &nav,
        &servant,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
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
    let coswalds = nav
        .place("Coswald's Yard")
        .expect("Coswald's Yard is a nav place")
        .node;
    let post = nav.node_point(coswalds);

    let mut world = base_world();
    for n in 0..3 {
        // Majors, so they are enrolled rather than pinned as well-keepers.
        world.add_character(person(
            &format!("mason{n}"),
            post,
            Some("mason"),
            Significance::Major,
        ));
    }
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let census = round.census(&world, &clock, 0.0);
    assert_eq!(census.total, 3);
    assert_eq!(
        census.at_post, 3,
        "all three stand at their post at the Kindling"
    );
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
    let ford = nav
        .place("Ford Well")
        .expect("Ford Well is a nav place")
        .node;
    let curb = nav.node_point(ford);
    let hop = nav.adjacency()[ford]
        .first()
        .expect("the well node has a neighbour")
        .to;
    let home = nav.node_point(hop);

    let mut world = base_world();
    world.add_character(person("keeper", curb, Some("mason"), Significance::Ambient));
    world.add_character(person(
        "servant",
        home,
        Some(HOUSEHOLD_OCCUPATIONS[0]),
        Significance::Ambient,
    ));

    let mut round = Round::new();
    let diagnostics = round.seed(&mut world, &nav, 0.0, &clock);
    assert!(
        round
            .sources()
            .iter()
            .any(|source| source.name == "Ford Well" && source.keeper.is_some()),
        "Ford Well was staffed: {diagnostics:?}"
    );
    assert_eq!(
        world.characters[&ActorId::from_raw("keeper")].position_m(),
        curb
    );

    let servant = ActorId::from_raw("servant");
    world
        .characters
        .get_mut(&servant)
        .unwrap()
        .state
        .needs
        .thirst = 0.0;

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
            if event.event_type == EventType::Sound
                && event.sound_id.as_deref() == Some("draw_water")
            {
                windlass_events += 1;
                assert!(
                    event.actor_id.is_none(),
                    "the windlass is a world sound, never attributed"
                );
                assert!(
                    event.witness_ids.is_empty(),
                    "a world sound has no witnesses to nudge"
                );
            }
        }
        if round.is_drawing_at("Ford Well") {
            drew = true;
        }
        if world.characters[&servant]
            .recent_history()
            .iter()
            .any(|line| line.contains("drew water"))
        {
            remembered = true;
        }
        if drew && world.characters[&servant].needs().thirst >= THIRST_MAX - 1.0 {
            drew_at_max = true;
        }
        // Home again: back within a stride of where they started, after drawing.
        // (The daily-round rung may set off again afterwards; reaching home is
        // what proves the water errand walked them back.)
        if drew_at_max && world.characters[&servant].position_m().distance(home) < 2.0 && now > 5.0
        {
            went_home = true;
            break;
        }
    }

    assert!(drew, "the servant reached the front of the queue and drew");
    assert!(
        windlass_events > 0,
        "the well's windlass was emitted as a world sound"
    );
    assert!(
        remembered,
        "the drawer remembers drawing, so they can be asked about it"
    );
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
        food: None,
        phase: Phase::Idle,
        travel_target: None,
        travel_for_intent: false,
        next_decision: 0.0,
        epoch: 0,
        evening_seed: None,
        excused: false,
    };
    for (id, household) in [
        ("trade_a", false),
        ("house_a", true),
        ("trade_b", false),
        ("house_b", true),
    ] {
        round
            .people
            .insert(ActorId::from_raw(id), townsperson(household));
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
        food: None,
        phase: Phase::Drawing,
        travel_target: None,
        travel_for_intent: false,
        next_decision: 0.0,
        epoch: 0,
        evening_seed: None,
        excused: false,
    };
    // Household: water for the home, even while the round leg says the shop.
    assert_eq!(
        delivery_point(&drawer(true), Office::Dayspring, Weekday::Bellday),
        home
    );
    // Trade: water for the workshop the current leg names.
    assert_eq!(
        delivery_point(&drawer(false), Office::Dayspring, Weekday::Bellday),
        shop
    );
    // At night the non-exempt carry it home; the curfew rung agrees on arrival.
    assert_eq!(
        delivery_point(&drawer(false), Office::Snuffing, Weekday::Bellday),
        home
    );
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
        assert!(
            !person.is_household,
            "a cloth worker draws with a trade vessel"
        );
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

    world
        .characters
        .get_mut(&fuller)
        .unwrap()
        .state
        .needs
        .thirst = 0.0;

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
    world.add_character(person(
        "keeper",
        nav.node_point(ford),
        Some("mason"),
        Significance::Ambient,
    ));
    world.add_character(person(
        "servant",
        nav.node_point(nav.adjacency()[ford][0].to),
        Some(HOUSEHOLD_OCCUPATIONS[0]),
        Significance::Ambient,
    ));

    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let servant = ActorId::from_raw("servant");
    world
        .characters
        .get_mut(&servant)
        .unwrap()
        .state
        .needs
        .thirst = THIRST_MAX;

    let one_game_hour = 3600.0 / 24.0;
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        one_game_hour,
        &player(),
        &BTreeSet::new(),
    );
    let expected = THIRST_MAX - 3600.0 * crate::THIRST_DECAY_PER_GAME_SECOND;
    let thirst = world.characters[&servant].needs().thirst;
    assert!(
        (thirst - expected).abs() < 1.0,
        "thirst {thirst} decayed to ~{expected} over one game hour"
    );
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
    world.add_character(person(
        "stranger",
        curb,
        Some("mason"),
        Significance::Ambient,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let keeper = ActorId::from_raw("stranger");
    assert!(
        round
            .sources()
            .iter()
            .any(|source| source.keeper.as_ref() == Some(&keeper)),
        "the ambient at the curb keeps the well"
    );
    let person = round
        .people
        .get(&keeper)
        .expect("the keeper is enrolled in the round");
    assert!(
        person.source.is_none(),
        "a keeper works the curb, never queues at it"
    );
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
        round
            .sources()
            .iter()
            .any(|source| source.keeper.as_ref() == Some(&keeper)),
        "the ambient at the curb keeps the well"
    );
    let home = round.people[&keeper].home.expect("a2gpk is housed");
    assert!(
        !round.people[&keeper].curfew_exempt,
        "keeping a well is not a night trade"
    );

    // At the Snuffing the curfew rung sends the keeper home.
    match decide_only(
        &round,
        &world,
        &nav,
        &keeper,
        0,
        Office::Snuffing,
        Weekday::Bellday,
    ) {
        Decision::Travel(target) => assert!(
            target.distance(home) < 1.0,
            "at the Snuffing the keeper heads home, not to {target:?}"
        ),
        other => panic!("expected Travel home, got {other:?}"),
    }

    // Morning: from their own doorstep, the round rung walks them back to the curb.
    world.characters.get_mut(&keeper).unwrap().state.position_m = home;
    match decide_only(
        &round,
        &world,
        &nav,
        &keeper,
        0,
        Office::Kindling,
        Weekday::Bellday,
    ) {
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
    world.add_character(person(
        "stranger",
        curb,
        Some("mason"),
        Significance::Ambient,
    ));
    world.add_character(person(
        "p0012",
        Vec3::new(42.5, WALK_Y, 142.5),
        Some("market_seller"),
        Significance::Minor,
    ));
    world.add_character(person(
        "mason_a",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    assert_eq!(
        round.enrolled(),
        3,
        "keeper, pacer and worker are all enrolled"
    );
    assert_eq!(
        round.census(&world, &clock, 0.0).total,
        3,
        "and the census counts all three"
    );

    // The pacer follows the ordinary market-trader round, not a scripted ping-pong.
    let pacer = ActorId::from_raw("p0012");
    assert!(
        world.characters[&pacer]
            .state
            .movement
            .as_ref()
            .is_none_or(|movement| movement.patrol.is_none()),
        "no permanent patrol is scripted onto p0012"
    );
    assert!(
        active_leg(
            &round.people[&pacer].legs,
            Office::Dayspring,
            Weekday::Second
        )
        .is_some(),
        "p0012 has an ordinary working day"
    );
}

/// The deterministic decision hash gives the same city every run.
#[test]
fn the_decision_hash_is_stable() {
    let id = ActorId::from_raw("servant");
    assert_eq!(
        hash01("round_decision", &id, 3),
        hash01("round_decision", &id, 3)
    );
    assert_ne!(
        hash01("round_decision", &id, 3),
        hash01("round_decision", &id, 4)
    );
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
        world.step_movement(0.5, nav, None);
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
    world.add_character(person(
        "a2gpk",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("domestic_servant"),
        Significance::Minor,
    ));
    let mut friend = person(
        "frnd1",
        Vec3::new(2.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Minor,
    );
    friend.state.knows.insert(ActorId::from_raw("a2gpk"));
    world.add_character(friend);
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    assert!(!world.places.is_empty(), "the registry reached the world");
    let coarse: Vec<&str> = world
        .places
        .coarse()
        .map(|entry| entry.name.as_str())
        .collect();
    assert!(
        coarse.contains(&"The Gradine") && coarse.contains(&"Reed Ward"),
        "{coarse:?}"
    );

    let servant = &world.characters[&ActorId::from_raw("a2gpk")];
    for entry in world.places.coarse() {
        assert!(
            servant.state.places_known.contains(&entry.id),
            "everyone holds the coarse handle for {}",
            entry.name
        );
    }
    let home = world
        .places
        .home_of(&ActorId::from_raw("a2gpk"))
        .expect("a2gpk is housed");
    assert_eq!(home.name, "A2GPK's house");
    assert!(
        servant.state.places_known.contains(&home.id),
        "you know your own house"
    );
    // The Fabric-ward test lore puts both in Fabric Ward: its places are theirs.
    let ford_well = world.places.named("Ford Well").expect("Ford Well is baked");
    assert_eq!(ford_well.ward.as_deref(), Some("fabric"));
    assert!(
        servant.state.places_known.contains(&ford_well.id),
        "own-ward places are known"
    );

    let friend = &world.characters[&ActorId::from_raw("frnd1")];
    assert!(
        friend.state.places_known.contains(&home.id),
        "knowing somebody means knowing the way to their door"
    );
    assert!(
        !servant.state.places_known.contains(
            &world
                .places
                .home_of(&ActorId::from_raw("frnd1"))
                .map(|entry| entry.id.clone())
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
    let line = apply_action(
        &mut world,
        &id,
        "go_to",
        &json!({"place_id": target.as_str()}),
    )
    .unwrap();
    assert_eq!(line, "WALKR sets off for The Gradine");
    assert!(
        world.characters[&id]
            .recent_history()
            .iter()
            .any(|line| line == "You set off for The Gradine."),
        "the walker remembers their own errand"
    );

    // "Sets off" means now: the very first tick lays the walk — no ladder
    // cadence beat between the reply and the first stride.
    tick_collect(&mut round, &mut world, &nav, &clock, 0.1);
    assert!(
        world.characters[&id].is_walking(),
        "a fresh errand walks the tick the round first sees it"
    );

    let gradine = world.places.named("The Gradine").unwrap().point;
    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.1, 2000, |world| {
        world.characters[&id].state.intent.is_none()
    });
    let walker = &world.characters[&id];
    assert!(
        walker.position_m().distance(gradine) <= PLACE_ARRIVE_RADIUS_M + 0.5,
        "the walker stands at the Gradine, not {:?}",
        walker.position_m()
    );
    assert!(
        walker
            .inbox()
            .iter()
            .any(|line| line == "You have arrived at The Gradine."),
        "arrival is a percept: {:?}",
        walker.inbox()
    );
    assert_eq!(
        nudges,
        std::slice::from_ref(&id),
        "arrival granted exactly one priority nudge"
    );
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
    apply_action(
        &mut world,
        &id,
        "go_to",
        &json!({"place_id": gradine.as_str()}),
    )
    .unwrap();
    apply_action(
        &mut world,
        &id,
        "go_to",
        &json!({"place_id": lanthorn.as_str()}),
    )
    .unwrap();
    match &world.characters[&id].state.intent {
        Some(TravelIntent {
            target: IntentTarget::Place { place_id, .. },
            ..
        }) => {
            assert_eq!(place_id, &lanthorn, "the second go_to replaced the first")
        }
        other => panic!("expected a place intent, got {other:?}"),
    }

    // Let the ladder set the walk, then stop: the walk halts, no percept lands.
    let mut now = 0.0;
    for _ in 0..20 {
        now += 0.5;
        world.step_movement(0.5, &nav, None);
        tick_collect(&mut round, &mut world, &nav, &clock, now);
        if world.characters[&id].is_walking() {
            break;
        }
    }
    assert!(
        world.characters[&id].is_walking(),
        "the errand walk is under way"
    );
    let inbox_before = world.characters[&id].inbox().len();
    apply_action(&mut world, &id, "stop", &json!({})).unwrap();
    now += 0.5;
    tick_collect(&mut round, &mut world, &nav, &clock, now);
    assert!(world.characters[&id].state.intent.is_none());
    assert!(
        !world.characters[&id].is_walking(),
        "stop halted the intent walk"
    );
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
    apply_action(
        &mut world,
        &id,
        "go_to",
        &json!({"place_id": tallage.as_str()}),
    )
    .unwrap();
    // Shrink the budget so the errand cannot possibly finish.
    world
        .characters
        .get_mut(&id)
        .unwrap()
        .state
        .intent
        .as_mut()
        .unwrap()
        .budget_seconds = 2.0;

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
    assert_eq!(
        nudges,
        std::slice::from_ref(&id),
        "the lapse granted the nudge"
    );
    assert!(
        !world.characters[&id].is_walking(),
        "the lapsed walk was halted"
    );
}

/// Curfew — a pressing rung — preempts a live errand, and the preemption names
/// its cause: "The curfew turned you back…".
#[test]
fn curfew_preempts_an_errand_with_a_cause_percept() {
    let nav = nav();
    let id = ActorId::from_raw("a2gpk"); // housed by the real bake
    let start = nav.node_point(nav.place("Seraph statue").expect("baked").node);
    let mut world = base_world();
    world.add_character(person(
        "a2gpk",
        start,
        Some("domestic_servant"),
        Significance::Minor,
    ));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
    assert!(round.people[&id].home.is_some(), "the bake houses a2gpk");

    let tallage = world.places.named("The Tallage").unwrap().id.clone();
    apply_action(
        &mut world,
        &id,
        "go_to",
        &json!({"place_id": tallage.as_str()}),
    )
    .unwrap();

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
    assert!(
        nudges.contains(&id),
        "the preempted errand still granted its nudge"
    );
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
    assert!(
        nav.is_walkable(target_at.x, target_at.z),
        "the street midline is walkable"
    );
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
    assert!(
        gap <= PERSON_ARRIVE_RADIUS_M + 0.5,
        "closed to conversation distance, gap {gap}"
    );
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
    world.characters.get_mut(&target).unwrap().state.position_m =
        Vec3::new(start.x + 300.0, WALK_Y, start.z);

    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.5, 200, |world| {
        world.characters[&follower].state.intent.is_none()
    });
    let inbox = world.characters[&follower].inbox();
    assert!(
        inbox
            .iter()
            .any(|line| line == "You have lost sight of a stranger (id targt)."),
        "{inbox:?}"
    );
    assert!(
        inbox.iter().any(|line| line
            == "You reach the spot where you last saw a stranger (id targt), but they are gone."),
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
    apply_action(
        &mut world,
        &id,
        "go_to",
        &json!({"place_id": gradine.as_str()}),
    )
    .unwrap();

    // Held in a warm exchange the whole time: a fresh errand walks on the very
    // first tick — never the exchange's 30 s memory, not even a cadence beat.
    let mut now = 0.1;
    tick(
        &mut round,
        &mut world,
        &nav,
        &clock,
        now,
        &player(),
        &warm(&id),
    );
    assert!(
        world.characters[&id].is_walking(),
        "the errand set off immediately despite the warm exchange"
    );
    assert!(world.characters[&id].state.intent.is_some());

    // A fresh addressed line stops them mid-stride to answer...
    interrupt_for_conversation(&mut round, &mut world, &id);
    assert!(
        !world.characters[&id].is_walking(),
        "they stop for the answer"
    );
    assert!(
        world.characters[&id].state.intent.is_some(),
        "without dropping the errand"
    );

    // ...and, still mid-conversation, the walk resumes on the next cadence.
    let resumed_by = now + LADDER_DECISION_MAX_SECONDS + 1.0;
    while !world.characters[&id].is_walking() {
        now += 0.5;
        assert!(now <= resumed_by, "the interrupted errand never resumed");
        world.step_movement(0.5, &nav, None);
        tick(
            &mut round,
            &mut world,
            &nav,
            &clock,
            now,
            &player(),
            &warm(&id),
        );
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
    teller_character.sheet.lore = person("x", Vec3::ZERO, Some("mason"), Significance::Major)
        .sheet
        .lore
        .clone();
    teller_character.sheet.lore.as_mut().unwrap().planning_ward = PlanningWard::Wallwright;
    world.add_character(teller_character);
    world.add_character(person(
        "askr1",
        Vec3::new(start.x + 3.0, WALK_Y, start.z),
        None,
        Significance::Major,
    ));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let lodge = world
        .places
        .named("The masons' lodge")
        .expect("baked")
        .id
        .clone();
    assert!(
        world.characters[&teller]
            .state
            .places_known
            .contains(&lodge)
    );
    assert!(!world.characters[&asker].state.places_known.contains(&lodge));

    // The asker cannot walk there yet: the id is not theirs to use.
    let error = apply_action(
        &mut world,
        &asker,
        "go_to",
        &json!({"place_id": lodge.as_str()}),
    )
    .unwrap_err();
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
    apply_action(
        &mut world,
        &asker,
        "go_to",
        &json!({"place_id": lodge.as_str()}),
    )
    .unwrap();
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
    world.add_character(person(
        "farbd",
        Vec3::new(start.x + 50.0, WALK_Y, start.z),
        None,
        Significance::Major,
    ));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    for (args, code) in [
        (json!({}), ActionErrorCode::InvalidArguments),
        (
            json!({"place_id": "pl_x", "person": "farbd"}),
            ActionErrorCode::InvalidArguments,
        ),
        (
            json!({"place_id": "pl_no_such"}),
            ActionErrorCode::UnknownPlace,
        ),
        (json!({"person": "ghost"}), ActionErrorCode::UnknownTarget),
        (json!({"person": "walkr"}), ActionErrorCode::SelfTarget),
        // 50 m away: out of you_see, so not a legal follow target.
        (json!({"person": "farbd"}), ActionErrorCode::OutOfRange),
    ] {
        let error = apply_action(&mut world, &id, "go_to", &args).unwrap_err();
        assert_eq!(error.code, code, "go_to {args}");
    }
    assert!(
        world.characters[&id].state.intent.is_none(),
        "no failed go_to left an intent"
    );

    let gradine = world.places.named("The Gradine").unwrap().id.clone();
    for (args, code) in [
        // The teller holds the id, but the target is beyond earshot.
        (
            json!({"person": "farbd", "place_id": gradine.as_str()}),
            ActionErrorCode::OutOfRange,
        ),
        // A handle the teller does not hold cannot be shared.
        (
            json!({"person": "farbd", "place_id": "pl_no_such"}),
            ActionErrorCode::UnknownPlace,
        ),
        (
            json!({"person": "walkr", "place_id": gradine.as_str()}),
            ActionErrorCode::SelfTarget,
        ),
    ] {
        let error = apply_action(&mut world, &id, "tell_way", &args).unwrap_err();
        assert_eq!(error.code, code, "tell_way {args}");
    }
}

/// [`Round::errand_debug`] — the character sheet's read-only errand view —
/// reduces the phase, the well, the queue standing and the walk target.
#[test]
fn errand_debug_reduces_the_phase_the_well_and_the_walk() {
    let ana = ActorId::from_raw("ana11");
    let mut round = Round::new();
    round.sources.push(WaterSource {
        name: "Chain Well".into(),
        draw_point: Vec3::new(5.0, WALK_Y, 5.0),
        draw_sound: "well_windlass",
        keeper: None,
        queue: vec![ActorId::from_raw("first"), ana.clone()],
        serving: None,
        keeper_next_sound: 0.0,
    });
    round.people.insert(
        ana.clone(),
        Townsperson {
            home: None,
            base: Vec3::ZERO,
            legs: Vec::new(),
            leash_m: 6.0,
            curfew_exempt: false,
            source: Some(0),
            is_household: true,
            food: None,
            phase: Phase::Queued,
            travel_target: None,
            travel_for_intent: false,
            next_decision: 0.0,
            epoch: 0,
            evening_seed: None,
            excused: false,
        },
    );

    // Never enrolled → no errand.
    assert_eq!(round.errand_debug(&ActorId::from_raw("ghost")), None);

    // Queued: the well is named and the standing counted (one drawer ahead).
    let queued = round.errand_debug(&ana).expect("enrolled");
    assert_eq!(queued.phase, Phase::Queued);
    assert_eq!(queued.well.as_deref(), Some("Chain Well"));
    assert_eq!(queued.ahead_in_queue, Some(1));
    assert_eq!(queued.walk_target, None);
    assert!(!queued.for_intent);

    // Approaching aims at the draw point; the queue standing no longer applies.
    round.people.get_mut(&ana).unwrap().phase = Phase::Approaching;
    let approaching = round.errand_debug(&ana).expect("enrolled");
    assert_eq!(approaching.walk_target, Some(Vec3::new(5.0, WALK_Y, 5.0)));
    assert_eq!(approaching.ahead_in_queue, None);

    // Travelling for the intent reports the decided target and the flag.
    {
        let person = round.people.get_mut(&ana).unwrap();
        person.phase = Phase::Travelling;
        person.travel_target = Some(Vec3::new(9.0, WALK_Y, 9.0));
        person.travel_for_intent = true;
    }
    let travelling = round.errand_debug(&ana).expect("enrolled");
    assert_eq!(travelling.walk_target, Some(Vec3::new(9.0, WALK_Y, 9.0)));
    assert!(travelling.for_intent);
}

// ----------------------------------------------------------------- M7: lamps

/// The four authored keeper ids and a spawn near each beat, so the test night
/// is spent lighting rather than crossing the fortified maze (adjacent squares
/// are 1.3–2 km apart on foot — the reason the beats exist at all).
const KEEPERS: &[(&str, f64, f64)] = &[
    ("dtbvl", -20.0, 356.0),   // Tobin Vell — the Wickmarket + the Gradine
    ("drhcr", 255.0, 160.0),   // Rohese Crake — Coswald's Yard
    ("p004m", -300.0, 86.0),   // Jos Rusk — the Tallage
    ("p004l", -300.0, -360.0), // Ede Pell — Maren's Green
];

/// The whole lamplighter slice on the committed graph: the seed stands the
/// posts up dark, each square on an authored keeper's beat; the dusk beats
/// light every post but each square's Belwyn's lamp, each act remembered in
/// the keeper's own words; and the Kindling snuffs the set at first light.
#[test]
fn the_lamplighters_dusk_beats_light_the_squares_and_dawn_snuffs_them() {
    let nav = nav();
    let mut world = base_world();
    for &(id, x, z) in KEEPERS {
        // Minor, so the well-keeper draft (ambients only) can never pin one
        // of them to a curb.
        world.add_character(person(
            id,
            Vec3::new(x, WALK_Y, z),
            Some("lamplighter"),
            Significance::Minor,
        ));
    }
    let mut round = Round::new();
    let clock = clock_at(Office::Lamplight);
    round.seed(&mut world, &nav, 0.0, &clock);

    let total = round.lamps().len();
    assert!(
        total >= 15,
        "five squares should carry a healthy ring of posts, got {total}"
    );
    assert!(
        round.lamps().iter().all(|lamp| !lamp.lit),
        "the seed is dark"
    );
    assert!(
        round.lamps().iter().all(|lamp| lamp.keeper.is_some()),
        "every square is on an authored beat"
    );
    let squares: BTreeSet<String> = round
        .lamps()
        .iter()
        .map(|lamp| lamp.square.clone())
        .collect();
    assert_eq!(squares.len(), 5, "all five squares carry posts");
    let revision_dark = round.lamp_revision();
    assert!(
        revision_dark >= 1,
        "the dark seed itself is a publishable revision"
    );

    // The dusk beats: every keeper works their own square(s), so the whole
    // city lights well inside the night.
    let mut now = 0.0;
    let dt = 0.5;
    let goal = total - squares.len(); // one Belwyn's lamp per square stays dark
    let lit = loop {
        beat(&mut round, &mut world, &nav, &clock, now, dt);
        now += dt;
        let lit = round.lamps().iter().filter(|lamp| lamp.lit).count();
        if lit == goal || clock.at(now).office == Office::Kindling {
            break lit;
        }
    };
    assert_eq!(
        lit, goal,
        "every lamp but each square's Belwyn's burns after the beats (t={now:.0}s)"
    );
    for square in &squares {
        let unlit: Vec<usize> = round
            .lamps()
            .iter()
            .enumerate()
            .filter(|(_, lamp)| lamp.square == *square && !lamp.lit)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            unlit.len(),
            1,
            "{square} keeps exactly one dark lamp — Belwyn's"
        );
    }
    assert!(round.lamp_revision() > revision_dark);
    // Each keeper remembers the act in their own words, so the player who
    // stops one can ask what they are doing (the M3 drawer's pattern).
    for &(id, ..) in KEEPERS {
        assert!(
            world.characters[&ActorId::from_raw(id)]
                .state
                .recent_history
                .iter()
                .any(|line| line.starts_with("You light the lamp at ")),
            "{id} remembers lighting"
        );
    }

    // First light: the Kindling snuffs the whole set at once.
    while clock.at(now).office != Office::Kindling {
        now += dt;
    }
    beat(&mut round, &mut world, &nav, &clock, now, dt);
    assert!(
        round.lamps().iter().all(|lamp| !lamp.lit),
        "the Kindling snuffs every lamp"
    );
}

/// Delay a lamplighter with conversation and their whole quarter stays dark
/// longer — the acceptance line of `features/50_cool_suggestions.md` #21. The
/// lamp rung is deferrable, so a warm exchange freezes the beat exactly where
/// it stands. Only Tobin is seeded here, so the frozen count is citywide —
/// which also proves a square whose keeper is missing simply stays dark.
#[test]
fn a_conversation_holds_the_taper() {
    let nav = nav();
    let id = ActorId::from_raw("dtbvl");
    let mut world = base_world();
    world.add_character(person(
        "dtbvl",
        Vec3::new(-20.0, WALK_Y, 356.0),
        Some("lamplighter"),
        Significance::Minor,
    ));
    let mut round = Round::new();
    let clock = clock_at(Office::Lamplight);
    round.seed(&mut world, &nav, 0.0, &clock);
    // His beat is the Wickmarket + the Gradine: 8 posts, minus one Belwyn's
    // per square = 6 to light tonight.
    let beat_size = round
        .lamps()
        .iter()
        .filter(|lamp| lamp.keeper.as_ref() == Some(&id))
        .count();
    assert_eq!(beat_size, 8, "Tobin keeps the Wickmarket and the Gradine");

    // Let the beat begin: at least one post alight, the night still young.
    let mut now = 0.0;
    let dt = 0.5;
    while round.lamps().iter().filter(|lamp| lamp.lit).count() < 2 {
        beat(&mut round, &mut world, &nav, &clock, now, dt);
        now += dt;
        assert!(now < 600.0, "the beat never began");
    }
    let lit_when_stopped = round.lamps().iter().filter(|lamp| lamp.lit).count();
    assert!(lit_when_stopped < 6, "the beat is still under way");

    // Two real minutes of talk: the deferrable rung waits, no lamp is lit.
    for _ in 0..240 {
        world.step_movement(dt, &nav, None);
        tick(
            &mut round,
            &mut world,
            &nav,
            &clock,
            now,
            &player(),
            &warm(&id),
        );
        now += dt;
    }
    assert_eq!(
        round.lamps().iter().filter(|lamp| lamp.lit).count(),
        lit_when_stopped,
        "a held keeper lights nothing — the quarter stays dark longer"
    );

    // The exchange lapses; the beat resumes on its own.
    let mut resumed = false;
    for _ in 0..480 {
        beat(&mut round, &mut world, &nav, &clock, now, dt);
        now += dt;
        if round.lamps().iter().filter(|lamp| lamp.lit).count() > lit_when_stopped {
            resumed = true;
            break;
        }
    }
    assert!(resumed, "the beat resumes once the talk goes cold");
}

/// Wallets and hunger, seeded across the enrolled cast (food & items M2,
/// `02_the_spark_standard.md` §4, `03_hunger.md` §1): every enrolled townsperson
/// carries a spark stack unless the sheet already gave them coin, and their
/// hunger is spread off the deterministic hash (floored at 40) unless the sheet
/// declares them hungry, when they seed low.
#[test]
fn the_round_seeds_wallets_and_hunger() {
    let nav = nav();
    let mut world = base_world();

    // A plain enrolled townsperson: no authored coin, no hunger memory.
    world.add_character(person(
        "wlta",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    ));

    // One who already holds a spark keeps exactly it — no wallet minted.
    let purse = ItemId::from_raw("purse");
    world.add_item(Item::stack(purse.clone(), "spark", 1));
    let mut holder = person(
        "wltb",
        Vec3::new(1.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    );
    holder.state.holds.push(purse.clone());
    world.add_character(holder);

    // One whose memory declares hunger seeds low (the Ilse hook: her static
    // `hungry` condition is dropped, the memory carries the low seed).
    let mut hungry = person(
        "wltc",
        Vec3::new(2.0, WALK_Y, 95.0),
        Some("mason"),
        Significance::Major,
    );
    hungry
        .state
        .memories
        .push("I am very hungry after the long road here".to_string());
    world.add_character(hungry);

    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));

    // A fresh spark wallet, in 2..=7, held and present in the world.
    let a = &world.characters[&ActorId::from_raw("wlta")];
    let wallet = ItemId::from_raw("w_wlta");
    assert!(
        a.holds().contains(&wallet),
        "a coinless townsperson is given a wallet"
    );
    let sparks = world.items[&wallet].quantity;
    assert!(
        (2..=7).contains(&sparks),
        "the seeded wallet is 2..=7, got {sparks}"
    );
    assert!(
        a.needs().hunger >= HUNGER_SEED_FLOOR,
        "hunger seeds no lower than the floor"
    );

    // The authored purse stands alone — no second same-stuff stack minted.
    let b = &world.characters[&ActorId::from_raw("wltb")];
    assert_eq!(b.holds(), [purse], "an authored purse is kept, not doubled");
    assert!(
        !world.items.contains_key(&ItemId::from_raw("w_wltb")),
        "no wallet for a coin-holder"
    );

    // The declared-hungry actor seeds low, and still gets a wallet.
    let c = &world.characters[&ActorId::from_raw("wltc")];
    assert_eq!(
        c.needs().hunger,
        HUNGER_SEED_DECLARED_HUNGRY,
        "a hunger memory seeds low"
    );
    assert!(
        c.holds().contains(&ItemId::from_raw("w_wltc")),
        "the hungry still carry a purse"
    );
}

/// The tavern hearth (food & items M2, `03_hunger.md` §4): a tavern trade
/// (curfew-exempt, never home) standing at a tavern is fed during a meal office
/// exactly as a diner at home — and *only* then. Without this branch the ~39
/// tavern trades, who work straight through every meal office, decay to nothing
/// forever, which was the review bug.
#[test]
fn the_tavern_hearth_feeds_its_trade_only_at_a_meal_office() {
    // Stand a famished tavern worker at a resolved tavern node and let a slice of
    // game time pass under `office`; return their hunger afterward.
    let hunger_at = |office: Office| -> f64 {
        let nav = nav();
        let mut world = base_world();
        world.add_character(person(
            "brew",
            Vec3::new(0.0, WALK_Y, 95.0),
            Some("tavern_worker"),
            Significance::Major,
        ));
        let mut round = Round::new();
        round.seed(&mut world, &nav, 0.0, &clock_at(office));
        assert!(!round.taverns.is_empty(), "the committed graph has taverns");
        let brew = ActorId::from_raw("brew");
        assert!(
            round.people[&brew].home.is_none(),
            "the test worker has no house — only the tavern can feed them"
        );
        let tavern = round.taverns[0];
        {
            let state = &mut world.characters.get_mut(&brew).expect("enrolled").state;
            state.position_m = tavern;
            state.needs.hunger = HUNGER_FAMISHED / 2.0;
        }
        decay_needs(&mut round, &mut world, &clock_at(office), 40.0);
        world.characters[&brew].needs().hunger
    };

    // High Wick — a meal office: the tavern hearth is warm, the trade is fed past
    // famished.
    assert!(
        hunger_at(Office::HighWick) > HUNGER_FAMISHED,
        "the tavern feeds its trade at High Wick"
    );
    // The Kindling — no meal office: even at the tavern the hearth is cold, so
    // they only decay.
    assert!(
        hunger_at(Office::Kindling) < HUNGER_FAMISHED / 2.0,
        "no meal office, no tavern hearth — the trade decays"
    );
}

/// The anchoress, bricked into her cell (`stationary` archetype: zero legs, no
/// homes.json house), has no home hearth and no tavern — so where she stands is
/// her hearth, fed during a meal office, or she starves forever (`03_hunger.md`
/// §4). The legless-actor case the review flagged.
#[test]
fn the_anchoress_is_fed_in_her_cell() {
    let nav = nav();
    let spawn = Vec3::new(194.5, WALK_Y, -92.0); // her authored squint
    let mut world = base_world();
    world.add_character(person(
        "aq7ld",
        spawn,
        Some("anchoress"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::HighWick));
    let id = ActorId::from_raw("aq7ld");
    assert!(
        round.people[&id].legs.is_empty(),
        "the anchoress has zero legs"
    );
    assert!(round.people[&id].home.is_none(), "and no homes.json house");
    world
        .characters
        .get_mut(&id)
        .expect("enrolled")
        .state
        .needs
        .hunger = HUNGER_FAMISHED / 2.0;
    decay_needs(&mut round, &mut world, &clock_at(Office::HighWick), 40.0);
    assert!(
        world.characters[&id].needs().hunger > HUNGER_FAMISHED,
        "her cell is her hearth during a meal office"
    );
}

/// Rung 3 sends a famished actor home to the hearth **only while a meal office is
/// serving** (`03_hunger.md` §3/§4). Outside one the hearth is cold, so the
/// famished worker keeps to the round instead of abandoning the morning city for
/// a dead grate — the census bug the review caught. The `FAMISHED_PRESSURE`
/// percept rides only with the divert.
#[test]
fn famished_diverts_home_only_while_the_hearth_is_serving() {
    let nav = nav();
    let mut world = base_world();
    world.add_character(person(
        "merc",
        Vec3::new(0.0, WALK_Y, 95.0),
        Some("merchant"),
        Significance::Major,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_on(Office::Dayspring, 1));
    let merc = ActorId::from_raw("merc");
    // Housed (a hearth to make for) and famished, standing well away from it.
    let home = Vec3::new(0.0, WALK_Y, 0.0);
    round.people.get_mut(&merc).expect("enrolled").home = Some(home);
    assert!(
        !round.people[&merc].curfew_exempt,
        "a market trader keeps no night post"
    );
    world
        .characters
        .get_mut(&merc)
        .expect("enrolled")
        .state
        .needs
        .hunger = HUNGER_FAMISHED / 3.0;

    // Dawn (Dayspring, no meal office): the hearth is cold — hunger does not march
    // them home; they keep to the round, and no famished pressure is injected.
    let (dawn, dawn_pressure) = decide(
        &round,
        &world,
        &nav,
        &merc,
        0,
        Office::Dayspring,
        Weekday::of_day(1),
    );
    assert_ne!(
        dawn_pressure,
        Some(FAMISHED_PRESSURE),
        "a cold hearth does not march the famished home at dawn"
    );
    assert!(
        !matches!(dawn, Decision::Travel(t) if t == home),
        "the famished worker is not sent home at dawn"
    );

    // High Wick (a meal office): the hearth is warm — now hunger sends them home,
    // with the excuse-yourself pressure.
    let (noon, noon_pressure) = decide(
        &round,
        &world,
        &nav,
        &merc,
        0,
        Office::HighWick,
        Weekday::of_day(1),
    );
    assert!(
        matches!(noon, Decision::Travel(t) if t == home),
        "famished at High Wick makes for the hearth, got {noon:?}"
    );
    assert_eq!(
        noon_pressure,
        Some(FAMISHED_PRESSURE),
        "and injects the famished pressure"
    );
}

// --------------------------------------------------------------------------- //
// The food stalls (M3): binding, restock, the silent purchase, the ledger.
// --------------------------------------------------------------------------- //

/// A stall stocked with three generic loaves, a 6-spark vendor float and a 5-spark
/// buyer queued — the fixture the purchase/restock tests share.
fn bread_stall_world() -> (World, Round, ActorId, ActorId, ItemId) {
    let mut world = base_world();
    let vendor = ActorId::from_raw("baker");
    world.add_character(person(
        "baker",
        Vec3::ZERO,
        Some("baker"),
        Significance::Minor,
    ));
    let stock_id = ItemId::from_raw("fs_baker_0_0");
    world.add_item(Item::stack(stock_id.clone(), "loaf", 3));
    world
        .characters
        .get_mut(&vendor)
        .unwrap()
        .state
        .holds
        .push(stock_id.clone());
    let vendor_wallet = ItemId::from_raw("w_baker");
    world.add_item(Item::stack(vendor_wallet.clone(), "spark", 6));
    world
        .characters
        .get_mut(&vendor)
        .unwrap()
        .state
        .holds
        .push(vendor_wallet);

    let buyer = ActorId::from_raw("buyer");
    world.add_character(person(
        "buyer",
        Vec3::new(2.0, WALK_Y, 0.0),
        None,
        Significance::Minor,
    ));
    let buyer_wallet = ItemId::from_raw("w_buyer");
    world.add_item(Item::stack(buyer_wallet.clone(), "spark", 5));
    world
        .characters
        .get_mut(&buyer)
        .unwrap()
        .state
        .holds
        .push(buyer_wallet);
    world.assert_invariants();

    let mut round = Round::default();
    round.food_trades.insert(
        "bread".into(),
        ResolvedTrade {
            occupations: vec!["baker".into()],
            listings: vec![ItemMatcher::new("loaf")],
            restock: Vec::new(),
            per_serving: None,
        },
    );
    round.stalls.push(FoodStall {
        name: "Bread".into(),
        site: "The Wickmarket".into(),
        pitch: Vec3::ZERO,
        trade: "bread".into(),
        vendor: Some(vendor.clone()),
        queue: vec![buyer.clone()],
        serving: None,
        preferred: None,
        open: OpenSpec {
            offices: vec![Office::HighWick],
            weekdays: None,
        },
        cry_next: 0.0,
    });
    (world, round, vendor, buyer, stock_id)
}

/// The silent purchase is an atomic swap that conserves sparks and takes exactly
/// one unit off the board (`04` §5). The stock-left figure the trace prints is
/// the real remaining count — the invariant behind the `[food]` sale line.
#[test]
fn a_purchase_is_an_atomic_swap_that_conserves_the_board() {
    let (mut world, mut round, vendor, buyer, stock_id) = bread_stall_world();
    let before = world.wallet_sparks(&buyer) + world.wallet_sparks(&vendor);

    let sale = try_purchase(&mut round, &mut world, 0, &buyer).expect("the buyer affords a loaf");
    assert_eq!(sale.price, 2, "the loaf is list price");
    assert_eq!(sale.item_display, "loaf");
    assert_eq!(
        sale.stock_left, 2,
        "one of three loaves left the board — no phantom unit"
    );

    assert_eq!(
        world.wallet_sparks(&buyer) + world.wallet_sparks(&vendor),
        before,
        "a purchase mints and burns nothing"
    );
    assert_eq!(world.wallet_sparks(&buyer), 3, "the buyer paid 2");
    assert_eq!(world.wallet_sparks(&vendor), 8, "the vendor took 2");
    let loaves: u32 = world.characters[&buyer]
        .holds()
        .iter()
        .filter_map(|id| world.items.get(id))
        .filter(|item| item.kind.as_str() == "loaf")
        .map(|item| item.quantity)
        .sum();
    assert_eq!(loaves, 1, "the buyer carries the loaf");
    assert_eq!(
        world.items.get(&stock_id).map(|item| item.quantity),
        Some(2),
        "the vendor's stack fell by one"
    );
    world.assert_invariants();
}

/// A buyer who cannot pay is a graceful no-sale: nothing moves, the board is
/// untouched, and the buyer keeps their coin.
#[test]
fn a_broke_buyer_is_a_no_sale() {
    let (mut world, mut round, _vendor, buyer, stock_id) = bread_stall_world();
    // Spend the buyer down to a single spark — a 2-spark loaf is out of reach.
    set_wallet(&mut world, &buyer, 1);
    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_none(),
        "a spark cannot buy a 2-spark loaf"
    );
    assert_eq!(
        world.items.get(&stock_id).map(|item| item.quantity),
        Some(3),
        "the board is untouched"
    );
    assert_eq!(world.wallet_sparks(&buyer), 1, "the buyer keeps their coin");
}

/// The famished buyer takes the cheapest edible they can afford: a lone spark
/// buys the herring, never the 2-spark loaf beside it (Ilse's exact arithmetic).
#[test]
fn a_purchase_takes_the_cheapest_affordable() {
    let (mut world, mut round, vendor, buyer, _rye) = bread_stall_world();
    // Add a 1-spark herring to the same board and pin the buyer to a single spark.
    let herring = ItemId::from_raw("fs_baker_0_1");
    world.add_item(Item::stack(herring.clone(), "herring", 4));
    world
        .characters
        .get_mut(&vendor)
        .unwrap()
        .state
        .holds
        .push(herring.clone());
    round
        .food_trades
        .get_mut("bread")
        .unwrap()
        .listings
        .push(ItemMatcher::new("herring"));
    set_wallet(&mut world, &buyer, 1);

    let sale = try_purchase(&mut round, &mut world, 0, &buyer).expect("a spark buys a herring");
    assert_eq!(sale.item_display, "herring");
    assert_eq!(sale.price, 1);
    assert_eq!(
        world.items.get(&herring).map(|item| item.quantity),
        Some(3),
        "a herring left the board"
    );
    assert_eq!(
        world.wallet_sparks(&buyer),
        0,
        "the buyer spent their last spark"
    );
}

/// The pot never depletes: it conjures a fresh bowl per sale, and the stall's
/// stock ledger stays empty.
#[test]
fn the_pot_conjures_a_bowl_per_serving() {
    let (mut world, mut round, vendor, buyer, _rye) = bread_stall_world();
    // Turn the stall into a pot: a per-serving stew, no stock stacks.
    round.food_trades.insert(
        "pot".into(),
        ResolvedTrade {
            occupations: vec!["cook".into()],
            listings: vec![ItemMatcher::new("stew")],
            restock: Vec::new(),
            per_serving: Some("stew".into()),
        },
    );
    round.stalls[0].trade = "pot".into();
    // strip the leftover loaf stack so only the pot remains
    let stock = ItemId::from_raw("fs_baker_0_0");
    world.items.remove(&stock);
    world
        .characters
        .get_mut(&vendor)
        .unwrap()
        .state
        .holds
        .retain(|id| id != &stock);

    let sale = try_purchase(&mut round, &mut world, 0, &buyer).expect("a bowl of stew");
    assert_eq!(sale.item_display, "bowl of stew");
    assert_eq!(sale.price, 2);
    assert_eq!(sale.stock_left, 0, "the pot keeps no board");
    let bowls = world.characters[&buyer]
        .holds()
        .iter()
        .filter_map(|id| world.items.get(id))
        .filter(|item| item.kind.as_str() == "stew")
        .count();
    assert_eq!(bowls, 1, "the buyer holds one bowl");
}

/// A famished vendor prefers personal food, but may eat a live listed unit when
/// it is their only available meal. M5 removed the parallel "board stock" list:
/// commercial stock is ordinary, provenance-carrying inventory.
#[test]
fn a_vendor_prefers_personal_food_to_live_listed_stock() {
    let (world, mut round, vendor, _buyer, _stock) = bread_stall_world();
    // The listed loaf is still real food when it is the only edible held.
    let held = held_edible(&round, &world, &world.characters[&vendor]);
    assert_eq!(held, Some(ItemId::from_raw("fs_baker_0_0")));
    // A personal, unlisted meal sorts ahead of commercially listed inventory.
    let mut world = world;
    let personal = ItemId::from_raw("mine1");
    world.add_item(Item::new(personal.clone(), "herring"));
    world
        .characters
        .get_mut(&vendor)
        .unwrap()
        .state
        .holds
        .push(personal.clone());
    round.stalls[0].vendor = Some(vendor.clone());
    assert_eq!(
        held_edible(&round, &world, &world.characters[&vendor]),
        Some(personal)
    );
}

/// Legacy restock replaces only quantities carrying that restock provenance.
/// Persistent supply-chain stock (the loaf here) survives Kindling, and a
/// repeated restock does not accumulate its own herring share.
#[test]
fn legacy_restock_preserves_persistent_stock_and_does_not_accumulate() {
    let (mut world, mut round, vendor, _buyer, loaf) = bread_stall_world();
    let trade = round.food_trades.get_mut("bread").unwrap();
    trade.listings.push(ItemMatcher::new("herring"));
    trade.restock.push(StockSpec {
        kind: "herring".into(),
        metadata: BTreeMap::new(),
        quantity: 2,
    });

    round.restock(&mut world, 2);
    assert_eq!(
        world.items[&loaf].quantity, 3,
        "persistent loaves survive legacy restock"
    );
    assert_eq!(
        world.held_quantity(&vendor, &ItemMatcher::new("herring")),
        2
    );
    round.restock(&mut world, 3);
    assert_eq!(world.items[&loaf].quantity, 3);
    assert_eq!(
        world.held_quantity(&vendor, &ItemMatcher::new("herring")),
        2
    );
    world.assert_invariants();
}

/// Regression (`bind_vendors` two-phase, `05` §3): when a vendor is reassigned
/// from a higher-index stall to a lower-index one in a single pass — a fish
/// trader moved off Maren's Green onto Coswald's on a Highmarket day — their
/// freshly-written `you_sell` must survive the departed stall's clear of its now
/// stale previous vendor. The old per-stall interleave wiped it, leaving an
/// actively-bound vendor with no price list. A genuinely-unbound former vendor
/// is still cleared in the same pass.
#[test]
fn bind_vendors_keeps_you_sell_when_a_vendor_moves_to_a_lower_index_stall() {
    let mut world = base_world();
    // Three fish traders: V nearest the low-index stall, Z its former keeper a
    // little farther, U the keeper of the high-index stall.
    world.add_character(person(
        "v0000",
        Vec3::new(1.0, WALK_Y, 0.0),
        Some("fish_trader"),
        Significance::Minor,
    ));
    world.add_character(person(
        "z0000",
        Vec3::new(5.0, WALK_Y, 0.0),
        Some("fish_trader"),
        Significance::Minor,
    ));
    world.add_character(person(
        "u0000",
        Vec3::new(99.0, WALK_Y, 100.0),
        Some("fish_trader"),
        Significance::Minor,
    ));

    let mut round = Round::default();
    round.food_trades.insert(
        "fish".into(),
        ResolvedTrade {
            occupations: vec!["fish_trader".into()],
            listings: vec![ItemMatcher::new("herring"), ItemMatcher::new("smoked_eel")],
            restock: Vec::new(),
            per_serving: None,
        },
    );

    // Enrol each with legs that route them to the stall sites (the eligibility
    // key `pick_vendor` reads) and a base that fixes the nearest-vendor tie-break.
    let mut enrol = |id: &str, base: Vec3, sites: &[&str]| {
        round.people.insert(
            ActorId::from_raw(id),
            Townsperson {
                home: None,
                base,
                legs: sites
                    .iter()
                    .map(|site| leg(Office::HighWick, site, None))
                    .collect(),
                leash_m: 2.0,
                curfew_exempt: false,
                source: None,
                is_household: false,
                food: None,
                phase: Phase::Idle,
                travel_target: None,
                travel_for_intent: false,
                next_decision: 0.0,
                epoch: 0,
                evening_seed: None,
                excused: false,
            },
        );
    };
    enrol(
        "v0000",
        Vec3::new(1.0, WALK_Y, 0.0),
        &["SITE_LOW", "SITE_HIGH"],
    );
    enrol("z0000", Vec3::new(5.0, WALK_Y, 0.0), &["SITE_LOW"]);
    enrol("u0000", Vec3::new(99.0, WALK_Y, 100.0), &["SITE_HIGH"]);

    let stall = |site: &str, pitch: Vec3, weekdays: Option<Vec<Weekday>>, vendor: &str| FoodStall {
        name: site.into(),
        site: site.into(),
        pitch,
        trade: "fish".into(),
        vendor: Some(ActorId::from_raw(vendor)),
        queue: Vec::new(),
        serving: None,
        preferred: None,
        open: OpenSpec {
            offices: vec![Office::HighWick],
            weekdays,
        },
        cry_next: 0.0,
    };
    // idx 0 (low): Highmarket-only, former keeper Z. idx 1 (high): every day,
    // former keeper V — the vendor about to move down onto idx 0.
    round.stalls.push(stall(
        "SITE_LOW",
        Vec3::new(0.0, WALK_Y, 0.0),
        Some(vec![Weekday::Highmarket]),
        "z0000",
    ));
    round.stalls.push(stall(
        "SITE_HIGH",
        Vec3::new(100.0, WALK_Y, 100.0),
        None,
        "v0000",
    ));

    // The price lists the previous pass left on the two former vendors.
    for id in ["v0000", "z0000"] {
        world
            .characters
            .get_mut(&ActorId::from_raw(id))
            .unwrap()
            .state
            .you_sell = vec![VendorListing {
            name: "herring".into(),
            price_sparks: 1,
        }];
    }

    round.bind_vendors(&mut world, Weekday::Highmarket);

    // V moved idx1 → idx0; U took idx1; Z is bumped from idx0.
    assert_eq!(
        round.stalls[0].vendor.as_ref(),
        Some(&ActorId::from_raw("v0000"))
    );
    assert_eq!(
        round.stalls[1].vendor.as_ref(),
        Some(&ActorId::from_raw("u0000"))
    );
    // The bug: V's you_sell is wiped by idx1's clear of its stale previous vendor.
    assert!(
        !world.characters[&ActorId::from_raw("v0000")]
            .state
            .you_sell
            .is_empty(),
        "the reassigned vendor keeps its price list across the same pass"
    );
    // The genuinely-unbound former vendor is cleared in the same pass.
    assert!(
        world.characters[&ActorId::from_raw("z0000")]
            .state
            .you_sell
            .is_empty(),
        "the bumped former vendor's price list is cleared"
    );
    // The new keeper of idx1 gets the fish list off the catalog.
    assert_eq!(
        world.characters[&ActorId::from_raw("u0000")].state.you_sell,
        vec![
            VendorListing {
                name: "herring".into(),
                price_sparks: 1
            },
            VendorListing {
                name: "smoked eel".into(),
                price_sparks: 3
            },
        ]
    );
}

/// End to end on the real graph: a famished passer-by at the Wickmarket seeks the
/// provisions stall, joins its queue, buys a herring and eats it — the whole rung-3 → walk
/// → queue → silent purchase → eat chain, with the self-percept both parties can
/// be asked about.
#[test]
fn a_famished_passerby_buys_and_eats_at_the_wickmarket() {
    let nav = nav();
    let clock = clock_on(Office::HighWick, 2); // Highmarket noon, the market's peak
    let mut world = base_world();
    let wickmarket = nav.node_point(nav.place("The Wickmarket").expect("baked").node);
    world.add_character(person(
        "prov2",
        wickmarket,
        Some("food_provisioner"),
        Significance::Minor,
    ));
    world.add_character(person(
        "hgry1",
        wickmarket + Vec3::new(8.0, 0.0, 4.0),
        Some("mason"),
        Significance::Minor,
    ));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let vendor = ActorId::from_raw("prov2");
    let hungry = ActorId::from_raw("hgry1");
    assert!(
        round
            .stalls()
            .iter()
            .any(|s| s.name.contains("provisions") && s.vendor.as_ref() == Some(&vendor)),
        "the provisioner keeps the provisions stall on a Highmarket noon"
    );
    // Keep the baker fed (so they simply stand their post), and starve the buyer.
    world
        .characters
        .get_mut(&vendor)
        .unwrap()
        .state
        .needs
        .hunger = 200.0;
    world
        .characters
        .get_mut(&hungry)
        .unwrap()
        .state
        .needs
        .hunger = 8.0; // famished
    set_wallet(&mut world, &hungry, 4);
    let hunger_before = world.characters[&hungry].needs().hunger;

    let (_now, _nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.0, 600, |world| {
        world.characters[&hungry]
            .recent_history()
            .iter()
            .any(|line| line.starts_with("You bought a herring"))
    });
    assert!(
        world.characters[&hungry]
            .recent_history()
            .iter()
            .any(|line| line.starts_with("You bought a herring from")),
        "the buyer remembers the purchase they can be asked about: {:?}",
        world.characters[&hungry].recent_history()
    );
    assert!(
        world.characters[&vendor]
            .recent_history()
            .iter()
            .any(|line| line == "You sold a herring for 1 spark."),
        "the vendor remembers the sale: {:?}",
        world.characters[&vendor].recent_history()
    );
    // Eating follows at the pitch: give it a few more beats and the gauge climbs.
    let (_now, _n) = beats_until(&mut round, &mut world, &nav, &clock, _now, 60, |world| {
        world.characters[&hungry].needs().hunger > hunger_before + 50.0
    });
    assert!(
        world.characters[&hungry].needs().hunger > hunger_before + 50.0,
        "the herring's satiety lifted the buyer out of famine"
    );
    // The eat is silent: the eater remembers their own meal (askable), but no
    // bystander `ate a herring` line floods the nearby vendor's inbox — the
    // market's zero-token discipline (`05` §4).
    assert!(
        world.characters[&hungry]
            .recent_history()
            .iter()
            .any(|line| line == "You ate a herring."),
        "the eater remembers their own meal: {:?}",
        world.characters[&hungry].recent_history()
    );
    assert!(
        !world.characters[&vendor]
            .inbox()
            .iter()
            .any(|line| line.contains("ate a herring")),
        "no bystander eat line reaches the vendor's inbox: {:?}",
        world.characters[&vendor].inbox()
    );
    // The sale clinked a coin — a player-only world sound, never attributed to an
    // actor, so it nudges no NPC (`04` §5).
    let events = world.drain_events();
    let clink = events
        .iter()
        .find(|event| event.sound_id.as_deref() == Some("coin_clink"));
    assert!(clink.is_some(), "the purchase emits a coin_clink");
    assert!(
        clink.unwrap().actor_id.is_none(),
        "the clink is an unattributed world sound"
    );

    // The purchase also emits the presentation-only generic `sale`
    // world event — vendor → buyer, the bought item, one unit — so the host can
    // play the hand-over between their bodies.
    let sale = events
        .iter()
        .find(|event| {
            event.event_type == crate::event::EventType::WorldEvent && event.kind == "sale"
        })
        .expect("the purchase emits a sale world event");
    assert_eq!(
        sale.actor_id.as_ref(),
        Some(&vendor),
        "the vendor performs the hand-over"
    );
    assert_eq!(
        sale.target_id.as_ref(),
        Some(&hungry),
        "the buyer receives it"
    );
    assert!(sale.item_id.is_some(), "the sold item rides the event");
    assert_eq!(sale.quantity, 1, "one unit per sale");
    // The purity rule: presentation-only means NO mind hears about it. Empty
    // recipients (inbox lines only ever come from `deliver` at an action site,
    // and no such site exists for this kind), no witnesses, and no percept
    // beyond the two self-percepts asserted above — every inbox in the world
    // stays free of the sale.
    assert!(
        sale.recipient_ids.is_empty(),
        "sale reaches no mind's inbox"
    );
    assert!(sale.witness_ids.is_empty(), "sale has no witnesses");
    for (id, character) in &world.characters {
        assert!(
            character
                .inbox()
                .iter()
                .all(|line| !line.contains("bought") && !line.contains("sold")),
            "{id} was told about a silent stall sale: {:?}",
            character.inbox()
        );
        assert!(
            character
                .recent_history()
                .iter()
                .all(|line| !line.contains("sale")),
            "{id} perceived the raw event kind: {:?}",
            character.recent_history()
        );
    }
    world.assert_invariants();
}

/// A vendor's `you_sell` price list is written the instant the round binds them
/// to a stall — off the catalog's authored listings, not the current stock
/// (`05_the_llm_seam.md` §3) — and swept the instant they are unbound, so the
/// section only ever appears on a currently-bound vendor.
#[test]
fn binding_a_vendor_writes_you_sell_and_unbinding_clears_it() {
    let nav = nav();
    let clock = clock_on(Office::HighWick, 2); // Highmarket noon, the market's peak
    let mut world = base_world();
    let wickmarket = nav.node_point(nav.place("The Wickmarket").expect("baked").node);
    world.add_character(person(
        "bakr3",
        wickmarket,
        Some("baker"),
        Significance::Minor,
    ));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);

    let baker = ActorId::from_raw("bakr3");
    assert!(
        round
            .stalls()
            .iter()
            .any(|s| s.name.contains("bread") && s.vendor.as_ref() == Some(&baker)),
        "the baker keeps the bread stall"
    );
    assert_eq!(
        world.characters[&baker].state.you_sell,
        vec![VendorListing {
            name: "loaf".into(),
            price_sparks: 2
        }],
        "binding writes the catalog-priced generic-loaf listing"
    );

    // Strip the baker from the enrolled cast and rebind: nobody is left to keep
    // the bread stall, so the vendor and the price list both fall away.
    round.people.remove(&baker);
    round.bind_vendors(&mut world, clock.at(0.0).weekday);
    assert!(
        round
            .stalls()
            .iter()
            .find(|s| s.name.contains("bread"))
            .unwrap()
            .vendor
            .is_none(),
        "the bread stall unbinds with no eligible keeper"
    );
    assert!(
        world.characters[&baker].state.you_sell.is_empty(),
        "an unbound vendor's sheet drops the you_sell section"
    );
}

/// Kindling no longer sweeps persistent supply-chain inventory. An offer over a
/// persistent loaf therefore remains valid through the legacy restock pass.
#[test]
fn persistent_offered_stock_survives_legacy_restock() {
    let (mut world, mut round, vendor, buyer, stock_id) = bread_stall_world();
    apply_action(
        &mut world,
        &vendor,
        "offer_item",
        &json!({"item_id": stock_id.as_str(), "quantity": 1, "target": buyer.as_str()}),
    )
    .unwrap();
    assert!(world.offers.contains_key(&stock_id), "the offer is live");
    round.restock(&mut world, 2);
    assert!(world.offers.contains_key(&stock_id));
    assert_eq!(world.items[&stock_id].quantity, 3);
    world.assert_invariants();
}

/// A listed unit committed to a live offer is unavailable to a mechanical sale.
/// Retracting releases it, after which the same sale succeeds.
#[test]
fn buying_cannot_spend_an_offered_stock_unit() {
    let (mut world, mut round, vendor, buyer, stock_id) = bread_stall_world();
    // One loaf left, and it is on offer to a bystander.
    world.items.get_mut(&stock_id).unwrap().quantity = 1;
    apply_action(
        &mut world,
        &vendor,
        "offer_item",
        &json!({"item_id": stock_id.as_str(), "quantity": 1, "target": buyer.as_str()}),
    )
    .unwrap();
    assert!(try_purchase(&mut round, &mut world, 0, &buyer).is_none());
    assert!(world.items.contains_key(&stock_id));
    apply_action(
        &mut world,
        &vendor,
        "retract_offer",
        &json!({"item_id": stock_id.as_str()}),
    )
    .unwrap();
    let sale = try_purchase(&mut round, &mut world, 0, &buyer).expect("the last loaf sells");
    assert_eq!(sale.stock_left, 0);
    assert!(
        world.characters[&buyer].holds().contains(&stock_id),
        "the whole stack id moved to the buyer"
    );
    assert!(!world.characters[&vendor].holds().contains(&stock_id));
    world.assert_invariants();
}

/// Offered sparks are committed too: a market sale may not spend them until the
/// offer is retracted.
#[test]
fn spending_cannot_use_an_offered_coin_stack() {
    let (mut world, mut round, vendor, buyer, _stock) = bread_stall_world();
    // The buyer holds exactly the price and has it out on offer to the vendor.
    set_wallet(&mut world, &buyer, 2);
    let coin = ItemId::from_raw("w_buyer");
    apply_action(
        &mut world,
        &buyer,
        "offer_item",
        &json!({"item_id": coin.as_str(), "quantity": 2, "target": vendor.as_str()}),
    )
    .unwrap();
    assert!(world.offers.contains_key(&coin), "the coin offer is live");
    assert!(try_purchase(&mut round, &mut world, 0, &buyer).is_none());
    assert_eq!(world.items[&coin].quantity, 2);
    apply_action(
        &mut world,
        &buyer,
        "retract_offer",
        &json!({"item_id": coin.as_str()}),
    )
    .unwrap();
    try_purchase(&mut round, &mut world, 0, &buyer).expect("the loaf is affordable");
    assert!(
        !world.items.contains_key(&coin),
        "the emptied purse is gone"
    );
    world.assert_invariants();
}

/// Exact-boundary wallet settlement refuses to reset a committed purse. Once
/// the offer is explicitly retracted, the same settlement succeeds.
#[test]
fn wallet_settlement_refuses_an_offered_spark_stack() {
    let (mut world, _round, vendor, buyer, _stock) = bread_stall_world();
    let coin = ItemId::from_raw("w_buyer");
    apply_action(
        &mut world,
        &buyer,
        "offer_item",
        &json!({"item_id": coin.as_str(), "quantity": 5, "target": vendor.as_str()}),
    )
    .unwrap();
    assert!(world.offers.contains_key(&coin));
    let error = world
        .settle_wallet_exact(&buyer, 0, "test_committed_wallet")
        .unwrap_err();
    assert_eq!(error.code, crate::InventoryErrorCode::ItemCommitted);
    assert_eq!(world.items[&coin].quantity, 5);
    apply_action(
        &mut world,
        &buyer,
        "retract_offer",
        &json!({"item_id": coin.as_str()}),
    )
    .unwrap();
    world
        .settle_wallet_exact(&buyer, 0, "test_clear_wallet")
        .unwrap();
    assert!(!world.items.contains_key(&coin), "the wallet is gone");
    world.assert_invariants();
}

/// A fed buyer carries food home only when their round is actually taking them
/// there (an active home leg) within the supper span — not merely because it is
/// evening (`04` §5). Otherwise they eat at the pitch, so the loaf is not hoarded.
#[test]
fn carry_home_only_when_actually_heading_home() {
    let home = Vec3::new(0.0, WALK_Y, 0.0);
    let shop = Vec3::new(50.0, WALK_Y, 0.0);
    let buyer = ActorId::from_raw("bYr");
    let mut round = Round::default();
    let legs = |office: Office| RoundLeg {
        from: office,
        at: if office == Office::Lamplight {
            home
        } else {
            shop
        },
        label: if office == Office::Lamplight {
            "home".into()
        } else {
            "shop".into()
        },
        doing: if office == Office::Lamplight {
            Arrival::Sleep
        } else {
            Arrival::Trade
        },
        only_on: None,
        is_home: office == Office::Lamplight,
    };
    let person = Townsperson {
        home: Some(home),
        base: home,
        legs: vec![legs(Office::Waning), legs(Office::Lamplight)],
        leash_m: DEFAULT_ROUND_LEASH_M,
        curfew_exempt: false,
        source: None,
        is_household: false,
        food: None,
        phase: Phase::Idle,
        travel_target: None,
        travel_for_intent: false,
        next_decision: 0.0,
        epoch: 0,
        evening_seed: None,
        excused: false,
    };
    round.people.insert(buyer.clone(), person);
    // At the Waning the active leg is still the shop → eat at the pitch.
    assert!(
        !should_carry(&round, &buyer, Office::Waning, Weekday::Second),
        "still at the market post: eat here"
    );
    // At Lamplight the active leg is home → carry it to the hearth.
    assert!(
        should_carry(&round, &buyer, Office::Lamplight, Weekday::Second),
        "headed home for supper: carry it"
    );
    // At noon nobody carries.
    assert!(
        !should_carry(&round, &buyer, Office::HighWick, Weekday::Second),
        "not the supper span"
    );
}

/// The low-hunger memory hook is a *first-person present* declaration, not a bare
/// substring (`03_hunger.md` §1/§6). Ilse's memory still seeds her; third-person
/// lore about someone else's hunger, or `hungry` embedded in a larger word, does
/// not — the silent-famine risk the review flagged.
#[test]
fn the_memory_hunger_hook_is_first_person_only() {
    assert!(
        memory_declares_hunger("I am very hungry after the long road here"),
        "Ilse still matches"
    );
    assert!(
        memory_declares_hunger("I feel hungry."),
        "a plain first-person declaration matches"
    );
    assert!(
        !memory_declares_hunger("the winter everyone went hungry"),
        "third-person lore does not seed famine"
    );
    assert!(
        !memory_declares_hunger("hungrycrake was the fisher's nickname"),
        "an embedded substring is not the word"
    );
    assert!(
        !memory_declares_hunger("I am weary after the long road"),
        "no hunger, no match"
    );
}

// --------------------------------------------------------------------------- //
// M5 road-boundary and settlement controller fixtures.
// --------------------------------------------------------------------------- //

fn road_party_world() -> World {
    let mut world = base_world();
    for (id, occupation, significance) in [
        ("rbrde", "merchant", Significance::Minor),
        ("cbred", "cargo_worker", Significance::Ambient),
        ("dbred", "cargo_worker", Significance::Ambient),
        ("rlant", "merchant", Significance::Minor),
        ("clant", "cargo_worker", Significance::Ambient),
    ] {
        world.add_character(person(id, Vec3::ZERO, Some(occupation), significance));
    }
    world
}

#[test]
fn exact_office_bootstrap_stages_and_enters_only_the_scheduled_party_once() {
    let nav = nav();
    let cases = [
        (0, None),
        (1, Some(("lantern_stone_gate", 2usize))),
        (2, Some(("brede_wool_gate", 3usize))),
        (3, Some(("brede_wool_gate", 3usize))),
        (4, Some(("lantern_stone_gate", 2usize))),
        (5, None),
        (6, None),
    ];
    for (day, expected) in cases {
        let mut world = road_party_world();
        let mut round = Round::new();
        round.seed(&mut world, &nav, 0.0, &clock_on(Office::Dayspring, day));

        let visible: Vec<(PartyId, usize, u64)> = round
            .road_parties
            .values()
            .filter(|party| party.state.phase == PartyPhase::InCity)
            .map(|party| {
                (
                    party.id.clone(),
                    party.members.len(),
                    party.state.trip_number,
                )
            })
            .collect();
        match expected {
            Some((party_id, members)) => {
                assert_eq!(visible.len(), 1, "day {day}");
                assert_eq!(visible[0].0.as_str(), party_id);
                assert_eq!(visible[0].1, members);
                assert_eq!(visible[0].2, 1);
                assert!(
                    round.road_parties[&visible[0].0]
                        .members
                        .iter()
                        .all(|member| world.is_present(member))
                );
                assert_eq!(round.road_carts(&world).len(), 1);
                let revision = world.world_revision;
                let visible_party = visible[0].0.clone();
                round.trigger_road_entry(&mut world, &visible_party, day);
                assert_eq!(
                    world.world_revision, revision,
                    "bootstrap cannot enter twice"
                );
                assert_eq!(round.party_state(&visible_party).unwrap().trip_number, 1);
            }
            None => {
                assert!(visible.is_empty(), "day {day} has no scheduled arrival");
                assert!(round.road_carts(&world).is_empty());
            }
        }
        world.assert_invariants();
    }

    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_on(Office::Kindling, 2));
    let party = PartyId::from_raw("brede_wool_gate");
    assert_eq!(
        round.party_state(&party).unwrap().phase,
        PartyPhase::StagedOutsideGate
    );
    assert_eq!(round.party_state(&party).unwrap().trip_number, 1);
    assert!(
        round.road_parties[&party]
            .members
            .iter()
            .all(|member| !world.is_present(member))
    );
    assert!(round.road_carts(&world).is_empty());
    let revision = world.world_revision;
    round.trigger_road_stage(&mut world, &party, 2);
    assert_eq!(
        world.world_revision, revision,
        "exact-Kindling bootstrap stages once"
    );
    assert_eq!(round.party_state(&party).unwrap().trip_number, 1);
}

#[test]
fn return_mode_clears_competing_state_and_the_next_boundary_unloads_every_member() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");
    let carter = ActorId::from_raw("cbred");
    let gate = round.road_parties[&party_id].gate_point;

    // Cargo remains represented on the same one derived cart after moving
    // from the leader to a carter; the cart is no second inventory container.
    let wool = world.characters[&leader]
        .holds()
        .iter()
        .find(|id| world.items[*id].kind.as_str() == "wool")
        .cloned()
        .expect("Brede manifest carries wool");
    let wool_quantity = world.items[&wool].quantity;
    let load_before = round.road_carts(&world)[0].load.clone();
    world
        .transfer_item_quantity(
            &leader,
            &carter,
            &wool,
            wool_quantity,
            "road:test:carter-wool",
        )
        .unwrap();
    assert_eq!(round.road_carts(&world)[0].load, load_before);
    assert!(world.characters[&carter].holds().contains(&wool));
    let personal = world
        .add_stock(&carter, &stock("generic", 1), "road:test:personal")
        .unwrap();

    // Move the leader's whole purse away. The old id remains owned by a city
    // actor, forcing the next boundary credit to probe a fresh deterministic id.
    let city = ActorId::from_raw("city1");
    world.add_character(person("city1", gate, None, Significance::Minor));
    let old_purse = world.characters[&leader]
        .holds()
        .iter()
        .find(|id| world.items[*id].kind.as_str() == "spark")
        .cloned()
        .unwrap();
    let leader_cash = world.items[&old_purse].quantity;
    world
        .transfer_item_quantity(
            &leader,
            &city,
            &old_purse,
            leader_cash,
            "road:test:whole-purse",
        )
        .unwrap();

    // Competing city state cannot own a returning member. Queue and errand
    // records, route intent, gesture, and a pending offer all disappear.
    round.sources[0].queue.push(carter.clone());
    round.stalls[0].queue.push(carter.clone());
    round.market_errands.insert(
        leader.clone(),
        MarketErrand {
            plan_id: "brede_broadcloth".into(),
            selected: None,
            bindings_seen: Vec::new(),
            phase: MarketErrandPhase::WaitingForOpen,
            spent_sparks: 0,
            last_failed_fingerprint: None,
            travel_deadline_real: None,
            deadline_hold_began_real: None,
        },
    );
    world.characters.get_mut(&leader).unwrap().state.intent = Some(TravelIntent {
        target: IntentTarget::Place {
            place_id: PlaceId::from_raw("somewhere"),
            name: "somewhere".into(),
            point: Vec3::new(100.0, WALK_Y, 100.0),
        },
        budget_seconds: 100.0,
        deadline: Some(100.0),
    });
    world
        .characters
        .get_mut(&leader)
        .unwrap()
        .state
        .needs
        .hunger = 0.0;
    world.characters.get_mut(&city).unwrap().state.position_m =
        world.characters[&carter].position_m();
    apply_action(
        &mut world,
        &carter,
        "offer_item",
        &json!({"item_id": personal.as_str(), "target": city.as_str()}),
    )
    .unwrap();
    world
        .characters
        .get_mut(&carter)
        .unwrap()
        .state
        .inbox
        .push("Unread at the gate".into());
    world
        .characters
        .get_mut(&carter)
        .unwrap()
        .state
        .recent_history
        .push("A durable road line".into());

    let revision = world.world_revision;
    round.begin_road_return(&mut world, &party_id, 2, &mut Vec::new());
    assert_eq!(
        world.world_revision,
        revision + 1,
        "begin-return is one atomic public transition"
    );
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::Returning
    );
    assert!(
        round
            .sources
            .iter()
            .all(|source| !source.queue.contains(&carter))
    );
    assert!(
        round
            .stalls
            .iter()
            .all(|stall| !stall.queue.contains(&carter))
    );
    assert!(!round.market_errands.contains_key(&leader));
    assert_eq!(
        round.closed_market_visits["brede_broadcloth"].end_reason,
        MarketVisitEnd::Returning
    );
    for member in &round.road_parties[&party_id].members {
        assert!(world.characters[member].state.leaving_city);
        assert!(world.characters[member].state.intent.is_none());
        assert!(world.characters[member].state.movement.is_none());
    }

    for member in round.road_parties[&party_id].members.clone() {
        world.characters.get_mut(&member).unwrap().state.position_m = gate;
    }
    let time = clock.at(0.0);
    let revision = world.world_revision;
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert_eq!(
        world.world_revision,
        revision + 1,
        "offer expiry and departure-pending publish as one transition"
    );
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::DeparturePending
    );
    assert!(!world.offers.contains_key(&personal));
    let revision = world.world_revision;
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert_eq!(
        world.world_revision,
        revision + 1,
        "departure is one atomic public transition"
    );
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::BeyondTheWalls
    );
    assert!(round.road_carts(&world).is_empty());
    assert!(world.characters[&carter].state.inbox.is_empty());
    assert_eq!(
        world.characters[&carter].recent_history(),
        ["A durable road line"]
    );

    round
        .road_parties
        .get_mut(&party_id)
        .unwrap()
        .last_trigger_day = None;
    let revision = world.world_revision;
    round.trigger_road_stage(&mut world, &party_id, 3);
    assert_eq!(
        world.world_revision,
        revision + 1,
        "the next boundary exchange is one atomic public transition"
    );
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::StagedOutsideGate
    );
    assert_eq!(round.party_state(&party_id).unwrap().trip_number, 2);
    assert!(
        !world.items.contains_key(&wool),
        "the carter's commercial wool unloaded"
    );
    assert!(
        world.characters[&carter].holds().contains(&personal),
        "personal cargo survives"
    );
    assert!(world.characters[&city].holds().contains(&old_purse));
    assert_eq!(world.wallet_sparks(&leader), 25);
    let new_purse = world.characters[&leader]
        .holds()
        .iter()
        .find(|id| world.items[*id].kind.as_str() == "spark")
        .unwrap();
    assert_ne!(
        new_purse, &old_purse,
        "cash-in probes past the transferred purse id"
    );
    assert_eq!(world.wallet_sparks(&carter), 4);
    world.assert_invariants();
}

#[test]
fn boundary_failure_and_missed_trip_leave_the_trip_and_manifest_unchanged() {
    let nav = nav();
    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock_on(Office::Kindling, 2));
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");

    // A party that is still staged at its next trigger logs a miss and receives
    // neither a duplicate manifest nor a fresh trip number.
    let quantities_before: BTreeMap<ItemId, u32> = world.characters[&leader]
        .holds()
        .iter()
        .map(|id| (id.clone(), world.items[id].quantity))
        .collect();
    let revision = world.world_revision;
    round
        .road_parties
        .get_mut(&party_id)
        .unwrap()
        .last_trigger_day = None;
    round.trigger_road_stage(&mut world, &party_id, 3);
    assert_eq!(round.party_state(&party_id).unwrap().trip_number, 1);
    assert_eq!(world.world_revision, revision);
    assert_eq!(
        world.characters[&leader]
            .holds()
            .iter()
            .map(|id| (id.clone(), world.items[id].quantity))
            .collect::<BTreeMap<_, _>>(),
        quantities_before
    );
    assert!(
        round
            .drain_food_log()
            .iter()
            .any(|line| line.contains("road_trip_missed"))
    );

    // Put the controller back beyond the wall and commit the leader's purse.
    // Boundary preflight must roll back the earlier cargo-unload stage too.
    round.road_parties.get_mut(&party_id).unwrap().state.phase = PartyPhase::BeyondTheWalls;
    round
        .road_parties
        .get_mut(&party_id)
        .unwrap()
        .last_trigger_day = None;
    let purse = world.characters[&leader]
        .holds()
        .iter()
        .find(|id| world.items[*id].kind.as_str() == "spark")
        .cloned()
        .unwrap();
    world.offers.insert(
        purse.clone(),
        Offer {
            item_id: purse.clone(),
            giver_id: leader.clone(),
            target_id: None,
            created_seq: 99,
            quantity: 1,
        },
    );
    let revision = world.world_revision;
    let trip = round.party_state(&party_id).unwrap().trip_number;
    round.trigger_road_stage(&mut world, &party_id, 3);
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::BeyondTheWalls
    );
    assert_eq!(round.party_state(&party_id).unwrap().trip_number, trip);
    assert_eq!(world.world_revision, revision);
    assert_eq!(
        world.characters[&leader]
            .holds()
            .iter()
            .map(|id| (id.clone(), world.items[id].quantity))
            .collect::<BTreeMap<_, _>>(),
        quantities_before
    );
    assert!(world.offers.contains_key(&purse));
    assert!(
        round
            .drain_food_log()
            .iter()
            .any(|line| line.contains("boundary_exchange_failed"))
    );
}

/// `law_and_order.md` M4/M5: the one choke point every mechanical mover lays a
/// route through refuses a body in the law's hands. `follow_escorts`' per-tick
/// clear runs *before* `round::tick`, so any mover that re-lays after it wins
/// the next movement slice — which is how a committed prisoner used to creep
/// out of the gaol at full walking speed and get branded an escapee for a walk
/// they never chose.
#[test]
fn set_route_refuses_a_body_in_the_laws_hands() {
    let mut world = base_world();
    world.add_character(person(
        "taken1",
        Vec3::ZERO,
        Some("domestic_servant"),
        Significance::Ambient,
    ));
    let taken = ActorId::from_raw("taken1");
    let station = crate::custody::Station {
        place_id: PlaceId::from_raw("pl_test"),
        name: "The Wool Gate".into(),
        point: Vec3::ZERO,
        stone_house: false,
    };
    world.custody.seize(
        taken.clone(),
        ActorId::from_raw("srgnt"),
        Some(1),
        station,
        0.0,
    );

    let path = vec![Vec3::new(10.0, WALK_Y, 0.0)];
    set_route(&mut world, &taken, path.clone());
    assert!(
        world.characters[&taken].state.movement.is_none(),
        "no round mover routes the law's prisoner"
    );

    // The moment the law lets go, the same call walks them again.
    world.custody.release(&taken);
    set_route(&mut world, &taken, path);
    assert!(world.characters[&taken].is_walking());
}

/// The full shape of the escape-brand bug: a road party's trading leg re-lays a
/// route every tick a member is `!is_walking()`, so the escort-side clear in
/// `follow_escorts` never won and a member committed at the gate arch walked to
/// Seven Lofts at full speed — past [`crate::custody::COMMITTED_ROAM_M`] within
/// seconds, into M4d's unanswerable escape notice. The threshold has to hold
/// under the engine's real per-poll order: escort clear first, round re-lay
/// second, movement slice last.
#[test]
fn a_committed_road_party_member_never_creeps_off_their_threshold() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let time = clock.at(0.0);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");
    let carter = ActorId::from_raw("cbred");
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::InCity
    );

    // Control: the Dayspring leg is live and really does route a free member —
    // the guarded stillness below means something.
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert!(
        world.characters[&leader].is_walking(),
        "control: the trading leg walks a free member"
    );

    // The law takes the carter where they stand and commits them there. Four
    // of the eight postings *are* gates, and the party entered at one: the
    // nearest station to a just-entered member is the arch itself.
    let station =
        crate::custody::nearest_station(&world.places, world.characters[&carter].position_m())
            .expect("the baked city has postings");
    assert_eq!(station.name, "The Wool Gate");
    world.custody.seize(
        carter.clone(),
        leader.clone(),
        Some(1),
        station.clone(),
        0.0,
    );
    world.custody.commit(&carter, 0.0);
    {
        let actor = world.characters.get_mut(&carter).expect("carter exists");
        actor.state.position_m = station.point;
        actor.state.movement = None;
        actor.state.intent = None;
    }

    // Ten seconds of polls in the engine's own order. Unguarded, the very
    // first re-lay wins every slice and 1.8 m/s crosses the 8 m roam radius
    // in under five.
    let mut now = 0.0;
    for _ in 0..200 {
        crate::custody::follow_escorts(&mut world, now);
        round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
        world.step_movement(0.05, &nav, None);
        now += 0.05;
    }
    let at = world.characters[&carter].position_m();
    let strayed = f64::hypot(at.x - station.point.x, at.z - station.point.z);
    assert!(
        strayed <= crate::custody::COMMITTED_ROAM_M,
        "a committed member stays at the threshold, strayed {strayed:.1} m"
    );
    assert!(
        world.characters[&carter].state.movement.is_none(),
        "the party lays no walk for the law's prisoner"
    );
    world.assert_invariants();
}

/// A road member is never enrolled in `people` — the party owns their feet —
/// but `go_to` accepts them like anybody else, so the intent pass has to reach
/// them all the same. Unticked, the errand never had its deadline stamped,
/// never arrived and never lapsed, while [`Round::tick_road_parties`] went on
/// honouring it above the trading leg: one `go_to` parked a carrier at a frozen
/// point for the rest of the trip, away from the counter their stock errand
/// needs them standing at.
#[test]
fn a_road_members_errand_arrives_with_a_percept_and_a_nudge() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::InCity
    );
    assert!(
        !round.people.contains_key(&leader),
        "the round deliberately never enrols a road member"
    );

    // Stand him a short street's walk from the Gradine and hand him the way
    // there: a road member is seeded no `places_known` of his own, so this is
    // the state `tell_way` — the only way he ever holds a handle — leaves him in.
    let gradine_node = nav.place("The Gradine").expect("baked").node;
    let entry = world.places.named("The Gradine").expect("baked");
    let (place_id, gradine) = (entry.id.clone(), entry.point);
    let up_the_street = nav.node_point(nav.adjacency()[gradine_node][0].to);
    let start = gradine + (up_the_street - gradine).normalize() * 20.0;
    assert!(
        nav.is_walkable(start.x, start.z),
        "the street midline is walkable"
    );
    {
        let actor = world
            .characters
            .get_mut(&leader)
            .expect("the leader exists");
        actor.state.position_m = start;
        actor.state.places_known.insert(place_id.clone());
    }

    apply_action(
        &mut world,
        &leader,
        "go_to",
        &json!({"place_id": place_id.as_str()}),
    )
    .unwrap();
    // The party is what walks him — the intent pass lays no route for somebody
    // it holds no phase for.
    tick_collect(&mut round, &mut world, &nav, &clock, 0.1);
    assert!(
        world.characters[&leader].is_walking(),
        "the trading leg walks the errand it is honouring"
    );

    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.1, 200, |world| {
        world.characters[&leader].state.intent.is_none()
    });
    let walker = &world.characters[&leader];
    assert!(
        walker.position_m().distance(gradine) <= PLACE_ARRIVE_RADIUS_M + 0.5,
        "the errand really was walked, not merely forgotten: {:?}",
        walker.position_m()
    );
    assert!(
        walker
            .inbox()
            .iter()
            .any(|line| line == "You have arrived at The Gradine."),
        "arrival is a percept for a road member too: {:?}",
        walker.inbox()
    );
    assert!(
        nudges.contains(&leader),
        "the arrival granted the priority nudge"
    );
    world.assert_invariants();
}

/// The other half of the same errand: a road member's `go_to {person}` is
/// walked by the party, so the party has to stop at the *errand's* distance and
/// not at its own six-metre leg radius — otherwise the follower is parked
/// within talking distance of somebody they are told they never caught.
#[test]
fn a_road_members_follow_closes_to_conversation_distance() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    let leader = ActorId::from_raw("rbrde");
    let target = ActorId::from_raw("targt");
    let gradine_node = nav.place("The Gradine").expect("baked").node;
    let start = nav.node_point(gradine_node);
    let up_the_street = nav.node_point(nav.adjacency()[gradine_node][0].to);
    let target_at = start + (up_the_street - start).normalize() * 12.0;
    world.add_character(person("targt", target_at, None, Significance::Major));
    world.nav = Some(std::sync::Arc::new(nav.clone()));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    // After the seed: entering the city puts every member on their gate point.
    world
        .characters
        .get_mut(&leader)
        .expect("the leader exists")
        .state
        .position_m = start;

    apply_action(&mut world, &leader, "go_to", &json!({"person": "targt"})).unwrap();
    let (_, nudges) = beats_until(&mut round, &mut world, &nav, &clock, 0.0, 200, |world| {
        world.characters[&leader].state.intent.is_none()
    });
    let gap = world.characters[&leader]
        .position_m()
        .distance(world.characters[&target].position_m());
    assert!(
        gap <= PERSON_ARRIVE_RADIUS_M + 0.5,
        "the party walked the follow all the way in, gap {gap}"
    );
    assert!(
        world.characters[&leader]
            .inbox()
            .iter()
            .any(|line| line == "You have caught up with a stranger (id targt)."),
        "{:?}",
        world.characters[&leader].inbox()
    );
    assert!(nudges.contains(&leader));
    world.assert_invariants();
}

/// A departure must not carry anybody out of the world while the law holds
/// them: `transition_presence` would dissolve a custody nobody released. The
/// held member stays behind — the cast is fixed and this is a named person
/// with a life in the city — and the party leaves without them, neither
/// stranded at the gate waiting for somebody who can never arrive nor held up
/// by the prisoner's conversation with their keeper.
#[test]
fn a_held_member_stays_behind_and_the_party_departs_without_them() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    world.add_character(person(
        "srgnt",
        Vec3::ZERO,
        Some("bailiff_and_gaoler"),
        Significance::Minor,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let time = clock.at(0.0);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");
    let carter = ActorId::from_raw("cbred");
    let third = ActorId::from_raw("dbred");
    let officer = ActorId::from_raw("srgnt");
    let gate = round.road_parties[&party_id].gate_point;

    round.begin_road_return(&mut world, &party_id, 2, &mut Vec::new());
    for member in round.road_parties[&party_id].members.clone() {
        world.characters.get_mut(&member).unwrap().state.position_m = gate;
    }
    // Committed at the arch itself, after the return already marked them
    // leaving — the departure has to unsay that flag for the one who stays.
    let station = crate::custody::nearest_station(&world.places, gate).expect("postings");
    world
        .custody
        .seize(carter.clone(), officer.clone(), Some(1), station, 0.0);
    world.custody.commit(&carter, 0.0);
    assert!(world.characters[&carter].state.leaving_city);

    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::DeparturePending,
        "a held crew member does not strand the party at the gate"
    );
    // The prisoner talking to their keeper holds nothing: only the departing
    // must be out of conversation.
    round.tick_road_parties(&mut world, &nav, time, 0.0, &warm(&carter));
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::BeyondTheWalls
    );
    assert!(!world.is_present(&leader));
    assert!(!world.is_present(&third));
    assert!(
        world.is_present(&carter),
        "the law's prisoner is not carried out of the world"
    );
    assert!(
        world.custody.is_confined(&carter),
        "the custody nobody released still stands"
    );
    assert!(
        !world.characters[&carter].state.leaving_city,
        "somebody staying is not leaving"
    );
    assert!(
        !round.road_parties[&party_id].members.contains(&carter),
        "the roster stops naming somebody who no longer travels"
    );
    let departed = round.drain_departed();
    assert!(departed.contains(&leader) && departed.contains(&third));
    assert!(
        !departed.contains(&carter),
        "the engine must not forget somebody who is still here"
    );
    assert!(
        round
            .drain_food_log()
            .iter()
            .any(|line| line.contains("road_left_behind")),
        "staying behind is said in the trace"
    );
    world.assert_invariants();
}

/// The cart, the manifest and the boundary exchange are all the leader's, so a
/// party whose *leader* the law holds does not leave at all: it waits out the
/// custody — the hold ceilings drain every one in minutes — and departs
/// whole once the law lets go.
#[test]
fn a_party_whose_leader_the_law_holds_waits_and_departs_whole_on_release() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    world.add_character(person(
        "srgnt",
        Vec3::ZERO,
        Some("bailiff_and_gaoler"),
        Significance::Minor,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let time = clock.at(0.0);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");
    let officer = ActorId::from_raw("srgnt");
    let gate = round.road_parties[&party_id].gate_point;

    round.begin_road_return(&mut world, &party_id, 2, &mut Vec::new());
    for member in round.road_parties[&party_id].members.clone() {
        world.characters.get_mut(&member).unwrap().state.position_m = gate;
    }
    let station = crate::custody::nearest_station(&world.places, gate).expect("postings");
    world
        .custody
        .seize(leader.clone(), officer.clone(), Some(1), station, 0.0);

    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::Returning,
        "no departure forms around a held leader"
    );

    world.custody.release(&leader);
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::BeyondTheWalls
    );
    assert_eq!(
        round.road_parties[&party_id].members.len(),
        3,
        "nobody was left behind"
    );
    world.assert_invariants();
}

/// The life the departure promises is actually handed over. The roster
/// `retain` is permanent and `seed` is one-shot, so a member the law keeps back
/// used to be dropped by every system that walks `people`: no legs, no leash
/// centre, no ladder cadence, a sheet still naming the departed cart's trading
/// leg, and a paving stone to stand on for the rest of the run. They are
/// enrolled at the gate instead — homeless, on their own occupation's round —
/// and the moment custody ends the ladder walks them to the day's leg.
#[test]
fn a_member_left_behind_is_enrolled_in_the_round_and_walks_once_released() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    world.add_character(person(
        "srgnt",
        Vec3::ZERO,
        Some("bailiff_and_gaoler"),
        Significance::Minor,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let time = clock.at(0.0);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let carter = ActorId::from_raw("cbred");
    let officer = ActorId::from_raw("srgnt");
    let gate = round.road_parties[&party_id].gate_point;
    assert!(
        !round.people.contains_key(&carter),
        "a crew still on the road belongs to the party, not to the round"
    );

    round.begin_road_return(&mut world, &party_id, 2, &mut Vec::new());
    for member in round.road_parties[&party_id].members.clone() {
        world.characters.get_mut(&member).unwrap().state.position_m = gate;
    }
    let station = crate::custody::nearest_station(&world.places, gate).expect("postings");
    world
        .custody
        .seize(carter.clone(), officer.clone(), Some(1), station, 0.0);
    world.custody.commit(&carter, 0.0);
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    round.tick_road_parties(&mut world, &nav, time, 0.0, &BTreeSet::new());
    assert_eq!(
        round.party_state(&party_id).unwrap().phase,
        PartyPhase::BeyondTheWalls
    );

    // Enrolled on the seed's own terms for somebody with no bed: the gate their
    // party's evening leg named is the leash centre, the legs are their
    // occupation's archetype, and the sheet stops promising a trade at a pitch
    // the cart took with it.
    let stranded = round
        .people
        .get(&carter)
        .expect("the law's leftovers are enrolled in the round");
    assert_eq!(
        stranded.home, None,
        "homes.json names no door for a road member, and none is invented"
    );
    assert_eq!(stranded.base, gate);
    let legs = stranded.legs.clone();
    assert!(
        !legs.is_empty() && legs.iter().all(|leg| !leg.is_home),
        "a stranded carter keeps the wharf archetype, minus the bed: {legs:?}"
    );
    assert_eq!(
        world.characters[&carter].state.economic_class,
        crate::EconomicClass::Visitor,
        "no party float is theirs, and no household settlement either"
    );
    let sheet_round = world.characters[&carter].state.daily_round.clone();
    assert!(
        sheet_round
            .iter()
            .all(|line| !line.contains("Seven Lofts") && !line.contains("The Wool Gate")),
        "the sheet stops naming the departed party's legs: {sheet_round:?}"
    );
    assert!(
        !world.characters[&carter].state.places_known.is_empty(),
        "somebody living here holds the coarse ways"
    );

    // Rung 0 pins them while the law has them; the ladder takes over the moment
    // it lets go, and the day's leg is a real walk from the gate.
    let target = active_leg(&legs, Office::Dayspring, Weekday::of_day(2))
        .expect("the carter's day has a Dayspring leg")
        .at;
    assert!(target.distance(gate) > ROUND_ARRIVE_RADIUS_M);
    let mut now = 0.0;
    for _ in 0..20 {
        now += 0.5;
        beat(&mut round, &mut world, &nav, &clock, now, 0.5);
    }
    assert_eq!(
        world.characters[&carter].position_m(),
        gate,
        "the law's prisoner walks nowhere"
    );

    world.custody.release(&carter);
    for _ in 0..120 {
        now += 0.5;
        beat(&mut round, &mut world, &nav, &clock, now, 0.5);
    }
    let closed = target.distance(gate) - target.distance(world.characters[&carter].position_m());
    assert!(
        closed > 1.0,
        "released, the stranded carter walks their own round's leg (closed {closed} m)"
    );
    world.assert_invariants();
}

/// A stock-errand world with the Brede grain cart bound at Seven Lofts and an
/// enrolled city buyer for the `betriss_grain` plan: the seller stands at his
/// pitch, the buyer stands at the party's gate with a long walk ahead. The
/// clock is 100 real hours per game day so the whole timeline below stays
/// inside Dayspring — nothing here is about offices.
fn grain_errand_fixture() -> (NavData, WorldClock, World, Round, ActorId) {
    let nav = nav();
    let clock = WorldClock::new(360_000.0, Office::Dayspring, 2, 0.05);
    let mut world = road_party_world();
    world.add_character(person(
        "p008s",
        Vec3::ZERO,
        Some("merchant"),
        Significance::Minor,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let buyer = ActorId::from_raw("p008s");
    let seller = ActorId::from_raw("rbrde");
    let pitch = round.counters["brede_grain_seven_lofts"].pitch;
    let gate = round.road_parties[&PartyId::from_raw("brede_wool_gate")].gate_point;
    world.characters.get_mut(&seller).unwrap().state.position_m = pitch;
    world.characters.get_mut(&buyer).unwrap().state.position_m = gate;
    (nav, clock, world, round, buyer)
}

/// `07_the_supply_chain.md`: a conversation or a pressing need *pauses* a
/// stock errand rather than ending it — but `travel_deadline_real` was a real
/// clock that never paused, so a buyer legitimately diverted mid-walk (the
/// famished rung, a lightning flinch, a chat) came back past the deadline,
/// took `TravelExpired`, and forfeited the binding for the rest of the day: a
/// timeout firing on state another system caused. The deadline now moves
/// forward by exactly the span the hold owned — and still expires a walk that
/// stays stuck once the errand has the body back.
#[test]
fn a_pressing_diversion_pauses_the_stock_travel_deadline() {
    let (nav, clock, mut world, mut round, buyer) = grain_errand_fixture();
    let quiet = BTreeSet::new();

    round.tick_stock_plans(&mut world, &nav, clock.at(0.0), 0.0, &quiet);
    let deadline = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("the walk stamps its deadline");
    assert!(world.characters[&buyer].is_walking(), "the errand walks");

    // The stand-in for any pressing hold: the lightning flinch is one of the
    // three OR'd conditions of the very errand-hold gate the deadline must
    // respect, and it needs no stall or hunger fixture.
    round
        .lightning_reflex_until
        .insert(buyer.clone(), deadline + 500.0);
    round.tick_stock_plans(&mut world, &nav, clock.at(10.0), 10.0, &quiet);
    round.tick_stock_plans(
        &mut world,
        &nav,
        clock.at(deadline + 100.0),
        deadline + 100.0,
        &quiet,
    );
    assert!(
        round.market_errands.contains_key(&buyer),
        "a held errand never expires, however long the hold"
    );

    // The first tick after the hold is exactly where the old clock fired.
    let resume = deadline + 501.0;
    round.tick_stock_plans(&mut world, &nav, clock.at(resume), resume, &quiet);
    assert!(
        round.market_errands.contains_key(&buyer),
        "the deadline was paused for the hold, not burned by it"
    );
    assert!(
        !round.closed_market_visits.contains_key("betriss_grain"),
        "the day's binding stays open"
    );
    let extended = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("still walking");
    assert!(
        (extended - (deadline + (resume - 10.0))).abs() < 1e-6,
        "the deadline moved by exactly the held span"
    );

    // Still meaningful: with the body its own again, a walk that never
    // arrives still ends the visit.
    let late = extended + 1.0;
    round.tick_stock_plans(&mut world, &nav, clock.at(late), late, &quiet);
    assert!(!round.market_errands.contains_key(&buyer));
    assert_eq!(
        round.closed_market_visits["betriss_grain"].end_reason,
        MarketVisitEnd::TravelExpired
    );
    world.assert_invariants();
}

/// The same pause for the conversation hold — the spec's own words are that
/// conversation pauses an errand rather than cancelling it, and a chat is no
/// more the walk's fault than hunger is.
#[test]
fn a_conversation_pauses_the_stock_travel_deadline() {
    let (nav, clock, mut world, mut round, buyer) = grain_errand_fixture();
    let quiet = BTreeSet::new();

    round.tick_stock_plans(&mut world, &nav, clock.at(0.0), 0.0, &quiet);
    let deadline = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("the walk stamps its deadline");

    let held = warm(&buyer);
    round.tick_stock_plans(&mut world, &nav, clock.at(5.0), 5.0, &held);
    let resume = deadline + 200.0;
    round.tick_stock_plans(&mut world, &nav, clock.at(resume), resume, &quiet);
    assert!(
        round.market_errands.contains_key(&buyer),
        "a long conversation does not cost the buyer the visit"
    );
    let extended = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("still walking");
    assert!((extended - (deadline + (resume - 5.0))).abs() < 1e-6);
    world.assert_invariants();
}

/// `07_the_supply_chain.md`: a seller who temporarily steps outside the
/// counter radius clears `selected` but does not reset or end the visit —
/// and that survival must include the walk's own clock. The flip used to null
/// `travel_deadline_real` and the rebind re-stamped a whole fresh budget, so
/// a genuinely stuck walk toward a seller who flickers on and off the pitch
/// churned between walking and waiting all day without ever tripping
/// `TravelExpired`. Now the same binding going absent and coming back keeps
/// the deadline it had, moved forward by exactly the span the buyer stood
/// waiting — and a walk that still never arrives still ends the visit.
#[test]
fn a_sellers_brief_excursion_never_rearms_the_stock_travel_deadline() {
    let (nav, clock, mut world, mut round, buyer) = grain_errand_fixture();
    let quiet = BTreeSet::new();
    let seller = ActorId::from_raw("rbrde");
    let pitch = round.counters["brede_grain_seven_lofts"].pitch;
    let away = pitch + Vec3::new(round.counters["brede_grain_seven_lofts"].radius_m + 5.0, 0.0, 0.0);

    round.tick_stock_plans(&mut world, &nav, clock.at(0.0), 0.0, &quiet);
    let deadline = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("the walk stamps its deadline");
    assert!(world.characters[&buyer].is_walking(), "the errand walks");

    // The seller steps off the pitch: the spec's documented pause — selection
    // cleared, the buyer stands and waits, the visit stays open.
    world.characters.get_mut(&seller).unwrap().state.position_m = away;
    round.tick_stock_plans(&mut world, &nav, clock.at(10.0), 10.0, &quiet);
    let errand = &round.market_errands[&buyer];
    assert_eq!(errand.selected, None, "the excursion clears the selection");
    assert_eq!(errand.phase, MarketErrandPhase::WaitingForOpen);
    assert_eq!(
        errand.travel_deadline_real,
        Some(deadline),
        "the flicker does not touch the walk's clock"
    );

    // The seller returns: the walk resumes with the budget it had, pushed by
    // exactly the ten seconds stood waiting — not re-stamped from now.
    world.characters.get_mut(&seller).unwrap().state.position_m = pitch;
    round.tick_stock_plans(&mut world, &nav, clock.at(20.0), 20.0, &quiet);
    let resumed = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("walking again");
    assert!(
        (resumed - (deadline + 10.0)).abs() < 1e-6,
        "the deadline moved by exactly the absent span, not a fresh budget"
    );

    // A second flicker buys nothing more than its own span either.
    world.characters.get_mut(&seller).unwrap().state.position_m = away;
    round.tick_stock_plans(&mut world, &nav, clock.at(30.0), 30.0, &quiet);
    world.characters.get_mut(&seller).unwrap().state.position_m = pitch;
    round.tick_stock_plans(&mut world, &nav, clock.at(40.0), 40.0, &quiet);
    let twice = round.market_errands[&buyer]
        .travel_deadline_real
        .expect("still walking");
    assert!((twice - (deadline + 20.0)).abs() < 1e-6);

    // Still meaningful: with the seller present and the walk simply never
    // arriving, the backstop the flickers used to push out forever now fires.
    let late = twice + 1.0;
    round.tick_stock_plans(&mut world, &nav, clock.at(late), late, &quiet);
    assert!(!round.market_errands.contains_key(&buyer));
    assert_eq!(
        round.closed_market_visits["betriss_grain"].end_reason,
        MarketVisitEnd::TravelExpired
    );
    world.assert_invariants();
}

/// The road's recall ends a live `go_to` through [`end_intent`], so the mind
/// that issued it is told why the body gave up — silent abandonment leaves it
/// believing an untruth (05_the_llm_seam.md §2). The one exception is a member
/// in the law's hands: the departure goes without them and the seizure's own
/// percepts already own their story.
#[test]
fn the_road_return_ends_a_live_intent_with_the_why_percept() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    world.add_character(person(
        "srgnt",
        Vec3::ZERO,
        Some("bailiff_and_gaoler"),
        Significance::Minor,
    ));
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let leader = ActorId::from_raw("rbrde");
    let carter = ActorId::from_raw("cbred");
    let officer = ActorId::from_raw("srgnt");

    let errand = |name: &str| {
        Some(TravelIntent {
            target: IntentTarget::Place {
                place_id: PlaceId::from_raw(name),
                name: name.into(),
                point: Vec3::new(100.0, WALK_Y, 100.0),
            },
            budget_seconds: 100.0,
            deadline: Some(100.0),
        })
    };
    world.characters.get_mut(&leader).unwrap().state.intent = errand("somewhere");
    world.characters.get_mut(&carter).unwrap().state.intent = errand("elsewhere");
    let station =
        crate::custody::nearest_station(&world.places, world.characters[&carter].position_m())
            .expect("postings");
    world
        .custody
        .seize(carter.clone(), officer.clone(), Some(1), station, 0.0);

    let mut nudges = Vec::new();
    round.begin_road_return(&mut world, &party_id, 2, &mut nudges);

    assert!(world.characters[&leader].state.intent.is_none());
    assert!(
        world.characters[&leader]
            .inbox()
            .iter()
            .any(|line| line
                == "The road turned you back before you reached somewhere — the party is leaving for the gate."),
        "the recall says why: {:?}",
        world.characters[&leader].inbox()
    );
    assert!(
        nudges.contains(&leader),
        "the lapse grants the same priority turn every end_intent does"
    );
    assert!(world.characters[&carter].state.intent.is_none());
    assert!(
        !world.characters[&carter]
            .inbox()
            .iter()
            .any(|line| line.contains("The road turned you back")),
        "a held member is not told they are leaving — they are not"
    );
    assert!(!nudges.contains(&carter));
    world.assert_invariants();
}

/// A member's offers expire at the gate through the same courtesy machinery
/// the distance sweep uses: both parties are told, and the `lapse_offer`
/// event the HUD notice rides is emitted — a bare `offers.remove` left the
/// other party's arm out toward a promise that no longer existed.
#[test]
fn gate_expired_offers_lapse_with_percepts_and_the_event() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let carter = ActorId::from_raw("cbred");
    let city = ActorId::from_raw("city1");
    let gate = round.road_parties[&party_id].gate_point;

    world.add_character(person("city1", gate, None, Significance::Minor));
    let personal = world
        .add_stock(&carter, &stock("generic", 1), "road:test:personal")
        .unwrap();
    world.characters.get_mut(&city).unwrap().state.position_m =
        world.characters[&carter].position_m();
    apply_action(
        &mut world,
        &carter,
        "offer_item",
        &json!({"item_id": personal.as_str(), "target": city.as_str()}),
    )
    .unwrap();

    round.begin_road_return(&mut world, &party_id, 2, &mut Vec::new());
    for member in round.road_parties[&party_id].members.clone() {
        world.characters.get_mut(&member).unwrap().state.position_m = gate;
    }
    for actor in [&carter, &city] {
        world.characters.get_mut(actor).unwrap().state.inbox.clear();
    }
    world.drain_events();

    round.tick_road_parties(&mut world, &nav, clock.at(0.0), 0.0, &BTreeSet::new());

    assert!(!world.offers.contains_key(&personal));
    assert!(
        world.characters[&carter]
            .inbox()
            .iter()
            .any(|line| line.contains("You are leaving through the gate")
                && line.contains("is yours again")),
        "the giver hears the expiry: {:?}",
        world.characters[&carter].inbox()
    );
    assert!(
        world.characters[&city]
            .inbox()
            .iter()
            .any(|line| line.contains("is leaving through the gate")
                && line.contains("no longer on offer")),
        "the target hears it too: {:?}",
        world.characters[&city].inbox()
    );
    let events = world.drain_events();
    let lapse = events
        .iter()
        .find(|event| event.kind == "lapse_offer")
        .expect("the gate expiry emits the event the HUD rides");
    assert_eq!(lapse.item_id.as_ref(), Some(&personal));
    assert_eq!(lapse.actor_id.as_ref(), Some(&carter));
    assert_eq!(lapse.target_id.as_ref(), Some(&city));
    assert_eq!(lapse.recipient_ids, vec![carter, city]);
    world.assert_invariants();
}

/// A returning carrier caught mid-conversation gets the road's own pressure
/// line and one turn's grace before the body walks — the ladder's `excused`
/// mechanism, carried by the party because road members are never enrolled in
/// `people`. Before this, the body simply strode off to the gate the moment
/// the exchange went cold, mid-topic, with no `system:` excuse ever injected.
#[test]
fn a_conversing_carrier_is_pressured_and_granted_one_grace_beat() {
    let nav = nav();
    let clock = clock_on(Office::Dayspring, 2);
    let mut world = road_party_world();
    let mut round = Round::new();
    round.seed(&mut world, &nav, 0.0, &clock);
    let party_id = PartyId::from_raw("brede_wool_gate");
    let carter = ActorId::from_raw("cbred");
    // Far from the gate, on a walkable pitch, so the fall-through walk is real.
    let away = round.counters["brede_grain_seven_lofts"].pitch;
    world.characters.get_mut(&carter).unwrap().state.position_m = away;

    round.begin_road_return(&mut world, &party_id, 2, &mut Vec::new());
    let held = warm(&carter);
    let pressure_lines = |world: &World| {
        world.characters[&carter]
            .inbox()
            .iter()
            .filter(|line| line.contains("the road party is leaving"))
            .count()
    };

    round.tick_road_parties(&mut world, &nav, clock.at(0.0), 0.0, &held);
    assert_eq!(pressure_lines(&world), 1, "the pressure is said, once");
    assert!(
        world.characters[&carter].state.movement.is_none(),
        "the percept comes before the feet"
    );

    // Inside the grace (the jitter floor is 1 s): still standing, still told
    // exactly once.
    round.tick_road_parties(&mut world, &nav, clock.at(0.9), 0.9, &held);
    assert_eq!(pressure_lines(&world), 1);
    assert!(world.characters[&carter].state.movement.is_none());

    // Grace spent (past the 6 s jitter ceiling): the body walks even though
    // the exchange is still warm — exactly the ladder's second pressing
    // decision.
    round.tick_road_parties(&mut world, &nav, clock.at(10.0), 10.0, &held);
    assert_eq!(pressure_lines(&world), 1, "the pressure is never repeated");
    assert!(
        world.characters[&carter].is_walking(),
        "one grace beat, then the gate"
    );
    world.assert_invariants();
}

#[test]
fn household_settlement_redistributes_before_minting_only_the_residual() {
    let mut world = base_world();
    for id in ["a0000", "b0000", "c0000"] {
        world.add_character(person(id, Vec3::ZERO, None, Significance::Minor));
    }
    let donor = ActorId::from_raw("a0000");
    let first = ActorId::from_raw("b0000");
    let second = ActorId::from_raw("c0000");
    world.credit_sparks(&donor, 10, "settlement:donor").unwrap();
    world
        .credit_sparks(&second, 2, "settlement:second")
        .unwrap();
    let cargo = world
        .add_stock(&donor, &stock("grain", 3), "settlement:cargo")
        .unwrap();
    let mut round = Round::new();
    round.household_reserves.insert(donor.clone(), 6);
    round.household_reserves.insert(first.clone(), 0);
    round.household_reserves.insert(second.clone(), 2);

    let receipt = round.settle_households(&mut world, 12).unwrap();
    assert_eq!(receipt.transfers.len(), 1);
    assert_eq!(receipt.transfers[0].donor, donor);
    assert_eq!(receipt.transfers[0].recipient, first);
    assert_eq!(receipt.transfers[0].sparks, 4);
    assert_eq!(receipt.institutional_payroll_sparks, 2);
    assert_eq!(world.spendable_sparks(&ActorId::from_raw("a0000")), 6);
    assert_eq!(world.spendable_sparks(&ActorId::from_raw("b0000")), 4);
    assert_eq!(world.spendable_sparks(&ActorId::from_raw("c0000")), 4);
    assert_eq!(
        world.items[&cargo].quantity, 3,
        "settlement never touches stock"
    );
    world.assert_invariants();
}

#[test]
fn the_watch_dispatcher_reports_a_skipped_day_then_recovers_without_a_false_positive() {
    let mut world = base_world();
    world.add_character(person("a0000", Vec3::ZERO, None, Significance::Minor));
    let resident = ActorId::from_raw("a0000");
    world
        .credit_sparks(&resident, 5, "settlement:positive")
        .unwrap();
    let mut round = Round::new();

    // Day 3 was sampled but its handler was skipped: the next Watch must say
    // so even though no resident has a zero wallet.
    round.last_household_watch_day = Some(3);
    round.last_household_settlement_day = Some(2);
    round.dispatch_household_settlement(&mut world, 4);
    assert_eq!(round.last_household_watch_day(), Some(4));
    assert_eq!(round.last_household_settlement_day(), Some(4));
    let trace = round.drain_food_log();
    assert!(
        trace
            .iter()
            .any(|line| line.contains("household_settlement_missed: sampled day 3"))
    );
    assert!(
        trace
            .iter()
            .any(|line| line.starts_with("household_settlement: day 4"))
    );

    // A completed day is paired with its sample, so the following Watch has no
    // false missed-settlement report.
    round.dispatch_household_settlement(&mut world, 5);
    let trace = round.drain_food_log();
    assert!(
        trace
            .iter()
            .all(|line| !line.starts_with("household_settlement_missed"))
    );
    assert_eq!(round.last_household_settlement_day(), Some(5));
}

#[test]
fn a_failed_watch_handler_is_left_incomplete_and_reported_next_watch() {
    let mut world = base_world();
    world.add_character(person("a0000", Vec3::ZERO, None, Significance::Minor));
    let resident = ActorId::from_raw("a0000");
    let purse = world
        .add_stock(
            &resident,
            &stock("spark", u32::MAX),
            "settlement:full-purse",
        )
        .unwrap();
    world.offers.insert(
        purse.clone(),
        Offer {
            item_id: purse.clone(),
            giver_id: resident.clone(),
            target_id: None,
            created_seq: 1,
            quantity: u32::MAX - 1,
        },
    );
    assert_eq!(world.spendable_sparks(&resident), 1);
    let mut round = Round::new();
    round.dispatch_household_settlement(&mut world, 7);

    assert_eq!(round.last_household_watch_day(), Some(7));
    assert_eq!(round.last_household_settlement_day(), None);
    assert!(
        round
            .drain_food_log()
            .iter()
            .any(|line| line.starts_with("household_settlement_failed: day 7"))
    );

    // Releasing the commitment makes the next handler valid. The dispatcher
    // first reports the incomplete day, then records this successful one.
    world.offers.remove(&purse);
    round.dispatch_household_settlement(&mut world, 8);
    let trace = round.drain_food_log();
    assert!(
        trace
            .iter()
            .any(|line| line.contains("household_settlement_missed: sampled day 7"))
    );
    assert!(
        trace
            .iter()
            .any(|line| line.starts_with("household_settlement: day 8"))
    );
    assert_eq!(round.last_household_settlement_day(), Some(8));
    world.assert_invariants();
}

fn active_production_fixture(work_minutes: u32) -> (World, Round, ActorId, WorldClock) {
    let producer = ActorId::from_raw("m0001");
    let mill = Vec3::new(12.0, WALK_Y, 9.0);
    let home = Vec3::new(-5.0, WALK_Y, -3.0);
    let mut world = base_world();
    world.add_character(person(
        producer.as_str(),
        mill,
        Some("miller"),
        Significance::Minor,
    ));
    let grain = world
        .add_stock(&producer, &stock("grain", 1), "production_fixture:grain")
        .unwrap();
    world
        .start_transform_job(TransformJob {
            job_id: "m0001:mill_flour:1:0".into(),
            spec_id: "mill_flour".into(),
            producer: producer.clone(),
            production_day: 1,
            start_slot: 0,
            inputs: vec![ReservedInput {
                item_id: grain,
                quantity: 1,
            }],
            outputs: vec![stock("flour", 1)],
            progress_work_minutes: 0.0,
        })
        .unwrap();

    let mut round = Round::new();
    round.people.insert(
        producer.clone(),
        Townsperson {
            home: Some(home),
            base: home,
            legs: vec![
                RoundLeg {
                    from: Office::Waning,
                    at: mill,
                    label: "Test Mill".into(),
                    doing: Arrival::Work,
                    only_on: None,
                    is_home: false,
                },
                RoundLeg {
                    from: Office::Lamplight,
                    at: home,
                    label: "home".into(),
                    doing: Arrival::Sleep,
                    only_on: None,
                    is_home: true,
                },
            ],
            leash_m: DEFAULT_ROUND_LEASH_M,
            curfew_exempt: false,
            source: None,
            is_household: false,
            food: None,
            phase: Phase::Idle,
            travel_target: None,
            travel_for_intent: false,
            next_decision: 0.0,
            epoch: 0,
            evening_seed: None,
            excused: false,
        },
    );
    round.production_plans.push(ResolvedProductionPlan {
        producer: producer.clone(),
        max_jobs_per_day: 2,
        transforms: vec![ResolvedTransformSpec {
            id: "mill_flour".into(),
            site: "Test Mill".into(),
            point: mill,
            consumes: vec![stock("grain", 1)],
            produces: vec![stock("flour", 1)],
            allowed_offices: vec![Office::Waning],
            work_minutes,
            desired_output_quantity: 4,
        }],
    });
    // One real second per game minute makes the expected spans legible.
    let clock = WorldClock::new(1_440.0, Office::Waning, 1, 0.05);
    round.production_last_game_days = clock.game_days(0.0);
    round.production_was_eligible.insert(producer.clone(), true);
    (world, round, producer, clock)
}

#[test]
fn coarse_production_jump_credits_only_the_open_authored_work_span() {
    let (mut world, mut round, producer, clock) = active_production_fixture(240);

    // 15:00 in the Waning to 02:00 next day crosses four offices. Only the
    // authored 15:00–18:00 Work leg is eligible: exactly 180 game minutes.
    round.tick_production(&mut world, &clock, 11.0 * 60.0, &BTreeSet::new());

    assert_eq!(clock.at(11.0 * 60.0).office, Office::Watch);
    let progress = world
        .active_transform_job(&producer)
        .unwrap()
        .progress_work_minutes;
    assert!(
        (progress - 180.0).abs() < 1e-6,
        "credited {progress} minutes"
    );
}

#[test]
fn conversation_pauses_production_without_retroactive_credit() {
    let (mut world, mut round, producer, clock) = active_production_fixture(240);

    // The first half-hour ended in conversation, so it earns nothing. Ending
    // the conversation establishes a fresh eligible endpoint but cannot earn
    // the preceding half-hour. Only the third interval is credited.
    round.tick_production(&mut world, &clock, 30.0, &warm(&producer));
    round.tick_production(&mut world, &clock, 60.0, &BTreeSet::new());
    round.tick_production(&mut world, &clock, 90.0, &BTreeSet::new());

    let progress = world
        .active_transform_job(&producer)
        .unwrap()
        .progress_work_minutes;
    assert!(
        (progress - 30.0).abs() < 1e-6,
        "credited {progress} minutes"
    );
}

#[test]
fn production_target_counts_every_output_with_the_same_stacking_key() {
    let (mut world, mut round, producer, clock) = active_production_fixture(45);
    world.transform_jobs.clear();
    let transform = &mut round.production_plans[0].transforms[0];
    transform.produces = vec![stock("flour", 1), stock("flour", 1)];
    transform.desired_output_quantity = 1;

    round.tick_production(&mut world, &clock, 0.0, &BTreeSet::new());
    assert!(
        world.active_transform_job(&producer).is_none(),
        "a two-unit batch cannot start against a one-unit target"
    );

    round.production_plans[0].transforms[0].desired_output_quantity = 2;
    round.tick_production(&mut world, &clock, 0.0, &BTreeSet::new());
    let job = world
        .active_transform_job(&producer)
        .expect("the same batch starts when the full output fits");
    assert_eq!(
        job.outputs
            .iter()
            .map(|output| output.quantity)
            .sum::<u32>(),
        2
    );
}

/// A registered destination for the round-edit guard tests, built by hand the
/// way the Night Office's own tests build their two-place registries.
fn register_place(world: &mut World, id: &str, name: &str, point: Vec3) -> PlaceId {
    let place_id = PlaceId::from_raw(id);
    world
        .places
        .insert(crate::places::PlaceEntry {
            id: place_id.clone(),
            name: name.into(),
            point,
            ward: None,
            coarse: true,
        })
        .expect("the test place registers");
    place_id
}

/// A night edit that would rename the one Work leg a half-done transform
/// accrues on is refused when the round applies it, and the author hears why —
/// while an edit to the same author's *unbound* leg still lands: only the
/// trade's leg is held, never the whole sheet.
#[test]
fn a_round_edit_that_would_strand_a_live_transform_is_refused_with_the_reason() {
    let (mut world, mut round, producer, _clock) = active_production_fixture(240);
    let bellstand = register_place(
        &mut world,
        "pl_bell",
        "The Bellstand",
        Vec3::new(30.0, WALK_Y, 30.0),
    );

    world
        .characters
        .get_mut(&producer)
        .unwrap()
        .state
        .round_edit = Some(RoundEdit {
        leg: 0,
        place_id: bellstand.clone(),
    });
    apply_round_edits(&mut round, &mut world, 0.0);

    assert!(
        world.characters[&producer].state.round_edit.is_none(),
        "a refused edit is consumed, not retried every tick"
    );
    assert_eq!(
        round.people[&producer].legs[0].label, "Test Mill",
        "the Work leg the job accrues on stands"
    );
    assert!(
        world.characters[&producer]
            .pending_history()
            .iter()
            .any(|line| line.contains("Test Mill") && line.contains("holds you there")),
        "the author learns the reason: {:?}",
        world.characters[&producer].pending_history()
    );

    // The sleep leg binds nothing, so the same destination is fine there.
    world
        .characters
        .get_mut(&producer)
        .unwrap()
        .state
        .round_edit = Some(RoundEdit {
        leg: 1,
        place_id: bellstand,
    });
    apply_round_edits(&mut round, &mut world, 0.0);
    assert_eq!(
        round.people[&producer].legs[1].label, "The Bellstand",
        "the unbound leg moved"
    );
    assert!(
        world.active_transform_job(&producer).is_some(),
        "the job rides out the harmless edit untouched"
    );
}

/// The stall arm of the same guard: the bound keeper's only leg at the stall's
/// site cannot be renamed out from under tomorrow's `bind_vendors`.
#[test]
fn a_round_edit_that_would_unstaff_a_stall_is_refused_with_the_reason() {
    let (mut world, mut round, keeper, _clock) = active_production_fixture(240);
    // Isolate the stall arm from the production arm.
    world.transform_jobs.clear();
    round.stalls.push(FoodStall {
        name: "Flour Board".into(),
        site: "Test Mill".into(),
        pitch: Vec3::ZERO,
        trade: "bread".into(),
        vendor: Some(keeper.clone()),
        queue: Vec::new(),
        serving: None,
        preferred: None,
        open: OpenSpec {
            offices: vec![Office::HighWick],
            weekdays: None,
        },
        cry_next: 0.0,
    });
    let bellstand = register_place(
        &mut world,
        "pl_bell",
        "The Bellstand",
        Vec3::new(30.0, WALK_Y, 30.0),
    );

    world.characters.get_mut(&keeper).unwrap().state.round_edit = Some(RoundEdit {
        leg: 0,
        place_id: bellstand,
    });
    apply_round_edits(&mut round, &mut world, 0.0);

    assert_eq!(
        round.people[&keeper].legs[0].label, "Test Mill",
        "the keeper's routing leg stands"
    );
    assert!(
        world.characters[&keeper]
            .pending_history()
            .iter()
            .any(|line| line.contains("Your stall at Test Mill")),
        "the keeper learns the reason: {:?}",
        world.characters[&keeper].pending_history()
    );
}

/// The safety net under the guard: a job stranded any other way — here the leg
/// label simply dies out from under it — releases its reservations after a
/// full idle game day, and the stock is usable again.
#[test]
fn a_stranded_transform_releases_its_reservations_after_a_full_idle_day() {
    let (mut world, mut round, producer, clock) = active_production_fixture(240);
    let grain = world.active_transform_job(&producer).unwrap().inputs[0]
        .item_id
        .clone();
    assert_eq!(
        world.uncommitted_quantity(&grain),
        0,
        "the input is committed while the job lives"
    );

    // Sever the leg-label binding directly: from the job's point of view every
    // lost worker — dead route data, a guard bypass, a future system — looks
    // exactly like this, and not a minute ever accrues again.
    round.people.get_mut(&producer).unwrap().legs[0].label = "Somewhere Else".into();

    // The fixture clock runs 1440 real seconds per game day. Half a day of
    // stillness is an honest pause, not abandonment.
    round.tick_production(&mut world, &clock, 0.0, &BTreeSet::new());
    round.tick_production(&mut world, &clock, 720.0, &BTreeSet::new());
    assert!(
        world.active_transform_job(&producer).is_some(),
        "half a day idle is still just paused"
    );

    round.tick_production(&mut world, &clock, 1441.0, &BTreeSet::new());
    assert!(
        world.active_transform_job(&producer).is_none(),
        "a full idle day abandons the job"
    );
    assert_eq!(
        world.uncommitted_quantity(&grain),
        1,
        "the reserved grain is uncommitted stock again"
    );
    let trace = round.drain_food_log();
    assert!(
        trace
            .iter()
            .any(|line| line.starts_with("transform_abandoned: producer")),
        "the abandonment is traced: {trace:?}"
    );
    world.assert_invariants();
}

/// A job whose progress figure keeps moving restarts the watchdog window every
/// pump; only a figure sitting exactly still for a whole day is swept.
#[test]
fn a_transform_still_accruing_work_is_never_swept() {
    let (mut world, mut round, producer, clock) = active_production_fixture(100_000);

    // Each pump a day apart crosses the authored Waning Work window, so the
    // progress figure moves between every pair of observations.
    round.tick_production(&mut world, &clock, 0.0, &BTreeSet::new());
    round.tick_production(&mut world, &clock, 1441.0, &BTreeSet::new());
    round.tick_production(&mut world, &clock, 2882.0, &BTreeSet::new());

    let job = world
        .active_transform_job(&producer)
        .expect("a working job is never abandoned");
    assert!(job.progress_work_minutes > 0.0, "work really accrued");
}

// --------------------------------------------------------------------------- //
// Weather shelter and exposed-market policy.
// --------------------------------------------------------------------------- //

fn weather_person(position: Vec3) -> Townsperson {
    Townsperson {
        home: None,
        base: position,
        legs: Vec::new(),
        leash_m: DEFAULT_ROUND_LEASH_M,
        curfew_exempt: false,
        source: None,
        is_household: false,
        food: None,
        phase: Phase::Idle,
        travel_target: None,
        travel_for_intent: false,
        next_decision: 0.0,
        epoch: 0,
        evening_seed: None,
        excused: false,
    }
}

fn test_weather(kind: WeatherKind, precipitation: f64) -> WeatherSample {
    WeatherSample {
        kind,
        cloud_cover: if matches!(kind, WeatherKind::Thunderstorm) {
            1.0
        } else {
            0.85
        },
        precipitation_kind: if precipitation > 0.0 {
            crate::PrecipitationKind::Rain
        } else {
            crate::PrecipitationKind::None
        },
        precipitation,
        wind_xz_mps: [3.0, -1.0],
        gust: 0.4,
        fog: 0.0,
        visibility_m: 120.0,
        surface_wetness: precipitation,
        standing_water: (precipitation - 0.5).max(0.0),
        thunder: matches!(kind, WeatherKind::Thunderstorm) as u8 as f64,
        semantic_revision: 41,
    }
}

#[test]
fn exposed_actor_uses_reachable_shelter_after_conversation_and_atomic_work() {
    let nav = nav();
    // Node 35 is a street node north-east of Bellfoot Passage (post-shrink
    // graph) with a route through the passage nodes. It is deliberately
    // outside every authored polygon and clear of door eaves.
    let start = nav.node_point(35);
    let actor = ActorId::from_raw("rainy");
    let mut world = base_world();
    world.add_character(person("rainy", start, Some("baker"), Significance::Minor));
    world.shelters = std::sync::Arc::new(
        crate::ShelterMap::from_json_str(include_str!("../../../../assets/world/shelters.json"))
            .expect("committed shelter data loads"),
    );
    world.current_weather = Some(test_weather(WeatherKind::Downpour, 0.88));
    assert!(!world.shelters.is_sheltered(start));

    let mut round = Round::default();
    round.people.insert(actor.clone(), weather_person(start));
    let clock = clock_at(Office::Dayspring);
    let mut nudges = Vec::new();

    // A live exchange retains precedence; it neither starts nor loses a
    // hidden weather intent.
    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        0.0,
        &warm(&actor),
        &mut nudges,
    );
    assert!(round.weather_shelter(&world, &actor).is_none());

    // An atomic well draw is equally committed.
    round.people.get_mut(&actor).unwrap().phase = Phase::Drawing;
    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        0.0,
        &BTreeSet::new(),
        &mut nudges,
    );
    assert!(round.weather_shelter(&world, &actor).is_none());

    // Once free, the same deterministic ladder claims the nearest public
    // shelter and lays a real route ending at a stable point inside its
    // covered polygon.
    round.people.get_mut(&actor).unwrap().phase = Phase::Idle;
    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        0.0,
        &BTreeSet::new(),
        &mut nudges,
    );
    assert_eq!(
        round.weather_shelter(&world, &actor),
        Some("bellstand_hearth")
    );
    let intent = round.weather_shelter_intents[&actor];
    assert!(world.shelters.shelters()[intent.shelter].contains(intent.target));
    assert!(
        world.characters[&actor].is_walking(),
        "the actor takes a routed walk"
    );

    // Clearing does not empty the shelter immediately. Nine game minutes is
    // below every actor's grace; twenty-one is above every 10--20 minute roll.
    world.current_weather = Some(test_weather(WeatherKind::Clear, 0.0));
    update_weather_shelter_intents(&mut round, &mut world, 0.0, 0.0);
    update_weather_shelter_intents(&mut round, &mut world, 9.0 / 1_440.0, 9.0);
    assert!(round.weather_shelter(&world, &actor).is_some());
    update_weather_shelter_intents(&mut round, &mut world, 21.0 / 1_440.0, 21.0);
    assert!(round.weather_shelter(&world, &actor).is_none());
}

/// A claim taken in the street dies at the holder's own door. The famished
/// rung can send a shelter-seeker home to the hearth mid-walk; the stored
/// claim sits above the round in `decide`, so without this release the fed
/// resident would be marched back out of a dry house into the rain — holding
/// the awning's capacity slot against a soaked neighbour the whole time.
#[test]
fn a_shelter_claim_dies_at_the_holders_own_door() {
    let home = Vec3::new(0.0, WALK_Y, 95.0);
    let away = Vec3::new(30.0, WALK_Y, 95.0);
    let id = ActorId::from_raw("homer");
    let mut world = base_world();
    world.add_character(person("homer", away, Some("baker"), Significance::Minor));
    // Hard rain the whole test: the hysteresis can never be the releaser.
    world.current_weather = Some(test_weather(WeatherKind::Downpour, 0.88));
    let mut round = Round::default();
    let mut homer = weather_person(away);
    homer.home = Some(home);
    round.people.insert(id.clone(), homer);
    round.weather_shelter_intents.insert(
        id.clone(),
        WeatherShelterIntent {
            shelter: 0,
            target: Vec3::new(-30.0, WALK_Y, 95.0),
            release_threshold: 0.22,
            below_since_days: None,
            release_after_days: 1.0,
        },
    );

    // In the street the claim stands through the sweep.
    update_weather_shelter_intents(&mut round, &mut world, 0.0, 0.0);
    assert!(
        round.weather_shelter_intents.contains_key(&id),
        "away from home, the claim is kept"
    );

    // At their own door it is released, rain or no rain.
    world.characters.get_mut(&id).unwrap().state.position_m = home;
    update_weather_shelter_intents(&mut round, &mut world, 0.0, 1.0);
    assert!(
        !round.weather_shelter_intents.contains_key(&id),
        "the hearth outranks the awning"
    );
}

/// The pause after answering a line is the answer's beat for a *follow* too:
/// `interrupt_for_conversation` stands the walker down, and the tracking
/// re-lay waits out the ladder cadence exactly as a place walk does. Per-poll
/// resumption had the follower back on their feet one movement tick later,
/// chasing a moving target out of the 20 m say radius while their reply was
/// still being written.
#[test]
fn an_interrupted_follow_waits_out_the_answers_beat() {
    let nav = nav();
    let start = Vec3::new(0.0, WALK_Y, 95.0);
    let quarry_at = Vec3::new(0.0, WALK_Y, 105.0);
    let chaser = ActorId::from_raw("chaser");
    let mut world = base_world();
    world.add_character(person("chaser", start, Some("mason"), Significance::Minor));
    world.add_character(person(
        "quarry",
        quarry_at,
        Some("baker"),
        Significance::Minor,
    ));
    let mut round = Round::default();
    round.people.insert(chaser.clone(), weather_person(start));
    // A live follow already under way (the stamped deadline marks it un-fresh),
    // walking when the player's line lands.
    world.characters.get_mut(&chaser).unwrap().state.intent = Some(TravelIntent {
        target: IntentTarget::Person {
            actor_id: ActorId::from_raw("quarry"),
            last_seen: quarry_at,
            visible: true,
        },
        budget_seconds: 600.0,
        deadline: Some(600.0),
    });
    {
        let person = round.people.get_mut(&chaser).unwrap();
        person.phase = Phase::Travelling;
        person.travel_target = Some(quarry_at);
        person.travel_for_intent = true;
        // The cadence the ladder would have scheduled from its last decision.
        person.next_decision = 5.0;
    }
    interrupt_for_conversation(&mut round, &mut world, &chaser);
    assert_eq!(round.people[&chaser].phase, Phase::Idle);

    // The very next poll does not put them back on their feet.
    let mut nudges = Vec::new();
    tick_intents(&mut round, &mut world, &nav, 1.0, &mut nudges);
    assert!(
        world.characters[&chaser].state.movement.is_none(),
        "the answer's beat holds the follow"
    );
    assert_eq!(round.people[&chaser].phase, Phase::Idle);

    // Once the cadence comes due, the chase resumes.
    tick_intents(&mut round, &mut world, &nav, 5.0, &mut nudges);
    assert!(
        world.characters[&chaser].state.movement.is_some(),
        "the cadence resumes the chase"
    );
    assert_eq!(round.people[&chaser].phase, Phase::Travelling);
}

#[test]
fn every_committed_shelter_has_a_valid_route_node_and_walkable_spread_point() {
    let nav = nav();
    let shelters =
        crate::ShelterMap::from_json_str(include_str!("../../../../assets/world/shelters.json"))
            .expect("committed shelter data loads");
    let actor = ActorId::from_raw("probe");
    for (index, shelter) in shelters.shelters().iter().enumerate() {
        assert!(
            shelter.route_node < nav.node_count(),
            "{} route node",
            shelter.id
        );
        let target = shelter_spread_target(&nav, shelter, &actor, index)
            .unwrap_or_else(|| panic!("{} has no walkable spread point", shelter.id));
        assert!(shelter.contains(target), "{} target {target:?}", shelter.id);
        assert!(
            nav.is_walkable(target.x, target.z),
            "{} target is not walkable",
            shelter.id
        );
    }
}

#[test]
fn every_food_pitch_uses_the_shared_shelter_map() {
    let nav = nav();
    let mut world = base_world();
    world.shelters = std::sync::Arc::new(
        crate::ShelterMap::from_json_str(include_str!("../../../../assets/world/shelters.json"))
            .expect("committed shelter data loads"),
    );
    let mut round = Round::default();
    round.seed(&mut world, &nav, 0.0, &clock_at(Office::HighWick));

    assert_eq!(
        round.stalls.len(),
        13,
        "the committed food document still has thirteen stalls \
         (eight food + the lore-wave chandlery/wares/simples/badge pitches)"
    );
    for stall in &round.stalls {
        assert!(
            world.shelters.is_sheltered(stall.pitch),
            "{} pitch {:?} should lie under its authored awning or hearth roof",
            stall.name,
            stall.pitch,
        );
    }
}

#[test]
fn nearby_lightning_causes_a_short_reflex_without_breaking_conversation() {
    let nav = nav();
    // Street nodes near the Bellfoot corner (post-shrink graph); the start is
    // outside every shelter polygon and clear of door eaves.
    let start = nav.node_point(35);
    let destination = nav.node_point(34);
    let actor = ActorId::from_raw("flash");
    let mut world = base_world();
    world.add_character(person("flash", start, Some("mason"), Significance::Minor));
    world.shelters = std::sync::Arc::new(
        crate::ShelterMap::from_json_str(include_str!("../../../../assets/world/shelters.json"))
            .expect("committed shelter data loads"),
    );
    world.current_weather = Some(test_weather(WeatherKind::Thunderstorm, 0.9));
    assert!(!world.shelters.is_sheltered(start));

    let mut round = Round::default();
    let mut townsman = weather_person(start);
    townsman.phase = Phase::Travelling;
    townsman.travel_target = Some(destination);
    round.people.insert(actor.clone(), townsman);
    set_route(&mut world, &actor, vec![destination]);
    let strike = crate::LightningStrike {
        id: 901,
        game_instant_days: 0.0,
        origin_m: [start.x, 520.0, start.z],
        strength: 0.8,
    };
    round.note_lightning(&world, &strike, 10.0);

    let clock = clock_at(Office::HighWick);
    let mut nudges = Vec::new();
    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        10.0,
        &warm(&actor),
        &mut nudges,
    );
    assert!(
        world.characters[&actor].is_walking(),
        "a reflex does not break an active exchange"
    );

    run_ladder(
        &mut round,
        &mut world,
        &nav,
        &clock,
        10.0,
        &BTreeSet::new(),
        &mut nudges,
    );
    assert!(
        !world.characters[&actor].is_walking(),
        "the free exposed walker stops for the flash"
    );
    assert_eq!(round.people[&actor].phase, Phase::Idle);

    round
        .lightning_reflex_until
        .retain(|_, until| 14.1 < *until);
    assert!(
        !matches!(
            decide(
                &round,
                &world,
                &nav,
                &actor,
                0,
                Office::HighWick,
                Weekday::Bellday,
            )
            .0,
            Decision::WeatherPause
        ),
        "the reflex expires within four real seconds"
    );
}

#[test]
fn exposed_stall_pauses_and_resumes_without_changing_stock() {
    let (mut world, mut round, _vendor, buyer, stock_id) = bread_stall_world();
    let pitch = round.stalls[0].pitch;
    round.people.insert(buyer.clone(), weather_person(pitch));
    round.people.get_mut(&buyer).unwrap().food = Some(FoodErrand {
        stall: 0,
        phase: FoodPhase::Queued,
    });
    let clock = clock_at(Office::HighWick);

    world.current_weather = Some(test_weather(WeatherKind::Thunderstorm, 0.9));
    assert!(!stall_weather_open(&world, &round.stalls[0]));
    service_stalls(&mut round, &mut world, &clock, 0.0, &player());
    assert!(
        round.stalls[0].queue.is_empty(),
        "a bare pitch releases its queue"
    );
    assert_eq!(
        world.items[&stock_id].quantity, 3,
        "closing consumes no board stock"
    );

    world.current_weather = Some(test_weather(WeatherKind::Clear, 0.0));
    round.stalls[0].queue.push(buyer.clone());
    round.people.get_mut(&buyer).unwrap().food = Some(FoodErrand {
        stall: 0,
        phase: FoodPhase::Queued,
    });
    service_stalls(&mut round, &mut world, &clock, 0.0, &player());
    assert!(
        round.stalls[0].serving.is_some(),
        "the same stall resumes service"
    );
    assert_eq!(
        world.items[&stock_id].quantity, 3,
        "starting service does not duplicate stock"
    );
    world.current_weather = Some(test_weather(WeatherKind::Thunderstorm, 0.9));
    service_stalls(&mut round, &mut world, &clock, 0.5, &player());
    assert_eq!(
        world.items[&stock_id].quantity, 2,
        "the one atomic sale finishes as the exposed stall closes"
    );

    // The shared data, not a second market-only list, makes a covered pitch
    // stay open in the same storm.
    world.shelters = std::sync::Arc::new(
        crate::ShelterMap::from_json_str(include_str!("../../../../assets/world/shelters.json"))
            .expect("committed shelter data loads"),
    );
    world.current_weather = Some(test_weather(WeatherKind::Thunderstorm, 0.9));
    assert!(world.shelters.is_sheltered(Vec3::ZERO));
    assert!(stall_weather_open(&world, &round.stalls[0]));
}

// --------------------------------------------------------------------------- //
// Chalking the Walls M1 — the cross at the counter
// --------------------------------------------------------------------------- //

/// Register a home for `who` so a `MarkAnchor::Household` resolves, and chalk
/// a cross on it by the ward's own hand.
fn chalk_the_door(world: &mut World, who: &ActorId) -> crate::ids::MarkId {
    let point = world
        .characters
        .get(who)
        .expect("the buyer exists")
        .position_m();
    world.places.add_home(who, "Test Body", point);
    crate::marks::draw_or_refresh(
        world,
        crate::marks::MarkKind::ChalkCross,
        crate::marks::MarkAnchor::Household(who.clone()),
        None,
        0.0,
    )
    .expect("the door resolves")
    .id
}

/// **The partition test** (`features/implemented/chalking_the_walls.md` §2.1, §6). The
/// refusal must read the chalk and *only* the chalk. `World.notices` is empty
/// here — emphatically so — and the sale is still refused. If this test were
/// hard to write, the partition would already be broken.
#[test]
fn a_chalked_buyer_is_refused_at_the_counter_with_no_notice_anywhere() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_some(),
        "the buyer can afford a loaf before anybody chalks anything"
    );

    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    chalk_the_door(&mut world, &buyer);
    assert!(
        world.notices.live().is_empty(),
        "the premise: not one notice exists in this world"
    );

    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_none(),
        "the cross alone refuses the sale — no notice is consulted, and none exists"
    );
}

/// The other half of §2.1: a *notice* with no chalk refuses nothing. Together
/// these two prove the readers were repointed rather than doubled up.
#[test]
fn an_unchalked_buyer_sells_even_while_the_ward_is_talking_about_them() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    world.notices.raise(
        "a buyer".into(),
        "owes and has not paid".into(),
        None,
        None,
        Some(0.0),
        buyer.clone(),
        Some(buyer.clone()),
        None,
        None,
    );
    assert!(
        !world.notices.live().is_empty(),
        "the premise: a live notice"
    );

    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_some(),
        "a notice is not chalk: the counter reads the wall, not the gossip"
    );
}

/// §3: below `faint_below` a mark is a fact about the past, not a rule. A
/// half-washed cross must let the sale through — that is what makes weathering
/// (and therefore scrubbing) mean anything.
#[test]
fn a_half_washed_cross_no_longer_refuses_anybody() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    let mark = chalk_the_door(&mut world, &buyer);
    let faint = world
        .mark_catalog
        .spec(crate::marks::MarkKind::ChalkCross)
        .expect("the cross is authored")
        .faint_below;
    world
        .marks
        .get_mut(mark)
        .expect("the cross is live")
        .strength = faint - 0.01;

    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_some(),
        "chalk you can barely see stops ruling"
    );
}

/// §2.3: no reader may branch on `author`. A cross the *player* forged refuses
/// exactly as hard as the ward's own, because nothing that reads a mark ever
/// asks who drew it.
#[test]
fn a_forged_cross_refuses_exactly_as_hard_as_the_wards_own() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    let point = world.characters[&buyer].position_m();
    world.places.add_home(&buyer, "Test Body", point);
    crate::marks::draw_or_refresh(
        &mut world,
        crate::marks::MarkKind::ChalkCross,
        crate::marks::MarkAnchor::Household(buyer.clone()),
        // A forger, not the ward.
        Some(ActorId::from_raw("player")),
        0.0,
    )
    .expect("the door resolves");

    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_none(),
        "a forged cross must refuse the sale exactly as the ward's own does"
    );
}

/// The ablation switch must reach this reader too, or `CATHEDRAL_NO_MARKS`
/// would still refuse sales on chalk it claims not to be simulating.
#[test]
fn ablated_chalk_refuses_nobody() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    chalk_the_door(&mut world, &buyer);
    world.marks_enabled = false;

    assert!(
        try_purchase(&mut round, &mut world, 0, &buyer).is_some(),
        "an ablated layer must not go on refusing sales"
    );
}

/// The whole M1 arc through the real `service_stalls`, and the hazard the spec
/// warns about: a refused buyer who still has coin, at a stall that still has
/// bread, re-selects on every `next_decision`. Without the stamp that is an
/// infinite loop with one inbox line a lap.
#[test]
fn a_chalk_refusal_is_one_scene_and_not_a_loop() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    let clock = clock_at(Office::HighWick);
    let pitch = round.stalls[0].pitch;
    round.people.insert(buyer.clone(), weather_person(pitch));
    round.people.get_mut(&buyer).unwrap().food = Some(FoodErrand {
        stall: 0,
        phase: FoodPhase::Queued,
    });
    round.stalls[0].serving = Some((buyer.clone(), 0.0));
    chalk_the_door(&mut world, &buyer);

    service_stalls(
        &mut round,
        &mut world,
        &clock,
        PURCHASE_SECONDS + 0.1,
        &player(),
    );

    // One line, naming the reason — the bare `None` arm could never say why.
    let inbox = world.characters[&buyer].state.inbox.clone();
    assert_eq!(inbox.len(), 1, "exactly one refusal line, got {inbox:?}");
    assert!(
        inbox[0].contains("chalk cross on your door"),
        "the line must name the reason: {}",
        inbox[0]
    );

    // …and a trace line the food log can be grepped for.
    assert!(
        round
            .food_log
            .iter()
            .any(|line| line.starts_with("refused_on_chalk;")),
        "a chalk refusal must be distinguishable in --trace-food from \
         'spent it mid-queue', which the bare None arm never was: {:?}",
        round.food_log
    );

    // The stamp now keeps them away from every board, so the ladder cannot
    // march them straight back into the same queue.
    assert!(
        nearest_open_stall(
            &round,
            &world,
            &buyer,
            pitch,
            Office::HighWick,
            Weekday::Highmarket,
            false,
        )
        .is_none(),
        "a refused buyer must not re-select a stall while the stamp stands"
    );

    // Re-running the whole service pass many times adds no further lines: the
    // refusal is a scene, not a treadmill.
    for tick in 1..20 {
        round.people.get_mut(&buyer).unwrap().food = Some(FoodErrand {
            stall: 0,
            phase: FoodPhase::Queued,
        });
        round.stalls[0].serving = Some((buyer.clone(), tick as f64));
        service_stalls(
            &mut round,
            &mut world,
            &clock,
            tick as f64 + PURCHASE_SECONDS + 0.1,
            &player(),
        );
    }
    assert_eq!(
        world.characters[&buyer].state.inbox.len(),
        1,
        "still one line after twenty passes — no inbox barrage, no paid turns"
    );
}

/// …and the stamp really does expire, so a scrubbed cross lets them eat again
/// rather than starving them for good.
#[test]
fn the_refusal_stamp_lifts_on_its_own() {
    let (mut world, mut round, _vendor, buyer, _stock_id) = bread_stall_world();
    let clock = clock_at(Office::HighWick);
    let pitch = round.stalls[0].pitch;
    round.people.insert(buyer.clone(), weather_person(pitch));
    round.people.get_mut(&buyer).unwrap().food = Some(FoodErrand {
        stall: 0,
        phase: FoodPhase::Queued,
    });
    round.stalls[0].serving = Some((buyer.clone(), 0.0));
    chalk_the_door(&mut world, &buyer);
    service_stalls(
        &mut round,
        &mut world,
        &clock,
        PURCHASE_SECONDS + 0.1,
        &player(),
    );
    assert!(!round.chalk_refused_until.is_empty(), "stamped");

    // The tick's own prune, half a game day later.
    let later = PURCHASE_SECONDS + 0.1 + CHALK_REFUSAL_GAME_DAYS * clock.seconds_per_day() + 1.0;
    round.chalk_refused_until.retain(|_, until| later < *until);
    assert!(
        round.chalk_refused_until.is_empty(),
        "the stamp is a pause, not a ban"
    );
}

// --------------------------------------------------------------------------- //
// Chalking the Walls M4 — the tally moves a queue
// --------------------------------------------------------------------------- //

/// **The C7 regression.** The spec proposed putting the tally penalty inside
/// `Round::nearest_staffed_source`. That function has two callers and both are
/// enrolment-time — `Townsperson.source` is written once, in a struct literal,
/// for an actor's whole life — so a penalty there is evaluated at world-seed
/// t=0, when no chalk exists, and moves nobody ever.
///
/// This pins the fix: `tallied_source` re-picks per trip, and chalk laid *after*
/// enrolment really does send the next drawer elsewhere.
#[test]
fn a_notched_well_sends_the_next_drawer_to_the_neighbour() {
    let mut world = World::new();
    let mut round = Round::new();
    // Two staffed curbs. `near` is 10 m from the drawer, `far` is 25 m — so
    // with no chalk anywhere the near one wins by a wide margin.
    round.sources.push(WaterSource {
        name: "Chain Well".into(),
        draw_point: Vec3::new(10.0, WALK_Y, 0.0),
        draw_sound: "draw_water",
        keeper: Some(ActorId::from_raw("keep1")),
        queue: Vec::new(),
        serving: None,
        keeper_next_sound: 0.0,
    });
    round.sources.push(WaterSource {
        name: "Ford Well".into(),
        draw_point: Vec3::new(25.0, WALK_Y, 0.0),
        draw_sound: "draw_water",
        keeper: Some(ActorId::from_raw("keep2")),
        queue: Vec::new(),
        serving: None,
        keeper_next_sound: 0.0,
    });
    // The registry the anchors resolve through.
    let nav = nav();
    world.places = crate::places::PlaceRegistry::from_json(
        r#"{"schema_version":1,"places":[
            {"id":"pl_w1","name":"Chain Well","node":0,"kind":"landmark","ward":"weigh"},
            {"id":"pl_w2","name":"Ford Well","node":1,"kind":"landmark","ward":"fabric"}
        ],"wards":[]}"#,
        &nav,
    )
    .expect("the two-well registry parses");

    let here = Vec3::new(0.0, WALK_Y, 0.0);
    assert_eq!(
        tallied_source(&round, &world, here),
        Some(0),
        "with bare curbs the near well wins — identical to nearest_staffed_source"
    );

    // Three strokes on the near curb is 18 m of extra walk: 10 + 18 = 28 > 25.
    let (id, _) = {
        let drawn = crate::marks::draw_or_refresh(
            &mut world,
            crate::marks::MarkKind::WellTally,
            crate::marks::MarkAnchor::Place("Chain Well".into()),
            None,
            0.0,
        )
        .expect("the well is a registered place");
        (drawn.id, drawn.fresh)
    };
    world.marks.get_mut(id).expect("the tally is live").strokes = 3;

    assert_eq!(
        tallied_source(&round, &world, here),
        Some(1),
        "a busy well loses its next drawer to the neighbour"
    );

    // Overnight the chalk half-washes; a faint tally rules nothing, so the
    // near well recovers its queue.
    let faint = world
        .mark_catalog
        .spec(crate::marks::MarkKind::WellTally)
        .expect("authored")
        .faint_below;
    world.marks.get_mut(id).expect("live").strength = faint - 0.01;
    assert_eq!(
        tallied_source(&round, &world, here),
        Some(0),
        "and recovers it when the chalk washes off"
    );
}

/// Ties still break by source index, so an exact tie binds identically every
/// run — the property the spec asks to be kept.
#[test]
fn an_exact_tie_still_binds_by_source_index() {
    let mut world = World::new();
    let mut round = Round::new();
    for (name, x) in [("Chain Well", 10.0), ("Ford Well", -10.0)] {
        round.sources.push(WaterSource {
            name: name.into(),
            draw_point: Vec3::new(x, WALK_Y, 0.0),
            draw_sound: "draw_water",
            keeper: Some(ActorId::from_raw("keep")),
            queue: Vec::new(),
            serving: None,
            keeper_next_sound: 0.0,
        });
    }
    let nav = nav();
    world.places = crate::places::PlaceRegistry::from_json(
        r#"{"schema_version":1,"places":[
            {"id":"pl_w1","name":"Chain Well","node":0,"kind":"landmark","ward":"weigh"},
            {"id":"pl_w2","name":"Ford Well","node":1,"kind":"landmark","ward":"fabric"}
        ],"wards":[]}"#,
        &nav,
    )
    .expect("parses");
    assert_eq!(
        tallied_source(&round, &world, Vec3::new(0.0, WALK_Y, 0.0)),
        Some(0),
        "equidistant curbs bind to the lower source index, every run"
    );
}

/// M4's ward-sign reader: a chalked place of resort draws its own ward's
/// evening crowd away from the taverns. The comparison is against the *same*
/// roll with the sign suppressed, which is what the spec's done-criterion asks
/// for — an absolute count would prove nothing, since the roll is a pure hash
/// and most people stay home either way.
#[test]
fn a_chalked_ward_sign_pulls_that_wards_evening_crowd() {
    let evenings = |chalked: bool| -> Vec<String> {
        let nav = nav();
        let mut world = base_world();
        let ids: Vec<ActorId> = HOUSED_AMBIENT_IDS
            .iter()
            .enumerate()
            .map(|(index, id)| {
                world.add_character(person(
                    id,
                    Vec3::new(index as f64, WALK_Y, 95.0),
                    Some("mason"),
                    Significance::Ambient,
                ));
                ActorId::from_raw(*id)
            })
            .collect();
        let mut round = Round::new();
        round.seed(&mut world, &nav, 0.0, &clock_at(Office::Dayspring));
        if chalked {
            // Every ward's authored place, so whichever wards these bodies
            // belong to have a sign to be drawn to.
            let places: Vec<String> = world
                .mark_catalog
                .ward_sign_places()
                .map(|(_, place)| place.to_string())
                .collect();
            for place in places {
                crate::marks::draw_or_refresh(
                    &mut world,
                    crate::marks::MarkKind::WardSign,
                    crate::marks::MarkAnchor::Place(place),
                    None,
                    0.0,
                );
            }
        }
        round.reroll_ambient_evenings(&mut world, 0);
        // Where each mover's evening leg now points.
        ids.iter()
            .filter_map(|id| {
                round
                    .people
                    .get(id)
                    .and_then(|person| person.evening_seed.as_ref().map(|(index, _)| *index))
                    .and_then(|index| round.people[id].legs.get(index))
                    .map(|leg| leg.label.clone())
            })
            .collect()
    };

    let bare = evenings(false);
    let chalked = evenings(true);
    assert!(!bare.is_empty(), "somebody's evening moved on a bare night");
    assert_eq!(
        bare.len(),
        chalked.len(),
        "the same people move either way — the sign changes WHERE, not WHETHER"
    );

    let signs: Vec<String> = {
        let world = base_world();
        world
            .mark_catalog
            .ward_sign_places()
            .map(|(_, place)| place.to_string())
            .collect()
    };
    let to_a_sign = |destinations: &[String]| -> usize {
        destinations
            .iter()
            .filter(|label| signs.iter().any(|place| place == *label))
            .count()
    };
    assert_eq!(
        to_a_sign(&bare),
        0,
        "with no chalk anywhere, nobody walks to a place of resort: {bare:?}"
    );
    assert!(
        to_a_sign(&chalked) > 0,
        "a chalked sign must pull somebody off the tavern road: {chalked:?}"
    );
}
