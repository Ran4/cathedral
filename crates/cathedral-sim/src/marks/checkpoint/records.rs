//! Closed v1 shapes; no public Deserialize bypasses aggregate admission.
#![allow(dead_code, private_interfaces)]
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option},
};
#[derive(Serialize, Deserialize)]
#[serde(remote = "MarkAnchor", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum MarkAnchorV1 {
    Household(ActorId),
    Place(#[serde(with = "common::TextV1")] String),
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Mark", deny_unknown_fields)]
pub(crate) struct MarkV1 {
    kind: MarkKind,
    #[serde(with = "MarkAnchorV1")]
    anchor: MarkAnchor,
    #[serde(deserialize_with = "required_option")]
    about: Option<ActorId>,
    #[serde(deserialize_with = "required_option")]
    author: Option<ActorId>,
    drawn_game_days: f64,
    last_decayed_game_days: f64,
    strength: f64,
    strokes: u32,
}
remote_adapters!(mark, Mark, MarkV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Marks", deny_unknown_fields)]
pub(crate) struct MarksV1 {
    #[serde(with = "mark::map")]
    live: BTreeMap<MarkId, Mark>,
    next_id: u64,
    #[serde(with = "calendar_anchor")]
    last_sweep_game_days: f64,
    #[serde(with = "float_bits")]
    decay_scale: f64,
    #[serde(with = "calendar_anchor")]
    last_beat_game_days: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "MarkKindSwitches", deny_unknown_fields)]
pub(crate) struct SwitchesV1 {
    cross: bool,
    tally: bool,
    ward_sign: bool,
}
/// The stored config accepts NaN and infinities, and ordinary decay falls back
/// without changing that stored value. IEEE bits preserve every payload and -0.
pub(crate) mod float_bits {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        v.to_bits().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        u64::deserialize(d).map(f64::from_bits)
    }
}
pub(crate) mod calendar_anchor {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &f64, s: S) -> std::result::Result<S::Ok, S::Error> {
        checkpoint::CalendarAnchorV1::from_legacy(*v)
            .map_err(serde::ser::Error::custom)?
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<f64, D::Error> {
        let v = checkpoint::CalendarAnchorV1::deserialize(d)?;
        v.validate().map_err(serde::de::Error::custom)?;
        Ok(v.legacy())
    }
}
// Borrowed catalog fingerprint only; this is deliberately not a decoder.
#[derive(Serialize)]
#[serde(remote = "MarkKindSpec")]
pub(crate) struct SpecV1 {
    label: String,
    meaning: String,
    faint_label: String,
    anchors: Vec<AnchorSlot>,
    half_life_days_dry: f64,
    half_life_days_wet: f64,
    sheltered_multiplier: f64,
    faint_below: f64,
    gone_below: f64,
    drawable_by_hand: bool,
    places: BTreeMap<String, String>,
    _places_doc: Option<String>,
}
pub(crate) struct CatalogV1;
impl CatalogV1 {
    pub fn serialize<S: serde::Serializer>(
        v: &MarkCatalog,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        struct Ref<'a>(&'a MarkKindSpec);
        impl Serialize for Ref<'_> {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                SpecV1::serialize(self.0, s)
            }
        }
        let mut map = s.serialize_map(Some(v.kinds.len()))?;
        for (k, v) in &v.kinds {
            map.serialize_entry(k, &Ref(v))?;
        }
        map.end()
    }
}
