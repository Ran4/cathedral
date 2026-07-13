//! Port of `prompt_playgound/tests/test_sim.py` — DistanceTests, SpeechTests,
//! OfferTests (sim.md §10 tests 1-24) and the snapshot invariant (test 27).
//!
//! The Python test bodies are the authority on setup values and expected
//! strings; the seed-world cases (25-26) live in `seed_tests.rs`.

use std::collections::BTreeSet;

use cathedral_sim::{
    ActionErrorCode, ActorId, Character, CharacterSheet, Control, EventType, HEARING_RADIUS_M,
    ITEM_INTERACTION_RADIUS_M, Item, ItemId, PLAYER_SPEECH_MAX_CHARS, RECENT_HISTORY_MAX_ENTRIES,
    SpatialActorUpdate, Vec3, World, apply_action, vec3, vec3_from_json,
};
use serde_json::{Value, json};

// ------------------------------------------------------------------- fixtures

fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

fn item(id: &str) -> ItemId {
    ItemId::from_raw(id)
}

/// `test_sim.py:26-36` — an LLM character on the X axis. The player variant
/// carries no voice key, by convention.
fn character(actor_id: &str, name: &str, x: f64) -> Character {
    controlled_character(actor_id, name, x, Control::Llm)
}

fn controlled_character(actor_id: &str, name: &str, x: f64, control: Control) -> Character {
    Character::from_sheet(CharacterSheet {
        id: actor(actor_id),
        name: name.to_string(),
        control,
        back_story: "test".into(),
        location_description: "test square".into(),
        appearance_key: name.to_lowercase(),
        voice_key: match control {
            Control::Player => None,
            Control::Llm => Some(name.to_lowercase()),
        },
        position_m: Vec3::new(x, 0.0, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore: None,
    })
}

/// `test_sim.py:39-47` — speaker@0, target@10, bystander@5, distant@20.0001.
fn speech_world() -> World {
    let mut world = World::new();
    world.add_character(character("speaker", "Speaker", 0.0));
    world.add_character(character("target", "Target", 10.0));
    world.add_character(character("bystander", "Bystander", 5.0));
    world.add_character(character("distant", "Distant", 20.0001));
    world
}

/// `test_sim.py:238-247` — giver@0 holding apple + pear, receiver@3, other@2.
fn offer_world() -> World {
    let mut world = World::new();
    let mut giver = character("giver", "Giver", 0.0);
    giver.state.holds = vec![item("apple"), item("pear")];
    world.add_character(giver);
    world.add_character(character("receiver", "Receiver", 3.0));
    world.add_character(character("other", "Other", 2.0));
    world.add_item(Item {
        id: item("apple"),
        name: "apple".into(),
        visual_key: "apple".into(),
    });
    world.add_item(Item {
        id: item("pear"),
        name: "pear".into(),
        visual_key: "pear".into(),
    });
    world
}

/// `test_sim.py:249-255` — the giver offers an item, targeted by default.
fn offer(world: &mut World, item_id: &str, target: Option<&str>) {
    let args = match target {
        Some(target) => json!({"item_id": item_id, "target": target}),
        None => json!({"item_id": item_id, "target": Value::Null}),
    };
    apply_action(world, &actor("giver"), "offer_item", &args)
        .unwrap_or_else(|error| panic!("offer of {item_id} failed: {error}"));
}

fn inbox(world: &World, actor_id: &str) -> Vec<String> {
    world.characters[&actor(actor_id)].inbox().to_vec()
}

fn move_to(world: &mut World, actor_id: &str, x: f64) {
    world
        .characters
        .get_mut(&actor(actor_id))
        .expect("character exists")
        .state
        .position_m = Vec3::new(x, 0.0, 0.0);
}

/// Presenting a turn's percepts and completing it: the pair the prompt layer
/// calls (`take_pending_history` + `absorb_presented_history`).
fn present_and_absorb(world: &mut World, actor_id: &str) {
    let character = world
        .characters
        .get_mut(&actor(actor_id))
        .expect("character exists");
    let presented = character.take_pending_history();
    character.absorb_presented_history(&presented);
}

// -------------------------------------------------------------- DistanceTests

/// Test 1 (`test_sim.py:51`): the hearing radius is inclusive, and the result is
/// in distance order. 20 ± 1e-6 is only resolvable in f64 (D1).
#[test]
fn the_hearing_boundary_is_inclusive() {
    let mut world = World::new();
    world.add_character(character("origin", "Origin", 0.0));
    world.add_character(character("inside", "Inside", HEARING_RADIUS_M - 1e-6));
    world.add_character(character("exact", "Exact", HEARING_RADIUS_M));
    world.add_character(character("outside", "Outside", HEARING_RADIUS_M + 1e-6));

    let heard = world.characters_within(Vec3::ZERO, HEARING_RADIUS_M, Some(&actor("origin")));

    assert_eq!(heard, [actor("inside"), actor("exact")]);
}

/// Test 2 (`test_sim.py:67`): a targeted offer reaches exactly 4 m and no
/// further — 4 ± 1e-6, again f64-only.
#[test]
fn the_interaction_boundary_is_inclusive() {
    for (distance, succeeds) in [
        (ITEM_INTERACTION_RADIUS_M - 1e-6, true),
        (ITEM_INTERACTION_RADIUS_M, true),
        (ITEM_INTERACTION_RADIUS_M + 1e-6, false),
    ] {
        let mut world = World::new();
        let mut giver = character("giver", "Giver", 0.0);
        giver.state.holds = vec![item("item")];
        world.add_character(giver);
        world.add_character(character("target", "Target", distance));
        world.add_item(Item::new(item("item"), "apple"));

        let result = apply_action(
            &mut world,
            &actor("giver"),
            "offer_item",
            &json!({"item_id": "item", "target": "target"}),
        );

        match (result, succeeds) {
            (Ok(_), true) => {}
            (Err(error), false) => {
                assert_eq!(error.code, ActionErrorCode::OutOfRange);
                assert!(
                    error.message.contains("more than 4 metres"),
                    "unexpected message at {distance}: {error}"
                );
            }
            (result, _) => panic!("distance {distance} gave {result:?}"),
        }
    }
}

/// Test 3 (`test_sim.py:97`): equal distances tie-break by id.
#[test]
fn the_recipient_order_is_distance_then_id() {
    let mut world = World::new();
    world.add_character(character("origin", "Origin", 0.0));
    world.add_character(character("z", "Z", 2.0));
    world.add_character(character("b", "B", 1.0));
    world.add_character(character("a", "A", -1.0));

    let heard = world.characters_within(Vec3::ZERO, 20.0, Some(&actor("origin")));

    assert_eq!(heard, [actor("a"), actor("b"), actor("z")]);
}

/// Test 4 (`test_sim.py:112`): NaN, ±inf, bools and strings never become a
/// position. (Python validates in `Vec3.__post_init__`; in Rust the same
/// rejection lives in the validated constructor and the JSON parse path.)
#[test]
fn a_position_rejects_non_finite_and_non_numeric_components() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(vec3(bad, 0.0, 0.0).is_err(), "accepted {bad} as x");
        assert!(vec3(0.0, bad, 0.0).is_err(), "accepted {bad} as y");
        assert!(vec3(0.0, 0.0, bad).is_err(), "accepted {bad} as z");
    }
    // A bool or a string is not a number, however JSON-shaped it looks.
    for bad in [
        json!({"x": true, "y": 0, "z": 0}),
        json!({"x": "1", "y": 0, "z": 0}),
        json!({"x": null, "y": 0, "z": 0}),
    ] {
        assert!(vec3_from_json(&bad).is_err(), "accepted {bad}");
    }
    // Non-finite floats cannot even survive a JSON round trip, so the parse
    // layer is the other half of this guard.
    assert!(serde_json::from_str::<Value>(r#"{"x": NaN, "y": 0, "z": 0}"#).is_err());
    // ...and the keys must be exactly x, y, z.
    assert!(vec3_from_json(&json!({"x": 1, "y": 2})).is_err());
    assert!(vec3_from_json(&json!({"x": 1, "y": 2, "z": 3, "w": 4})).is_err());
    assert_eq!(
        vec3_from_json(&json!({"x": 1, "y": 2.5, "z": -3})).unwrap(),
        Vec3::new(1.0, 2.5, -3.0)
    );
}

// ---------------------------------------------------------------- SpeechTests

/// Test 5 (`test_sim.py:119`): a targeted line reaches the target and the nearby
/// bystander (who sees a stranger), is stripped, and skips the distant one.
#[test]
fn targeted_speech_reaches_the_target_and_a_nearby_bystander() {
    let mut world = speech_world();

    apply_action(
        &mut world,
        &actor("speaker"),
        "say",
        &json!({"target": "target", "text": "  hello  "}),
    )
    .unwrap();

    assert!(
        inbox(&world, "target")
            .last()
            .unwrap()
            .contains("said to you")
    );
    assert!(
        inbox(&world, "bystander")
            .last()
            .unwrap()
            .contains("said to a stranger (id target)")
    );
    assert!(inbox(&world, "distant").is_empty());

    let event = world.drain_events().pop().unwrap();
    assert_eq!(event.text.as_deref(), Some("hello"));
    // Distance order: the bystander at 5 m precedes the target at 10 m.
    assert_eq!(event.recipient_ids, [actor("bystander"), actor("target")]);
}

/// Test 6 (`test_sim.py:129`): the speaker's own line goes straight into
/// `recent_history`; a received one stays pending until a prompt presents it.
#[test]
fn recent_history_keeps_received_and_own_speech() {
    let mut world = speech_world();
    world
        .characters
        .get_mut(&actor("speaker"))
        .unwrap()
        .state
        .knows
        .insert(actor("target"));

    apply_action(
        &mut world,
        &actor("speaker"),
        "say",
        &json!({"target": "target", "text": "hello"}),
    )
    .unwrap();

    assert_eq!(
        world.characters[&actor("speaker")].recent_history(),
        [r#"You said to Target: "hello""#]
    );
    // Received speech is still unread: pending, not yet remembered.
    assert_eq!(
        world.characters[&actor("target")].pending_history(),
        [r#"A stranger (id speaker) said to you: "hello""#]
    );
    assert!(
        world.characters[&actor("target")]
            .recent_history()
            .is_empty()
    );
    assert!(
        world.characters[&actor("bystander")]
            .pending_history()
            .last()
            .unwrap()
            .contains("said to a stranger")
    );

    present_and_absorb(&mut world, "target");

    assert_eq!(
        world.characters[&actor("target")].recent_history(),
        [r#"A stranger (id speaker) said to you: "hello""#]
    );
    assert!(
        world.characters[&actor("target")]
            .pending_history()
            .is_empty()
    );
}

/// Test 7 (`test_sim.py:155`): the window holds 16 lines and drops the oldest.
#[test]
fn recent_history_is_bounded() {
    let mut world = speech_world();
    for index in 0..RECENT_HISTORY_MAX_ENTRIES + 3 {
        apply_action(
            &mut world,
            &actor("speaker"),
            "say",
            &json!({"text": format!("line {index}")}),
        )
        .unwrap();
    }
    present_and_absorb(&mut world, "target");

    let history = world.characters[&actor("target")].recent_history();
    assert_eq!(history.len(), RECENT_HISTORY_MAX_ENTRIES);
    assert!(!history[0].contains("line 0"));
    assert!(
        history
            .last()
            .unwrap()
            .contains(&format!("line {}", RECENT_HISTORY_MAX_ENTRIES + 2))
    );
}

/// Test 8 (`test_sim.py:168`): a broadcast reaches a hearer at exactly 20 m and
/// carries no "said to" attribution.
#[test]
fn a_broadcast_reaches_the_exact_boundary_without_a_target() {
    let mut world = speech_world();
    move_to(&mut world, "distant", HEARING_RADIUS_M);

    apply_action(
        &mut world,
        &actor("speaker"),
        "say",
        &json!({"target": Value::Null, "text": "hear ye"}),
    )
    .unwrap();

    for hearer in ["target", "bystander", "distant"] {
        assert!(!inbox(&world, hearer).is_empty(), "{hearer} heard nothing");
    }
    assert!(!inbox(&world, "target").last().unwrap().contains("said to"));
}

/// Test 9 (`test_sim.py:175`): a missing, self, or out-of-range target is an
/// error — it never silently degrades into a broadcast.
#[test]
fn a_bad_self_or_distant_target_never_broadcasts() {
    let mut world = speech_world();

    for (bad_target, code) in [
        ("missing", ActionErrorCode::UnknownTarget),
        ("speaker", ActionErrorCode::SelfTarget),
        ("distant", ActionErrorCode::OutOfRange),
    ] {
        let error = apply_action(
            &mut world,
            &actor("speaker"),
            "say",
            &json!({"target": bad_target, "text": "secret"}),
        )
        .unwrap_err();

        assert_eq!(error.code, code, "target {bad_target}");
        assert!(inbox(&world, "target").is_empty());
        assert!(inbox(&world, "bystander").is_empty());
        assert!(world.drain_events().is_empty());
    }
}

/// Test 10 (`test_sim.py:186`): the `say` schema is strict. The last two Python
/// cases (a lone surrogate, a non-string key) cannot reach `apply_action` in
/// Rust — serde_json rejects them at the parse layer, one level out, which is
/// where the guarantee now lives (sim.md risk 6).
#[test]
fn the_speech_schema_and_limits_are_strict() {
    let mut world = speech_world();
    let speaker = actor("speaker");

    for (args, code) in [
        (json!({}), ActionErrorCode::InvalidArguments),
        (json!({"text": ""}), ActionErrorCode::InvalidArguments),
        (json!({"text": 7}), ActionErrorCode::InvalidArguments),
        (
            json!({"text": "x", "unexpected": 1}),
            ActionErrorCode::InvalidArguments,
        ),
        (
            json!({"text": "x".repeat(PLAYER_SPEECH_MAX_CHARS + 1)}),
            ActionErrorCode::TextTooLong,
        ),
        (
            json!({"text": "bad\u{0}speech"}),
            ActionErrorCode::InvalidArguments,
        ),
        (json!([]), ActionErrorCode::InvalidArguments),
    ] {
        let error = apply_action(&mut world, &speaker, "say", &args).unwrap_err();
        assert_eq!(error.code, code, "args {args}");
    }

    // A lone surrogate and a non-string key are malformed JSON, not malformed
    // arguments: the reply parser refuses them before the sim ever sees them.
    assert!(serde_json::from_str::<Value>(r#"{"text": "bad\ud800speech"}"#).is_err());
    assert!(serde_json::from_str::<Value>(r#"{"text": "x", 1: "bad key"}"#).is_err());

    // Exactly at the limit is fine, and the limit counts scalar values (D11).
    assert!(
        apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"text": "é".repeat(PLAYER_SPEECH_MAX_CHARS)}),
        )
        .is_ok()
    );
    assert!(world.drain_events().len() == 1);
}

/// Test 11 (`test_sim.py:203`): the speaker never hears themselves.
#[test]
fn the_speaker_never_receives_their_own_utterance() {
    let mut world = speech_world();

    apply_action(
        &mut world,
        &actor("speaker"),
        "say",
        &json!({"text": "hello"}),
    )
    .unwrap();

    assert!(inbox(&world, "speaker").is_empty());
    assert_eq!(
        world.characters[&actor("speaker")].recent_history(),
        [r#"You said aloud: "hello""#]
    );
}

/// Test 12 (`test_sim.py:209`): `wait` changes nothing at all.
#[test]
fn wait_is_a_valid_no_op() {
    let mut world = speech_world();
    let speaker = actor("speaker");
    let revision = world.world_revision;

    let line = apply_action(&mut world, &speaker, "wait", &json!({})).unwrap();

    assert_eq!(line, "Speaker waits");
    assert_eq!(world.world_revision, revision);
    assert!(world.drain_events().is_empty());
    assert!(apply_action(&mut world, &speaker, "wait", &json!({"unexpected": true})).is_err());
}

/// Test 13 (`test_sim.py:219`): the player is a structured recipient without an
/// inbox — prose accumulates only for the actors that will be prompted.
#[test]
fn the_player_receives_structured_speech_without_private_inbox_growth() {
    let mut world = World::new();
    world.add_character(character("speaker", "Speaker", 0.0));
    world.add_character(controlled_character(
        "player",
        "Player",
        1.0,
        Control::Player,
    ));

    for index in 0..3 {
        apply_action(
            &mut world,
            &actor("speaker"),
            "say",
            &json!({"text": format!("hello {index}")}),
        )
        .unwrap();
    }

    let events = world.drain_events();
    assert!(inbox(&world, "player").is_empty());
    assert_eq!(events.len(), 3);
    assert!(
        events
            .iter()
            .all(|event| event.recipient_ids == [actor("player")])
    );
}

// ----------------------------------------------------------------- OfferTests

/// Test 14 (`test_sim.py:257`): offering does not transfer, and two offers of
/// two items coexist.
#[test]
fn an_offer_does_not_transfer_and_multiple_offers_coexist() {
    let mut world = offer_world();
    let revision = world.world_revision;

    offer(&mut world, "apple", Some("receiver"));
    offer(&mut world, "pear", None);

    assert_eq!(
        world.characters[&actor("giver")].holds(),
        [item("apple"), item("pear")]
    );
    let offered: BTreeSet<ItemId> = world.offers.keys().cloned().collect();
    assert_eq!(offered, BTreeSet::from([item("apple"), item("pear")]));
    assert!(world.world_revision > revision);
    assert!(world.offers[&item("apple")].created_seq < world.offers[&item("pear")].created_seq);
}

/// Test 15 (`test_sim.py:269`): only the target may accept a targeted offer, and
/// accepting is the one thing that moves an item.
#[test]
fn only_the_target_may_accept_a_targeted_offer() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));

    let error = apply_action(
        &mut world,
        &actor("other"),
        "accept_offered_item",
        &json!({"item_id": "apple"}),
    )
    .unwrap_err();
    assert_eq!(error.code, ActionErrorCode::NotOfferTarget);

    apply_action(
        &mut world,
        &actor("receiver"),
        "accept_offered_item",
        &json!({"item_id": "apple"}),
    )
    .unwrap();

    assert!(
        !world.characters[&actor("giver")]
            .holds()
            .contains(&item("apple"))
    );
    assert!(
        world.characters[&actor("receiver")]
            .holds()
            .contains(&item("apple"))
    );
    assert!(!world.offers.contains_key(&item("apple")));
    world.assert_invariants();
}

/// Test 16 (`test_sim.py:283`): a broadcast offer goes to whoever takes it first.
#[test]
fn the_first_accept_of_a_broadcast_offer_wins() {
    let mut world = offer_world();
    offer(&mut world, "apple", None);

    apply_action(
        &mut world,
        &actor("other"),
        "accept_offered_item",
        &json!({"item_id": "apple"}),
    )
    .unwrap();

    let error = apply_action(
        &mut world,
        &actor("receiver"),
        "accept_offered_item",
        &json!({"item_id": "apple"}),
    )
    .unwrap_err();
    assert_eq!(error.code, ActionErrorCode::NoOffer);
}

/// Test 17 (`test_sim.py:293`): accept revalidates proximity, and a failed range
/// check leaves the offer standing.
#[test]
fn accept_revalidates_distance_but_the_offer_persists() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));
    move_to(&mut world, "receiver", 5.0);

    let error = apply_action(
        &mut world,
        &actor("receiver"),
        "accept_offered_item",
        &json!({"item_id": "apple"}),
    )
    .unwrap_err();

    assert_eq!(error.code, ActionErrorCode::OutOfRange);
    assert!(error.message.contains("more than 4 metres"));
    assert!(world.offers.contains_key(&item("apple")));
}

