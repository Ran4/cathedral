//! Coarse top-down precipitation occlusion.
//!
//! The procedural city is batched, so asking render entities which roof owns a
//! point would be both slow and incomplete. This 2 m grid is instead baked once
//! from the same committed plan the city builder consumes, then augmented with
//! the deliberately permeable structures (passages, awnings and well roofs)
//! whose lower space matters to rain and listener audio.

use bevy::prelude::*;
use serde::Deserialize;

const CELL_M: f32 = 2.0;
const MIN_X: f32 = -620.0;
const MIN_Z: f32 = -700.0;
const MAX_X: f32 = 550.0;
const MAX_Z: f32 = 520.0;
const GROUND_Y: f32 = 0.72;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CoverMaterial {
    #[default]
    Open,
    Slate,
    Tile,
    Thatch,
    Stone,
    Timber,
    Canvas,
    #[allow(
        dead_code,
        reason = "reserved for glazed galleries in authored cover data"
    )]
    Glass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverSample {
    pub impact_y: f32,
    pub ground_y: f32,
    pub material: CoverMaterial,
    pub sheltered_listener: bool,
    /// True only on rendered non-Cut road ribbons and court/square paving.
    /// Used by the one-time puddle bake; it has no movement semantics.
    pub puddle_surface: bool,
}

