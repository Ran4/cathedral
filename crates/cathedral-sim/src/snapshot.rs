//! The public snapshot (`sim.py:323-368`) — everything the game may know.
//!
//! Privacy invariant: no `back_story`, `memories`, `goal`, `voice_key`,
//! `inbox`, `recent_history`, `pending_history`, or `knows` ever appears here.

use serde::{Deserialize, Serialize};

use crate::{
    character::Control,
    ids::{ActorId, ItemId},
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
    pub appearance_key: String,
    pub holds: Vec<ItemId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemSnapshot {
    pub id: ItemId,
    pub name: String,
    pub visual_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OfferSnapshot {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub target_id: Option<ActorId>,
    pub created_seq: i64,
}