/// Test 18 (`test_sim.py:302`): declining clears the offer and leaves the item
/// with the giver; a broadcast offer can only be ignored.
#[test]
fn a_targeted_offer_can_be_declined_and_a_broadcast_one_cannot() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));

    apply_action(
        &mut world,
        &actor("receiver"),
        "decline_offer",
        &json!({"item_id": "apple"}),
    )
    .unwrap();

    assert!(
        world.characters[&actor("giver")]
            .holds()
            .contains(&item("apple"))
    );
    assert!(!world.offers.contains_key(&item("apple")));

    offer(&mut world, "apple", None);
    let error = apply_action(
        &mut world,
        &actor("receiver"),
        "decline_offer",
        &json!({"item_id": "apple"}),
    )
    .unwrap_err();

    assert_eq!(error.code, ActionErrorCode::BroadcastCannotDecline);
    assert!(error.message.contains("open to anyone"));
}

/// Test 19 (`test_sim.py:313`): decline revalidates proximity too, and the offer
/// survives the failure.
#[test]
fn decline_revalidates_distance() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));
    move_to(&mut world, "receiver", 100.0);

    let error = apply_action(
        &mut world,
        &actor("receiver"),
        "decline_offer",
        &json!({"item_id": "apple"}),
    )
    .unwrap_err();

    assert_eq!(error.code, ActionErrorCode::OutOfRange);
    assert!(world.offers.contains_key(&item("apple")));
}

