//! Named world areas and position-to-place resolution.
//!
//! `assets/world/areas.json` is read by the host and handed to
//! [`AreaMap::from_json_str`].  This module deliberately has no filesystem or
//! Bevy dependency: area ownership and the words an actor sees in a prompt are
//! authoritative simulation behavior.

use std::{collections::BTreeSet, f64::consts::TAU, fmt};

use serde::{Deserialize, Serialize};

use crate::math::{Vec3, vec3_serde};

const SUPPORTED_SCHEMA_VERSION: u32 = 1;
const COMPASS_SECTORS: [&str; 16] = [
    "north",
    "north-northeast",
    "northeast",
    "east-northeast",
    "east",
    "east-southeast",
    "southeast",
    "south-southeast",
    "south",
    "south-southwest",
    "southwest",
    "west-southwest",
    "west",
    "west-northwest",
    "northwest",
    "north-northwest",
];

/// Area data that cannot be used without making location lookup ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaMapError {
    pub message: String,
}

impl AreaMapError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AreaMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AreaMapError {}

/// One signed world axis from the coordinate-system declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisDirection {
    #[serde(rename = "+x")]
    PositiveX,
    #[serde(rename = "-x")]
    NegativeX,
    #[serde(rename = "+y")]
    PositiveY,
    #[serde(rename = "-y")]
    NegativeY,
    #[serde(rename = "+z")]
    PositiveZ,
    #[serde(rename = "-z")]
    NegativeZ,
}

impl AxisDirection {
    fn vector(self) -> Vec3 {
        match self {
            Self::PositiveX => Vec3::X,
            Self::NegativeX => Vec3::NEG_X,
            Self::PositiveY => Vec3::Y,
            Self::NegativeY => Vec3::NEG_Y,
            Self::PositiveZ => Vec3::Z,
            Self::NegativeZ => Vec3::NEG_Z,
        }
    }

    fn unsigned_axis(self) -> char {
        match self {
            Self::PositiveX | Self::NegativeX => 'x',
            Self::PositiveY | Self::NegativeY => 'y',
            Self::PositiveZ | Self::NegativeZ => 'z',
        }
    }
}

/// How world axes map onto geographic language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateSystem {
    pub units: String,
    pub north: AxisDirection,
    pub east: AxisDirection,
    pub up: AxisDirection,
}

/// One non-overlapping rectangular part of a logical area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaBox {
    #[serde(with = "vec3_serde")]
    pub min_m: Vec3,
    #[serde(with = "vec3_serde")]
    pub max_m: Vec3,
}

impl AreaBox {
    /// Inclusive minimum and exclusive maximum on every axis.
    pub fn contains(&self, position: Vec3) -> bool {
        self.min_m.x <= position.x
            && position.x < self.max_m.x
            && self.min_m.y <= position.y
            && position.y < self.max_m.y
            && self.min_m.z <= position.z
            && position.z < self.max_m.z
    }

    fn overlaps_interior(&self, other: &Self) -> bool {
        self.min_m.x.max(other.min_m.x) < self.max_m.x.min(other.max_m.x)
            && self.min_m.y.max(other.min_m.y) < self.max_m.y.min(other.max_m.y)
            && self.min_m.z.max(other.min_m.z) < self.max_m.z.min(other.max_m.z)
    }

    fn nearest_horizontal_point(&self, position: Vec3) -> Vec3 {
        Vec3::new(
            position.x.clamp(self.min_m.x, self.max_m.x),
            position.y,
            position.z.clamp(self.min_m.z, self.max_m.z),
        )
    }
}

/// A logical named place. Its boxes are a union for lookup purposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub id: String,
    pub label: String,
    pub boxes: Vec<AreaBox>,
}

/// One logical area ranked by horizontal distance from a query position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestArea<'a> {
    pub area: &'a Area,
    pub horizontal_distance_m: f64,
}

/// The validated, authoritative place map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaMap {
    pub schema_version: u32,
    pub coordinate_system: CoordinateSystem,
    pub areas: Vec<Area>,
}

impl Default for AreaMap {
    fn default() -> Self {
        Self {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            coordinate_system: CoordinateSystem {
                units: "meters".to_string(),
                north: AxisDirection::PositiveX,
                east: AxisDirection::NegativeZ,
                up: AxisDirection::PositiveY,
            },
            areas: Vec::new(),
        }
    }
}

