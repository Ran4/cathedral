//! `apply_action` and the eleven verbs (`sim.py:627-1019`).
//!
//! `args` stays `serde_json::Value` on purpose: both model output and protocol
//! data are untrusted, and a malformed shape must become an [`ActionError`],
//! never a panic. Python's object-identity actor check becomes a map lookup.
//!
//! Any error leaves the world unmodified — with the deliberate exception of the
//! offer "repair" paths ([`repair_and_fail`]), which delete a stale offer and
//! bump the public revision *before* failing.

use serde_json::{Map, Value};

use crate::{
    GO_TO_BUDGET_FACTOR, GO_TO_MIN_BUDGET_SECONDS, GOAL_MAX_CHARS, GOAL_NONE, HEARING_RADIUS_M,
    HUNGER_MAX, ITEM_INTERACTION_RADIUS_M, MEMORY_MAX_CHARS, OFFER_LAPSE_RADIUS_M,
    PLAYER_SPEECH_MAX_CHARS, POCKET_PLAIN_SIGHT_RADIUS_M, POCKET_SLOT_CAPACITY, THIRST_MAX,
    WALK_SPEED_MPS,
    character::{
        ActiveGesture, BodySlot, GutEntry, IntentTarget, PocketedUnit, RoundEdit, TravelIntent,
    },
    error::{ActionError, ActionErrorCode},
    event::DomainEvent,
    gesture::{GestureKind, GestureSpec, GestureTarget},
    ids::{ActorId, ItemId, PlaceId},
    inventory::{InventoryError, InventoryErrorCode},
    item::{
        CONDITION_METADATA_KEY, CONDITION_POOPSTAINED, CONDITION_WET, Item, ItemKind, ItemSize,
        POOP_KIND,
    },
    math::Vec3,
    offer::Offer,
    perception::{cap_first, emit_sound, identify_ids},
    pyfmt::{py_repr, py_repr_str, py_strip},
    world::World,
};

/// Validate and apply one action, returning its terminal transcript line.
pub fn apply_action(
    world: &mut World,
    actor_id: &ActorId,
    verb: &str,
    args: &Value,
) -> Result<String, ActionError> {
    apply_action_at(world, actor_id, verb, args, None)
}

/// [`apply_action`] with the actor's position temporarily frozen at
/// `position_override`.
///
/// Speech-to-text can finish after newer spatial updates have landed; the
/// utterance is then applied at the position it was recorded at, without
/// rewinding the authoritative position (`server.py:1701-1717`).
pub fn apply_action_at(
    world: &mut World,
    actor_id: &ActorId,
    verb: &str,
    args: &Value,
    position_override: Option<Vec3>,
) -> Result<String, ActionError> {
    if !world.is_present(actor_id) {
        return Err(ActionError::new(
            ActionErrorCode::UnknownActor,
            "acting character is not part of this world",
        ));
    }
    let Some(frozen) = position_override else {
        return dispatch(world, actor_id, verb, args);
    };
    let actor = world.characters.get_mut(actor_id).expect("checked above");
    let original = std::mem::replace(&mut actor.state.position_m, frozen);
    let result = dispatch(world, actor_id, verb, args);
    world
        .characters
        .get_mut(actor_id)
        .expect("characters are never removed")
        .state
        .position_m = original;
    result
}

fn inventory_action_error(error: InventoryError) -> ActionError {
    let code = match error.code {
        InventoryErrorCode::UnknownActor => ActionErrorCode::UnknownActor,
        InventoryErrorCode::UnknownItem => ActionErrorCode::UnknownItem,
        InventoryErrorCode::NotOwner => ActionErrorCode::NotOwner,
        InventoryErrorCode::BadQuantity => ActionErrorCode::BadQuantity,
        InventoryErrorCode::ItemCommitted => ActionErrorCode::ItemCommitted,
        InventoryErrorCode::OutputCapacityReserved => ActionErrorCode::OutputCapacityReserved,
        _ => ActionErrorCode::InvalidAction,
    };
    ActionError::new(code, error.message)
}

fn dispatch(
    world: &mut World,
    actor_id: &ActorId,
    verb: &str,
    args: &Value,
) -> Result<String, ActionError> {
    // A non-string verb is impossible here (the reply parser types it), which
    // is where Python's `invalid_action` check lived.
    let result = match verb {
        "wait" => wait(world, actor_id, args),
        "say" => say(world, actor_id, args),
        "offer_item" => offer_item(world, actor_id, args),
        "accept_offered_item" => accept_offered_item(world, actor_id, args),
        "decline_offer" => decline_offer(world, actor_id, args),
        "retract_offer" => retract_offer(world, actor_id, args),
        "eat" => eat(world, actor_id, args),
        "pocket_item" => pocket_item(world, actor_id, args),
        "retrieve_item" => retrieve_item(world, actor_id, args),
        "swallow" => swallow(world, actor_id, args),
        "spit" => spit(world, actor_id, args),
        "gargle" => gargle(world, actor_id, args),
        "expel" => expel(world, actor_id, args),
        "set_goal" => set_goal(world, actor_id, args),
        "make_sound" => make_sound(world, actor_id, args),
        "gesture" => gesture(world, actor_id, args),
        "remember" => remember(world, actor_id, args),
        "forget" => forget(world, actor_id, args),
        "go_to" => go_to(world, actor_id, args),
        "stop" => stop(world, actor_id, args),
        "set_round" => set_round(world, actor_id, args),
        "tell_way" => tell_way(world, actor_id, args),
        "raise_notice" => raise_notice(world, actor_id, args),
        "settle_notice" => settle_notice(world, actor_id, args),
        // Checked last, after every verb has had its chance to match.
        unknown => Err(ActionError::new(
            ActionErrorCode::UnknownVerb,
            format!("unknown verb: {unknown}"),
        )),
    };
    // "dance loops until the actor's next non-`wait` action" (`npc_bodies.md`
    // §7): any successful non-`wait` action ends a running loop. `gesture` is
    // excepted here because it manages its own `active_gesture` — `dance` sets
    // it, every other kind clears it — so a re-issued `dance` is not cut short
    // by its own success.
    if result.is_ok() && verb != "wait" && verb != "gesture" {
        set_active_gesture(world, actor_id, None);
    }
    result
}

// ---------------------------------------------------------------- validators

/// `_args` (`sim.py:527-547`): the value must be a JSON object; a missing
/// required key is reported before an unknown one, each naming the
/// alphabetically-first offender.
fn args_object<'a>(
    value: &'a Value,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a Map<String, Value>, ActionError> {
    let Value::Object(map) = value else {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            "action arguments must be a JSON object",
        ));
    };
    let mut missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|key| !map.contains_key(*key))
        .collect();
    missing.sort_unstable();
    if let Some(key) = missing.first() {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            format!("missing required argument: {key}"),
        ));
    }
    let mut unknown: Vec<&str> = map
        .keys()
        .map(String::as_str)
        .filter(|key| {
            !required.iter().any(|known| known == key) && !optional.iter().any(|known| known == key)
        })
        .collect();
    unknown.sort_unstable();
    if let Some(key) = unknown.first() {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            format!("unknown argument: {key}"),
        ));
    }
    Ok(map)
}

/// An optional argument: omitted and explicitly-null are the same thing.
fn optional_arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|value| !value.is_null())
}

fn parse_actor_id(value: &Value, field_name: &str) -> Result<ActorId, ActionError> {
    value
        .as_str()
        .and_then(|text| ActorId::new(text).ok())
        .ok_or_else(|| {
            ActionError::new(
                ActionErrorCode::InvalidArguments,
                format!("{field_name} must be a non-empty character id"),
            )
        })
}

fn parse_item_id(value: &Value) -> Result<ItemId, ActionError> {
    value
        .as_str()
        .and_then(|text| ItemId::new(text).ok())
        .ok_or_else(|| {
            ActionError::new(
                ActionErrorCode::InvalidArguments,
                "item_id must be a non-empty item id",
            )
        })
}

/// `_text` (`sim.py:499-524`): a string, stripped, non-empty, at most
/// `max_chars` Unicode scalar values (D11), free of control characters. The
/// stripped text is what every downstream percept and event carries.
///
/// The strip is [`py_strip`], not `str::trim`: Python's whitespace set also
/// contains the C0 separators `\x1c..\x1f`. `str::trim` would leave them in
/// place, where the length check counts them and the control-character check
/// then *rejects* text CPython strips clean and accepts.
fn parse_text(value: &Value, field_name: &str, max_chars: usize) -> Result<String, ActionError> {
    let Some(text) = value.as_str() else {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            format!("{field_name} must be a string"),
        ));
    };
    let text = py_strip(text);
    if text.is_empty() {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            format!("{field_name} must not be empty"),
        ));
    }
    if text.chars().count() > max_chars {
        return Err(ActionError::new(
            ActionErrorCode::TextTooLong,
            format!("{field_name} is too long (maximum {max_chars} characters)"),
        ));
    }
    if text.chars().any(is_forbidden_control) {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            format!("{field_name} contains control characters"),
        ));
    }
    Ok(text.to_string())
}

/// Python's exact ranges: below 0x20 except `\n` and `\t`, plus 0x7F..=0x9F.
fn is_forbidden_control(character: char) -> bool {
    (character < '\u{20}' && character != '\n' && character != '\t')
        || ('\u{7f}'..='\u{9f}').contains(&character)
}

/// Python's `{id!r}` for an id that reaches an error message.
///
/// Not `'{id}'`: an id carrying an apostrophe makes CPython switch to double
/// quotes (`repr("it's")` is `"it's"`), and these messages become model-visible
/// `system:` inbox lines, i.e. prompt bytes.
fn repr_id(id: &str) -> String {
    py_repr_str(id)
}

/// Python's `f"{value:g}"` for the radii in messages: `20.0` renders as `20`.
fn format_g(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// A self-introduction is ordinary speech, not a special verb. Match the
/// speaker's full name case-insensitively at word boundaries so "Nan" does not
/// match "nanny".
fn text_mentions_name(text: &str, name: &str) -> bool {
    let text = text.to_lowercase();
    let name = name.to_lowercase();
    text.match_indices(&name).any(|(start, matched)| {
        let end = start + matched.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
    })
}

// ------------------------------------------------------------------- helpers

fn nearby(world: &World, actor_id: &ActorId, radius: f64) -> Vec<ActorId> {
    let origin = world.characters[actor_id].position_m();
    world.characters_within(origin, radius, Some(actor_id))
}

fn require_interaction_range(
    world: &World,
    actor_id: &ActorId,
    other_id: &ActorId,
) -> Result<(), ActionError> {
    let actor = &world.characters[actor_id];
    let other = &world.characters[other_id];
    if actor.position_m().distance_squared(other.position_m())
        > ITEM_INTERACTION_RADIUS_M * ITEM_INTERACTION_RADIUS_M
    {
        return Err(ActionError::new(
            ActionErrorCode::OutOfRange,
            format!(
                "{} is more than {} metres away",
                identify_ids(world, actor_id, other_id),
                format_g(ITEM_INTERACTION_RADIUS_M)
            ),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn world_event(
    world: &mut World,
    kind: &str,
    actor_id: &ActorId,
    target_id: Option<ActorId>,
    item_id: Option<ItemId>,
    quantity: u32,
    recipients: Vec<ActorId>,
) -> i64 {
    let position = world.characters[actor_id].position_m();
    world.emit(DomainEvent::world_event(
        kind,
        actor_id.clone(),
        target_id,
        item_id,
        quantity,
        position,
        recipients,
    ))
}

/// "a herring" for a single unit, "3 sparks" for many — the noun phrase (with
/// article) that the counted percept lines embed where the old wording said
/// "a {name}" (`01_items_and_stacks.md` §4).
fn counted_phrase(world: &World, item: &crate::item::Item, quantity: u32) -> String {
    if quantity <= 1 {
        let name = world.item_catalog.display_name(item);
        format!("{} {name}", indefinite_article(&name))
    } else {
        format!("{quantity} {}", world.item_catalog.display_plural(item))
    }
}

/// "a" or "an". Spelling, not phonetics — the catalog's vowel-initial names
/// (apple, ale, onion, egg) are all honest cases, and these lines are read by a
/// language model that will happily mirror "a apple" back at the player.
fn indefinite_article(name: &str) -> &'static str {
    match name.chars().next() {
        Some(first) if "aeiou".contains(first.to_ascii_lowercase()) => "an",
        _ => "a",
    }
}

/// The bare counted noun — "herring" / "3 sparks" — for the "the {noun}" and
/// "the offered {noun}" positions (accept/decline/retract), where no article is
/// wanted. `n <= 1` is the singular display name, byte-identical to today.
fn counted_noun(world: &World, item: &crate::item::Item, quantity: u32) -> String {
    if quantity <= 1 {
        world.item_catalog.display_name(item)
    } else {
        format!("{quantity} {}", world.item_catalog.display_plural(item))
    }
}

/// Parse `offer_item`'s optional `quantity` argument: an integer `1..=stack`, or
/// the whole stack when omitted.
fn parse_offer_quantity(
    args: &Map<String, Value>,
    stack_quantity: u32,
) -> Result<u32, ActionError> {
    let Some(value) = optional_arg(args, "quantity") else {
        return Ok(stack_quantity);
    };
    match value
        .as_u64()
        .and_then(|raw| u32::try_from(raw).ok())
        .filter(|quantity| *quantity >= 1 && *quantity <= stack_quantity)
    {
        Some(quantity) => Ok(quantity),
        None => Err(ActionError::new(
            ActionErrorCode::BadQuantity,
            format!(
                "quantity must be a whole number between 1 and {stack_quantity} (you hold {stack_quantity})"
            ),
        )),
    }
}

/// A failed action that still mutates: the offer was stale, so it is deleted
/// and the public revision bumped *before* the error is returned. Callers must
/// keep using this instead of an early `?` — the mutation is the point
/// (`sim.py:808-822, 867-876, 913-917`).
fn repair_and_fail(
    world: &mut World,
    item_id: &ItemId,
    code: ActionErrorCode,
    message: impl Into<String>,
) -> ActionError {
    world.offers.remove(item_id);
    world.touch_public_state();
    ActionError::new(code, message)
}

fn deliver(world: &mut World, lines: Vec<(ActorId, String)>, percept: bool) {
    for (recipient, text) in lines {
        let character = world
            .characters
            .get_mut(&recipient)
            .expect("recipients come from the world");
        if percept {
            character.notify_percept(text);
        } else {
            character.notify(text);
        }
    }
}

// --------------------------------------------------------------------- verbs

fn wait(world: &World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    args_object(args, &[], &[])?;
    Ok(format!("{} waits", world.characters[actor_id].name()))
}

fn say(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["text"], &["target"])?;
    let text = parse_text(&parsed["text"], "text", PLAYER_SPEECH_MAX_CHARS)?;
    let target_value = optional_arg(parsed, "target");

    let origin = world.characters[actor_id].position_m();
    let hearers = world.characters_within(origin, HEARING_RADIUS_M, Some(actor_id));

    let mut target: Option<ActorId> = None;
    if let Some(value) = target_value {
        let target_id = parse_actor_id(value, "target")?;
        if !world.is_present(&target_id) {
            return Err(ActionError::new(
                ActionErrorCode::UnknownTarget,
                format!("there is nobody with id {}", repr_id(target_id.as_str())),
            ));
        }
        if target_id == *actor_id {
            return Err(ActionError::new(
                ActionErrorCode::SelfTarget,
                "you cannot speak to yourself",
            ));
        }
        // An explicit target that is bad or distant is an error; it NEVER
        // falls back to broadcast.
        if !hearers.contains(&target_id) {
            return Err(ActionError::new(
                ActionErrorCode::OutOfRange,
                format!(
                    "{} is more than {} metres away",
                    identify_ids(world, actor_id, &target_id),
                    format_g(HEARING_RADIUS_M)
                ),
            ));
        }
        target = Some(target_id);
    }

    // Muffled speech (`extra_pockets.md` M1): a cheeked coin marks how the
    // words *land*, never what they say — flavour-only, because the model
    // already works hard to read past STT noise and garbled text would only
    // teach it to distrust the transcript. The speaker's own line, the
    // `DomainEvent` the host reads and the transcript are all untouched; an
    // empty mouth changes zero bytes.
    let muffle = if world.characters[actor_id].pocketed_in_slot(BodySlot::Mouth) > 0 {
        " through a full mouth"
    } else {
        ""
    };

    let line = if let Some(target_id) = &target {
        let own = format!(
            "You said to {}: \"{text}\"",
            identify_ids(world, actor_id, target_id)
        );
        world
            .characters
            .get_mut(actor_id)
            .expect("the speaker is in the world")
            .remember_percept(own);
        let percepts = hearers
            .iter()
            .map(|recipient| {
                let speaker = cap_first(&identify_ids(world, recipient, actor_id));
                let line = if recipient == target_id {
                    format!("{speaker} said to you{muffle}: \"{text}\"")
                } else {
                    format!(
                        "{speaker} said to {}{muffle}: \"{text}\"",
                        identify_ids(world, recipient, target_id)
                    )
                };
                (recipient.clone(), line)
            })
            .collect();
        deliver(world, percepts, true);
        format!(
            "{} -> {}: \"{text}\"",
            world.characters[actor_id].name(),
            world.characters[target_id].name()
        )
    } else {
        world
            .characters
            .get_mut(actor_id)
            .expect("the speaker is in the world")
            .remember_percept(format!("You said aloud: \"{text}\""));
        let percepts = hearers
            .iter()
            .map(|recipient| {
                let speaker = cap_first(&identify_ids(world, recipient, actor_id));
                (
                    recipient.clone(),
                    format!("{speaker} said{muffle}: \"{text}\""),
                )
            })
            .collect();
        deliver(world, percepts, true);
        format!("{} (aloud): \"{text}\"", world.characters[actor_id].name())
    };

    let speaker_name = world.characters[actor_id].name().to_string();
    if text_mentions_name(&text, &speaker_name) {
        let mut learned = false;
        for hearer in &hearers {
            let observer = world
                .characters
                .get_mut(hearer)
                .expect("hearers come from the world");
            if !observer.control().is_llm() {
                learned |= observer.state.knows.insert(actor_id.clone());
            }
        }
        if learned {
            world.touch_public_state();
        }
    }

    world.emit(DomainEvent::speech(
        actor_id.clone(),
        target,
        text,
        origin,
        hearers,
    ));
    Ok(line)
}

fn offer_item(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &["target", "quantity"])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    if !world.characters[actor_id].holds().contains(&item_id) {
        return Err(ActionError::new(
            ActionErrorCode::NotOwner,
            format!(
                "you hold no item with id {} (item_id takes an id, not a name)",
                repr_id(item_id.as_str())
            ),
        ));
    }
    let Some(item) = world.items.get(&item_id).cloned() else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownItem,
            format!("there is no item with id {}", repr_id(item_id.as_str())),
        ));
    };

    // Optional quantity: an integer `1..=stack`, defaulting to the whole stack.
    // The offered portion stays in the giver's stack until accepted.
    let quantity = parse_offer_quantity(parsed, item.quantity)?;
    let available = world.available_for_offer_replacement(&item_id);
    if quantity > available {
        return Err(ActionError::new(
            ActionErrorCode::ItemCommitted,
            format!(
                "only {available} units are free; retract or replace an existing offer first"
            ),
        ));
    }
    let held_phrase = counted_phrase(world, &item, quantity);
    let offered_noun = counted_noun(world, &item, quantity);

    let mut target: Option<ActorId> = None;
    if let Some(value) = optional_arg(parsed, "target") {
        let target_id = parse_actor_id(value, "target")?;
        if !world.is_present(&target_id) {
            return Err(ActionError::new(
                ActionErrorCode::UnknownTarget,
                format!("there is nobody with id {}", repr_id(target_id.as_str())),
            ));
        }
        if target_id == *actor_id {
            return Err(ActionError::new(
                ActionErrorCode::SelfTarget,
                "you cannot offer an item to yourself",
            ));
        }
        require_interaction_range(world, actor_id, &target_id)?;
        target = Some(target_id);
    }

    let old_offer = world.offers.get(&item_id).cloned();
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);

    // Re-offering to a different target displaces the old one: a nearby jilted
    // target is told and gets a structured retract_offer BEFORE the new offer.
    // Displaced-but-absent or out-of-range targets get neither.
    if let Some(old) = old_offer.as_ref()
        && let Some(old_target) = old.target_id.clone()
        && Some(&old_target) != target.as_ref()
        && world.is_present(&old_target)
        && hearers.contains(&old_target)
    {
        let old_noun = counted_noun(world, &item, old.quantity);
        let line = format!(
            "{} withdrew the offered {old_noun} (id {item_id})",
            cap_first(&identify_ids(world, &old_target, actor_id))
        );
        deliver(world, vec![(old_target.clone(), line)], false);
        world_event(
            world,
            "retract_offer",
            actor_id,
            Some(old_target.clone()),
            Some(item_id.clone()),
            old.quantity,
            vec![old_target],
        );
    }

    let line = if let Some(target_id) = &target {
        let lines = hearers
            .iter()
            .map(|observer| {
                let giver = cap_first(&identify_ids(world, observer, actor_id));
                let line = if observer == target_id {
                    format!("{giver} held out {held_phrase} (id {item_id}) to you")
                } else {
                    format!(
                        "{giver} offered {held_phrase} to {}",
                        identify_ids(world, observer, target_id)
                    )
                };
                (observer.clone(), line)
            })
            .collect();
        deliver(world, lines, false);
        format!(
            "{} offers the {offered_noun} to {}",
            world.characters[actor_id].name(),
            world.characters[target_id].name()
        )
    } else {
        let lines = hearers
            .iter()
            .map(|observer| {
                let giver = cap_first(&identify_ids(world, observer, actor_id));
                (
                    observer.clone(),
                    format!(
                        "{giver} held out {held_phrase} (id {item_id}) to anyone who wanted it"
                    ),
                )
            })
            .collect();
        deliver(world, lines, false);
        format!(
            "{} offers the {offered_noun} to anyone nearby",
            world.characters[actor_id].name()
        )
    };

    // The item does NOT move: it stays in the giver's holds until accepted.
    let sequence = world_event(
        world,
        "offer_item",
        actor_id,
        target.clone(),
        Some(item_id.clone()),
        quantity,
        hearers,
    );
    world.offers.insert(
        item_id.clone(),
        Offer {
            item_id,
            giver_id: actor_id.clone(),
            target_id: target,
            created_seq: sequence,
            quantity,
        },
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(line)
}