/// Test 20 (`test_sim.py:322`): retracting needs no proximity — and a target too
/// far to hear it gets no magic percept.
#[test]
fn retract_needs_no_proximity_and_a_distant_target_gets_no_magic_history() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));
    world
        .characters
        .get_mut(&actor("receiver"))
        .unwrap()
        .state
        .inbox
        .clear();
    move_to(&mut world, "receiver", 100.0);

    apply_action(
        &mut world,
        &actor("giver"),
        "retract_offer",
        &json!({"item_id": "apple"}),
    )
    .unwrap();

    assert!(inbox(&world, "receiver").is_empty());
    assert!(!world.offers.contains_key(&item("apple")));
}

/// Test 21 (`test_sim.py:332`): re-offering displaces the old target, who is
/// told in prose and gets a `retract_offer` event *before* the new offer.
#[test]
fn a_reoffer_replaces_the_offer_and_notifies_a_near_displaced_target() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));
    world.drain_events();
    world
        .characters
        .get_mut(&actor("receiver"))
        .unwrap()
        .state
        .inbox
        .clear();

    offer(&mut world, "apple", Some("other"));

    assert!(inbox(&world, "receiver")[0].contains("withdrew"));
    assert_eq!(
        world.offers[&item("apple")].target_id.as_ref(),
        Some(&actor("other"))
    );

    let events = world.drain_events();
    let kinds: Vec<&str> = events.iter().map(|event| event.kind.as_str()).collect();
    assert_eq!(kinds, ["retract_offer", "offer_item"]);
    assert_eq!(events[0].target_id.as_ref(), Some(&actor("receiver")));
    assert_eq!(events[0].recipient_ids, [actor("receiver")]);
}

