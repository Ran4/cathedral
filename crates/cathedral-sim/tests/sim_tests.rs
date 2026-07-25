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
        pockets: Vec::new(),
        frontbutt: None,
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
        position_m: Vec3::new(x, 0.0, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: BTreeSet::new(),
        lore: None,
        presence: cathedral_sim::Presence::InCity,
        presence_epoch: 0,
        economic_class: cathedral_sim::EconomicClass::Resident,
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
    world.add_item(Item::new(item("apple"), "apple"));
    world.add_item(Item::new(item("pear"), "pear"));
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

/// The M5 commitment rule supersedes Python test 23: eating never silently
/// retracts a promise, so an entirely offered singular item stays untouched.
#[test]
fn eating_an_entirely_offered_singular_item_is_rejected() {
    let mut world = offer_world();
    offer(&mut world, "apple", Some("receiver"));

    let error = apply_action(
        &mut world,
        &actor("giver"),
        "eat",
        &json!({"item_id": "apple"}),
    )
    .unwrap_err();

    assert_eq!(error.code, ActionErrorCode::ItemCommitted);
    assert!(world.items.contains_key(&item("apple")));
    assert!(world.offers.contains_key(&item("apple")));
    assert!(world.characters[&actor("giver")].holds().contains(&item("apple")));
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
        // visual one (the `appearance` snapshot is public and stays public).
        "a secret voice",
    ] {
        assert!(
            !encoded.contains(private_value),
            "the snapshot leaks the value {private_value}: {encoded}"
        );
    }

    // A player move rides the hot channel: applied, but it does not bump the
    // public revision — the host owns the player's transform.
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
    assert_eq!(
        world.public_snapshot(&actor("player")).world_revision,
        before,
        "a player move stays off the cold channel"
    );

    // A non-player move is a public change, and the revision is monotonic in it.
    world
        .update_positions(
            2,
            &[SpatialActorUpdate::new(
                actor("sv3n1"),
                Vec3::new(9.0, 2.0, 3.0),
                None,
            )],
        )
        .unwrap();
    assert!(
        world.public_snapshot(&actor("player")).world_revision > before,
        "a non-player move bumps the revision"
    );
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

// ------------------------------------------------- M0: kinds, stacks & merges
//
// The split/merge logic is the only genuinely subtle code in M0, so it is
// tested hard here — including the risk ledger's property test (any sequence of
// offers/accepts/eats conserves total quantity per kind).

/// Giver@0, receiver@1 (well within the 4 m exchange radius), no items yet.
fn stack_world() -> World {
    let mut world = World::new();
    world.add_character(character("giver", "Giver", 0.0));
    world.add_character(character("receiver", "Receiver", 1.0));
    world
}

/// Put a stack on a holder.
fn hold_stack(world: &mut World, holder: &str, id: &str, kind: &str, quantity: u32) {
    world.add_item(Item::stack(item(id), kind, quantity));
    world
        .characters
        .get_mut(&actor(holder))
        .unwrap()
        .state
        .holds
        .push(item(id));
}

fn hold_cloth(world: &mut World, holder: &str, id: &str, grade: &str) {
    world.add_item(Item::new(item(id), "cloth").with_metadata("grade", grade));
    world
        .characters
        .get_mut(&actor(holder))
        .unwrap()
        .state
        .holds
        .push(item(id));
}

fn offer_n(world: &mut World, giver: &str, id: &str, target: &str, quantity: Option<u32>) {
    let args = match quantity {
        Some(q) => json!({"item_id": id, "target": target, "quantity": q}),
        None => json!({"item_id": id, "target": target}),
    };
    apply_action(world, &actor(giver), "offer_item", &args)
        .unwrap_or_else(|error| panic!("offer failed: {error}"));
}

fn accept(world: &mut World, taker: &str, id: &str) -> Result<String, ActionErrorCode> {
    apply_action(world, &actor(taker), "accept_offered_item", &json!({"item_id": id}))
        .map_err(|error| error.code)
}

/// The milestone's headline scenario: a scripted turn offers `quantity: 2`, the
/// counterpart accepts, and the giver is left with 1 and the receiver holds 2 —
/// under a fresh, deterministic id.
#[test]
fn a_partial_offer_splits_the_stack_leaving_one_and_two() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "spk", "spark", 3);

    offer_n(&mut world, "giver", "spk", "receiver", Some(2));
    accept(&mut world, "receiver", "spk").unwrap();

    // Giver keeps the remainder under the original id.
    assert_eq!(world.characters[&actor("giver")].holds(), [item("spk")]);
    assert_eq!(world.items[&item("spk")].quantity, 1);

    // Receiver holds exactly one new stack of 2, with a fresh id.
    let received = world.characters[&actor("receiver")].holds().to_vec();
    assert_eq!(received.len(), 1);
    assert_ne!(received[0], item("spk"));
    assert_eq!(world.items[&received[0]].quantity, 2);
    assert_eq!(world.items[&received[0]].kind.as_str(), "spark");
    world.assert_invariants();
}

