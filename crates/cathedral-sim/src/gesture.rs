//! The deliberate body: the `gesture` verb's kind vocabulary and its catalog
//! (`features/npc_bodies.md` §7).
//!
//! A gesture is a communicative motion an LLM commands with an action line —
//! waves, beckons, points, bows, dances — so nearby minds get a percept and can
//! answer it (the mind→body contract, §3). The catalog is a `const` table, not
//! a data file: every kind needs bespoke pose code host-side, so a row without
//! matching animation would be a lie (§7). Adding a gesture is one row here and
//! one pose function in `src/smart_actors/body.rs`.

use serde::{Deserialize, Serialize};

/// How long a looping gesture (`dance`) plays before the engine ends it on its
/// own — the other end of "until the actor's next non-`wait` action" (§7). The
/// action layer has no clock, so the engine stamps the deadline on the first
/// poll that sees the loop and clears it here.
pub const DANCE_MAX_SECONDS: f64 = 60.0;

/// The eight motions the `gesture` verb can command (`features/npc_bodies.md`
/// §7 table). `snake_case` so `shake_head` matches the `kind` the model writes
/// and the wire form the snapshot carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GestureKind {
    Wave,
    Beckon,
    Nod,
    ShakeHead,
    Shrug,
    Point,
    Bow,
    Dance,
}

/// Whether — and how — a gesture takes a `to` target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureTarget {
    /// No target at all: a `to` is an error (`shrug`, `dance`).
    None,
    /// A visible person, optional (`wave`, `nod`, `shake_head`, `bow`).
    OptionalPerson,
    /// A visible person, required (`beckon`).
    RequiredPerson,
    /// A visible person or a known place handle, required (`point`).
    RequiredPersonOrPlace,
}

/// One catalog row: the `kind` string the model writes, the target rule, the
/// duration, whether it loops, and the four percept templates.
#[derive(Debug, Clone, Copy)]
pub struct GestureSpec {
    pub kind: GestureKind,
    /// The `kind` argument the model writes, and what the parser matches.
    pub verb: &'static str,
    pub target: GestureTarget,
    /// Real seconds the motion plays host-side; loopers ignore it.
    pub duration_seconds: f32,
    pub loops: bool,
    /// Third-person percept for witnesses. `{A}` = the actor as the witness
    /// sees them; `{B}` = the target as the witness sees them ("you" when the
    /// witness *is* the target). `witness_targeted` is used when a target
    /// resolved, `witness_untargeted` otherwise.
    pub witness_targeted: &'static str,
    pub witness_untargeted: &'static str,
    /// Second-person percept for the actor's own history — `{B}` filled from
    /// the actor's own perspective.
    pub own_targeted: &'static str,
    pub own_untargeted: &'static str,
}

