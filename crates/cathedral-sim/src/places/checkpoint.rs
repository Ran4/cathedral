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
#[cfg(test)]
mod complete_layout_tests {
    use super::*;
    use crate::checkpoint::{CheckpointBudget, Cohort, complete::meter::DecodeMeter};
    use std::mem::size_of;
    fn table<K, V>(map: &HashMap<K, V>) -> usize {
        if map.capacity() == 0 {
            return 0;
        }
        (map.capacity() + 1).next_power_of_two() * (size_of::<(K, V)>() + 1) + 64
    }
    #[derive(Serialize, Deserialize)]
    struct Wrapped(#[serde(with = "PlaceRegistryV1")] PlaceRegistry);
    #[test]
    fn complete_place_registry_rebuilt_indexes_fit_precharged_layout() {
        for n in [1, 6, 12, 64, 1000] {
            let mut places = PlaceRegistry::default();
            for i in 0..n {
                let id = ActorId::from_raw(format!("resident-{i}"));
                places.add_home(&id, &format!("resident number {i}"), Vec3::ZERO);
                places
                    .insert(PlaceEntry {
                        id: PlaceId::from_raw(format!("public-{i}")),
                        name: format!("public place {i}"),
                        point: Vec3::ZERO,
                        ward: Some("ward".into()),
                        coarse: false,
                    })
                    .unwrap();
            }
            let bytes = serde_json::to_vec(&Wrapped(places)).unwrap();
            let budget = CheckpointBudget::default();
            let mut reservation = budget.reserve(Cohort::LoadCandidate, 4096).unwrap();
            let meter = DecodeMeter::new(&mut reservation, bytes.len()).unwrap();
            let Wrapped(p) = meter.decode(&bytes).unwrap();
            let mut retained = size_of::<PlaceRegistry>()
                + p.entries.capacity() * size_of::<PlaceEntry>()
                + 32
                + table(&p.by_id)
                + table(&p.by_name)
                + table(&p.home_by_owner)
                + table(&p.owner_by_home);
            retained += p
                .entries
                .iter()
                .map(|e| {
                    e.id.allocated_bytes()
                        + e.name.capacity()
                        + e.ward.as_ref().map_or(0, String::capacity)
                        + 96
                })
                .sum::<usize>();
            retained += p
                .by_id
                .keys()
                .map(|id| id.allocated_bytes() + 32)
                .sum::<usize>();
            retained += p.by_name.keys().map(|s| s.capacity() + 32).sum::<usize>();
            retained += p
                .home_by_owner
                .keys()
                .map(|id| id.allocated_bytes() + 32)
                .sum::<usize>();
            retained += p
                .owner_by_home
                .iter()
                .map(|(place, owner)| place.allocated_bytes() + owner.allocated_bytes() + 64)
                .sum::<usize>();
            assert!(
                meter.expanded() >= retained,
                "n={n}, meter={}, actual capacity upper={retained}",
                meter.expanded()
            );
            assert_eq!(p.entries.len(), 2 * n);
            assert_eq!(p.home_by_owner.len(), n);
        }
    }
}
