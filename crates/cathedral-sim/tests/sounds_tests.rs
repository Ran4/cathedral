//! Port of `prompt_playgound/tests/test_sounds.py` — CatalogTests and
//! SoundEventTests (sim.md §10 tests 28-44, the S1-S6/S8 cases from
//! `features/sounds.md`).
//!
//! ServerSoundTests (45-49) exercise the player's HUD percept, the sound
//! cooldown, the scheduler nudge and the world-sound trigger: all engine-level,
//! ported with `engine_tests.rs`.

use std::collections::BTreeSet;

use cathedral_sim::{
    ActionErrorCode, ActorId, Character, CharacterSheet, Control, EventType, Sound, SoundCatalog,
    SpatialActorUpdate, Vec3, World, apply_action, emit_sound, sees,
};
use serde_json::{Value, json};

/// The host reads this file; the tests compile it in.
const CATALOG_TOML: &str = include_str!("../../../assets/sounds/catalog.toml");

// ------------------------------------------------------------------- fixtures

fn catalog() -> SoundCatalog {
    SoundCatalog::from_toml_str(CATALOG_TOML).expect("the shipped catalog must load")
}

fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

/// A world whose `make_sound` and world-sound triggers resolve against the
/// shipped catalog.
fn world() -> World {
    let mut world = World::new();
    world.sound_catalog = catalog();
    world
}

/// `test_sounds.py:43-63`.
fn character(
    actor_id: &str,
    name: &str,
    position: (f64, f64, f64),
    facing_yaw: f64,
    control: Control,
    knows: &[&str],
) -> Character {
    Character::from_sheet(CharacterSheet {
        id: actor(actor_id),
        name: name.to_string(),
        control,
        back_story: "test".into(),
        location_description: "test square".into(),
        appearance: Default::default(),
        voice_key: match control {
            Control::Player => None,
            Control::Llm => Some(name.to_lowercase()),
        },
        position_m: Vec3::new(position.0, position.1, position.2),
        facing_yaw,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: knows.iter().map(|id| actor(id)).collect::<BTreeSet<_>>(),
        lore: None,
        presence: cathedral_sim::Presence::InCity,
        presence_epoch: 0,
        economic_class: cathedral_sim::EconomicClass::Resident,
    })
}

/// An LLM character facing -Z (yaw 0), knowing nobody.
fn npc(actor_id: &str, name: &str, position: (f64, f64, f64)) -> Character {
    character(actor_id, name, position, 0.0, Control::Llm, &[])
}

/// `test_sounds.py:66-70` — the yaw that points `observer` straight at
/// `subject` (yaw 0 faces -Z).
fn yaw_towards(observer: Vec3, subject: Vec3) -> f64 {
    let dx = subject.x - observer.x;
    let dz = subject.z - observer.z;
    (-dx).atan2(-dz)
}

fn face(world: &mut World, observer: &str, subject: &str, offset_radians: f64) {
    let from = world.characters[&actor(observer)].position_m();
    let to = world.characters[&actor(subject)].position_m();
    world
        .characters
        .get_mut(&actor(observer))
        .expect("observer exists")
        .state
        .facing_yaw = yaw_towards(from, to) + offset_radians;
}

fn inbox(world: &World, actor_id: &str) -> Vec<String> {
    world.characters[&actor(actor_id)].inbox().to_vec()
}

fn fart(world: &mut World, actor_id: &str) -> String {
    apply_action(
        world,
        &actor(actor_id),
        "make_sound",
        &json!({"sound": "fart"}),
    )
    .expect("the fart is emittable")
}

/// The bell is a world sound: never emittable by an actor, so it is rung
/// directly (the `debug_sound` / town-bell trigger's path).
fn ring_the_bell(world: &mut World, position: Vec3) -> String {
    let bell = world
        .sound_catalog
        .get("town_bell")
        .cloned()
        .expect("the catalog has a town bell");
    emit_sound(world, None, &bell, Some(position))
}

