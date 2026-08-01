//! Chalk on the walls, M0 — the medium (`features/chalking_the_walls.md` §4).
//!
//! The unit tests in `marks.rs` pin the catalog and the decay *curve*. These
//! pin the parts that need a real `World`: weathering under real weather and
//! real shelter, an anchor that stops resolving, and the sheet bullet.

use cathedral_sim::marks::{
    self, MARK_NOTICE_RADIUS_M, MARKS_MAX, Mark, MarkAnchor, MarkKind, Marks,
};
use cathedral_sim::math::Vec3;
use cathedral_sim::nav::NavData;
use cathedral_sim::places::PlaceRegistry;
use cathedral_sim::weather::{ShelterMap, WeatherSample};
use cathedral_sim::{ActorId, Character, CharacterSheet, Control, World};

/// An LLM character at `x` on the axis, the `sim_tests.rs` shape.
fn character(actor_id: &str, name: &str, x: f64) -> Character {
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(actor_id),
        name: name.to_string(),
        control: Control::Llm,
        back_story: "test".into(),
        location_description: "test square".into(),
        appearance: Default::default(),
        voice_key: Some(name.to_lowercase()),
        position_m: Vec3::new(x, 0.91, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: std::collections::BTreeSet::new(),
        lore: None,
        presence: cathedral_sim::Presence::InCity,
        presence_epoch: 0,
        economic_class: cathedral_sim::EconomicClass::Resident,
    })
}

/// A 60×10 line graph with four nodes, the shape the prompt tests use.
fn nav() -> NavData {
    let (w, h) = (60usize, 10usize);
    let bitset = vec![0xFF_u8; (w * h).div_ceil(8)];
    let nav_json = format!(
        r#"{{
          "schema_version": 1,
          "grid": {{"x0": -5.0, "z0": -5.0, "cell_m": 1.0, "w": {w}, "h": {h},
                    "agent_radius_m": 0.35, "bitset_file": "x.bin",
                    "bitset_bits": {bits}, "bitset_sha256": ""}},
          "nodes": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]],
          "edges": [[0, 1, 2.0], [1, 2, 2.0], [2, 3, 2.0]],
          "places": [{{"name": "a", "node": 0, "kind": "place"}},
                     {{"name": "b", "node": 3, "kind": "place"}}],
          "sites": [],
          "doors": [],
          "reference": {{"forecourt": 0}}
        }}"#,
        bits = w * h
    );
    NavData::from_parts(&nav_json, &bitset).unwrap()
}

/// Two registered places — node 0 and node 3, thirty metres apart — plus a
/// home for `debtor` at node 1 and one for `sheltered` at node 2.
fn world_with_places() -> World {
    let nav = nav();
    let mut world = World::new();
    let registry_json = r#"{
        "schema_version": 1,
        "places": [
            {"id": "pl_well0", "name": "Chain Well", "node": 0, "kind": "landmark", "ward": "weigh"},
            {"id": "pl_far00", "name": "Ford Well", "node": 3, "kind": "landmark", "ward": "fabric"}
        ],
        "wards": []
    }"#;
    world.places = PlaceRegistry::from_json(registry_json, &nav).unwrap();
    world.places.add_home(
        &ActorId::from_raw("debtor"),
        "Ede Clove",
        Vec3::new(10.0, 0.91, 0.0),
    );
    world.places.add_home(
        &ActorId::from_raw("housed"),
        "Tam Rud",
        Vec3::new(20.0, 0.91, 0.0),
    );
    world
}

fn chalk(world: &mut World, anchor: MarkAnchor) -> cathedral_sim::ids::MarkId {
    marks::draw_or_refresh(world, kind_for(&anchor), anchor, None, 0.0)
        .expect("the anchor resolves")
        .0
}

fn kind_for(anchor: &MarkAnchor) -> MarkKind {
    match anchor {
        MarkAnchor::Household(_) => MarkKind::ChalkCross,
        MarkAnchor::Place(_) => MarkKind::WellTally,
    }
}

