//! Only the two publication variants owned by pollen/journal are admitted.
use super::*;
use crate::checkpoint::{records as common, serde_support::remote_adapters};
#[derive(Serialize, Deserialize)]
#[serde(remote = "JournalEntry", deny_unknown_fields)]
pub(crate) struct JournalEntryV1 {
    #[serde(with = "common::TextV1")]
    word: String,
    #[serde(with = "common::text::option")]
    from: Option<String>,
    #[serde(with = "common::text::option")]
    place: Option<String>,
    #[serde(with = "common::TextV1")]
    when: String,
    hops: u8,
    tellings: u16,
    wards: u8,
}
remote_adapters!(journal_entry, JournalEntry, JournalEntryV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "WardHeatRow", deny_unknown_fields)]
pub(crate) struct WardHeatRowV1 {
    ward: crate::lore::PlanningWard,
    #[serde(with = "common::TextV1")]
    label: String,
    #[serde(with = "crate::math::vec3_serde")]
    at: Vec3,
    heat_pct: u8,
    words: u8,
}
remote_adapters!(ward_heat_row, WardHeatRow, WardHeatRowV1);
pub(crate) struct JournalV1;
impl JournalV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &EngineMessage,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            #[serde(with = "journal_entry::vec")]
            entries: &'a [JournalEntry],
            standing: &'a [String],
        }
        match v {
            EngineMessage::Journal { entries, standing } => Ref { entries, standing }.serialize(s),
            _ => Err(serde::ser::Error::custom("invalid journal cache variant")),
        }
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<EngineMessage, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            #[serde(with = "journal_entry::vec")]
            entries: Vec<JournalEntry>,
            #[serde(with = "common::text::vec")]
            standing: Vec<String>,
        }
        let v = Wire::deserialize(d)?;
        Ok(EngineMessage::Journal {
            entries: v.entries,
            standing: v.standing,
        })
    }
}
remote_adapters!(journal, EngineMessage, JournalV1);
pub(crate) struct WardHeatV1;
impl WardHeatV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &EngineMessage,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            #[serde(with = "ward_heat_row::vec")]
            wards: &'a [WardHeatRow],
        }
        match v {
            EngineMessage::WardHeat { wards } => Ref { wards }.serialize(s),
            _ => Err(serde::ser::Error::custom("invalid ward heat cache variant")),
        }
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<EngineMessage, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            #[serde(with = "ward_heat_row::vec")]
            wards: Vec<WardHeatRow>,
        }
        let v = Wire::deserialize(d)?;
        Ok(EngineMessage::WardHeat { wards: v.wards })
    }
}
remote_adapters!(ward_heat, EngineMessage, WardHeatV1);
