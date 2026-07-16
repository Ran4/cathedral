//! The wayfinding registry — the sim side of `places_you_know` (M5,
//! `features/movement/05_the_llm_seam.md` §3).
//!
//! The model never sees a coordinate: it acts on **opaque `place_id` handles**
//! it has been given, the same mental model the sheet already uses for people.
//! This module owns the id → place mapping. The named places and the eight
//! ward anchors are baked by `scripts/bake_places.py` into
//! `assets/world/places.json` (embedded here, like the round content); the
//! per-character home entries — "Tam Rud's house" — are added at seed time,
//! because their names are the cast's.
//!
//! Which handles a given character *holds* is [`crate::CharacterState`]'s
//! `places_known`, seeded by the round and grown by `tell_way`. The registry is
//! the world's: one namespace, so a shared id means the same place to everyone.

use std::collections::HashMap;

use serde::Deserialize;

use crate::{
    ids::{ActorId, PlaceId},
    math::Vec3,
    nav::NavData,
};

/// The baked registry document. Embedded so both hosts get it with no wiring,
/// exactly as the round content is (`round.rs`).
const PLACES_JSON: &str = include_str!("../../../assets/world/places.json");

/// A registry that cannot be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceError {
    pub message: String,
}

impl std::fmt::Display for PlaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PlaceError {}

// --------------------------------------------------------------------------- //
// The baked document, straight off the JSON
// --------------------------------------------------------------------------- //
#[derive(Debug, Deserialize)]
struct PlacesDoc {
    schema_version: u32,
    places: Vec<PlaceDoc>,
    wards: Vec<WardDoc>,
}

#[derive(Debug, Deserialize)]
struct PlaceDoc {
    id: String,
    name: String,
    node: usize,
    kind: String,
    ward: String,
}

#[derive(Debug, Deserialize)]
struct WardDoc {
    id: String,
    ward: String,
    name: String,
    node: usize,
}

// --------------------------------------------------------------------------- //
// The public model
// --------------------------------------------------------------------------- //
/// One walkable destination the model can hold a handle to.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceEntry {
    pub id: PlaceId,
    /// Prompt-facing: how people *speak* of the place — grounding a spoken name
    /// against `places_you_know` happens on the LLM's side of the seam, so this
    /// name is the whole alias mechanism (05_the_llm_seam.md §3).
    pub name: String,
    /// The walkable point a `go_to` routes to (the place's nav node).
    pub point: Vec3,
    /// The planning ward the place lies in (snake_case) — how "the places of
    /// your own ward" are seeded. `None` for the ward anchors themselves and
    /// for homes.
    pub ward: Option<String>,
    /// Whether everyone in the city holds this handle: the major squares and
    /// the ward anchors, so getting somewhere always has a legal first step.
    pub coarse: bool,
}

/// The world's id → place mapping. Empty (the default) in a world without a
/// nav graph, which keeps every frozen fixture rendering an empty
/// `places_you_know`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceRegistry {
    entries: Vec<PlaceEntry>,
    by_id: HashMap<PlaceId, usize>,
    /// Nav-place display name → entry, for resolving workplace/leg labels.
    /// Homes are deliberately absent (two Tam Ruds would collide, and no round
    /// leg ever names a house by prose).
    by_name: HashMap<String, usize>,
    /// Housed character → their home's entry.
    home_by_owner: HashMap<ActorId, usize>,
}

impl PlaceRegistry {
    /// Parse and validate the embedded baked registry against the nav graph.
    pub fn from_embedded(nav: &NavData) -> Result<Self, PlaceError> {
        Self::from_json(PLACES_JSON, nav)
    }

