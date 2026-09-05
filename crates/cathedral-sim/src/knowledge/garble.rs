//! How a telling goes wrong on the way round, and why that is checkable.
//!
//! Deterministic, seeded per `(fact.sequence, carrier, hops)`, bounded to a
//! fixed vocabulary, and never inventing a person: the subject becomes another
//! **named** actor of the same ward or trade, the place an adjacent area, the
//! day ±1 per garbled hop. Which fields may move is the fact's own
//! [`GarbleMask`](super::GarbleMask); the rest is load-bearing truth.
//!
//! Because the roll is a pure function of that seed and a view is stored as
//! deltas, the transmission chain is a **reconstruction** and not a log:
//! [`super::chain`] walks `Held::from` back and this module recomputes each
//! link's view, which is what makes "who told you that?" answerable for the
//! player, for a sergeant and for a test.
//!
//! The pool it draws from is deliberately **time-invariant**: a lore ward and an
//! occupation, neither of which is ever written at runtime. A pool that moved
//! with the subject's feet would make the same link recompute to a different
//! name an hour later, and the reconstruction above would be a lie.

use std::hash::{DefaultHasher, Hash, Hasher};

use crate::character::Character;
use crate::ids::ActorId;
use crate::world::World;

use super::{DAY_OFFSET_MAX, Fact, FactView, GARBLE_CHANCE_PER_HOP, GARBLE_SUBJECT_POOL_MAX};

/// The fact as this carrier has it, at this many removes.
///
/// A pure function of `(fact.sequence, carrier, hops)` — never of the clock,
/// never of a fresh draw, and never of anything that moves while the world runs.
/// `hops == 0` returns a pristine view without a special case, because
/// [`wrong_chance`] is exactly 0.0 there: a witness is not wrong about what they
/// saw.
pub fn view_for(world: &World, fact: &Fact, carrier: &ActorId, hops: u8) -> FactView {
    let mut view = FactView::default();
    if hops == 0 || !fact.garble.any() {
        return view;
    }
    let chance = wrong_chance(hops);
    let step = u64::from(hops);

    if fact.garble.subject
        && let Some(subject) = fact.subject.first()
        && roll("pollen_garble_subject", fact.sequence, carrier, step) < chance
    {
        let mut pool = same_ward_or_trade(world, subject);
        // Never the carrier themselves and never another of the fact's own
        // subjects: a person is not handed a story in which they are the one it
        // is about (the sheet would then have to drop the bullet under the
        // self-subject rule, and a swap that renders nothing is a swap wasted),
        // and swapping one subject for another of the same fact moves nothing.
        pool.retain(|id| id != carrier && !fact.subject.contains(id));
        if !pool.is_empty() {
            let index = pick(
                "pollen_garble_subject_pick",
                fact.sequence,
                carrier,
                step,
                pool.len(),
            );
            view.subject = Some(pool.swap_remove(index));
        }
    }

    if fact.garble.place
        && let Some(place) = fact.place
        && roll("pollen_garble_place", fact.sequence, carrier, step) < chance
    {
        let neighbours = world.area_adjacency.neighbours(place);
        // Empty in every world with an empty `AreaMap` — the goldens and the
        // hermetic tests — so a place garble is a no-op there. Stated, not
        // discovered.
        if !neighbours.is_empty() {
            let index = pick(
                "pollen_garble_place_pick",
                fact.sequence,
                carrier,
                step,
                neighbours.len(),
            );
            view.place = Some(neighbours[index]);
        }
    }

    if fact.garble.day && fact.day.is_some() {
        view.day_offset = day_drift(fact.sequence, carrier, hops);
    }

    view
}

