//! Chalk on the walls, M0 — the medium (`features/implemented/chalking_the_walls.md` §4).
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
use cathedral_sim::{ActorId, Character, CharacterSheet, Control, PromptEnv, World};

#[path = "prompt_support/mod.rs"]
mod prompt_support;

use prompt_support::{asset, repo_root};

/// The shipped prompt environment — `turn.j2` + `night.j2` + `strings.toml`.
/// A sheet test that rendered from an inlined template would prove nothing.
fn prompt_env() -> PromptEnv {
    PromptEnv::new(
        &asset("prompts/turn.j2"),
        &asset("prompts/night.j2"),
        &asset("prompts/strings.toml"),
    )
    .expect("the shipped prompt assets must load")
}

/// The real baked registry, against the real nav graph — the only thing that
/// can catch a typo in an authored place name.
fn real_place_registry() -> PlaceRegistry {
    let nav_json = std::fs::read_to_string(repo_root().join("assets/world/navigation.json"))
        .expect("the baked nav graph is readable");
    let bitset = std::fs::read(repo_root().join("assets/world/navigation.bin"))
        .expect("the baked bitset is readable");
    let nav = NavData::from_parts(&nav_json, &bitset).expect("the baked nav graph parses");
    PlaceRegistry::from_embedded(&nav).expect("the baked registry parses")
}

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
        .id
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

    // Two game-minutes later it runs, and charges the whole elapsed span —
    // but reports *no published change*, because two minutes of a nine-day
    // half-life does not move the whole-percent value the wire carries. That
    // distinction is the whole point of the return value; see
    // `a_drying_mark_does_not_churn_the_snapshot_every_game_minute`.
    let two_minutes = 2.0 / 1440.0;
    let published = marks::sweep(&mut world, two_minutes);
    assert!(
        world.marks.get(id).unwrap().strength < 1.0,
        "it did weather"
    );
    assert!(
        !published,
        "…but a change too small for the wire must not be reported as one"
    );
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
    let first_draw =
        marks::draw_or_refresh(&mut world, MarkKind::ChalkCross, anchor.clone(), None, 0.0)
            .unwrap();
    let first = first_draw.id;
    assert!(first_draw.fresh, "the first stroke is a new mark");
    assert!(
        first_draw.evicted.is_none(),
        "nothing was pushed off the walls"
    );

    world.marks.get_mut(first).unwrap().strength = 0.2;
    let second_draw =
        marks::draw_or_refresh(&mut world, MarkKind::ChalkCross, anchor, None, 1.0).unwrap();

    assert!(
        !second_draw.fresh,
        "the second is a refresh, not a new mark"
    );
    assert_eq!(first, second_draw.id, "and keeps the same id");
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

/// The snapshot-churn guard. A game-minute of a nine-day half-life multiplies
/// strength by 0.99995 — a change twelve orders of magnitude above `f64`
/// epsilon — so a raw `strength != before` test reports "changed" on *every*
/// sweep. The engine bumps `world_revision` on a true return, which would
/// re-serialize and re-send the whole 137 KB snapshot every 2.5 real seconds,
/// forever, because a cross was drying somewhere in the city.
#[test]
fn a_drying_mark_does_not_churn_the_snapshot_every_game_minute() {
    let mut world = world_with_places();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    world.marks.rewind_sweep_clock(0.0);

    // Sweep a full game-hour, one game-minute at a time, as the engine does.
    let mut published_changes = 0usize;
    for minute in 1..=60 {
        if marks::sweep(&mut world, minute as f64 / 1440.0) {
            published_changes += 1;
        }
    }

    let strength = world.marks.get(id).unwrap().strength;
    assert!(strength < 1.0, "an hour of drying really did weather it");
    assert!(
        published_changes <= 1,
        "60 game-minutes of drying reported {published_changes} publishable changes; \
         at most one whole-percent step is possible in an hour of a nine-day half-life, \
         and every extra one is a full snapshot re-sent for nothing"
    );
}