/// The split id is a pure function of parent id and event sequence, so the same
/// world replays to the same id.
#[test]
fn the_split_id_is_deterministic() {
    let mint = |()| {
        let mut world = stack_world();
        hold_stack(&mut world, "giver", "spk", "spark", 3);
        offer_n(&mut world, "giver", "spk", "receiver", Some(2));
        accept(&mut world, "receiver", "spk").unwrap();
        world.characters[&actor("receiver")].holds()[0].clone()
    };
    assert_eq!(mint(()), mint(()));
}

/// A whole-stack offer to a receiver holding no same-stuff keeps the stack's id.
#[test]
fn a_whole_stack_offer_moves_the_id_intact() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "spk", "spark", 4);

    offer_n(&mut world, "giver", "spk", "receiver", None);
    accept(&mut world, "receiver", "spk").unwrap();

    assert!(world.characters[&actor("giver")].holds().is_empty());
    assert_eq!(world.characters[&actor("receiver")].holds(), [item("spk")]);
    assert_eq!(world.items[&item("spk")].quantity, 4);
    world.assert_invariants();
}

/// A whole-stack offer folds into the receiver's same-stuff stack: the receiver's
/// id survives, the moving id disappears.
#[test]
fn a_whole_stack_offer_merges_into_the_receivers_stack() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "give", "spark", 2);
    hold_stack(&mut world, "receiver", "keep", "spark", 5);

    offer_n(&mut world, "giver", "give", "receiver", None);
    accept(&mut world, "receiver", "give").unwrap();

    assert!(world.characters[&actor("giver")].holds().is_empty());
    assert_eq!(world.characters[&actor("receiver")].holds(), [item("keep")]);
    assert_eq!(world.items[&item("keep")].quantity, 7);
    assert!(!world.items.contains_key(&item("give")), "the moving id is gone");
    world.assert_invariants();
}

/// A partial offer into an existing same-stuff stack mints no new id.
#[test]
fn a_partial_offer_folds_into_an_existing_stack_without_a_new_id() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "give", "spark", 3);
    hold_stack(&mut world, "receiver", "keep", "spark", 1);

    offer_n(&mut world, "giver", "give", "receiver", Some(2));
    accept(&mut world, "receiver", "give").unwrap();

    assert_eq!(world.items[&item("give")].quantity, 1);
    assert_eq!(world.characters[&actor("receiver")].holds(), [item("keep")]);
    assert_eq!(world.items[&item("keep")].quantity, 3);
    world.assert_invariants();
}

