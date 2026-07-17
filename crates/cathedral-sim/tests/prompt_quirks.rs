//! The quirks with no Python test of their own (prompt.md §8, "Additional
//! Rust-only tests").
//!
//! Every one of these looks like a bug on first reading. They are current
//! Python behavior and the game depends on them; the tests exist so that
//! changing any of them has to be deliberate (R21).

mod prompt_support;

use cathedral_sim::{Item, ItemId, Offer, Vec3, apply_action, prompt::render_prompt};
use prompt_support::{actor, prompt_env, seed_world, sheet};
use serde_json::json;

/// 1. A targeted offer whose target has left the world renders as
///    `"to": "anyone"` — indistinguishable from a broadcast (`prompt.py:186-196`).
#[test]
fn a_dangling_offer_target_renders_as_anyone() {
    let env = prompt_env();
    let mut world = seed_world();
    apply_action(
        &mut world,
        &actor("k0fb1"),
        "offer_item",
        &json!({"item_id": "c0prs", "target": "sv3n1"}),
    )
    .unwrap();

    let rendered = sheet(&world, "k0fb1", &env);
    assert_eq!(rendered["you_offer"][0]["to"]["id"], "sv3n1");

    world.characters.remove(&actor("sv3n1"));

    let rendered = sheet(&world, "k0fb1", &env);
    assert_eq!(rendered["you_offer"][0]["to"], json!("anyone"));
}

/// 2. An item that has left the world is skipped everywhere: `you_hold`,
///    `you_offer` and `offered_to_you` alike (`prompt.py:182-184`, `:222-226`).
#[test]
fn items_missing_from_the_world_are_skipped_in_every_section() {
    let env = prompt_env();
    let mut world = seed_world();
    apply_action(
        &mut world,
        &actor("k0fb1"),
        "offer_item",
        &json!({"item_id": "c0prs", "target": "sv3n1"}),
    )
    .unwrap();
    assert!(sheet(&world, "k0fb1", &env).get("you_offer").is_some());
    assert!(sheet(&world, "sv3n1", &env).get("offered_to_you").is_some());

    // The item evaporates; the offer and `holds` still reference it.
    world.items.remove(&ItemId::from_raw("c0prs"));

    let giver = sheet(&world, "k0fb1", &env);
    assert!(giver.get("you_offer").is_none());
    assert_eq!(giver["you_hold"], json!([]));
    assert!(sheet(&world, "sv3n1", &env).get("offered_to_you").is_none());
}

/// 3. Both sections follow one global sort: `(created_seq, item_id)` — neither
///    insertion order nor the map's key order (`prompt.py:162-163`, `:180`).
#[test]
fn offers_sort_by_created_seq_then_item_id_in_both_sections() {
    let env = prompt_env();
    let mut world = seed_world();
    let conny = actor("cb947");
    let sven = actor("sv3n1");

    for (item_id, name) in [("zz001", "anvil"), ("aa002", "loaf"), ("mm003", "rope")] {
        world.add_item(Item::new(ItemId::from_raw(item_id), name));
        world
            .characters
            .get_mut(&conny)
            .unwrap()
            .state
            .holds
            .push(ItemId::from_raw(item_id));
    }
    // Created in the order zz001, aa002, mm003, with sequences 30, 10, 10.
    for (item_id, created_seq) in [("zz001", 30), ("aa002", 10), ("mm003", 10)] {
        world.offers.insert(
            ItemId::from_raw(item_id),
            Offer {
                item_id: ItemId::from_raw(item_id),
                giver_id: conny.clone(),
                target_id: Some(sven.clone()),
                created_seq,
                quantity: 1,
            },
        );
    }

    let item_ids = |value: &serde_json::Value| -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["item"]["id"].as_str().unwrap().to_string())
            .collect()
    };

    // seq 10 before seq 30; within seq 10, aa002 before mm003.
    assert_eq!(
        item_ids(&sheet(&world, "cb947", &env)["you_offer"]),
        ["aa002", "mm003", "zz001"]
    );
    assert_eq!(
        item_ids(&sheet(&world, "sv3n1", &env)["offered_to_you"]),
        ["aa002", "mm003", "zz001"]
    );
}

