//! Items (`sim.py:83-87`). Items live only in `World.items`; possession is
//! expressed by id in `Character.holds`.

use serde::{Deserialize, Serialize};

use crate::ids::ItemId;

fn generic() -> String {
    "generic".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    #[serde(default = "generic")]
    pub visual_key: String,
}

impl Item {
    /// An item with the default `generic` visual key.
    pub fn new(id: ItemId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            visual_key: generic(),
        }
    }
}