/// …and the other half of that contract: a change the wire *can* see must be
/// reported, or a mark would visibly stick at one opacity until something else
/// happened to bump the revision.
#[test]
fn a_change_the_wire_can_see_is_reported() {
    let mut world = world_with_places();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    world.marks.rewind_sweep_clock(0.0);
    let half_life = world
        .mark_catalog
        .spec(MarkKind::ChalkCross)
        .unwrap()
        .half_life_days_dry;

    assert!(
        marks::sweep(&mut world, half_life),
        "halving the chalk is 50 whole percent steps and must be published"
    );
    assert_eq!(
        world.public_snapshot(&ActorId::from_raw("player")).marks[0].strength_pct,
        50,
        "and the wire carries the quantized value the guard compared"
    );
    let _ = id;
}

/// M0's third done-criterion, which nothing previously covered: a mark 3 m from
/// an actor must reach the **rendered sheet**, not merely the data layer.
/// Deleting the `marks_here` section, garbling `mark_bullet` or dropping
/// `marks_note` used to leave the whole suite green.
#[test]
fn a_nearby_mark_renders_into_the_actual_sheet() {
    let mut world = world_with_places();
    world.add_character(character("sv3n1", "Sven", 7.0));
    // The debtor must be a *live character* Sven has not met, not merely a
    // registered home owner. If they are only an owner, `world.characters.get`
    // returns `None` and the sheet falls back to "a stranger" whether or not
    // the `knows` gate is there at all — so deleting the gate would leave this
    // test green and the one line that can leak a real name untested.
    world.add_character(character("debtor", "Ede Clove", 10.5));
    assert!(
        !world.characters[&ActorId::from_raw("sv3n1")]
            .knows()
            .contains(&ActorId::from_raw("debtor")),
        "the premise: Sven has never met Ede Clove"
    );
    chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );

    let env = prompt_env();
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("sv3n1"), None, &env)
            .expect("the sheet renders");

    assert!(
        sheet.contains("**marks_here**"),
        "the section is missing from the rendered sheet:\n{sheet}"
    );
    assert!(
        sheet.contains("chalk on the walls within 8 metres, nearest first"),
        "the marks_note parenthesis is missing:\n{sheet}"
    );
    assert!(
        sheet.contains("a chalk cross at knee height"),
        "the label is missing:\n{sheet}"
    );
    assert!(
        sheet.contains("this household owes and has not paid"),
        "the meaning is missing:\n{sheet}"
    );
    assert!(sheet.contains("3.0 m"), "the distance is missing:\n{sheet}");
    // Sven does not know the debtor, so the door is a stranger's — the
    // unknown-people rule, applied to chalk.
    assert!(
        sheet.contains("a stranger (you don't know their name)'s door"),
        "an unknown occupant must not be named:\n{sheet}"
    );
    assert!(
        !sheet.contains("Ede Clove"),
        "the registry's home label leaked a name Sven was never told:\n{sheet}"
    );
}

/// The self clause. Nobody's `knows` set contains their own id, so without it
/// the debtor standing at their own chalked door — the single most likely line
/// this section will ever render — is told it belongs to a stranger.
#[test]
fn your_own_chalked_door_is_yours_and_not_a_strangers() {
    let mut world = world_with_places();
    world.add_character(character("debtor", "Ede Clove", 10.5));
    chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );

    let env = prompt_env();
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("debtor"), None, &env)
            .expect("the sheet renders");

    assert!(
        sheet.contains("on your own door"),
        "your own door must read as yours:\n{sheet}"
    );
    // Scoped to the bullet: `turn.j2`'s own standing prose says "stranger"
    // several times, so a bare `!contains("stranger")` would fail on the
    // template rather than on the thing under test.
    assert!(
        !sheet.contains("'s door, 0.5 m"),
        "your own door must not be spelled as anybody else's:\n{sheet}"
    );
}

/// A typo in an authored ward-sign place name is otherwise silent: `named()`
/// is an exact lookup, so the sign simply never appears, with no error and no
/// log. The M0 test only checked the eight ward *keys*, which a typo in a
/// *value* sails straight past.
#[test]
fn every_authored_ward_sign_place_resolves_against_the_real_registry() {
    let catalog = cathedral_sim::marks::MarkCatalog::default();
    let places = real_place_registry();
    let mut checked = 0usize;
    for (ward, place) in catalog.ward_sign_places() {
        assert!(
            places.named(place).is_some(),
            "ward {ward}'s authored place of resort {place:?} is not a registered \
             place name — the sign would never appear, silently. Check the exact \
             spelling in assets/world/places.json."
        );
        checked += 1;
    }
    assert_eq!(checked, 8, "one place of resort per ward");
}

