//! Prompt rendering (`tests/test_prompt_scheduler.py::PromptTests` 1-8, plus the
//! drain/restore contracts the scheduler tests pin from the outside).

mod prompt_support;

use cathedral_sim::{
    ActorId, Control, ItemId, LoreProfile, Vec3, apply_action,
    prompt::{render_prompt, render_prompt_and_drain},
};
use prompt_support::{actor, compact, md_section, prompt_env, seed_world, sheet};
use serde_json::json;

/// 1. `test_metric_people_have_distance_and_perspective_name`
#[test]
fn metric_people_have_distance_and_perspective_name() {
    let env = prompt_env();
    let world = seed_world();
    let rendered = sheet(&world, "sv3n1", &env);

    let people = rendered["you_see"]["people"].as_array().unwrap();
    let ids: Vec<&str> = people.iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["cb947", "k0fb1", "player"]);

    assert_eq!(people[0]["name"], "Conny");
    assert!(
        people[1]["name"].as_str().unwrap().contains("stranger"),
        "{}",
        people[1]["name"]
    );
    // round(sqrt(1.8² + 2.0²), 1)
    assert_eq!(people[0]["distance_m"], json!(2.7));

    assert_eq!(
        rendered["you_are"]["position_m"],
        json!({"x": -1.8, "y": 0.91, "z": 114.0})
    );
    assert_eq!(rendered["you_are"]["location_description"], "The Gradine");
}

/// The clock reaches the model through the sheet, not as a percept
/// (`features/movement/01_the_clock.md` §7): a field when the world has a clock,
/// and absent — keeping the frozen fixtures byte-identical — when it does not.
#[test]
fn the_sheet_carries_the_hour_only_when_the_world_has_a_clock() {
    use cathedral_sim::{Office, WorldClock};

    let env = prompt_env();
    let mut world = seed_world();

    // No clock (the golden fixtures' case): the hour is omitted entirely.
    let before = sheet(&world, "sv3n1", &env);
    assert!(
        before["you_are"].get("the_hour").is_none(),
        "the hour must be absent without a clock"
    );

    // With a clock, the office renders as a phrase between location and position.
    world.current_time = Some(WorldClock::new(3600.0, Office::Lamplight, 0, 0.05).at(0.0));
    let after = sheet(&world, "sv3n1", &env);
    let the_hour = after["you_are"]["the_hour"].as_str().unwrap();
    assert!(the_hour.starts_with("Lamplight"), "was: {the_hour}");
    assert!(the_hour.contains("lamps are being lit"), "was: {the_hour}");
    // Nothing else about `you_are` changed.
    assert_eq!(
        after["you_are"]["location_description"],
        before["you_are"]["location_description"]
    );
    assert_eq!(after["you_are"]["position_m"], before["you_are"]["position_m"]);
}

#[test]
fn a_moved_character_gets_a_freshly_resolved_prompt_location() {
    let env = prompt_env();
    let mut world = seed_world();
    let sven = actor("sv3n1");

    // Seed prose is retained for backwards-compatible data loading, but it is
    // no longer authoritative when an area map is present.
    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .sheet
        .location_description = "an intentionally stale seed description".to_string();

    assert_eq!(
        sheet(&world, "sv3n1", &env)["you_are"]["location_description"],
        "The Gradine"
    );
    world.characters.get_mut(&sven).unwrap().state.position_m = Vec3::new(0.0, 0.91, 60.0);
    assert_eq!(
        sheet(&world, "sv3n1", &env)["you_are"]["location_description"],
        "Inside the Lanthorn (Great Church of Saint Ambrelle)"
    );
}