/// Test 22 (`test_sim.py:351`): a displaced *player* gets the structured events
/// and no prose at all.
#[test]
fn a_displaced_player_gets_structured_feedback_only() {
    let mut world = offer_world();
    world
        .characters
        .get_mut(&actor("receiver"))
        .unwrap()
        .sheet
        .control = Control::Player;

    offer(&mut world, "apple", Some("receiver"));
    world.drain_events();

    offer(&mut world, "apple", Some("other"));

    assert!(inbox(&world, "receiver").is_empty());
    let events = world.drain_events();
    assert_eq!(events[0].kind, "retract_offer");
    assert_eq!(events[0].recipient_ids, [actor("receiver")]);
    assert_eq!(events[1].kind, "offer_item");
    assert!(events[1].recipient_ids.contains(&actor("receiver")));
}

/// Test 23 (`test_sim.py:370`): eating an offered item retracts it implicitly
/// and removes the item from the world — items are singular.
#[test]
fn eating_retracts_and_removes_the_singular_item() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));

    apply_action(
        &mut world,
        &actor("giver"),
        "eat",
        &json!({"item_id": "apple"}),
    )
    .unwrap();

    assert!(!world.items.contains_key(&item("apple")));
    assert!(!world.offers.contains_key(&item("apple")));
    assert!(
        !world.characters[&actor("giver")]
            .holds()
            .contains(&item("apple"))
    );
    world.assert_invariants();
}