// --------------------------------------------------------------------------- //
// M1 — the ward's own hand
// --------------------------------------------------------------------------- //

fn raise_against(world: &mut World, accused: &str, raised_game_days: f64) {
    world.notices.raise(
        "a body".into(),
        "owes and has not paid".into(),
        None,
        None,
        Some(raised_game_days),
        ActorId::from_raw("sergeant"),
        Some(ActorId::from_raw(accused)),
        None,
        None,
    );
}

fn cross_on(world: &World, who: &str) -> Option<cathedral_sim::ids::MarkId> {
    world
        .marks
        .find(
            MarkKind::ChalkCross,
            &MarkAnchor::Household(ActorId::from_raw(who)),
        )
        .map(|(id, _)| id)
}

/// The writer's age gate: a wrong settled promptly never reaches the wall.
#[test]
fn a_fresh_notice_chalks_nobodys_door() {
    let mut world = world_with_places();
    raise_against(&mut world, "debtor", 0.0);

    // One game-hour later — well inside CROSS_AFTER_GAME_DAYS.
    cathedral_sim::notices::chalk_the_debtors(&mut world, 1.0 / 24.0);
    assert!(
        cross_on(&world, "debtor").is_none(),
        "two game days have not passed; nothing is chalked yet"
    );

    cathedral_sim::notices::chalk_the_debtors(
        &mut world,
        cathedral_sim::notices::CROSS_AFTER_GAME_DAYS + 0.5,
    );
    assert!(
        cross_on(&world, "debtor").is_some(),
        "past the age gate the ward chalks the door"
    );
}

/// Idempotent: the beat runs every game day for as long as the word stands,
/// and leaves one cross, never a wall of them.
#[test]
fn the_wards_beat_leaves_one_cross_however_often_it_runs() {
    let mut world = world_with_places();
    raise_against(&mut world, "debtor", 0.0);
    for day in 3..12 {
        cathedral_sim::notices::chalk_the_debtors(&mut world, day as f64);
    }
    assert_eq!(
        world.marks.len(),
        1,
        "one live cross per debtor, never a second"
    );
}

/// The re-chalk, which is what makes scrubbing *buy a day* rather than an
/// amnesty: a cross scrubbed at night is back on the door by the next beat.
#[test]
fn a_scrubbed_cross_comes_back_on_the_next_days_beat() {
    let mut world = world_with_places();
    raise_against(&mut world, "debtor", 0.0);
    cathedral_sim::notices::chalk_the_debtors(&mut world, 3.0);
    let scrubbed = cross_on(&world, "debtor").expect("chalked");
    world.marks.remove(scrubbed);
    assert!(cross_on(&world, "debtor").is_none(), "scrubbed clean");

    // Same game day: the beat has already run, so the wall stays bare — the
    // scrub really did buy something.
    cathedral_sim::notices::chalk_the_debtors(&mut world, 3.4);
    assert!(
        cross_on(&world, "debtor").is_none(),
        "the beat is once a day; scrubbing buys the rest of that day"
    );

    // Next day the sergeant comes round again.
    cathedral_sim::notices::chalk_the_debtors(&mut world, 4.0);
    assert!(
        cross_on(&world, "debtor").is_some(),
        "and no longer than that: the ward repairs its own database"
    );
}

/// A faint cross is restored to full strength by the beat rather than left to
/// finish washing off — the same idempotent call, no special case.
#[test]
fn the_beat_restores_a_half_washed_cross() {
    let mut world = world_with_places();
    raise_against(&mut world, "debtor", 0.0);
    cathedral_sim::notices::chalk_the_debtors(&mut world, 3.0);
    let id = cross_on(&world, "debtor").expect("chalked");
    world.marks.get_mut(id).unwrap().strength = 0.1;

    cathedral_sim::notices::chalk_the_debtors(&mut world, 4.0);
    assert_eq!(
        world.marks.get(id).unwrap().strength,
        1.0,
        "the sergeant goes over it again"
    );
}

