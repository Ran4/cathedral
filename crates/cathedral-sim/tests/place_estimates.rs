//! Known-place estimates follow streets, refresh with the speaker's position,
//! and never enlarge anyone's geographical knowledge.

mod prompt_support;

use cathedral_sim::{
    NavData, PlaceId, PlaceRegistry, Vec3, WALK_Y, World, apply_action, prompt::render_prompt,
};
use prompt_support::{actor, md_section, prompt_env, seed_world, sheet};
use serde_json::{Value, json};
use std::sync::Arc;

fn city() -> World {
    let mut world = seed_world();
    let (w, h) = (140usize, 140usize);
    let mut bitset = vec![0xFF; (w * h).div_ceil(8)];
    // A blocked endpoint at (100, 20), twenty metres beyond its nearest node.
    let cell = 40 * w + 120;
    bitset[cell / 8] &= !(1 << (7 - cell % 8));
    let nav = NavData::from_parts(&json!({
        "schema_version": 1,
        "grid": {"x0": -20.0, "z0": -20.0, "cell_m": 1.0, "w": w, "h": h,
            "agent_radius_m": 0.35, "bitset_file": "x.bin", "bitset_bits": w*h, "bitset_sha256": ""},
        // The workplace is 100 m away directly, but the U-shaped street is 300 m.
        // Node 4 is disconnected, even though it stands close to the workplace.
        "nodes": [[0,0],[0,100],[100,100],[100,0],[105,0]],
        "edges": [[0,1,2],[1,2,2],[2,3,2]],
        "places": [], "sites": [], "doors": [], "reference": {"forecourt": 0}
    }).to_string(), &bitset).unwrap();
    world.places = PlaceRegistry::from_json(&json!({
        "schema_version": 1,
        "places": [
            {"id":"pl_work", "name":"Workplace", "node":3, "kind":"place", "ward":"wick"},
            {"id":"pl_lost", "name":"Disconnected yard", "node":4, "kind":"place", "ward":"wick"},
            {"id":"pl_unkn", "name":"Unknown corner", "node":1, "kind":"place", "ward":"wick"}
        ], "wards": []
    }).to_string(), &nav).unwrap();
    world.nav = Some(Arc::new(nav));
    let sven = world.characters.get_mut(&actor("sv3n1")).unwrap();
    sven.state.position_m = Vec3::new(0.0, WALK_Y, -10.0);
    sven.state
        .places_known
        .extend([PlaceId::from_raw("pl_work"), PlaceId::from_raw("pl_lost")]);
    sven.state.daily_round = vec!["at Dayspring: work at Workplace".into()];
    world
}

fn place(world: &World, person: &str, id: &str) -> Value {
    sheet(world, person, &prompt_env())["places_you_know"]
        .as_array()
        .unwrap()
        .iter()
        .find(|place| place["place_id"] == id)
        .unwrap()
        .clone()
}

#[test]
fn estimates_follow_streets_and_the_compact_prompt_format() {
    let world = city();
    assert_eq!(
        place(&world, "sv3n1", "pl_work")["walk"],
        json!({"distance_m":310.0,"minutes":2.5})
    );
    let rendered = render_prompt(&world, &actor("sv3n1"), None, &prompt_env()).unwrap();
    assert!(rendered.contains("**places_you_know** (place_id — name — distance — time to walk on foot from here; approximate street estimates; go_to takes these place_ids):"));
    assert_eq!(
        md_section(&rendered, "places_you_know").unwrap(),
        [
            "pl_lost — Disconnected yard — walking estimate unavailable",
            "pl_work — Workplace — 310 m — 2.5 min",
        ]
    );
    assert!(!rendered.contains("Unknown corner"));
    assert!(place(&world, "sv3n1", "pl_lost").get("walk").is_none());
}

#[test]
fn estimates_refresh_from_the_speaker_including_short_walks_and_arrival() {
    let mut world = city();
    // Warm the workplace table before moving to another origin.
    let _ = place(&world, "sv3n1", "pl_work");
    for (x, z, expected) in [
        (0.0, 100.0, json!({"distance_m":200.0,"minutes":1.6})),
        (100.0, 8.0, json!({"distance_m":10.0,"minutes":0.1})),
        (100.0, 5.0, json!({"distance_m":0.0,"minutes":0.0})),
    ] {
        world
            .characters
            .get_mut(&actor("sv3n1"))
            .unwrap()
            .state
            .position_m = Vec3::new(x, WALK_Y, z);
        assert_eq!(place(&world, "sv3n1", "pl_work")["walk"], expected);
    }
    let rendered = render_prompt(&world, &actor("sv3n1"), None, &prompt_env()).unwrap();
    assert!(rendered.contains("pl_work — Workplace — right here"));
    // Being five metres away across disconnected graph components isn't arrival.
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .position_m = Vec3::new(105.0, WALK_Y, 0.0);
    assert!(place(&world, "sv3n1", "pl_work").get("walk").is_none());
}

#[test]
fn a_learned_way_gets_the_learners_distance_and_does_not_change_travel_handles() {
    let mut world = city();
    world
        .characters
        .get_mut(&actor("cb947"))
        .unwrap()
        .state
        .position_m = Vec3::new(0.0, WALK_Y, 5.0);
    assert_eq!(
        sheet(&world, "cb947", &prompt_env())["places_you_know"],
        json!([])
    );
    apply_action(
        &mut world,
        &actor("sv3n1"),
        "tell_way",
        &json!({"person":"cb947","place_id":"pl_work"}),
    )
    .unwrap();
    assert_eq!(
        place(&world, "cb947", "pl_work")["walk"],
        json!({"distance_m":310.0,"minutes":2.4})
    );
    assert_eq!(
        place(&world, "sv3n1", "pl_work")["walk"]["minutes"],
        json!(2.5)
    );
    apply_action(
        &mut world,
        &actor("cb947"),
        "go_to",
        &json!({"place_id":"pl_work"}),
    )
    .unwrap();
}

#[test]
fn home_offsets_are_included_but_blocked_endpoints_and_missing_graphs_supply_no_estimate() {
    let mut world = city();
    let home = world
        .places
        .add_home(&actor("sv3n1"), "Sven", Vec3::new(100.0, WALK_Y, 10.0));
    let blocked = world
        .places
        .add_home(&actor("cb947"), "Conny", Vec3::new(100.0, WALK_Y, 20.0));
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .places_known
        .extend([home.clone(), blocked.clone()]);
    assert_eq!(
        place(&world, "sv3n1", home.as_str())["walk"],
        json!({"distance_m":320.0,"minutes":2.5})
    );
    assert!(
        place(&world, "sv3n1", blocked.as_str())
            .get("walk")
            .is_none()
    );
    world.nav = None;
    let rendered = render_prompt(&world, &actor("sv3n1"), None, &prompt_env()).unwrap();
    assert!(rendered.contains("pl_work — Workplace — walking estimate unavailable"));
    assert!(place(&world, "sv3n1", home.as_str()).get("walk").is_none());
}
