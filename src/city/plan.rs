//! Typed access to the authoritative cadastral plan in `lore/places`.
//!
//! The JSON and SVG are generated from the same source.  Keeping the game on
//! the JSON means the 3D city cannot silently drift away from the bird's-eye
//! map when a parcel is adjusted.

use std::collections::BTreeSet;

use serde::Deserialize;

pub(super) const SOURCE: &str = include_str!("../../lore/places/ombreval_buildings.json");

#[derive(Debug, Deserialize)]
pub(super) struct CityPlan {
    pub schema_version: u32,
    pub coordinate_system: CoordinateSystem,
    pub wall_polygon_xz: Vec<[f32; 2]>,
    pub buildings: Vec<Building>,
    pub sites: Vec<Site>,
    pub roads: Vec<Road>,
    pub fixtures: Vec<Fixture>,
    pub named_place_index: Vec<NamedPlace>,
    pub statistics: Statistics,
}

#[derive(Debug, Deserialize)]
pub(super) struct CoordinateSystem {
    pub units: String,
    pub north: String,
    pub east: String,
    pub up: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Building {
    pub id: String,
    pub name: Option<String>,
    pub polygon: Vec<[f32; 2]>,
    #[serde(rename = "use")]
    pub use_name: String,
    pub material: String,
    pub levels: u8,
    pub named: bool,
    pub district: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Site {
    pub id: String,
    pub name: String,
    pub polygon: Vec<[f32; 2]>,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Road {
    pub id: String,
    pub name: String,
    pub points: Vec<[f32; 2]>,
    pub width_m: f32,
    pub tier: String,
    pub label: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct Fixture {
    pub id: String,
    pub kind: String,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub angle_deg: f32,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct NamedPlace {
    pub number: u8,
    pub name: String,
    pub anchor: [f32; 2],
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Statistics {
    pub total_buildings: usize,
    pub named_or_reserved_buildings: usize,
    pub unnamed_urban_fabric_buildings: usize,
    pub roads: usize,
    pub named_places: usize,
    pub fixtures: usize,
}

pub(super) fn load() -> CityPlan {
    let plan: CityPlan = serde_json::from_str(SOURCE)
        .expect("lore/places/ombreval_buildings.json must be valid cadastral data");
    validate(&plan).expect("the authoritative Ombreval plan must remain internally consistent");
    plan
}

fn validate(plan: &CityPlan) -> Result<(), String> {
    if plan.schema_version != 1 {
        return Err(format!(
            "unsupported city-plan schema {}; expected 1",
            plan.schema_version
        ));
    }
    if plan.coordinate_system.units != "meters"
        || plan.coordinate_system.north != "+x"
        || plan.coordinate_system.east != "-z"
        || plan.coordinate_system.up != "+y"
    {
        return Err("city-plan coordinate convention no longer matches the game".into());
    }
    if plan.buildings.len() != plan.statistics.total_buildings
        || plan.roads.len() != plan.statistics.roads
        || plan.fixtures.len() != plan.statistics.fixtures
        || plan.named_place_index.len() != plan.statistics.named_places
    {
        return Err("city-plan statistics do not match their inventories".into());
    }
    if plan.statistics.named_or_reserved_buildings + plan.statistics.unnamed_urban_fabric_buildings
        != plan.statistics.total_buildings
    {
        return Err("named and ordinary building totals do not add up".into());
    }
    if plan.wall_polygon_xz.len() < 3 {
        return Err("wall polygon needs at least three points".into());
    }

    let mut ids = BTreeSet::new();
    for building in &plan.buildings {
        validate_polygon(&building.id, &building.polygon)?;
        if building.levels == 0 {
            return Err(format!("building '{}' has no levels", building.id));
        }
        if !ids.insert(building.id.as_str()) {
            return Err(format!("duplicate building id '{}'", building.id));
        }
        if building.named
            && building
                .name
                .as_deref()
                .is_none_or(|name| name.trim().is_empty())
        {
            return Err(format!("named building '{}' has no name", building.id));
        }
    }
    for site in &plan.sites {
        validate_polygon(&site.id, &site.polygon)?;
        if !ids.insert(site.id.as_str()) {
            return Err(format!("duplicate plan id '{}'", site.id));
        }
    }
    for road in &plan.roads {
        if road.points.len() < 2
            || !road.width_m.is_finite()
            || road.width_m <= 0.0
            || road.points.iter().flatten().any(|value| !value.is_finite())
        {
            return Err(format!("road '{}' has invalid geometry", road.id));
        }
        if !ids.insert(road.id.as_str()) {
            return Err(format!("duplicate plan id '{}'", road.id));
        }
    }
    for fixture in &plan.fixtures {
        if fixture.position.into_iter().any(|value| !value.is_finite())
            || fixture
                .size
                .into_iter()
                .any(|value| !value.is_finite() || value <= 0.0)
            || !fixture.angle_deg.is_finite()
        {
            return Err(format!("fixture '{}' has invalid geometry", fixture.id));
        }
        if !ids.insert(fixture.id.as_str()) {
            return Err(format!("duplicate plan id '{}'", fixture.id));
        }
    }

    for (index, place) in plan.named_place_index.iter().enumerate() {
        if place.number as usize != index + 1 {
            return Err("named-place numbering must be contiguous from one".into());
        }
        if place.name.trim().is_empty()
            || place.kind.trim().is_empty()
            || place.anchor.into_iter().any(|value| !value.is_finite())
        {
            return Err(format!("named place {} is invalid", place.number));
        }
    }

    Ok(())
}

fn validate_polygon(id: &str, polygon: &[[f32; 2]]) -> Result<(), String> {
    if polygon.len() < 3 || polygon.iter().flatten().any(|value| !value.is_finite()) {
        return Err(format!("'{id}' has an invalid polygon"));
    }
    if signed_area(polygon).abs() < 0.01 {
        return Err(format!("'{id}' has a degenerate polygon"));
    }
    Ok(())
}

pub(super) fn signed_area(polygon: &[[f32; 2]]) -> f32 {
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f32>()
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_plan_loads_with_every_inventory_entry() {
        let plan = load();

        assert_eq!(plan.buildings.len(), 2_566);
        assert_eq!(plan.statistics.named_or_reserved_buildings, 65);
        assert_eq!(plan.statistics.unnamed_urban_fabric_buildings, 2_501);
        assert_eq!(plan.roads.len(), 49);
        assert_eq!(plan.fixtures.len(), 91);
        assert_eq!(plan.named_place_index.len(), 69);
        assert_eq!(plan.sites.len(), 23);
    }

    #[test]
    fn fixed_lanthorn_and_map_orientation_have_not_drifted() {
        let plan = load();
        let lanthorn = plan
            .buildings
            .iter()
            .find(|building| building.id == "named_lanthorn")
            .expect("the Lanthorn footprint is required");

        assert_eq!(lanthorn.polygon.len(), 12);
        assert!(lanthorn.polygon.contains(&[-44.0, 81.0]));
        assert_eq!(plan.coordinate_system.north, "+x");
        assert_eq!(plan.coordinate_system.east, "-z");
    }

    #[test]
    fn every_numbered_place_has_a_unique_number_and_anchor() {
        let plan = load();
        let numbers = plan
            .named_place_index
            .iter()
            .map(|place| place.number)
            .collect::<BTreeSet<_>>();

        assert_eq!(numbers.len(), 69);
        assert_eq!(numbers.first(), Some(&1));
        assert_eq!(numbers.last(), Some(&69));
    }

    #[test]
    fn every_numbered_place_has_a_non_overlapping_simulation_area() {
        let plan = load();
        let map =
            cathedral_sim::AreaMap::from_json_str(include_str!("../../assets/world/areas.json"))
                .expect("the shipped area map must validate");
        let expected_area_ids = [
            "lanthorn_interior",
            "great_rose",
            "gradine",
            "dawn_bearer_vicinity",
            "seraph_vicinity",
            "skinners_court",
            "ford_well",
            "chapter_house",
            "wickmarket",
            "coswalds_yard",
            "tallage",
            "marens_green",
            "bellstand",
            "the_cut",
            "chain_bridge",
            "tally_bridge",
            "old_sluice",
            "ropewalk_cut",
            "shambles",
            "toll_house",
            "bonded_warehouse",
            "lise_copps_pawnshop",
            "ferrants_house",
            "gaunt_house",
            "gaunt_passage",
            "gaunt_weighing_yard",
            "saint_marens_church",
            "saint_marens_churchyard",
            "charnel_door",
            "alder_moorings",
            "eel_bridge",
            "hungry_ox",
            "tanners_slip",
            "eelback_alley",
            "marens_slip",
            "brine_cellar",
            "drapers_reach",
            "tenterhook_lane",
            "the_needle",
            "cinder_row",
            "burnt_court",
            "glaziers_guildhall",
            "masons_lodge",
            "malt_passage",
            "crookneck_lane",
            "osanne_vells_stall",
            "ilvane_chapel",
            "ilvane_anchorhold",
            "bellfoot_passage",
            "bellstand_tower",
            "colm_stone",
            "bellfounders_yard",
            "wool_gate",
            "stone_gate",
            "harne_gate",
            "river_gate",
            "reed_postern",
            "seven_lofts",
            "outer_wharves",
            "slate_cistern",
            "tenter_cistern",
            "lodge_well",
            "three_curb",
            "chain_well",
            "reed_cistern",
            "step_cistern",
            "bitter_well",
            "shambles_well",
            "seven_lofts_tanks",
        ];

        assert_eq!(map.areas.len(), 70, "69 lore places plus Lanthorn grounds");
        for (place, expected_id) in plan.named_place_index.iter().zip(expected_area_ids) {
            let height = if place.number == 2 { 84.0 } else { 0.91 };
            let position =
                cathedral_sim::Vec3::new(place.anchor[0] as f64, height, place.anchor[1] as f64);
            let actual = map.containing_area(position).unwrap_or_else(|| {
                panic!(
                    "place {} '{}' has no area at {:?}",
                    place.number, place.name, place.anchor
                )
            });
            assert_eq!(
                actual.id, expected_id,
                "place {} '{}' resolves to the wrong area",
                place.number, place.name
            );
        }
    }

    #[test]
    fn distributed_population_spawns_clear_city_buildings_walls_and_fixtures() {
        const CLEARANCE_M: f32 = 1.49;

        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let seed = cathedral_backends::world_data::load_world_seed(
            &root.join("assets"),
            &root.join("lore"),
        )
        .expect("the shipped population loads");
        let plan = load();
        let distributed: Vec<_> = seed
            .characters
            .iter()
            .filter(|character| {
                character
                    .lore
                    .as_ref()
                    .is_some_and(|lore| lore.significance != cathedral_sim::Significance::Major)
            })
            .collect();
        let non_major = cathedral_backends::world_data::character_sources(&root.join("lore"))
            .expect("the lore cast is readable")
            .iter()
            .filter(|(path, source)| {
                let sheet: serde_json::Value = serde_json::from_str(source)
                    .unwrap_or_else(|error| panic!("character file {path} does not parse: {error}"));
                sheet["significance"] != "major"
            })
            .count();
        assert_eq!(distributed.len(), non_major);

        for character in distributed {
            let point = [character.position_m.x as f32, character.position_m.z as f32];
            assert!(
                point_in_polygon(point, &plan.wall_polygon_xz)
                    && polygon_distance_squared(point, &plan.wall_polygon_xz)
                        >= CLEARANCE_M * CLEARANCE_M,
                "{} is outside or too close to the city wall",
                character.id
            );
            for building in &plan.buildings {
                assert!(
                    !point_in_polygon(point, &building.polygon)
                        && polygon_distance_squared(point, &building.polygon)
                            >= CLEARANCE_M * CLEARANCE_M,
                    "{} intersects building {}",
                    character.id,
                    building.id
                );
            }
            for fixture in &plan.fixtures {
                let angle = (-fixture.angle_deg).to_radians();
                let dx = point[0] - fixture.position[0];
                let dz = point[1] - fixture.position[1];
                let local_x = dx * angle.cos() - dz * angle.sin();
                let local_z = dx * angle.sin() + dz * angle.cos();
                let half_x = fixture.size[0] * 0.5 + CLEARANCE_M;
                let half_z = fixture.size[1] * 0.5 + CLEARANCE_M;
                assert!(
                    local_x.abs() >= half_x || local_z.abs() >= half_z,
                    "{} intersects fixture {}",
                    character.id,
                    fixture.id
                );
            }
        }
    }

    fn point_in_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
        let mut inside = false;
        for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
            if (a[1] > point[1]) != (b[1] > point[1])
                && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
            {
                inside = !inside;
            }
        }
        inside
    }

    fn polygon_distance_squared(point: [f32; 2], polygon: &[[f32; 2]]) -> f32 {
        polygon
            .iter()
            .zip(polygon.iter().cycle().skip(1))
            .map(|(a, b)| point_segment_distance_squared(point, *a, *b))
            .reduce(f32::min)
            .expect("validated polygons are non-empty")
    }

    fn point_segment_distance_squared(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
        let dx = b[0] - a[0];
        let dz = b[1] - a[1];
        let length_squared = dx * dx + dz * dz;
        if length_squared == 0.0 {
            return (point[0] - a[0]).powi(2) + (point[1] - a[1]).powi(2);
        }
        let along =
            (((point[0] - a[0]) * dx + (point[1] - a[1]) * dz) / length_squared).clamp(0.0, 1.0);
        (point[0] - (a[0] + along * dx)).powi(2) + (point[1] - (a[1] + along * dz)).powi(2)
    }
}
