//! Closed v1 records; no seed defaults or runtime deserialization bypass.
#![allow(private_interfaces)]
use super::*;
use crate::checkpoint::serde_support::remote_adapters;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Future {
    Never,
    At(f64),
}
pub(crate) mod future {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        if *v == f64::INFINITY {
            Future::Never.serialize(s)
        } else if v.is_finite() && *v >= 0.0 {
            Future::At(*v).serialize(s)
        } else {
            Err(serde::ser::Error::custom("invalid continuity pacing"))
        }
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        match Future::deserialize(d)? {
            Future::Never => Ok(f64::INFINITY),
            Future::At(x) if x.is_finite() && x >= 0.0 => Ok(x),
            _ => Err(serde::de::Error::custom("invalid continuity pacing")),
        }
    }
}
mod event {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &SpeechEventId,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        if v.0.len() > MAX_EVENT_ID_BYTES {
            return Err(serde::ser::Error::custom("floor event id byte limit"));
        }
        v.0.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<SpeechEventId, D::Error> {
        Ok(SpeechEventId(
            crate::checkpoint::BoundedText::<MAX_EVENT_ID_BYTES>::deserialize(d)?.0,
        ))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "AwaitedSpeech", deny_unknown_fields)]
struct AwaitedV1 {
    #[serde(with = "event")]
    event_id: SpeechEventId,
    #[serde(with = "future")]
    deadline: f64,
    blocks_player_reaction: bool,
}
remote_adapters!(awaited, AwaitedSpeech, AwaitedV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "ConversationFloor", deny_unknown_fields)]
pub(crate) struct FloorV1 {
    #[serde(with = "awaited::vec")]
    awaiting: Vec<AwaitedSpeech>,
    #[serde(with = "future")]
    foreground_floor_until: f64,
    #[serde(with = "future")]
    background_floor_until: f64,
    #[serde(with = "future")]
    player_hold_until: f64,
}