fn accept_offered_item(
    world: &mut World,
    actor_id: &ActorId,
    args: &Value,
) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;

    let Some(offer) = world.offers.get(&item_id).cloned() else {
        return Err(ActionError::new(
            ActionErrorCode::NoOffer,
            format!(
                "nobody is offering you an item with id {}",
                repr_id(item_id.as_str())
            ),
        ));
    };
    if offer.giver_id == *actor_id {
        return Err(ActionError::new(
            ActionErrorCode::OwnOffer,
            "that is your own offer (retract_offer to withdraw it)",
        ));
    }
    if offer
        .target_id
        .as_ref()
        .is_some_and(|target| target != actor_id)
    {
        return Err(ActionError::new(
            ActionErrorCode::NotOfferTarget,
            format!(
                "nobody is offering you an item with id {}",
                repr_id(item_id.as_str())
            ),
        ));
    }
    let giver_id = offer.giver_id.clone();
    if !world.is_present(&giver_id) {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::NoOffer,
            "the person offering it no longer exists",
        ));
    }
    // The offer survives a failed range check.
    require_interaction_range(world, actor_id, &giver_id)?;
    if !world.characters[&giver_id].holds().contains(&item_id) {
        let message = format!(
            "{} no longer holds that item",
            identify_ids(world, actor_id, &giver_id)
        );
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            message,
        ));
    }
    let Some(item) = world.items.get(&item_id).cloned() else {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "the offered item no longer exists",
        ));
    };

    // The offered quantity may have shrunk below what was promised (the giver
    // ate or sold some meanwhile) — a stale offer, repaired like any other.
    let quantity = offer.quantity;
    if item.quantity < quantity {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "they no longer hold that many",
        ));
    }

    let accepted_noun = counted_noun(world, &item, quantity);
    let took_phrase = counted_phrase(world, &item, quantity);

    world
        .transfer_offered_item(
            &giver_id,
            actor_id,
            &item_id,
            quantity,
            &format!("offer_accept:{}:{}", world.event_sequence + 1, item_id),
        )
        .map_err(inventory_action_error)?;
    world.offers.remove(&item_id);

    // The giver is guaranteed inside this radius, being at most 4 m away.
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    let lines = hearers
        .iter()
        .map(|observer| {
            let taker = cap_first(&identify_ids(world, observer, actor_id));
            let line = if observer == &giver_id {
                format!("{taker} accepted the {accepted_noun} (id {item_id}) you offered")
            } else {
                format!(
                    "{taker} took {took_phrase} from {}",
                    identify_ids(world, observer, &giver_id)
                )
            };
            (observer.clone(), line)
        })
        .collect();
    deliver(world, lines, false);
    world_event(
        world,
        "accept_offered_item",
        actor_id,
        Some(giver_id.clone()),
        Some(item_id.clone()),
        quantity,
        hearers,
    );
    // Presentation keeps its long-standing accept event, while economic
    // observability gets the purpose-neutral transfer edge required to
    // distinguish negotiated gifts/trades from catalog-price `sale`s.
    world_event(
        world,
        "item_transfer",
        &giver_id,
        Some(actor_id.clone()),
        Some(item_id.clone()),
        quantity,
        Vec::new(),
    );
    // law_and_order.md M3.5: a transfer is a transfer. It clears a word only in
    // the two cases no verb could reach — the named taking handed back, and the
    // player as the wronged party — and otherwise merely *offers* itself as
    // restitution to whoever took it, who decides with `settle_notice`.
    for notice in crate::notices::settle_on_return(world, &giver_id, actor_id, &item_id) {
        announce_settled(world, &notice, &giver_id, "restitution made");
    }
    offer_restitution(world, &giver_id, actor_id);
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} takes the {accepted_noun} from {}",
        world.characters[actor_id].name(),
        world.characters[&giver_id].name()
    ))
}

fn decline_offer(
    world: &mut World,
    actor_id: &ActorId,
    args: &Value,
) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;

    let offer = world.offers.get(&item_id).cloned();
    // Checked before the not-yours test: a broadcast offer is simply ignored.
    if offer
        .as_ref()
        .is_some_and(|offer| offer.target_id.is_none())
    {
        return Err(ActionError::new(
            ActionErrorCode::BroadcastCannotDecline,
            "that offer is open to anyone, not addressed to you - just ignore it",
        ));
    }
    let Some(offer) = offer.filter(|offer| offer.target_id.as_ref() == Some(actor_id)) else {
        return Err(ActionError::new(
            ActionErrorCode::NoOffer,
            format!(
                "nobody is offering you an item with id {}",
                repr_id(item_id.as_str())
            ),
        ));
    };
    let giver_id = offer.giver_id;
    if !world.is_present(&giver_id) {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::NoOffer,
            "the person offering it no longer exists",
        ));
    }
    // The offer survives a failed range check.
    require_interaction_range(world, actor_id, &giver_id)?;
    let Some(item) = world.items.get(&item_id).cloned() else {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "the offered item no longer exists",
        ));
    };
    let declined_noun = counted_noun(world, &item, offer.quantity);
    let declined_phrase = counted_phrase(world, &item, offer.quantity);

    // The giver keeps the item.
    world.offers.remove(&item_id);
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    let lines = hearers
        .iter()
        .map(|observer| {
            let decliner = cap_first(&identify_ids(world, observer, actor_id));
            let line = if observer == &giver_id {
                format!("{decliner} declined the {declined_noun} (id {item_id}) you offered")
            } else {
                format!(
                    "{decliner} declined {declined_phrase} from {}",
                    identify_ids(world, observer, &giver_id)
                )
            };
            (observer.clone(), line)
        })
        .collect();
    deliver(world, lines, false);
    world_event(
        world,
        "decline_offer",
        actor_id,
        Some(giver_id.clone()),
        Some(item_id),
        offer.quantity,
        hearers,
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} declines the {declined_noun} from {}",
        world.characters[actor_id].name(),
        world.characters[&giver_id].name()
    ))
}

fn retract_offer(
    world: &mut World,
    actor_id: &ActorId,
    args: &Value,
) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;

    let Some(offer) = world
        .offers
        .get(&item_id)
        .cloned()
        .filter(|offer| offer.giver_id == *actor_id)
    else {
        return Err(ActionError::new(
            ActionErrorCode::NoOffer,
            format!(
                "you have no pending offer of an item with id {}",
                repr_id(item_id.as_str())
            ),
        ));
    };
    let Some(item) = world.items.get(&item_id).cloned() else {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "the offered item no longer exists",
        ));
    };
    let withdrawn_noun = counted_noun(world, &item, offer.quantity);

    // No proximity requirement at all — but a distant target gets no percept.
    world.offers.remove(&item_id);
    let mut recipients: Vec<ActorId> = Vec::new();
    if let Some(target_id) = &offer.target_id
        && world.is_present(target_id)
        && let Some(target) = world.characters.get(target_id)
        && world.characters[actor_id]
            .position_m()
            .distance_squared(target.position_m())
            <= HEARING_RADIUS_M * HEARING_RADIUS_M
    {
        let line = format!(
            "{} withdrew the offered {withdrawn_noun} (id {item_id})",
            cap_first(&identify_ids(world, target_id, actor_id))
        );
        deliver(world, vec![(target_id.clone(), line)], false);
        recipients.push(target_id.clone());
    }
    // target_id is reported even when the target was never notified or is gone.
    world_event(
        world,
        "retract_offer",
        actor_id,
        offer.target_id,
        Some(item_id),
        offer.quantity,
        recipients,
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} retracts the offer of the {withdrawn_noun}",
        world.characters[actor_id].name()
    ))
}

/// Lapse every targeted offer whose two parties have drifted more than
/// [`OFFER_LAPSE_RADIUS_M`] apart, and say so to both.
///
/// The one offer resolution nobody *acts*: it is deliberately not a verb, so no
/// reply can ask for it and no model has to remember to. Until it existed an
/// unanswered offer stood forever — the giver's arm out, the offered units
/// committed against eating or selling — and walking away made it *unanswerable*
/// rather than resolving it, because an accept and a decline both need 4 m. A
/// promise neither party can see or refuse is not a promise (the same reasoning
/// that expires an offer at the city gate, `round.rs`).
///
/// Broadcast offers are left alone: they name nobody to drift away from, and
/// they travel with the giver, so anyone beside them can always still take one.
///
/// Pure, like everything else in the sim: distance only, no clock. Returns the
/// items whose offers lapsed.
pub fn lapse_distant_offers(world: &mut World) -> Vec<ItemId> {
    let lapsing: Vec<(ItemId, ActorId, ActorId, u32)> = world
        .offers
        .values()
        .filter_map(|offer| {
            let target_id = offer.target_id.clone()?;
            // A departure already has its own expiry rules; this one only
            // judges two people who are both here.
            if !world.is_present(&offer.giver_id) || !world.is_present(&target_id) {
                return None;
            }
            let giver = world.characters.get(&offer.giver_id)?;
            let target = world.characters.get(&target_id)?;
            let apart = giver.position_m().distance_squared(target.position_m());
            (apart > OFFER_LAPSE_RADIUS_M * OFFER_LAPSE_RADIUS_M).then(|| {
                (
                    offer.item_id.clone(),
                    offer.giver_id.clone(),
                    target_id,
                    offer.quantity,
                )
            })
        })
        .collect();
    if lapsing.is_empty() {
        return Vec::new();
    }

    let mut lapsed: Vec<ItemId> = Vec::new();
    for (item_id, giver_id, target_id, quantity) in lapsing {
        // An offer of an item that has left the world shows on no sheet, so
        // nobody is told it ended — the same silence `repair_and_fail` keeps.
        let Some(item) = world.items.get(&item_id).cloned() else {
            world.offers.remove(&item_id);
            lapsed.push(item_id);
            continue;
        };
        let noun = counted_noun(world, &item, quantity);
        world.offers.remove(&item_id);
        let giver_line = format!(
            "{} is too far away now; the {noun} (id {item_id}) you held out is yours again",
            cap_first(&identify_ids(world, &giver_id, &target_id))
        );
        let target_line = format!(
            "You are too far from {} now; the {noun} (id {item_id}) they held out for you is no longer on offer",
            identify_ids(world, &target_id, &giver_id)
        );
        deliver(
            world,
            vec![
                (giver_id.clone(), giver_line),
                (target_id.clone(), target_line),
            ],
            false,
        );
        // Both parties, and nobody else: at this distance there is no bystander
        // who can see them both, so there is no third party to tell.
        world_event(
            world,
            "lapse_offer",
            &giver_id,
            Some(target_id.clone()),
            Some(item_id.clone()),
            quantity,
            vec![giver_id.clone(), target_id],
        );
        lapsed.push(item_id);
    }
    world.touch_public_state();
    world.assert_invariants();
    lapsed
}

fn eat(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    if !world.characters[actor_id].holds().contains(&item_id) {
        return Err(ActionError::new(
            ActionErrorCode::NotOwner,
            format!(
                "you hold no item with id {} (item_id takes an id, not a name)",
                repr_id(item_id.as_str())
            ),
        ));
    }
    let Some(item) = world.items.get(&item_id).cloned() else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownItem,
            format!("there is no item with id {}", repr_id(item_id.as_str())),
        ));
    };
    // Eating a non-food kind fails; today's `eat` allowed anything.
    if !world.item_catalog.is_edible(&item) {
        return Err(ActionError::new(
            ActionErrorCode::NotEdible,
            format!(
                "a {} is not food",
                world.item_catalog.display_name(&item)
            ),
        ));
    }
    // The satiety this unit restores, applied to the eater's hunger below
    // (`features/food_and_items/03_hunger.md` §2). An ad-hoc test kind that is
    // edible-by-tolerance carries no catalog satiety and simply feeds nothing.
    let satiety = world.item_catalog.satiety(&item).unwrap_or(0);
    // Drinks (thirst > satiety in the catalog) run through the same verb and
    // consume rule; only the narrated verb and the thirst refill differ.
    let quench = world.item_catalog.thirst_quench(&item).unwrap_or(0);
    let is_drink = world.item_catalog.is_drink(&item);
    // `eat` consumes exactly one unit, so the wording is always singular.
    let eaten_phrase = counted_phrase(world, &item, 1);
    let eaten_noun = counted_noun(world, &item, 1);

    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);

    // Eating is an ordinary owner-side inventory operation. It may consume an
    // uncommitted unit, but it never silently cancels an offer or transform
    // promise; the actor must explicitly retract/replace the offer first.
    world
        .consume_item_quantity(actor_id, &item_id, 1)
        .map_err(inventory_action_error)?;

    // The gauge: satiety lifts hunger toward full, and the eater's next sheet
    // simply stops calling them hungry — the loop closes with no memory hygiene
    // (`03_hunger.md` §5). Since `extra_pockets.md` M1 the player eats through
    // this same verb (`EngineCommand::PlayerEat`), so the write is not an
    // enrolled townsperson's alone.
    if let Some(eater) = world.characters.get_mut(actor_id) {
        let hunger = &mut eater.state.needs.hunger;
        *hunger = (*hunger + f64::from(satiety)).min(HUNGER_MAX);
        let thirst = &mut eater.state.needs.thirst;
        *thirst = (*thirst + f64::from(quench)).min(THIRST_MAX);
    }
    // The gut starts its clock on the meal (`extra_pockets.md` M3); a clock-less
    // world queues nothing at all.
    queue_gut(world, actor_id, None);

    let (past, present) = if is_drink {
        ("drank", "drinks")
    } else {
        ("ate", "eats")
    };
    let lines = hearers
        .iter()
        .map(|observer| {
            let eater = cap_first(&identify_ids(world, observer, actor_id));
            (observer.clone(), format!("{eater} {past} {eaten_phrase}"))
        })
        .collect();
    deliver(world, lines, false);
    world_event(world, "eat", actor_id, None, Some(item_id), 1, hearers);
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} {present} the {eaten_noun}",
        world.characters[actor_id].name()
    ))
}

// --------------------------------------------------- body pockets and the gut

/// The item a pocket verb names: held by the actor, and present in the world.
/// The two error wordings are `eat`'s, verbatim — the model learns one sentence
/// for "that is not an id you hold".
fn held_item(world: &World, actor_id: &ActorId, item_id: &ItemId) -> Result<Item, ActionError> {
    if !world.characters[actor_id].holds().contains(item_id) {
        return Err(ActionError::new(
            ActionErrorCode::NotOwner,
            format!(
                "you hold no item with id {} (item_id takes an id, not a name)",
                repr_id(item_id.as_str())
            ),
        ));
    }
    world.items.get(item_id).cloned().ok_or_else(|| {
        ActionError::new(
            ActionErrorCode::UnknownItem,
            format!("there is no item with id {}", repr_id(item_id.as_str())),
        )
    })
}

/// The mouth-slot entry a `swallow`/`spit`/`gargle` names, or the error that
/// says why not: pocketed elsewhere is `wrong_slot`, pocketed nowhere is
/// `not_pocketed` (with the verb that would fix it).
fn require_mouthful(
    world: &World,
    actor_id: &ActorId,
    item_id: &ItemId,
) -> Result<Item, ActionError> {
    match world.characters[actor_id].pocket_slot_of(item_id) {
        Some(BodySlot::Mouth) => {}
        Some(_) => {
            let item = world.items.get(item_id);
            let noun = item.map_or_else(
                || "thing".to_string(),
                |item| counted_noun(world, item, 1),
            );
            return Err(ActionError::new(
                ActionErrorCode::WrongSlot,
                format!("the {noun} is not in your mouth"),
            ));
        }
        None => {
            return Err(ActionError::new(
                ActionErrorCode::NotPocketed,
                format!(
                    "nothing with id {} rides in your mouth - pocket_item it there first",
                    repr_id(item_id.as_str())
                ),
            ));
        }
    }
    world.items.get(item_id).cloned().ok_or_else(|| {
        ActionError::new(
            ActionErrorCode::UnknownItem,
            format!("there is no item with id {}", repr_id(item_id.as_str())),
        )
    })
}

/// Take the actor's first pocket entry in `slot` holding `item_id`, returning
/// where it sat so a failed follow-up can put it back.
fn take_pocket_entry(world: &mut World, actor_id: &ActorId, item_id: &ItemId) -> Option<(usize, PocketedUnit)> {
    let pockets = &mut world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .state
        .pockets;
    let index = pockets.iter().position(|unit| &unit.item_id == item_id)?;
    Some((index, pockets.remove(index)))
}

fn restore_pocket_entry(world: &mut World, actor_id: &ActorId, index: usize, unit: PocketedUnit) {
    let pockets = &mut world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .state
        .pockets;
    let index = index.min(pockets.len());
    pockets.insert(index, unit);
}

/// A deterministic operation key for the stack forks these verbs cause. The
/// event sequence is monotonic per world, so no two pocket operations collide.
fn pocket_operation_key(world: &World, verb: &str, actor_id: &ActorId) -> String {
    format!("{verb}:{actor_id}:{}", world.event_sequence)
}

/// The two-ring percept fan-out the pocket verbs share. Everyone in earshot
/// gets a line, but only the watchers standing at arm's length
/// ([`POCKET_PLAIN_SIGHT_RADIUS_M`]) get the one that names what moved and
/// which cavity took it: concealment is a matter of distance, not of magic.
/// Returns (everyone told, the ones who saw it plainly) — the first list is the
/// world event's recipients, the second its witnesses, which is what the engine
/// hands the next turn to.
fn deliver_pocket_percept(
    world: &mut World,
    actor_id: &ActorId,
    seen_near: &str,
    seen_far: &str,
) -> (Vec<ActorId>, Vec<ActorId>) {
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    let plain_sight = nearby(world, actor_id, POCKET_PLAIN_SIGHT_RADIUS_M);
    let lines: Vec<(ActorId, String)> = hearers
        .iter()
        .map(|observer| {
            let who = cap_first(&identify_ids(world, observer, actor_id));
            let what = match plain_sight.contains(observer) {
                true => seen_near,
                false => seen_far,
            };
            (observer.clone(), format!("{who} {what}"))
        })
        .collect();
    deliver(world, lines, true);
    (hearers, plain_sight)
}