fn present_and_absorb(world: &mut World, actor_id: &str) {
    let character = world
        .characters
        .get_mut(&actor(actor_id))
        .expect("character exists");
    let presented = character.take_pending_history();
    character.absorb_presented_history(&presented);
}

// --------------------------------------------------------------- CatalogTests

/// Test 28 (`test_sounds.py:74`): every row is wire- and filesystem-safe (the
/// id is the `assets/sounds/{id}.mp3` basename).
#[test]
fn the_catalog_rows_are_wire_and_filesystem_safe() {
    let catalog = catalog();
    assert!(!catalog.sounds().is_empty());

    for sound in catalog.sounds() {
        // The map key IS the id in Python; here the row carries it, and the
        // loader is what guarantees uniqueness.
        assert_eq!(catalog.get(&sound.sound_id), Some(sound));
        assert!(cathedral_sim::sounds::is_valid_sound_id(&sound.sound_id));
        assert!(sound.audible_distance > 0.0);
        assert!(sound.duration_seconds > 0.0);
        assert!(!sound.sfx_prompt.is_empty());
        assert!(!sound.heard.is_empty());
    }
    for ambient in catalog.ambients() {
        assert!(cathedral_sim::sounds::is_valid_sound_id(&ambient.sound_id));
    }
}

/// Test 29 (`test_sounds.py:86`): the town bell is a world sound — no actor may
/// ring it, and it is never attributable.
#[test]
fn the_town_bell_is_not_actor_emittable() {
    let catalog = catalog();
    let emittable = catalog.emittable_sound_ids();

    assert!(!emittable.contains(&"town_bell"));
    assert!(emittable.contains(&"fart"));
    assert_eq!(catalog.get("town_bell").unwrap().seen, None);
}

// ------------------------------------------------------------ SoundEventTests

/// Test 30 / S1 (`test_sounds.py:95`): one event, and the recipients are exactly
/// those inside the radius.
#[test]
fn a_fart_emits_one_event_to_exactly_the_characters_in_range() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(npc("near", "Near", (5.0, 0.0, 0.0)));
    world.add_character(npc("boundary", "Boundary", (20.0, 0.0, 0.0)));
    world.add_character(npc("outside", "Outside", (20.0001, 0.0, 0.0)));

    let line = fart(&mut world, "actor");

    assert_eq!(line, "Sven farted.");
    let events = world.drain_events();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.event_type, EventType::Sound);
    assert_eq!(event.sound_id.as_deref(), Some("fart"));
    assert_eq!(event.event_id(), format!("sound-{}", event.sequence));
    assert_eq!(event.actor_id.as_ref(), Some(&actor("actor")));
    assert_eq!(event.audible_distance, Some(20.0));
    assert_eq!(event.position_m, Some(Vec3::ZERO));
    assert_eq!(
        event.recipient_ids.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([actor("near"), actor("boundary")])
    );
    assert!(inbox(&world, "outside").is_empty());
}

