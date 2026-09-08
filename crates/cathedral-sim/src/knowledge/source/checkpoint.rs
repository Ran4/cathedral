//! Private persistence bridge. No runtime provenance trait or public projection.
use super::*;
use crate::checkpoint::{CheckpointError, Result, records::TextV1};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Ref<'a> {
    Authored,
    Claimed(&'a ActorId),
    Custody(&'a ActorId),
    ItemWith {
        item: &'a ItemId,
        holder: &'a ActorId,
    },
    QuestPhase {
        quest: &'a str,
        phase: u8,
    },
    Event {
        kind: &'a str,
        sequence: i64,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Wire {
    Authored,
    Claimed(ActorId),
    Custody(ActorId),
    ItemWith {
        item: ItemId,
        holder: ActorId,
    },
    QuestPhase {
        #[serde(with = "TextV1")]
        quest: String,
        phase: u8,
    },
    Event {
        #[serde(with = "TextV1")]
        kind: String,
        sequence: i64,
    },
}
/// Only this adapter can see the payload. Its type has no Debug implementation.
pub(crate) struct SourceV1;
impl SourceV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &FactSource,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let value = match &v.0 {
            Provenance::Authored => Ref::Authored,
            Provenance::Claimed(a) => Ref::Claimed(a),
            Provenance::Custody(a) => Ref::Custody(a),
            Provenance::ItemWith { item, holder } => Ref::ItemWith { item, holder },
            Provenance::QuestPhase { quest, phase } => Ref::QuestPhase {
                quest,
                phase: *phase,
            },
            Provenance::Event { kind, sequence } => Ref::Event {
                kind,
                sequence: *sequence,
            },
        };
        value.serialize(s)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<FactSource, D::Error> {
        // Unknown variants, invalid scalar types and nested unknown fields may
        // include input contents in serde's error. Never forward that payload.
        let wire = Wire::deserialize(d)
            .map_err(|_| serde::de::Error::custom("invalid sealed fact provenance"))?;
        Ok(FactSource(match wire {
            Wire::Authored => Provenance::Authored,
            Wire::Claimed(a) => Provenance::Claimed(a),
            Wire::Custody(a) => Provenance::Custody(a),
            Wire::ItemWith { item, holder } => Provenance::ItemWith { item, holder },
            Wire::QuestPhase { quest, phase } => Provenance::QuestPhase { quest, phase },
            Wire::Event { kind, sequence } => Provenance::Event { kind, sequence },
        }))
    }
}
pub(crate) fn validate(v: &FactSource) -> Result<()> {
    let id = crate::ids::is_valid_id;
    let text = |s: &str| s.len() <= crate::checkpoint::records::MAX_TEXT_BYTES;
    let valid = match &v.0 {
        Provenance::Authored => true,
        Provenance::Claimed(a) | Provenance::Custody(a) => id(a.as_str()),
        Provenance::ItemWith { item, holder } => id(item.as_str()) && id(holder.as_str()),
        Provenance::QuestPhase { quest, .. } => text(quest),
        Provenance::Event { kind, .. } => text(kind),
    };
    if valid {
        Ok(())
    } else {
        Err(CheckpointError::new(
            "knowledge",
            "invalid sealed fact provenance",
        ))
    }
}