impl AreaMap {
    /// Parse and fully validate `assets/world/areas.json`.
    pub fn from_json_str(source: &str) -> Result<Self, AreaMapError> {
        let map: Self = serde_json::from_str(source)
            .map_err(|error| AreaMapError::new(format!("invalid area map: {error}")))?;
        map.validate()?;
        Ok(map)
    }

    pub fn validate(&self) -> Result<(), AreaMapError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(AreaMapError::new(format!(
                "unsupported area-map schema version {}; expected {SUPPORTED_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        self.validate_coordinate_system()?;

        let mut ids = BTreeSet::new();
        for area in &self.areas {
            if !valid_area_id(&area.id) {
                return Err(AreaMapError::new(format!(
                    "area id '{}' must contain only lowercase ASCII letters, digits, and underscores, and must start with a letter",
                    area.id
                )));
            }
            if !ids.insert(area.id.as_str()) {
                return Err(AreaMapError::new(format!(
                    "duplicate area id '{}'",
                    area.id
                )));
            }
            if area.label.trim().is_empty() {
                return Err(AreaMapError::new(format!(
                    "area '{}' has an empty label",
                    area.id
                )));
            }
            if area.boxes.is_empty() {
                return Err(AreaMapError::new(format!(
                    "area '{}' has no boxes",
                    area.id
                )));
            }
            for (box_index, bounds) in area.boxes.iter().enumerate() {
                for (axis, min, max) in [
                    ('x', bounds.min_m.x, bounds.max_m.x),
                    ('y', bounds.min_m.y, bounds.max_m.y),
                    ('z', bounds.min_m.z, bounds.max_m.z),
                ] {
                    if !min.is_finite() || !max.is_finite() {
                        return Err(AreaMapError::new(format!(
                            "area '{}' box {box_index} has a non-finite {axis} coordinate",
                            area.id
                        )));
                    }
                    if min >= max {
                        return Err(AreaMapError::new(format!(
                            "area '{}' box {box_index} needs min_m.{axis} < max_m.{axis}",
                            area.id
                        )));
                    }
                }
            }
        }

        let boxes: Vec<(&Area, usize, &AreaBox)> = self
            .areas
            .iter()
            .flat_map(|area| {
                area.boxes
                    .iter()
                    .enumerate()
                    .map(move |(index, bounds)| (area, index, bounds))
            })
            .collect();
        for (left_index, (left_area, left_box_index, left)) in boxes.iter().enumerate() {
            for (right_area, right_box_index, right) in &boxes[left_index + 1..] {
                if left.overlaps_interior(right) {
                    return Err(AreaMapError::new(format!(
                        "area boxes overlap: '{}' box {} and '{}' box {}",
                        left_area.id, left_box_index, right_area.id, right_box_index
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_coordinate_system(&self) -> Result<(), AreaMapError> {
        let coordinates = &self.coordinate_system;
        if coordinates.units != "meters" {
            return Err(AreaMapError::new(format!(
                "area-map units must be 'meters', got '{}'",
                coordinates.units
            )));
        }
        if coordinates.up.unsigned_axis() != 'y'
            || !matches!(coordinates.north.unsigned_axis(), 'x' | 'z')
            || !matches!(coordinates.east.unsigned_axis(), 'x' | 'z')
            || coordinates.north.unsigned_axis() == coordinates.east.unsigned_axis()
        {
            return Err(AreaMapError::new(
                "area-map coordinates must declare up on y and north/east on distinct x/z axes",
            ));
        }
        Ok(())
    }

    /// The containing logical area, if any.
    pub fn containing_area(&self, position: Vec3) -> Option<&Area> {
        self.areas
            .iter()
            .find(|area| area.boxes.iter().any(|bounds| bounds.contains(position)))
    }

    /// Logical areas nearest to `position`, ordered by distance and then stable
    /// ID. Distance is measured horizontally to the nearest point on any box
    /// in the area's union. The maximum distance is inclusive.
    pub fn nearest_areas(
        &self,
        position: Vec3,
        maximum_distance_m: f64,
        limit: usize,
    ) -> Vec<NearestArea<'_>> {
        if !maximum_distance_m.is_finite() || maximum_distance_m < 0.0 || limit == 0 {
            return Vec::new();
        }
        let maximum_squared = maximum_distance_m * maximum_distance_m;
        let mut nearest: Vec<NearestArea<'_>> = self
            .areas
            .iter()
            .filter_map(|area| {
                let distance_squared = nearest_horizontal_point(area, position)?.0;
                (distance_squared <= maximum_squared).then_some(NearestArea {
                    area,
                    horizontal_distance_m: distance_squared.sqrt(),
                })
            })
            .collect();
        nearest.sort_by(|left, right| {
            left.horizontal_distance_m
                .total_cmp(&right.horizontal_distance_m)
                .then_with(|| left.area.id.cmp(&right.area.id))
        });
        nearest.truncate(limit);
        nearest
    }

    /// Resolve the exact prompt description for a world position.
    pub fn location_description(&self, position: Vec3) -> Option<String> {
        if let Some(area) = self.containing_area(position) {
            return Some(area.label.clone());
        }

        let mut nearest: Option<(&Area, f64, Vec3)> = None;
        for area in &self.areas {
            let Some((distance_squared, point)) = nearest_horizontal_point(area, position) else {
                continue;
            };
            let replace = match nearest {
                None => true,
                Some((best_area, best_distance, _)) => {
                    distance_squared < best_distance
                        || (distance_squared == best_distance && area.id < best_area.id)
                }
            };
            if replace {
                nearest = Some((area, distance_squared, point));
            }
        }

        let (area, distance_squared, point) = nearest?;
        if distance_squared == 0.0 {
            return Some(format!("0 meters from {}", area.label));
        }
        let distance_m = round_half_up(distance_squared.sqrt());
        let noun = if distance_m == 1 { "meter" } else { "meters" };
        let direction = self.compass_direction(position - point);
        Some(format!("{distance_m} {noun} {direction} of {}", area.label))
    }

    fn compass_direction(&self, displacement: Vec3) -> &'static str {
        let north = displacement.dot(self.coordinate_system.north.vector());
        let east = displacement.dot(self.coordinate_system.east.vector());
        let clockwise = east.atan2(north).rem_euclid(TAU);
        let sector_width = TAU / COMPASS_SECTORS.len() as f64;
        let index = ((clockwise / sector_width + 0.5).floor() as usize) % COMPASS_SECTORS.len();
        COMPASS_SECTORS[index]
    }
}

fn nearest_horizontal_point(area: &Area, position: Vec3) -> Option<(f64, Vec3)> {
    let mut nearest: Option<(f64, Vec3)> = None;
    for bounds in &area.boxes {
        let point = bounds.nearest_horizontal_point(position);
        let distance_squared = (position.x - point.x).powi(2) + (position.z - point.z).powi(2);
        if nearest
            .as_ref()
            .is_none_or(|(best, _)| distance_squared < *best)
        {
            nearest = Some((distance_squared, point));
        }
    }
    nearest
}

fn valid_area_id(id: &str) -> bool {
    let mut characters = id.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn round_half_up(value: f64) -> u64 {
    (value + 0.5).floor() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(areas: &str) -> AreaMap {
        AreaMap::from_json_str(&format!(
            r#"{{
                "schema_version": 1,
                "coordinate_system": {{"units":"meters","north":"+x","east":"-z","up":"+y"}},
                "areas": [{areas}]
            }}"#
        ))
        .unwrap()
    }

    fn area(id: &str, label: &str, boxes: &str) -> String {
        format!(r#"{{"id":"{id}","label":"{label}","boxes":[{boxes}]}}"#)
    }

    fn bounds(min: (f64, f64, f64), max: (f64, f64, f64)) -> String {
        format!(
            r#"{{"min_m":{{"x":{},"y":{},"z":{}}},"max_m":{{"x":{},"y":{},"z":{}}}}}"#,
            min.0, min.1, min.2, max.0, max.1, max.2
        )
    }

    #[test]
    fn shipped_json_loads_and_has_the_initial_four_areas() {
        let map = AreaMap::from_json_str(include_str!("../../../assets/world/areas.json"))
            .expect("the shipped map loads");
        let ids: Vec<&str> = map.areas.iter().map(|area| area.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "lanthorn_grounds",
                "lanthorn_interior",
                "dawn_bearer_vicinity",
                "seraph_vicinity"
            ]
        );
        for (position, label) in [
            (
                Vec3::new(0.0, 0.91, 114.0),
                "The grounds of the Lanthorn (Great Church of Saint Ambrelle)",
            ),
            (
                Vec3::new(0.0, 0.91, 60.0),
                "Inside the Lanthorn (Great Church of Saint Ambrelle)",
            ),
            (
                Vec3::new(-72.0, 0.91, 190.0),
                "Next to the Dawn Bearer statue",
            ),
            (Vec3::new(72.0, 0.91, 190.0), "Next to the Seraph statue"),
        ] {
            assert_eq!(map.location_description(position).unwrap(), label);
        }
    }

    #[test]
    fn shared_boundaries_have_one_deterministic_owner() {
        let left = area("left", "Left", &bounds((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)));
        let right = area("right", "Right", &bounds((1.0, 0.0, 0.0), (2.0, 1.0, 1.0)));
        let map = map(&format!("{left},{right}"));
        assert_eq!(
            map.containing_area(Vec3::new(1.0, 0.5, 0.5)).unwrap().id,
            "right"
        );
    }

    #[test]
    fn overlap_is_rejected_with_both_box_names_even_within_one_area() {
        let boxes = format!(
            "{},{}",
            bounds((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
            bounds((1.0, 1.0, 1.0), (3.0, 3.0, 3.0))
        );
        let source = area("same", "Same", &boxes);
        let json = format!(
            r#"{{"schema_version":1,"coordinate_system":{{"units":"meters","north":"+x","east":"-z","up":"+y"}},"areas":[{source}]}}"#
        );
        let error = AreaMap::from_json_str(&json).unwrap_err();
        assert!(error.message.contains("'same' box 0"), "{error}");
        assert!(error.message.contains("'same' box 1"), "{error}");

        let left = area("left", "Left", &bounds((0.0, 0.0, 0.0), (2.0, 2.0, 2.0)));
        let right = area("right", "Right", &bounds((1.0, 1.0, 1.0), (3.0, 3.0, 3.0)));
        let json = format!(
            r#"{{"schema_version":1,"coordinate_system":{{"units":"meters","north":"+x","east":"-z","up":"+y"}},"areas":[{left},{right}]}}"#
        );
        let error = AreaMap::from_json_str(&json).unwrap_err();
        assert!(error.message.contains("'left' box 0"), "{error}");
        assert!(error.message.contains("'right' box 0"), "{error}");
    }

    #[test]
    fn duplicate_ids_empty_fields_bad_bounds_and_bad_coordinates_are_rejected() {
        let one = area("one", "One", &bounds((0.0, 0.0, 0.0), (1.0, 1.0, 1.0)));
        for (source, message) in [
            (format!("{one},{one}"), "duplicate area id"),
            (
                area("one", "  ", &bounds((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))),
                "empty label",
            ),
            (area("one", "One", ""), "has no boxes"),
            (
                area("one", "One", &bounds((1.0, 0.0, 0.0), (1.0, 1.0, 1.0))),
                "min_m.x",
            ),
        ] {
            let json = format!(
                r#"{{"schema_version":1,"coordinate_system":{{"units":"meters","north":"+x","east":"-z","up":"+y"}},"areas":[{source}]}}"#
            );
            let error = AreaMap::from_json_str(&json).unwrap_err();
            assert!(error.message.contains(message), "{error}");
        }

        let bad_axis = r#"{"schema_version":1,"coordinate_system":{"units":"meters","north":"+x","east":"-x","up":"+y"},"areas":[]}"#;
        assert!(AreaMap::from_json_str(bad_axis).is_err());
        let non_finite = r#"{"schema_version":1,"coordinate_system":{"units":"meters","north":"+x","east":"-z","up":"+y"},"areas":[{"id":"one","label":"One","boxes":[{"min_m":{"x":0,"y":0,"z":0},"max_m":{"x":1e400,"y":1,"z":1}}]}]}"#;
        assert!(AreaMap::from_json_str(non_finite).is_err());
    }

    #[test]
    fn a_multi_box_area_uses_union_containment_and_its_closest_box() {
        let boxes = format!(
            "{},{}",
            bounds((0.0, 0.0, 0.0), (1.0, 2.0, 1.0)),
            bounds((10.0, 0.0, 0.0), (11.0, 2.0, 1.0))
        );
        let map = map(&area("two_parts", "Two Parts", &boxes));
        assert_eq!(
            map.location_description(Vec3::new(10.5, 1.0, 0.5)).unwrap(),
            "Two Parts"
        );
        assert_eq!(
            map.location_description(Vec3::new(12.6, 1.0, 0.5)).unwrap(),
            "2 meters north of Two Parts"
        );
    }

    #[test]
    fn nearest_distance_uses_box_surface_rounds_half_up_and_singularizes_one() {
        let map = map(&area(
            "square",
            "The Square",
            &bounds((0.0, 0.0, 0.0), (10.0, 2.0, 10.0)),
        ));
        assert_eq!(
            map.location_description(Vec3::new(11.49, 1.0, 5.0))
                .unwrap(),
            "1 meter north of The Square"
        );
        assert_eq!(
            map.location_description(Vec3::new(11.5, 1.0, 5.0)).unwrap(),
            "2 meters north of The Square"
        );
        assert_eq!(
            map.location_description(Vec3::new(13.0, 1.0, 14.0))
                .unwrap(),
            "5 meters northwest of The Square"
        );
    }

    #[test]
    fn all_sixteen_compass_sectors_follow_declared_world_axes() {
        let map = map(&area(
            "point",
            "Point",
            &bounds((-0.1, 0.0, -0.1), (0.1, 1.0, 0.1)),
        ));
        let radius = 10.0;
        for (index, expected) in COMPASS_SECTORS.into_iter().enumerate() {
            let angle = index as f64 * TAU / 16.0;
            // north=+x and east=-z in this map.
            let position = Vec3::new(radius * angle.cos(), 0.5, -radius * angle.sin());
            let description = map.location_description(position).unwrap();
            assert!(
                description.contains(&format!(" {expected} of Point")),
                "{description}"
            );
        }
    }

    #[test]
    fn compass_language_changes_with_the_coordinate_metadata() {
        let source = area(
            "point",
            "Point",
            &bounds((-0.1, 0.0, -0.1), (0.1, 1.0, 0.1)),
        );
        let map = AreaMap::from_json_str(&format!(
            r#"{{"schema_version":1,"coordinate_system":{{"units":"meters","north":"-z","east":"-x","up":"+y"}},"areas":[{source}]}}"#
        ))
        .unwrap();
        assert_eq!(
            map.location_description(Vec3::new(0.0, 0.5, -10.0))
                .unwrap(),
            "10 meters north of Point"
        );
        assert_eq!(
            map.location_description(Vec3::new(-10.0, 0.5, 0.0))
                .unwrap(),
            "10 meters east of Point"
        );
    }

    #[test]
    fn equal_area_distance_ties_break_by_stable_id_not_file_order() {
        let z = area(
            "z_area",
            "Zed",
            &bounds((-2.0, 0.0, -1.0), (-1.0, 1.0, 1.0)),
        );
        let a = area(
            "a_area",
            "Alpha",
            &bounds((1.0, 0.0, -1.0), (2.0, 1.0, 1.0)),
        );
        let map = map(&format!("{z},{a}"));
        assert_eq!(
            map.location_description(Vec3::new(0.0, 0.5, 0.0)).unwrap(),
            "1 meter south of Alpha"
        );
    }

    #[test]
    fn directly_above_a_box_has_zero_horizontal_distance_and_no_bearing() {
        let map = map(&area(
            "square",
            "The Square",
            &bounds((0.0, 0.0, 0.0), (10.0, 2.0, 10.0)),
        ));
        assert_eq!(
            map.location_description(Vec3::new(5.0, 20.0, 5.0)).unwrap(),
            "0 meters from The Square"
        );
    }

    #[test]
    fn nearest_area_lists_limit_range_and_tie_break_by_stable_id() {
        let areas = (0..12)
            .rev()
            .map(|index| {
                let id = format!("area_{index:02}");
                let (min_x, max_x) = match index {
                    0 => (10.0, 11.0),
                    1 => (-11.0, -10.0),
                    3 => (350.0, 351.0),
                    _ => (index as f64 * 100.0, index as f64 * 100.0 + 1.0),
                };
                area(
                    &id,
                    &format!("Area {index}"),
                    &bounds((min_x, 0.0, 0.0), (max_x, 1.0, 1.0)),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let map = map(&areas);
        let nearest = map.nearest_areas(Vec3::ZERO, 350.0, 8);
        let ids: Vec<&str> = nearest
            .iter()
            .map(|match_| match_.area.id.as_str())
            .collect();
        assert_eq!(ids, ["area_00", "area_01", "area_02", "area_03"]);
        assert_eq!(nearest[0].horizontal_distance_m, 10.0);
        assert_eq!(nearest[1].horizontal_distance_m, 10.0);
        assert_eq!(nearest[3].horizontal_distance_m, 350.0);

        let limited = map.nearest_areas(Vec3::ZERO, 2_000.0, 8);
        assert_eq!(limited.len(), 8);
        assert_eq!(limited.last().unwrap().area.id, "area_07");
        assert!(map.nearest_areas(Vec3::ZERO, 350.0, 0).is_empty());
    }
}