/// Test 31 / S2 (`test_sounds.py:119`): facing the actor upgrades the percept to
/// an attributed one; the one turned away only hears it.
#[test]
fn the_witness_split_follows_facing() {
    let mut world = world();
    world.add_character(npc("sv1", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(character(
        "facing",
        "Conny",
        (0.0, 0.0, 5.0),
        0.0,
        Control::Llm,
        &["sv1"],
    ));
    world.add_character(character(
        "away",
        "Mott",
        (0.0, 0.0, -5.0),
        0.0,
        Control::Llm,
        &["sv1"],
    ));
    face(&mut world, "facing", "sv1", 0.0);
    face(&mut world, "away", "sv1", std::f64::consts::PI);

    fart(&mut world, "sv1");

    assert_eq!(inbox(&world, "facing"), ["Sven farted."]);
    assert_eq!(inbox(&world, "away"), ["[You heard a big fart!]"]);
    let event = world.drain_events().remove(0);
    assert_eq!(
        event.recipient_ids.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([actor("facing"), actor("away")])
    );
    assert_eq!(event.witness_ids, [actor("facing")]);
}

/// Test 32 / S3 (`test_sounds.py:138`): a witness who does not know the actor
/// still only gets a stranger — `identify` plus the leading capital.
#[test]
fn a_stranger_witness_is_attributed_by_id_only() {
    let mut world = world();
    world.add_character(npc("p0", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(npc("stranger", "Ilse", (0.0, 0.0, 5.0)));
    face(&mut world, "stranger", "p0", 0.0);

    fart(&mut world, "p0");

    assert_eq!(inbox(&world, "stranger"), ["A stranger (id p0) farted."]);
}

/// Test 33 / S4 (`test_sounds.py:151`): a world sound has no actor, no
/// witnesses, and one identical percept for everyone — however they are facing.
#[test]
fn a_world_sound_is_never_attributed() {
    let mut world = world();
    // Facing the bell must make no difference at all.
    world.add_character(npc("listener", "Conny", (0.0, 0.0, 10.0)));

    let line = ring_the_bell(&mut world, Vec3::ZERO);

    assert_eq!(line, "[The town bell is ringing.]");
    let event = world.drain_events().remove(0);
    assert_eq!(event.actor_id, None);
    assert!(event.witness_ids.is_empty());
    assert_eq!(event.recipient_ids, [actor("listener")]);
    assert_eq!(inbox(&world, "listener"), ["[The town bell is ringing.]"]);
}

/// Test 34 / S5 (`test_sounds.py:167`): each sound carries its own range — and a
/// world sound excludes nobody, so the erstwhile farter hears the bell too.
#[test]
fn the_audible_distance_is_honoured_per_sound() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(npc("distant", "Pike", (30.0, 0.0, 0.0)));

    fart(&mut world, "actor");

    let fart_event = world.drain_events().remove(0);
    assert!(
        fart_event.recipient_ids.is_empty(),
        "30 m > the fart's 20 m"
    );
    assert!(inbox(&world, "distant").is_empty());

    ring_the_bell(&mut world, Vec3::ZERO);

    let bell = world.drain_events().remove(0);
    assert_eq!(
        bell.recipient_ids.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([actor("actor"), actor("distant")])
    );
    assert_eq!(inbox(&world, "distant"), ["[The town bell is ringing.]"]);
}

/// Test 35 / S6 (`test_sounds.py:186`): the cone is horizontal only. A balcony
/// 15 m up but 3 m out still bears on the actor; directly overhead there is no
/// bearing at all, and no bearing fails dark.
#[test]
fn the_view_cone_is_horizontal_only() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(character(
        "balcony",
        "Conny",
        (0.0, 15.0, 3.0),
        0.0,
        Control::Llm,
        &["actor"],
    ));
    world.add_character(character(
        "overhead",
        "Mott",
        (0.0, 15.0, 0.0),
        0.0,
        Control::Llm,
        &["actor"],
    ));
    face(&mut world, "balcony", "actor", 0.0);

    fart(&mut world, "actor");

    let event = world.drain_events().remove(0);
    assert_eq!(event.witness_ids, [actor("balcony")]);
    assert_eq!(inbox(&world, "balcony"), ["Sven farted."]);
    assert_eq!(inbox(&world, "overhead"), ["[You heard a big fart!]"]);
}

/// Test 36 (`test_sounds.py:206`): sound percepts share the rolling window with
/// speech. The recipients' lines stay pending until a prompt presents them; the
/// emitter remembers their own act at once, with an empty inbox.
#[test]
fn sound_percepts_enter_recent_history_for_all_parties() {
    let mut world = world();
    world.add_character(npc("sv1", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(character(
        "facing",
        "Conny",
        (0.0, 0.0, 5.0),
        0.0,
        Control::Llm,
        &["sv1"],
    ));
    world.add_character(character(
        "away",
        "Mott",
        (0.0, 0.0, -5.0),
        0.0,
        Control::Llm,
        &["sv1"],
    ));
    face(&mut world, "facing", "sv1", 0.0);
    face(&mut world, "away", "sv1", std::f64::consts::PI);

    fart(&mut world, "sv1");

    assert_eq!(
        world.characters[&actor("facing")].pending_history(),
        ["Sven farted."]
    );
    assert_eq!(
        world.characters[&actor("away")].pending_history(),
        ["[You heard a big fart!]"]
    );
    assert_eq!(
        world.characters[&actor("sv1")].recent_history(),
        ["You farted."]
    );
    assert!(inbox(&world, "sv1").is_empty());

    present_and_absorb(&mut world, "facing");
    present_and_absorb(&mut world, "away");

    assert_eq!(
        world.characters[&actor("facing")].recent_history(),
        ["Sven farted."]
    );
    assert_eq!(
        world.characters[&actor("away")].recent_history(),
        ["[You heard a big fart!]"]
    );
}

/// Test 37 (`test_sounds.py:232`): a world sound graduates into history
/// unattributed.
#[test]
fn a_world_sound_enters_recent_history_unattributed() {
    let mut world = world();
    world.add_character(npc("listener", "Conny", (0.0, 0.0, 10.0)));

    ring_the_bell(&mut world, Vec3::ZERO);
    present_and_absorb(&mut world, "listener");

    assert_eq!(
        world.characters[&actor("listener")].recent_history(),
        ["[The town bell is ringing.]"]
    );
}

/// Test 38 (`test_sounds.py:242`): speech and sounds share one window, in order.
#[test]
fn speech_and_sounds_share_one_history_window() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(npc("listener", "Conny", (0.0, 0.0, 5.0)));
    face(&mut world, "listener", "actor", 0.0);

    apply_action(
        &mut world,
        &actor("actor"),
        "say",
        &json!({"text": "hello"}),
    )
    .unwrap();
    fart(&mut world, "actor");
    present_and_absorb(&mut world, "listener");

    assert_eq!(
        world.characters[&actor("listener")].recent_history(),
        [
            r#"A stranger (id actor) said: "hello""#,
            "A stranger (id actor) farted.",
        ]
    );
}

/// Test 39 (`test_sounds.py:262`): the player accumulates no prose whatsoever —
/// the HUD renders the structured event instead.
#[test]
fn the_player_never_accumulates_recent_history() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(character(
        "player",
        "Player",
        (0.0, 0.0, 5.0),
        0.0,
        Control::Player,
        &[],
    ));

    fart(&mut world, "actor");

    let player = &world.characters[&actor("player")];
    assert!(player.recent_history().is_empty());
    assert!(player.pending_history().is_empty());
    assert!(player.inbox().is_empty());
    // ...but they are still a recipient of the structured event.
    let event = world.drain_events().remove(0);
    assert_eq!(event.recipient_ids, [actor("player")]);
}

/// Test 40 / S8 (`test_sounds.py:275`): an unknown id, a non-emittable catalog
/// row and a null are all the same `unknown_sound` — a non-emittable sound must
/// not be distinguishable from one that does not exist.
#[test]
fn an_unknown_or_non_emittable_sound_is_rejected_without_an_event() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));

    for (value, needle) in [
        (json!("burp"), Some("burp")),
        (json!("town_bell"), Some("town_bell")),
        (Value::Null, None),
    ] {
        let error = apply_action(
            &mut world,
            &actor("actor"),
            "make_sound",
            &json!({"sound": value}),
        )
        .unwrap_err();

        assert_eq!(error.code, ActionErrorCode::UnknownSound, "value {value}");
        if let Some(needle) = needle {
            assert!(
                error.message.contains(needle),
                "the message should name the value: {error}"
            );
        }
    }
    assert!(world.drain_events().is_empty());
}