/// A world event that also records who *saw* it, not merely who was in range —
/// the engine turns that list into an immediate reaction
/// ([`crate::Engine::nudge_pocket_witness`]). Distinct from `world_event` so
/// the twenty other call sites keep their exact shape.
fn world_event_witnessed(
    world: &mut World,
    kind: &str,
    actor_id: &ActorId,
    item_id: Option<ItemId>,
    recipients: Vec<ActorId>,
    witnesses: Vec<ActorId>,
) -> i64 {
    let position = world.characters[actor_id].position_m();
    let mut event = DomainEvent::world_event(
        kind,
        actor_id.clone(),
        None,
        item_id,
        1,
        position,
        recipients,
    );
    event.witness_ids = witnesses;
    world.emit(event)
}

/// `pocket_item` (`features/extra_pockets.md` M1/M2): hide one palmable
/// stack-unit in a body cavity. The unit stays in `holds` — it is a reservation
/// like an offer promise — but nobody can see it any more. What everyone *can*
/// see is the motion, and (deliberately) not what moved: a cheeked coin and a
/// cheeked draught read the same from two metres.
fn pocket_item(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id", "slot"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    let slot_value = &parsed["slot"];
    let Some(slot) = slot_value.as_str().and_then(BodySlot::from_wire) else {
        return Err(ActionError::new(
            ActionErrorCode::WrongSlot,
            format!("there is no body slot {}", py_repr(slot_value)),
        ));
    };
    let item = held_item(world, actor_id, &item_id)?;
    let actor = &world.characters[actor_id];
    if !actor.has_body_slot(slot) {
        return Err(ActionError::new(
            ActionErrorCode::WrongSlot,
            format!("you have no {}", slot.as_str()),
        ));
    }
    if world.item_catalog.size(&item) != ItemSize::Palmable {
        return Err(ActionError::new(
            ActionErrorCode::TooBig,
            format!(
                "a {} does not fit in your {}",
                world.item_catalog.display_name(&item),
                slot.as_str()
            ),
        ));
    }
    if actor.pocketed_in_slot(slot) >= POCKET_SLOT_CAPACITY {
        return Err(ActionError::new(
            ActionErrorCode::SlotFull,
            format!("your {} is full", slot.as_str()),
        ));
    }
    if world.uncommitted_quantity(&item_id) == 0 {
        return Err(ActionError::new(
            ActionErrorCode::ItemCommitted,
            "no unit of that stack is free; retract or replace the offer first",
        ));
    }

    // The metadata economy (M2), stamped before the unit is reserved — a
    // pocketed unit is committed, so the fork has to happen while it is free.
    // A mouth wets whatever is not already a drink or already conditioned; a
    // lower slot sharing with a stool stains. An empty lower slot leaves no
    // mark: what fouls a thing is the company it keeps, not the cavity.
    let is_drink = world.item_catalog.is_drink(&item);
    let condition = item.metadata.get(CONDITION_METADATA_KEY).map(String::as_str);
    let stamp = if slot == BodySlot::Mouth {
        (!is_drink && condition.is_none()).then_some(CONDITION_WET)
    } else {
        let shares_with_a_stool = world.characters[actor_id].pockets().iter().any(|unit| {
            unit.slot == slot
                && world
                    .items
                    .get(&unit.item_id)
                    .is_some_and(|other| other.kind.as_str() == POOP_KIND)
        });
        // A stool is never stamped (it *is* the stamp) and neither is anything
        // already fouled: either way the restamp forks a stack for no change.
        (shares_with_a_stool
            && item.kind.as_str() != POOP_KIND
            && condition != Some(CONDITION_POOPSTAINED))
        .then_some(CONDITION_POOPSTAINED)
    };
    let pocketed_id = match stamp {
        None => item_id.clone(),
        Some(value) => {
            let key = pocket_operation_key(world, "pocket", actor_id);
            world
                .restamp_metadata(actor_id, &item_id, 1, CONDITION_METADATA_KEY, value, &key)
                .map_err(inventory_action_error)?
        }
    };
    // Re-read: the restamp may have forked or merged the stack the unit rides.
    let pocketed = world.items[&pocketed_id].clone();
    let phrase = counted_phrase(world, &pocketed, 1);
    let noun = counted_noun(world, &pocketed, 1);
    // What a watcher saw go in was still clean: the stamp is what the cavity
    // does to it, not what was carried across the square.
    let seen_phrase = counted_phrase(world, &item, 1);

    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .state
        .pockets
        .push(PocketedUnit {
            slot,
            item_id: pocketed_id.clone(),
        });

    // Body language, like `gesture`: a percept, not an inbox notice. The
    // item-blind wording is the mechanic, but only at a distance — somebody at
    // arm's length sees the thing and the cavity both, because hoisting your
    // clothes in a market is not sleight of hand. A mouthful of drink is the
    // exception at every range, and deliberately so: cheeking a draught has to
    // look exactly like drinking it.
    let (own, seen_near, seen_far) = match (slot, is_drink) {
        (BodySlot::Mouth, true) => (
            format!("You took a mouthful of {phrase}."),
            format!("took a mouthful of {seen_phrase}"),
            format!("took a mouthful of {seen_phrase}"),
        ),
        (BodySlot::Mouth, false) => (
            format!("You slipped the {noun} into your mouth."),
            format!("slipped {seen_phrase} into their mouth"),
            "slipped something into their mouth".to_string(),
        ),
        (BodySlot::Butt, _) => (
            format!("You tucked the {noun} away, out of sight."),
            format!("hitched up their clothes and pushed {seen_phrase} up their arse"),
            "slipped something out of sight beneath their clothes".to_string(),
        ),
        (BodySlot::Frontbutt, _) => (
            format!("You tucked the {noun} away, out of sight."),
            format!("hitched up their clothes and pushed {seen_phrase} up their cunt"),
            "slipped something out of sight beneath their clothes".to_string(),
        ),
    };
    let (hearers, plain_sight) = deliver_pocket_percept(world, actor_id, &seen_near, &seen_far);
    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .remember_percept(own);

    // Only what somebody plainly saw is worth a turn of their attention, and
    // only from a lower slot: a hand at a mouth is the sly act the cutpurse
    // defence needs, but a man hoisting his clothes in front of you is not
    // something you go on selling fish through.
    world_event_witnessed(
        world,
        "pocket_item",
        actor_id,
        Some(pocketed_id),
        hearers.clone(),
        match slot {
            BodySlot::Mouth => Vec::new(),
            _ => plain_sight,
        },
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} pockets the {noun} ({})",
        world.characters[actor_id].name(),
        slot.as_str()
    ))
}

/// `retrieve_item` — back into the hand. The slot is derived, never asked for:
/// the model already told the world where it went.
fn retrieve_item(
    world: &mut World,
    actor_id: &ActorId,
    args: &Value,
) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    let Some(slot) = world.characters[actor_id].pocket_slot_of(&item_id) else {
        return Err(ActionError::new(
            ActionErrorCode::NotPocketed,
            format!(
                "nothing with id {} rides in your body slots",
                repr_id(item_id.as_str())
            ),
        ));
    };
    let item = world.items[&item_id].clone();
    let noun = counted_noun(world, &item, 1);
    let phrase = counted_phrase(world, &item, 1);
    take_pocket_entry(world, actor_id, &item_id).expect("the entry was just found");

    // Coming out is as visible as going in — and here the close watcher gets
    // the thing as it now is, stains and all, because that is what they see.
    let (seen_near, seen_far) = match slot {
        BodySlot::Mouth => (
            format!("took {phrase} from their mouth"),
            "took something from their mouth".to_string(),
        ),
        BodySlot::Butt => (
            format!("pulled {phrase} out of their arse"),
            "fetched something from beneath their clothes".to_string(),
        ),
        BodySlot::Frontbutt => (
            format!("pulled {phrase} out of their cunt"),
            "fetched something from beneath their clothes".to_string(),
        ),
    };
    let (hearers, plain_sight) = deliver_pocket_percept(world, actor_id, &seen_near, &seen_far);
    let own = if slot == BodySlot::Mouth {
        format!("You took the {noun} back out of your mouth.")
    } else {
        format!("You brought the {noun} back out from under your clothes.")
    };
    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .remember_percept(own);

    world_event_witnessed(
        world,
        "retrieve_item",
        actor_id,
        Some(item_id),
        hearers.clone(),
        match slot {
            BodySlot::Mouth => Vec::new(),
            _ => plain_sight,
        },
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} retrieves the {noun}",
        world.characters[actor_id].name()
    ))
}

/// `swallow` — the second stage of drinking, and the smuggler's trick. Food
/// feeds; anything else goes down whole and comes back on the gut clock
/// (`extra_pockets.md` M3). The only thing a bystander gets is a 2 m gulp:
/// swallowing the evidence has to be quiet, or it is not a mechanic.
fn swallow(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    let item = require_mouthful(world, actor_id, &item_id)?;
    let noun = counted_noun(world, &item, 1);

    let (index, unit) = take_pocket_entry(world, actor_id, &item_id).expect("checked above");
    if let Err(error) = world.consume_item_quantity(actor_id, &item_id, 1) {
        restore_pocket_entry(world, actor_id, index, unit);
        return Err(inventory_action_error(error));
    }

    if world.item_catalog.is_edible(&item) {
        let satiety = world.item_catalog.satiety(&item).unwrap_or(0);
        let quench = world.item_catalog.thirst_quench(&item).unwrap_or(0);
        if let Some(eater) = world.characters.get_mut(actor_id) {
            let hunger = &mut eater.state.needs.hunger;
            *hunger = (*hunger + f64::from(satiety)).min(HUNGER_MAX);
            let thirst = &mut eater.state.needs.thirst;
            *thirst = (*thirst + f64::from(quench)).min(THIRST_MAX);
        }
        queue_gut(world, actor_id, None);
    } else {
        // Swallow the evidence: the thing itself is queued, and comes back
        // stained on its own schedule.
        queue_gut(world, actor_id, Some((item.kind.clone(), item.metadata.clone())));
    }

    if world.sounds_enabled
        && let Some(sound) = world.sound_catalog.get("gulp").cloned()
    {
        emit_sound(world, Some(actor_id), &sound, None);
    }
    world_event(world, "swallow", actor_id, None, Some(item_id), 1, Vec::new());
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} swallows the {noun}",
        world.characters[actor_id].name()
    ))
}

/// `spit` — the insult with a real payload. A drink is gone (the mouthful
/// splashes); anything solid lands on the target, who now holds a wet thing.
/// Targeted only for now: items on the floor do not exist
/// (`extra_pockets.md`, open questions).
fn spit(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id", "target"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    let target_id = parse_actor_id(&parsed["target"], "target")?;
    if !world.is_present(&target_id) {
        return Err(ActionError::new(
            ActionErrorCode::UnknownTarget,
            format!("there is nobody with id {}", repr_id(target_id.as_str())),
        ));
    }
    if target_id == *actor_id {
        return Err(ActionError::new(
            ActionErrorCode::SelfTarget,
            "you cannot spit at yourself",
        ));
    }
    require_interaction_range(world, actor_id, &target_id)?;
    let item = require_mouthful(world, actor_id, &item_id)?;
    let phrase = counted_phrase(world, &item, 1);
    let noun = counted_noun(world, &item, 1);
    let is_drink = world.item_catalog.is_drink(&item);

    let (index, unit) = take_pocket_entry(world, actor_id, &item_id).expect("checked above");
    let moved = if is_drink {
        world.consume_item_quantity(actor_id, &item_id, 1)
    } else {
        let key = pocket_operation_key(world, "spit", actor_id);
        world
            .transfer_item_quantity(actor_id, &target_id, &item_id, 1, &key)
            .map(|_| ())
    };
    if let Err(error) = moved {
        restore_pocket_entry(world, actor_id, index, unit);
        return Err(inventory_action_error(error));
    }

    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    let lines: Vec<(ActorId, String)> = hearers
        .iter()
        .map(|observer| {
            let who = cap_first(&identify_ids(world, observer, actor_id));
            let line = if observer == &target_id {
                format!("{who} spat {phrase} at you!")
            } else {
                format!(
                    "{who} spat {phrase} at {}",
                    identify_ids(world, observer, &target_id)
                )
            };
            (observer.clone(), line)
        })
        .collect();
    deliver(world, lines, true);
    let own = format!(
        "You spat {phrase} at {}.",
        identify_ids(world, actor_id, &target_id)
    );
    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .remember_percept(own);

    world_event(
        world,
        "spit",
        actor_id,
        Some(target_id.clone()),
        Some(item_id),
        1,
        hearers,
    );
    raise_ward_notice_for(
        world,
        actor_id,
        "spat upon a neighbour in the open street",
        Some(target_id.clone()),
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} spits {noun} at {}",
        world.characters[actor_id].name(),
        world.characters[&target_id].name()
    ))
}

/// `gargle` — pure theatre, and a sound. The mouthful survives and stays where
/// it was; gargling holy water does nothing mechanical at all, which is the
/// joke (`extra_pockets.md`, open questions).
fn gargle(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["item_id"], &[])?;
    let item_id = parse_item_id(&parsed["item_id"])?;
    let item = require_mouthful(world, actor_id, &item_id)?;
    if !world.item_catalog.is_drink(&item) {
        return Err(ActionError::new(
            ActionErrorCode::NotEdible,
            "you can only gargle a drink",
        ));
    }
    let noun = counted_noun(world, &item, 1);
    // The catalog row carries the whole percept fan-out — nothing changes hands.
    if world.sounds_enabled
        && let Some(sound) = world.sound_catalog.get("gargle").cloned()
    {
        emit_sound(world, Some(actor_id), &sound, None);
    }
    world_event(world, "gargle", actor_id, None, Some(item_id), 1, Vec::new());
    Ok(format!(
        "{} gargles the {noun}",
        world.characters[actor_id].name()
    ))
}

/// `expel` — whatever rides below comes out where you stand. Stools are gone
/// for good (the world has no ground items); anything else simply stops being
/// hidden. Where you do it matters: an officer in earshot puts it on the ward's
/// tongues.
fn expel(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    args_object(args, &[], &[])?;
    let lower: Vec<PocketedUnit> = world.characters[actor_id]
        .pockets()
        .iter()
        .filter(|unit| unit.slot.is_lower())
        .cloned()
        .collect();
    if lower.is_empty() {
        return Err(ActionError::new(
            ActionErrorCode::NothingToExpel,
            "nothing rides in your lower slots",
        ));
    }
    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .state
        .pockets
        .retain(|unit| !unit.slot.is_lower());

    // A stool is left in the gutter (there is nowhere else for it); anything
    // else was in `holds` all along and simply stops being concealed.
    let mut voided = 0u32;
    for unit in &lower {
        let is_stool = world
            .items
            .get(&unit.item_id)
            .is_some_and(|item| item.kind.as_str() == POOP_KIND);
        if is_stool && world.consume_item_quantity(actor_id, &unit.item_id, 1).is_ok() {
            voided += 1;
        }
    }

    if world.sounds_enabled
        && let Some(sound) = world.sound_catalog.get("soft_report").cloned()
    {
        emit_sound(world, Some(actor_id), &sound, None);
    }
    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .remember_percept("You relieved yourself where you stood.");
    world_event(
        world,
        "expel",
        actor_id,
        None,
        None,
        lower.len() as u32,
        Vec::new(),
    );
    if voided > 0 {
        raise_ward_notice_for(world, actor_id, "fouled the street in open view", None);
    }
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} relieves themself where they stand",
        world.characters[actor_id].name()
    ))
}

/// The law's own eyes (`law_and_order.md` M3 × `extra_pockets.md` M4): a public
/// indecency an officer *witnesses* needs no report and no verb — the nearest
/// law-cast character within earshot raises the notice mechanically, from their
/// own point of view. The prose carries a name only when that officer knows it;
/// otherwise "a stranger", never an id (the unknown-people rule). An officer
/// fouling the street themself is nonsense, and is skipped.
fn raise_ward_notice_for(
    world: &mut World,
    offender_id: &ActorId,
    deed: &str,
    wronged: Option<ActorId>,
) {
    if crate::notices::is_law(&world.characters[offender_id]) {
        return;
    }
    // `nearby` is ordered by distance then id, so this is the nearest officer.
    let Some(officer_id) = nearby(world, offender_id, HEARING_RADIUS_M)
        .into_iter()
        .find(|id| world.characters.get(id).is_some_and(crate::notices::is_law))
    else {
        return;
    };
    let officer = &world.characters[&officer_id];
    let about = if officer.knows().contains(offender_id) {
        world.characters[offender_id].name().to_string()
    } else {
        "a stranger".to_string()
    };
    let place = world
        .area_map
        .location_description(world.characters[offender_id].position_m());
    // Stamped exactly as `raise_notice` does: a clock-less world raises an
    // undated notice that never decays.
    let since = world
        .current_time
        .map(|time| format!("{}'s {}", time.weekday.label(), time.office.label()));
    let raised_game_days = world.current_time.map(|time| time.day as f64 + time.fraction);
    let notice_id = world.notices.raise(
        about,
        deed.to_string(),
        place,
        since,
        raised_game_days,
        officer_id,
        Some(offender_id.clone()),
        wronged,
        // Neither wrong this raises *takes* anything — a spitter leaves the
        // mouthful behind, a fouler leaves worse — so there is nothing whose
        // return could settle the word on its own (M3.5). Only `raise_notice`'s
        // optional `taken` ever fills this in.
        None,
    );
    let line = world
        .notices
        .live()
        .iter()
        .find(|notice| notice.id == notice_id)
        .expect("just raised")
        .line();
    // The offender is excluded from the carriers — the ward talks *about* them.
    let carriers = crate::notices::carrier_ids(world, notice_id, offender_id);
    let lines = carriers
        .into_iter()
        .map(|carrier| (carrier, format!("word in the ward: {line}")))
        .collect();
    deliver(world, lines, true);
}

/// Queue what the gut is working on (`extra_pockets.md` M3): `None` is a meal
/// (one stool per gut, however many meals — a second meal coalesces into the
/// one already brewing), `Some((kind, metadata))` a swallowed inedible on its
/// way back. Formation time is a pure function of (actor, meal, clock): the
/// `hash01` idiom, so the headless runner and the tests replay identically.
///
/// A clock-less world (the hermetic fixtures) queues nothing at all, which is
/// what keeps the frozen golden prompts byte-identical.
pub(crate) fn queue_gut(
    world: &mut World,
    actor_id: &ActorId,
    contents: Option<(ItemKind, std::collections::BTreeMap<String, String>)>,
) {
    let Some(time) = world.current_time else {
        return;
    };
    let Some(actor) = world.characters.get(actor_id) else {
        return;
    };
    let (kind, metadata) = match contents {
        None => {
            if actor
                .state
                .gut
                .iter()
                .any(|entry| entry.kind.as_str() == POOP_KIND)
            {
                return;
            }
            (
                ItemKind::from_raw(POOP_KIND),
                std::collections::BTreeMap::new(),
            )
        }
        Some(contents) => contents,
    };
    let game_days = time.day as f64 + time.fraction;
    let spread = crate::world::hash01("gut_clock", actor_id, actor.state.gut.len() as u64);
    let due_game_days = game_days + crate::GUT_MIN_GAME_DAYS + spread * crate::GUT_SPREAD_GAME_DAYS;
    world
        .characters
        .get_mut(actor_id)
        .expect("checked above")
        .state
        .gut
        .push(GutEntry {
            kind,
            metadata,
            due_game_days,
        });
}

fn set_goal(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["goal"], &[])?;
    let value = &parsed["goal"];
    let goal = if value.is_null() {
        GOAL_NONE.to_string()
    } else {
        parse_text(value, "goal", GOAL_MAX_CHARS)?
    };
    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    actor.state.goal = goal;
    // Quirk (D15): the assignment is compared against the sentinel afterwards,
    // so the literal string "None" (even " None ") also drops the goal.
    if actor.state.goal == GOAL_NONE {
        Ok(format!("{} drops their goal", actor.name()))
    } else {
        Ok(format!("{} now wants: {}", actor.name(), actor.state.goal))
    }
}

