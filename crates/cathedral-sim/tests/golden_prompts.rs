//! The byte-diff that proved the port (ARCHITECTURE §4.1) — now the byte-diff
//! that pins the prompt.
//!
//! `tests/fixtures/prompts/` began as Python HEAD output; each scenario's
//! manifest row carries the FULL world state its prompt was rendered from, so
//! this test rebuilds the world declaratively: a divergence in `build_world`
//! or `apply_action` can never mask a prompt divergence, and vice versa.
//!
//! **The fixtures are blessed, not incidental.** The scenarios' *worlds* still
//! come from Python HEAD; their prompt bytes have been regenerated twice, both
//! times for a *decided* prompt change: M5's sheet additions
//! (`places_you_know`, `moving` — `features/movement/05_the_llm_seam.md` §3),
//! and the markdown sheet (the JSON fence became `sheet_markdown`'s sections
//! at roughly half the token cost). They remain the witness that
//! `assets/prompts/turn.j2` and `sheet_markdown` still say what they said.
//! Change a fixture only when you have *decided* to change the prompt, and say
//! so in the commit — the ignored [`regenerate_golden_fixtures`] test below is
//! the documented way to do it
//! (`cargo test -p cathedral-sim --test golden_prompts -- --ignored`).

mod prompt_support;

use std::{collections::BTreeSet, fs};

use cathedral_sim::{
    ActorId, Character, CharacterSheet, Control, Item, ItemId, LoreProfile, Offer, Vec3,
    VendorListing, World,
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
    /// Optional, M4-additive: a lore profile so a fixture can render the
    /// `**you**` sentence and the computed hunger condition. Existing fixtures
    /// omit it (`None`), keeping their bytes stable.
    #[serde(default)]
    lore: Option<LoreProfile>,
    /// Optional, M4-additive: the hunger gauge, so the famished-holder fixture
    /// pins the computed `famished` condition. Omitted → the seed default (full).
    #[serde(default)]
    hunger: Option<f64>,
    /// Optional, M4-additive: a bound vendor's `you_sell` price list, so the
    /// vendor-with-stock fixture pins the section the round writes at bind time.
    #[serde(default)]
    you_sell: Vec<VendorListing>,
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
                // Appearance never enters a prompt; the fixture worlds carry
                // none and the default body renders byte-identical sheets.
                appearance: Default::default(),
                voice_key: dump.voice_key,
                position_m: Vec3::new(dump.position_m.x, dump.position_m.y, dump.position_m.z),
                facing_yaw: dump.facing_yaw,
                holds: dump.holds,
                goal: dump.goal,
                memories: dump.memories,
                knows: dump.knows,
                lore: dump.lore,
                presence: cathedral_sim::Presence::InCity,
                presence_epoch: 0,
                economic_class: cathedral_sim::EconomicClass::Resident,
            });
            character.state.inbox = dump.inbox;
            character.state.recent_history = dump.recent_history;
            character.state.pending_history = dump.pending_history;
            if let Some(hunger) = dump.hunger {
                character.state.needs.hunger = hunger;
            }
            character.state.you_sell = dump.you_sell;
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
        "{name}: byte {at} differs (fixture {} bytes, rust {} bytes)\n\
         fixture: {:?}\n   rust: {:?}",
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
fn every_scenario_renders_byte_identically_to_the_fixtures() {
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

/// Rewrite every fixture from the current renderer — the documented path for a
/// *decided* prompt change (the M1 collision exporter's write-a-file idiom).
/// Ignored so an ordinary `cargo test` can never silently bless a drift; run it
/// deliberately, diff the result, and say so in the commit:
///
/// ```sh
/// cargo test -p cathedral-sim --test golden_prompts -- --ignored
/// ```
#[test]
#[ignore = "regenerates the frozen fixtures in place; run only for a decided prompt change"]
fn regenerate_golden_fixtures() {
    let env = prompt_env();
    let dir = fixtures_dir();
    let manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(dir.join("manifest.json")).expect("the manifest is checked in"),
    )
    .expect("the manifest parses");

    for scenario in manifest.scenarios {
        let actor_id = ActorId::from_raw(&scenario.actor_id);
        let mut world = scenario.world.build();
        let rendered = match scenario.mode.as_str() {
            "preview" => render_prompt(&world, &actor_id, None, &env)
                .unwrap_or_else(|error| panic!("{}: {error}", scenario.name)),
            "drain" => {
                render_prompt_and_drain(&mut world, &actor_id, &env)
                    .unwrap_or_else(|error| panic!("{}: {error}", scenario.name))
                    .0
            }
            other => panic!("{}: unknown mode '{other}'", scenario.name),
        };
        fs::write(dir.join(&scenario.file), rendered)
            .unwrap_or_else(|error| panic!("write {}: {error}", scenario.file));
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
