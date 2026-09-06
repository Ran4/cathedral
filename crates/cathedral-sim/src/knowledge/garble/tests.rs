//! Garbling's own tests: the seal, the determinism, the vocabulary, the band and
//! the pool's bound.
//!
//! `Fact::source` is private, so facts are built through [`FactCatalog::from_json`]
//! exactly as an integration test must — which gets the loader's
//! mask-names-the-placeholder validation exercised for free.
//!
//! Carriers are **synthetic ids that are not in the world**: [`view_for`] reads a
//! carrier only as a hash input and as the one exclusion from the pool, so 200 of
//! them cost 200 `String`s and no bodies. That is also the honest fixture — a
//! carrier is whoever happens to be standing in the ward.

use std::sync::Arc;

use super::*;
use crate::areas::AreaMap;
use crate::character::{CharacterSheet, Control};
use crate::ids::{AreaKey, FactId, FactKey};
use crate::knowledge::{
    AreaAdjacency, DAY_OFFSET_MAX, FactCatalog, FactView, GARBLE_SUBJECT_POOL_MAX,
};
use crate::lore::{LoreProfile, PlanningWard, Significance};
use crate::math::Vec3;

/// Three areas inside [`GARBLE_AREA_RADIUS_M`] of each other, so
/// `AreaAdjacency::build` has neighbours to hand back. A hermetic world's empty
/// map is the *other* case and `an_empty_pool_leaves_the_subject_standing` is
/// where the no-op half is stated.
const AREAS: &str = r#"{"schema_version": 1,
    "coordinate_system": {"units": "meters", "north": "+x", "east": "-z", "up": "+y"},
    "areas": [
      {"id": "wickmarket", "label": "The Wickmarket",
       "boxes": [{"min_m": {"x": 0.0, "y": 0.0, "z": 0.0},
                  "max_m": {"x": 10.0, "y": 10.0, "z": 10.0}}]},
      {"id": "shambles", "label": "The Shambles",
       "boxes": [{"min_m": {"x": 40.0, "y": 0.0, "z": 0.0},
                  "max_m": {"x": 50.0, "y": 10.0, "z": 10.0}}]},
      {"id": "fordwell", "label": "Ford Well",
       "boxes": [{"min_m": {"x": 80.0, "y": 0.0, "z": 0.0},
                  "max_m": {"x": 90.0, "y": 10.0, "z": 10.0}}]}
    ]}"#;

fn profile(occupation: Option<&str>, ward: PlanningWard, generated: bool) -> LoreProfile {
    LoreProfile {
        significance: Significance::Minor,
        planning_ward: ward,
        age: 30,
        gender: "f".into(),
        occupation_id: occupation.map(str::to_string),
        occupation_display: occupation.map(str::to_string),
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Wick".into(),
        father: None,
        mother: None,
        children: Vec::new(),
        circumstances: Vec::new(),
        conditions: Vec::new(),
        home: None,
        home_point_m: None,
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
        generated,
    }
}

fn body(id: &str, lore: Option<LoreProfile>) -> Character {
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: ActorId::from_raw(id),
        name: format!("Body {id}"),
        control: Control::Llm,
        back_story: String::new(),
        location_description: String::new(),
        appearance: Default::default(),
        voice_key: None,
        position_m: Vec3::new(0.0, crate::WALK_Y, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: Default::default(),
        lore,
        presence: crate::character::Presence::InCity,
        presence_epoch: 0,
        economic_class: crate::character::EconomicClass::Resident,
    })
}

/// A world with the three-area map, its adjacency built, and a small Wick cohort:
/// `subjct` (the fact's subject), four ward-mates and one out-of-ward
/// trade-mate — so a swap has somewhere to land and the cohort rule has both
/// limbs.
fn garble_world() -> World {
    let mut world = World::new();
    world.area_map = AreaMap::from_json_str(AREAS).expect("the test area map loads");
    world.area_adjacency = Arc::new(AreaAdjacency::build(&world.area_map));
    world.add_character(body(
        "subjct",
        Some(profile(Some("chandler"), PlanningWard::Wick, false)),
    ));
    for id in ["wick01", "wick02", "wick03", "wick04"] {
        world.add_character(body(
            id,
            Some(profile(Some("market_seller"), PlanningWard::Wick, false)),
        ));
    }
    // Same trade, another ward: the `or trade` limb.
    world.add_character(body(
        "chand1",
        Some(profile(Some("chandler"), PlanningWard::Weigh, false)),
    ));
    // Neither ward nor trade: must never be drawn.
    world.add_character(body(
        "farawy",
        Some(profile(Some("bailiff"), PlanningWard::Weigh, false)),
    ));
    world
}

