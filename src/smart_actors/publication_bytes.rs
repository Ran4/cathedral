//! Allocation admission for the in-process publication queue. Count retained
//! capacities, not serialized size; shared audio is conservatively charged in
//! full. BTree metadata uses a conservative node overhead per retained entry.

use cathedral_sim::{ActorId, EngineMessage, PublicSnapshot};

fn vec<T>(values: &Vec<T>, nested: impl Fn(&T) -> usize) -> usize {
    values.capacity() * std::mem::size_of::<T>() + values.iter().map(nested).sum::<usize>()
}
fn text(value: &String) -> usize {
    value.capacity()
}
fn optional(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, text)
}
fn ids(values: &Vec<ActorId>) -> usize {
    vec(values, ActorId::allocated_bytes)
}
fn actor(value: &Option<ActorId>) -> usize {
    value.as_ref().map_or(0, ActorId::allocated_bytes)
}
fn resident(value: &cathedral_sim::round::residents::ResidentStatus) -> usize {
    text(&value.patch)
        + text(&value.patch_description)
        + optional(&value.spot)
        + optional(&value.destination_spot)
}

fn snapshot(value: &PublicSnapshot) -> usize {
    value.player_id.allocated_bytes()
        + vec(&value.actors, |a| {
            a.id.allocated_bytes()
                + text(&a.name_for_player)
                + optional(&a.appearance.bespoke)
                + a.resident.as_ref().map_or(0, resident)
                + vec(&a.holds, |id| id.allocated_bytes())
                + vec(&a.statuses, |_| 0)
                + vec(&a.pockets, |(_, id)| id.allocated_bytes())
        })
        + vec(&value.items, |item| {
            item.id.allocated_bytes() + item.kind.allocated_bytes() + text(&item.display_name)
            + text(&item.display_plural) + text(&item.visual_key)
            // A minimally occupied BTree node still fits this per-entry bound.
            + (item.metadata.len() + 1) * 1024
            + item.metadata.iter().map(|(k, v)| text(k) + text(v)).sum::<usize>()
        })
        + vec(&value.offers, |offer| {
            offer.item_id.allocated_bytes()
                + offer.giver_id.allocated_bytes()
                + actor(&offer.target_id)
        })
        + vec(&value.road_carts, |cart| {
            cart.party_id.allocated_bytes()
                + cart.leader_id.allocated_bytes()
                + vec(&cart.load, |_| 0)
        })
        + vec(&value.marks, |_| 0)
}

pub(super) fn message(message: &EngineMessage) -> usize {
    use EngineMessage::*;
    std::mem::size_of::<EngineMessage>()
        + match message {
            ActionReceipt(receipt) => {
                text(&receipt.outcome.code)
                    + text(&receipt.outcome.message)
                    + vec(&receipt.affected, |r| text(&r.kind) + text(&r.id))
            }
            CommandAdmissionRefused { reason, .. } => text(&reason.code) + text(&reason.message),
            Ready {
                snapshot: value, ..
            }
            | Snapshot(value) => snapshot(value),
            Clock { .. } | Weather(_) | Lightning(_) => 0,
            Movement { moved } => vec(moved, |motion| motion.actor_id.allocated_bytes()),
            ResidentStates { residents } => vec(residents, |(id, state)| {
                id.allocated_bytes() + resident(state)
            }),
            Lamps { lamps } => vec(lamps, |lamp| text(&lamp.square)),
            Dogs { dogs } => vec(dogs, |dog| dog.id.allocated_bytes() + text(&dog.name)),
            Speech {
                event_id,
                speaker_id,
                target_id,
                text: line,
                recipient_ids,
                speaker_name_for_player,
                ..
            } => {
                text(&event_id.0)
                    + speaker_id.allocated_bytes()
                    + actor(target_id)
                    + text(line)
                    + ids(recipient_ids)
                    + text(speaker_name_for_player)
            }
            Sound {
                event_id,
                sound_id,
                sound_class,
                actor_id,
                recipient_ids,
                witness_ids,
                text_for_player,
                ..
            } => {
                text(event_id)
                    + text(sound_id)
                    + text(sound_class)
                    + actor(actor_id)
                    + ids(recipient_ids)
                    + ids(witness_ids)
                    + optional(text_for_player)
            }
            WorldEvent {
                event_id,
                kind,
                actor_id,
                target_id,
                item_id,
                recipient_ids,
                ..
            } => {
                text(event_id)
                    + text(kind)
                    + actor_id.allocated_bytes()
                    + actor(target_id)
                    + item_id.as_ref().map_or(0, |id| id.allocated_bytes())
                    + ids(recipient_ids)
            }
            Gesture {
                event_id,
                actor_id,
                target_id,
                recipient_ids,
                ..
            } => {
                text(event_id) + actor_id.allocated_bytes() + actor(target_id) + ids(recipient_ids)
            }
            CommandResult {
                request_id,
                error_code,
                message,
                ..
            } => text(request_id) + optional(error_code) + text(message),
            TranscriptionResult {
                request_id,
                text: line,
                error,
            } => text(request_id) + optional(line) + optional(error),
            Status(status) => {
                text(&status.state)
                    + actor(&status.actor_id)
                    + optional(&status.message)
                    + optional(&status.backend)
            }
            TtsReady {
                event_id,
                wav_bytes,
            } => text(&event_id.0) + wav_bytes.len() + 2 * std::mem::size_of::<usize>(),
            TtsChunk {
                event_id, samples, ..
            } => {
                text(&event_id.0)
                    + std::mem::size_of_val(samples.as_ref())
                    + 2 * std::mem::size_of::<usize>()
            }
            TtsStreamEnd { event_id, .. } => text(&event_id.0),
            TtsFailed { event_id, reason } => text(&event_id.0) + text(reason),
            PromptExchange {
                actor_id,
                actor_name,
                prompt,
                answer,
                error,
                ..
            } => {
                actor_id.allocated_bytes()
                    + text(actor_name)
                    + text(prompt)
                    + optional(answer)
                    + optional(error)
            }
            LawStanding { notices, custody } => {
                vec(notices, |n| text(&n.line) + text(&n.clears_when))
                    + custody.as_ref().map_or(0, |c| {
                        ids(&c.holder_ids)
                            + actor(&c.officer_id)
                            + text(&c.officer_name)
                            + text(&c.station_name)
                            + optional(&c.release_office)
                            + optional(&c.booked_as)
                    })
            }
            Journal { entries, standing } => {
                vec(entries, |e| {
                    text(&e.word) + optional(&e.from) + optional(&e.place) + text(&e.when)
                }) + vec(standing, text)
            }
            WardHeat { wards } => vec(wards, |ward| text(&ward.label)),
            ChalkStanding { anchors, .. } => vec(anchors, |a| {
                text(&a.handle) + text(&a.label) + vec(&a.kinds, |_| 0)
            }),
            Diagnostic(line) => text(line),
        }
}
