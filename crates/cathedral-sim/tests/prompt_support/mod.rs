//! Shared fixtures for the prompt test binaries.
//!
//! Tests may read files (the crate itself may not), so they load the real
//! `assets/prompts/*` the game ships — a prompt test that rendered from an
//! inlined template would prove nothing.

#![allow(dead_code)]

use std::{fs, path::PathBuf};

use cathedral_sim::{
    ActorId, AreaMap, PromptEnv, SoundCatalog, World, WorldConfig, WorldSeed, build_world,
    prompt::render_sheet_value,
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

/// The shipped prompt environment: `turn.j2` + `night.j2` + `strings.toml`.
pub fn prompt_env() -> PromptEnv {
    PromptEnv::new(
        &asset("prompts/turn.j2"),
        &asset("prompts/night.j2"),
        &asset("prompts/strings.toml"),
    )
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

/// The sheet's structured JSON view — [`render_sheet_value`] serializes the
/// very struct the prompt renders as markdown, so the many tests that assert
/// on sheet *data* (not layout) read it here.
pub fn sheet(world: &World, actor_id: &str, env: &PromptEnv) -> Value {
    render_sheet_value(world, &actor(actor_id), None, env).expect("the actor renders a sheet")
}

/// One markdown sheet section of a rendered prompt, as its bullet texts —
/// `None` if the section is absent (`you_offer` when empty), `Some(vec![])`
/// for the inline empty form (`**you_hold** — nothing`).
pub fn md_section(prompt: &str, label: &str) -> Option<Vec<String>> {
    let header = format!("**{label}**");
    let mut lines = prompt.lines();
    let found = lines.find(|line| line.starts_with(&header))?;
    if !found.ends_with(':') {
        return Some(Vec::new());
    }
    Some(
        lines
            .take_while(|line| line.starts_with("- "))
            .map(|line| line["- ".len()..].to_string())
            .collect(),
    )
}

/// The `what_you_know` bullets of a rendered prompt — `None` when the section
/// is absent, which is the knowing-nothing majority's case.
///
/// [`md_section`] cannot read this block: its `take_while(starts_with("- "))`
/// stops at the blank line the header is followed by, because the block puts its
/// instruction paragraph between the header and the bullets. Scanning stops at
/// the next section label, so the ignorance rule's own bullets in `turn.j2`
/// (which render *above* the sheet) can never be mistaken for held facts.
pub fn known_bullets(prompt: &str) -> Option<Vec<String>> {
    let mut lines = prompt.lines();
    lines.find(|line| line.starts_with("**what_you_know**"))?;
    let mut bullets = Vec::new();
    for line in lines {
        if line.starts_with("**") {
            break;
        }
        if let Some(bullet) = line.strip_prefix("- ") {
            bullets.push(bullet.to_string());
        }
    }
    Some(bullets)
}

/// Whitespace-normalized prompt — the Python footer assertions compare against
/// `" ".join(rendered.split())`.
pub fn compact(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}