    /// Parse and validate a registry document (tests hand in compact ones).
    pub fn from_json(json: &str, nav: &NavData) -> Result<Self, PlaceError> {
        let doc: PlacesDoc = serde_json::from_str(json)
            .map_err(|error| PlaceError { message: format!("invalid places.json: {error}") })?;
        if doc.schema_version != 1 {
            return Err(PlaceError {
                message: format!("unsupported places schema {}; expected 1", doc.schema_version),
            });
        }
        let mut registry = Self::default();
        for place in doc.places {
            if place.node >= nav.node_count() {
                return Err(PlaceError {
                    message: format!("place {:?} refers to node {} beyond the graph", place.name, place.node),
                });
            }
            registry.insert(PlaceEntry {
                id: PlaceId::from_raw(place.id),
                name: place.name,
                point: nav.node_point(place.node),
                ward: Some(place.ward),
                coarse: place.kind == "major",
            })?;
        }
        for ward in doc.wards {
            if ward.node >= nav.node_count() {
                return Err(PlaceError {
                    message: format!("ward {:?} refers to node {} beyond the graph", ward.ward, ward.node),
                });
            }
            registry.insert(PlaceEntry {
                id: PlaceId::from_raw(ward.id),
                name: ward.name,
                point: nav.node_point(ward.node),
                ward: None,
                coarse: true,
            })?;
        }
        Ok(registry)
    }

    fn insert(&mut self, entry: PlaceEntry) -> Result<usize, PlaceError> {
        if self.by_id.contains_key(&entry.id) {
            return Err(PlaceError {
                message: format!("duplicate place id '{}'", entry.id),
            });
        }
        let index = self.entries.len();
        self.by_id.insert(entry.id.clone(), index);
        self.by_name.entry(entry.name.clone()).or_insert(index);
        self.entries.push(entry);
        Ok(index)
    }

    /// Register a housed character's home — "Tam Rud's house" — under a
    /// deterministic opaque id. Idempotent per owner.
    pub fn add_home(&mut self, owner: &ActorId, owner_name: &str, point: Vec3) -> PlaceId {
        if let Some(&index) = self.home_by_owner.get(owner) {
            return self.entries[index].id.clone();
        }
        let id = self.free_id(&format!("home:{owner}"));
        let index = self.entries.len();
        self.by_id.insert(id.clone(), index);
        self.home_by_owner.insert(owner.clone(), index);
        self.entries.push(PlaceEntry {
            id: id.clone(),
            name: format!("{owner_name}'s house"),
            point,
            ward: None,
            coarse: false,
        });
        id
    }

    /// A `pl_xxxx` id from the key's hash, salted deterministically past any
    /// collision with the baked ids or earlier homes — the bake's scheme
    /// (`scripts/bake_places.py`), so every handle looks alike to the model.
    fn free_id(&self, key: &str) -> PlaceId {
        const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
        for salt in 0u64.. {
            let salted = if salt == 0 {
                key.to_string()
            } else {
                format!("{key}#{salt}")
            };
            let mut value = fnv1a64(&salted);
            let mut chars = [0u8; 4];
            for slot in &mut chars {
                *slot = ALPHABET[(value % 36) as usize];
                value /= 36;
            }
            let candidate = PlaceId::from_raw(format!(
                "pl_{}",
                std::str::from_utf8(&chars).expect("the alphabet is ASCII")
            ));
            if !self.by_id.contains_key(&candidate) {
                return candidate;
            }
        }
        unreachable!("36^4 ids cannot all be taken")
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: &PlaceId) -> Option<&PlaceEntry> {
        self.by_id.get(id).map(|&index| &self.entries[index])
    }

    /// The entry a nav-place display name resolves to (never a home).
    pub fn named(&self, name: &str) -> Option<&PlaceEntry> {
        self.by_name.get(name).map(|&index| &self.entries[index])
    }

    /// A housed character's home entry.
    pub fn home_of(&self, owner: &ActorId) -> Option<&PlaceEntry> {
        self.home_by_owner
            .get(owner)
            .map(|&index| &self.entries[index])
    }

    /// The handles everyone in the city holds — the major squares and the ward
    /// anchors, the "legal first step" of every journey.
    pub fn coarse(&self) -> impl Iterator<Item = &PlaceEntry> {
        self.entries.iter().filter(|entry| entry.coarse)
    }

    /// The named places of one planning ward (snake_case key).
    pub fn ward_places<'a>(&'a self, ward: &'a str) -> impl Iterator<Item = &'a PlaceEntry> {
        self.entries
            .iter()
            .filter(move |entry| entry.ward.as_deref() == Some(ward))
    }
}