/// §4 M1's settling clause: settling stops the re-chalk but does **not** erase
/// the cross. A settled debt whose mark is still up for two dry days is
/// correct — the database heals slowly.
#[test]
fn settling_stops_the_re_chalk_without_scrubbing_the_door() {
    let mut world = world_with_places();
    raise_against(&mut world, "debtor", 0.0);
    cathedral_sim::notices::chalk_the_debtors(&mut world, 3.0);
    let id = cross_on(&world, "debtor").expect("chalked");

    let notice_id = world.notices.live()[0].id;
    world.notices.settle(notice_id);
    assert!(
        world.notices.live().is_empty(),
        "the word is off the tongues"
    );

    world.marks.get_mut(id).unwrap().strength = 0.4;
    cathedral_sim::notices::chalk_the_debtors(&mut world, 5.0);
    assert_eq!(
        world.marks.get(id).unwrap().strength,
        0.4,
        "settling stops the beat — it does not restore the chalk"
    );
    assert!(
        cross_on(&world, "debtor").is_some(),
        "…and it does not scrub it either; the chalk weathers off on its own"
    );
}

/// An unhoused accused has no door. Not a fault — there is simply nothing to
/// chalk, and the beat must not panic on it.
#[test]
fn an_accused_with_no_door_is_skipped_quietly() {
    let mut world = world_with_places();
    raise_against(&mut world, "vagrant", 0.0);
    cathedral_sim::notices::chalk_the_debtors(&mut world, 5.0);
    assert!(
        world.marks.is_empty(),
        "no home, no door, no cross, no panic"
    );
}

/// The per-kind switch has to reach the writer, or `marks.cross: false` would
/// silence nothing.
#[test]
fn the_cross_switch_silences_the_wards_hand() {
    let mut world = world_with_places();
    world.mark_kinds.cross = false;
    raise_against(&mut world, "debtor", 0.0);
    cathedral_sim::notices::chalk_the_debtors(&mut world, 5.0);
    assert!(world.marks.is_empty(), "the cross writer is switched off");
}

/// Two live notices naming the same person are still one door and one cross.
#[test]
fn two_notices_against_one_body_chalk_one_door() {
    let mut world = world_with_places();
    raise_against(&mut world, "debtor", 0.0);
    raise_against(&mut world, "debtor", 0.1);
    cathedral_sim::notices::chalk_the_debtors(&mut world, 5.0);
    assert_eq!(world.marks.len(), 1, "one debtor, one door, one cross");
}

// --------------------------------------------------------------------------- //
// M2 — the pen and the two verbs
// --------------------------------------------------------------------------- //

use cathedral_sim::{Item, ItemId, apply_action};
use serde_json::json;

/// Put a chalk pen in somebody's hand.
fn give_a_pen(world: &mut World, who: &str) {
    let id = ItemId::from_raw(format!("pen_{who}"));
    world.add_item(Item::new(id.clone(), "chalk_pen"));
    world
        .characters
        .get_mut(&ActorId::from_raw(who))
        .expect("the actor exists")
        .state
        .holds
        .push(id);
}

/// A world where `sv3n1` stands at Ede Clove's door with chalk in hand.
fn world_at_a_door() -> World {
    let mut world = world_with_places();
    // 1 m from the door at (10, 0.91, 0) — inside CHALK_REACH_M.
    world.add_character(character("sv3n1", "Sven", 9.0));
    give_a_pen(&mut world, "sv3n1");
    world
}

/// §2.6: the pen gates the verb.
#[test]
fn drawing_without_a_pen_is_refused() {
    let mut world = world_with_places();
    world.add_character(character("sv3n1", "Sven", 9.0));
    let error = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect_err("no pen, no mark");
    assert_eq!(error.code.as_str(), "no_pen");
    assert!(world.marks.is_empty());
}