/// One shelter covering node 2 (Tam Rud's door) and nothing else.
fn shelter_over_tam_ruds_door() -> ShelterMap {
    let json = r#"{
        "schema_version": 1,
        "shelters": [
            {
                "id": "sh_arcade",
                "label": "the arcade",
                "polygon_xz": [[18.0, -2.0], [22.0, -2.0], [22.0, 2.0], [18.0, 2.0]],
                "route_node": 2,
                "cover": "stone"
            }
        ]
    }"#;
    ShelterMap::from_json_str(json).expect("the test shelter parses")
}

fn rain(precipitation: f64) -> WeatherSample {
    WeatherSample {
        precipitation,
        ..WeatherSample::CLEAR
    }
}

// --------------------------------------------------------------------------- //

/// The spec's headline decay claim, against a real world and a known span.
#[test]
fn chalk_halves_over_one_dry_half_life() {
    let mut world = world_with_places();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    let half_life = world
        .mark_catalog
        .spec(MarkKind::ChalkCross)
        .unwrap()
        .half_life_days_dry;

    assert_eq!(
        world.marks.get(id).unwrap().strength,
        1.0,
        "fresh chalk is full"
    );
    marks::sweep(&mut world, half_life);
    let strength = world.marks.get(id).unwrap().strength;
    assert!(
        (strength - 0.5).abs() < 1e-9,
        "one dry half-life should halve it, got {strength}"
    );
}

/// §3's shelter clause, which is the reason the sim carries a `ShelterMap` at
/// all. A hand-built `World` has an *empty* one, so this test builds it.
#[test]
fn a_sheltered_wall_outlasts_an_open_one_under_the_same_rain() {
    let mut world = world_with_places();
    world.shelters = std::sync::Arc::new(shelter_over_tam_ruds_door());
    world.current_weather = Some(rain(1.0));

    let exposed = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    let covered = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("housed")),
    );
    assert!(
        world.shelters.is_sheltered(Vec3::new(20.0, 0.91, 0.0)),
        "the test shelter must actually cover Tam Rud's door, or this proves nothing"
    );
    assert!(
        !world.shelters.is_sheltered(Vec3::new(10.0, 0.91, 0.0)),
        "and must not cover Ede Clove's"
    );

    marks::sweep(&mut world, 0.2);
    let open = world.marks.get(exposed).unwrap().strength;
    let under = world.marks.get(covered).unwrap().strength;
    assert!(
        under > open,
        "chalk under cover must outlast chalk in the rain: {under} vs {open}"
    );
    assert!(
        open < 0.8,
        "an open wall in full rain should wash noticeably, got {open}"
    );
}

/// A dangling anchor is a fact of life — homes rebind, places get renamed —
/// and must be a silence, not a panic and not a leak.
#[test]
fn a_mark_whose_anchor_stops_resolving_disappears_without_panicking() {
    let mut world = world_with_places();
    // Nobody named `ghost` is housed, and no place is named `The Nowhere`.
    let orphan_household = Mark {
        kind: MarkKind::ChalkCross,
        anchor: MarkAnchor::Household(ActorId::from_raw("ghost")),
        about: None,
        author: None,
        drawn_game_days: 0.0,
        last_decayed_game_days: 0.0,
        strength: 1.0,
        strokes: 1,
    };
    let orphan_place = Mark {
        anchor: MarkAnchor::Place("The Nowhere".to_string()),
        kind: MarkKind::WellTally,
        ..orphan_household.clone()
    };
    let real = chalk(&mut world, MarkAnchor::Place("Chain Well".to_string()));
    let (gone_a, _) = world.marks.insert(orphan_household);
    let (gone_b, _) = world.marks.insert(orphan_place);
    assert_eq!(world.marks.len(), 3);

    marks::sweep(&mut world, 0.001);

    assert!(
        world.marks.get(gone_a).is_none(),
        "a homeless household anchor drops"
    );
    assert!(
        world.marks.get(gone_b).is_none(),
        "an unregistered place anchor drops"
    );
    assert!(
        world.marks.get(real).is_some(),
        "the resolvable mark survives"
    );

    // And it never reaches a sheet or the wire either.
    let snapshot = world.public_snapshot(&ActorId::from_raw("player"));
    assert_eq!(
        snapshot.marks.len(),
        1,
        "only the resolvable mark is published"
    );
}