fn make_sound(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["sound"], &[])?;
    let value = &parsed["sound"];
    // Ids only, no name fallback. A non-emittable catalog sound (the town bell)
    // is indistinguishable from an unknown id: no information leak.
    let sound = value
        .as_str()
        .and_then(|id| world.sound_catalog.get(id))
        .filter(|sound| sound.actor_emittable)
        .cloned();
    let Some(sound) = sound else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownSound,
            format!("there is no sound {}", py_repr(value)),
        ));
    };
    // Checked after the catalog lookup: an unknown sound in a disabled world
    // still reports unknown_sound.
    if !world.sounds_enabled {
        return Err(ActionError::new(
            ActionErrorCode::SoundsDisabled,
            "sounds are disabled in this world",
        ));
    }
    Ok(emit_sound(world, Some(actor_id), &sound, None))
}

/// A resolved `gesture` target: none, a person, or a place (its name).
enum GestureTargetResolved {
    None,
    Person(ActorId),
    Place(String),
}

/// Set (or clear) an actor's looping gesture, bumping the public revision only
/// when it actually changes — `active_gesture` is a public snapshot field, so a
/// no-op write must not republish the world.
fn set_active_gesture(world: &mut World, actor_id: &ActorId, kind: Option<GestureKind>) {
    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    if actor.active_gesture() == kind {
        return;
    }
    actor.state.active_gesture = kind.map(|kind| ActiveGesture {
        kind,
        deadline: None,
    });
    world.touch_public_state();
}

/// `gesture` — the deliberate body (`features/npc_bodies.md` §7). A
/// communicative motion the model commands, mirroring `make_sound`'s arg shape
/// (`{"kind": "wave", "to": "optional name"}`): witnesses within the 20 m
/// social radius get a percept (unknown-people named), the actor remembers
/// their own act, `dance` sets the looping `active_gesture`, and the host gets
/// a transient `EngineMessage::Gesture` to play the pose.
fn gesture(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["kind"], &["to"])?;
    let kind_value = &parsed["kind"];
    let Some(kind) = kind_value.as_str().and_then(GestureKind::from_verb) else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownGesture,
            format!("there is no gesture {}", py_repr(kind_value)),
        ));
    };
    let spec = kind.spec();
    let to_value = optional_arg(parsed, "to");

    // Sight reuses the 20 m social radius, exactly like `say`; occlusion is
    // ignored the same way. Resolved once, before the target check needs it.
    let origin = world.characters[actor_id].position_m();
    let witnesses = world.characters_within(origin, HEARING_RADIUS_M, Some(actor_id));
    let resolved = resolve_gesture_target(world, actor_id, &witnesses, spec, to_value)?;

    // The actor's own recollection first, then every witness's percept.
    let own = render_own_gesture(world, actor_id, spec, &resolved);
    world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world")
        .remember_percept(own);
    let percepts: Vec<(ActorId, String)> = witnesses
        .iter()
        .map(|recipient| {
            (
                recipient.clone(),
                render_witness_gesture(world, recipient, actor_id, spec, &resolved),
            )
        })
        .collect();
    deliver(world, percepts, true);

    // Only a looper (`dance`) sets snapshot state; every other kind ends a
    // running loop — the same rule `dispatch` enforces for the other verbs.
    set_active_gesture(world, actor_id, if spec.loops { Some(kind) } else { None });

    // The transient host trigger, presented like speech: the player is in
    // `recipient_ids` only when within the radius. A place-pointed gesture
    // carries no `target_id` — the host cannot aim at a place.
    let target_actor = match &resolved {
        GestureTargetResolved::Person(id) => Some(id.clone()),
        GestureTargetResolved::None | GestureTargetResolved::Place(_) => None,
    };
    world.emit(DomainEvent::gesture(
        actor_id.clone(),
        target_actor,
        kind.as_verb(),
        origin,
        witnesses,
    ));

    Ok(render_transcript_gesture(world, actor_id, spec, &resolved))
}

/// Resolve the `to` argument against the catalog's target rule: no-target
/// gestures reject a `to`, required ones demand it, and `point` accepts a known
/// place handle first (paired with how `tell_way` resolves places) before
/// falling back to a visible person (like `say`'s target).
fn resolve_gesture_target(
    world: &World,
    actor_id: &ActorId,
    witnesses: &[ActorId],
    spec: &GestureSpec,
    to_value: Option<&Value>,
) -> Result<GestureTargetResolved, ActionError> {
    if matches!(spec.target, GestureTarget::None) {
        if to_value.is_some() {
            return Err(ActionError::new(
                ActionErrorCode::InvalidArguments,
                format!("a {} is not aimed at anyone", spec.verb),
            ));
        }
        return Ok(GestureTargetResolved::None);
    }

    let Some(value) = to_value else {
        return match spec.target {
            GestureTarget::OptionalPerson => Ok(GestureTargetResolved::None),
            GestureTarget::RequiredPerson => Err(ActionError::new(
                ActionErrorCode::InvalidArguments,
                format!("{} needs someone to {}: pass \"to\"", spec.verb, spec.verb),
            )),
            GestureTarget::RequiredPersonOrPlace => Err(ActionError::new(
                ActionErrorCode::InvalidArguments,
                "point needs a person or a place to point to: pass \"to\"",
            )),
            GestureTarget::None => unreachable!("handled above"),
        };
    };

    // `point` may aim at a place the actor holds a way to — resolved like a
    // `tell_way` place, and only if it is on the actor's whitelist.
    if matches!(spec.target, GestureTarget::RequiredPersonOrPlace)
        && let Some(text) = value.as_str()
        && let Ok(place_id) = PlaceId::new(text)
        && world.characters[actor_id]
            .state
            .places_known
            .contains(&place_id)
        && let Some(entry) = world.places.get(&place_id)
    {
        return Ok(GestureTargetResolved::Place(entry.name.clone()));
    }

    // Otherwise a visible person, exactly like `say`'s explicit target.
    let target_id = parse_actor_id(value, "to")?;
    if !world.is_present(&target_id) {
        return Err(ActionError::new(
            ActionErrorCode::UnknownTarget,
            format!("there is nobody with id {}", repr_id(target_id.as_str())),
        ));
    }
    if target_id == *actor_id {
        return Err(ActionError::new(
            ActionErrorCode::SelfTarget,
            "you cannot gesture at yourself",
        ));
    }
    if !witnesses.contains(&target_id) {
        return Err(ActionError::new(
            ActionErrorCode::OutOfRange,
            format!(
                "{} is more than {} metres away",
                identify_ids(world, actor_id, &target_id),
                format_g(HEARING_RADIUS_M)
            ),
        ));
    }
    Ok(GestureTargetResolved::Person(target_id))
}

/// The actor's own second-person percept: `{B}` from the actor's perspective.
fn render_own_gesture(
    world: &World,
    actor_id: &ActorId,
    spec: &GestureSpec,
    resolved: &GestureTargetResolved,
) -> String {
    match resolved {
        GestureTargetResolved::None => spec.own_untargeted.to_string(),
        GestureTargetResolved::Person(id) => spec
            .own_targeted
            .replace("{B}", &identify_ids(world, actor_id, id)),
        GestureTargetResolved::Place(name) => spec.own_targeted.replace("{B}", name),
    }
}

/// One witness's third-person percept: `{A}` and `{B}` from the witness's
/// perspective, with "you" when the witness is the target — unknown people
/// render through [`identify_ids`].
fn render_witness_gesture(
    world: &World,
    recipient: &ActorId,
    actor_id: &ActorId,
    spec: &GestureSpec,
    resolved: &GestureTargetResolved,
) -> String {
    let actor_name = cap_first(&identify_ids(world, recipient, actor_id));
    match resolved {
        GestureTargetResolved::None => spec.witness_untargeted.replace("{A}", &actor_name),
        GestureTargetResolved::Person(id) => {
            let target = if recipient == id {
                "you".to_string()
            } else {
                identify_ids(world, recipient, id)
            };
            spec.witness_targeted
                .replace("{A}", &actor_name)
                .replace("{B}", &target)
        }
        GestureTargetResolved::Place(name) => spec
            .witness_targeted
            .replace("{A}", &actor_name)
            .replace("{B}", name),
    }
}

/// The omniscient run-transcript line: real names, no perspective (matching
/// `make_sound`'s "{actor} …" transcript form, trailing period and all).
fn render_transcript_gesture(
    world: &World,
    actor_id: &ActorId,
    spec: &GestureSpec,
    resolved: &GestureTargetResolved,
) -> String {
    let actor_name = world.characters[actor_id].name();
    match resolved {
        GestureTargetResolved::None => spec.witness_untargeted.replace("{A}", actor_name),
        GestureTargetResolved::Person(id) => spec
            .witness_targeted
            .replace("{A}", actor_name)
            .replace("{B}", world.characters[id].name()),
        GestureTargetResolved::Place(name) => spec
            .witness_targeted
            .replace("{A}", actor_name)
            .replace("{B}", name),
    }
}

fn remember(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["memory"], &[])?;
    let memory = parse_text(&parsed["memory"], "memory", MEMORY_MAX_CHARS)?;
    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    if !actor.state.memories.contains(&memory) {
        actor.state.memories.push(memory.clone());
    }
    Ok(format!("{} remembers: {memory}", actor.name()))
}

fn forget(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["memory"], &[])?;
    let memory = parse_text(&parsed["memory"], "memory", MEMORY_MAX_CHARS)?;
    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    if let Some(index) = actor
        .state
        .memories
        .iter()
        .position(|existing| existing == &memory)
    {
        actor.state.memories.remove(index);
        return Ok(format!("{} forgets: {memory}", actor.name()));
    }
    // Otherwise the first memory (insertion order) that contains, or is
    // contained by, the argument — and the *stored* text is echoed back.
    if let Some(index) = actor
        .state
        .memories
        .iter()
        .position(|existing| existing.contains(&memory) || memory.contains(existing.as_str()))
    {
        let existing = actor.state.memories.remove(index);
        return Ok(format!("{} forgets: {existing}", actor.name()));
    }
    // A miss is a transcript line, not an error.
    Ok(format!(
        "{} tried to forget something they never knew: {memory}",
        actor.name()
    ))
}

fn parse_place_id(value: &Value) -> Result<PlaceId, ActionError> {
    value
        .as_str()
        .and_then(|text| PlaceId::new(text).ok())
        .ok_or_else(|| {
            ActionError::new(
                ActionErrorCode::InvalidArguments,
                "place_id must be a non-empty place id",
            )
        })
}

/// The route a fresh intent would walk, and the real-seconds budget it earns:
/// [`GO_TO_BUDGET_FACTOR`] × the route's expected travel time, floored for
/// doorstep trips. Both computed at intent time, which is what lets `no_route`
/// fail here instead of stranding the walker later
/// (`features/implemented/movement/05_the_llm_seam.md` §2).
fn route_budget(
    world: &World,
    actor_id: &ActorId,
    target: Vec3,
    no_route_message: String,
) -> Result<f64, ActionError> {
    let route = world
        .nav
        .as_deref()
        .and_then(|nav| nav.route_between(world.characters[actor_id].position_m(), target));
    let Some(route) = route else {
        return Err(ActionError::new(ActionErrorCode::NoRoute, no_route_message));
    };
    Ok((GO_TO_BUDGET_FACTOR * route.length_m / WALK_SPEED_MPS).max(GO_TO_MIN_BUDGET_SECONDS))
}

/// `go_to` — set a travel intent; it does not move anyone
/// (`features/implemented/movement/05_the_llm_seam.md` §2). The behaviour ladder walks it,
/// arrival and lapse are percepts, and a second `go_to` replaces the first
/// silently — the model issued both; it needs no telling.
fn go_to(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    if world.characters[actor_id].state.leaving_city {
        return Err(ActionError::new(
            ActionErrorCode::LeavingCity,
            "your party is leaving the city; the road controller owns your movement",
        ));
    }
    let parsed = args_object(args, &[], &["place_id", "person"])?;
    let place_value = optional_arg(parsed, "place_id");
    let person_value = optional_arg(parsed, "person");

    let intent = match (place_value, person_value) {
        (Some(_), Some(_)) => {
            return Err(ActionError::new(
                ActionErrorCode::InvalidArguments,
                "go_to takes either place_id or person, not both",
            ));
        }
        (None, None) => {
            return Err(ActionError::new(
                ActionErrorCode::InvalidArguments,
                "go_to needs a place_id or a person",
            ));
        }
        (Some(place_value), None) => {
            let place_id = parse_place_id(place_value)?;
            // The whitelist: you cannot walk to a place you were never handed a
            // handle for. An id outside the actor's set and an id outside the
            // world are the same error — no information leak about which
            // handles exist.
            let entry = world
                .places
                .get(&place_id)
                .filter(|_| world.characters[actor_id].state.places_known.contains(&place_id));
            let Some(entry) = entry else {
                return Err(ActionError::new(
                    ActionErrorCode::UnknownPlace,
                    format!(
                        "you know no way to a place with id {} (places_you_know lists the ways you know)",
                        repr_id(place_id.as_str())
                    ),
                ));
            };
            let (name, point) = (entry.name.clone(), entry.point);
            let budget_seconds = route_budget(
                world,
                actor_id,
                point,
                format!("no way through the streets leads to {name} from where you stand"),
            )?;
            TravelIntent {
                target: IntentTarget::Place {
                    place_id,
                    name,
                    point,
                },
                budget_seconds,
                deadline: None,
            }
        }
        (None, Some(person_value)) => {
            let target_id = parse_actor_id(person_value, "person")?;
            if !world.is_present(&target_id) {
                return Err(ActionError::new(
                    ActionErrorCode::UnknownTarget,
                    format!("there is nobody with id {}", repr_id(target_id.as_str())),
                ));
            }
            if target_id == *actor_id {
                return Err(ActionError::new(
                    ActionErrorCode::SelfTarget,
                    "you cannot follow yourself",
                ));
            }
            // Gated on sight: valid only for someone currently in you_see.
            // Without the gate a hoarded id is a tracking device — finding an
            // absent person routes through the knowledge system, not around it.
            let visible = nearby(world, actor_id, HEARING_RADIUS_M);
            if !visible.contains(&target_id) {
                return Err(ActionError::new(
                    ActionErrorCode::OutOfRange,
                    format!(
                        "you cannot see {} from here (go_to a person needs them in you_see)",
                        identify_ids(world, actor_id, &target_id)
                    ),
                ));
            }
            let last_seen = world.characters[&target_id].position_m();
            let budget_seconds = route_budget(
                world,
                actor_id,
                last_seen,
                format!(
                    "no way through the streets reaches {} from where you stand",
                    identify_ids(world, actor_id, &target_id)
                ),
            )?;
            TravelIntent {
                target: IntentTarget::Person {
                    actor_id: target_id,
                    last_seen,
                    visible: true,
                },
                budget_seconds,
                deadline: None,
            }
        }
    };

    let destination = match &intent.target {
        IntentTarget::Place { name, .. } => name.clone(),
        IntentTarget::Person { actor_id: target_id, .. } => {
            identify_ids(world, actor_id, target_id)
        }
    };
    let (line, own_line) = match &intent.target {
        IntentTarget::Place { .. } => (
            format!(
                "{} sets off for {destination}",
                world.characters[actor_id].name()
            ),
            format!("You set off for {destination}."),
        ),
        IntentTarget::Person { actor_id: target_id, .. } => (
            format!(
                "{} sets off after {}",
                world.characters[actor_id].name(),
                world.characters[target_id].name()
            ),
            format!("You set off after {destination}."),
        ),
    };
    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    actor.state.intent = Some(intent);
    // The walker's own recollection that they are under way — without it the
    // model forgets its errand the turn after issuing it.
    actor.remember_percept(own_line);
    Ok(line)
}

/// `stop {}` — abandon the current `go_to` and go back to your own business.
/// Self-initiated, so it emits no percept; the round halts the walk on its
/// next tick.
fn stop(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    args_object(args, &[], &[])?;
    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    if actor.state.intent.take().is_some() {
        Ok(format!("{} turns back to their own business", actor.name()))
    } else {
        Ok(format!("{} has no errand to abandon", actor.name()))
    }
}

/// `set_round` — move **one leg** of the standing daily round (movement M6,
/// `05_the_llm_seam.md` §4).
///
/// The Night Office's verb, and the only way anything but the seed writes a
/// Round. It is an intent, not an edit: the resolved legs belong to
/// [`crate::round::Round`] and this layer holds only a [`World`], so the
/// decision is recorded on the character and [`crate::round::tick`] carries it
/// out — the same shape as `go_to`.
///
/// Deliberately not listed in `turn.j2`: a daytime turn has no reason to spend
/// tokens on it. The dispatch table is one table, so a model that reaches for
/// it anyway gets a working edit rather than a lie about which verbs exist.
fn set_round(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["leg", "place_id"], &[])?;
    let place_id = parse_place_id(&parsed["place_id"])?;
    // The whitelist, exactly as `go_to`'s: you cannot point your day at a place
    // you were never handed a handle for, and an id outside your set and an id
    // outside the world are the same error.
    if !world.characters[actor_id]
        .state
        .places_known
        .contains(&place_id)
    {
        return Err(ActionError::new(
            ActionErrorCode::UnknownPlace,
            format!(
                "you know no way to a place with id {} (places_you_know lists the ways you know)",
                repr_id(place_id.as_str())
            ),
        ));
    }
    set_round_leg(world, actor_id, &parsed["leg"], &place_id)
}

/// The shared body of `set_round`: validate the leg number against the actor's
/// own round and record the edit.
///
/// Split out because the Night Office's **ward** batch edits somebody else's
/// round — the ward decided it, so the ward lane calls this directly with the
/// named person (`crate::night`), having first taught them the way. The
/// place-handle whitelist above is the *self* rule and stays with the verb.
pub(crate) fn set_round_leg(
    world: &mut World,
    actor_id: &ActorId,
    leg: &Value,
    place_id: &PlaceId,
) -> Result<String, ActionError> {
    let Some(entry) = world.places.get(place_id) else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownPlace,
            format!(
                "there is no place with id {}",
                repr_id(place_id.as_str())
            ),
        ));
    };
    let place_name = entry.name.clone();

    let legs = world.characters[actor_id].state.daily_round.len();
    if legs == 0 {
        return Err(ActionError::new(
            ActionErrorCode::InvalidAction,
            "you keep no standing round to change",
        ));
    }
    // The sheet numbers legs from 1, so the model does too; anything outside
    // that range names a leg that is not on the sheet in front of it.
    let Some(number) = leg.as_u64().filter(|number| (1..=legs as u64).contains(number)) else {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            format!(
                "leg must be one of the {legs} leg numbers in your_round (1 to {legs})"
            ),
        ));
    };

    let actor = world
        .characters
        .get_mut(actor_id)
        .expect("the actor is in the world");
    actor.state.round_edit = Some(RoundEdit {
        leg: number as usize - 1,
        place_id: place_id.clone(),
    });
    Ok(format!(
        "{} makes {place_name} leg {number} of their round",
        actor.name()
    ))
}

/// `tell_way` — the knowledge-transfer verb
/// (`features/implemented/movement/05_the_llm_seam.md` §3). The receiving LLM stores
/// nothing: the id is written into the target's `places_known` — sim state —
/// and at the next render the place is simply there. Targeted, not broadcast:
/// eavesdroppers learn nothing.
fn tell_way(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(args, &["person", "place_id"], &[])?;
    let target_id = parse_actor_id(&parsed["person"], "person")?;
    let place_id = parse_place_id(&parsed["place_id"])?;

    if !world.is_present(&target_id) {
        return Err(ActionError::new(
            ActionErrorCode::UnknownTarget,
            format!("there is nobody with id {}", repr_id(target_id.as_str())),
        ));
    }
    if target_id == *actor_id {
        return Err(ActionError::new(
            ActionErrorCode::SelfTarget,
            "you cannot tell yourself the way",
        ));
    }
    // The speaker must hold the id — you cannot share a way you do not know.
    let entry = world
        .places
        .get(&place_id)
        .filter(|_| world.characters[actor_id].state.places_known.contains(&place_id));
    let Some(entry) = entry else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownPlace,
            format!(
                "you know no way to a place with id {} yourself",
                repr_id(place_id.as_str())
            ),
        ));
    };
    let place_name = entry.name.clone();
    // The target must be in earshot — the existing 20 m hearing rule.
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    if !hearers.contains(&target_id) {
        return Err(ActionError::new(
            ActionErrorCode::OutOfRange,
            format!(
                "{} is more than {} metres away",
                identify_ids(world, actor_id, &target_id),
                format_g(HEARING_RADIUS_M)
            ),
        ));
    }

    // One inbox line makes the arrival of knowledge narratable news — the name
    // from the world registry, not from anyone's prose.
    let told = format!(
        "{} told you the way to {place_name}.",
        cap_first(&identify_ids(world, &target_id, actor_id))
    );
    let own = format!(
        "You told {} the way to {place_name}.",
        identify_ids(world, actor_id, &target_id)
    );
    world
        .characters
        .get_mut(&target_id)
        .expect("the target is in the world")
        .state
        .places_known
        .insert(place_id);
    deliver(world, vec![(target_id.clone(), told)], true);
    world
        .characters
        .get_mut(actor_id)
        .expect("the speaker is in the world")
        .remember_percept(own);
    Ok(format!(
        "{} tells {} the way to {place_name}",
        world.characters[actor_id].name(),
        world.characters[&target_id].name()
    ))
}

