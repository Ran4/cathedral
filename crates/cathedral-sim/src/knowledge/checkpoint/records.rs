//! Explicit private records, with no authoring defaults or runtime serde.
#![allow(dead_code, private_interfaces)] // Remote adapters stay within the private owner.
use super::*;
use crate::checkpoint::{
    records as common,
    serde_support::{remote_adapters, required_option, unique_map, unique_set},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "FactKey", transparent)]
pub(crate) struct FactKeyV1(u32);
remote_adapters!(fact_key, FactKey, FactKeyV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "AreaKey", transparent)]
pub(crate) struct AreaKeyV1(u16);
remote_adapters!(area_key, AreaKey, AreaKeyV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "GarbleMask", deny_unknown_fields)]
pub(crate) struct GarbleV1 {
    subject: bool,
    place: bool,
    day: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Fact", deny_unknown_fields)]
pub(crate) struct FactV1 {
    id: FactId,
    #[serde(with = "FactKeyV1")]
    key: FactKey,
    sequence: i64,
    subject: Vec<ActorId>,
    #[serde(with = "area_key::option")]
    place: Option<AreaKey>,
    #[serde(deserialize_with = "required_option")]
    day: Option<i64>,
    #[serde(with = "common::TextV1")]
    said: String,
    #[serde(with = "common::text::map")]
    own: BTreeMap<ActorId, String>,
    #[serde(with = "unique_set")]
    seeded: BTreeSet<ActorId>,
    #[serde(with = "GarbleV1")]
    garble: GarbleMask,
    decays: bool,
    topic: Topic,
    #[serde(deserialize_with = "required_option")]
    minted_game_days: Option<f64>,
    #[serde(with = "unique_set")]
    quiet_among: BTreeSet<ActorId>,
    #[serde(with = "common::text::option")]
    craft_ear: Option<String>,
    #[serde(with = "super::super::source::checkpoint::SourceV1")]
    source: FactSource,
}
remote_adapters!(fact, Fact, FactV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "FactView", deny_unknown_fields)]
pub(crate) struct FactViewV1 {
    #[serde(deserialize_with = "required_option")]
    subject: Option<ActorId>,
    #[serde(with = "area_key::option")]
    place: Option<AreaKey>,
    day_offset: i8,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Holding", deny_unknown_fields)]
pub(crate) struct HoldingV1 {
    #[serde(with = "FactKeyV1")]
    key: FactKey,
    hops: u8,
    heat_at_learn: f32,
    #[serde(deserialize_with = "required_option")]
    learned_on: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    from: Option<ActorId>,
    #[serde(with = "FactViewV1")]
    view: FactView,
}
remote_adapters!(holding, Holding, HoldingV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "Drift", deny_unknown_fields)]
pub(crate) struct DriftV1 {
    heat: f32,
    hops: u8,
    #[serde(deserialize_with = "required_option")]
    via: Option<ActorId>,
    stir: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Occasion", deny_unknown_fields)]
pub(crate) struct OccasionV1 {
    #[serde(deserialize_with = "required_option")]
    subject: Option<ActorId>,
    #[serde(deserialize_with = "required_option")]
    from: Option<ActorId>,
    at_game_days: f64,
    offered: bool,
}
remote_adapters!(occasion, Occasion, OccasionV1);
#[derive(Serialize, Deserialize)]
#[serde(remote = "LearnedHow", deny_unknown_fields)]
pub(crate) struct LearnedV1 {
    #[serde(with = "common::TextV1")]
    word: String,
    #[serde(deserialize_with = "required_option")]
    at: Option<f64>,
    #[serde(with = "area_key::option")]
    place: Option<AreaKey>,
    #[serde(deserialize_with = "required_option")]
    from: Option<ActorId>,
    hops: u8,
    tellings: u16,
    wards: u8,
    wards_seen: u8,
    #[serde(with = "unique_set")]
    mouths_seen: BTreeSet<ActorId>,
    unattributed_seen: bool,
}
remote_adapters!(learned, LearnedHow, LearnedV1);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HoldingRow {
    actor: ActorId,
    #[serde(with = "holding::vec")]
    rows: Vec<Holding>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AirRow {
    ward: PlanningWard,
    key: u32,
    #[serde(with = "DriftV1")]
    drift: Drift,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    #[serde(with = "fact::vec")]
    live: Vec<Fact>,
    by_id: Vec<(FactId, u32)>,
    next_key: u32,
    next_sequence: i64,
    holdings: Vec<HoldingRow>,
    air: Vec<AirRow>,
    last_sweep_game_days: checkpoint::CalendarAnchorV1,
    #[serde(with = "occasion::map")]
    occasions: BTreeMap<ActorId, Occasion>,
    #[serde(with = "unique_map")]
    raises: BTreeMap<ActorId, (i64, Office, u8)>,
    #[serde(with = "learned::map")]
    player_learned: BTreeMap<FactId, LearnedHow>,
    receipts_revision: u64,
    #[serde(deserialize_with = "required_option")]
    seated: Option<(ActorId, Vec<u32>)>,
    #[serde(deserialize_with = "required_option")]
    player_stage_stir: Option<u32>,
    player_stage_tellings: Vec<(u32, ActorId)>,
    last_hearsay_beat_game_days: checkpoint::CalendarAnchorV1,
    hearsay_raised: Vec<(u32, ActorId)>,
}
struct Facts<'a>(&'a BTreeMap<FactKey, Fact>);
impl Serialize for Facts<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a>(#[serde(with = "FactV1")] &'a Fact);
        s.collect_seq(self.0.values().map(Ref))
    }
}
struct ById<'a>(&'a BTreeMap<FactId, FactKey>);
impl Serialize for ById<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|(id, key)| (id, key.0)))
    }
}
struct Holdings<'a>(&'a BTreeMap<ActorId, Vec<Holding>>);
impl Serialize for Holdings<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            actor: &'a ActorId,
            #[serde(with = "holding::vec")]
            rows: &'a [Holding],
        }
        s.collect_seq(self.0.iter().map(|(actor, rows)| Ref { actor, rows }))
    }
}
struct Air<'a>(&'a BTreeMap<(PlanningWard, FactKey), Drift>);
impl Serialize for Air<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            ward: PlanningWard,
            key: u32,
            #[serde(with = "DriftV1")]
            drift: &'a Drift,
        }
        s.collect_seq(self.0.iter().map(|((ward, key), drift)| Ref {
            ward: *ward,
            key: key.0,
            drift,
        }))
    }
}
struct Pairs<'a>(&'a BTreeSet<(FactKey, ActorId)>);
impl Serialize for Pairs<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|(key, actor)| (key.0, actor)))
    }
}
struct Keys<'a>(&'a [FactKey]);
impl Serialize for Keys<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|k| k.0))
    }
}
pub(crate) struct KnowledgeV1;
impl KnowledgeV1 {
    pub(crate) fn serialize<S: serde::Serializer>(
        v: &Knowledge,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Ref<'a> {
            live: Facts<'a>,
            by_id: ById<'a>,
            next_key: u32,
            next_sequence: i64,
            holdings: Holdings<'a>,
            air: Air<'a>,
            last_sweep_game_days: checkpoint::CalendarAnchorV1,
            #[serde(with = "occasion::map")]
            occasions: &'a BTreeMap<ActorId, Occasion>,
            raises: &'a BTreeMap<ActorId, (i64, Office, u8)>,
            #[serde(with = "learned::map")]
            player_learned: &'a BTreeMap<FactId, LearnedHow>,
            receipts_revision: u64,
            seated: Option<(&'a ActorId, Keys<'a>)>,
            player_stage_stir: Option<u32>,
            player_stage_tellings: Pairs<'a>,
            last_hearsay_beat_game_days: checkpoint::CalendarAnchorV1,
            hearsay_raised: Pairs<'a>,
        }
        Ref {
            live: Facts(&v.live),
            by_id: ById(&v.by_id),
            next_key: v.next_key,
            next_sequence: v.next_sequence,
            holdings: Holdings(&v.holdings),
            air: Air(&v.air),
            last_sweep_game_days: checkpoint::CalendarAnchorV1::from_legacy(v.last_sweep_game_days)
                .map_err(serde::ser::Error::custom)?,
            occasions: &v.occasions,
            raises: &v.raises,
            player_learned: &v.player_learned,
            receipts_revision: v.receipts_revision,
            seated: v.seated.as_ref().map(|(a, k)| (a, Keys(k))),
            player_stage_stir: v.player_stage_stir,
            player_stage_tellings: Pairs(&v.player_stage_tellings),
            last_hearsay_beat_game_days: checkpoint::CalendarAnchorV1::from_legacy(
                v.last_hearsay_beat_game_days,
            )
            .map_err(serde::ser::Error::custom)?,
            hearsay_raised: Pairs(&v.hearsay_raised),
        }
        .serialize(s)
    }
    pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Knowledge, D::Error> {
        let v = Wire::deserialize(d)?;
        fn map<K: Ord, V, E: serde::de::Error>(
            rows: impl IntoIterator<Item = (K, V)>,
        ) -> std::result::Result<BTreeMap<K, V>, E> {
            let mut out = BTreeMap::new();
            for (key, value) in rows {
                if out.insert(key, value).is_some() {
                    return Err(E::custom("duplicate knowledge key"));
                }
            }
            Ok(out)
        }
        fn set<E: serde::de::Error>(
            rows: Vec<(u32, ActorId)>,
        ) -> std::result::Result<BTreeSet<(FactKey, ActorId)>, E> {
            let mut out = BTreeSet::new();
            for (key, actor) in rows {
                if !out.insert((FactKey(key), actor)) {
                    return Err(E::custom("duplicate knowledge pair"));
                }
            }
            Ok(out)
        }
        v.last_sweep_game_days
            .validate()
            .map_err(serde::de::Error::custom)?;
        v.last_hearsay_beat_game_days
            .validate()
            .map_err(serde::de::Error::custom)?;
        Ok(Knowledge {
            live: map(v.live.into_iter().map(|f| (f.key, f)))?,
            by_id: map(v.by_id.into_iter().map(|(id, key)| (id, FactKey(key))))?,
            next_key: v.next_key,
            next_sequence: v.next_sequence,
            holdings: Arc::new(map(v.holdings.into_iter().map(|r| (r.actor, r.rows)))?),
            air: Arc::new(map(v
                .air
                .into_iter()
                .map(|r| ((r.ward, FactKey(r.key)), r.drift)))?),
            last_sweep_game_days: v.last_sweep_game_days.legacy(),
            occasions: v.occasions,
            raises: v.raises,
            player_learned: v.player_learned,
            receipts_revision: v.receipts_revision,
            seated: v
                .seated
                .map(|(a, k)| (a, k.into_iter().map(FactKey).collect())),
            player_stage_stir: v.player_stage_stir,
            player_stage_tellings: set(v.player_stage_tellings)?,
            last_hearsay_beat_game_days: v.last_hearsay_beat_game_days.legacy(),
            hearsay_raised: set(v.hearsay_raised)?,
        })
    }
}

#[cfg(test)]
pub(super) fn layout() {
    macro_rules! row {($($t:ty),*)=>{$(println!("{}={}",stringify!($t),std::mem::size_of::<$t>());)*}}
    row!(
        Wire, HoldingRow, AirRow, FactV1, HoldingV1, LearnedV1, FactViewV1, OccasionV1, DriftV1
    );
}
