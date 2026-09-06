//! Manual, offline reproduction/measurement setup. Writes prompts for the
//! one-shot runner; it never calls a provider itself or opens a game window.
//!
//! cargo test -p cathedral-backends --test place_estimates_evidence -- --ignored --nocapture
//! Output defaults to /tmp/cathedral-wayfinding (override CATHEDRAL_WAYFINDING_EVIDENCE_DIR).

use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, NavData, Office, PlaceId, PromptEnv, Round, SoundCatalog, Vec3, WALK_Y,
    WorldClock, WorldConfig, build_world, extra_ambient_sheets, prompt::render_prompt,
    spread_over_walkable,
};
use serde_json::json;
use std::{fs, hint::black_box, path::PathBuf, sync::Arc, time::Instant};

fn section<'a>(prompt: &'a str, label: &str) -> &'a str {
    let start = prompt.find(&format!("**{label}**")).unwrap();
    let end = prompt[start..]
        .find("\n\n**")
        .map_or(prompt.len(), |end| start + end);
    &prompt[start..end]
}

#[test]
#[ignore = "manual evidence: writes replay prompts and measures rendering/cache costs"]
fn prepare_colm_replay_and_measure_cache() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::env::var_os("CATHEDRAL_WAYFINDING_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/cathedral-wayfinding"));
    fs::create_dir_all(&out).unwrap();
    let read = |path: &str| fs::read_to_string(root.join(path)).unwrap();
    let env = PromptEnv::new(
        &read("assets/prompts/turn.j2"),
        &read("assets/prompts/night.j2"),
        &read("assets/prompts/strings.toml"),
    )
    .unwrap();
    let nav = Arc::new(
        NavData::from_parts(
            &read("assets/world/navigation.json"),
            &fs::read(root.join("assets/world/navigation.bin")).unwrap(),
        )
        .unwrap(),
    );
    // Session 799 used 1,000 generated citizens. Seeding the whole crowd also
    // reproduces the house assignment (which depends on door occupancy).
    let seed = load_world_seed(&root.join("assets"), &root.join("lore"))
        .unwrap()
        .with_extra_ambient(extra_ambient_sheets(
            &nav,
            &spread_over_walkable(&nav, 1000),
            0,
        ))
        .unwrap();
    let mut world = build_world(
        &seed,
        WorldConfig {
            area_map: AreaMap::from_json_str(&read("assets/world/areas.json")).unwrap(),
            sound_catalog: SoundCatalog::from_toml_str(&read("assets/sounds/catalog.toml"))
                .unwrap(),
            ..WorldConfig::default()
        },
    );
    world.nav = Some(nav.clone());
    let clock = WorldClock::new(3600.0, Office::Dayspring, 2, 0.05);
    Round::new().seed(&mut world, &nav, 0.0, &clock);
    world.current_time = Some(clock.at(0.0));
    let archived_prompt = read(
        "features/implemented/npcs_know_how_far_their_workplace_is_evidence/colm_original_prompt.txt",
    );
    let old_prompt = archived_prompt.as_str();
    let old_places = section(old_prompt, "places_you_know");
    let id = ActorId::from_raw("x00334");
    let colm = world.characters.get_mut(&id).unwrap();
    assert_eq!(colm.name(), "Colm Threefinger");
    colm.state.position_m = Vec3::new(323.0, WALK_Y, 117.0); // archived, rounded coordinates
    colm.state.places_known = old_places
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|line| PlaceId::from_raw(line.split_once(' ').unwrap().0))
        .collect();
    for place in &colm.state.places_known {
        assert!(
            world.places.get(place).is_some(),
            "archived handle {place} must resolve"
        );
    }

    let cold = Instant::now();
    let rendered = render_prompt(&world, &id, None, &env).unwrap();
    let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let new_places = section(&rendered, "places_you_know");
    let old_sheet = old_prompt
        .split_once("Your sheet:\n\n")
        .unwrap()
        .1
        .split_once("\n\nTake one or more actions.")
        .unwrap()
        .0;
    let prefix = rendered.split_once("Your sheet:\n\n").unwrap().0;
    let footer = rendered
        .rsplit_once("\n\nTake one or more actions.")
        .unwrap()
        .1;
    // Preserve the actual sheet, especially rough STT, chronology, and Hamel's
    // interruption. Replace only the instructions and computed place rows.
    let replay = format!(
        "{prefix}Your sheet:\n\n{}\n\nTake one or more actions.{footer}",
        old_sheet.replacen(old_places, new_places, 1)
    );
    fs::write(out.join("colm_original_prompt.txt"), old_prompt).unwrap();
    fs::write(out.join("colm_replay_prompt.txt"), &replay).unwrap();

    let question =
        "Okay how many meters do you need to how long does it take to walk to there from here?";
    for (name, replacement) in [
        (
            "distance",
            "How many metres is it from here to your workplace?",
        ),
        (
            "walking_time",
            "How long does it take to walk from here to your workplace?",
        ),
        (
            "explicit_switch",
            "How long does it take to walk from here to Coswald's Yard?",
        ),
        (
            "unknown",
            "How far is the Glass Orchard from here, and how long would it take to walk there?",
        ),
    ] {
        assert!(replay.contains(question));
        fs::write(
            out.join(format!("colm_{name}_prompt.txt")),
            replay.replacen(question, replacement, 1),
        )
        .unwrap();
    }

    let mut samples = Vec::new();
    for _ in 0..300 {
        let at = Instant::now();
        black_box(render_prompt(&world, &id, None, &env).unwrap());
        samples.push(at.elapsed().as_secs_f64() * 1_000_000.0);
    }
    samples.sort_by(f64::total_cmp);
    // Isolate steady-state cache lookup cost from template/history rendering.
    let goal = nav.place("The Harne Gate").unwrap().node;
    let at = Instant::now();
    for start in 0..1_000_000 {
        black_box(nav.cached_distance_m(black_box(start % nav.node_count()), black_box(goal)));
    }
    let lookup_ns = at.elapsed().as_secs_f64() * 1000.0; // ns / one million samples
    let metrics = json!({
        "characters": world.characters.len(), "nav_nodes": nav.node_count(),
        "known_places": world.characters[&id].state.places_known.len(),
        "position_source": "archived x=323 z=117, rounded to whole metres",
        "cold_prompt_ms": cold_ms, "warm_prompt_p50_us": samples[150], "warm_prompt_p95_us": samples[285],
        "warm_distance_lookup_ns": lookup_ns,
        "old_prompt_bytes": old_prompt.len(), "new_prompt_bytes": replay.len(),
        "old_places_bytes": old_places.len(), "new_places_bytes": new_places.len(),
    });
    fs::write(
        out.join("metrics.json"),
        serde_json::to_string_pretty(&metrics).unwrap(),
    )
    .unwrap();
    println!("{metrics}\n{new_places}\nEvidence: {}", out.display());
}
