//! Explicit notice wire; historical settlement linkage is never reconstructed.
#![allow(dead_code, private_interfaces)] // Private remote-adapter family only.
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option, unique_set},
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(remote = "Rung", rename_all = "snake_case")]
pub(crate) enum RungV1 {
    Hearsay,
    Word,
    Summoned,
    Warranted,
}
remote_adapters!(rung, Rung, RungV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Summons", deny_unknown_fields)]
pub(crate) struct SummonsV1 {
    by: ActorId,
    office: Office,
    #[serde(deserialize_with = "required_option")]
    due_game_days: Option<f64>,
}
remote_adapters!(summons, Summons, SummonsV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "WardNotice", deny_unknown_fields)]
pub(crate) struct WardNoticeV1 {
    id: u64,
    #[serde(with = "common::TextV1")]
    about: String,
    #[serde(with = "common::TextV1")]
    deed: String,
    #[serde(with = "common::text::option")]
    place: Option<String>,
    #[serde(with = "common::text::option")]
    since: Option<String>,
    #[serde(deserialize_with = "required_option")]
    raised_game_days: Option<f64>,
    raised_by: ActorId,
    #[serde(deserialize_with = "required_option")]
    accused: Option<ActorId>,
    #[serde(deserialize_with = "required_option")]
    wronged: Option<ActorId>,
    #[serde(deserialize_with = "required_option")]
    taken: Option<ItemId>,
    #[serde(with = "summons::option")]
    summons: Option<Summons>,
    warrant: bool,
    hearsay: bool,
    #[serde(with = "unique_set")]
    served: BTreeSet<ActorId>,
}
remote_adapters!(notice, WardNotice, WardNoticeV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Notices", deny_unknown_fields)]
pub(crate) struct NoticesV1 {
    #[serde(with = "notice::vec")]
    live: Vec<WardNotice>,
    next_id: u64,
}
