//! The public snapshot (`sim.py:323-368`) — everything the game may know.
//!
//! Privacy invariant: no `back_story`, `memories`, `goal`, `voice_key`,
//! `inbox`, `recent_history`, `pending_history`, or `knows` ever appears here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    appearance::AppearanceSnapshot,
    character::{BodySlot, Control, StatusKind},
    gesture::GestureKind,
    ids::{ActorId, ItemId, MarkId},
    item::ItemKind,
    marks::MarkKind,
    math::Vec3,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicSnapshot {
    pub world_revision: i64,
    pub player_id: ActorId,
    /// Sorted by id.
    pub actors: Vec<ActorSnapshot>,
    /// Sorted by id.
    pub items: Vec<ItemSnapshot>,
    /// Sorted by `(created_seq, item_id)`.
    pub offers: Vec<OfferSnapshot>,
    /// Presentation-only carts derived from road-party topology and live cargo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub road_carts: Vec<crate::round::RoadCart>,
    /// The chalk on the walls (`features/chalking_the_walls.md`), sorted by id.
    /// Skipped when empty — the universal case, and every frozen fixture's
    /// case — so a city nobody has chalked serializes byte-identically to
    /// before the feature existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<PublicMark>,
}

/// One chalk mark, as the *renderer* needs it and no more.
///
/// The label and the meaning are deliberately absent: the host reads those
/// out of the same `assets/world/marks.json` the sim compiles in, so no prose
/// crosses the wire. `author` is absent because no reader may branch on it,
/// and `about` because the render has no use for it.
///
/// **`strength` is quantized on purpose.** A raw `f64` strength serializes at
/// full 17-significant-digit precision (`0.9330329915368074`) — 18 bytes for
/// precision an opacity ramp cannot use. Rounding an `f64` does not help
/// (`0.001`-rounded still prints `1.5709999999999997`); integer quantization
/// is the only encoding that is short by construction. At ≤102 bytes a mark,
/// [`MARKS_MAX`](crate::marks::MARKS_MAX) fits the snapshot's measured
/// 26,815-byte headroom several times over.
///
/// There is deliberately **no orientation here.** The sim does not know which
/// way a door faces: a [`PlaceEntry`](crate::places::PlaceEntry) carries a
/// walkable point and nothing else, and homes carry less. The host owns the
/// city geometry and the collision world, so it is the half of the seam that
/// can actually answer "which wall, facing where" — and a made-up yaw crossing
/// the wire would be a lie the renderer then had to honour.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PublicMark {
    pub id: MarkId,
    pub kind: MarkKind,
    /// The anchor site in metres. A plain array, not `vec3_serde`: the object
    /// form costs 12 bytes a mark to name axes the host never reads by name.
    pub point: [f64; 3],
    /// `strength * 100`, rounded — the opacity ramp. Whether a mark counts as
    /// half-washed is the catalog's `faint_below` applied to this, host-side.
    pub strength_pct: u8,
    /// Tally notches; `1` for every other kind.
    pub strokes: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActorSnapshot {
    pub id: ActorId,
    /// `"You"`, the real name if the player knows them, else
    /// `"a stranger (id {id})"`.
    pub name_for_player: String,
    pub control: Control,
    #[serde(with = "crate::math::vec3_serde")]
    pub position_m: Vec3,
    pub facing_yaw: f64,
    /// The sim-composed body appearance (`features/npc_bodies.md` §2): dress
    /// class, headgear, tint seed, and the named majors' bespoke override.
    pub appearance: AppearanceSnapshot,
    pub holds: Vec<ItemId>,
    /// The looping gesture the actor is holding, or `None`
    /// (`features/npc_bodies.md` §7). Only `dance` loops today; a one-shot
    /// gesture rides `EngineMessage::Gesture` and never lands here. Present so
    /// a player who arrives mid-loop still sees the dance. Skipped when `None`
    /// so the common case adds nothing to the 500-actor snapshot's size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_gesture: Option<GestureKind>,
    /// Publicly-visible carriage axes (`features/npc_bodies.md` §8): drunkenness,
    /// weariness, each a finite `0..=1`, ordered by kind. The host reads them to
    /// dress the walk (sway, stoop) without touching the actor's position.
    /// Skipped when empty — the universal case — so the 500-actor snapshot's
    /// size (and every frozen serialization test) is unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<(StatusKind, f32)>,
    /// Pocketed stack-units (`features/extra_pockets.md`): which held stack
    /// rides in which body slot, one entry per unit. The items stay in `holds`
    /// too — this is the reservation, which the host uses for the player's own
    /// inventory screen and to keep pocketed things out of NPC hands. Skipped
    /// when empty — the universal case — so the 500-actor snapshot's size is
    /// unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pockets: Vec<(BodySlot, ItemId)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemSnapshot {
    pub id: ItemId,
    pub kind: ItemKind,
    /// The catalog-derived display name, sent so the host never needs the
    /// catalog.
    pub display_name: String,
    /// The catalog-derived plural noun phrase ("loaves", "bowls of stew"), sent
    /// for the same reason: the host pluralizes counts without the catalog.
    pub display_plural: String,
    pub visual_key: String,
    /// How many units in this stack (always ≥ 1).
    pub quantity: u32,
    /// The catalog-declared descriptors that are part of stack identity.
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfferSnapshot {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub target_id: Option<ActorId>,
    pub created_seq: i64,
}