/// …and with one, the mark goes up and is authored to the hand that drew it.
#[test]
fn a_hand_with_chalk_marks_a_door_within_reach() {
    let mut world = world_at_a_door();
    let line = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect("the door is within reach");
    assert!(line.contains("chalks"), "unexpected line: {line}");
    assert_eq!(world.marks.len(), 1);
    let (_, mark) = world.marks.iter().next().expect("the mark is live");
    assert_eq!(
        mark.author,
        Some(ActorId::from_raw("sv3n1")),
        "the hand is recorded — for the trace, never for a reader"
    );
    assert_eq!(
        mark.about,
        Some(ActorId::from_raw("debtor")),
        "`about` is derived from the anchor, never passed in"
    );
}

/// §2.2: you cannot chalk a blank wall, and out of reach is out of reach.
#[test]
fn there_is_nothing_to_chalk_out_of_arms_reach() {
    let mut world = world_with_places();
    world.add_character(character("sv3n1", "Sven", 0.0));
    give_a_pen(&mut world, "sv3n1");
    let error = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        // Ede Clove's door is 10 m away.
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect_err("a door ten metres off is not within reach");
    assert_eq!(error.code.as_str(), "nothing_to_chalk");
}

/// The catalog decides where a kind may hang, not the hand.
#[test]
fn a_tally_cannot_be_drawn_on_a_door() {
    let mut world = world_at_a_door();
    let error = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "well_tally", "anchor": "debtor"}),
    )
    .expect_err("a tally does not belong on a door");
    assert_eq!(error.code.as_str(), "unknown_kind");
}

#[test]
fn an_unknown_kind_and_a_double_mark_are_both_refused() {
    let mut world = world_at_a_door();
    let error = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "tallow_smear", "anchor": "debtor"}),
    )
    .expect_err("everything is chalk");
    assert_eq!(error.code.as_str(), "unknown_kind");

    apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect("the first cross goes up");
    let error = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect_err("the second does not");
    assert_eq!(error.code.as_str(), "already_marked");
    assert_eq!(world.marks.len(), 1);
}

/// Scrubbing needs no pen — a wet sleeve is a wet sleeve — but it does need
/// the mark to be within reach.
#[test]
fn scrubbing_needs_no_pen_but_does_need_reach() {
    let mut world = world_at_a_door();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );

    // Somebody with no pen at all, standing at the same door.
    world.add_character(character("bare1", "Bare Hands", 9.5));
    let line = apply_action(
        &mut world,
        &ActorId::from_raw("bare1"),
        "scrub_mark",
        &json!({"mark_id": id.0}),
    )
    .expect("a sleeve is enough");
    assert!(line.contains("scrubs"), "unexpected line: {line}");
    assert!(world.marks.is_empty(), "the wall is clean");
}

#[test]
fn scrubbing_out_of_reach_or_nothing_is_refused() {
    let mut world = world_at_a_door();
    let id = chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    world.add_character(character("far01", "Far Off", 30.0));
    let error = apply_action(
        &mut world,
        &ActorId::from_raw("far01"),
        "scrub_mark",
        &json!({"mark_id": id.0}),
    )
    .expect_err("twenty metres is not arm's reach");
    assert_eq!(error.code.as_str(), "out_of_range");
    assert!(
        world.marks.get(id).is_some(),
        "and the chalk is still there"
    );

    let error = apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "scrub_mark",
        &json!({"mark_id": 9999}),
    )
    .expect_err("there is no mark 9999");
    assert_eq!(error.code.as_str(), "no_such_mark");
}

/// §2.3 through the verb: a forged cross is drawn by a hand and refuses just
/// as hard. This is the player's whole side of the feature.
#[test]
fn a_forged_cross_is_indistinguishable_to_every_reader() {
    let mut world = world_at_a_door();
    apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect("forged");
    assert!(
        marks::binding_mark_about(&world, MarkKind::ChalkCross, &ActorId::from_raw("debtor"))
            .is_some(),
        "the reader binds on a forged cross exactly as on the ward's own"
    );
}

