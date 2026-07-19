//! The public snapshot (`sim.py:323-368`) — everything the game may know.
//!
//! Privacy invariant: no `back_story`, `memories`, `goal`, `voice_key`,
//! `inbox`, `recent_history`, `pending_history`, or `knows` ever appears here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    appearance::AppearanceSnapshot,
    character::{Control, StatusKind},
    gesture::GestureKind,
    ids::{ActorId, ItemId},
    item::ItemKind,
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