/// 2. `test_player_starts_visible_can_leave_range_and_never_gets_a_prompt`
#[test]
fn player_starts_visible_can_leave_range_and_never_gets_a_prompt() {
    let env = prompt_env();
    let mut world = seed_world();
    let player = actor("player");

    let visible = |world: &cathedral_sim::World| -> bool {
        sheet(world, "sv3n1", &env)["you_see"]["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|person| person["id"] == "player")
    };

    assert!(visible(&world));
    world.characters.get_mut(&player).unwrap().state.position_m = Vec3::new(0.0, 0.91, 68.0);
    assert!(!visible(&world));
    world.characters.get_mut(&player).unwrap().state.position_m = Vec3::new(-2.0, 0.91, 113.0);
    assert!(visible(&world));

    // The human is never renderable, at any distance.
    let error = render_prompt(&world, &player, None, &env).unwrap_err();
    assert_eq!(
        error.message,
        "the human-controlled player must never receive an LLM prompt"
    );
}

/// 3. `test_offered_to_you_is_actionable_only_within_four_metres`
#[test]
fn offered_to_you_is_actionable_only_within_four_metres() {
    let env = prompt_env();
    let mut world = seed_world();

    apply_action(
        &mut world,
        &actor("k0fb1"),
        "offer_item",
        &json!({"item_id": "c0prs", "target": "sv3n1"}),
    )
    .unwrap();

    assert!(sheet(&world, "sv3n1", &env).get("offered_to_you").is_some());

    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .position_m = Vec3::new(-20.0, 0.91, 114.0);

    // Out of the 4 m interaction radius: the KEY disappears, it is not empty.
    assert!(sheet(&world, "sv3n1", &env).get("offered_to_you").is_none());
    // The giver's own section is not distance-filtered at all.
    assert!(sheet(&world, "k0fb1", &env).get("you_offer").is_some());
}

/// 4. `test_render_and_drain_moves_old_events_to_prompt`
#[test]
fn render_and_drain_moves_old_events_to_prompt() {
    let env = prompt_env();
    let mut world = seed_world();
    let sven = actor("sv3n1");
    {
        let state = &mut world.characters.get_mut(&sven).unwrap().state;
        state.inbox = vec!["first".into(), "second".into()];
        state.pending_history = vec!["first".into(), "second".into()];
        state.recent_history = vec![r#"You said aloud: "earlier""#.into()];
    }

    let (prompt, presented) = render_prompt_and_drain(&mut world, &sven, &env).unwrap();

    assert_eq!(
        md_section(&prompt, "since_your_last_turn").unwrap(),
        ["first", "second"]
    );
    assert_eq!(
        md_section(&prompt, "recent_history").unwrap(),
        [r#"You said aloud: "earlier""#]
    );
    assert_eq!(presented, ["first", "second"]);

    let sven_character = &world.characters[&sven];
    assert!(sven_character.inbox().is_empty());
    assert!(sven_character.pending_history().is_empty());
    // Graduation waits for the turn to complete, so a failed turn can re-present
    // the same lines as new instead of duplicating them into recent_history.
    assert_eq!(
        sven_character.recent_history(),
        [r#"You said aloud: "earlier""#]
    );

    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .absorb_presented_history(&presented);
    assert_eq!(
        world.characters[&sven].recent_history(),
        [r#"You said aloud: "earlier""#, "first", "second"]
    );
}

/// 5. `test_failed_prompt_render_restores_drained_events`
#[test]
fn failed_prompt_render_restores_drained_events() {
    let env = prompt_env();
    let mut world = seed_world();
    let sven = actor("sv3n1");
    {
        let character = world.characters.get_mut(&sven).unwrap();
        character.state.inbox = vec!["must survive".into()];
        character.state.pending_history = vec!["must survive".into()];
        character.sheet.control = Control::Player;
    }

    let error = render_prompt_and_drain(&mut world, &sven, &env).unwrap_err();
    assert_eq!(
        error.message,
        "the human-controlled player must never receive an LLM prompt"
    );

    let character = &world.characters[&sven];
    assert_eq!(character.inbox(), ["must survive"]);
    assert_eq!(character.pending_history(), ["must survive"]);
    assert!(character.recent_history().is_empty());
}

/// The restore is a PREPEND: anything that arrived while the render failed stays
/// behind the lines it never got to present (`prompt.py:257-260`).
#[test]
fn a_failed_render_restores_by_prepending() {
    let env = prompt_env();
    let mut world = seed_world();
    let ghost = ActorId::from_raw("nobody");
    assert!(render_prompt_and_drain(&mut world, &ghost, &env).is_err());

    // With a real actor: drain, fail, and observe ordering against a line that
    // landed after the drain. The only in-Rust render failure is the control
    // check, so flip it mid-flight by seeding a player-controlled actor.
    let sven = actor("sv3n1");
    {
        let character = world.characters.get_mut(&sven).unwrap();
        character.state.inbox = vec!["old".into()];
        character.state.pending_history = vec!["old".into()];
        character.sheet.control = Control::Player;
    }
    assert!(render_prompt_and_drain(&mut world, &sven, &env).is_err());
    let character = world.characters.get_mut(&sven).unwrap();
    character.sheet.control = Control::Llm;
    character.notify_percept("new");

    assert_eq!(world.characters[&sven].inbox(), ["old", "new"]);
    assert_eq!(world.characters[&sven].pending_history(), ["old", "new"]);
}

/// 6. `test_prompt_distinguishes_hearing_from_a_conversational_turn`
#[test]
fn prompt_distinguishes_hearing_from_a_conversational_turn() {
    let env = prompt_env();
    let world = seed_world();
    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    let compacted = compact(&rendered);

    for sentence in [
        "Speech in your history is what you could hear, not necessarily speech addressed to you.",
        "whether the speaker is talking to you, to the group, or to somebody else",
        "Do not answer merely to announce that you are not the named person.",
        r#"Questions to "anyone", "everyone", or the group are open to relevant answers."#,
    ] {
        assert!(compacted.contains(sentence), "missing: {sentence}");
    }
}

#[test]
fn prompt_treats_player_speech_as_noisy_stt_without_correcting_the_player() {
    let env = prompt_env();
    let world = seed_world();
    let compacted = compact(&render_prompt(&world, &actor("sv3n1"), None, &env).unwrap());

    for sentence in [
        "Speech from the player (id player) comes from imperfect automatic speech-to-text",
        "silently infer the player's likely meaning",
        r#"if the transcript says "pill rack" but the context clearly supports "peel rack""#,
        "Do not correct or lecture the player about a likely transcription error",
        "If two materially different meanings remain plausible, ask one short, natural clarification question.",
    ] {
        assert!(compacted.contains(sentence), "missing: {sentence}");
    }
}

/// 7. `test_prompt_gives_contrastive_audience_examples_and_general_wait_rule`
#[test]
fn prompt_gives_contrastive_audience_examples_and_general_wait_rule() {
    let env = prompt_env();
    let world = seed_world();
    let compacted = compact(&render_prompt(&world, &actor("sv3n1"), None, &env).unwrap());

    for sentence in [
        r#"if Ilse hears "Sven, do you have fish?", Ilse normally uses `wait {}` alone."#,
        r#"If Sven hears "Sven, do you have fish?", Sven answers if he can."#,
        "If Ilse is alone with the speaker and hears them ask for Sven",
        r#"After "Does anyone have fish?", answer only if you have a relevant answer"#,
        "nothing useful and socially appropriate for you to do, even if you just overheard something new.",
    ] {
        assert!(compacted.contains(sentence), "missing: {sentence}");
    }
}

/// 8. `test_wait_rule_sound_exemption_tracks_sounds_enabled` — extended per
///    prompt.md §8: the make_sound example line tracks the flag too, and the
///    placeholder tokens never survive into either variant.
#[test]
fn wait_rule_sound_exemption_and_example_track_sounds_enabled() {
    let env = prompt_env();
    let mut world = seed_world();

    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    let compacted = compact(&rendered);
    assert!(compacted.contains("is a social act, not background"));
    assert!(!compacted.contains("__SOUND_WAIT_EXEMPTION__"));
    assert!(!compacted.contains("__MAKE_SOUND_EXAMPLE__"));
    // The catalog's emittable ids, in catalog order.
    assert!(rendered.contains(
        r#"make_sound {"sound": "fart"}                         # Deliberately make a noise everyone within earshot perceives; sounds: fart, glass_break"#
    ));

    world.sounds_enabled = false;
    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    let compacted = compact(&rendered);
    assert!(!compacted.contains("is a social act"));
    assert!(!compacted.contains("__SOUND_WAIT_EXEMPTION__"));
    assert!(!compacted.contains("__MAKE_SOUND_EXAMPLE__"));
    assert!(!rendered.contains("make_sound"));
    // Removing the placeholder must leave the next example line intact.
    assert!(rendered.contains("eat {\"item_id\": \"fzbn9\"}"));
    assert!(rendered.contains("\nset_goal {\"goal\": \"Eat fish\"}\n"));
}

/// The markers FakeCognition parses must keep their shape (R4): a renderer
/// drift that lost either would break the flagship offline e2e silently.
#[test]
fn the_sheet_carries_the_markers_the_fake_cognition_reads() {
    let env = prompt_env();
    let world = seed_world();
    let rendered = render_prompt(&world, &actor("k0fb1"), None, &env).unwrap();
    assert_eq!(cathedral_sim::fake::sheet_name(&rendered), Some("Ilse"));
    assert!(md_section(&rendered, "since_your_last_turn").is_some());
    assert_eq!(rendered.matches("**you** — ").count(), 1);
    assert_eq!(rendered.matches("**since_your_last_turn**").count(), 1);
}

/// 11/12 (SchedulerTests, re-hosted): the prompt snapshots the inbox at drain
/// time, and a failed turn re-queues the drained percepts as new — one line is
/// never in `since_your_last_turn` and `recent_history` at once, and graduates
/// exactly once.
#[test]
fn a_failed_turn_requeues_percepts_without_duplication() {
    let env = prompt_env();
    let mut world = seed_world();
    let sven = actor("sv3n1");
    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .notify_percept(r#"Conny said to you: "Fresh fish!""#);

    // Turn 1 drains…
    let (prompt, presented) = render_prompt_and_drain(&mut world, &sven, &env).unwrap();
    assert_eq!(
        md_section(&prompt, "since_your_last_turn").unwrap(),
        [r#"Conny said to you: "Fresh fish!""#]
    );
    // The empty-history sentinel renders inline, not as a bullet.
    assert!(prompt.contains("**recent_history** — nothing yet"), "{prompt}");

    // …an event arrives while the completion is in flight…
    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .notify_percept("[You heard a big fart!]");

    // …and the provider fails: the scheduler pushes both lists back onto the
    // FRONT (scheduler.py:228-229), which is what the caller must do.
    {
        let state = &mut world.characters.get_mut(&sven).unwrap().state;
        let mut inbox = presented.clone();
        inbox.append(&mut state.inbox);
        state.inbox = inbox;
        let mut pending = presented.clone();
        pending.append(&mut state.pending_history);
        state.pending_history = pending;
    }

    // The retry shows the percept as new again, and NOT in recent_history.
    let (retry, presented) = render_prompt_and_drain(&mut world, &sven, &env).unwrap();
    assert_eq!(
        md_section(&retry, "since_your_last_turn").unwrap(),
        [
            r#"Conny said to you: "Fresh fish!""#,
            "[You heard a big fart!]"
        ]
    );
    assert!(retry.contains("**recent_history** — nothing yet"), "{retry}");

    // After the successful retry each line graduates exactly once.
    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .absorb_presented_history(&presented);
    let character = &world.characters[&sven];
    assert_eq!(
        character.recent_history(),
        [
            r#"Conny said to you: "Fresh fish!""#,
            "[You heard a big fart!]"
        ]
    );
    assert!(character.pending_history().is_empty());
    assert!(character.inbox().is_empty());
}

/// A `you_hold` id that no longer resolves is skipped, and the key stays `[]`
/// rather than disappearing (unlike `you_offer`).
#[test]
fn you_hold_is_always_present_and_skips_dangling_ids() {
    let env = prompt_env();
    let mut world = seed_world();
    assert_eq!(sheet(&world, "cb947", &env)["you_hold"], json!([]));

    world
        .characters
        .get_mut(&actor("cb947"))
        .unwrap()
        .state
        .holds = vec![ItemId::from_raw("ghost")];
    assert_eq!(sheet(&world, "cb947", &env)["you_hold"], json!([]));
}

#[test]
fn lore_profiles_are_structured_but_extended_lore_is_not_paid_every_turn() {
    let env = prompt_env();
    let mut world = seed_world();
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .sheet
        .lore = Some(LoreProfile {
        significance: cathedral_sim::Significance::Major,
        planning_ward: cathedral_sim::PlanningWard::Cinder,
        age: 19,
        gender: "m".into(),
        occupation_id: Some("smith".into()),
        occupation_display: Some("Smith".into()),
        title: Some("Blacksmith".into()),
        rank: Some("apprentice".into()),
        faction_role: None,
        illegal_activity: None,
        district: "Cinder Row".into(),
        father: None,
        mother: None,
        children: vec![actor("k0fb1")],
        circumstances: vec!["recent_migrant".into()],
        conditions: vec!["singed eyebrows".into()],
        home: Some("a house in the Cinder Ward, off Cinder Row".into()),
        core_character_description: "The prompt uses back_story for this.".into(),
        extended_character_description: "SECRET EXTENDED DETAIL".into(),
        curiosity: None,
    });

    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    let sheet = sheet(&world, "sv3n1", &env);
    assert_eq!(sheet["lore_profile"]["age"], 19);
    assert_eq!(sheet["lore_profile"]["occupation"], "Smith");
    assert_eq!(sheet["lore_profile"]["rank"], "apprentice");
    assert!(sheet["lore_profile"].get("significance").is_none());
    assert!(sheet["lore_profile"].get("planning_ward").is_none());
    // The internal occupation id never reaches the model's data.
    assert!(sheet["lore_profile"].get("occupation_id").is_none());
    assert_eq!(
        sheet["lore_profile"]["circumstances"],
        json!(["recent_migrant"])
    );
    assert_eq!(
        sheet["lore_profile"]["children"][0],
        json!({"id": "k0fb1", "name": "Ilse"})
    );
    assert_eq!(
        sheet["lore_profile"]["conditions"],
        json!(["singed eyebrows"])
    );
    // …and the markdown folds it into the `**you**` line — the baked home
    // included, so "Where do you live?" is grounded (see `homes.rs`).
    assert!(
        rendered.contains(
            "**you** — Sven, 19, male — Blacksmith (Smith, apprentice) of Cinder Row. \
             Home: a house in the Cinder Ward, off Cinder Row. \
             Family: children: Ilse (id k0fb1). Circumstances: recent_migrant. \
             Conditions: singed eyebrows."
        ),
        "{rendered}"
    );
    assert!(!rendered.contains("SECRET EXTENDED DETAIL"));
}

/// The `Home:` sentence carries the wayfinding handle the seed mints for the
/// actor's own door — `(go_to pl_…)` — and it is the same handle
/// `places_you_know` lists as their house, so walking home is one `go_to`
/// straight off the `**you**` line.
#[test]
fn the_home_line_carries_its_wayfinding_handle_when_registered() {
    use cathedral_sim::PlaceRegistry;

    let env = prompt_env();
    let mut world = seed_world();

    let mut registry = PlaceRegistry::default();
    let home_id = registry.add_home(&actor("sv3n1"), "Sven", Vec3::new(1.0, 0.91, 2.0));
    world.places = registry;
    {
        let sven = world.characters.get_mut(&actor("sv3n1")).unwrap();
        sven.state.places_known.insert(home_id.clone());
        sven.sheet.lore = Some(LoreProfile {
            significance: cathedral_sim::Significance::Major,
            planning_ward: cathedral_sim::PlanningWard::Cinder,
            age: 19,
            gender: "m".into(),
            occupation_id: Some("smith".into()),
            occupation_display: Some("Smith".into()),
            title: Some("Blacksmith".into()),
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "Cinder Row".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            home: Some("a house in the Cinder Ward, off Cinder Row".into()),
            core_character_description: String::new(),
            extended_character_description: String::new(),
            curiosity: None,
        });
    }

    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    assert!(
        rendered.contains(&format!(
            "Home: a house in the Cinder Ward, off Cinder Row (go_to {home_id})."
        )),
        "{rendered}"
    );
    assert!(
        rendered.contains(&format!("{home_id} Sven's house")),
        "the Home handle must be the places_you_know house entry: {rendered}"
    );
}

/// M5: `places_you_know` renders the actor's held handles — sorted by name so
/// the list reads the same turn after turn — and an empty set renders as an
/// empty list, never an omitted key.
#[test]
fn places_you_know_renders_the_held_handles_sorted_by_name() {
    use cathedral_sim::{NavData, PlaceRegistry};

    let env = prompt_env();
    let mut world = seed_world();
    assert_eq!(
        sheet(&world, "sv3n1", &env)["places_you_know"],
        json!([]),
        "no registry, no handles: an honest empty list"
    );

    // A compact graph and registry — the line nav from the module tests.
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
    let nav = NavData::from_parts(&nav_json, &bitset).unwrap();
    let registry_json = r#"{
        "schema_version": 1,
        "places": [
            {"id": "pl_zz01", "name": "The Wickmarket", "node": 0, "kind": "major", "ward": "wick"},
            {"id": "pl_aa01", "name": "Ford Well", "node": 3, "kind": "landmark", "ward": "fabric"}
        ],
        "wards": [
            {"id": "pl_mm01", "ward": "reed", "name": "Reed Ward", "node": 1}
        ]
    }"#;
    world.places = PlaceRegistry::from_json(registry_json, &nav).unwrap();
    {
        let sven = world.characters.get_mut(&actor("sv3n1")).unwrap();
        for id in ["pl_zz01", "pl_aa01", "pl_mm01", "pl_gone"] {
            sven.state.places_known.insert(cathedral_sim::PlaceId::from_raw(id));
        }
    }

    let rendered = sheet(&world, "sv3n1", &env);
    assert_eq!(
        rendered["places_you_know"],
        json!([
            {"place_id": "pl_aa01", "name": "Ford Well"},
            {"place_id": "pl_mm01", "name": "Reed Ward"},
            {"place_id": "pl_zz01", "name": "The Wickmarket"}
        ]),
        "sorted by name, dangling handles skipped, place_id (not id) as the key"
    );
    // Nobody else's sheet gains a handle from Sven's knowledge.
    assert_eq!(sheet(&world, "cb947", &env)["places_you_know"], json!([]));
}

/// M5: every `you_see` person carries `moving` — `!is_settled()`, the novelty
/// gate's own threshold — and the offer sections stay `moving`-free.
#[test]
fn you_see_people_carry_the_moving_flag() {
    use cathedral_sim::Movement;

    let env = prompt_env();
    let mut world = seed_world();
    // Conny walks; Ilse stands.
    world
        .characters
        .get_mut(&actor("cb947"))
        .unwrap()
        .state
        .movement = Some(Movement {
        path: vec![Vec3::new(50.0, 0.91, 0.0)],
        speed: 1.8,
        gait_phase: 0.0,
        patrol: None,
        choke_wait: 0.0,
    });

    let rendered = sheet(&world, "sv3n1", &env);
    let people = rendered["you_see"]["people"].as_array().unwrap();
    let moving: Vec<(&str, bool)> = people
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["moving"].as_bool().unwrap()))
        .collect();
    assert_eq!(
        moving,
        [("cb947", true), ("k0fb1", false), ("player", false)],
        "a man crossing the square is moving; the ones who stand are not"
    );

    // The offer sections reuse the person shape but never the flag.
    apply_action(
        &mut world,
        &actor("sv3n1"),
        "offer_item",
        &json!({"item_id": "fzbn9", "target": "cb947"}),
    )
    .unwrap();
    let rendered = sheet(&world, "sv3n1", &env);
    let to = &rendered["you_offer"][0]["to"];
    assert!(to.get("moving").is_none(), "no moving flag in you_offer: {to}");
}