/// Metadata is part of identity: broadcloth never merges with kersey.
#[test]
fn different_metadata_never_merges() {
    let mut world = stack_world();
    hold_cloth(&mut world, "giver", "broad", "broadcloth");
    hold_cloth(&mut world, "receiver", "kersey", "kersey");

    offer_n(&mut world, "giver", "broad", "receiver", None);
    accept(&mut world, "receiver", "broad").unwrap();

    let mut held = world.characters[&actor("receiver")].holds().to_vec();
    held.sort();
    assert_eq!(held, [item("broad"), item("kersey")]);
    world.assert_invariants();
}

/// A non-stackable kind (a served bowl of stew) is one stack of quantity 1; a
/// second bowl is a second id, and the two coexist on one holder.
#[test]
fn non_stackable_bowls_do_not_merge() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "bowl1", "stew", 1);
    hold_stack(&mut world, "receiver", "bowl2", "stew", 1);

    offer_n(&mut world, "giver", "bowl1", "receiver", None);
    accept(&mut world, "receiver", "bowl1").unwrap();

    let mut held = world.characters[&actor("receiver")].holds().to_vec();
    held.sort();
    assert_eq!(held, [item("bowl1"), item("bowl2")]);
    world.assert_invariants();
}

/// `offer_item` rejects a quantity outside `1..=stack`.
#[test]
fn offering_a_bad_quantity_is_an_error() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "spk", "spark", 2);

    for bad in [0u32, 3, 99] {
        let error = apply_action(
            &mut world,
            &actor("giver"),
            "offer_item",
            &json!({"item_id": "spk", "target": "receiver", "quantity": bad}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::BadQuantity, "quantity {bad}");
    }
    assert!(world.offers.is_empty());
}

/// `eat` decrements a stack, removes it at zero, and refuses non-food.
#[test]
fn eating_decrements_then_removes_and_refuses_non_food() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "hrr", "herring", 2);
    hold_stack(&mut world, "giver", "spk", "spark", 1);

    apply_action(&mut world, &actor("giver"), "eat", &json!({"item_id": "hrr"})).unwrap();
    assert_eq!(world.items[&item("hrr")].quantity, 1);

    apply_action(&mut world, &actor("giver"), "eat", &json!({"item_id": "hrr"})).unwrap();
    assert!(!world.items.contains_key(&item("hrr")), "the last unit is gone");

    let error =
        apply_action(&mut world, &actor("giver"), "eat", &json!({"item_id": "spk"})).unwrap_err();
    assert_eq!(error.code, ActionErrorCode::NotEdible);
    world.assert_invariants();
}

/// An accept whose promised quantity no longer fits the shrunken stack fails as
/// a stale offer (and repairs it).
#[test]
fn an_accept_of_more_than_remains_is_a_stale_offer() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "hrr", "herring", 2);

    offer_n(&mut world, "giver", "hrr", "receiver", Some(2));
    // Simulate a corrupt/legacy writer shrinking the stack behind the offer.
    // The public eat path cannot create this state now that commitments are
    // enforced centrally, but accept still repairs stale imported state.
    world.items.get_mut(&item("hrr")).unwrap().quantity = 1;

    assert_eq!(accept(&mut world, "receiver", "hrr"), Err(ActionErrorCode::StaleOffer));
    assert!(world.offers.is_empty(), "the stale offer was repaired");
    assert_eq!(world.items[&item("hrr")].quantity, 1);
    world.assert_invariants();
}

