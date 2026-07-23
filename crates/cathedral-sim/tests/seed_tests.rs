//! The seeded cast (`test_sim.py` SeedAndSnapshotTests 25-27, llm-headless.md
//! §3.1). Prompt goldens are rendered from this data, so every string, position
//! and yaw here is a byte-for-byte contract with `main.py:build_world()` — a
//! "tidy-up" (Ilse's missing full stop, say) is a behavior change.

use std::collections::BTreeSet;

use cathedral_sim::{
    ActorId, Control, HEARING_RADIUS_M, ItemId, SoundCatalog, Vec3, World, WorldConfig, WorldSeed,
    apply_action, build_world,
};
use serde_json::json;

/// The host reads these files; the tests compile them in.
const SEED_JSON: &str = include_str!("fixtures/demo_seed.json");
const CATALOG_TOML: &str = include_str!("../../../assets/sounds/catalog.toml");

fn seeded_world() -> World {
    let seed = WorldSeed::from_json_str(SEED_JSON).expect("the shipped seed must load");
    build_world(
        &seed,
        WorldConfig {
            sound_catalog: SoundCatalog::from_toml_str(CATALOG_TOML)
                .expect("the shipped catalog must load"),
            ..Default::default()
        },
    )
}

fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

/// Test 25: cast, inventory, control and positions.
#[test]
fn the_seed_preserves_cast_inventory_control_and_positions() {
    let world = seeded_world();
    assert_eq!(world.characters.len(), 4);

    let player = &world.characters[&actor("player")];
    assert_eq!(player.control(), Control::Player);
    assert_eq!(player.position_m(), Vec3::new(0.0, 0.91, 95.0));
    assert_eq!(player.name(), "Player");
    assert_eq!(player.voice_key(), None);
    assert_eq!(
        *player.knows(),
        BTreeSet::from([actor("sv3n1"), actor("cb947"), actor("k0fb1")])
    );

    let sven = &world.characters[&actor("sv3n1")];
    assert_eq!(sven.name(), "Sven");
    assert_eq!(sven.control(), Control::Llm);
    assert_eq!(sven.position_m(), Vec3::new(-1.8, 0.91, 114.0));
    assert_eq!(sven.holds(), [ItemId::from_raw("fzbn9")]);
    assert_eq!(sven.voice_key(), Some("sven"));
    // The demo trio keeps its established looks through the structured seam.
    assert_eq!(sven.appearance().bespoke.as_deref(), Some("sven"));
    assert_eq!(sven.appearance().outfit, cathedral_sim::OutfitClass::Craftsman);
    assert_eq!(*sven.knows(), BTreeSet::from([actor("cb947")]));
    assert_eq!(sven.memories(), ["I'm going to get some fish"]);

    let conny = &world.characters[&actor("cb947")];
    assert_eq!(conny.name(), "Conny");
    assert_eq!(conny.appearance().bespoke.as_deref(), Some("conny"));
    assert_eq!(conny.position_m(), Vec3::new(0.0, 0.91, 112.0));
    assert!(conny.holds().is_empty());
    assert_eq!(*conny.knows(), BTreeSet::from([actor("sv3n1")]));
    assert_eq!(
        conny.memories(),
        ["Sven still owes me two coppers for that fish"]
    );

    let ilse = &world.characters[&actor("k0fb1")];
    assert_eq!(ilse.name(), "Ilse");
    assert_eq!(ilse.appearance().bespoke.as_deref(), Some("ilse"));
    assert_eq!(ilse.position_m(), Vec3::new(1.8, 0.91, 114.0));
    assert_eq!(ilse.holds(), [ItemId::from_raw("c0prs")]);
    // The pilgrim knows nobody — that is the whole point of her.
    assert!(ilse.knows().is_empty());
    assert_eq!(
        ilse.memories(),
        ["I am very hungry after the long road here"]
    );

    // Every NPC is within earshot of the player's spawn, or the opening scene
    // never starts.
    let player_position = player.position_m();
    for npc in ["sv3n1", "cb947", "k0fb1"] {
        let distance = world.characters[&actor(npc)]
            .position_m()
            .distance(player_position);
        assert!(
            distance <= HEARING_RADIUS_M,
            "{npc} spawns {distance} m from the player, out of earshot"
        );
    }
}

/// The strings prompts are rendered from. Ilse's back story genuinely has no
/// trailing period in `main.py`; keep it.
#[test]
fn the_back_stories_are_verbatim() {
    let world = seeded_world();
    let forecourt = "On the grand forecourt just outside the cathedral's west entrance";

    assert_eq!(
        world.characters[&actor("sv3n1")].back_story(),
        "Born poor, you are now a blacksmith apprentice. You live in a large \
         citystate surrounding a large cathedral, and you work in one of the \
         back streets."
    );
    assert_eq!(
        world.characters[&actor("cb947")].back_story(),
        "A fisherman who sells his catch on the town square. You know most \
         faces in the quarter, including Sven, the blacksmith's apprentice."
    );
    let ilse = world.characters[&actor("k0fb1")].back_story();
    assert_eq!(
        ilse,
        "A pilgrim who arrived in the citystate this morning to see the great \
         cathedral. You know nobody here"
    );
    assert!(
        !ilse.ends_with('.'),
        "Ilse's missing full stop is load-bearing"
    );
    assert_eq!(
        world.characters[&actor("player")].back_story(),
        "A human visitor exploring the cathedral city."
    );

    for id in ["sv3n1", "cb947", "k0fb1", "player"] {
        assert_eq!(
            world.characters[&actor(id)].location_description(),
            forecourt
        );
    }
}