/// Test 24 (`test_sim.py:377`): `offer_item` rejects self-targets, unknown ids,
/// extra keys, non-string targets, and items the actor does not hold.
#[test]
fn offer_rejects_self_unknown_targets_extra_fields_and_unheld_items() {
    let mut world = offer_world();
    let giver = actor("giver");

    for (args, code) in [
        (
            json!({"item_id": "apple", "target": "giver"}),
            ActionErrorCode::SelfTarget,
        ),
        (
            json!({"item_id": "apple", "target": "missing"}),
            ActionErrorCode::UnknownTarget,
        ),
        (
            json!({"item_id": "apple", "extra": 1}),
            ActionErrorCode::InvalidArguments,
        ),
        (
            json!({"item_id": "apple", "target": 4}),
            ActionErrorCode::InvalidArguments,
        ),
        // An id-shaped string nobody holds — `item_id` takes ids, not names.
        (json!({"item_id": "not-an-id"}), ActionErrorCode::NotOwner),
    ] {
        let error = apply_action(&mut world, &giver, "offer_item", &args).unwrap_err();
        assert_eq!(error.code, code, "args {args}");
    }
    assert!(world.offers.is_empty());
    assert!(world.drain_events().is_empty());
}

// ------------------------------------------------------------- SnapshotTests

