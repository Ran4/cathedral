//! Shared fixtures for the prompt test binaries.
//!
//! Tests may read files (the crate itself may not), so they load the real
//! `assets/prompts/*` the game ships — a prompt test that rendered from an
//! inlined template would prove nothing.

#![allow(dead_code)]

use std::{fs, path::PathBuf};

use cathedral_sim::{
    ActorId, AreaMap, PromptEnv, SoundCatalog, World, WorldConfig, WorldSeed, build_world,
    prompt::render_prompt,
};
use serde_json::Value;

pub fn repo_root() -> PathBuf {
    // crates/cathedral-sim → repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

pub fn asset(relative: &str) -> String {
    let path = repo_root().join("assets").join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prompts")
}

pub fn demo_seed() -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/demo_seed.json"),
    )
    .expect("the preserved demo seed loads")
}

/// The shipped prompt environment: `turn.j2` + `strings.toml`.
pub fn prompt_env() -> PromptEnv {
    PromptEnv::new(&asset("prompts/turn.j2"), &asset("prompts/strings.toml"))
        .expect("the shipped prompt assets must load")
}

pub fn catalog() -> SoundCatalog {
    SoundCatalog::from_toml_str(&asset("sounds/catalog.toml")).expect("the shipped catalog loads")
}

pub fn areas() -> AreaMap {
    AreaMap::from_json_str(&asset("world/areas.json")).expect("the shipped area map loads")
}

/// The preserved compact demo world, with the real sound catalog so
/// the footer lists the emittable ids.
pub fn seed_world() -> World {
    let seed = WorldSeed::from_json_str(&demo_seed()).expect("the demo seed loads");
    build_world(
        &seed,
        WorldConfig {
            area_map: areas(),
            sounds_enabled: true,
            sound_catalog: catalog(),
            ..WorldConfig::default()
        },
    )
}

pub fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

/// The ```json fence of a rendered prompt, parsed back — the Python tests'
/// `sheet()` helper.
pub fn sheet_of(prompt: &str) -> Value {
    let after = prompt
        .split_once("```json\n")
        .expect("the prompt carries a json fence")
        .1;
    let block = after
        .split_once("\n```")
        .expect("the json fence is closed")
        .0;
    serde_json::from_str(block).expect("the sheet is valid JSON")
}

/// Render and return the sheet, for the many tests that only look at the JSON.
pub fn sheet(world: &World, actor_id: &str, env: &PromptEnv) -> Value {
    let prompt =
        render_prompt(world, &actor(actor_id), None, env).expect("the actor renders a prompt");
    sheet_of(&prompt)
}

/// Whitespace-normalized prompt — the Python footer assertions compare against
/// `" ".join(rendered.split())`.
pub fn compact(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}
