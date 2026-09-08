//! Closed constructor records: a new runtime field requires an owner decision.
#![allow(private_interfaces)]
use super::*;
use crate::checkpoint::{
    records::{OperationIdV1, TextV1, text},
    serde_support::{remote_adapters, required_option},
};
#[derive(Serialize, Deserialize)]
#[serde(remote = "TurnLane", rename_all = "snake_case", deny_unknown_fields)]
enum LaneV1 {
    PlayerReaction,
    Handoff,
    Idle,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "InFlight", deny_unknown_fields)]
struct FlightV1 {
    actor_id: ActorId,
    presence_epoch: u64,
    #[serde(with = "crate::traits::checkpoint::request_id")]
    request_id: RequestId,
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    #[serde(with = "LaneV1")]
    lane: TurnLane,
    #[serde(with = "text::vec")]
    drained_events: Vec<String>,
    #[serde(with = "text::vec")]
    presented: Vec<String>,
    #[serde(with = "TextV1")]
    prompt: String,
}
remote_adapters!(flight, InFlight, FlightV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "RetryWork", deny_unknown_fields)]
struct RetryV1 {
    #[serde(with = "OperationIdV1")]
    semantic: OperationId,
    presence_epoch: u64,
}
remote_adapters!(retry, RetryWork, RetryV1);
/// Infinity is a reachable normalized delay/backoff or future pacing anchor.
/// Finite historical future values are not constrained by the capture horizon.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Future {
    Never,
    At(f64),
}
mod future {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        if *v == f64::INFINITY {
            Future::Never.serialize(s)
        } else if v.is_finite() && *v >= 0.0 {
            Future::At(*v).serialize(s)
        } else {
            Err(serde::ser::Error::custom("invalid scheduler pacing"))
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        match Future::deserialize(d)? {
            Future::Never => Ok(f64::INFINITY),
            Future::At(x) if x.is_finite() && x >= 0.0 => Ok(x),
            _ => Err(serde::de::Error::custom("invalid scheduler pacing")),
        }
    }
}
pub(crate) mod raw_float {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Bits {
        bits: u64,
    }
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        Bits { bits: v.to_bits() }.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        Ok(f64::from_bits(Bits::deserialize(d)?.bits))
    }
}
mod count {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &usize,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        u64::try_from(*v)
            .map_err(serde::ser::Error::custom)?
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<usize, D::Error> {
        usize::try_from(u64::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "NpcScheduler", deny_unknown_fields)]
pub(crate) struct SchedulerV1 {
    order: Vec<ActorId>,
    #[serde(with = "future")]
    minimum_delay_seconds: f64,
    #[serde(with = "future")]
    maximum_backoff_seconds: f64,
    #[serde(with = "count")]
    round_robin_index: usize,
    priority_handoffs: VecDeque<ActorId>,
    player_reactions: VecDeque<ActorId>,
    #[serde(with = "flight::option")]
    in_flight: Option<InFlight>,
    #[serde(with = "retry::map")]
    retry_work: BTreeMap<ActorId, RetryWork>,
    #[serde(with = "crate::traits::checkpoint::completion::option")]
    held_result: Option<Completion>,
    #[serde(with = "future")]
    next_turn_at: f64,
    provider_failures: u32,
    running: bool,
    #[serde(deserialize_with = "required_option")]
    submitted: Option<ActorId>,
}