/// 4. The markdown sheet is raw UTF-8: non-ASCII history reaches the model
///    verbatim, where the JSON sheet used to `ensure_ascii`-escape it into
///    `\uXXXX` (and pay tokens for the privilege).
#[test]
fn non_ascii_history_renders_verbatim() {
    let env = prompt_env();
    let mut world = seed_world();
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .recent_history = vec!["I\u{2019}ll go \u{1F41F}".into()];

    let rendered = render_prompt(&world, &actor("sv3n1"), None, &env).unwrap();
    assert!(rendered.contains("- I\u{2019}ll go \u{1F41F}"), "{rendered}");
    assert!(!rendered.contains("\\u2019"), "{rendered}");
}

/// 5. The verb matcher's corners: uppercase is accepted and lowercased, a bare
///    `wait` is not an action, no space is needed before `{`, and a comment may be
///    glued to the JSON.
#[test]
fn the_verb_matcher_corners() {
    let (actions, errors) = cathedral_sim::parse_reply(
        "SAY {\"text\": \"x\"}\nwait\nsay{\"text\":\"y\"}\nwait {}#c\n  Wait\t {}  ",
    );
    let verbs: Vec<&str> = actions.iter().map(|(verb, _)| verb.as_str()).collect();
    assert_eq!(verbs, ["say", "say", "wait", "wait"]);
    assert_eq!(errors, ["not understood: wait"]);
}

/// 6. A cleared goal is the literal string `"None"`, never JSON `null` (D15).
#[test]
fn a_cleared_goal_is_the_string_none() {
    let env = prompt_env();
    let mut world = seed_world();
    let conny = actor("cb947");

    apply_action(
        &mut world,
        &conny,
        "set_goal",
        &json!({"goal": "Sell fish"}),
    )
    .unwrap();
    assert_eq!(sheet(&world, "cb947", &env)["current_goal"], "Sell fish");

    apply_action(&mut world, &conny, "set_goal", &json!({"goal": null})).unwrap();
    assert_eq!(sheet(&world, "cb947", &env)["current_goal"], json!("None"));
    // Seeded characters start there too.
    assert_eq!(sheet(&seed_world(), "sv3n1", &env)["current_goal"], "None");
}

/// 7. Empty collections: `people` renders `[]` (the key survives), and the two
///    history fields fall back to their one-element sentinels.
#[test]
fn empty_people_and_the_history_fallbacks() {
    let env = prompt_env();
    let mut world = seed_world();
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .position_m = Vec3::new(0.0, 0.91, 400.0);

    let rendered = sheet(&world, "sv3n1", &env);
    assert_eq!(rendered["you_see"]["people"], json!([]));
    assert_eq!(rendered["since_your_last_turn"], json!(["nothing"]));
    assert_eq!(rendered["recent_history"], json!(["nothing yet"]));

    // `stored_memories` has no fallback — an empty list stays empty.
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .memories
        .clear();
    assert_eq!(sheet(&world, "sv3n1", &env)["stored_memories"], json!([]));
}

/// 8. `you_offer` / `offered_to_you` are OMITTED when empty, not rendered as
///    empty sections, and they sit between `you_hold` and `you_see`: the sheet's
///    section order is the `Sheet` struct's declaration order.
#[test]
fn the_sheet_section_order_and_the_omitted_offer_sections() {
    let env = prompt_env();
    let mut world = seed_world();

    let sections = |world: &cathedral_sim::World| -> Vec<String> {
        let prompt = render_prompt(world, &actor("sv3n1"), None, &env).unwrap();
        // A section header is a line that opens with a bold label.
        prompt
            .lines()
            .filter(|line| line.starts_with("**"))
            .map(|line| line[2..].split("**").next().unwrap().to_string())
            .collect()
    };

    assert_eq!(
        sections(&world),
        [
            "you",
            "back_story",
            "you_are",
            "places_you_know",
            "you_hold",
            "you_see",
            "since_your_last_turn",
            "recent_history",
            "stored_memories",
            "the_only_languages_you_know",
            "current_goal",
        ]
    );

    apply_action(
        &mut world,
        &actor("sv3n1"),
        "offer_item",
        &json!({"item_id": "fzbn9", "target": "cb947"}),
    )
    .unwrap();
    apply_action(
        &mut world,
        &actor("k0fb1"),
        "offer_item",
        &json!({"item_id": "c0prs", "target": "sv3n1"}),
    )
    .unwrap();

    assert_eq!(
        sections(&world),
        [
            "you",
            "back_story",
            "you_are",
            "places_you_know",
            "you_hold",
            "you_offer",
            "offered_to_you",
            "you_see",
            "since_your_last_turn",
            "recent_history",
            "stored_memories",
            "the_only_languages_you_know",
            "current_goal",
        ]
    );
}