fn seed_one(world: &mut World, row: &str) -> FactKey {
    let json = format!("{{\"schema_version\": 1, \"facts\": [{row}]}}");
    let catalog = FactCatalog::from_json(&json).expect("the row parses");
    let diagnostics = catalog.seed(world);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    world
        .knowledge
        .key_of(&FactId::from_raw("test.garble.row"))
        .expect("the row installed")
}

/// The `subject,place,day` row every mask test but T3 runs on.
const ALL_ROW: &str = r#"{"id": "test.garble.row", "topic": "law",
    "said": "{subject} was seen at {place} {day}", "subject": ["subjct"],
    "place": "wickmarket", "day": 0, "garble": "subject,place,day",
    "seeded": ["wick01"]}"#;

fn carriers(count: usize) -> Vec<ActorId> {
    (0..count)
        .map(|index| ActorId::from_raw(format!("c{index:04}")))
        .collect()
}

/// T1. `view_for(hops = 0)` is pristine for everybody, on a fact whose mask lets
/// all three fields move: a witness is not wrong about what they saw, and it needs
/// no special case in the caller because `wrong_chance(0)` is exactly 0.0.
#[test]
fn a_witness_is_never_garbled() {
    let mut world = garble_world();
    let key = seed_one(&mut world, ALL_ROW);
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    for carrier in carriers(200) {
        assert_eq!(
            view_for(&world, &fact, &carrier, 0),
            FactView::default(),
            "{carrier} was garbled at hops 0"
        );
    }
    assert_eq!(wrong_chance(0), 0.0);
}

/// T2. Six hundred calls, one answer each, byte for byte — the roll is a hash of
/// stable inputs and never a fresh draw. The failure message names the call index,
/// because "some call in six hundred disagreed" is not a bug report.
#[test]
fn the_same_view_never_changes_its_mind() {
    let mut world = garble_world();
    let key = seed_one(&mut world, ALL_ROW);
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    let carriers = carriers(150);
    let mut first: Vec<FactView> = Vec::new();
    for hops in 1..=4u8 {
        for carrier in &carriers {
            first.push(view_for(&world, &fact, carrier, hops));
        }
    }
    assert_eq!(first.len(), 600);
    let mut index = 0;
    for hops in 1..=4u8 {
        for carrier in &carriers {
            let again = view_for(&world, &fact, carrier, hops);
            assert_eq!(
                again, first[index],
                "call {index} ({carrier} at hops {hops}) changed its mind: \
                 {:?} then {again:?}",
                first[index]
            );
            index += 1;
        }
    }
}

/// A reconstruction cannot depend on where anybody went, who is present, whose
/// name the player learned, or what other fact arrived between the two reads.
#[test]
fn a_view_survives_movement_presence_and_knowledge_changes() {
    let mut world = garble_world();
    let key = seed_one(&mut world, ALL_ROW);
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    let carrier = ActorId::from_raw("wick02");
    let before: Vec<_> = (1..=8)
        .map(|hops| view_for(&world, &fact, &carrier, hops))
        .collect();
    assert!(before.iter().any(|view| view.subject.is_some()));
    assert!(before.iter().any(|view| view.place.is_some()));
    assert!(before.iter().any(|view| view.day_offset != 0));

    for (index, body) in world.characters.values_mut().enumerate() {
        body.state.position_m = Vec3::new(500.0 * index as f64, 20.0, -400.0);
        body.state.presence = crate::character::Presence::BeyondTheWalls;
        body.state.knows.extend(world.roster.iter().cloned());
    }
    // Generated arrivals do not alter the fixed authored vocabulary, even in a
    // small fixture where the subject pool has not reached its size cap.
    world.add_character(body(
        "x00001",
        Some(profile(Some("chandler"), PlanningWard::Wick, true)),
    ));
    let catalog = FactCatalog::from_json(
        r#"{"schema_version":1,"facts":[{"id":"test.later", "topic":"law", "said":"a later event"}]}"#,
    ).unwrap();
    assert!(catalog.seed(&mut world).is_empty());

    for (hops, expected) in (1..=8).zip(before) {
        assert_eq!(
            view_for(&world, &fact, &carrier, hops),
            expected,
            "hops {hops}"
        );
    }
}

