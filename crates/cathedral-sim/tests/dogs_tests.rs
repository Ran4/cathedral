//! The street dogs (`features/implemented/dogs.md`): the pack wanders deterministically,
//! rides its own hot channel, and appears on every nearby sheet — while the
//! frozen fixtures, the cold snapshot and the revision counter never move.

mod prompt_support;

use cathedral_sim::{
    ActorId, Capabilities, Engine, EngineConfig, EngineMessage, FakeCognition, NavData, NullSight,
    NullTranscription, NullTts, TtsBackendKind, Vec3, WALK_Y, dogs, prompt::render_prompt,
};
use prompt_support::{actor, md_section, prompt_env, seed_world, sheet};
use serde_json::json;

/// A compact all-walkable strip with four line nodes — the prompt tests' nav.
fn line_nav() -> NavData {
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
    NavData::from_parts(&nav_json, &bitset).expect("the line nav validates")
}

/// The committed street graph, for the engine-level test.
const NAV_JSON: &str = include_str!("../../../assets/world/navigation.json");
const NAV_BIN: &[u8] = include_bytes!("../../../assets/world/navigation.bin");

// ---------------------------------------------------------------- the wander

/// The pack drifts on the walkable surface, deterministically: same seed, same
/// steps, same positions — and nobody ever stands on stone.
#[test]
fn the_pack_wanders_walkable_ground_deterministically() {
    let nav = line_nav();
    let mut world = seed_world();
    world.dogs = dogs::seed_pack(&nav);
    let mut second = dogs::seed_pack(&nav);
    assert_eq!(world.dogs.len(), 10, "the authored pack");

    // 150 game-seconds in movement slices, stepped exactly as `tick_movement`
    // steps them — on the world's own pack, where the revision would show a
    // leak into the cold snapshot chain.
    let revision_before = world.world_revision;
    for _ in 0..3000 {
        dogs::step_dogs(&mut world.dogs, 0.05, &nav);
        dogs::step_dogs(&mut second, 0.05, &nav);
    }
    let first = world.dogs.clone();
    assert_eq!(first, second, "hashed rolls, never an RNG");
    assert_eq!(
        world.world_revision, revision_before,
        "a wandering pack never touches the cold snapshot chain"
    );

    let moved = first
        .iter()
        .filter(|dog| dog.position_m.distance(dog.base) > 0.5)
        .count();
    assert!(moved > 0, "150 s of city time and nobody stirred?");
    for dog in &first {
        assert!(
            nav.is_walkable(dog.position_m.x, dog.position_m.z),
            "{} stands on stone at {:?}",
            dog.id,
            dog.position_m
        );
        assert_eq!(dog.position_m.y, WALK_Y, "dogs stay on the walk plane");
    }
}

// ---------------------------------------------------------------- the sheet

/// A dog within the 20 m hearing radius is on the sheet — nearest first, prose
/// description only (no id, no name), the `moving` flag riding the speed — and
/// a dog past the radius is not.
#[test]
fn nearby_dogs_are_on_the_sheet_nearest_first() {
    let env = prompt_env();
    let mut world = seed_world();
    let nav = line_nav();
    let mut pack = dogs::seed_pack(&nav);
    // Sven stands at (-1.8, 0.91, 114). Bracken 3 m off and ambling; Marrow
    // 8 m off and still; Sedge far out of the radius.
    pack[0].position_m = Vec3::new(1.2, WALK_Y, 114.0);
    pack[0].speed = dogs::DOG_TROT_MPS;
    pack[1].position_m = Vec3::new(-9.8, WALK_Y, 114.0);
    pack[2].position_m = Vec3::new(-1.8, WALK_Y, 165.0);
    world.dogs = pack.into_iter().take(3).collect();

    let rendered = sheet(&world, "sv3n1", &env);
    assert_eq!(
        rendered["dogs_nearby"],
        json!([
            {"description": "a rangy brindle dog", "distance_m": 3.0, "moving": true},
            {"description": "a heavy black dog with a grey muzzle", "distance_m": 8.0, "moving": false},
        ]),
        "nearest first, no id, no kennel name, the far dog absent"
    );

    // The same dogs on the next sheet over — no `knows` set gates a dog.
    let conny = sheet(&world, "cb947", &env);
    assert!(
        conny["dogs_nearby"]
            .as_array()
            .is_some_and(|dogs| !dogs.is_empty()),
        "whoever stands near a dog sees it"
    );
}