/// Test 27 (`test_sim.py:424`): the snapshot is public-only and its revision is
/// monotonic. The seed-world variant lives in `seed_tests.rs`.
///
/// The privacy invariant is the load-bearing half: everything a character
/// thinks, remembers, wants, hears, or is voiced by stays out of the projection
/// the player's client receives.
#[test]
fn the_snapshot_is_public_and_monotonic() {
    let mut world = World::new();
    world.add_character(controlled_character(
        "player",
        "Player",
        0.0,
        Control::Player,
    ));
    for (id, name, x) in [
        ("sv3n1", "Sven", 1.0),
        ("cb947", "Conny", 2.0),
        ("k0fb1", "Ilse", 3.0),
    ] {
        let mut npc = character(id, name, x);
        npc.sheet.back_story = "a secret back story".into();
        npc.sheet.voice_key = Some("a secret voice".into());
        npc.state.goal = "a secret goal".into();
        npc.state.memories = vec!["a secret memory".into()];
        npc.state.knows.insert(actor("player"));
        world.add_character(npc);
    }
    // Fill every private buffer the sim owns, so an accidental derive would show.
    apply_action(
        &mut world,
        &actor("sv3n1"),
        "say",
        &json!({"text": "a secret utterance"}),
    )
    .unwrap();
    world.drain_events();

    let snapshot = world.public_snapshot(&actor("player"));
    let encoded = serde_json::to_string(&snapshot).unwrap();

    assert_eq!(snapshot.actors.len(), 4);
    for private_field in [
        "back_story",
        "memories",
        "goal",
        "voice_key",
        "inbox",
        "recent_history",
        "pending_history",
        "knows",
    ] {
        assert!(
            !encoded.contains(private_field),
            "the snapshot leaks the field {private_field}: {encoded}"
        );
    }
    for private_value in [
        "a secret back story",
        "a secret goal",
        "a secret memory",
        "a secret utterance",
        // The voice key is private too — it is an NPC's TTS identity, not a
        // visual one (`appearance_key` is public and stays public).
        "a secret voice",
    ] {
        assert!(
            !encoded.contains(private_value),
            "the snapshot leaks the value {private_value}: {encoded}"
        );
    }

    let before = snapshot.world_revision;
    world
        .update_positions(
            1,
            &[SpatialActorUpdate::new(
                actor("player"),
                Vec3::new(1.0, 2.0, 3.0),
                None,
            )],
        )
        .unwrap();

    assert!(world.public_snapshot(&actor("player")).world_revision > before);
}

/// The event id prefixes the game keys speech dedupe and sound playback on
/// (`sim.py:153-157`): a `world_event` gets the `world` prefix, not `world_event`.
#[test]
fn event_ids_carry_the_type_prefix() {
    let mut world = offer_world();
    apply_action(
        &mut world,
        &actor("giver"),
        "say",
        &json!({"text": "hello"}),
    )
    .unwrap();
    offer(&mut world, "apple", Some("receiver"));

    let events = world.drain_events();
    assert_eq!(events[0].event_type, EventType::Speech);
    assert_eq!(
        events[0].event_id(),
        format!("speech-{}", events[0].sequence)
    );
    assert_eq!(events[1].event_type, EventType::WorldEvent);
    assert_eq!(
        events[1].event_id(),
        format!("world-{}", events[1].sequence)
    );
}