/// T3. The mask seals a field. With `place` alone the place moves for somebody and
/// the subject and the day never do — which is what makes "the rest is
/// load-bearing truth" a property of the type and not a promise.
#[test]
fn the_garble_mask_seals_a_field() {
    let mut world = garble_world();
    let key = seed_one(
        &mut world,
        r#"{"id": "test.garble.row", "topic": "law",
            "said": "{subject} was seen at {place} {day}", "subject": ["subjct"],
            "place": "wickmarket", "day": 0, "garble": "place",
            "seeded": ["wick01"]}"#,
    );
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    assert_eq!(
        (fact.garble.subject, fact.garble.place, fact.garble.day),
        (false, true, false)
    );
    let mut place_moved = 0;
    for carrier in carriers(200) {
        for hops in 1..=5u8 {
            let view = view_for(&world, &fact, &carrier, hops);
            assert!(
                view.subject.is_none(),
                "a sealed subject moved for {carrier} at hops {hops}"
            );
            assert_eq!(
                view.day_offset, 0,
                "a sealed day moved for {carrier} at hops {hops}"
            );
            if view.place.is_some() {
                place_moved += 1;
            }
        }
    }
    assert!(
        place_moved > 0,
        "the one unsealed field never moved in 1000 draws"
    );
}

/// T4. **The garble never invents a person.** Every swapped subject is a named,
/// non-generated body this world has, is never one of the fact's own subjects, and
/// is never the carrier — a person is not handed a story in which they are the one
/// it is about.
#[test]
fn a_garbled_subject_is_always_somebody_the_city_has() {
    let mut world = garble_world();
    let key = seed_one(&mut world, ALL_ROW);
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    let mut swaps = 0;
    // The cohort itself carries too, which is the only way the carrier exclusion
    // is exercised at all.
    let mut all: Vec<ActorId> = carriers(200);
    all.extend(world.roster.iter().cloned());
    for carrier in &all {
        for hops in 1..=6u8 {
            let Some(swapped) = view_for(&world, &fact, carrier, hops).subject else {
                continue;
            };
            swaps += 1;
            assert!(
                world.roster.contains(&swapped),
                "{swapped} is not in this world's roster"
            );
            let lore = world
                .characters
                .get(&swapped)
                .and_then(Character::lore)
                .unwrap_or_else(|| panic!("{swapped} has no lore profile"));
            assert!(!lore.generated, "{swapped} is a generated body");
            assert!(
                !fact.subject.contains(&swapped),
                "the swap landed on the fact's own subject"
            );
            assert_ne!(&swapped, carrier, "the swap landed on the carrier");
            assert!(
                lore.planning_ward == PlanningWard::Wick
                    || lore.occupation_id.as_deref() == Some("chandler"),
                "{swapped} shares neither the subject's ward nor their trade"
            );
        }
    }
    assert!(
        swaps > 0,
        "no subject ever moved in {} draws",
        all.len() * 6
    );
}

/// T5. ±1 per garbled hop, clamped: no offset ever leaves the band, at any depth a
/// `u8` can hold, and somebody at three removes is wrong about the day — so the
/// clamp is a guard and not the shape.
#[test]
fn the_day_never_drifts_past_the_band() {
    let mut world = garble_world();
    let key = seed_one(&mut world, ALL_ROW);
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    let carriers = carriers(200);
    let mut drifted_at_three = 0;
    for hops in 0..=32u8 {
        for carrier in &carriers {
            let view = view_for(&world, &fact, carrier, hops);
            assert!(
                view.day_offset.abs() <= DAY_OFFSET_MAX,
                "{carrier} at hops {hops} drifted to {}",
                view.day_offset
            );
            if hops == 3 && view.day_offset != 0 {
                drifted_at_three += 1;
            }
        }
    }
    assert!(
        drifted_at_three > 0,
        "nobody at three removes was wrong about the day"
    );
}

/// T6. `1 − wrong_chance(n)` is the derivation's own row: right 0.650 / 0.423 /
/// 0.275 / 0.179 at one to four removes — hearsay worth acting on at one hop, a
/// chain worth walking at four.
#[test]
fn the_wrong_chance_matches_the_derivation() {
    for (hops, right) in [(1u8, 0.650), (2, 0.423), (3, 0.275), (4, 0.179)] {
        let measured = 1.0 - wrong_chance(hops);
        assert!(
            (measured - right).abs() < 1e-3,
            "at hops {hops} a field is right {measured:.4} of the time, not {right}"
        );
    }
}