/// The witness seam: chalking a neighbour's door in front of somebody is a
/// social event, so it is the one place in this feature a percept is worth a
/// paid turn (§2.8).
#[test]
fn chalking_in_front_of_somebody_is_seen_and_named() {
    let mut world = world_at_a_door();
    world.add_character(character("watch1", "Watcher", 10.5));
    apply_action(
        &mut world,
        &ActorId::from_raw("sv3n1"),
        "draw_mark",
        &json!({"kind": "chalk_cross", "anchor": "debtor"}),
    )
    .expect("drawn");

    let seen = world.characters[&ActorId::from_raw("watch1")]
        .state
        .inbox
        .clone();
    assert_eq!(seen.len(), 1, "the witness is told once, got {seen:?}");
    assert!(
        seen[0].contains("chalked"),
        "the line names the act plainly: {}",
        seen[0]
    );
}

/// The sheet offers the handle rather than making the model invent one, and
/// the verb only appears when a hand could actually use it.
#[test]
fn the_sheet_lists_what_a_hand_could_chalk() {
    let mut world = world_at_a_door();
    let env = prompt_env();
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("sv3n1"), None, &env)
            .expect("the sheet renders");
    assert!(
        sheet.contains("**you_could_chalk**"),
        "the anchor list is missing:\n{sheet}"
    );
    assert!(sheet.contains("draw_mark"), "the verb is missing:\n{sheet}");

    // Take the pen away and the verb goes with it — but nothing already drawn
    // is erased (§2.6).
    chalk(
        &mut world,
        MarkAnchor::Household(ActorId::from_raw("debtor")),
    );
    world
        .characters
        .get_mut(&ActorId::from_raw("sv3n1"))
        .unwrap()
        .state
        .holds
        .clear();
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("sv3n1"), None, &env)
            .expect("the sheet renders");
    assert!(!sheet.contains("draw_mark"), "no pen, no verb:\n{sheet}");
    assert!(
        sheet.contains("scrub_mark"),
        "but a sleeve still scrubs what is within reach:\n{sheet}"
    );
    assert_eq!(world.marks.len(), 1, "and the mark itself is untouched");
}

// --------------------------------------------------------------------------- //
// M4 — the ward-sign's vocabulary
// --------------------------------------------------------------------------- //

/// The ward-sign hangs on a place, never a door, and the catalog says which
/// place each ward may name.
#[test]
fn a_ward_sign_hangs_only_where_the_catalog_says() {
    let catalog = cathedral_sim::marks::MarkCatalog::default();
    let household = MarkAnchor::Household(ActorId::from_raw("debtor"));
    assert!(!catalog.accepts(MarkKind::WardSign, &household));
    assert!(catalog.accepts(
        MarkKind::WardSign,
        &MarkAnchor::Place("The Wickmarket".into())
    ));
    assert_eq!(catalog.ward_sign_place("wick"), Some("The Wickmarket"));
    assert_eq!(catalog.ward_sign_place("no_such_ward"), None);
}

/// The per-kind switches reach the tally and the sign as well as the cross.
#[test]
fn the_tally_and_ward_sign_switches_silence_their_own_kinds() {
    let mut world = world_with_places();
    world.mark_kinds.tally = false;
    assert!(
        marks::draw_or_refresh(
            &mut world,
            MarkKind::WellTally,
            MarkAnchor::Place("Chain Well".into()),
            None,
            0.0,
        )
        .is_none(),
        "the tally writer is switched off"
    );
    world.mark_kinds.tally = true;
    assert!(
        marks::draw_or_refresh(
            &mut world,
            MarkKind::WellTally,
            MarkAnchor::Place("Chain Well".into()),
            None,
            0.0,
        )
        .is_some(),
        "and on again"
    );
}

