//! Read-only capacity inventory for complete-checkpoint Running evidence.
use super::*;
use std::mem::size_of;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct NavStorageInventory {
    pub nodes: usize,
    pub graph_and_indexes_bytes: usize,
    pub cache_rows_retained: usize,
    pub cache_retained_bytes: usize,
    pub cache_maximum_bytes: usize,
    pub total_with_full_cache_bytes: usize,
}
fn vector<T>(v: &Vec<T>) -> usize {
    v.capacity()
        .saturating_mul(size_of::<T>())
        .saturating_add(32)
}
fn string(s: &String) -> usize {
    s.capacity().saturating_add(32)
}
fn table<K, V>(v: &HashMap<K, V>) -> usize {
    if v.capacity() == 0 {
        return 0;
    }
    let buckets = v
        .capacity()
        .saturating_add(1)
        .checked_next_power_of_two()
        .unwrap_or(usize::MAX);
    buckets
        .saturating_mul(size_of::<(K, V)>().saturating_add(1))
        .saturating_add(64)
}
impl NavData {
    /// Counts retained capacities without filling a cache. Includes both the
    /// currently allocated cache rows and the upper bound if every row is warm.
    /// HashMap uses the exact-build SwissTable bucket/control-byte layout, with
    /// rounded bucket count and64 bytes for control tail/alignment/allocation.
    pub fn checkpoint_storage_inventory(&self) -> NavStorageInventory {
        let mut graph = size_of::<Self>()
            + vector(&self.bitset)
            + vector(&self.nodes)
            + vector(&self.adjacency)
            + self.adjacency.iter().map(vector).sum::<usize>()
            + vector(&self.places)
            + vector(&self.sites)
            + vector(&self.doors)
            + table(&self.place_by_name)
            + table(&self.door_by_building);
        graph += self
            .places
            .iter()
            .map(|p| string(&p.name) + string(&p.kind))
            .sum::<usize>();
        graph += self
            .sites
            .iter()
            .map(|p| string(&p.id) + string(&p.name))
            .sum::<usize>();
        graph += self
            .doors
            .iter()
            .map(|p| string(&p.building))
            .sum::<usize>();
        graph += self
            .place_by_name
            .keys()
            .chain(self.door_by_building.keys())
            .map(string)
            .sum::<usize>();
        graph +=
            vector(&self.resident_places.patches) + vector(&self.resident_places.shelter_spots);
        for p in &self.resident_places.patches {
            graph += string(&p.id)
                + string(&p.building)
                + string(&p.description)
                + vector(&p.spots)
                + vector(&p.graph_path)
                + vector(&p.home_path)
                + p.home_building.as_ref().map_or(0, string)
                + p.spots.iter().map(|s| string(&s.id)).sum::<usize>();
        }
        graph += self
            .resident_places
            .shelter_spots
            .iter()
            .map(|p| string(&p.shelter) + string(&p.spot.id))
            .sum::<usize>();
        if let Some(index) = &self.node_index {
            graph += vector(&index.starts) + vector(&index.members);
        }
        let nodes = self.nodes.len();
        let cache_base = self.distance_cache.0.len() * size_of::<OnceLock<Box<[f32]>>>() + 64;
        let rows = self.distance_cache.0.iter().filter_map(|r| r.get());
        let retained = cache_base
            + rows
                .clone()
                .map(|r| r.len() * size_of::<f32>() + 32)
                .sum::<usize>();
        let maximum = cache_base.saturating_add(
            nodes.saturating_mul(nodes.saturating_mul(size_of::<f32>()).saturating_add(32)),
        );
        NavStorageInventory {
            nodes,
            graph_and_indexes_bytes: graph,
            cache_rows_retained: rows.count(),
            cache_retained_bytes: retained,
            cache_maximum_bytes: maximum,
            total_with_full_cache_bytes: graph.saturating_add(maximum),
        }
    }
    /// Graph clones share this derived allocation; independent parses do not.
    pub fn checkpoint_shares_distance_cache(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.distance_cache.0, &other.distance_cache.0)
    }
}