fn fnv1a64(text: &str) -> u64 {
    let mut value: u64 = 0xCBF2_9CE4_8422_2325;
    for byte in text.as_bytes() {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x1_0000_0000_01B3);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line nav from `world.rs`'s movement tests: four nodes along +x.
    fn line_nav() -> NavData {
        let w = 60usize;
        let h = 10usize;
        let bitset = vec![0xFF_u8; (w * h).div_ceil(8)];
        let json = format!(
            r#"{{
              "schema_version": 1,
              "grid": {{"x0": -5.0, "z0": -5.0, "cell_m": 1.0, "w": {w}, "h": {h},
                        "agent_radius_m": 0.35, "bitset_file": "x.bin",
                        "bitset_bits": {bits}, "bitset_sha256": ""}},
              "nodes": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]],
              "edges": [[0, 1, 2.0], [1, 2, 2.0], [2, 3, 2.0]],
              "places": [{{"name": "a", "node": 0, "kind": "place"}},
                         {{"name": "b", "node": 3, "kind": "place"}}],
              "sites": [],
              "doors": [],
              "reference": {{"forecourt": 0}}
            }}"#,
            bits = w * h
        );
        NavData::from_parts(&json, &bitset).expect("the hand-built nav validates")
    }

    #[test]
    fn a_compact_registry_loads_and_answers_every_lookup() {
        let nav = line_nav();
        let json = r#"{
            "schema_version": 1,
            "places": [
                {"id": "pl_aaaa", "name": "The Square", "node": 0, "kind": "major", "ward": "reed"},
                {"id": "pl_bbbb", "name": "The Well", "node": 3, "kind": "landmark", "ward": "reed"}
            ],
            "wards": [
                {"id": "pl_cccc", "ward": "reed", "name": "Reed Ward", "node": 1}
            ]
        }"#;
        let mut registry = PlaceRegistry::from_json(json, &nav).expect("loads");
        assert_eq!(registry.len(), 3);

        let square = registry.named("The Square").expect("the square is named");
        assert!(square.coarse, "a major place is a coarse destination");
        assert!(!registry.named("The Well").unwrap().coarse);
        let coarse: Vec<&str> = registry.coarse().map(|entry| entry.name.as_str()).collect();
        assert_eq!(coarse, ["The Square", "Reed Ward"]);
        let ward: Vec<&str> = registry.ward_places("reed").map(|e| e.name.as_str()).collect();
        assert_eq!(ward, ["The Square", "The Well"]);

        let owner = ActorId::from_raw("tam4r");
        let home_id = registry.add_home(&owner, "Tam Rud", Vec3::new(20.0, 0.91, 0.0));
        assert!(home_id.as_str().starts_with("pl_"));
        assert_eq!(home_id.as_str().len(), 7, "pl_ + four base-36 characters");
        assert_eq!(registry.home_of(&owner).unwrap().name, "Tam Rud's house");
        assert_eq!(registry.get(&home_id).unwrap().name, "Tam Rud's house");
        // Idempotent: a second registration returns the same handle.
        assert_eq!(registry.add_home(&owner, "Tam Rud", Vec3::ZERO), home_id);
        // Homes never enter the name lookup.
        assert!(registry.named("Tam Rud's house").is_none());
    }

    #[test]
    fn the_embedded_registry_is_rejected_against_a_foreign_graph() {
        // The real places.json indexes nodes far beyond the 4-node line nav.
        let error = PlaceRegistry::from_embedded(&line_nav()).unwrap_err();
        assert!(error.message.contains("beyond the graph"), "{error}");
    }

    #[test]
    fn duplicate_ids_are_a_load_error() {
        let nav = line_nav();
        let json = r#"{
            "schema_version": 1,
            "places": [
                {"id": "pl_aaaa", "name": "One", "node": 0, "kind": "place", "ward": "reed"},
                {"id": "pl_aaaa", "name": "Two", "node": 1, "kind": "place", "ward": "reed"}
            ],
            "wards": []
        }"#;
        let error = PlaceRegistry::from_json(json, &nav).unwrap_err();
        assert!(error.message.contains("duplicate"), "{error}");
    }
}
