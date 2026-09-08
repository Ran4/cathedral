//! Ordered entries and home bindings are authoritative; lookup maps are rebuilt.
use super::*;
use crate::{
    character::checkpoint::point,
    checkpoint::{Result, records, serde_support::unique_map},
};
use serde::{Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    version: u16,
    #[serde(with = "records::place::vec")]
    entries: Vec<PlaceEntry>,
    #[serde(with = "unique_map")]
    homes: BTreeMap<ActorId, PlaceId>,
}
struct Homes<'a>(&'a PlaceRegistry);
impl Serialize for Homes<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(Some(self.0.owner_by_home.len()))?;
        for e in &self.0.entries {
            if let Some(owner) = self.0.owner_by_home.get(&e.id) {
                map.serialize_entry(owner, &e.id)?;
            }
        }
        map.end()
    }
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    #[serde(with = "records::place::vec")]
    entries: &'a [PlaceEntry],
    homes: Homes<'a>,
}
pub(crate) struct PlaceRegistryV1;
impl PlaceRegistryV1 {
    pub(crate) fn serialize<S: Serializer>(
        p: &PlaceRegistry,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        View {
            version: 1,
            entries: &p.entries,
            homes: Homes(p),
        }
        .serialize(s)
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<PlaceRegistry, D::Error> {
        let data = Data::deserialize(d)?;
        data.candidate().map_err(serde::de::Error::custom)
    }
}
impl Data {
    fn candidate(self) -> Result<PlaceRegistry> {
        check(self.version == 1, "unsupported place registry version")?;
        let mut p = PlaceRegistry {
            entries: self.entries,
            ..Default::default()
        };
        for (index, e) in p.entries.iter().enumerate() {
            check(e.id.is_valid(), "invalid place identity")?;
            point(e.point)?;
            check(
                p.by_id.insert(e.id.clone(), index).is_none(),
                "duplicate place identity",
            )?;
        }
        for (owner, home) in self.homes {
            check(owner.is_valid(), "invalid home owner identity")?;
            let index = *p.by_id.get(&home).ok_or_else(|| {
                crate::checkpoint::CheckpointError::new("places", "home references missing place")
            })?;
            check(
                p.owner_by_home.insert(home, owner.clone()).is_none(),
                "multiple owners of home",
            )?;
            p.home_by_owner.insert(owner, index);
        }
        for (index, e) in p.entries.iter().enumerate() {
            if !p.owner_by_home.contains_key(&e.id) {
                p.by_name.entry(e.name.clone()).or_insert(index);
            }
        }
        Ok(p)
    }
}
pub(crate) fn validate(
    p: &PlaceRegistry,
    actors: &BTreeMap<ActorId, crate::Character>,
) -> Result<()> {
    check(
        p.entries.len() == p.by_id.len() && p.owner_by_home.len() == p.home_by_owner.len(),
        "place indexes disagree",
    )?;
    let mut names = BTreeMap::new();
    for (index, e) in p.entries.iter().enumerate() {
        check(
            e.id.is_valid() && p.by_id.get(&e.id) == Some(&index),
            "place index identity mismatch",
        )?;
        point(e.point)?;
        if let Some(owner) = p.owner_by_home.get(&e.id) {
            check(
                actors.contains_key(owner) && p.home_by_owner.get(owner) == Some(&index),
                "invalid home binding",
            )?;
            check(
                e.ward.is_none() && !e.coarse,
                "home entry has public ward flags",
            )?;
        } else {
            let first = *names.entry(&e.name).or_insert(index);
            check(
                p.by_name.get(&e.name) == Some(&first),
                "place name index mismatch",
            )?;
        }
    }
    check(names.len() == p.by_name.len(), "extra place name index")?;
    Ok(())
}

impl PlaceRegistry {
    pub(crate) fn checkpoint_entry_count(&self) -> usize {
        self.entries.len()
    }
}

fn check(ok: bool, reason: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(crate::checkpoint::CheckpointError::new("places", reason))
    }
}