/// `raise_notice` (`law_and_order.md` M3): a law-cast actor puts a wrong on
/// the ward's tongues. The prose (`about`/`deed`/`where`) is what carriers
/// hear and repeat — descriptions and places, never ids — while the optional
/// `accused`/`wronged` ids are the private linkage a settlement needs to find
/// its notice, and `taken` (M3.5) is the one thing whose return settles the
/// word without anybody having to judge it.
fn raise_notice(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError> {
    let parsed = args_object(
        args,
        &["about", "deed"],
        &["where", "accused", "wronged", "taken"],
    )?;
    if !crate::notices::is_law(&world.characters[actor_id]) {
        return Err(ActionError::new(
            ActionErrorCode::InvalidAction,
            "only those who serve the city's law raise ward notices - report the wrong aloud to a sergeant, a gate keeper, or the Tallage instead",
        ));
    }
    let about = parse_text(&parsed["about"], "about", PLAYER_SPEECH_MAX_CHARS)?;
    let deed = parse_text(&parsed["deed"], "deed", PLAYER_SPEECH_MAX_CHARS)?;
    let place = optional_arg(parsed, "where")
        .map(|value| parse_text(value, "where", PLAYER_SPEECH_MAX_CHARS))
        .transpose()?;
    let parse_person = |key: &str| -> Result<Option<ActorId>, ActionError> {
        let Some(value) = optional_arg(parsed, key) else {
            return Ok(None);
        };
        let person = parse_actor_id(value, key)?;
        if !world.characters.contains_key(&person) {
            return Err(ActionError::new(
                ActionErrorCode::UnknownTarget,
                format!("there is nobody with id {}", repr_id(person.as_str())),
            ));
        }
        Ok(Some(person))
    };
    let accused = parse_person("accused")?;
    let wronged = parse_person("wronged")?;
    if accused.as_ref() == Some(actor_id) {
        return Err(ActionError::new(
            ActionErrorCode::SelfTarget,
            "you cannot raise the ward against yourself",
        ));
    }
    let taken = optional_arg(parsed, "taken")
        .map(|value| {
            let item_id = parse_item_id(value)?;
            if !world.items.contains_key(&item_id) {
                return Err(ActionError::new(
                    ActionErrorCode::UnknownItem,
                    format!("there is no item with id {}", repr_id(item_id.as_str())),
                ));
            }
            Ok(item_id)
        })
        .transpose()?;

    // Stamped from the host-set clock, like the sheet's `the_day`; a clock-less
    // world raises an undated notice that never decays (hermetic tests).
    let since = world
        .current_time
        .map(|time| format!("{}'s {}", time.weekday.label(), time.office.label()));
    let raised_game_days = world.current_time.map(|time| time.day as f64 + time.fraction);
    let notice_id = world.notices.raise(
        about,
        deed,
        place,
        since,
        raised_game_days,
        actor_id.clone(),
        accused.clone(),
        wronged,
        taken,
    );
    let line = world
        .notices
        .live()
        .iter()
        .find(|notice| notice.id == notice_id)
        .expect("just raised")
        .line();

    // The word travels now, not on proximity: the law cast always, citizens
    // diluted through the deterministic carry roll. The percept (not a bare
    // notify) is load-bearing — a non-empty inbox is what admits the idle
    // turn that lets a carrier speak of it.
    let carriers = crate::notices::carrier_ids(world, notice_id, actor_id);
    let lines = carriers
        .iter()
        .map(|carrier| (carrier.clone(), format!("word in the ward: {line}")))
        .collect();
    deliver(world, lines, true);
    world
        .characters
        .get_mut(actor_id)
        .expect("the raiser is in the world")
        .remember_percept(format!("You put the word in the ward: {line}"));
    world_event(
        world,
        "raise_notice",
        actor_id,
        accused,
        None,
        1,
        carriers,
    );
    Ok(format!(
        "{} puts the word in the ward: {line}",
        world.characters[actor_id].name()
    ))
}

/// `settle_notice` (`law_and_order.md` M3.5): one word comes off the ward's
/// tongues because somebody with standing said so. The law cast may settle any
/// notice — a fine taken, a talking-to given, or a purse quietly pocketed — and
/// the wronged party may settle their own, law or not. Per-notice, never a
/// blanket clear, and there is no counter-verb: *not* settling is refusing, and
/// an unanswered word simply decays.
fn settle_notice(
    world: &mut World,
    actor_id: &ActorId,
    args: &Value,
) -> Result<String, ActionError> {
    let parsed = args_object(args, &["notice_id"], &[])?;
    let Some(notice_id) = parsed["notice_id"].as_u64() else {
        return Err(ActionError::new(
            ActionErrorCode::InvalidArguments,
            "notice_id must be the number of a notice in word_in_the_ward",
        ));
    };
    let Some(notice) = world.notices.get(notice_id) else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownTarget,
            format!("the ward is saying no notice numbered {notice_id}"),
        ));
    };
    if !crate::notices::may_settle(world, actor_id, notice) {
        return Err(ActionError::new(
            ActionErrorCode::InvalidAction,
            "only those who serve the city's law - or the one who was wronged - can take a word off the ward's tongues",
        ));
    }
    let notice = world.notices.settle(notice_id).expect("just found above");
    let line = notice.line();
    let carriers = announce_settled(world, &notice, actor_id, "the wrong answered");
    world
        .characters
        .get_mut(actor_id)
        .expect("the settler is in the world")
        .remember_percept(format!("You settled the ward's word: {line}"));
    world_event(
        world,
        "settle_notice",
        actor_id,
        notice.accused.clone(),
        None,
        1,
        carriers,
    );
    // No `touch_public_state`: notices are prompt state, not carriage state —
    // `raise_notice` republishes nothing either.
    Ok(format!(
        "{} settles the ward's word: {line}",
        world.characters[actor_id].name()
    ))
}

/// Tell the carriers a word has died, and hand back who was told (the event's
/// recipients). `except` is whoever already knows — the accused whose
/// restitution ended it, or the settler, who gets their own history line.
fn announce_settled(
    world: &mut World,
    notice: &crate::notices::WardNotice,
    except: &ActorId,
    reason: &str,
) -> Vec<ActorId> {
    let settled_line = format!("the ward's word is settled, {reason}: {}", notice.line());
    let carriers = crate::notices::carrier_ids(world, notice.id, except);
    let lines = carriers
        .iter()
        .map(|carrier| (carrier.clone(), settled_line.clone()))
        .collect();
    deliver(world, lines, true);
    carriers
}