/// The substitution pool for a garbled subject: named actors sharing the
/// subject's **lore** ward or `occupation_id`, in roster order, capped at
/// [`GARBLE_SUBJECT_POOL_MAX`].
///
/// Roster order and a lore-ward cohort are `prompt::ward_minors`' own shape, so
/// this is a fourth of a kind and not a new pattern. Three exclusions, each for a
/// reason:
///
/// - **no lore profile** — the generated crowd are strangers with no authored
///   sheet and the player has no profile at all, and the spec's vocabulary is
///   "another *named* actor". This is also what bounds the walk: a lore-less
///   subject returns immediately rather than scanning 20,000 bodies.
/// - **`generated`** — same reason, from the other side: a rumour that names a
///   procedural body names nobody the city knows.
/// - **the subject** — obviously.
///
/// Presence is deliberately **not** filtered: a rumour may name somebody who has
/// left the city, and a pool that shrank when somebody went indoors would break
/// the reconstruction in [`view_for`]'s doc comment.
pub fn same_ward_or_trade(world: &World, subject: &ActorId) -> Vec<ActorId> {
    let Some(profile) = world.characters.get(subject).and_then(Character::lore) else {
        return Vec::new();
    };
    if profile.generated {
        return Vec::new();
    }
    let ward = profile.planning_ward;
    let trade = profile.occupation_id.as_deref();
    let mut pool: Vec<ActorId> = Vec::new();
    for actor_id in &world.roster {
        if actor_id == subject {
            continue;
        }
        let Some(other) = world.characters.get(actor_id).and_then(Character::lore) else {
            continue;
        };
        if other.generated {
            continue;
        }
        let same_ward = other.planning_ward == ward;
        let same_trade = trade.is_some() && other.occupation_id.as_deref() == trade;
        if !(same_ward || same_trade) {
            continue;
        }
        pool.push(actor_id.clone());
        if pool.len() == GARBLE_SUBJECT_POOL_MAX {
            break;
        }
    }
    pool
}

/// `P(a maskable field is wrong at n removes) = 1 − (1 − g)^n`.
///
/// Right 0.650 / 0.423 / 0.275 / 0.179 at hops 1–4 — hearsay worth acting on at
/// one hop, a chain worth walking at four (`02_numbers.md` §3, where
/// [`GARBLE_CHANCE_PER_HOP`] is bracketed from exactly those two readings).
fn wrong_chance(hops: u8) -> f64 {
    1.0 - (1.0 - GARBLE_CHANCE_PER_HOP).powi(i32::from(hops))
}

/// ±1 **per garbled hop**, clamped to [`DAY_OFFSET_MAX`] — the spec's own words,
/// implemented as the walk it describes rather than as one draw, so a deep
/// telling is wrong about the day by more than a shallow one and two hops can
/// cancel. Two hashes per hop; `hops` is bounded by heat in practice (4–5) and
/// by `u8` in law.
fn day_drift(sequence: i64, carrier: &ActorId, hops: u8) -> i8 {
    let mut offset: i32 = 0;
    for hop in 1..=u64::from(hops) {
        if roll("pollen_garble_day", sequence, carrier, hop) < GARBLE_CHANCE_PER_HOP {
            offset += if roll("pollen_garble_day_sign", sequence, carrier, hop) < 0.5 {
                -1
            } else {
                1
            };
        }
    }
    let bound = i32::from(DAY_OFFSET_MAX);
    offset.clamp(-bound, bound) as i8
}

/// `notices::carries`' roll, verbatim in shape: a hash of stable inputs in
/// `[0, 1)`, never a fresh draw.
fn roll(salt: &str, sequence: i64, carrier: &ActorId, step: u64) -> f64 {
    (seed(salt, sequence, carrier, step) >> 11) as f64 / (1u64 << 53) as f64
}

/// A stable index into a stably-ordered pool. Separate salt from [`roll`], so
/// "is it wrong" and "wrong how" cannot correlate.
fn pick(salt: &str, sequence: i64, carrier: &ActorId, step: u64, len: usize) -> usize {
    debug_assert!(len > 0, "pick over an empty pool");
    (seed(salt, sequence, carrier, step) % len as u64) as usize
}

fn seed(salt: &str, sequence: i64, carrier: &ActorId, step: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    sequence.hash(&mut hasher);
    carrier.hash(&mut hasher);
    step.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;