/// The markdown section renders under `you_see`, bullets in the person-bullet
/// shape, and the template explainer rides only a sheet that carries a dog —
/// a dog-less prompt keeps its exact bytes (the golden fixtures are the
/// standing witness; this pins the mechanism).
#[test]
fn the_markdown_section_and_explainer_appear_only_with_a_dog() {
    let env = prompt_env();
    let mut world = seed_world();
    let nav = line_nav();
    let mut pack = dogs::seed_pack(&nav);
    pack[0].position_m = Vec3::new(1.2, WALK_Y, 114.0);
    pack[0].speed = dogs::DOG_TROT_MPS;
    world.dogs = pack.into_iter().take(1).collect();

    let prompt = render_prompt(&world, &actor("sv3n1"), None, &env).expect("sven renders");
    assert_eq!(
        md_section(&prompt, "dogs_nearby"),
        Some(vec!["a rangy brindle dog, 3.0 m, moving".to_string()])
    );
    assert!(
        prompt.contains("dogs_nearby lists the street dogs about you"),
        "the explainer paragraph rides a dog-bearing sheet"
    );
    let see_at = prompt.find("**you_see**").unwrap();
    let dogs_at = prompt.find("**dogs_nearby**").unwrap();
    assert!(
        see_at < dogs_at,
        "dogs sit under you_see, where the eyes are"
    );

    // 25 m off: the section and the explainer both vanish.
    world.dogs[0].position_m = Vec3::new(-1.8, WALK_Y, 139.5);
    let prompt = render_prompt(&world, &actor("sv3n1"), None, &env).expect("sven renders");
    assert_eq!(md_section(&prompt, "dogs_nearby"), None);
    assert!(!prompt.contains("street dogs about you"));
}

// ---------------------------------------------------------------- the engine

/// Dogs ride `EngineMessage::Dogs` — published once at the start so the host
/// can spawn resting bodies, republished as the pack moves — and none of it
/// ever bumps `world_revision` or triggers a `Snapshot`.
#[test]
fn dogs_ride_their_own_hot_channel_without_touching_the_revision() {
    let nav =
        std::sync::Arc::new(NavData::from_parts(NAV_JSON, NAV_BIN).expect("the committed nav"));
    let mut engine = Engine::new(
        EngineConfig {
            nav: Some(nav.clone()),
            ..EngineConfig::default()
        },
        &cathedral_sim::WorldSeed::from_json_str(&prompt_support::demo_seed())
            .expect("the demo seed loads"),
        prompt_support::areas(),
        prompt_support::catalog(),
        prompt_env(),
        Box::new(FakeCognition::new()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, WALK_Y, 111.0), 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player");

    assert_eq!(engine.world().dogs.len(), 10, "the pack seeded off the nav");

    // The first poll carries `Ready` and the pack's opening publication.
    let first = engine.poll(0.0, Vec::new());
    assert!(
        first
            .iter()
            .any(|message| matches!(message, EngineMessage::Dogs { dogs } if dogs.len() == 10)),
        "the opening Dogs message lets the host spawn resting bodies"
    );

    // (The revision may move in this window — the daily round is live in any
    // nav-bearing engine; the wander test above pins that the dogs themselves
    // never touch it.)
    let mut seen_moving = false;
    let mut now = 0.0;
    for _ in 0..60 {
        now += 0.2;
        for message in engine.poll(now, Vec::new()) {
            if let EngineMessage::Dogs { dogs } = message {
                seen_moving |= dogs.iter().any(|dog| f64::from(dog.speed) > 0.5);
            }
        }
    }
    assert!(seen_moving, "12 s of city time and no dog on the move?");
    for dog in &engine.world().dogs {
        assert!(
            nav.is_walkable(dog.position_m.x, dog.position_m.z),
            "{} stands on stone at {:?}",
            dog.id,
            dog.position_m
        );
    }

    // The pack has no switch — it is always on. The one thing it still needs is
    // somewhere walkable to stand, so a nav-less engine seeds an empty kennel,
    // which is why the frozen fixtures are untouched by the feature.
    let off = Engine::new(
        EngineConfig {
            nav: None,
            ..EngineConfig::default()
        },
        &cathedral_sim::WorldSeed::from_json_str(&prompt_support::demo_seed())
            .expect("the demo seed loads"),
        prompt_support::areas(),
        prompt_support::catalog(),
        prompt_env(),
        Box::new(FakeCognition::new()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, WALK_Y, 111.0), 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player");
    assert!(
        off.world().dogs.is_empty(),
        "no nav graph, nowhere to stand, no pack"
    );
}

/// The player's own record never lists dogs anywhere a person would go — the
/// cold snapshot carries no dogs at all, so the 500-actor privacy test and
/// every frozen serialization stay untouched by the feature.
#[test]
fn the_cold_snapshot_carries_no_dogs() {
    let mut world = seed_world();
    let nav = line_nav();
    world.dogs = dogs::seed_pack(&nav);
    let snapshot = world.public_snapshot(&ActorId::from_raw("player"));
    let serialized = serde_json::to_string(&snapshot).expect("snapshots serialize");
    assert!(
        !serialized.contains("dog"),
        "dogs ride the hot channel, never the snapshot: {serialized}"
    );
}