/// The percept that keeps a dropped verb from reading as a cheat (M3.5): when
/// the accused hands something to a law officer or to the wronged, the acceptor
/// is told this may be what the ward's word wants and that `settle_notice` is
/// theirs to call. The engine gives them the turn to answer it in
/// ([`crate::Engine::nudge_restitution_acceptor`]) — with the question in front
/// of them, an officer who pockets the purse and keeps the word alive is a
/// story rather than a bug. The player is never told: he has no verbs, and the
/// transfers he could settle already settled themselves.
fn offer_restitution(world: &mut World, giver_id: &ActorId, acceptor_id: &ActorId) {
    if !world.characters[acceptor_id].control().is_llm() {
        return;
    }
    let lines: Vec<(ActorId, String)> =
        crate::notices::restitution_candidates(world, giver_id, acceptor_id)
            .into_iter()
            .filter_map(|notice_id| world.notices.get(notice_id))
            .map(|notice| {
                (
                    acceptor_id.clone(),
                    format!(
                        "what you were just handed may be what the ward's word wants \
                         (notice {}): {} - settle_notice if it answers the wrong, or let the \
                         word stand and say why",
                        notice.id,
                        notice.line()
                    ),
                )
            })
            .collect();
    deliver(world, lines, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        character::{Character, CharacterSheet, Control},
        item::Item,
        sounds::{Sound, SoundCatalog},
    };
    use serde_json::json;
    use std::collections::BTreeSet;

    fn character(id: &str, name: &str, x: f64) -> Character {
        Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: ActorId::from_raw(id),
            name: name.into(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test square".into(),
            appearance: Default::default(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
            presence: crate::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::EconomicClass::Resident,
        })
    }

    fn speech_world() -> World {
        let mut world = World::new();
        world.add_character(character("speaker", "Speaker", 0.0));
        world.add_character(character("target", "Target", 10.0));
        world.add_character(character("bystander", "Bystander", 5.0));
        world.add_character(character("distant", "Distant", 20.0001));
        world
    }

    #[test]
    fn targeted_speech_reaches_the_target_and_nearby_bystanders_in_distance_order() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");

        let line = apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"target": "target", "text": "  hello  "}),
        )
        .unwrap();

        assert_eq!(line, "Speaker -> Target: \"hello\"");
        let target = &world.characters[&ActorId::from_raw("target")];
        assert!(target.inbox().last().unwrap().contains("said to you"));
        let bystander = &world.characters[&ActorId::from_raw("bystander")];
        assert!(
            bystander
                .inbox()
                .last()
                .unwrap()
                .contains("said to a stranger (id target)")
        );
        assert!(
            world.characters[&ActorId::from_raw("distant")]
                .inbox()
                .is_empty()
        );

        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.text.as_deref(), Some("hello"));
        assert_eq!(
            event.recipient_ids,
            vec![ActorId::from_raw("bystander"), ActorId::from_raw("target")]
        );
    }

    #[test]
    fn a_heard_self_introduction_teaches_the_human_observer_the_speakers_name() {
        let mut world = World::new();
        let speaker = ActorId::from_raw("nan01");
        let player = ActorId::from_raw("player");
        world.add_character(character("nan01", "Nan", 0.0));
        let mut player_character = character("player", "Player", 2.0);
        player_character.sheet.control = Control::Player;
        world.add_character(player_character);

        assert_eq!(
            identify_ids(&world, &player, &speaker),
            "a stranger (id nan01)"
        );
        let revision = world.world_revision;
        apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"target": "player", "text": "The nanny calls me nothing. I am Nan."}),
        )
        .unwrap();

        assert!(world.characters[&player].knows().contains(&speaker));
        assert_eq!(identify_ids(&world, &player, &speaker), "Nan");
        assert!(world.world_revision > revision);
    }

    #[test]
    fn a_name_substring_is_not_mistaken_for_an_introduction() {
        let mut world = World::new();
        let speaker = ActorId::from_raw("nan01");
        let player = ActorId::from_raw("player");
        world.add_character(character("nan01", "Nan", 0.0));
        let mut player_character = character("player", "Player", 2.0);
        player_character.sheet.control = Control::Player;
        world.add_character(player_character);

        apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"target": "player", "text": "The nanny brought bananas."}),
        )
        .unwrap();
        assert!(!world.characters[&player].knows().contains(&speaker));
    }

    #[test]
    fn a_bad_self_or_distant_target_never_broadcasts() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        let revision = world.world_revision;

        for (target, code) in [
            ("nobody", ActionErrorCode::UnknownTarget),
            ("speaker", ActionErrorCode::SelfTarget),
            ("distant", ActionErrorCode::OutOfRange),
        ] {
            let error = apply_action(
                &mut world,
                &speaker,
                "say",
                &json!({"target": target, "text": "hello"}),
            )
            .unwrap_err();
            assert_eq!(error.code, code, "target {target}");
        }
        assert!(world.characters.values().all(|c| c.inbox().is_empty()));
        assert!(world.drain_events().is_empty());
        assert_eq!(world.world_revision, revision);
        // The out-of-range message names the radius the Python way.
        let error = apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"target": "distant", "text": "hello"}),
        )
        .unwrap_err();
        assert!(error.message.contains("more than 20 metres away"));
    }

    #[test]
    fn say_validation_is_strict() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        for (args, code) in [
            (json!({}), ActionErrorCode::InvalidArguments),
            (json!({"text": "   "}), ActionErrorCode::InvalidArguments),
            (json!({"text": 5}), ActionErrorCode::InvalidArguments),
            (
                json!({"text": "hi", "extra": 1}),
                ActionErrorCode::InvalidArguments,
            ),
            (json!({"text": "\u{0}"}), ActionErrorCode::InvalidArguments),
            (json!([]), ActionErrorCode::InvalidArguments),
            (
                json!({"text": "x".repeat(PLAYER_SPEECH_MAX_CHARS + 1)}),
                ActionErrorCode::TextTooLong,
            ),
        ] {
            let error = apply_action(&mut world, &speaker, "say", &args).unwrap_err();
            assert_eq!(error.code, code, "args {args}");
        }
        // Exactly at the limit is fine, and counts scalar values, not bytes.
        assert!(
            apply_action(
                &mut world,
                &speaker,
                "say",
                &json!({"text": "é".repeat(PLAYER_SPEECH_MAX_CHARS)}),
            )
            .is_ok()
        );
    }

    /// `_text` strips with `str.strip()`, whose whitespace set includes the C0
    /// separators `\x1c..\x1f` — Rust's `char::is_whitespace` does not. With
    /// `str::trim` these survived the strip and were then rejected as control
    /// characters, turning an action Python accepts into an `ActionError`.
    #[test]
    fn text_is_stripped_with_pythons_whitespace_set() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");

        let line = apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"text": "\u{1c}hello\u{1f}"}),
        )
        .unwrap();
        assert_eq!(line, "Speaker (aloud): \"hello\"");
        assert_eq!(
            world.drain_events().pop().unwrap().text.as_deref(),
            Some("hello")
        );

        // The length cap is measured over the *stripped* text, so a trailing
        // separator does not push a maximum-length line over the limit.
        assert!(
            apply_action(
                &mut world,
                &speaker,
                "say",
                &json!({"text": format!("{}\u{1f}", "x".repeat(PLAYER_SPEECH_MAX_CHARS))}),
            )
            .is_ok()
        );
        // An *interior* separator is still a control character, and still fatal.
        assert_eq!(
            apply_action(&mut world, &speaker, "say", &json!({"text": "a\u{1f}b"}))
                .unwrap_err()
                .code,
            ActionErrorCode::InvalidArguments
        );
        // A string of nothing but them is empty, not a control-character error.
        let error = apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"text": "\u{1c}\u{1f}"}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::InvalidArguments);
        assert_eq!(error.message, "text must not be empty");
    }

    /// The id in an error message is `{id!r}`, and these messages become
    /// model-visible `system:` inbox lines — so an apostrophe has to flip
    /// CPython's quoting, not produce `'it's'`.
    #[test]
    fn ids_in_error_messages_are_python_reprs() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");

        let error = apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"target": "gh0st", "text": "hi"}),
        )
        .unwrap_err();
        assert_eq!(error.message, "there is nobody with id 'gh0st'");

        let error = apply_action(
            &mut world,
            &speaker,
            "say",
            &json!({"target": "it's", "text": "hi"}),
        )
        .unwrap_err();
        assert_eq!(error.message, "there is nobody with id \"it's\"");

        let error = apply_action(
            &mut world,
            &speaker,
            "accept_offered_item",
            &json!({"item_id": "it's"}),
        )
        .unwrap_err();
        assert_eq!(
            error.message,
            "nobody is offering you an item with id \"it's\""
        );
    }

    #[test]
    fn a_frozen_position_moves_the_utterance_but_not_the_speaker() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        let far = Vec3::new(100.0, 0.0, 0.0);

        apply_action_at(
            &mut world,
            &speaker,
            "say",
            &json!({"text": "hello"}),
            Some(far),
        )
        .unwrap();

        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.position_m, Some(far));
        assert!(event.recipient_ids.is_empty(), "nobody is near (100,0,0)");
        assert_eq!(world.characters[&speaker].position_m(), Vec3::ZERO);
    }

    fn offer_world() -> World {
        let mut world = World::new();
        let mut giver = character("giver", "Giver", 0.0);
        giver.state.holds.push(ItemId::from_raw("apple"));
        world.add_character(giver);
        world.add_character(character("receiver", "Receiver", 3.0));
        world.add_item(Item::new(ItemId::from_raw("apple"), "apple"));
        world
    }

    #[test]
    fn accept_revalidates_distance_and_the_offer_survives() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "receiver"}),
        )
        .unwrap();

        world
            .characters
            .get_mut(&receiver)
            .unwrap()
            .state
            .position_m = Vec3::new(5.0, 0.0, 0.0);
        let revision = world.world_revision;

        let error = apply_action(
            &mut world,
            &receiver,
            "accept_offered_item",
            &json!({"item_id": "apple"}),
        )
        .unwrap_err();

        assert_eq!(error.code, ActionErrorCode::OutOfRange);
        assert!(error.message.contains("more than 4 metres"));
        assert!(world.offers.contains_key(&ItemId::from_raw("apple")));
        assert_eq!(world.world_revision, revision);
        world.assert_invariants();
    }

    #[test]
    fn a_stale_offer_is_repaired_and_bumps_the_revision_before_failing() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "receiver"}),
        )
        .unwrap();

        // The giver no longer holds the item (a sim-external mutation stands in
        // for the races Python guards against).
        world
            .characters
            .get_mut(&giver)
            .unwrap()
            .state
            .holds
            .clear();
        let revision = world.world_revision;

        let error = apply_action(
            &mut world,
            &receiver,
            "accept_offered_item",
            &json!({"item_id": "apple"}),
        )
        .unwrap_err();

        assert_eq!(error.code, ActionErrorCode::StaleOffer);
        // The failed action still mutated: offer gone, revision bumped.
        assert!(world.offers.is_empty());
        assert_eq!(world.world_revision, revision + 1);
    }

    /// Walk the receiver `x` metres out and sweep. Returns what lapsed.
    fn drift_and_sweep(world: &mut World, x: f64) -> Vec<ItemId> {
        world
            .characters
            .get_mut(&ActorId::from_raw("receiver"))
            .unwrap()
            .state
            .position_m = Vec3::new(x, 0.0, 0.0);
        lapse_distant_offers(world)
    }

    fn offer_the_apple(world: &mut World, target: Option<&str>) {
        let args = match target {
            Some(target) => json!({"item_id": "apple", "target": target}),
            None => json!({"item_id": "apple"}),
        };
        apply_action(world, &ActorId::from_raw("giver"), "offer_item", &args).unwrap();
        world.drain_events();
    }

    #[test]
    fn an_offer_lapses_once_the_two_have_drifted_out_of_earshot() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        let apple = ItemId::from_raw("apple");
        offer_the_apple(&mut world, Some("receiver"));
        for actor in [&giver, &receiver] {
            world.characters.get_mut(actor).unwrap().state.inbox.clear();
        }
        let revision = world.world_revision;

        assert_eq!(drift_and_sweep(&mut world, 25.0), vec![apple.clone()]);

        assert!(world.offers.is_empty());
        // The giver keeps what they held out, and it is uncommitted again.
        assert_eq!(
            world.characters[&giver].holds(),
            std::slice::from_ref(&apple)
        );
        assert_eq!(world.uncommitted_quantity(&apple), 1);
        assert_eq!(world.world_revision, revision + 1);

        assert!(world.characters[&giver].inbox()[0].contains("is too far away now"));
        assert!(
            world.characters[&receiver].inbox()[0].contains("no longer on offer"),
            "the target hears it too: {:?}",
            world.characters[&receiver].inbox()
        );

        let events = world.drain_events();
        let kinds: Vec<&str> = events.iter().map(|event| event.kind.as_str()).collect();
        assert_eq!(kinds, ["lapse_offer"]);
        assert_eq!(events[0].actor_id.as_ref(), Some(&giver));
        assert_eq!(events[0].target_id.as_ref(), Some(&receiver));
        assert_eq!(events[0].item_id.as_ref(), Some(&apple));
        assert_eq!(events[0].recipient_ids, vec![giver, receiver]);
        world.assert_invariants();
    }

    #[test]
    fn a_lapse_frees_the_offered_units_the_giver_could_not_spend() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        offer_the_apple(&mut world, Some("receiver"));

        // The promise holds the apple hostage while it stands.
        let error =
            apply_action(&mut world, &giver, "eat", &json!({"item_id": "apple"})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::ItemCommitted);

        drift_and_sweep(&mut world, 25.0);
        apply_action(&mut world, &giver, "eat", &json!({"item_id": "apple"})).unwrap();
        world.assert_invariants();
    }

    #[test]
    fn an_offer_survives_right_up_to_the_lapse_radius() {
        let mut world = offer_world();
        let apple = ItemId::from_raw("apple");
        offer_the_apple(&mut world, Some("receiver"));

        // Inclusive at exactly 20 m, like `offered_to_you`'s own boundary.
        assert!(drift_and_sweep(&mut world, OFFER_LAPSE_RADIUS_M).is_empty());
        assert!(world.offers.contains_key(&apple));
        assert!(world.drain_events().is_empty());

        assert_eq!(
            drift_and_sweep(&mut world, OFFER_LAPSE_RADIUS_M + 0.01),
            vec![apple]
        );
    }

    #[test]
    fn a_broadcast_offer_never_lapses_on_distance() {
        let mut world = offer_world();
        let apple = ItemId::from_raw("apple");
        offer_the_apple(&mut world, None);

        // It names nobody to drift from, and it travels with the giver: whoever
        // stands beside them can still take it.
        assert!(drift_and_sweep(&mut world, 400.0).is_empty());
        assert!(world.offers.contains_key(&apple));
        assert!(world.drain_events().is_empty());
    }

    #[test]
    fn a_lapse_gives_the_player_structured_feedback_but_no_prose() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        world.characters.get_mut(&receiver).unwrap().sheet.control = Control::Player;
        offer_the_apple(&mut world, Some("receiver"));

        drift_and_sweep(&mut world, 25.0);

        // No inbox prose for a player — but the HUD's toast is keyed on the
        // recipient list, so he must still be on it.
        assert!(world.characters[&receiver].inbox().is_empty());
        let events = world.drain_events();
        assert_eq!(events[0].kind, "lapse_offer");
        assert_eq!(events[0].recipient_ids, vec![giver, receiver]);
    }

    #[test]
    fn a_departed_party_to_an_offer_is_left_to_the_gates_own_expiry() {
        let mut world = offer_world();
        let receiver = ActorId::from_raw("receiver");
        let apple = ItemId::from_raw("apple");
        offer_the_apple(&mut world, Some("receiver"));
        world.characters.get_mut(&receiver).unwrap().state.presence =
            crate::Presence::BeyondTheWalls;

        // Someone outside the walls has no distance worth judging; the road
        // party's own rules already expired this at the gate.
        assert!(drift_and_sweep(&mut world, 25.0).is_empty());
        assert!(world.offers.contains_key(&apple));
    }

    /// giver@0 holds the apple, receiver@3, other@2 — Python's OfferTests world.
    fn displacement_world() -> World {
        let mut world = World::new();
        let mut giver = character("giver", "Giver", 0.0);
        giver.state.holds.push(ItemId::from_raw("apple"));
        world.add_character(giver);
        world.add_character(character("receiver", "Receiver", 3.0));
        world.add_character(character("other", "Other", 2.0));
        world.add_item(Item::new(ItemId::from_raw("apple"), "apple"));
        world
    }

    #[test]
    fn reoffering_displaces_the_old_target_with_a_retract_event_before_the_new_offer() {
        let mut world = displacement_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        let other = ActorId::from_raw("other");
        let apple = ItemId::from_raw("apple");

        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "receiver"}),
        )
        .unwrap();
        world.drain_events();
        world
            .characters
            .get_mut(&receiver)
            .unwrap()
            .state
            .inbox
            .clear();

        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "other"}),
        )
        .unwrap();

        assert!(world.characters[&receiver].inbox()[0].contains("withdrew"));
        assert_eq!(world.offers[&apple].target_id.as_ref(), Some(&other));

        // The jilted target's retract lands BEFORE the new offer, and they are
        // its sole recipient.
        let events = world.drain_events();
        let kinds: Vec<&str> = events.iter().map(|event| event.kind.as_str()).collect();
        assert_eq!(kinds, ["retract_offer", "offer_item"]);
        assert_eq!(events[0].target_id.as_ref(), Some(&receiver));
        assert_eq!(events[0].recipient_ids, vec![receiver.clone()]);
        // The item never moved: it stays with the giver until accepted.
        assert_eq!(world.characters[&giver].holds(), [apple]);
    }

    #[test]
    fn a_displaced_player_gets_structured_feedback_but_no_prose() {
        let mut world = displacement_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        world.characters.get_mut(&receiver).unwrap().sheet.control = Control::Player;

        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "receiver"}),
        )
        .unwrap();
        world.drain_events();

        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "other"}),
        )
        .unwrap();

        assert!(world.characters[&receiver].inbox().is_empty());
        let events = world.drain_events();
        assert_eq!(events[0].kind, "retract_offer");
        assert_eq!(events[0].recipient_ids, vec![receiver.clone()]);
        assert_eq!(events[1].kind, "offer_item");
        assert!(events[1].recipient_ids.contains(&receiver));
    }

    #[test]
    fn eating_an_offered_item_fails_without_displacing_the_promise() {
        let mut world = displacement_world();
        let giver = ActorId::from_raw("giver");
        let receiver = ActorId::from_raw("receiver");
        let apple = ItemId::from_raw("apple");

        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "receiver"}),
        )
        .unwrap();
        world.drain_events();
        world.characters.get_mut(&receiver).unwrap().state.inbox.clear();

        let error =
            apply_action(&mut world, &giver, "eat", &json!({"item_id": "apple"})).unwrap_err();

        assert_eq!(error.code, ActionErrorCode::ItemCommitted);
        assert!(world.items.contains_key(&apple));
        assert!(world.offers.contains_key(&apple));
        assert_eq!(world.characters[&giver].holds(), [apple]);
        assert!(world.characters[&receiver].inbox().is_empty());
        assert!(world.drain_events().is_empty());
        world.assert_invariants();
    }

    #[test]
    fn eating_uses_only_the_uncommitted_part_of_a_partially_offered_stack() {
        let mut world = displacement_world();
        let giver = ActorId::from_raw("giver");
        let apple = ItemId::from_raw("apple");
        world.items.get_mut(&apple).unwrap().quantity = 2;

        apply_action(
            &mut world,
            &giver,
            "offer_item",
            &json!({"item_id": "apple", "target": "receiver", "quantity": 1}),
        )
        .unwrap();
        apply_action(&mut world, &giver, "eat", &json!({"item_id": "apple"})).unwrap();

        assert_eq!(world.items[&apple].quantity, 1);
        assert_eq!(world.offers[&apple].quantity, 1);
        let error =
            apply_action(&mut world, &giver, "eat", &json!({"item_id": "apple"})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::ItemCommitted);
        assert_eq!(world.items[&apple].quantity, 1);
        assert_eq!(world.offers[&apple].quantity, 1);
        world.assert_invariants();
    }

    #[test]
    fn drinking_refills_thirst_and_is_narrated_as_drinking() {
        let mut world = displacement_world();
        let drinker = ActorId::from_raw("giver");
        let ale = ItemId::from_raw("ale01");
        world.add_item(Item::new(ale.clone(), "ale"));
        world
            .characters
            .get_mut(&drinker)
            .unwrap()
            .state
            .holds
            .push(ale.clone());
        {
            let needs = &mut world.characters.get_mut(&drinker).unwrap().state.needs;
            needs.thirst = 40.0;
            needs.hunger = 40.0;
        }

        let result =
            apply_action(&mut world, &drinker, "eat", &json!({"item_id": "ale01"})).unwrap();

        assert_eq!(result, "Giver drinks the pot of ale");
        let needs = &world.characters[&drinker].state.needs;
        assert_eq!(needs.thirst, 200.0, "ale quenches 160");
        assert_eq!(needs.hunger, 65.0, "ale feeds a little (25)");
        assert!(!world.items.contains_key(&ale));
        // The hearers' percept uses the same verb.
        assert!(
            world.characters[&ActorId::from_raw("receiver")]
                .inbox()
                .iter()
                .any(|line| line.contains("drank a pot of ale")),
            "hearers hear a drink, not a meal"
        );
        world.assert_invariants();
    }

    #[test]
    fn the_interaction_boundary_is_inclusive_at_four_metres() {
        for (distance, expected) in [
            (ITEM_INTERACTION_RADIUS_M - 1e-6, true),
            (ITEM_INTERACTION_RADIUS_M, true),
            (ITEM_INTERACTION_RADIUS_M + 1e-6, false),
        ] {
            let mut world = offer_world();
            let giver = ActorId::from_raw("giver");
            let receiver = ActorId::from_raw("receiver");
            world
                .characters
                .get_mut(&receiver)
                .unwrap()
                .state
                .position_m = Vec3::new(distance, 0.0, 0.0);

            let result = apply_action(
                &mut world,
                &giver,
                "offer_item",
                &json!({"item_id": "apple", "target": "receiver"}),
            );

            match (result, expected) {
                (Ok(_), true) => {}
                (Err(error), false) => {
                    assert_eq!(error.code, ActionErrorCode::OutOfRange);
                    assert!(error.message.contains("more than 4 metres"));
                }
                (result, _) => panic!("distance {distance} gave {result:?}"),
            }
        }
    }

    #[test]
    fn the_snapshot_is_public_and_renders_strangers_by_id() {
        let mut world = displacement_world();
        let giver = ActorId::from_raw("giver");
        // The receiver knows the giver; nobody else knows anyone.
        world
            .characters
            .get_mut(&ActorId::from_raw("receiver"))
            .unwrap()
            .state
            .knows
            .insert(giver.clone());
        apply_action(
            &mut world,
            &giver,
            "set_goal",
            &json!({"goal": "a secret plan"}),
        )
        .unwrap();

        let snapshot = world.public_snapshot(&ActorId::from_raw("receiver"));
        let serialized = serde_json::to_string(&snapshot).unwrap();

        for private in [
            "back_story",
            "memories",
            "goal",
            "secret plan",
            "voice_key",
            "inbox",
            "recent_history",
            "pending_history",
            "knows",
        ] {
            assert!(!serialized.contains(private), "snapshot leaked {private}");
        }
        let names: Vec<&str> = snapshot
            .actors
            .iter()
            .map(|actor| actor.name_for_player.as_str())
            .collect();
        // Sorted by id: giver, other, receiver.
        assert_eq!(names, ["Giver", "a stranger (id other)", "You"]);
    }

    #[test]
    fn make_sound_hides_non_emittable_rows_behind_the_unknown_sound_error() {
        let mut world = offer_world();
        world.sound_catalog = SoundCatalog::new(
            vec![
                Sound::new(
                    "fart",
                    "body",
                    20.0,
                    "[You heard a big fart!]",
                    Some("{actor} farted.".into()),
                    "prompt",
                    1.5,
                    true,
                )
                .unwrap(),
                Sound::new(
                    "town_bell",
                    "bell",
                    600.0,
                    "[The town bell is ringing.]",
                    None,
                    "prompt",
                    9.0,
                    false,
                )
                .unwrap(),
            ],
            vec![],
        )
        .unwrap();
        let giver = ActorId::from_raw("giver");

        let receiver = ActorId::from_raw("receiver");
        // The receiver faces -Z by default, with the giver off to the -X side:
        // out of the cone, so the sound is heard but not attributed.
        let line =
            apply_action(&mut world, &giver, "make_sound", &json!({"sound": "fart"})).unwrap();
        assert_eq!(line, "Giver farted.");
        assert_eq!(
            world.characters[&receiver].inbox(),
            ["[You heard a big fart!]"]
        );

        // Turned towards the giver they witness it — and, not knowing them,
        // attribute it to a stranger by id.
        world
            .characters
            .get_mut(&receiver)
            .unwrap()
            .state
            .facing_yaw = std::f64::consts::FRAC_PI_2;
        apply_action(&mut world, &giver, "make_sound", &json!({"sound": "fart"})).unwrap();
        assert_eq!(
            world.characters[&receiver].inbox().last().unwrap(),
            "A stranger (id giver) farted."
        );
        // The emitter remembers their own act, with an empty inbox — the
        // repeat coalesced into one counted history entry.
        assert_eq!(
            world.characters[&giver].recent_history(),
            ["You farted. (2 times now)"]
        );
        assert!(world.characters[&giver].inbox().is_empty());

        for value in [json!("burp"), json!("town_bell"), json!(null)] {
            let error = apply_action(&mut world, &giver, "make_sound", &json!({"sound": value}))
                .unwrap_err();
            assert_eq!(error.code, ActionErrorCode::UnknownSound);
        }
        // `{sound_value!r}` is a *Python* repr: a list is `['fart']`, not JSON's
        // `["fart"]`, and the message is model-visible.
        for (value, expected) in [
            (json!("burp"), "there is no sound 'burp'"),
            (json!(null), "there is no sound None"),
            (json!(["fart"]), "there is no sound ['fart']"),
            (json!({"id": "fart"}), "there is no sound {'id': 'fart'}"),
            (json!(3), "there is no sound 3"),
        ] {
            let error = apply_action(&mut world, &giver, "make_sound", &json!({"sound": value}))
                .unwrap_err();
            assert_eq!(error.message, expected);
        }

        world.sounds_enabled = false;
        let error =
            apply_action(&mut world, &giver, "make_sound", &json!({"sound": "fart"})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::SoundsDisabled);
        assert!(error.message.contains("disabled"));
    }

    #[test]
    fn goal_none_sentinel_drops_the_goal() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");

        let line = apply_action(&mut world, &giver, "set_goal", &json!({"goal": "eat"})).unwrap();
        assert_eq!(line, "Giver now wants: eat");

        // The literal string "None" drops it, exactly like JSON null (D15).
        let line =
            apply_action(&mut world, &giver, "set_goal", &json!({"goal": " None "})).unwrap();
        assert_eq!(line, "Giver drops their goal");
        assert_eq!(world.characters[&giver].goal(), GOAL_NONE);

        apply_action(&mut world, &giver, "set_goal", &json!({"goal": "eat"})).unwrap();
        apply_action(&mut world, &giver, "set_goal", &json!({"goal": null})).unwrap();
        assert_eq!(world.characters[&giver].goal(), GOAL_NONE);
    }

    #[test]
    fn forget_falls_back_to_a_substring_match_and_echoes_the_stored_text() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        apply_action(
            &mut world,
            &giver,
            "remember",
            &json!({"memory": "The pilgrim with id k0fb1 is called Ilse"}),
        )
        .unwrap();
        // Duplicates are not stored twice.
        apply_action(
            &mut world,
            &giver,
            "remember",
            &json!({"memory": "The pilgrim with id k0fb1 is called Ilse"}),
        )
        .unwrap();
        assert_eq!(world.characters[&giver].memories().len(), 1);

        let line = apply_action(&mut world, &giver, "forget", &json!({"memory": "k0fb1"})).unwrap();
        assert_eq!(
            line,
            "Giver forgets: The pilgrim with id k0fb1 is called Ilse"
        );
        assert!(world.characters[&giver].memories().is_empty());

        let line =
            apply_action(&mut world, &giver, "forget", &json!({"memory": "nothing"})).unwrap();
        assert_eq!(
            line,
            "Giver tried to forget something they never knew: nothing"
        );
    }

    /// A held handle in a world with no street graph fails `no_route` — the
    /// route is priced at intent time, so an unroutable errand never starts
    /// (`features/implemented/movement/05_the_llm_seam.md` §2). And `stop` with nothing to
    /// stop is a harmless line, not an error.
    #[test]
    fn go_to_without_a_graph_is_no_route_and_stop_is_always_safe() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        // A home entry needs no nav graph to register.
        let home_id = world
            .places
            .add_home(&ActorId::from_raw("receiver"), "Receiver", Vec3::new(9.0, 0.0, 0.0));
        world
            .characters
            .get_mut(&giver)
            .unwrap()
            .state
            .places_known
            .insert(home_id.clone());

        let error = apply_action(
            &mut world,
            &giver,
            "go_to",
            &json!({"place_id": home_id.as_str()}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::NoRoute);
        assert!(
            error.message.contains("Receiver's house"),
            "the refusal names the place: {}",
            error.message
        );
        assert!(world.characters[&giver].state.intent.is_none());

        // An unheld handle is unknown_place even though the registry names it.
        let error = apply_action(
            &mut world,
            &ActorId::from_raw("receiver"),
            "go_to",
            &json!({"place_id": home_id.as_str()}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownPlace);

        let line = apply_action(&mut world, &giver, "stop", &json!({})).unwrap();
        assert_eq!(line, "Giver has no errand to abandon");
    }

    #[test]
    fn wait_is_a_no_op_and_unknown_verbs_are_reported_last() {
        let mut world = offer_world();
        let giver = ActorId::from_raw("giver");
        let revision = world.world_revision;

        assert_eq!(
            apply_action(&mut world, &giver, "wait", &json!({})).unwrap(),
            "Giver waits"
        );
        assert!(world.drain_events().is_empty());
        assert_eq!(world.world_revision, revision);
        assert_eq!(
            apply_action(&mut world, &giver, "wait", &json!({"x": 1}))
                .unwrap_err()
                .code,
            ActionErrorCode::InvalidArguments
        );

        // `dance` is now a gesture *kind* (`gesture {"kind": "dance"}`), not a
        // bare verb (npc_bodies M4); a bare `dance` verb is still unknown, and
        // an unknown verb is still reported only after arg validation.
        let error = apply_action(&mut world, &giver, "dance", &json!({})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownVerb);
        assert_eq!(error.message, "unknown verb: dance");

        let error =
            apply_action(&mut world, &ActorId::from_raw("ghost"), "wait", &json!({})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownActor);
    }

    // ----------------------------------------------------------- gestures (M4)

    #[test]
    fn a_targeted_wave_reaches_the_target_the_bystander_and_the_actors_own_history() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        // The waver knows the target by name; nobody else knows anyone, so the
        // one test exercises both the known and the stranger renderings.
        world
            .characters
            .get_mut(&speaker)
            .unwrap()
            .state
            .knows
            .insert(ActorId::from_raw("target"));
        let revision = world.world_revision;

        let line = apply_action(
            &mut world,
            &speaker,
            "gesture",
            &json!({"kind": "wave", "to": "target"}),
        )
        .unwrap();

        assert_eq!(line, "Speaker waves at Target.");
        // The target sees "you"; a nearby bystander sees the third person; the
        // actor remembers their own act in the second person.
        assert_eq!(
            world.characters[&ActorId::from_raw("target")]
                .inbox()
                .last()
                .unwrap(),
            "A stranger (id speaker) waves at you."
        );
        assert_eq!(
            world.characters[&ActorId::from_raw("bystander")]
                .inbox()
                .last()
                .unwrap(),
            "A stranger (id speaker) waves at a stranger (id target)."
        );
        assert_eq!(
            world.characters[&speaker].recent_history().last().unwrap(),
            "You wave at Target."
        );
        assert!(
            world.characters[&ActorId::from_raw("distant")]
                .inbox()
                .is_empty()
        );

        // Transient like speech: an event with the witnesses, no public-state
        // bump (a non-looping gesture is not snapshot state).
        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.event_type, crate::event::EventType::Gesture);
        assert_eq!(event.kind, "wave");
        assert_eq!(event.target_id, Some(ActorId::from_raw("target")));
        assert_eq!(
            event.recipient_ids,
            vec![ActorId::from_raw("bystander"), ActorId::from_raw("target")]
        );
        assert_eq!(world.world_revision, revision);
    }

    #[test]
    fn an_untargeted_wave_reads_as_a_wave_to_everyone_nearby() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");

        let line = apply_action(&mut world, &speaker, "gesture", &json!({"kind": "wave"})).unwrap();
        assert_eq!(line, "Speaker waves.");
        assert_eq!(
            world.characters[&ActorId::from_raw("bystander")]
                .inbox()
                .last()
                .unwrap(),
            "A stranger (id speaker) waves."
        );
        assert_eq!(
            world.characters[&speaker].recent_history().last().unwrap(),
            "You wave."
        );
        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.target_id, None);
    }

    #[test]
    fn an_unknown_gesture_kind_is_a_standard_action_error() {
        let mut world = speech_world();
        let error = apply_action(
            &mut world,
            &ActorId::from_raw("speaker"),
            "gesture",
            &json!({"kind": "boogie"}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownGesture);
        assert!(error.message.contains("boogie"), "{}", error.message);
    }

    #[test]
    fn required_target_gestures_demand_a_target_and_no_target_gestures_reject_one() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");

        // beckon and point require a target.
        for kind in ["beckon", "point"] {
            let error = apply_action(&mut world, &speaker, "gesture", &json!({"kind": kind}))
                .unwrap_err();
            assert_eq!(error.code, ActionErrorCode::InvalidArguments, "{kind}");
        }
        // shrug and dance take none.
        for kind in ["shrug", "dance"] {
            let error = apply_action(
                &mut world,
                &speaker,
                "gesture",
                &json!({"kind": kind, "to": "target"}),
            )
            .unwrap_err();
            assert_eq!(error.code, ActionErrorCode::InvalidArguments, "{kind}");
        }
    }

    #[test]
    fn a_gesture_target_must_exist_be_in_range_and_not_be_the_actor() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        for (target, code) in [
            ("nobody", ActionErrorCode::UnknownTarget),
            ("speaker", ActionErrorCode::SelfTarget),
            ("distant", ActionErrorCode::OutOfRange),
        ] {
            let error = apply_action(
                &mut world,
                &speaker,
                "gesture",
                &json!({"kind": "beckon", "to": target}),
            )
            .unwrap_err();
            assert_eq!(error.code, code, "{target}");
        }
        // A failed gesture leaves the world untouched.
        assert!(world.drain_events().is_empty());
    }

    #[test]
    fn dance_sets_active_gesture_and_the_next_non_wait_action_ends_it() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        let revision = world.world_revision;

        let line = apply_action(&mut world, &speaker, "gesture", &json!({"kind": "dance"})).unwrap();
        assert_eq!(line, "Speaker is dancing.");
        assert_eq!(
            world.characters[&speaker].active_gesture(),
            Some(GestureKind::Dance)
        );
        // Setting the loop is public state — the revision bumped.
        assert!(world.world_revision > revision);
        let after_dance = world.world_revision;

        // A `wait` does not end the loop…
        apply_action(&mut world, &speaker, "wait", &json!({})).unwrap();
        assert_eq!(
            world.characters[&speaker].active_gesture(),
            Some(GestureKind::Dance)
        );
        assert_eq!(world.world_revision, after_dance);

        // …but the next non-`wait` action does, bumping the revision again.
        apply_action(&mut world, &speaker, "gesture", &json!({"kind": "wave"})).unwrap();
        assert_eq!(world.characters[&speaker].active_gesture(), None);
        assert!(world.world_revision > after_dance);
    }

    #[test]
    fn point_accepts_a_known_place_handle_and_names_it() {
        let mut world = speech_world();
        let speaker = ActorId::from_raw("speaker");
        let place_id = world
            .places
            .add_home(&ActorId::from_raw("target"), "Target", Vec3::new(4.0, 0.0, 0.0));
        world
            .characters
            .get_mut(&speaker)
            .unwrap()
            .state
            .places_known
            .insert(place_id.clone());

        let line = apply_action(
            &mut world,
            &speaker,
            "gesture",
            &json!({"kind": "point", "to": place_id.as_str()}),
        )
        .unwrap();
        assert_eq!(line, "Speaker points toward Target's house.");
        assert_eq!(
            world.characters[&speaker].recent_history().last().unwrap(),
            "You point toward Target's house."
        );
        // A place-pointed gesture carries no person target on the event.
        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.target_id, None);
        assert_eq!(event.kind, "point");

        // An unheld place id falls through to person resolution and misses.
        let stray = world
            .places
            .add_home(&ActorId::from_raw("bystander"), "Bystander", Vec3::new(3.0, 0.0, 0.0));
        let error = apply_action(
            &mut world,
            &speaker,
            "gesture",
            &json!({"kind": "point", "to": stray.as_str()}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownTarget);
    }

    // ------------------------------------------------- ward notices (M3)

    /// `character` with a lore profile: an occupation and an authored
    /// curiosity. `Some(0.0)` never carries gossip, `Some(1.0)` always does —
    /// the authored number is clamped but otherwise the last word, which is
    /// what makes the carry roll assertable.
    fn lored(id: &str, name: &str, x: f64, occupation: &str, curiosity: Option<f64>) -> Character {
        let mut person = character(id, name, x);
        person.sheet.lore = Some(crate::lore::LoreProfile {
            significance: crate::Significance::Ambient,
            planning_ward: crate::lore::PlanningWard::Fabric,
            age: 30,
            gender: "m".into(),
            occupation_id: Some(occupation.into()),
            occupation_display: None,
            title: None,
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "Fabric".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            home: None,
            core_character_description: String::new(),
            extended_character_description: String::new(),
            curiosity,
        });
        person
    }

    /// A sergeant, a second officer, a talkative and a taciturn citizen, a
    /// thief holding the taking (and a loaf to trade honestly with), and the
    /// wronged boy.
    fn ward_world() -> World {
        let mut world = World::new();
        world.add_character(lored("srgnt", "Sergeant", 0.0, "bailiff_and_gaoler", Some(0.0)));
        world.add_character(lored("gatek", "Gatekeeper", 5.0, "watchman_and_keeper", Some(0.0)));
        world.add_character(lored("gossp", "Gossip", 5.0, "baker", Some(1.0)));
        world.add_character(lored("quiet", "Quiet", 5.0, "baker", Some(0.0)));
        let mut thief = lored("thief", "Thief", 2.0, "carter", Some(0.0));
        thief.state.holds.push(ItemId::from_raw("spark"));
        thief.state.holds.push(ItemId::from_raw("loafx"));
        world.add_character(thief);
        world.add_character(lored("wrngd", "Wronged", 3.0, "tenter_boy", Some(0.0)));
        world.add_item(Item::new(ItemId::from_raw("spark"), "spark"));
        world.add_item(Item::new(ItemId::from_raw("loafx"), "loaf"));
        world
    }

    fn raise_args() -> Value {
        json!({
            "about": "an outland stranger in a grey hood",
            "deed": "took a boy's spark and gave no badge",
            "where": "the tenter-frames",
            "accused": "thief",
            "wronged": "wrngd",
        })
    }

    /// The same word with the taking named — the one arg that lets a return
    /// settle itself, with nobody judging anything (M3.5).
    fn raise_args_naming_the_taking() -> Value {
        let mut args = raise_args();
        args["taken"] = json!("spark");
        args
    }

    /// The sergeant puts the word in the ward.
    fn raise_the_word(world: &mut World, args: &Value) {
        apply_action(world, &ActorId::from_raw("srgnt"), "raise_notice", args).unwrap();
    }

    /// `giver` holds `item` out and `taker` takes it.
    fn hand_over(world: &mut World, giver: &str, taker: &str, item: &str) {
        apply_action(
            world,
            &ActorId::from_raw(giver),
            "offer_item",
            &json!({"item_id": item, "target": taker}),
        )
        .unwrap();
        apply_action(
            world,
            &ActorId::from_raw(taker),
            "accept_offered_item",
            &json!({"item_id": item}),
        )
        .unwrap();
    }

    fn inbox_has(world: &World, id: &str, prefix: &str) -> bool {
        world.characters[&ActorId::from_raw(id)]
            .inbox()
            .iter()
            .any(|line| line.starts_with(prefix))
    }

    #[test]
    fn raise_notice_is_law_only_and_the_word_reaches_carriers() {
        let mut world = ward_world();

        // A baker has no standing to raise the ward.
        let error = apply_action(
            &mut world,
            &ActorId::from_raw("gossp"),
            "raise_notice",
            &raise_args(),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::InvalidAction);

        let line = apply_action(
            &mut world,
            &ActorId::from_raw("srgnt"),
            "raise_notice",
            &raise_args(),
        )
        .unwrap();
        assert!(line.contains("Sergeant puts the word in the ward"), "{line}");
        assert_eq!(world.notices.live().len(), 1);
        let notice = &world.notices.live()[0];
        assert_eq!(
            notice.line(),
            "an outland stranger in a grey hood — took a boy's spark and gave no badge, at the tenter-frames"
        );

        // The law always carries the word — distance is no object — and the
        // talkative carry it too; the taciturn are spared. The raiser keeps it
        // in their own history, not their inbox.
        let heard = |id: &str| {
            world.characters[&ActorId::from_raw(id)]
                .inbox()
                .iter()
                .any(|line| line.starts_with("word in the ward: "))
        };
        assert!(heard("gatek"));
        assert!(heard("gossp"));
        assert!(!heard("quiet"));
        assert!(!heard("srgnt"));
        assert!(
            world.characters[&ActorId::from_raw("srgnt")]
                .recent_history()
                .last()
                .unwrap()
                .starts_with("You put the word in the ward"),
        );

        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.kind, "raise_notice");
        assert_eq!(event.target_id, Some(ActorId::from_raw("thief")));
    }

    #[test]
    fn raise_notice_validates_its_people() {
        let mut world = ward_world();
        let sergeant = ActorId::from_raw("srgnt");

        let error = apply_action(
            &mut world,
            &sergeant,
            "raise_notice",
            &json!({"about": "a man", "deed": "a wrong", "accused": "nobody"}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownTarget);

        let error = apply_action(
            &mut world,
            &sergeant,
            "raise_notice",
            &json!({"about": "a man", "deed": "a wrong", "accused": "srgnt"}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::SelfTarget);
        assert!(world.notices.is_empty());
    }

    // -------------------------------------- settlement as an act (M3.5)

    /// The whole of Problem 4 in one test: ordinary commerce with an accused no
    /// longer launders the wrong, and a fine into an officer's hand no longer
    /// absolves anybody by itself. What the officer gets instead is the
    /// question, and the notice stays live until they answer it.
    #[test]
    fn a_transfer_from_the_accused_never_settles_the_word_by_itself() {
        let mut world = ward_world();
        raise_the_word(&mut world, &raise_args_naming_the_taking());

        // The talkative baker buys the thief's loaf, as anyone might.
        hand_over(&mut world, "thief", "gossp", "loafx");
        assert_eq!(
            world.notices.live().len(),
            1,
            "ordinary commerce cannot launder a theft"
        );
        assert!(
            !inbox_has(&world, "gossp", "what you were just handed"),
            "a bystander is asked nothing - they have no standing to settle"
        );

        // The thief pays the gate keeper the very spark the notice names. Even
        // that settles nothing: the wronged is the boy, and whether a spark in
        // an officer's palm answers the wrong is the officer's to judge.
        hand_over(&mut world, "thief", "gatek", "spark");
        assert_eq!(
            world.notices.live().len(),
            1,
            "a fine is an offer of restitution, not an absolution"
        );
        assert!(
            inbox_has(&world, "gatek", "what you were just handed"),
            "the officer is asked whether this answers the word"
        );
        let asked = world.characters[&ActorId::from_raw("gatek")]
            .inbox()
            .iter()
            .find(|line| line.starts_with("what you were just handed"))
            .unwrap();
        assert!(
            asked.contains("(notice 1)") && asked.contains("settle_notice"),
            "it names the notice and the verb: {asked}"
        );
    }

    /// The verb itself: one notice, chosen, and the ward hears that word die.
    #[test]
    fn settle_notice_clears_exactly_the_notice_it_names() {
        let mut world = ward_world();
        raise_the_word(&mut world, &raise_args());
        // A second wrong by the same man — settling one may not settle both.
        raise_the_word(
            &mut world,
            &json!({
                "about": "the same carter",
                "deed": "fouled the street",
                "accused": "thief",
            }),
        );
        assert_eq!(world.notices.live().len(), 2);

        let line = apply_action(
            &mut world,
            &ActorId::from_raw("gatek"),
            "settle_notice",
            &json!({"notice_id": 1}),
        )
        .unwrap();
        assert!(line.starts_with("Gatekeeper settles the ward's word"), "{line}");

        let live = world.notices.live();
        assert_eq!(live.len(), 1, "never a blanket clear");
        assert_eq!(live[0].id, 2, "the other wrong is still going around");
        assert!(
            inbox_has(&world, "gossp", "the ward's word is settled, the wrong answered"),
            "the carriers hear it die"
        );
        assert!(
            !inbox_has(&world, "gatek", "the ward's word is settled"),
            "the settler is not told their own news"
        );
        assert!(
            world.characters[&ActorId::from_raw("gatek")]
                .recent_history()
                .last()
                .unwrap()
                .starts_with("You settled the ward's word"),
        );
        let event = world.drain_events().pop().unwrap();
        assert_eq!(event.kind, "settle_notice");
        assert_eq!(event.target_id, Some(ActorId::from_raw("thief")));
    }

    /// Standing: the law, and the one who was wronged. Nobody else — a
    /// bystander cannot forgive a spark that was never theirs.
    #[test]
    fn only_the_law_and_the_wronged_may_settle_a_word() {
        let mut world = ward_world();
        raise_the_word(&mut world, &raise_args());

        let error = apply_action(
            &mut world,
            &ActorId::from_raw("gossp"),
            "settle_notice",
            &json!({"notice_id": 1}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::InvalidAction);
        assert_eq!(world.notices.live().len(), 1);

        let error = apply_action(
            &mut world,
            &ActorId::from_raw("srgnt"),
            "settle_notice",
            &json!({"notice_id": 7}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownTarget);

        // The boy is no officer, but it is his spark to forgive.
        apply_action(
            &mut world,
            &ActorId::from_raw("wrngd"),
            "settle_notice",
            &json!({"notice_id": 1}),
        )
        .unwrap();
        assert!(world.notices.is_empty());
    }

    /// The first settlement no verb can reach: the accused hands back the very
    /// thing the notice names, to the person it was taken from. Nothing to
    /// judge, so nobody is asked to judge it — and it needs `taken` to have
    /// been recorded, or the boy is merely being handed something.
    #[test]
    fn returning_the_named_taking_settles_the_word_without_the_verb() {
        let mut world = ward_world();
        raise_the_word(&mut world, &raise_args_naming_the_taking());
        hand_over(&mut world, "thief", "wrngd", "spark");
        assert!(world.notices.is_empty(), "the taking is back where it belongs");
        assert!(
            inbox_has(&world, "gatek", "the ward's word is settled, restitution made"),
            "the carriers hear the word die"
        );

        // Without `taken`, the same handover is only an offer of restitution:
        // whether a loaf answers a stolen spark is the boy's to say.
        let mut world = ward_world();
        raise_the_word(&mut world, &raise_args());
        hand_over(&mut world, "thief", "wrngd", "loafx");
        assert_eq!(world.notices.live().len(), 1);
        assert!(
            inbox_has(&world, "wrngd", "what you were just handed"),
            "the wronged is asked, and holds the verb to answer with"
        );
    }

    /// The second: the player as the wronged party. He has no verbs at all, so
    /// an NPC's restitution to him must settle mechanically or never settle.
    #[test]
    fn the_player_accepting_restitution_settles_the_word() {
        let mut world = ward_world();
        world
            .characters
            .get_mut(&ActorId::from_raw("wrngd"))
            .unwrap()
            .sheet
            .control = Control::Player;
        // No `taken` recorded, and the loaf is not the spark: for the player it
        // settles anyway, because nothing else ever could.
        raise_the_word(&mut world, &raise_args());
        hand_over(&mut world, "thief", "wrngd", "loafx");

        assert!(world.notices.is_empty(), "the player's acceptance is the answer");
        assert!(
            !inbox_has(&world, "wrngd", "what you were just handed"),
            "and he is never asked a question he has no verb to answer"
        );
    }

    // ------------------------------------------- body pockets (extra_pockets.md)

    /// A carrier at 0 with three sparks, a loaf, a pot of ale and an apple; a
    /// watcher at 2 and a nosey neighbour at 5 (both strangers to everyone), and
    /// one soul 30 m off, out of every radius.
    fn pocket_world() -> World {
        let mut world = World::new();
        let mut carrier = character("carry", "Carrier", 0.0);
        for id in ["sparks", "loafx", "alepot", "applex"] {
            carrier.state.holds.push(ItemId::from_raw(id));
        }
        world.add_character(carrier);
        world.add_character(character("watch", "Watcher", 2.0));
        world.add_character(character("nosey", "Nosey", 5.0));
        world.add_character(character("faroff", "Faroff", 30.0));
        world.add_item(Item::stack(ItemId::from_raw("sparks"), "spark", 3));
        world.add_item(Item::new(ItemId::from_raw("loafx"), "loaf"));
        world.add_item(Item::new(ItemId::from_raw("alepot"), "ale"));
        world.add_item(Item::new(ItemId::from_raw("applex"), "apple"));
        world
    }

    fn only_pocketed(world: &World, actor_id: &ActorId) -> (crate::character::BodySlot, ItemId) {
        let snapshot = world.characters[actor_id].pocket_snapshot();
        assert_eq!(snapshot.len(), 1, "expected exactly one pocketed unit");
        snapshot[0].clone()
    }

    fn condition_of(world: &World, item_id: &ItemId) -> Option<String> {
        world.items[item_id]
            .metadata
            .get(CONDITION_METADATA_KEY)
            .cloned()
    }

    /// The classic cutpurse defence (`extra_pockets.md`, the fun list): a coin
    /// in the cheek forks the stack wet, cannot be spent while it rides there,
    /// and comes back — still wet — the moment it is retrieved.
    #[test]
    fn a_cheeked_spark_forks_the_stack_wet_and_cannot_be_spent_until_retrieved() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");

        let line = apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "sparks", "slot": "mouth"}),
        )
        .unwrap();
        assert_eq!(line, "Carrier pockets the wet spark (mouth)");

        // Metadata is stack identity, so the wet unit left the dry stack.
        assert_eq!(world.items[&ItemId::from_raw("sparks")].quantity, 2);
        let (slot, wet_id) = only_pocketed(&world, &carrier);
        assert_eq!(slot, crate::character::BodySlot::Mouth);
        assert_eq!(condition_of(&world, &wet_id).as_deref(), Some("wet"));
        assert_eq!(world.items[&wet_id].quantity, 1);
        // A pocketed unit is a commitment, exactly like an offer promise.
        assert_eq!(world.uncommitted_quantity(&wet_id), 0);
        let error = apply_action(
            &mut world,
            &carrier,
            "offer_item",
            &json!({"item_id": wet_id.as_str(), "target": "watch"}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::ItemCommitted);

        // The transition is visible, and how much of it depends on where the
        // watcher stands: at two metres the coin itself is plain, at five it is
        // "something", at thirty there is nothing to hear about at all.
        let saw = |id: &str| {
            world.characters[&ActorId::from_raw(id)]
                .inbox()
                .last()
                .cloned()
        };
        assert_eq!(
            saw("watch").as_deref(),
            Some("A stranger (id carry) slipped a spark into their mouth")
        );
        assert_eq!(
            saw("nosey").as_deref(),
            Some("A stranger (id carry) slipped something into their mouth")
        );
        assert!(
            world.characters[&ActorId::from_raw("faroff")]
                .inbox()
                .is_empty()
        );

        apply_action(
            &mut world,
            &carrier,
            "retrieve_item",
            &json!({"item_id": wet_id.as_str()}),
        )
        .unwrap();
        assert!(world.characters[&carrier].pockets().is_empty());
        assert_eq!(condition_of(&world, &wet_id).as_deref(), Some("wet"));
        assert_eq!(world.uncommitted_quantity(&wet_id), 1);
        world.assert_invariants();
    }

    /// Every refusal the body has, and the house rule that a failed action
    /// leaves the world exactly as it found it.
    #[test]
    fn the_pocket_verbs_refuse_what_a_body_cannot_do_and_change_nothing() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        let revision = world.world_revision;

        for (verb, args, code) in [
            (
                "pocket_item",
                json!({"item_id": "loafx", "slot": "mouth"}),
                ActionErrorCode::TooBig,
            ),
            (
                "pocket_item",
                json!({"item_id": "sparks", "slot": "frontbutt"}),
                ActionErrorCode::WrongSlot,
            ),
            (
                "pocket_item",
                json!({"item_id": "sparks", "slot": "ear"}),
                ActionErrorCode::WrongSlot,
            ),
            (
                "pocket_item",
                json!({"item_id": "gh0st", "slot": "mouth"}),
                ActionErrorCode::NotOwner,
            ),
            (
                "retrieve_item",
                json!({"item_id": "sparks"}),
                ActionErrorCode::NotPocketed,
            ),
            (
                "swallow",
                json!({"item_id": "sparks"}),
                ActionErrorCode::NotPocketed,
            ),
            (
                "gargle",
                json!({"item_id": "alepot"}),
                ActionErrorCode::NotPocketed,
            ),
            ("expel", json!({}), ActionErrorCode::NothingToExpel),
            (
                "spit",
                json!({"item_id": "sparks", "target": "carry"}),
                ActionErrorCode::SelfTarget,
            ),
            (
                "spit",
                json!({"item_id": "sparks", "target": "nobody"}),
                ActionErrorCode::UnknownTarget,
            ),
            (
                "spit",
                json!({"item_id": "sparks", "target": "faroff"}),
                ActionErrorCode::OutOfRange,
            ),
        ] {
            let error = apply_action(&mut world, &carrier, verb, &args).unwrap_err();
            assert_eq!(error.code, code, "{verb} {args}");
        }

        assert!(world.characters.values().all(|c| c.inbox().is_empty()));
        assert!(world.characters[&carrier].pockets().is_empty());
        assert!(world.drain_events().is_empty());
        assert_eq!(world.world_revision, revision);
        world.assert_invariants();
    }

    /// Two in a cheek is the resolved capacity — and objectively funnier than
    /// one. The third simply does not fit.
    #[test]
    fn a_cheek_takes_two_units_and_the_third_does_not_fit() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        let cheek = json!({"item_id": "sparks", "slot": "mouth"});

        apply_action(&mut world, &carrier, "pocket_item", &cheek).unwrap();
        apply_action(&mut world, &carrier, "pocket_item", &cheek).unwrap();
        assert_eq!(
            world.characters[&carrier].pocketed_in_slot(crate::character::BodySlot::Mouth),
            2
        );
        // Both units merged into the one wet stack, and both are committed.
        let snapshot = world.characters[&carrier].pocket_snapshot();
        assert_eq!(snapshot[0].1, snapshot[1].1);
        assert_eq!(world.pocketed_quantity(&snapshot[0].1), 2);

        let error = apply_action(&mut world, &carrier, "pocket_item", &cheek).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::SlotFull);
        assert_eq!(error.message, "your mouth is full");
        world.assert_invariants();
    }

    /// Two-stage drinking: the mouthful reads to everyone as drinking (which is
    /// what makes cheeking a deception at all), and swallowing applies the very
    /// same satiety and thirst `eat` would have.
    #[test]
    fn a_mouthful_of_ale_reads_as_drinking_and_swallowing_it_feeds_the_drinker() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        {
            let needs = &mut world.characters.get_mut(&carrier).unwrap().state.needs;
            needs.hunger = 0.0;
            needs.thirst = 0.0;
        }

        let line = apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "alepot", "slot": "mouth"}),
        )
        .unwrap();
        assert_eq!(line, "Carrier pockets the pot of ale (mouth)");
        // A drink is named: from two metres this is a person having a drink.
        assert_eq!(
            world.characters[&ActorId::from_raw("watch")]
                .inbox()
                .last()
                .unwrap(),
            "A stranger (id carry) took a mouthful of a pot of ale"
        );
        // Nothing in your mouth can be eaten until you retrieve it.
        let error = apply_action(&mut world, &carrier, "eat", &json!({"item_id": "alepot"}))
            .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::ItemCommitted);

        let line = apply_action(&mut world, &carrier, "swallow", &json!({"item_id": "alepot"}))
            .unwrap();
        assert_eq!(line, "Carrier swallows the pot of ale");
        let carrier_character = &world.characters[&carrier];
        assert_eq!(carrier_character.needs().hunger, 25.0);
        assert_eq!(carrier_character.needs().thirst, 160.0);
        assert!(carrier_character.pockets().is_empty());
        assert!(!world.items.contains_key(&ItemId::from_raw("alepot")));
        // A clock-less world has no gut clock to start.
        assert!(carrier_character.state.gut.is_empty());
        world.assert_invariants();
    }

    /// The gut clock (M3): a meal queues one stool however many meals it takes,
    /// a swallowed inedible queues its own return, and both are stamped from
    /// the world clock — never a fresh draw.
    #[test]
    fn meals_coalesce_in_the_gut_and_a_swallowed_key_queues_its_own_return() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        world.add_item(Item::new(ItemId::from_raw("keyxx"), "key"));
        world
            .characters
            .get_mut(&carrier)
            .unwrap()
            .state
            .holds
            .push(ItemId::from_raw("keyxx"));
        world.current_time =
            Some(crate::clock::WorldClock::new(3600.0, crate::clock::Office::HighWick, 2, 0.05).at(0.0));
        let now_days = world.current_time.unwrap().day as f64 + world.current_time.unwrap().fraction;

        apply_action(&mut world, &carrier, "eat", &json!({"item_id": "applex"})).unwrap();
        apply_action(&mut world, &carrier, "eat", &json!({"item_id": "loafx"})).unwrap();
        let gut = world.characters[&carrier].state.gut.clone();
        assert_eq!(gut.len(), 1, "one stool brews, however many meals");
        assert_eq!(gut[0].kind.as_str(), "poop");
        assert!(gut[0].due_game_days >= now_days + crate::GUT_MIN_GAME_DAYS);
        assert!(
            gut[0].due_game_days
                <= now_days + crate::GUT_MIN_GAME_DAYS + crate::GUT_SPREAD_GAME_DAYS
        );

        // Swallowing the evidence: the key rides the gut on its own schedule.
        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "keyxx", "slot": "mouth"}),
        )
        .unwrap();
        let (_, wet_key) = only_pocketed(&world, &carrier);
        apply_action(
            &mut world,
            &carrier,
            "swallow",
            &json!({"item_id": wet_key.as_str()}),
        )
        .unwrap();
        let gut = world.characters[&carrier].state.gut.clone();
        assert_eq!(gut.len(), 2);
        assert_eq!(gut[1].kind.as_str(), "key");
        assert_eq!(
            gut[1].metadata.get(CONDITION_METADATA_KEY).map(String::as_str),
            Some(CONDITION_WET),
            "the mouth's wet stamp travels with it"
        );
        world.assert_invariants();
    }

    /// Spitting: a solid lands on the person you spat it at (they now hold a wet
    /// thing), a mouthful of drink is simply gone, and the square understands
    /// both.
    #[test]
    fn spitting_hands_a_solid_to_the_target_and_a_drink_is_gone() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        let watcher = ActorId::from_raw("watch");
        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "sparks", "slot": "mouth"}),
        )
        .unwrap();
        let (_, wet_id) = only_pocketed(&world, &carrier);

        let line = apply_action(
            &mut world,
            &carrier,
            "spit",
            &json!({"item_id": wet_id.as_str(), "target": "watch"}),
        )
        .unwrap();
        assert_eq!(line, "Carrier spits wet spark at Watcher");
        assert!(world.characters[&carrier].pockets().is_empty());
        assert_eq!(world.characters[&watcher].holds(), std::slice::from_ref(&wet_id));
        assert_eq!(condition_of(&world, &wet_id).as_deref(), Some("wet"));
        assert_eq!(
            world.characters[&watcher].inbox().last().unwrap(),
            "A stranger (id carry) spat a wet spark at you!"
        );
        assert_eq!(
            world.characters[&ActorId::from_raw("nosey")]
                .inbox()
                .last()
                .unwrap(),
            "A stranger (id carry) spat a wet spark at a stranger (id watch)"
        );

        // A mouthful of drink splashes and is gone; nobody gains a pot of ale.
        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "alepot", "slot": "mouth"}),
        )
        .unwrap();
        apply_action(
            &mut world,
            &carrier,
            "spit",
            &json!({"item_id": "alepot", "target": "watch"}),
        )
        .unwrap();
        assert!(!world.items.contains_key(&ItemId::from_raw("alepot")));
        assert_eq!(world.characters[&watcher].holds(), [wet_id]);
        world.assert_invariants();
    }

    /// The butt economy (M2/M3/M4): what joins a stool is stained, `expel`
    /// leaves the stool in the gutter and gives everything else back, and an
    /// officer in earshot puts it on the ward's tongues — in prose, with no ids.
    #[test]
    fn a_stool_stains_its_company_and_expelling_it_before_an_officer_raises_the_ward() {
        let mut world = pocket_world();
        world.add_character(lored(
            "srgnt",
            "Sergeant",
            6.0,
            "bailiff_and_gaoler",
            Some(0.0),
        ));
        let carrier = ActorId::from_raw("carry");
        let stool = ItemId::from_raw("turd1");

        // Seeded state: a stool already rides the breeches (the digest pass is
        // the engine's business; this verb only has to deal with the result).
        world.add_item(Item::new(stool.clone(), "poop"));
        {
            let state = &mut world.characters.get_mut(&carrier).unwrap().state;
            state.holds.push(stool.clone());
            state.pockets.push(PocketedUnit {
                slot: crate::character::BodySlot::Butt,
                item_id: stool.clone(),
            });
        }
        world.assert_invariants();

        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "applex", "slot": "butt"}),
        )
        .unwrap();
        let stained = world.characters[&carrier]
            .pocket_snapshot()
            .into_iter()
            .map(|(_, item_id)| item_id)
            .find(|item_id| world.items[item_id].kind.as_str() == "apple")
            .unwrap();
        assert_eq!(
            condition_of(&world, &stained).as_deref(),
            Some("poopstained"),
            "an apple that shares a slot with a stool does not come out clean"
        );

        let line = apply_action(&mut world, &carrier, "expel", &json!({})).unwrap();
        assert_eq!(line, "Carrier relieves themself where they stand");
        assert!(world.characters[&carrier].pockets().is_empty());
        assert!(!world.items.contains_key(&stool), "left in the gutter");
        assert!(
            world.characters[&carrier].holds().contains(&stained),
            "the apple was in hand all along; the reservation simply ended"
        );

        // The ward hears about it, from the officer's own eyes: a description,
        // never an id (`notices.rs`, the unknown-people rule).
        assert_eq!(world.notices.live().len(), 1);
        let notice = &world.notices.live()[0];
        assert_eq!(notice.line(), "a stranger — fouled the street in open view");
        assert_eq!(notice.raised_by, ActorId::from_raw("srgnt"));
        assert_eq!(notice.accused, Some(carrier.clone()));
        assert!(!notice.line().contains("carry"));
        assert!(
            world.characters[&ActorId::from_raw("srgnt")]
                .inbox()
                .iter()
                .any(|line| line.starts_with("word in the ward: ")),
        );
        assert!(
            !world.characters[&carrier]
                .inbox()
                .iter()
                .any(|line| line.starts_with("word in the ward: ")),
            "the ward talks about the offender, not to them"
        );
        world.assert_invariants();
    }

    /// Concealment is a matter of distance, not magic: the neighbour at two
    /// metres sees the thing and the cavity both, the one at five sees a motion
    /// under the clothes, and thirty metres away there is nothing to tell. An
    /// empty lower slot marks nothing — only the company of a stool does, which
    /// is why the apple comes back out exactly as it went in.
    #[test]
    fn a_pocketing_is_plain_at_arms_length_and_a_mere_motion_across_the_square() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        let saw = |world: &World, id: &str| {
            world.characters[&ActorId::from_raw(id)]
                .inbox()
                .last()
                .cloned()
        };

        let line = apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "applex", "slot": "butt"}),
        )
        .unwrap();
        assert_eq!(line, "Carrier pockets the apple (butt)");
        let (slot, apple) = only_pocketed(&world, &carrier);
        assert_eq!(slot, crate::character::BodySlot::Butt);
        assert_eq!(
            condition_of(&world, &apple),
            None,
            "an empty cavity is not a stool"
        );
        assert_eq!(
            saw(&world, "watch").as_deref(),
            Some("A stranger (id carry) hitched up their clothes and pushed an apple up their arse")
        );
        assert_eq!(
            saw(&world, "nosey").as_deref(),
            Some("A stranger (id carry) slipped something out of sight beneath their clothes")
        );
        assert!(
            world.characters[&ActorId::from_raw("faroff")]
                .inbox()
                .is_empty()
        );

        // Coming out is as visible as going in, and by the same two rings.
        apply_action(
            &mut world,
            &carrier,
            "retrieve_item",
            &json!({"item_id": apple.as_str()}),
        )
        .unwrap();
        assert_eq!(
            saw(&world, "watch").as_deref(),
            Some("A stranger (id carry) pulled an apple out of their arse")
        );
        assert_eq!(
            saw(&world, "nosey").as_deref(),
            Some("A stranger (id carry) fetched something from beneath their clothes")
        );

        // The frontbutt names its own side of the body to the close watcher.
        world
            .characters
            .get_mut(&carrier)
            .expect("the carrier is in the world")
            .sheet
            .frontbutt = Some(true);
        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "sparks", "slot": "frontbutt"}),
        )
        .unwrap();
        assert_eq!(
            saw(&world, "watch").as_deref(),
            Some("A stranger (id carry) hitched up their clothes and pushed a spark up their cunt")
        );
        world.assert_invariants();
    }

    /// Spitting at a neighbour under an officer's eye is the ward's business —
    /// and the officer's notice keeps the private linkage that lets restitution
    /// settle it later (`law_and_order.md` M3).
    #[test]
    fn spitting_at_someone_before_an_officer_puts_it_on_the_wards_tongues() {
        let mut world = pocket_world();
        world.add_character(lored(
            "srgnt",
            "Sergeant",
            6.0,
            "bailiff_and_gaoler",
            Some(0.0),
        ));
        let carrier = ActorId::from_raw("carry");
        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "sparks", "slot": "mouth"}),
        )
        .unwrap();
        let (_, wet_id) = only_pocketed(&world, &carrier);

        apply_action(
            &mut world,
            &carrier,
            "spit",
            &json!({"item_id": wet_id.as_str(), "target": "watch"}),
        )
        .unwrap();

        assert_eq!(world.notices.live().len(), 1);
        let notice = &world.notices.live()[0];
        assert_eq!(
            notice.line(),
            "a stranger — spat upon a neighbour in the open street"
        );
        assert_eq!(notice.accused, Some(carrier));
        assert_eq!(notice.wronged, Some(ActorId::from_raw("watch")));
        world.assert_invariants();
    }

    /// An officer who spits raises nothing: the ward against itself is nonsense.
    #[test]
    fn the_law_does_not_raise_the_ward_against_its_own_spitting() {
        let mut world = pocket_world();
        let mut sergeant = lored("srgnt", "Sergeant", 1.0, "bailiff_and_gaoler", Some(0.0));
        sergeant.state.holds.push(ItemId::from_raw("srgspk"));
        world.add_character(sergeant);
        world.add_item(Item::new(ItemId::from_raw("srgspk"), "spark"));
        let sergeant = ActorId::from_raw("srgnt");

        apply_action(
            &mut world,
            &sergeant,
            "pocket_item",
            &json!({"item_id": "srgspk", "slot": "mouth"}),
        )
        .unwrap();
        let (_, wet_id) = only_pocketed(&world, &sergeant);
        apply_action(
            &mut world,
            &sergeant,
            "spit",
            &json!({"item_id": wet_id.as_str(), "target": "watch"}),
        )
        .unwrap();
        assert!(world.notices.is_empty());
        world.assert_invariants();
    }

    /// Expelling with nobody's law in earshot is nobody's business.
    #[test]
    fn a_private_moment_raises_no_notice() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        let stool = ItemId::from_raw("turd1");
        world.add_item(Item::new(stool.clone(), "poop"));
        {
            let state = &mut world.characters.get_mut(&carrier).unwrap().state;
            state.holds.push(stool.clone());
            state.pockets.push(PocketedUnit {
                slot: crate::character::BodySlot::Butt,
                item_id: stool,
            });
        }

        apply_action(&mut world, &carrier, "expel", &json!({})).unwrap();
        assert!(world.notices.is_empty());
        world.assert_invariants();
    }

    /// Muffled speech (`extra_pockets.md` M1, the resolved open question):
    /// listeners are *told* the words came through a full mouth; the words
    /// themselves are never garbled, and the marker leaves with the mouthful.
    #[test]
    fn a_full_mouth_marks_the_listeners_line_and_never_the_words() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        let watcher = ActorId::from_raw("watch");

        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "sparks", "slot": "mouth"}),
        )
        .unwrap();
        let (_, wet_id) = only_pocketed(&world, &carrier);

        let line = apply_action(&mut world, &carrier, "say", &json!({"text": "good day"})).unwrap();
        // The transcript and the event the host reads are untouched.
        assert_eq!(line, "Carrier (aloud): \"good day\"");
        assert_eq!(
            world.drain_events().pop().unwrap().text.as_deref(),
            Some("good day")
        );
        assert_eq!(
            world.characters[&watcher].inbox().last().unwrap(),
            "A stranger (id carry) said through a full mouth: \"good day\""
        );
        // Targeted speech carries it too, after the person spoken to.
        apply_action(
            &mut world,
            &carrier,
            "say",
            &json!({"target": "watch", "text": "and to you"}),
        )
        .unwrap();
        assert_eq!(
            world.characters[&watcher].inbox().last().unwrap(),
            "A stranger (id carry) said to you through a full mouth: \"and to you\""
        );
        assert_eq!(
            world.characters[&ActorId::from_raw("nosey")]
                .inbox()
                .last()
                .unwrap(),
            "A stranger (id carry) said to a stranger (id watch) through a full mouth: \
             \"and to you\""
        );
        // The speaker's own recollection is never marked.
        assert_eq!(
            world.characters[&carrier].recent_history().last().unwrap(),
            "You said to a stranger (id watch): \"and to you\""
        );

        // An empty mouth speaks plainly again.
        apply_action(
            &mut world,
            &carrier,
            "retrieve_item",
            &json!({"item_id": wet_id.as_str()}),
        )
        .unwrap();
        apply_action(&mut world, &carrier, "say", &json!({"text": "better"})).unwrap();
        assert_eq!(
            world.characters[&watcher].inbox().last().unwrap(),
            "A stranger (id carry) said: \"better\""
        );
    }

    /// `gargle` is theatre with a sound: the mouthful survives, in the mouth.
    #[test]
    fn gargling_keeps_the_mouthful_and_refuses_anything_solid() {
        let mut world = pocket_world();
        let carrier = ActorId::from_raw("carry");
        world.sound_catalog = SoundCatalog::new(
            vec![
                Sound::new(
                    "gargle",
                    "body",
                    6.0,
                    "[You heard someone gargling nearby.]",
                    Some("{actor} gargled noisily.".into()),
                    "prompt",
                    2.0,
                    true,
                )
                .unwrap(),
            ],
            Vec::new(),
        )
        .unwrap();

        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "sparks", "slot": "mouth"}),
        )
        .unwrap();
        let (_, wet_id) = only_pocketed(&world, &carrier);
        let error = apply_action(
            &mut world,
            &carrier,
            "gargle",
            &json!({"item_id": wet_id.as_str()}),
        )
        .unwrap_err();
        assert_eq!(error.code, ActionErrorCode::NotEdible);

        apply_action(
            &mut world,
            &carrier,
            "retrieve_item",
            &json!({"item_id": wet_id.as_str()}),
        )
        .unwrap();
        apply_action(
            &mut world,
            &carrier,
            "pocket_item",
            &json!({"item_id": "alepot", "slot": "mouth"}),
        )
        .unwrap();
        let line = apply_action(&mut world, &carrier, "gargle", &json!({"item_id": "alepot"}))
            .unwrap();
        assert_eq!(line, "Carrier gargles the pot of ale");
        // Still in the mouth, still a pot of ale.
        assert_eq!(
            only_pocketed(&world, &carrier),
            (crate::character::BodySlot::Mouth, ItemId::from_raw("alepot"))
        );
        // The watcher is not facing the carrier, so the sound is unattributed —
        // the verb itself delivers no percept at all; the catalog row is the
        // whole fan-out.
        assert_eq!(
            world.characters[&ActorId::from_raw("watch")]
                .inbox()
                .last()
                .unwrap(),
            "[You heard someone gargling nearby.]"
        );
        world.assert_invariants();
    }
}
