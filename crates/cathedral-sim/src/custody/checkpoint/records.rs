//! Explicit custody records. Station is frozen history, never a fresh lookup.
#![allow(dead_code, private_interfaces)] // Private remote-adapter family only.
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option},
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(remote = "Station", deny_unknown_fields)]
pub(crate) struct StationV1 {
    place_id: PlaceId,
    #[serde(with = "common::TextV1")]
    name: String,
    #[serde(with = "crate::math::vec3_serde")]
    point: Vec3,
    stone_house: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Confinement", rename_all = "snake_case")]
pub(crate) enum ConfinementV1 {
    InCharge,
    Committed,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "CustodyRecord", deny_unknown_fields)]
pub(crate) struct CustodyRecordV1 {
    #[serde(deserialize_with = "required_option")]
    officer: Option<ActorId>,
    holders: Vec<ActorId>,
    #[serde(deserialize_with = "required_option")]
    notice_id: Option<u64>,
    #[serde(with = "StationV1")]
    station: Station,
    #[serde(with = "ConfinementV1")]
    state: Confinement,
    authored: bool,
    closing: bool,
    #[serde(deserialize_with = "required_option")]
    sentence_office: Option<crate::clock::Office>,
    #[serde(deserialize_with = "required_option")]
    sentence_due_game_days: Option<f64>,
    seized_at: f64,
    #[serde(deserialize_with = "required_option")]
    committed_at: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    officer_last_turn: Option<f64>,
    struggles: u64,
}
remote_adapters!(record, CustodyRecord, CustodyRecordV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Custody", deny_unknown_fields)]
pub(crate) struct CustodyV1 {
    #[serde(with = "record::map")]
    held: BTreeMap<ActorId, CustodyRecord>,
}