/// A tally saturates rather than running away, and reads honestly at every
/// count — the M4 reader turns `strokes` straight into metres of extra walk,
/// so an unbounded count would be an unbounded penalty.
#[test]
fn a_tally_saturates_and_reads_honestly() {
    let mut world = world_with_places();
    let id = chalk(&mut world, MarkAnchor::Place("Chain Well".into()));
    let catalog = world.mark_catalog.clone();

    world.marks.get_mut(id).unwrap().strokes = 1;
    assert!(
        catalog
            .label_for(world.marks.get(id).unwrap())
            .contains("one stroke")
    );

    world.marks.get_mut(id).unwrap().strokes = 4;
    assert!(
        catalog
            .label_for(world.marks.get(id).unwrap())
            .contains("four strokes")
    );

    world.marks.get_mut(id).unwrap().strokes = cathedral_sim::marks::TALLY_STROKES_MAX;
    let label = catalog.label_for(world.marks.get(id).unwrap());
    assert!(label.contains("and more"), "saturates: {label}");
}

/// **The M3 regression.** Chalk that never bumps `world_revision` is chalk the
/// renderer never hears about: the host only rebuilds its batch when the
/// revision moves. Every LLM verb bumped it by hand, so the two *verbs* were
/// fine — and the ward's beat, the well's tally, the Night Office's sign and
/// the drive action, which all reach the world through `draw_or_refresh`, drew
/// marks that were in the snapshot and invisible on screen.
///
/// Found by standing at a well in the game and seeing bare stone.
#[test]
fn every_way_of_drawing_a_mark_tells_the_host() {
    let mut world = world_with_places();

    let before = world
        .public_snapshot(&ActorId::from_raw("player"))
        .world_revision;
    let drawn = marks::draw_or_refresh(
        &mut world,
        MarkKind::WellTally,
        MarkAnchor::Place("Chain Well".into()),
        None,
        0.0,
    )
    .expect("drawn");
    let after_draw = world
        .public_snapshot(&ActorId::from_raw("player"))
        .world_revision;
    assert!(
        after_draw > before,
        "drawing must bump the revision, or the renderer never rebuilds"
    );

    // A refresh moves the published strength, so it counts too.
    world.marks.get_mut(drawn.id).unwrap().strength = 0.3;
    marks::draw_or_refresh(
        &mut world,
        MarkKind::WellTally,
        MarkAnchor::Place("Chain Well".into()),
        None,
        1.0,
    )
    .expect("refreshed");
    let after_refresh = world
        .public_snapshot(&ActorId::from_raw("player"))
        .world_revision;
    assert!(after_refresh > after_draw, "a re-chalk is a visible change");

    // …and so does wiping it off.
    marks::scrub(&mut world, drawn.id).expect("scrubbed");
    let after_scrub = world
        .public_snapshot(&ActorId::from_raw("player"))
        .world_revision;
    assert!(after_scrub > after_refresh, "a scrub is a visible change");
    assert!(
        world
            .public_snapshot(&ActorId::from_raw("player"))
            .marks
            .is_empty(),
        "and the wall really is bare"
    );
}

// --------------------------------------------------------------------------- //
// M3 — the player's own hand
// --------------------------------------------------------------------------- //

/// What the player's HUD is offered, and the reason it is computed here rather
/// than host-side: only this half knows a door from a well.
///
/// The filter has to be all three clauses. Drawable-by-hand alone would offer a
/// sign no hand draws; the anchor slot alone would offer a well-tally on a
/// front door; and leaving out "not already there" would offer a hold whose
/// only possible outcome is `already_marked`.
#[test]
fn a_hand_is_offered_only_the_signs_that_anchor_would_take() {
    let mut world = world_with_places();
    let door = MarkAnchor::Household(ActorId::from_raw("debtor"));
    let well = MarkAnchor::Place("Chain Well".into());

    assert_eq!(
        cathedral_sim::actions::drawable_kinds_at(&world, &door),
        vec![MarkKind::ChalkCross],
        "a door takes a cross and nothing else"
    );
    assert_eq!(
        cathedral_sim::actions::drawable_kinds_at(&world, &well),
        vec![MarkKind::WellTally, MarkKind::WardSign],
        "a place takes both of the place signs"
    );

    // Chalk one of them and it drops out of the offer, because drawing it again
    // could only ever be refused.
    chalk(&mut world, well.clone());
    assert_eq!(
        cathedral_sim::actions::drawable_kinds_at(&world, &well),
        vec![MarkKind::WardSign],
        "the tally is already up"
    );
}