/// T7. An empty pool leaves the subject standing — the subject then simply does not
/// move — and the other fields still do. Both ways of being unnameable: a generated
/// body, and a body with no lore at all.
#[test]
fn an_empty_pool_leaves_the_subject_standing() {
    let mut world = garble_world();
    world.add_character(body(
        "x00001",
        Some(profile(Some("market_seller"), PlanningWard::Wick, true)),
    ));
    world.add_character(body("nolore", None));
    for subject in ["x00001", "nolore"] {
        assert!(
            same_ward_or_trade(&world, &ActorId::from_raw(subject)).is_empty(),
            "{subject} has a substitution pool and should have none"
        );
    }
    // A body this world does not have at all is the third empty case.
    assert!(same_ward_or_trade(&world, &ActorId::from_raw("absent")).is_empty());

    let mut place_moved = 0;
    for subject in ["x00001", "nolore"] {
        let mut world = world.clone();
        let key = seed_one(
            &mut world,
            &format!(
                r#"{{"id": "test.garble.row", "topic": "law",
                     "said": "{{subject}} was seen at {{place}} {{day}}",
                     "subject": ["{subject}"], "place": "wickmarket", "day": 0,
                     "garble": "subject,place,day", "seeded": ["wick01"]}}"#
            ),
        );
        let fact = world.knowledge.fact(key).expect("the fact").clone();
        for carrier in carriers(200) {
            for hops in 1..=4u8 {
                let view = view_for(&world, &fact, &carrier, hops);
                assert!(
                    view.subject.is_none(),
                    "an unnameable subject ({subject}) was swapped for {:?}",
                    view.subject
                );
                if view.place.is_some() {
                    place_moved += 1;
                }
            }
        }
    }
    assert!(place_moved > 0, "the place stopped moving with the subject");
}

/// T8. The pool is bounded and lore-only: forty ward-mates plus forty generated
/// bodies give exactly [`GARBLE_SUBJECT_POOL_MAX`] candidates and not one
/// procedural id, which is what keeps the walk inside the authored prefix at
/// `--extra-ambient 20000`.
#[test]
fn the_subject_pool_is_bounded_and_lore_only() {
    let mut world = World::new();
    world.add_character(body(
        "subjct",
        Some(profile(Some("chandler"), PlanningWard::Wick, false)),
    ));
    for index in 0..40 {
        // Interleaved, so a generated body is never merely "after the cap".
        world.add_character(body(
            &format!("x{index:05}"),
            Some(profile(Some("chandler"), PlanningWard::Wick, true)),
        ));
        world.add_character(body(
            &format!("w{index:05}"),
            Some(profile(Some("market_seller"), PlanningWard::Wick, false)),
        ));
    }
    let pool = same_ward_or_trade(&world, &ActorId::from_raw("subjct"));
    assert_eq!(pool.len(), GARBLE_SUBJECT_POOL_MAX, "{pool:?}");
    for member in &pool {
        assert!(
            !member.as_str().starts_with('x'),
            "a generated body reached the pool: {member}"
        );
        assert!(
            world
                .characters
                .get(member)
                .and_then(Character::lore)
                .is_some_and(|lore| !lore.generated)
        );
    }
    // Roster order, so the pool is the same list run to run.
    let again = same_ward_or_trade(&world, &ActorId::from_raw("subjct"));
    assert_eq!(pool, again);
}

/// The place a swap lands on is one of the map's own neighbours, never an
/// invented key — the same "bounded to a fixed vocabulary" claim T4 makes for
/// people, on the other axis.
#[test]
fn a_garbled_place_is_always_an_adjacent_area() {
    let mut world = garble_world();
    let key = seed_one(&mut world, ALL_ROW);
    let fact = world.knowledge.fact(key).expect("the fact").clone();
    let place = fact.place.expect("the row names a place");
    let legal: Vec<AreaKey> = world.area_adjacency.neighbours(place).to_vec();
    assert!(!legal.is_empty(), "the three-area map has neighbours");
    for carrier in carriers(200) {
        for hops in 1..=4u8 {
            if let Some(moved) = view_for(&world, &fact, &carrier, hops).place {
                assert!(legal.contains(&moved), "{moved:?} is not adjacent");
                assert_ne!(moved, place, "a neighbour is never the area itself");
            }
        }
    }
}