impl Default for CoverSample {
    fn default() -> Self {
        Self {
            impact_y: GROUND_Y,
            ground_y: GROUND_Y,
            material: CoverMaterial::Open,
            sheltered_listener: false,
            puddle_surface: false,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct PrecipitationOcclusionMap {
    width: usize,
    height: usize,
    cells: Vec<CoverSample>,
}

impl Default for PrecipitationOcclusionMap {
    fn default() -> Self {
        Self::from_city_plan()
    }
}

#[derive(Deserialize)]
struct Plan {
    buildings: Vec<Building>,
    roads: Vec<Road>,
    sites: Vec<Site>,
}

#[derive(Deserialize)]
struct Building {
    id: String,
    polygon: Vec<[f32; 2]>,
    #[serde(rename = "use")]
    use_name: String,
    material: String,
    levels: u32,
}

#[derive(Deserialize)]
struct Road {
    points: Vec<[f32; 2]>,
    width_m: f32,
    tier: String,
}

#[derive(Deserialize)]
struct Site {
    polygon: Vec<[f32; 2]>,
    kind: String,
}

impl PrecipitationOcclusionMap {
    pub fn from_city_plan() -> Self {
        let width = ((MAX_X - MIN_X) / CELL_M).ceil() as usize;
        let height = ((MAX_Z - MIN_Z) / CELL_M).ceil() as usize;
        let mut map = Self {
            width,
            height,
            cells: vec![CoverSample::default(); width * height],
        };
        let plan: Plan =
            serde_json::from_str(include_str!("../../lore/places/ombreval_buildings.json"))
                .expect("the city plan already validated by the city builder");

        for road in &plan.roads {
            if road.tier == "cut" {
                continue;
            }
            for segment in road.points.windows(2) {
                map.rasterize_road_surface(segment[0], segment[1], road.width_m * 0.5);
            }
        }
        for site in &plan.sites {
            if matches!(
                site.kind.as_str(),
                "square" | "court" | "precinct" | "monument"
            ) {
                map.rasterize_puddle_polygon(&site.polygon);
            }
        }

        for building in plan.buildings {
            let material = roof_material(&building.material);
            let impact_y = if building.id == "named_lanthorn" {
                84.0
            } else {
                1.0 + building.levels.max(1) as f32 * 3.25 + 2.1
            };
            let permeable = building.use_name == "bridge"
                || matches!(
                    building.id.as_str(),
                    "named_lanthorn" | "named_malt_house" | "named_gaunt_house"
                );
            map.rasterize(&building.polygon, impact_y, material, permeable);
        }

        // The Bellfoot stair porch is authored separately from the plan's
        // building shells. Its generous roof rectangle is the named drive-mode
        // acceptance point at the tower's foot.
        map.rasterize_rect(
            Vec2::new(28.3, -176.0),
            Vec2::new(43.8, -158.0),
            8.2,
            CoverMaterial::Stone,
            true,
        );

        // Gate vaults: they are open routes under masonry, not house interiors.
        for (centre, half, height) in [
            (Vec2::new(-24.5, 332.0), Vec2::new(8.0, 10.0), 13.0),
            (Vec2::new(346.5, 94.5), Vec2::new(10.0, 8.0), 13.0),
            (Vec2::new(10.5, -465.5), Vec2::new(9.0, 9.0), 13.0),
            (Vec2::new(-353.5, -94.5), Vec2::new(9.0, 9.0), 13.0),
            (Vec2::new(-310.5, -377.5), Vec2::new(8.0, 8.0), 10.0),
        ] {
            map.rasterize_rect(
                centre - half,
                centre + half,
                height,
                CoverMaterial::Stone,
                true,
            );
        }

        // Social market roofs. Only their small pitch is covered; the square
        // around each remains honestly exposed.
        for centre in [
            Vec2::new(-15.05, 249.55),
            Vec2::new(-19.95, 247.45),
            Vec2::new(223.5, 108.57),
            Vec2::new(-213.43, 63.07),
            Vec2::new(-211.05, -254.45),
            Vec2::new(-215.95, -256.55),
        ] {
            map.rasterize_rect(
                centre - Vec2::new(3.2, 2.4),
                centre + Vec2::new(3.2, 2.4),
                3.4,
                CoverMaterial::Canvas,
                true,
            );
        }

        // The two cooked-food counters are inside public tavern hearth rooms.
        // Their graph place nodes sit at the serving pitches, so mark only the
        // compact rooms instead of treating the whole building shell as public
        // precipitation cover.
        for centre in [Vec2::new(-231.9, -324.7), Vec2::new(25.9, -173.9)] {
            map.rasterize_rect(
                centre - Vec2::splat(3.2),
                centre + Vec2::splat(3.2),
                7.0,
                CoverMaterial::Tile,
                true,
            );
        }

        // Roofed public water points and catch sheds. These are intentionally
        // explicit because their posts occupy only tiny collision footprints.
        for (centre, half, material, height) in [
            (
                Vec2::new(88.0, 35.0),
                Vec2::new(3.2, 3.0),
                CoverMaterial::Tile,
                4.4,
            ),
            (
                Vec2::new(172.0, 120.0),
                Vec2::new(3.1, 2.8),
                CoverMaterial::Slate,
                4.2,
            ),
            (
                Vec2::new(-239.2, 218.6),
                Vec2::new(3.4, 4.0),
                CoverMaterial::Tile,
                4.3,
            ),
            (
                Vec2::new(8.0, 254.4),
                Vec2::new(3.2, 1.8),
                CoverMaterial::Slate,
                3.2,
            ),
            (
                Vec2::new(135.0, 184.2),
                Vec2::new(3.8, 1.9),
                CoverMaterial::Tile,
                3.2,
            ),
            (
                Vec2::new(-196.5, -231.8),
                Vec2::new(3.1, 1.7),
                CoverMaterial::Tile,
                3.2,
            ),
        ] {
            map.rasterize_rect(centre - half, centre + half, height, material, true);
        }
        map
    }

    pub fn sample(&self, x: f32, z: f32) -> CoverSample {
        self.index(x, z)
            .map_or_else(CoverSample::default, |index| self.cells[index])
    }

    pub fn is_sheltered(&self, position: Vec3) -> bool {
        let sample = self.sample(position.x, position.z);
        sample.sheltered_listener && position.y < sample.impact_y - 0.15
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        (Vec2::new(MIN_X, MIN_Z), Vec2::new(MAX_X, MAX_Z))
    }

    fn index(&self, x: f32, z: f32) -> Option<usize> {
        if !x.is_finite() || !z.is_finite() || x < MIN_X || z < MIN_Z {
            return None;
        }
        let col = ((x - MIN_X) / CELL_M) as usize;
        let row = ((z - MIN_Z) / CELL_M) as usize;
        (row < self.height && col < self.width).then_some(row * self.width + col)
    }

    fn rasterize_rect(
        &mut self,
        min: Vec2,
        max: Vec2,
        impact_y: f32,
        material: CoverMaterial,
        sheltered: bool,
    ) {
        self.rasterize(
            &[
                [min.x, min.y],
                [max.x, min.y],
                [max.x, max.y],
                [min.x, max.y],
            ],
            impact_y,
            material,
            sheltered,
        );
    }

    fn rasterize(
        &mut self,
        polygon: &[[f32; 2]],
        impact_y: f32,
        material: CoverMaterial,
        sheltered: bool,
    ) {
        if polygon.len() < 3 || !impact_y.is_finite() {
            return;
        }
        let min_x = polygon.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = polygon
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_z = polygon.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_z = polygon
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let first_col = (((min_x - MIN_X) / CELL_M).floor() as isize).max(0) as usize;
        let last_col = (((max_x - MIN_X) / CELL_M).ceil() as usize).min(self.width);
        let first_row = (((min_z - MIN_Z) / CELL_M).floor() as isize).max(0) as usize;
        let last_row = (((max_z - MIN_Z) / CELL_M).ceil() as usize).min(self.height);
        for row in first_row..last_row {
            for col in first_col..last_col {
                let point = [
                    MIN_X + (col as f32 + 0.5) * CELL_M,
                    MIN_Z + (row as f32 + 0.5) * CELL_M,
                ];
                if !point_in_polygon(point, polygon) {
                    continue;
                }
                let cell = &mut self.cells[row * self.width + col];
                if impact_y >= cell.impact_y {
                    cell.impact_y = impact_y;
                    cell.material = material;
                }
                cell.sheltered_listener |= sheltered;
            }
        }
    }

    fn rasterize_road_surface(&mut self, a: [f32; 2], b: [f32; 2], half_width: f32) {
        if !half_width.is_finite() || half_width <= 0.0 {
            return;
        }
        let min_x = a[0].min(b[0]) - half_width;
        let max_x = a[0].max(b[0]) + half_width;
        let min_z = a[1].min(b[1]) - half_width;
        let max_z = a[1].max(b[1]) + half_width;
        let first_col = (((min_x - MIN_X) / CELL_M).floor() as isize).max(0) as usize;
        let last_col = (((max_x - MIN_X) / CELL_M).ceil() as usize).min(self.width);
        let first_row = (((min_z - MIN_Z) / CELL_M).floor() as isize).max(0) as usize;
        let last_row = (((max_z - MIN_Z) / CELL_M).ceil() as usize).min(self.height);
        for row in first_row..last_row {
            for col in first_col..last_col {
                let point = [
                    MIN_X + (col as f32 + 0.5) * CELL_M,
                    MIN_Z + (row as f32 + 0.5) * CELL_M,
                ];
                if distance_squared_to_segment(point, a, b) <= half_width * half_width {
                    self.cells[row * self.width + col].puddle_surface = true;
                }
            }
        }
    }

    fn rasterize_puddle_polygon(&mut self, polygon: &[[f32; 2]]) {
        if polygon.len() < 3 {
            return;
        }
        let min_x = polygon
            .iter()
            .map(|point| point[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = polygon
            .iter()
            .map(|point| point[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_z = polygon
            .iter()
            .map(|point| point[1])
            .fold(f32::INFINITY, f32::min);
        let max_z = polygon
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let first_col = (((min_x - MIN_X) / CELL_M).floor() as isize).max(0) as usize;
        let last_col = (((max_x - MIN_X) / CELL_M).ceil() as usize).min(self.width);
        let first_row = (((min_z - MIN_Z) / CELL_M).floor() as isize).max(0) as usize;
        let last_row = (((max_z - MIN_Z) / CELL_M).ceil() as usize).min(self.height);
        for row in first_row..last_row {
            for col in first_col..last_col {
                let point = [
                    MIN_X + (col as f32 + 0.5) * CELL_M,
                    MIN_Z + (row as f32 + 0.5) * CELL_M,
                ];
                if point_in_polygon(point, polygon) {
                    self.cells[row * self.width + col].puddle_surface = true;
                }
            }
        }
    }
}

fn distance_squared_to_segment(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let point = Vec2::from_array(point);
    let a = Vec2::from_array(a);
    let b = Vec2::from_array(b);
    let edge = b - a;
    let length_squared = edge.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance_squared(a);
    }
    let along = ((point - a).dot(edge) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(a + edge * along)
}

fn roof_material(material: &str) -> CoverMaterial {
    match material {
        "slate" | "limestone" => CoverMaterial::Slate,
        "terracotta" | "stone_timber" => CoverMaterial::Tile,
        "thatch" => CoverMaterial::Thatch,
        "timber" | "half_timber" => CoverMaterial::Timber,
        _ => CoverMaterial::Stone,
    }
}

fn point_in_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let [x0, z0] = polygon[current];
        let [x1, z1] = polygon[previous];
        if (z0 > point[1]) != (z1 > point[1])
            && point[0] < (x1 - x0) * (point[1] - z0) / (z1 - z0) + x0
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_cover_probes_and_open_square_are_classified() {
        let map = PrecipitationOcclusionMap::default();
        for (name, point) in [
            ("Lanthorn nave", Vec3::new(0.0, 1.5, 20.0)),
            ("Bellfoot Passage", Vec3::new(35.8, 1.5, -167.0)),
            ("Chain Bridge", Vec3::new(-213.5, 1.5, 297.5)),
            ("Wickmarket awning", Vec3::new(-15.05, 1.5, 249.55)),
            ("Ford Well roof", Vec3::new(88.0, 1.5, 35.0)),
        ] {
            assert!(map.is_sheltered(point), "{name} should be covered");
            assert!(map.sample(point.x, point.z).impact_y > point.y);
        }
        assert!(!map.is_sheltered(Vec3::new(-17.5, 1.5, 241.5)));
    }

    #[test]
    fn flying_above_a_roof_is_exposed_but_keeps_the_roof_impact() {
        let map = PrecipitationOcclusionMap::default();
        let sample = map.sample(0.0, 20.0);
        assert!(sample.impact_y > 50.0);
        assert!(!map.is_sheltered(Vec3::new(0.0, sample.impact_y + 5.0, 20.0)));
    }

    #[test]
    fn puddle_surface_follows_cobbles_and_excludes_the_dry_cut() {
        let map = PrecipitationOcclusionMap::default();
        assert!(map.sample(-17.5, 241.5).puddle_surface);
        assert!(!map.sample(-213.5, -140.0).puddle_surface);
    }

    #[test]
    fn committed_cover_grid_stays_within_its_memory_budget() {
        let map = PrecipitationOcclusionMap::default();
        let bytes = map.cells.len() * std::mem::size_of::<CoverSample>();
        assert!(bytes < 8 * 1024 * 1024, "cover map uses {bytes} bytes");
    }
}
