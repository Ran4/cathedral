//! The byte-diff that proved the port (ARCHITECTURE §4.1).
//!
//! `tests/fixtures/prompts/` was generated from Python HEAD by
//! `prompt_playgound/scripts/gen_prompt_goldens.py`, which rendered each
//! scenario through `prompt.py` and wrote the exact bytes an NPC's turn was
//! asked in. Each scenario's manifest row carries the FULL world state its
//! prompt was rendered from, so this test rebuilds the world declaratively: a
//! divergence in `build_world` or `apply_action` can never mask a prompt
//! divergence, and vice versa.
//!
//! **The fixtures are frozen.** The generator and the Python it read are
//! deleted (P7), and there is nothing left to regenerate them from — the Rust
//! renderer is the truth from here on. That is exactly what makes them worth
//! keeping: they are the last independent witness that `assets/prompts/turn.j2`
//! and `PyAsciiFormatter` still say what Python said. Change a fixture only
//! when you have *decided* to change the prompt, and say so in the commit.

mod prompt_support;

use std::{collections::BTreeSet, fs};

use cathedral_sim::{
    ActorId, Character, CharacterSheet, Control, Item, ItemId, Offer, Vec3, World,
    prompt::{render_prompt, render_prompt_and_drain},
    py_round,
};
use prompt_support::{catalog, fixtures_dir, prompt_env};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    file: String,
    actor_id: String,
    mode: String,
    world: WorldDump,
    presented: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct WorldDump {
    sounds_enabled: bool,
    items: Vec<Item>,
    characters: Vec<CharacterDump>,
    offers: Vec<Offer>,
}

/// The character sheet plus the three runtime lists the prompt reads.
#[derive(Deserialize)]
struct CharacterDump {
    id: ActorId,
    name: String,
    control: Control,
    back_story: String,
    location_description: String,
    appearance_key: String,
    voice_key: Option<String>,
    position_m: Position,
    facing_yaw: f64,
    holds: Vec<ItemId>,
    goal: String,
    memories: Vec<String>,
    knows: BTreeSet<ActorId>,
    inbox: Vec<String>,
    recent_history: Vec<String>,
    pending_history: Vec<String>,
}

#[derive(Deserialize)]
struct Position {
    x: f64,
    y: f64,
    z: f64,
}

impl WorldDump {
    fn build(self) -> World {
        let mut world = World::new();
        world.sounds_enabled = self.sounds_enabled;
        world.sound_catalog = catalog();
        for item in self.items {
            world.add_item(item);
        }
        for dump in self.characters {
            let mut character = Character::from_sheet(CharacterSheet {
                id: dump.id,
                name: dump.name,
                control: dump.control,
                back_story: dump.back_story,
                location_description: dump.location_description,
                appearance_key: dump.appearance_key,
                voice_key: dump.voice_key,
                position_m: Vec3::new(dump.position_m.x, dump.position_m.y, dump.position_m.z),
                facing_yaw: dump.facing_yaw,
                holds: dump.holds,
                goal: dump.goal,
                memories: dump.memories,
                knows: dump.knows,
            });
            character.state.inbox = dump.inbox;
            character.state.recent_history = dump.recent_history;
            character.state.pending_history = dump.pending_history;
            world.add_character(character);
        }
        for offer in self.offers {
            world.offers.insert(offer.item_id.clone(), offer);
        }
        world
    }
}

/// Show the first differing byte with its context — a stray space or a lost
/// newline is otherwise invisible in a 7 KB diff.
fn assert_bytes_eq(name: &str, expected: &str, actual: &str) {
    if expected == actual {
        return;
    }
    let at = expected
        .bytes()
        .zip(actual.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| expected.len().min(actual.len()));
    let from = at.saturating_sub(60);
    panic!(
        "{name}: byte {at} differs (python {} bytes, rust {} bytes)\n\
         python: {:?}\n  rust: {:?}",
        expected.len(),
        actual.len(),
        &expected.as_bytes()[from..(at + 60).min(expected.len())]
            .escape_ascii()
            .to_string(),
        &actual.as_bytes()[from..(at + 60).min(actual.len())]
            .escape_ascii()
            .to_string(),
    );
}

#[test]
fn every_scenario_renders_byte_identically_to_python() {
    let env = prompt_env();
    let dir = fixtures_dir();
    let manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(dir.join("manifest.json")).expect("the manifest is checked in"),
    )
    .expect("the manifest parses");

    assert!(
        manifest.scenarios.len() >= 15,
        "the fixture set lost scenarios"
    );

    for scenario in manifest.scenarios {
        let expected = fs::read_to_string(dir.join(&scenario.file))
            .unwrap_or_else(|error| panic!("read {}: {error}", scenario.file));
        let actor_id = ActorId::from_raw(&scenario.actor_id);
        let mut world = scenario.world.build();

        let rendered = match scenario.mode.as_str() {
            "preview" => render_prompt(&world, &actor_id, None, &env)
                .unwrap_or_else(|error| panic!("{}: {error}", scenario.name)),
            "drain" => {
                let (rendered, presented) = render_prompt_and_drain(&mut world, &actor_id, &env)
                    .unwrap_or_else(|error| panic!("{}: {error}", scenario.name));
                assert_eq!(
                    Some(&presented),
                    scenario.presented.as_ref(),
                    "{}: presented percepts",
                    scenario.name
                );
                let character = &world.characters[&actor_id];
                assert!(character.inbox().is_empty(), "{}", scenario.name);
                assert!(character.pending_history().is_empty(), "{}", scenario.name);
                rendered
            }
            other => panic!("{}: unknown mode '{other}'", scenario.name),
        };

        assert_bytes_eq(&scenario.name, &expected, &rendered);
    }
}

#[derive(Deserialize)]
struct RoundCases {
    cases: Vec<RoundCase>,
}

#[derive(Deserialize)]
struct RoundCase {
    value: f64,
    digits: usize,
    expected: f64,
}

/// `py_round` against CPython's own `round()` outputs (prompt.md R5): the naive
/// `(x * 10ⁿ).round() / 10ⁿ` disagrees on half-way-representable values, and
/// `distance_m` would then differ by one decimal in a real prompt.
#[test]
fn py_round_reproduces_cpython_round() {
    let cases: RoundCases = serde_json::from_str(
        &fs::read_to_string(fixtures_dir().join("py_round_cases.json"))
            .expect("the rounding cases are checked in"),
    )
    .expect("the rounding cases parse");

    assert!(cases.cases.len() > 500, "the rounding corpus shrank");
    for case in cases.cases {
        let rounded = py_round(case.value, case.digits);
        assert_eq!(
            rounded.to_bits(),
            case.expected.to_bits(),
            "round({}, {}) = {} in CPython, {rounded} here",
            case.value,
            case.digits,
            case.expected,
        );
    }
}
