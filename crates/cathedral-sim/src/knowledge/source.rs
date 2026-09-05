//! Why a fact is true — sealed.

use std::fmt;

use crate::ids::{ActorId, ItemId};
use crate::world::World;

/// Why a fact is true. **Never rendered anywhere** — not a prompt, a projection,
/// a log line, a journal entry or a `Debug` string.
///
/// Sealed three ways rather than one, because a projection-walking test cannot
/// see `EngineMessage::Diagnostic(format!("{fact:?}"))` written in good faith by
/// somebody who has not read this comment: the payload is private to this
/// module, there is no `Serialize`/`Deserialize`/`Display` to reach for, and
/// `Debug` prints a placeholder. A fact says *what*; this says why it is so,
/// and in a quest that is usually the answer the player is looking for.
///
/// **Must not derive or implement** `Serialize`, `Deserialize`, `Display`, a
/// derived `Debug`, or `Default`.
#[derive(Clone, PartialEq, Eq)]
pub struct FactSource(Provenance);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Provenance {
    /// An author bound it to nothing; it is simply so.
    Authored,
    /// Somebody said it. The only thing an LLM can make, and the whole safety
    /// argument for `raise_word` in one variant.
    Claimed(ActorId),
    /// True while the law holds this person.
    Custody(ActorId),
    /// True while this item is still in this pair of hands. Items have no
    /// position in this sim (`item.rs`): an item is wherever its holder stands,
    /// so "the item moved" means "it changed hands or left the world".
    ItemWith { item: ItemId, holder: ActorId },
    /// True while this quest phase stands.
    QuestPhase { quest: String, phase: u8 },
    /// Minted from an event and never re-checked.
    Event { kind: String, sequence: i64 },
}

impl fmt::Debug for FactSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FactSource(<sealed>)")
    }
}

impl FactSource {
    pub fn authored() -> Self {
        Self(Provenance::Authored)
    }

    pub fn claimed(by: ActorId) -> Self {
        Self(Provenance::Claimed(by))
    }

    pub fn custody(of: ActorId) -> Self {
        Self(Provenance::Custody(of))
    }

    pub fn item_with(item: ItemId, holder: ActorId) -> Self {
        Self(Provenance::ItemWith { item, holder })
    }

    pub fn quest_phase(quest: impl Into<String>, phase: u8) -> Self {
        Self(Provenance::QuestPhase {
            quest: quest.into(),
            phase,
        })
    }

    pub fn event(kind: impl Into<String>, sequence: i64) -> Self {
        Self(Provenance::Event {
            kind: kind.into(),
            sequence,
        })
    }

    /// A model can mint claims; it can never mint truths. The one bit of
    /// provenance anything outside this module may read.
    pub fn is_claimed(&self) -> bool {
        matches!(self.0, Provenance::Claimed(_))
    }

    /// The mouth a raised word walks back to. `None` for every other route.
    pub fn claimant(&self) -> Option<&ActorId> {
        match &self.0 {
            Provenance::Claimed(who) => Some(who),
            _ => None,
        }
    }

    /// Whether the world still bears this out. `false` drops the fact from
    /// `live` and from every holding, which clears it off every sheet on the
    /// next turn with no `forget` and no LLM cooperation.
    ///
    /// M5 wires [`crate::knowledge::invalidate_stale`] into `pollen::sweep`'s
    /// stir beat; until then only tests call it. Every arm has its body here —
    /// no later milestone edits this file.
    pub(crate) fn still_true(&self, world: &World) -> bool {
        match &self.0 {
            Provenance::Authored | Provenance::Event { .. } => true,
            // No quest system exists yet, so a phase cannot have advanced. The
            // quest specs bind this when they are rewritten against M5.
            Provenance::QuestPhase { .. } => true,
            Provenance::Claimed(who) => world.characters.contains_key(who),
            Provenance::Custody(who) => world.custody.holds(who),
            Provenance::ItemWith { item, holder } => world
                .characters
                .get(holder)
                .is_some_and(|character| character.holds().contains(item)),
        }
    }
}
