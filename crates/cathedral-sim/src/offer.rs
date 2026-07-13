//! Pending offers (`sim.py:125-131`). `World.offers` is keyed by item id, so
//! at most one offer per item is live; re-offering replaces.

use serde::{Deserialize, Serialize};

use crate::ids::{ActorId, ItemId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    /// `None` = broadcast: anyone in range may accept, first wins.
    pub target_id: Option<ActorId>,
    /// The sequence of the `offer_item` event that created this offer.
    pub created_seq: i64,
}