/// The prompt criterion: a mark 3 m from an actor renders a bullet carrying
/// the distance and the meaning.
#[test]
fn a_mark_within_reach_reaches_the_sheet_with_its_distance_and_meaning() {
    let mut world = world_with_places();
    world.add_character(character("sv3n1", "Sven", 7.0));
    chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );

    let near = marks::marks_within(&world, Vec3::new(7.0, 0.91, 0.0), MARK_NOTICE_RADIUS_M);
    assert_eq!(
        near.len(),
        1,
        "the cross is 3 m off and inside the 8 m radius"
    );
    assert!(
        (near[0].distance_m - 3.0).abs() < 1e-9,
        "3 m, got {}",
        near[0].distance_m
    );
    assert_eq!(near[0].meaning, "this household owes and has not paid");
    assert_eq!(near[0].label, "a chalk cross at knee height");
    assert_eq!(near[0].occupant, Some(ActorId::from_raw("debtor")));

    // …and nothing at all from across the graph.
    let far = marks::marks_within(&world, Vec3::new(30.0, 0.91, 0.0), MARK_NOTICE_RADIUS_M);
    assert!(far.is_empty(), "20 m away is out of the 8 m radius");
}

/// Below `faint_below` a mark still *renders* — qualified — but stops ruling.
#[test]
fn a_half_washed_mark_still_shows_but_no_longer_binds() {
    let mut world = world_with_places();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    let spec = world
        .mark_catalog
        .spec(MarkKind::ChalkCross)
        .unwrap()
        .clone();

    world.marks.get_mut(id).unwrap().strength = spec.faint_below - 0.01;
    let mark = world.marks.get(id).unwrap();
    assert!(world.mark_catalog.is_faint(mark));
    assert!(
        !world.mark_catalog.is_binding(mark),
        "a faint mark rules nothing"
    );
    assert_eq!(
        world.mark_catalog.label_for(mark),
        "a half-washed chalk cross at knee height",
        "and says so"
    );

    world.marks.get_mut(id).unwrap().strength = spec.faint_below + 0.01;
    let mark = world.marks.get(id).unwrap();
    assert!(
        world.mark_catalog.is_binding(mark),
        "just above the line it rules again"
    );
}

/// Under `gone_below` the chalk is removed, and **nobody is told**: chalk
/// washing off is not news, so it must not reach an inbox.
#[test]
fn washing_off_removes_the_mark_and_writes_to_nobodys_inbox() {
    let mut world = world_with_places();
    world.add_character(character("sv3n1", "Sven", 10.0));
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    world.marks.get_mut(id).unwrap().strength = 0.06;
    world.marks.rewind_sweep_clock(0.0);

    // Ten dry days takes 0.06 below the catalog's gone_below of 0.05.
    marks::sweep(&mut world, 10.0);

    assert!(world.marks.get(id).is_none(), "it washed off");
    let inbox = world
        .characters
        .get(&ActorId::from_raw("sv3n1"))
        .unwrap()
        .state
        .inbox
        .clone();
    assert!(
        inbox.is_empty(),
        "chalk washing off is not news; nobody's inbox hears about it, got {inbox:?}"
    );
}

/// §2.7: the sweep is gated to once a game-minute, not once a poll.
#[test]
fn the_sweep_runs_at_most_once_a_game_minute() {
    let mut world = world_with_places();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    world.marks.rewind_sweep_clock(0.0);

    // A tenth of a game-minute later: nothing happens at all.
    let tenth_of_a_minute = 1.0 / 1440.0 / 10.0;
    assert!(
        !marks::sweep(&mut world, tenth_of_a_minute),
        "a sub-minute poll must not sweep"
    );
    assert_eq!(
        world.marks.get(id).unwrap().strength,
        1.0,
        "and must not weather"
    );

    // Two game-minutes later it runs, and charges the whole elapsed span.
    let two_minutes = 2.0 / 1440.0;
    assert!(
        marks::sweep(&mut world, two_minutes),
        "a minute boundary sweeps"
    );
    assert!(world.marks.get(id).unwrap().strength < 1.0, "and weathers");
}

