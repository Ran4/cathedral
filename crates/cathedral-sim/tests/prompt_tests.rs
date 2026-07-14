//! Prompt rendering (`tests/test_prompt_scheduler.py::PromptTests` 1-8, plus the
//! drain/restore contracts the scheduler tests pin from the outside).

mod prompt_support;

use cathedral_sim::{
    ActorId, Control, ItemId, LoreProfile, Vec3, apply_action,
    prompt::{render_prompt, render_prompt_and_drain},
};
use prompt_support::{actor, compact, prompt_env, seed_world, sheet, sheet_of};
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
        people[1]["name"].as_str().unwrap().contains("unknown"),
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
    let rendered = sheet_of(&prompt);

    assert_eq!(rendered["since_your_last_turn"], json!(["first", "second"]));
    assert_eq!(
        rendered["recent_history"],
        json!([r#"You said aloud: "earlier""#])
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

/// The fence FakeCognition parses must keep its shape (R4): a template drift
/// that renamed either key would break the flagship offline e2e silently.
#[test]
fn the_json_fence_carries_the_keys_the_fake_cognition_reads() {
    let env = prompt_env();
    let world = seed_world();
    let rendered = render_prompt(&world, &actor("k0fb1"), None, &env).unwrap();
    let fence = sheet_of(&rendered);
    assert!(fence.get("name").is_some());
    assert!(fence.get("since_your_last_turn").is_some());
    assert_eq!(rendered.matches("```json").count(), 1);
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
        sheet_of(&prompt)["since_your_last_turn"],
        json!([r#"Conny said to you: "Fresh fish!""#])
    );
    assert_eq!(sheet_of(&prompt)["recent_history"], json!(["nothing yet"]));

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
        sheet_of(&retry)["since_your_last_turn"],
        json!([
            r#"Conny said to you: "Fresh fish!""#,
            "[You heard a big fart!]"
        ])
    );
    assert_eq!(sheet_of(&retry)["recent_history"], json!(["nothing yet"]));

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
        age: 19,
        gender: "m".into(),
        occupation_id: "smith".into(),
        occupation_display: "Smith".into(),
        title: "Blacksmith".into(),
        rank: Some("apprentice".into()),
        faction_role: None,
        illegal_activity: None,
        district: "Cinder Row".into(),
        father: None,
        mother: None,
        children: vec![actor("k0fb1")],
        conditions: vec!["singed eyebrows".into()],
        core_character_description: "The prompt uses back_story for this.".into(),
        extended_character_description: "SECRET EXTENDED DETAIL".into(),
    });

    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    let sheet = sheet_of(&rendered);
    assert_eq!(sheet["lore_profile"]["age"], 19);
    assert_eq!(sheet["lore_profile"]["occupation"], "Smith");
    assert_eq!(sheet["lore_profile"]["rank"], "apprentice");
    assert_eq!(
        sheet["lore_profile"]["children"][0],
        json!({"id": "k0fb1", "name": "Ilse"})
    );
    assert_eq!(
        sheet["lore_profile"]["conditions"],
        json!(["singed eyebrows"])
    );
    assert!(!rendered.contains("SECRET EXTENDED DETAIL"));
}
