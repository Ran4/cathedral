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
    GOAL_MAX_CHARS, GOAL_NONE, HEARING_RADIUS_M, ITEM_INTERACTION_RADIUS_M, MEMORY_MAX_CHARS,
    PLAYER_SPEECH_MAX_CHARS,
    error::{ActionError, ActionErrorCode},
    event::DomainEvent,
    ids::{ActorId, ItemId},
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
    if !world.characters.contains_key(actor_id) {
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

fn dispatch(
    world: &mut World,
    actor_id: &ActorId,
    verb: &str,
    args: &Value,
) -> Result<String, ActionError> {
    // A non-string verb is impossible here (the reply parser types it), which
    // is where Python's `invalid_action` check lived.
    match verb {
        "wait" => wait(world, actor_id, args),
        "say" => say(world, actor_id, args),
        "offer_item" => offer_item(world, actor_id, args),
        "accept_offered_item" => accept_offered_item(world, actor_id, args),
        "decline_offer" => decline_offer(world, actor_id, args),
        "retract_offer" => retract_offer(world, actor_id, args),
        "eat" => eat(world, actor_id, args),
        "set_goal" => set_goal(world, actor_id, args),
        "make_sound" => make_sound(world, actor_id, args),
        "remember" => remember(world, actor_id, args),
        "forget" => forget(world, actor_id, args),
        // Checked last, after every verb has had its chance to match.
        unknown => Err(ActionError::new(
            ActionErrorCode::UnknownVerb,
            format!("unknown verb: {unknown}"),
        )),
    }
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

fn world_event(
    world: &mut World,
    kind: &str,
    actor_id: &ActorId,
    target_id: Option<ActorId>,
    item_id: Option<ItemId>,
    recipients: Vec<ActorId>,
) -> i64 {
    let position = world.characters[actor_id].position_m();
    world.emit(DomainEvent::world_event(
        kind,
        actor_id.clone(),
        target_id,
        item_id,
        position,
        recipients,
    ))
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
        if !world.characters.contains_key(&target_id) {
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
                    format!("{speaker} said to you: \"{text}\"")
                } else {
                    format!(
                        "{speaker} said to {}: \"{text}\"",
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
                (recipient.clone(), format!("{speaker} said: \"{text}\""))
            })
            .collect();
        deliver(world, percepts, true);
        format!("{} (aloud): \"{text}\"", world.characters[actor_id].name())
    };

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
    let parsed = args_object(args, &["item_id"], &["target"])?;
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
    let Some(item) = world.items.get(&item_id) else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownItem,
            format!("there is no item with id {}", repr_id(item_id.as_str())),
        ));
    };
    let item_name = item.name.clone();

    let mut target: Option<ActorId> = None;
    if let Some(value) = optional_arg(parsed, "target") {
        let target_id = parse_actor_id(value, "target")?;
        if !world.characters.contains_key(&target_id) {
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
    if let Some(old_target) = old_offer.as_ref().and_then(|offer| offer.target_id.clone())
        && Some(&old_target) != target.as_ref()
        && world.characters.contains_key(&old_target)
        && hearers.contains(&old_target)
    {
        let line = format!(
            "{} withdrew the offered {item_name} (id {item_id})",
            cap_first(&identify_ids(world, &old_target, actor_id))
        );
        deliver(world, vec![(old_target.clone(), line)], false);
        world_event(
            world,
            "retract_offer",
            actor_id,
            Some(old_target.clone()),
            Some(item_id.clone()),
            vec![old_target],
        );
    }

    let line = if let Some(target_id) = &target {
        let lines = hearers
            .iter()
            .map(|observer| {
                let giver = cap_first(&identify_ids(world, observer, actor_id));
                let line = if observer == target_id {
                    format!("{giver} held out a {item_name} (id {item_id}) to you")
                } else {
                    format!(
                        "{giver} offered a {item_name} to {}",
                        identify_ids(world, observer, target_id)
                    )
                };
                (observer.clone(), line)
            })
            .collect();
        deliver(world, lines, false);
        format!(
            "{} offers the {item_name} to {}",
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
                        "{giver} held out a {item_name} (id {item_id}) to anyone who wanted it"
                    ),
                )
            })
            .collect();
        deliver(world, lines, false);
        format!(
            "{} offers the {item_name} to anyone nearby",
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
        hearers,
    );
    world.offers.insert(
        item_id.clone(),
        Offer {
            item_id,
            giver_id: actor_id.clone(),
            target_id: target,
            created_seq: sequence,
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
    let giver_id = offer.giver_id;
    if !world.characters.contains_key(&giver_id) {
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
    let Some(item) = world.items.get(&item_id) else {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "the offered item no longer exists",
        ));
    };
    let item_name = item.name.clone();

    world.offers.remove(&item_id);
    let giver = world.characters.get_mut(&giver_id).expect("checked above");
    giver.state.holds.retain(|held| held != &item_id);
    world
        .characters
        .get_mut(actor_id)
        .expect("the taker is in the world")
        .state
        .holds
        .push(item_id.clone());

    // The giver is guaranteed inside this radius, being at most 4 m away.
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    let lines = hearers
        .iter()
        .map(|observer| {
            let taker = cap_first(&identify_ids(world, observer, actor_id));
            let line = if observer == &giver_id {
                format!("{taker} accepted the {item_name} (id {item_id}) you offered")
            } else {
                format!(
                    "{taker} took a {item_name} from {}",
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
        Some(item_id),
        hearers,
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} takes the {item_name} from {}",
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
    if !world.characters.contains_key(&giver_id) {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::NoOffer,
            "the person offering it no longer exists",
        ));
    }
    // The offer survives a failed range check.
    require_interaction_range(world, actor_id, &giver_id)?;
    let Some(item) = world.items.get(&item_id) else {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "the offered item no longer exists",
        ));
    };
    let item_name = item.name.clone();

    // The giver keeps the item.
    world.offers.remove(&item_id);
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    let lines = hearers
        .iter()
        .map(|observer| {
            let decliner = cap_first(&identify_ids(world, observer, actor_id));
            let line = if observer == &giver_id {
                format!("{decliner} declined the {item_name} (id {item_id}) you offered")
            } else {
                format!(
                    "{decliner} declined a {item_name} from {}",
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
        hearers,
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} declines the {item_name} from {}",
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
    let Some(item) = world.items.get(&item_id) else {
        return Err(repair_and_fail(
            world,
            &item_id,
            ActionErrorCode::StaleOffer,
            "the offered item no longer exists",
        ));
    };
    let item_name = item.name.clone();

    // No proximity requirement at all — but a distant target gets no percept.
    world.offers.remove(&item_id);
    let mut recipients: Vec<ActorId> = Vec::new();
    if let Some(target_id) = &offer.target_id
        && let Some(target) = world.characters.get(target_id)
        && world.characters[actor_id]
            .position_m()
            .distance_squared(target.position_m())
            <= HEARING_RADIUS_M * HEARING_RADIUS_M
    {
        let line = format!(
            "{} withdrew the offered {item_name} (id {item_id})",
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
        recipients,
    );
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} retracts the offer of the {item_name}",
        world.characters[actor_id].name()
    ))
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
    let Some(item) = world.items.get(&item_id) else {
        return Err(ActionError::new(
            ActionErrorCode::UnknownItem,
            format!("there is no item with id {}", repr_id(item_id.as_str())),
        ));
    };
    let item_name = item.name.clone();

    let offer = world.offers.remove(&item_id);
    let hearers = nearby(world, actor_id, HEARING_RADIUS_M);
    // The implicit retraction notifies, but deliberately emits NO
    // retract_offer event (asymmetric with re-offer displacement).
    if let Some(target_id) = offer.and_then(|offer| offer.target_id)
        && world.characters.contains_key(&target_id)
        && hearers.contains(&target_id)
    {
        let line = format!(
            "{} withdrew the offered {item_name} (id {item_id})",
            cap_first(&identify_ids(world, &target_id, actor_id))
        );
        deliver(world, vec![(target_id, line)], false);
    }

    world
        .characters
        .get_mut(actor_id)
        .expect("the eater is in the world")
        .state
        .holds
        .retain(|held| held != &item_id);
    // Items are singular: eating one removes it from the world forever.
    world.items.remove(&item_id);

    let lines = hearers
        .iter()
        .map(|observer| {
            let eater = cap_first(&identify_ids(world, observer, actor_id));
            (observer.clone(), format!("{eater} ate a {item_name}"))
        })
        .collect();
    deliver(world, lines, false);
    world_event(world, "eat", actor_id, None, Some(item_id), hearers);
    world.touch_public_state();
    world.assert_invariants();
    Ok(format!(
        "{} eats the {item_name}",
        world.characters[actor_id].name()
    ))
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
            id: ActorId::from_raw(id),
            name: name.into(),
            control: Control::Llm,
            back_story: "test".into(),
            location_description: "test square".into(),
            appearance_key: name.to_lowercase(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
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
    fn eating_an_offered_item_retracts_it_without_emitting_a_retract_event() {
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

        let line = apply_action(&mut world, &giver, "eat", &json!({"item_id": "apple"})).unwrap();

        assert_eq!(line, "Giver eats the apple");
        // Items are singular: the world forgets them entirely.
        assert!(!world.items.contains_key(&apple));
        assert!(!world.offers.contains_key(&apple));
        assert!(world.characters[&giver].holds().is_empty());
        // The displaced target is told in prose...
        assert!(
            world.characters[&receiver]
                .inbox()
                .iter()
                .any(|line| line.contains("withdrew"))
        );
        // ...but the implicit retraction deliberately emits NO retract_offer
        // event, unlike re-offer displacement. The HUD learns of the removal
        // from the snapshot instead.
        let kinds: Vec<String> = world
            .drain_events()
            .into_iter()
            .map(|event| event.kind)
            .collect();
        assert_eq!(kinds, ["eat"]);
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
        // The emitter remembers their own act, with an empty inbox.
        assert_eq!(
            world.characters[&giver].recent_history(),
            ["You farted.", "You farted."]
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

        let error = apply_action(&mut world, &giver, "dance", &json!({})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownVerb);
        assert_eq!(error.message, "unknown verb: dance");

        let error =
            apply_action(&mut world, &ActorId::from_raw("ghost"), "wait", &json!({})).unwrap_err();
        assert_eq!(error.code, ActionErrorCode::UnknownActor);
    }
}