/// §2.7's cap, enforced through the real drawing path rather than the raw
/// collection: a player with a pen and an afternoon cannot grow world state
/// without limit.
#[test]
fn drawing_past_the_cap_evicts_rather_than_growing() {
    let mut world = world_with_places();
    let nav = nav();
    for index in 0..(MARKS_MAX + 20) {
        let owner = ActorId::from_raw(format!("p{index:04}"));
        world
            .places
            .add_home(&owner, &format!("Body {index}"), Vec3::new(10.0, 0.91, 0.0));
        marks::draw_or_refresh(
            &mut world,
            MarkKind::ChalkCross,
            MarkAnchor::Household(owner),
            None,
            index as f64 * 0.001,
        )
        .expect("each home resolves");
    }
    let _ = nav;
    assert_eq!(
        world.marks.len(),
        MARKS_MAX,
        "the cap holds through the drawing path"
    );
}

/// Idempotence: a beat that runs every day leaves one mark, not a wall of them
/// — and a re-draw restores strength without minting a second id.
#[test]
fn drawing_the_same_mark_twice_refreshes_it_in_place() {
    let mut world = world_with_places();
    let anchor = MarkAnchor::Household(ActorId::from_raw("debtor"));
    let (first, fresh) =
        marks::draw_or_refresh(&mut world, MarkKind::ChalkCross, anchor.clone(), None, 0.0)
            .unwrap();
    assert!(fresh, "the first stroke is a new mark");

    world.marks.get_mut(first).unwrap().strength = 0.2;
    let (second, fresh) =
        marks::draw_or_refresh(&mut world, MarkKind::ChalkCross, anchor, None, 1.0).unwrap();

    assert!(!fresh, "the second is a refresh, not a new mark");
    assert_eq!(first, second, "and keeps the same id");
    assert_eq!(world.marks.len(), 1, "one live cross per anchor, never two");
    assert_eq!(
        world.marks.get(first).unwrap().strength,
        1.0,
        "back to full"
    );
    assert_eq!(
        world.marks.get(first).unwrap().drawn_game_days,
        0.0,
        "a refresh restores strength without resetting age"
    );
}

/// A kind may only hang where the catalog says it may — the reason
/// `draw_mark` cannot put a well-tally on somebody's front door.
#[test]
fn a_kind_cannot_hang_on_an_anchor_the_catalog_refuses() {
    let mut world = world_with_places();
    let refused = marks::draw_or_refresh(
        &mut world,
        MarkKind::WellTally,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
        None,
        0.0,
    );
    assert!(refused.is_none(), "a tally does not belong on a door");
    assert!(world.marks.is_empty());
}

/// The ablation switch stops new chalk without erasing what is already up.
#[test]
fn the_ablation_switch_stops_new_chalk_but_does_not_erase_the_walls() {
    let mut world = world_with_places();
    let existing = chalk(&mut world, MarkAnchor::Place("Chain Well".to_string()));

    world.marks_enabled = false;
    let refused = marks::draw_or_refresh(
        &mut world,
        MarkKind::ChalkCross,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
        None,
        0.0,
    );
    assert!(refused.is_none(), "no new chalk while ablated");
    assert!(
        world.marks.get(existing).is_some(),
        "but the walls are not scrubbed by a config flag"
    );
}

/// `Marks` iterates in id order, so every sheet, snapshot and eviction is the
/// same on a replay.
#[test]
fn marks_iterate_in_a_stable_order() {
    let mut marks = Marks::default();
    let mut ids = Vec::new();
    for index in 0..8 {
        let (id, _) = marks.insert(Mark {
            kind: MarkKind::ChalkCross,
            anchor: MarkAnchor::Household(ActorId::from_raw(format!("p{index}"))),
            about: None,
            author: None,
            drawn_game_days: 0.0,
            last_decayed_game_days: 0.0,
            strength: 1.0,
            strokes: 1,
        });
        ids.push(id);
    }
    let seen: Vec<_> = marks.iter().map(|(id, _)| id).collect();
    assert_eq!(seen, ids, "insertion order is id order is iteration order");
}