/// Yaw 0 faces -Z (Bevy). NPCs never turn, so a seeded facing is permanent —
/// and it gates who witnesses a sound.
#[test]
fn the_seeded_facings_survive_the_json_round_trip_bit_for_bit() {
    let world = seeded_world();

    // Sven looks at Conny, who stands to his south-east.
    assert_eq!(
        world.characters[&actor("sv3n1")].facing_yaw(),
        (-1.8f64).atan2(2.0)
    );
    // Conny looks down the west approach, at arriving visitors.
    assert_eq!(world.characters[&actor("cb947")].facing_yaw(), 0.0);
    // Ilse gazes up at the cathedral she walked here to see.
    assert_eq!(
        world.characters[&actor("k0fb1")].facing_yaw(),
        std::f64::consts::PI
    );
    assert_eq!(world.characters[&actor("player")].facing_yaw(), 0.0);
}

#[test]
fn the_two_items_are_seeded_in_order_with_their_kinds() {
    let seed = WorldSeed::from_json_str(SEED_JSON).unwrap();
    let ids: Vec<&str> = seed.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["fzbn9", "c0prs"]);

    let world = seeded_world();
    let fish = &world.items[&ItemId::from_raw("fzbn9")];
    assert_eq!(fish.kind.as_str(), "herring");
    assert_eq!(fish.quantity, 1);
    assert_eq!(world.item_catalog.display_name(fish), "herring");
    assert_eq!(world.item_catalog.visual_key(fish), "fish");
    let coin = &world.items[&ItemId::from_raw("c0prs")];
    assert_eq!(coin.kind.as_str(), "spark");
    assert_eq!(world.item_catalog.display_name(coin), "spark");
    assert_eq!(world.item_catalog.visual_key(coin), "copper_coin");
}

/// Test 26: the player is not a special case — the same validator, the same
/// percepts.
#[test]
fn the_player_uses_the_same_action_validator() {
    let mut world = seeded_world();
    let player = actor("player");

    // Sven is 19.08 m away: inside the 20 m hearing radius.
    let line = apply_action(
        &mut world,
        &player,
        "say",
        &json!({"target": "sv3n1", "text": "Good morning"}),
    )
    .unwrap();
    assert!(
        line.contains("Good morning"),
        "unexpected transcript: {line}"
    );

    let sven = &world.characters[&actor("sv3n1")];
    assert_eq!(sven.inbox().len(), 1);
    assert!(
        sven.inbox()[0].contains("said to you"),
        "unexpected percept: {}",
        sven.inbox()[0]
    );

    // The player accumulates no prose of their own — they get structured events.
    assert!(world.characters[&player].inbox().is_empty());
    assert!(world.characters[&player].recent_history().is_empty());

    // And the player is refused exactly like an NPC would be.
    let error = apply_action(
        &mut world,
        &player,
        "say",
        &json!({"target": "player", "text": "hi"}),
    )
    .unwrap_err();
    assert_eq!(error.code.as_str(), "self_target");
}

/// Test 27: the snapshot is public-only and its revision is monotonic.
#[test]
fn the_snapshot_is_public_and_monotonic() {
    let mut world = seeded_world();
    let player = actor("player");

    let snapshot = world.public_snapshot(&player);
    assert_eq!(snapshot.actors.len(), 4);
    let encoded = serde_json::to_string(&snapshot).unwrap();
    for private in [
        "back_story",
        "memories",
        "goal",
        "voice_key",
        "inbox",
        "recent_history",
        "knows",
    ] {
        assert!(
            !encoded.contains(private),
            "the snapshot leaks {private}: {encoded}"
        );
    }

    // A player move rides the hot channel: applied to live state, but it does
    // not bump the public revision — the host owns the player's own transform
    // (`World::update_positions`).
    let before = snapshot.world_revision;
    world
        .update_positions(
            0,
            &[cathedral_sim::SpatialActorUpdate::new(
                player.clone(),
                Vec3::new(0.0, 0.91, 100.0),
                None,
            )],
        )
        .unwrap();
    assert_eq!(
        world.public_snapshot(&player).world_revision,
        before,
        "a player move stays off the cold channel"
    );

    // A non-player move is a public change, and the revision is monotonic in it.
    world
        .update_positions(
            1,
            &[cathedral_sim::SpatialActorUpdate::new(
                actor("sv3n1"),
                Vec3::new(1.0, 0.91, 50.0),
                None,
            )],
        )
        .unwrap();
    assert!(
        world.public_snapshot(&player).world_revision > before,
        "a non-player move bumps the revision"
    );
}

/// The player knows all three NPCs by name; the NPCs are strangers to each
/// other in exactly one direction (Ilse knows nobody, nobody knows Ilse).
#[test]
fn the_snapshot_renders_names_from_the_players_knowledge() {
    let world = seeded_world();
    let snapshot = world.public_snapshot(&actor("player"));
    let mut names: Vec<(&str, &str)> = snapshot
        .actors
        .iter()
        .map(|a| (a.id.as_str(), a.name_for_player.as_str()))
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            ("cb947", "Conny"),
            ("k0fb1", "Ilse"),
            ("player", "You"),
            ("sv3n1", "Sven"),
        ]
    );
}