/// The ablation switches reach the *offer*, not only the writer: a kind
/// switched off in `config.ron` is written by nobody (`draw_or_refresh`
/// refuses it), so it must never surface as a Hold-C choice — every hold it
/// invited could only come back refused.
#[test]
fn a_switched_off_kind_is_offered_to_no_hand() {
    let mut world = world_with_places();
    let door = MarkAnchor::Household(ActorId::from_raw("debtor"));
    let well = MarkAnchor::Place("Chain Well".into());

    world.mark_kinds.cross = false;
    assert!(
        cathedral_sim::actions::drawable_kinds_at(&world, &door).is_empty(),
        "a door has nothing left to offer with the cross off"
    );
    assert_eq!(
        cathedral_sim::actions::drawable_kinds_at(&world, &well),
        vec![MarkKind::WellTally, MarkKind::WardSign],
        "the place signs are untouched — the positive control"
    );

    world.mark_kinds = Default::default();
    world.mark_kinds.tally = false;
    assert_eq!(
        cathedral_sim::actions::drawable_kinds_at(&world, &well),
        vec![MarkKind::WardSign],
        "the tally drops out alone"
    );

    world.mark_kinds = Default::default();
    world.mark_kinds.ward_sign = false;
    assert_eq!(
        cathedral_sim::actions::drawable_kinds_at(&world, &well),
        vec![MarkKind::WellTally],
        "and the sign drops out alone"
    );

    world.mark_kinds = Default::default();
    world.marks_enabled = false;
    assert!(
        cathedral_sim::actions::drawable_kinds_at(&world, &door).is_empty()
            && cathedral_sim::actions::drawable_kinds_at(&world, &well).is_empty(),
        "the whole-layer switch empties every offer"
    );
}

/// …and the sheet's half of the same rule: an anchor no enabled kind may hang
/// on is not a handle, so the `**you_could_chalk**` section and the
/// `draw_mark` verb go dark with the switch rather than inviting attempts
/// that cost a paid turn each and can only be refused.
#[test]
fn the_sheet_offers_no_handle_a_switch_has_darkened() {
    let mut world = world_at_a_door();
    let env = prompt_env();

    world.mark_kinds.cross = false;
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("sv3n1"), None, &env)
            .expect("the sheet renders");
    assert!(
        !sheet.contains("**you_could_chalk**"),
        "the only kind a door takes is off, so the door is no handle:\n{sheet}"
    );
    assert!(
        !sheet.contains("draw_mark"),
        "and the verb goes with it:\n{sheet}"
    );

    // The positive control: a hand at the well still sees the place signs.
    world.add_character(character("w1ll0", "Willo", 1.0));
    give_a_pen(&mut world, "w1ll0");
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("w1ll0"), None, &env)
            .expect("the sheet renders");
    assert!(
        sheet.contains("**you_could_chalk**") && sheet.contains("Chain Well"),
        "the still-enabled place signs keep the well on offer:\n{sheet}"
    );

    // The whole layer off: no handles for anyone, anywhere.
    world.mark_kinds = Default::default();
    world.marks_enabled = false;
    let sheet =
        cathedral_sim::prompt::render_prompt(&world, &ActorId::from_raw("w1ll0"), None, &env)
            .expect("the sheet renders");
    assert!(
        !sheet.contains("**you_could_chalk**") && !sheet.contains("draw_mark"),
        "the whole-layer switch empties the sheet too:\n{sheet}"
    );
}

/// The handle the sheet lists, the handle the verb resolves and the handle the
/// player's hand sends are one string — spelled once, in one function.
#[test]
fn an_anchor_is_spelled_the_same_way_everywhere() {
    let world = world_at_a_door();
    let listed = cathedral_sim::actions::chalkable_anchors(&world, &ActorId::from_raw("sv3n1"));
    let (handle, anchor) = listed.first().expect("the door is within reach");
    assert_eq!(handle, &cathedral_sim::actions::anchor_handle(anchor));
    assert_eq!(handle, "debtor", "a household is spelled by its owner's id");
    assert_eq!(
        cathedral_sim::actions::anchor_handle(&MarkAnchor::Place("Chain Well".into())),
        "Chain Well",
        "and a place by its registry name"
    );
}