/// Test 41 (`test_sounds.py:290`): a sounds-disabled world refuses `make_sound`
/// — and says so, rather than pretending the sound does not exist.
#[test]
fn disabled_sounds_reject_make_sound() {
    let mut world = world();
    world.sounds_enabled = false;
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));

    let error = apply_action(
        &mut world,
        &actor("actor"),
        "make_sound",
        &json!({"sound": "fart"}),
    )
    .unwrap_err();

    assert_eq!(error.code, ActionErrorCode::SoundsDisabled);
    assert!(error.message.contains("disabled"));
    assert!(world.drain_events().is_empty());
}

/// Test 42 (`test_sounds.py:300`): the cone edge is inclusive — with a 90° cone,
/// exactly 45.0° off the bearing sees, 45.5° does not.
#[test]
fn the_view_cone_boundary_is_inclusive() {
    let mut world = world();
    world.view_cone_degrees = 90.0;
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    world.add_character(npc("observer", "Conny", (0.0, 0.0, 5.0)));

    face(&mut world, "observer", "actor", 45.0f64.to_radians());
    assert!(sees(
        &world.characters[&actor("observer")],
        &world.characters[&actor("actor")],
        world.view_cone_degrees,
    ));

    face(&mut world, "observer", "actor", 45.5f64.to_radians());
    assert!(!sees(
        &world.characters[&actor("observer")],
        &world.characters[&actor("actor")],
        world.view_cone_degrees,
    ));
}