/// The M4 acceptance walk (`features/food_and_items/06_milestones.md` §M4),
/// mechanically: famished Ilse holds one spark; she offers it to the vendor, who
/// accepts, offers a herring back, and she accepts and eats it. The world dump
/// then shows her hands empty, her hunger lifted clear of the condition, and the
/// spark in the vendor's purse — the completed trade session 224's hallucinated
/// market only ever pretended to make.
#[test]
fn the_ilse_purchase_walks_offer_accept_and_eat() {
    use cathedral_sim::{HUNGER_FAMISHED, HUNGER_HUNGRY};

    let mut world = World::new();
    world.add_character(character("ilse", "Ilse", 0.0));
    world.add_character(character("wyn", "Wyn", 1.0)); // within the 4 m exchange radius
    // Famished, one spark to her name — Ilse's exact arithmetic.
    world.characters.get_mut(&actor("ilse")).unwrap().state.needs.hunger = HUNGER_FAMISHED - 7.0;
    hold_stack(&mut world, "ilse", "c0prs", "spark", 1);
    hold_stack(&mut world, "wyn", "hrr", "herring", 6);
    world.assert_invariants();

    // 1. Ilse offers her one spark; 2. the vendor accepts — the coin moves once.
    offer_n(&mut world, "ilse", "c0prs", "wyn", Some(1));
    accept(&mut world, "wyn", "c0prs").expect("the vendor takes the spark");
    assert!(
        world.characters[&actor("ilse")].holds().is_empty(),
        "Ilse spent her only spark"
    );
    assert_eq!(world.items[&item("c0prs")].quantity, 1, "the spark is whole, just moved");
    assert!(
        world.characters[&actor("wyn")].holds().contains(&item("c0prs")),
        "the spark is in the vendor's purse"
    );

    // 3. The vendor offers a herring; 4. Ilse accepts — one splits off the board.
    offer_n(&mut world, "wyn", "hrr", "ilse", Some(1));
    accept(&mut world, "ilse", "hrr").expect("Ilse takes the herring");
    assert_eq!(world.items[&item("hrr")].quantity, 5, "one herring left the board");
    let herring = world.characters[&actor("ilse")]
        .holds()
        .iter()
        .find(|id| world.items.get(*id).is_some_and(|item| item.kind.as_str() == "herring"))
        .cloned()
        .expect("Ilse now holds a herring");

    // 5. Ilse eats it: the herring's satiety (70) lifts her clear of the condition.
    apply_action(&mut world, &actor("ilse"), "eat", &json!({ "item_id": herring.as_str() })).unwrap();
    assert!(
        world.characters[&actor("ilse")].holds().is_empty(),
        "she carries nothing after the meal — wallet empty, hands empty"
    );
    assert!(
        world.characters[&actor("ilse")].needs().hunger >= HUNGER_HUNGRY,
        "the meal cleared the hunger condition: hunger is {}",
        world.characters[&actor("ilse")].needs().hunger
    );
    world.assert_invariants();
}

/// Definition of done: two same-stuff stacks on one holder is a bug the
/// invariants catch.
#[test]
#[should_panic(expected = "same-stuff")]
fn two_same_stuff_stacks_on_one_holder_panics() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "a", "spark", 1);
    hold_stack(&mut world, "giver", "b", "spark", 1);
    world.assert_invariants();
}

/// Definition of done: quantity 0 is unrepresentable.
#[test]
#[should_panic(expected = "quantity 0")]
fn a_zero_quantity_stack_panics() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "z", "spark", 0);
    world.assert_invariants();
}