/// The catalog (`features/npc_bodies.md` §7). Order is the table's; the
/// unreachable templates (a required-target gesture's `_untargeted`, a
/// no-target gesture's `_targeted`) still carry a sensible string so a future
/// caller cannot render blank.
pub const GESTURES: &[GestureSpec] = &[
    GestureSpec {
        kind: GestureKind::Wave,
        verb: "wave",
        target: GestureTarget::OptionalPerson,
        duration_seconds: 1.5,
        loops: false,
        witness_targeted: "{A} waves at {B}.",
        witness_untargeted: "{A} waves.",
        own_targeted: "You wave at {B}.",
        own_untargeted: "You wave.",
    },
    GestureSpec {
        kind: GestureKind::Beckon,
        verb: "beckon",
        target: GestureTarget::RequiredPerson,
        duration_seconds: 1.5,
        loops: false,
        witness_targeted: "{A} beckons {B} closer.",
        witness_untargeted: "{A} beckons.",
        own_targeted: "You beckon {B} closer.",
        own_untargeted: "You beckon.",
    },
    GestureSpec {
        kind: GestureKind::Nod,
        verb: "nod",
        target: GestureTarget::OptionalPerson,
        duration_seconds: 0.8,
        loops: false,
        witness_targeted: "{A} nods to {B}.",
        witness_untargeted: "{A} nods.",
        own_targeted: "You nod to {B}.",
        own_untargeted: "You nod.",
    },
    GestureSpec {
        kind: GestureKind::ShakeHead,
        verb: "shake_head",
        target: GestureTarget::OptionalPerson,
        duration_seconds: 0.9,
        loops: false,
        witness_targeted: "{A} shakes their head at {B}.",
        witness_untargeted: "{A} shakes their head.",
        own_targeted: "You shake your head at {B}.",
        own_untargeted: "You shake your head.",
    },
    GestureSpec {
        kind: GestureKind::Shrug,
        verb: "shrug",
        target: GestureTarget::None,
        duration_seconds: 1.0,
        loops: false,
        witness_targeted: "{A} shrugs.",
        witness_untargeted: "{A} shrugs.",
        own_targeted: "You shrug.",
        own_untargeted: "You shrug.",
    },
    GestureSpec {
        kind: GestureKind::Point,
        verb: "point",
        target: GestureTarget::RequiredPersonOrPlace,
        duration_seconds: 1.2,
        loops: false,
        witness_targeted: "{A} points toward {B}.",
        witness_untargeted: "{A} points.",
        own_targeted: "You point toward {B}.",
        own_untargeted: "You point.",
    },
    GestureSpec {
        kind: GestureKind::Bow,
        verb: "bow",
        target: GestureTarget::OptionalPerson,
        duration_seconds: 1.8,
        loops: false,
        witness_targeted: "{A} bows to {B}.",
        witness_untargeted: "{A} bows.",
        own_targeted: "You bow to {B}.",
        own_untargeted: "You bow.",
    },
    GestureSpec {
        kind: GestureKind::Dance,
        verb: "dance",
        target: GestureTarget::None,
        duration_seconds: 0.0,
        loops: true,
        witness_targeted: "{A} is dancing.",
        witness_untargeted: "{A} is dancing.",
        own_targeted: "You are dancing.",
        own_untargeted: "You are dancing.",
    },
];

impl GestureKind {
    /// The catalog row for this kind.
    pub fn spec(self) -> &'static GestureSpec {
        GESTURES
            .iter()
            .find(|spec| spec.kind == self)
            .expect("every kind has exactly one catalog row")
    }

    /// The `kind` string this gesture is written and serialized as.
    pub fn as_verb(self) -> &'static str {
        self.spec().verb
    }

    /// Resolve the `kind` string the model wrote to a gesture, or `None` for an
    /// unknown kind (the caller turns that into the standard action error).
    pub fn from_verb(verb: &str) -> Option<Self> {
        GESTURES
            .iter()
            .find(|spec| spec.verb == verb)
            .map(|spec| spec.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_verb_and_the_catalog_is_complete() {
        let kinds = [
            GestureKind::Wave,
            GestureKind::Beckon,
            GestureKind::Nod,
            GestureKind::ShakeHead,
            GestureKind::Shrug,
            GestureKind::Point,
            GestureKind::Bow,
            GestureKind::Dance,
        ];
        assert_eq!(GESTURES.len(), kinds.len());
        for kind in kinds {
            assert_eq!(GestureKind::from_verb(kind.as_verb()), Some(kind));
            assert_eq!(kind.spec().kind, kind);
        }
        assert_eq!(GestureKind::from_verb("cartwheel"), None);
        // Exactly one looper, and it is `dance`.
        let loopers: Vec<_> = GESTURES.iter().filter(|spec| spec.loops).collect();
        assert_eq!(loopers.len(), 1);
        assert_eq!(loopers[0].kind, GestureKind::Dance);
    }

    #[test]
    fn serde_uses_the_snake_case_verb() {
        assert_eq!(
            serde_json::to_string(&GestureKind::ShakeHead).unwrap(),
            "\"shake_head\""
        );
        assert_eq!(
            serde_json::from_str::<GestureKind>("\"dance\"").unwrap(),
            GestureKind::Dance
        );
    }
}
