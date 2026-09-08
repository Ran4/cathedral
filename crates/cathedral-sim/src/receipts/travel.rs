//! Receipt adapter for the existing TravelIntent/Round undertaking. This is
//! not the later generic operation/duty kernel.
use super::*;
use crate::{ActionError, ActionErrorCode, ActorId, World};

pub(crate) fn commit_actor_action(
    world: &mut World,
    now: f64,
    ticket: Ticket,
    actor: &ActorId,
    verb: &str,
    args: &Value,
) -> (Result<String, ActionError>, Receipt) {
    let id = ticket.id;
    let result = if matches!(verb, "go_to" | "set_round")
        && world.travel_actions.len() + world.round_actions.len() >= PROTECTED_CAPACITY
    {
        Err(ActionError::new(
            ActionErrorCode::InvalidAction,
            "active travel receipt capacity is full",
        ))
    } else {
        crate::actions::apply_action(world, actor, verb, args)
    };
    let outcome = match &result {
        Ok(_) if verb == "go_to" => {
            end_travel(
                world,
                actor,
                now,
                ReceiptState::Superseded,
                "travel_replaced",
                "a new travel intent replaced this errand",
            );
            // The batch protected this semantic root before any reply effects.
            world
                .characters
                .get_mut(actor)
                .expect("validated actor")
                .state
                .intent
                .as_mut()
                .expect("successful go_to commits an intent")
                .receipt = Some(id);
            world.travel_actions.insert(actor.clone(), id);
            Outcome::new(
                ReceiptState::Accepted,
                "travel_accepted",
                "the travel intent is accepted; arrival is pending",
            )
        }
        Ok(_) if verb == "set_round" => {
            bind_round_edit(world, actor, id, now, false);
            Outcome::new(
                ReceiptState::Accepted,
                "round_edit_accepted",
                "the schedule edit is queued for the Round to validate",
            )
        }
        Ok(line) => {
            if verb == "stop" {
                end_travel(
                    world,
                    actor,
                    now,
                    ReceiptState::Interrupted,
                    "travel_stopped",
                    "the actor stopped this errand",
                );
            }
            Outcome::completed(line)
        }
        Err(error) => Outcome::rejected(error.code.as_str(), &error.message),
    };
    let affected = [
        AffectedRef::new("actor", actor.as_str()),
        args.get("item_id")
            .and_then(Value::as_str)
            .and_then(|id| AffectedRef::new("item", id)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let receipt = world.command_ledger.finish(ticket, now, outcome, affected);
    (result, receipt)
}

pub(crate) fn release_finished_root(world: &mut World, id: OperationId) {
    if !world.command_ledger.operation_pending(id)
        && !world.operations.owns_root(id)
        && !world
            .travel_actions
            .values()
            .chain(world.round_actions.values())
            .chain(world.speech_actions.iter())
            .any(|command| command.operation == id)
    {
        world.command_ledger.unprotect(id);
    }
}

pub(crate) fn end_travel(
    world: &mut World,
    actor: &ActorId,
    now: f64,
    state: ReceiptState,
    code: &str,
    message: &str,
) {
    let Some(id) = world.travel_actions.remove(actor) else {
        return;
    };
    let _ = world
        .command_ledger
        .advance(id, now, Outcome::new(state, code, message));
    release_finished_root(world, id.operation);
}

pub(crate) fn progress_travel(world: &mut World, actor: &ActorId, now: f64) {
    let Some(id) = world.travel_actions.get(actor).copied() else {
        return;
    };
    if world
        .command_ledger
        .get(id)
        .is_some_and(|receipt| receipt.outcome.state == ReceiptState::Accepted)
    {
        let _ = world.command_ledger.advance(
            id,
            now,
            Outcome::new(
                ReceiptState::InProgress,
                "travel_started",
                "the actor is walking the accepted errand",
            ),
        );
    }
}

/// Covers every existing direct intent writer: custody, confinement, road
/// departure, debug staging and future duty replacements. Only the explicit
/// Round arrival signal can report Completed; disappearance alone never does.
pub(crate) fn reconcile_travel(world: &mut World, now: f64) {
    let ended_edits: Vec<_> = world
        .round_actions
        .iter()
        .filter_map(|(actor, id)| {
            let matching = world.characters.get(actor).is_some_and(|a| {
                a.state.round_edit.as_ref().is_some_and(|edit| {
                    edit.receipt == Some(*id) && edit.presence_epoch == Some(a.state.presence_epoch)
                })
            });
            (!matching || !world.is_present(actor)).then_some(actor.clone())
        })
        .collect();
    for actor in ended_edits {
        end_round_edit(
            world,
            &actor,
            now,
            ReceiptState::Superseded,
            "round_edit_obsolete",
            "the subject departed or the pending schedule edit was replaced",
        );
    }

    let ended: Vec<_> = world
        .travel_actions
        .iter()
        .filter_map(|(actor, id)| {
            let matching = world
                .characters
                .get(actor)
                .and_then(|a| a.state.intent.as_ref())
                .is_some_and(|intent| intent.receipt == Some(*id));
            (!matching || !world.is_present(actor)).then_some(actor.clone())
        })
        .collect();
    for actor in ended {
        let (state, code, reason) = if !world.is_present(&actor) {
            (
                ReceiptState::Interrupted,
                "actor_departed",
                "the actor left the city before completing the errand",
            )
        } else if world.custody.holds(&actor) || world.custody.prisoners_of(&actor).len() > 0 {
            (
                ReceiptState::Superseded,
                "custody_duty",
                "custody replaced the travel intent",
            )
        } else {
            (
                ReceiptState::Superseded,
                "intent_replaced",
                "the travel intent was removed or replaced before a confirmed arrival",
            )
        };
        end_travel(world, &actor, now, state, code, reason);
    }
}

/// Bind an already validated pending edit; its actual Round consumer owns completion.
pub(crate) fn bind_round_edit(
    world: &mut World,
    actor: &ActorId,
    id: CommandId,
    now: f64,
    teach: bool,
) {
    end_round_edit(
        world,
        actor,
        now,
        ReceiptState::Superseded,
        "round_edit_replaced",
        "a later schedule edit replaced this pending edit",
    );
    let character = world
        .characters
        .get_mut(actor)
        .expect("validated edit actor");
    let edit = character
        .state
        .round_edit
        .as_mut()
        .expect("validated pending edit");
    edit.receipt = Some(id);
    edit.presence_epoch = Some(character.state.presence_epoch);
    edit.teach_place_on_commit = teach;
    world.round_actions.insert(actor.clone(), id);
}

pub(crate) fn end_round_edit(
    world: &mut World,
    actor: &ActorId,
    now: f64,
    state: ReceiptState,
    code: &str,
    message: &str,
) {
    let Some(id) = world.round_actions.remove(actor) else {
        return;
    };
    let _ = world
        .command_ledger
        .advance(id, now, Outcome::new(state, code, message));
    release_finished_root(world, id.operation);
}
