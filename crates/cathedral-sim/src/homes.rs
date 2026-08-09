//! The baked home-binding (`assets/world/homes.json`, written by
//! `scripts/bake_homes.py`), embedded like the rest of the round content so
//! both hosts get it with no wiring.
//!
//! Two consumers read it, for two different things. The **round** (`round.rs`)
//! reads `homes[*].point` — where the housed walk at the Snuffing. The **cast
//! loader** (`lore.rs`) reads `place_description` — the spoken form of the
//! same binding ("a house in the Cinder Ward, near the Shambles well"), which
//! becomes [`crate::lore::LoreProfile::home`] and reaches the prompt's
//! `**you**` line, so "Where do you live?" has a grounded answer consistent
//! with where the feet actually go
//! (`features/npc_knows_where_it_lives__inject_home_into_prompt.md`).
//!
//! The ~100 people with a homeless circumstance have no `homes` entry — no bed
//! is content (`features/implemented/movement/04_the_round.md` §3) — but they *do* get a
//! `bedless` entry: an explicit no-fixed-bed framing, so the model plays the
//! circumstance instead of silently inventing a cottage.

use std::collections::HashMap;

use serde::Deserialize;

/// The baked file, embedded. Static data, compiled in, not read at runtime:
/// the sim keeps its no-IO decree.
pub(crate) const HOMES_JSON: &str = include_str!("../../../assets/world/homes.json");

#[derive(Debug, Deserialize)]
pub(crate) struct HomesDoc {
    pub homes: HashMap<String, HomeEntry>,
    #[serde(default)]
    pub bedless: HashMap<String, BedlessEntry>,
}

/// One housed character's binding. The spatial fields beyond `point` (the
/// building id, the door node) are bake bookkeeping the sim never needs.
#[derive(Debug, Deserialize)]
pub(crate) struct HomeEntry {
    pub point: [f64; 2],
    #[serde(default)]
    pub place_description: Option<String>,
    /// The building's ward-level district, exactly as the bake read it off
    /// `lore/places/ombreval_buildings.json` ("Cinder Ward", "Bell and Sluice
    /// Wards"). Bookkeeping for the cast — the round never needs it — but it is
    /// the only ward map the sim has (see [`ward_marks`]).
    #[serde(default)]
    pub ward: Option<String>,
}

/// One bedless character's framing — prompt-only; there is nowhere to walk.
#[derive(Debug, Deserialize)]
pub(crate) struct BedlessEntry {
    pub place_description: String,
}

/// Every baked `place_description`, housed and bedless alike, keyed by actor
/// id. A character in neither map (a sheet added since the last bake) simply
/// gets no `home` line — the round content test pins full coverage, so this
/// degrades only between a new sheet and its re-bake.
pub(crate) fn place_descriptions() -> HashMap<String, String> {
    let Ok(doc) = serde_json::from_str::<HomesDoc>(HOMES_JSON) else {
        // Unreachable for the committed asset (tests parse it); an empty map
        // merely renders no home lines rather than poisoning world building.
        return HashMap::new();
    };
    let mut descriptions: HashMap<String, String> = doc
        .bedless
        .into_iter()
        .map(|(id, entry)| (id, entry.place_description))
        .collect();
    for (id, entry) in doc.homes {
        if let Some(description) = entry.place_description {
            descriptions.insert(id, description);
        }
    }
    descriptions
}

/// Every baked home as a *ward mark*: the door's XZ and the ward-level district
/// the bake read off that door's building.
///
/// The sim has no ward polygons. `lore/places/ombreval_buildings.json` carries
/// every building's district and is authoring input, not a shipped asset; the
/// eight ward anchors in `places.json` are single points, and a ward is not a
/// disc. What the sim *does* embed is these 413 doors, each already labelled by
/// the building it belongs to — so the nearest of them is the best ward map
/// available in-process, and a good one: measured against the authored district
/// of all 913 doors whose building has one, nearest-of-413 agrees on **94.2%**,
/// against 82.9% for the nearest of the eight ward anchors and 79.2% for the
/// nearest named place.
///
/// Used by [`crate::crowd`] to say which ward a generated citizen's door stands
/// in. Empty (rather than a panic) if the asset will not parse, which simply
/// leaves the crowd unhoused as it was before M4.
pub(crate) fn ward_marks() -> Vec<([f64; 2], String)> {
    let Ok(doc) = serde_json::from_str::<HomesDoc>(HOMES_JSON) else {
        return Vec::new();
    };
    let mut marks: Vec<([f64; 2], String)> = doc
        .homes
        .into_iter()
        .filter_map(|(_, entry)| Some((entry.point, entry.ward?)))
        .collect();
    // The map iterates in hash order; the nearest-mark search below breaks ties
    // by index, so the order has to be the same in every run.
    marks.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then(left.0[0].total_cmp(&right.0[0]))
            .then(left.0[1].total_cmp(&right.0[1]))
    });
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ward map the crowd's doors are labelled from: every baked home, its
    /// point and its building's district, in an order that does not depend on
    /// the hash map it came out of.
    #[test]
    fn the_ward_marks_cover_the_baked_homes_and_come_out_in_one_order() {
        let marks = ward_marks();
        assert!(marks.len() >= 400, "only {} ward marks", marks.len());
        assert_eq!(marks, ward_marks(), "the mark order must not wander");
        let wards: std::collections::BTreeSet<&str> =
            marks.iter().map(|(_, ward)| ward.as_str()).collect();
        assert!(
            wards.contains("Cinder Ward") && wards.len() >= 8,
            "the marks name only {wards:?}"
        );
    }

    /// The full coverage guard lives in `round/tests.rs` next to the other
    /// bake checks; this pins the merged view the cast loader consumes.
    #[test]
    fn the_merged_descriptions_speak_for_housed_and_bedless_alike() {
        let descriptions = place_descriptions();
        assert!(
            descriptions.len() >= 500,
            "only {} descriptions — the bake shrank",
            descriptions.len()
        );
        // A housed smith and the enclosed anchoress, both grounded.
        assert!(descriptions["sv3n1"].starts_with("a house in the "));
        assert!(descriptions["aq7ld"].contains("anchorhold cell"));
    }
}