/// Test 43 (`test_sounds.py:313`): a spatial update may carry a facing, and a
/// non-finite yaw is refused.
#[test]
fn positions_may_carry_a_facing_and_reject_a_non_finite_yaw() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    let id = actor("actor");

    world
        .update_positions(
            1,
            &[SpatialActorUpdate::new(
                id.clone(),
                Vec3::new(1.0, 0.0, 0.0),
                Some(2.5),
            )],
        )
        .unwrap();
    assert_eq!(world.characters[&id].facing_yaw(), 2.5);

    let error = world
        .update_positions(
            2,
            &[SpatialActorUpdate::new(
                id.clone(),
                Vec3::new(1.0, 0.0, 0.0),
                Some(f64::NAN),
            )],
        )
        .unwrap_err();
    assert_eq!(error.code.as_str(), "invalid_position");
    // The refused update changed nothing.
    assert_eq!(world.characters[&id].facing_yaw(), 2.5);
}

/// Test 44 (`test_sounds.py:324`): turning your head is not a public state
/// change — but the yaw still applies (the next snapshot reads it).
#[test]
fn facing_changes_never_bump_the_public_revision() {
    let mut world = world();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    let id = actor("actor");
    let revision = world.world_revision;

    let changed = world
        .update_positions(
            1,
            &[SpatialActorUpdate::new(
                id.clone(),
                world.characters[&id].position_m(),
                Some(1.0),
            )],
        )
        .unwrap();

    assert!(!changed);
    assert_eq!(world.world_revision, revision);
    assert_eq!(world.characters[&id].facing_yaw(), 1.0);
}

/// The percept texts the sim renders come from the catalog file, not from Rust
/// (D3/D18) — a `Sound` built by hand renders its own strings.
#[test]
fn the_percept_texts_come_from_the_catalog_row() {
    let mut world = World::new();
    world.sound_catalog = SoundCatalog::new(
        vec![
            Sound::new(
                "fart",
                "body",
                20.0,
                "[a made-up heard line]",
                Some("{actor} did a made-up thing.".into()),
                "prompt",
                1.5,
                true,
            )
            .unwrap(),
        ],
        vec![],
    )
    .unwrap();
    world.add_character(npc("actor", "Sven", (0.0, 0.0, 0.0)));
    // Standing at -Z and facing further -Z (yaw 0): the actor is behind them.
    world.add_character(npc("listener", "Conny", (0.0, 0.0, -5.0)));

    let line = fart(&mut world, "actor");

    assert_eq!(line, "Sven did a made-up thing.");
    assert_eq!(inbox(&world, "listener"), ["[a made-up heard line]"]);
    assert_eq!(
        world.characters[&actor("actor")].recent_history(),
        ["You did a made-up thing."]
    );
}