/// Total quantity per kind is conserved by any sequence of offers/accepts/eats —
/// eaten units are the only sink (risk ledger's property test).
#[test]
fn any_sequence_conserves_quantity_per_kind() {
    use std::collections::BTreeMap;

    let mut world = World::new();
    world.add_character(character("a0", "A0", 0.0));
    world.add_character(character("a1", "A1", 1.0));
    world.add_character(character("a2", "A2", 2.0));
    hold_stack(&mut world, "a0", "s0", "spark", 4);
    hold_stack(&mut world, "a0", "h0", "herring", 3);
    hold_stack(&mut world, "a1", "s1", "spark", 2);
    hold_stack(&mut world, "a1", "h1", "herring", 2);
    hold_stack(&mut world, "a2", "s2", "spark", 5);
    hold_stack(&mut world, "a2", "h2", "herring", 1);
    let initial_spark = 11u32;
    let initial_herring = 6u32;

    let actors = ["a0", "a1", "a2"];
    let total_by_kind = |world: &World| -> BTreeMap<String, u32> {
        let mut totals = BTreeMap::new();
        for item in world.items.values() {
            *totals.entry(item.kind.as_str().to_string()).or_insert(0) += item.quantity;
        }
        totals
    };

    let mut rng: u64 = 0x0123_4567_89ab_cdef;
    let next = |rng: &mut u64| -> u64 {
        *rng = rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *rng >> 33
    };

    let mut eaten_spark = 0u32;
    let mut eaten_herring = 0u32;

    for _ in 0..400 {
        let holder = actors[(next(&mut rng) % 3) as usize];
        let held = world.characters[&actor(holder)].holds().to_vec();
        match next(&mut rng) % 3 {
            0 => {
                // offer a random slice to a random other actor
                if !held.is_empty() {
                    let id = &held[(next(&mut rng) % held.len() as u64) as usize];
                    let stack_q = world.items[id].quantity;
                    let quantity = 1 + (next(&mut rng) % stack_q as u64) as u32;
                    let target = actors[(next(&mut rng) % 3) as usize];
                    if target != holder {
                        let _ = apply_action(
                            &mut world,
                            &actor(holder),
                            "offer_item",
                            &json!({"item_id": id.as_str(), "target": target, "quantity": quantity}),
                        );
                    }
                }
            }
            1 => {
                // accept a random live offer, as its target
                let offers: Vec<(String, Option<String>)> = world
                    .offers
                    .values()
                    .map(|offer| {
                        (
                            offer.item_id.as_str().to_string(),
                            offer.target_id.as_ref().map(|id| id.as_str().to_string()),
                        )
                    })
                    .collect();
                if !offers.is_empty() {
                    let (id, target) = &offers[(next(&mut rng) % offers.len() as u64) as usize];
                    let taker = target
                        .clone()
                        .unwrap_or_else(|| actors[(next(&mut rng) % 3) as usize].to_string());
                    let _ = apply_action(
                        &mut world,
                        &actor(&taker),
                        "accept_offered_item",
                        &json!({"item_id": id}),
                    );
                }
            }
            _ => {
                // eat something edible
                if !held.is_empty() {
                    let id = held[(next(&mut rng) % held.len() as u64) as usize].clone();
                    let is_food = world.item_catalog.is_edible(&world.items[&id]);
                    if is_food {
                        let kind = world.items[&id].kind.as_str().to_string();
                        if apply_action(
                            &mut world,
                            &actor(holder),
                            "eat",
                            &json!({"item_id": id.as_str()}),
                        )
                        .is_ok()
                        {
                            if kind == "spark" {
                                eaten_spark += 1;
                            } else if kind == "herring" {
                                eaten_herring += 1;
                            }
                        }
                    }
                }
            }
        }

        world.assert_invariants();
        let totals = total_by_kind(&world);
        assert_eq!(
            totals.get("spark").copied().unwrap_or(0) + eaten_spark,
            initial_spark,
            "spark conservation"
        );
        assert_eq!(
            totals.get("herring").copied().unwrap_or(0) + eaten_herring,
            initial_herring,
            "herring conservation"
        );
    }
    // The run actually exercised eating (herring is the only edible kind here).
    assert!(eaten_herring > 0, "the sequence never ate");
}

/// The world-dump label counts a stack: `spark (c0prs) ×3`, single items plain.
#[test]
fn the_dump_label_counts_stacks() {
    let mut world = stack_world();
    hold_stack(&mut world, "giver", "c0prs", "spark", 3);
    hold_stack(&mut world, "receiver", "one", "herring", 1);
    assert_eq!(
        world.item_dump_label(&world.items[&item("c0prs")]),
        "spark (c0prs) ×3"
    );
    assert_eq!(
        world.item_dump_label(&world.items[&item("one")]),
        "herring (one)"
    );
}
