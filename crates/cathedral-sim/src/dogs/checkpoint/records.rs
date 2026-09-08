//! Explicit closed v1 owner records, private to admitted component decoders.
#![allow(dead_code)]
use super::*;
use crate::checkpoint::{records::TextV1, serde_support::remote_adapters};
#[derive(Serialize, Deserialize)]
#[serde(remote = "DogCoat", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DogCoatV1 {
    Brindle,
    Black,
    Grey,
    Fawn,
    White,
    Pied,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Dog", deny_unknown_fields)]
pub(crate) struct DogV1 {
    id: DogId,
    #[serde(with = "TextV1")]
    name: String,
    #[serde(with = "TextV1")]
    description: String,
    #[serde(with = "DogCoatV1")]
    coat: DogCoat,
    build: f32,
    #[serde(with = "crate::math::vec3_serde")]
    base: Vec3,
    leash_m: f64,
    #[serde(with = "crate::math::vec3_serde")]
    position_m: Vec3,
    facing_yaw: f64,
    speed: f64,
    gait_phase: f64,
    #[serde(with = "crate::checkpoint::records::point::vec")]
    path: Vec<Vec3>,
    rest_s: f64,
    epoch: u64,
}
remote_adapters!(dog, Dog, DogV1);
