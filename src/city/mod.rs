//! The cadastral city of Ombreval.
//!
//! `lore/places/ombreval_buildings.json` is the same plan that produces the
//! zoomable bird's-eye SVG.  The game consumes those exact parcels, routes,
//! sites, fixtures, storey counts, materials, and stable IDs instead of
//! inventing a second procedural grid here.

mod monuments;
mod plan;
pub mod water;

use std::{
    collections::{BTreeMap, HashMap},
    f32::consts::PI,
};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::{
    controller::CollisionWorld,
    materials::{FLOOR_TEXTURE_SPAN_METERS, load_repeating_texture},
};

use monuments::build_approach_monuments;
use plan::{Building, CityPlan, Fixture, Road, Site};

const GROUND_MIN_X: f32 = -710.0;
const GROUND_MAX_X: f32 = 550.0;
const GROUND_MIN_Z: f32 = -745.0;
const GROUND_MAX_Z: f32 = 650.0;
const WALL_HEIGHT: f32 = 14.0;
const WALL_THICKNESS: f32 = 3.2;
const BUILDING_FLOOR_HEIGHT: f32 = 3.15;

pub struct CityPlugin;

impl Plugin for CityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_city);
    }
}

#[derive(Resource, Debug, Clone, Copy)]
#[allow(dead_code)] // Runtime inventory evidence, read directly by regression tests and inspectors.
struct CityBuildStats {
    planned_buildings: usize,
    rendered_plan_buildings: usize,
    named_places: usize,
    roads: usize,
    sites: usize,
    fixtures: usize,
    wharf_sheds: usize,
}

#[derive(Component, Debug)]
#[allow(dead_code)] // Stable lore metadata for debug picking and regression coverage.
struct LorePlaceNumber(u8);

#[derive(Clone)]
struct CityMeshes {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    sphere: Handle<Mesh>,
    pyramid: Handle<Mesh>,
    /// A cylinder with its middle taken out: the hollow mouth of a well.
    curb_ring: Handle<Mesh>,
}

#[derive(Clone)]
struct CityMaterials {
    ground: Handle<StandardMaterial>,
    cobbles: Handle<StandardMaterial>,
    paving: Handle<StandardMaterial>,
    dry_cut: Handle<StandardMaterial>,
    yard: Handle<StandardMaterial>,
    limestone: Handle<StandardMaterial>,
    fieldstone: Handle<StandardMaterial>,
    plaster: Handle<StandardMaterial>,
    half_timber: Handle<StandardMaterial>,
    terracotta: Handle<StandardMaterial>,
    slate: Handle<StandardMaterial>,
    thatch: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
    dark_wood: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    bronze: Handle<StandardMaterial>,
    window: Handle<StandardMaterial>,
    /// The warm pane of a hanging lantern: lit from within, always.
    lantern_glass: Handle<StandardMaterial>,
    cloth_ochre: Handle<StandardMaterial>,
    cloth_russet: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    /// The wet lining you see when you lean over a curb.
    well_shaft: Handle<StandardMaterial>,
    /// Water at the bottom of a shaft or behind a draw hatch: the same stuff as
    /// `water`, read in the dark.
    well_water: Handle<StandardMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum WallKind {
    Limestone,
    Fieldstone,
    Plaster,
    HalfTimber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RoofKind {
    Terracotta,
    Slate,
    Thatch,
}

struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    /// Multiplied into every vertex written while it is set — the per-building
    /// tint jitter and grime bands ride this brush into the batched mesh.
    brush: [f32; 4],
}

impl Default for MeshData {
    fn default() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            brush: [1.0; 4],
        }
    }
}

impl MeshData {
    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Every vertex written until the next call is multiplied by this colour.
    /// One value per building breaks the 2,500-clones monotony for free; the
    /// per-vertex grime gradient in `add_extruded_walls` stacks on top of it.
    fn set_brush(&mut self, color: [f32; 3]) {
        self.brush = [color[0], color[1], color[2], 1.0];
    }

    fn reset_brush(&mut self) {
        self.brush = [1.0; 4];
    }

    fn vertex(&mut self, position: Vec3, normal: Vec3, uv: Vec2) -> u32 {
        self.vertex_shaded(position, normal, uv, 1.0)
    }

    /// `shade` darkens toward 0.0 on top of the brush — the grime dial.
    fn vertex_shaded(&mut self, position: Vec3, normal: Vec3, uv: Vec2, shade: f32) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push(position.to_array());
        self.normals.push(normal.normalize_or(Vec3::Y).to_array());
        self.uvs.push(uv.to_array());
        self.colors.push([
            self.brush[0] * shade,
            self.brush[1] * shade,
            self.brush[2] * shade,
            self.brush[3],
        ]);
        index
    }

    fn quad(&mut self, points: [Vec3; 4], normal: Vec3, uvs: [Vec2; 4]) {
        let first = self.positions.len() as u32;
        for (point, uv) in points.into_iter().zip(uvs) {
            self.vertex(point, normal, uv);
        }
        self.indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    fn triangle(&mut self, mut points: [Vec3; 3], uvs: [Vec2; 3], upward: bool) {
        let mut normal = (points[1] - points[0]).cross(points[2] - points[0]);
        let mut uvs = uvs;
        if upward && normal.y < 0.0 {
            points.swap(1, 2);
            uvs.swap(1, 2);
            normal = -normal;
        }
        let first = self.positions.len() as u32;
        for (point, uv) in points.into_iter().zip(uvs) {
            self.vertex(point, normal, uv);
        }
        self.indices
            .extend_from_slice(&[first, first + 1, first + 2]);
    }

    fn into_mesh(self) -> Mesh {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_indices(Indices::U32(self.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        if self.colors.iter().any(|color| *color != [1.0; 4]) {
            mesh.with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        } else {
            mesh
        }
    }
}

fn build_city(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut collision_world: ResMut<CollisionWorld>,
) {
    let plan = plan::load();
    let doors = door_edges();
    let city_meshes = create_meshes(&mut meshes);
    let city_materials = create_materials(&asset_server, &mut materials);

    build_ground_context(
        &mut commands,
        &mut meshes,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_sites_and_roads(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan.sites,
        &plan.roads,
    );
    let rendered_plan_buildings = build_buildings(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan,
        &doors,
        &mut collision_world,
    );
    build_fixtures(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan.fixtures,
        &mut collision_world,
    );
    build_named_details(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan,
        &mut collision_world,
    );
    build_street_galleries(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan,
        &mut collision_world,
    );
    build_covered_passages(&mut commands, &mut meshes, &city_materials, &plan);
    build_square_arcades(&mut commands, &mut meshes, &city_materials, &plan, &doors);
    build_yard_stairs(
        &mut commands,
        &mut meshes,
        &city_materials,
        &plan,
        &doors,
        &mut collision_world,
    );
    build_street_props(&mut commands, &mut meshes, &city_materials, &plan, &doors);
    build_fortifications(
        &mut commands,
        &city_meshes,
        &city_materials,
        &plan,
        &mut collision_world,
    );
    build_approach_monuments(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut collision_world,
    );
    spawn_place_markers(&mut commands, &plan);

    commands.insert_resource(CityBuildStats {
        planned_buildings: plan.buildings.len(),
        rendered_plan_buildings,
        named_places: plan.named_place_index.len(),
        roads: plan.roads.len(),
        sites: plan.sites.len(),
        fixtures: plan.fixtures.len(),
        wharf_sheds: 15,
    });
}

fn create_meshes(meshes: &mut Assets<Mesh>) -> CityMeshes {
    CityMeshes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16).build()),
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(16, 10)),
        pyramid: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4).build()),
        curb_ring: meshes.add(water::curb_ring_mesh()),
    }
}

fn create_materials(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> CityMaterials {
    let textured = |materials: &mut Assets<StandardMaterial>,
                    path: &'static str,
                    tint: Color,
                    roughness: f32| {
        materials.add(StandardMaterial {
            base_color: tint,
            base_color_texture: Some(load_repeating_texture(asset_server, path)),
            perceptual_roughness: roughness,
            reflectance: 0.28,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };

    CityMaterials {
        ground: textured(
            materials,
            "textures/ombreval_yard_ground.png",
            Color::srgb(0.62, 0.60, 0.54),
            0.97,
        ),
        cobbles: textured(
            materials,
            "textures/ombreval_cobbles.png",
            Color::srgb(0.72, 0.70, 0.66),
            0.9,
        ),
        paving: textured(
            materials,
            "textures/ombreval_paving.png",
            Color::srgb(0.78, 0.75, 0.69),
            0.86,
        ),
        dry_cut: textured(
            materials,
            "textures/ombreval_dry_cut.png",
            Color::srgb(0.72, 0.68, 0.59),
            0.98,
        ),
        yard: textured(
            materials,
            "textures/ombreval_yard_ground.png",
            Color::srgb(0.62, 0.58, 0.50),
            0.97,
        ),
        limestone: textured(
            materials,
            "textures/ombreval_limestone.png",
            Color::srgb(0.90, 0.87, 0.80),
            0.82,
        ),
        fieldstone: textured(
            materials,
            "textures/ombreval_fieldstone.png",
            Color::srgb(0.72, 0.69, 0.65),
            0.91,
        ),
        plaster: textured(
            materials,
            "textures/ombreval_plaster.png",
            Color::srgb(0.86, 0.81, 0.70),
            0.9,
        ),
        half_timber: textured(
            materials,
            "textures/ombreval_half_timber.png",
            Color::srgb(0.82, 0.78, 0.70),
            0.87,
        ),
        terracotta: textured(
            materials,
            "textures/ombreval_terracotta.png",
            Color::srgb(0.78, 0.70, 0.62),
            0.88,
        ),
        slate: textured(
            materials,
            "textures/ombreval_slate.png",
            Color::srgb(0.72, 0.75, 0.78),
            0.8,
        ),
        thatch: textured(
            materials,
            "textures/ombreval_thatch.png",
            Color::srgb(0.72, 0.67, 0.55),
            0.96,
        ),
        timber: textured(
            materials,
            "textures/ombreval_timber.png",
            Color::srgb(0.72, 0.65, 0.56),
            0.88,
        ),
        dark_wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.075, 0.045, 0.028),
            perceptual_roughness: 0.86,
            // Door leaves and shutters are authored as single wall-plane
            // panels; without this, half of them face the wrong way and
            // vanish, leaving pale see-through doorways.
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        iron: matte(materials, Color::srgb(0.055, 0.06, 0.06), 0.68),
        bronze: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.12, 0.075),
            metallic: 0.8,
            perceptual_roughness: 0.46,
            ..default()
        }),
        window: materials.add(StandardMaterial {
            base_color: Color::srgb(0.035, 0.045, 0.052),
            emissive: LinearRgba::rgb(0.012, 0.014, 0.014),
            // Rough enough that the environment map reads as a dull sheen, not
            // a sky-mirror: leaded quarrel glass, and most panes sit in shade.
            perceptual_roughness: 0.55,
            reflectance: 0.32,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        lantern_glass: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.55),
            emissive: LinearRgba::rgb(5.5, 2.8, 0.9),
            perceptual_roughness: 0.3,
            ..default()
        }),
        cloth_ochre: materials.add(StandardMaterial {
            base_color: Color::srgb(0.33, 0.24, 0.12),
            perceptual_roughness: 0.92,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        cloth_russet: materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.10, 0.07),
            perceptual_roughness: 0.92,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(0.12, 0.27, 0.30, 0.94),
            metallic: 0.05,
            perceptual_roughness: 0.2,
            reflectance: 0.62,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        well_shaft: materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.11, 0.10),
            perceptual_roughness: 0.95,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        // A shaft is a hole with a roof over it, so nothing down there catches
        // the sun. The faint emissive is what a real well surface gets for free
        // and this one cannot: the sky, bounced back up at whoever leans in.
        well_water: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.19, 0.21),
            emissive: LinearRgba::rgb(0.02, 0.05, 0.06),
            metallic: 0.15,
            perceptual_roughness: 0.08,
            reflectance: 0.85,
            ..default()
        }),
    }
}

fn matte(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    roughness: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        ..default()
    })
}

fn build_ground_context(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    primitives: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let mut ground = MeshData::default();
    add_surface_quad(
        &mut ground,
        GROUND_MIN_X,
        GROUND_MAX_X,
        GROUND_MIN_Z,
        GROUND_MAX_Z,
        -0.035,
        9.0,
    );
    spawn_batch(
        commands,
        meshes,
        &materials.ground,
        ground,
        "Ombreval ground",
    );
    collision_world.add_box(
        Vec3::new(GROUND_MIN_X, -1.2, GROUND_MIN_Z),
        Vec3::new(GROUND_MAX_X, 0.0, GROUND_MAX_Z),
    );

    // The Serle remains wholly beyond the south wall, exactly as on the map.
    let mut river = MeshData::default();
    add_surface_quad(&mut river, -690.0, -575.0, -735.0, 640.0, 0.025, 18.0);
    spawn_batch(commands, meshes, &materials.water, river, "The Serle");

    // The SVG includes fifteen individual wharf sheds and quay reaches outside
    // the machine-readable urban-building inventory.  They are nevertheless
    // authored map buildings and therefore belong in the 3D context.
    for index in 0_usize..15 {
        let z = 95.0 - index as f32 * 38.0;
        let center = Vec3::new(-568.0, 3.0, z);
        spawn_box_named(
            commands,
            primitives,
            if index.is_multiple_of(2) {
                &materials.fieldstone
            } else {
                &materials.timber
            },
            center,
            Vec3::new(24.0, 6.0, 27.0),
            format!("Outer wharf shed {:02}", index + 1),
        );
        spawn_mesh_named(
            commands,
            &primitives.pyramid,
            &materials.terracotta,
            Transform::from_xyz(-568.0, 7.3, z).with_scale(Vec3::new(18.5, 3.0, 20.5)),
            format!("Outer wharf shed {:02} roof", index + 1),
        );
        collision_world.add_box(
            Vec3::new(-580.0, 0.0, z - 13.5),
            Vec3::new(-556.0, 8.5, z + 13.5),
        );

        spawn_box_named(
            commands,
            primitives,
            &materials.timber,
            Vec3::new(-590.0, 0.18, z),
            Vec3::new(16.0, 0.35, 27.0),
            format!("Outer wharf quay {:02}", index + 1),
        );
        for post_z in [z - 12.0, z + 12.0] {
            spawn_cylinder(
                commands,
                primitives,
                &materials.dark_wood,
                Vec3::new(-598.0, 1.5, post_z),
                0.35,
                3.0,
            );
        }
    }
}

fn build_sites_and_roads(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    sites: &[Site],
    roads: &[Road],
) {
    let mut paved_sites = MeshData::default();
    let mut yard_sites = MeshData::default();

    for site in sites {
        let center = polygon_center(&site.polygon);
        commands.spawn((
            Name::new(format!("{} site [{}]", site.name, site.id)),
            Transform::from_xyz(center.x, 0.012, center.y),
            Visibility::default(),
        ));
        // The cathedral has its own non-overlapping floor and apron meshes.
        if site.id == "lanthorn_precinct" {
            continue;
        }
        let target = match site.kind.as_str() {
            "square" | "monument" | "precinct" => &mut paved_sites,
            _ => &mut yard_sites,
        };
        add_polygon_surface(target, &site.polygon, 0.012, FLOOR_TEXTURE_SPAN_METERS);
    }
    spawn_batch(
        commands,
        meshes,
        &materials.paving,
        paved_sites,
        "Named paved places",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.yard,
        yard_sites,
        "Named working grounds",
    );

    let mut cobbles = MeshData::default();
    let mut cut = MeshData::default();
    for road in roads {
        let marker = road.points[road.points.len() / 2];
        let prefix = if road.label { "Named route" } else { "Street" };
        commands.spawn((
            Name::new(format!("{prefix}: {} [{}]", road.name, road.id)),
            Transform::from_xyz(marker[0], 0.024, marker[1]),
            Visibility::default(),
        ));
        let target = if road.tier == "cut" {
            &mut cut
        } else {
            &mut cobbles
        };
        add_road_ribbon(target, road, 0.024);
    }
    spawn_batch(
        commands,
        meshes,
        &materials.cobbles,
        cobbles,
        "The forty-eight streets of Ombreval",
    );
    spawn_batch(commands, meshes, &materials.dry_cut, cut, "The dry Cut");
}

/// The building → door-edge map baked into `navigation.json`. The renderer draws
/// each door on the same polygon edge the sim walks to, so the visible door and
/// the nav door are the same door; a building with no reachable edge has no door.
fn door_edges() -> HashMap<String, usize> {
    cathedral_sim::door_edges_from_json(include_str!("../../assets/world/navigation.json"))
        .expect("the committed navigation.json parses")
}

fn build_buildings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
    collision_world: &mut CollisionWorld,
) -> usize {
    let mut walls = BTreeMap::<WallKind, MeshData>::new();
    let mut roofs = BTreeMap::<RoofKind, MeshData>::new();
    let mut windows = MeshData::default();
    let mut doors = MeshData::default();
    let mut frames = MeshData::default();
    let mut timber_frames = MeshData::default();
    let mut rendered = 0;

    for building in &plan.buildings {
        // The fixed Lanthorn shell is built by `scene.rs`, with the authored
        // interior, openings, towers, dome, and real collision.  Its cadastral
        // polygon is still validated and counted, but must not be filled solid.
        if building.id == "named_lanthorn" {
            continue;
        }

        let (base_y, eave_y) = building_verticals(building);
        let dominant_wall = wall_kind(&building.material);
        let tint = building_tint(building);
        let openings = plan_facade_openings(
            building,
            door_edges.get(&building.id).copied(),
            base_y,
            eave_y,
        );
        let bands = jetty_bands(building, base_y, eave_y);
        let roof_polygon: Vec<[f32; 2]> = match &bands {
            Some(bands) => {
                add_jettied_walls(
                    &mut walls,
                    &mut frames,
                    &mut timber_frames,
                    stable_hash(&building.id),
                    bands,
                    tint,
                    base_y,
                    &openings,
                );
                bands
                    .last()
                    .expect("jetty_bands never returns an empty stack")
                    .polygon
                    .clone()
            }
            None => {
                add_building_walls(
                    &mut walls,
                    &mut timber_frames,
                    building,
                    dominant_wall,
                    base_y,
                    eave_y,
                    tint,
                    &openings,
                );
                building.polygon.clone()
            }
        };

        // The Bellstand tower ends in an authored open belfry, not a gable.
        let roof_height = if building.id == "named_bellstand_tower" {
            0.0
        } else {
            let roof_kind = roof_kind(building);
            // Half-timber gables sit over plaster now, like their storeys.
            let gable_kind = if dominant_wall == WallKind::HalfTimber {
                WallKind::Plaster
            } else {
                dominant_wall
            };
            let roof_mesh = roofs.entry(roof_kind).or_default();
            roof_mesh.set_brush(tint);
            let gable_mesh = walls.entry(gable_kind).or_default();
            gable_mesh.set_brush(tint);
            let (roof_height, ridge) =
                add_building_roof(roof_mesh, gable_mesh, &roof_polygon, eave_y);
            roofs.entry(roof_kind).or_default().reset_brush();
            walls.entry(gable_kind).or_default().reset_brush();
            if let Some(ridge) = ridge {
                add_chimneys(
                    walls.entry(WallKind::Fieldstone).or_default(),
                    building,
                    ridge,
                );
            }
            roof_height
        };
        match &bands {
            Some(bands) => {
                for band in bands {
                    let scoped = band_openings(&openings, band);
                    add_facade_openings_on(
                        &mut windows,
                        &mut doors,
                        &mut frames,
                        &band.polygon,
                        &scoped,
                    );
                }
            }
            None => add_facade_openings_on(
                &mut windows,
                &mut doors,
                &mut frames,
                &building.polygon,
                &openings,
            ),
        }
        add_footprint_colliders(
            collision_world,
            &building.polygon,
            base_y,
            eave_y + roof_height,
        );
        rendered += 1;

        if building.named {
            let center = polygon_center(&building.polygon);
            commands.spawn((
                Name::new(format!(
                    "{} [{}]",
                    building
                        .name
                        .as_deref()
                        .expect("validated named building must have a name"),
                    building.id
                )),
                Transform::from_xyz(center.x, base_y, center.y),
                Visibility::default(),
            ));
        }
    }

    for (kind, mesh) in walls {
        let (material, name) = match kind {
            WallKind::Limestone => (&materials.limestone, "Limestone buildings"),
            WallKind::Fieldstone => (&materials.fieldstone, "Fieldstone buildings"),
            WallKind::Plaster => (&materials.plaster, "Plastered buildings"),
            WallKind::HalfTimber => (&materials.half_timber, "Half-timbered buildings"),
        };
        spawn_batch(commands, meshes, material, mesh, name);
    }
    for (kind, mesh) in roofs {
        let (material, name) = match kind {
            RoofKind::Terracotta => (&materials.terracotta, "Clay-tiled roofs"),
            RoofKind::Slate => (&materials.slate, "Slate roofs"),
            RoofKind::Thatch => (&materials.thatch, "Thatch roofs"),
        };
        spawn_batch(commands, meshes, material, mesh, name);
    }
    spawn_batch(
        commands,
        meshes,
        &materials.window,
        windows,
        "Ombreval windows",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        doors,
        "Ombreval doors and shutters",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        frames,
        "Ombreval reveals, sills and lintels",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        timber_frames,
        "Ombreval timber framing",
    );

    rendered
}

/// A stable per-building colour multiplier: small value and warm/cool swings
/// that keep 1,100 same-material façades from rendering as one wall.
fn building_tint(building: &Building) -> [f32; 3] {
    let hash = stable_hash(&building.id);
    let value = 0.86 + (hash % 61) as f32 / 60.0 * 0.20;
    let warmth = 0.965 + ((hash >> 8) % 41) as f32 / 40.0 * 0.07;
    [value * warmth, value, value / warmth]
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OpeningKind {
    Window { shutters: bool },
    Door,
}

/// One rectangular hole in one façade edge, in edge-local coordinates:
/// `along` metres from the edge's first vertex, `center_y` in world height.
#[derive(Debug, Clone, Copy)]
struct FacadeOpening {
    along: f32,
    center_y: f32,
    width: f32,
    height: f32,
    hash: u32,
    kind: OpeningKind,
}

impl FacadeOpening {
    fn min_y(&self) -> f32 {
        self.center_y - self.height * 0.5
    }

    fn max_y(&self) -> f32 {
        self.center_y + self.height * 0.5
    }
}

/// Decide every opening of a building up front, per polygon edge, so the wall
/// builder can punch the holes the modules then line. Openings are the reason
/// the walls can no longer be four blind quads.
fn plan_facade_openings(
    building: &Building,
    door_edge: Option<usize>,
    base_y: f32,
    eave_y: f32,
) -> Vec<Vec<FacadeOpening>> {
    let mut all = vec![Vec::new(); building.polygon.len()];
    if building.use_name == "bridge" || building.id == "named_malt_house" {
        return all;
    }
    let building_hash = stable_hash(&building.id);
    let shutters_allowed = matches!(
        wall_kind(&building.material),
        WallKind::Plaster | WallKind::HalfTimber
    );
    for (edge_index, (a, b)) in building
        .polygon
        .iter()
        .zip(building.polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let length = Vec2::from_array(*a).distance(Vec2::from_array(*b));
        if length < 3.2 {
            continue;
        }
        let openings = &mut all[edge_index];
        let door_here = door_edge == Some(edge_index);
        if door_here {
            openings.push(FacadeOpening {
                along: length * 0.5,
                center_y: base_y + 1.25,
                width: 1.35,
                height: 2.5,
                hash: building_hash,
                kind: OpeningKind::Door,
            });
        }

        let count = ((length - 1.0) / 4.2).floor().clamp(1.0, 4.0) as usize;
        let floors = ((eave_y - base_y) / BUILDING_FLOOR_HEIGHT)
            .floor()
            .clamp(1.0, 4.0) as usize;
        for floor in 0..floors {
            let y = base_y + 2.05 + floor as f32 * BUILDING_FLOOR_HEIGHT;
            if y + 0.75 >= eave_y {
                continue;
            }
            for index in 0..count {
                let opening_hash = building_hash
                    ^ (edge_index as u32).wrapping_mul(0x9E37_79B9)
                    ^ (floor as u32).wrapping_mul(0x85EB_CA6B)
                    ^ (index as u32).wrapping_mul(0xC2B2_AE35);
                // A skipped window here and there keeps the grid from reading
                // as a punch card; the jitter keeps storeys off the plumb line.
                if opening_hash % 9 == 0 {
                    continue;
                }
                let jitter = ((opening_hash >> 4) % 61) as f32 / 60.0 - 0.5;
                let along = (length * (index as f32 + 1.0) / (count as f32 + 1.0) + jitter * 0.7)
                    .clamp(0.9, length - 0.9);
                // Medieval ground floors are wall, not glass: smaller, higher
                // openings on the street level, generous casements above.
                let (width, height) = if floor == 0 {
                    (0.78, 1.02)
                } else {
                    (1.0, 1.35)
                };
                // Nothing may overlap the doorway.
                if door_here
                    && floor == 0
                    && (along - length * 0.5).abs() < (1.35 + width) * 0.5 + 0.3
                {
                    continue;
                }
                openings.push(FacadeOpening {
                    along,
                    center_y: y,
                    width,
                    height,
                    hash: opening_hash,
                    kind: OpeningKind::Window {
                        shutters: shutters_allowed && floor <= 1 && opening_hash % 100 < 42,
                    },
                });
            }
        }
    }
    all
}

/// One storey band of a (possibly jettied) building: its own footprint and the
/// per-edge shift that maps original-edge `along` coordinates onto it.
struct StoreyBand {
    polygon: Vec<[f32; 2]>,
    bottom: f32,
    top: f32,
    /// Outward offset from the cadastral footprint (0 on the ground floor).
    offset: f32,
    /// Per edge: how far this band's edge start slid backward, i.e. what to add
    /// to an original-polygon `along` to land on the same wall point here.
    start_extensions: Vec<f32>,
}

/// Cantilever per jetty step. Two steps on a three-storey house add up to
/// two-thirds of a metre of street closing in overhead.
const JETTY_STEP: f32 = 0.34;

/// Offset a convex polygon outward by `distance`, mitring the corners. Returns
/// the new ring and, per edge, how far its start vertex slid backward along
/// the edge direction (needed to keep openings on the same wall point).
fn offset_convex_polygon(polygon: &[[f32; 2]], distance: f32) -> Option<(Vec<[f32; 2]>, Vec<f32>)> {
    let n = polygon.len();
    let orientation = plan::signed_area(polygon).signum();
    let mut ring = Vec::with_capacity(n);
    let mut extensions = vec![0.0_f32; n];
    for i in 0..n {
        let prev = Vec2::from_array(polygon[(i + n - 1) % n]);
        let here = Vec2::from_array(polygon[i]);
        let next = Vec2::from_array(polygon[(i + 1) % n]);
        let dir_in = (here - prev).normalize_or_zero();
        let dir_out = (next - here).normalize_or_zero();
        let normal_in = Vec2::new(dir_in.y, -dir_in.x) * orientation;
        let normal_out = Vec2::new(dir_out.y, -dir_out.x) * orientation;
        let miter = (normal_in + normal_out).normalize_or_zero();
        let denominator = miter.dot(normal_out);
        if denominator < 0.4 {
            // Sharper than ~130° of turn: the miter would spike; skip jetties
            // on this footprint rather than render a blade.
            return None;
        }
        let miter_length = distance / denominator;
        ring.push((here + miter * miter_length).to_array());
        // The offset corner moves against the outgoing edge direction by the
        // projection of the miter onto it.
        extensions[i] = -(miter * miter_length).dot(dir_out);
    }
    Some((ring, extensions))
}

/// The jettied storey stack for a building, or `None` for the plain path.
/// Only ordinary convex half-timber quads of 2+ storeys cantilever.
fn jetty_bands(building: &Building, base_y: f32, eave_y: f32) -> Option<Vec<StoreyBand>> {
    if base_y > 0.1
        || building.named
        || building.polygon.len() != 4
        || !polygon_is_convex(&building.polygon)
        || !matches!(wall_kind(&building.material), WallKind::HalfTimber)
        || building.levels < 2
        || stable_hash(&building.id) % 10 >= 8
    {
        return None;
    }
    let mut bands = Vec::new();
    let mut bottom = base_y;
    let mut storey = 0;
    while bottom < eave_y - 0.05 {
        let top = (bottom + BUILDING_FLOOR_HEIGHT).min(eave_y);
        // Ground floor sits on the cadastral line; each storey above steps out
        // one jetty, capped after two steps so alleys stay passable.
        let offset = JETTY_STEP * storey.min(2) as f32;
        let (polygon, start_extensions) = if offset > 0.0 {
            offset_convex_polygon(&building.polygon, offset)?
        } else {
            (building.polygon.clone(), vec![0.0; building.polygon.len()])
        };
        bands.push(StoreyBand {
            polygon,
            bottom,
            top,
            offset,
            start_extensions,
        });
        bottom = top;
        storey += 1;
    }
    (bands.len() >= 2).then_some(bands)
}

/// Openings re-addressed onto one storey band: only the rows inside the band,
/// with `along` corrected for the band's slid edge starts.
fn band_openings(openings: &[Vec<FacadeOpening>], band: &StoreyBand) -> Vec<Vec<FacadeOpening>> {
    openings
        .iter()
        .enumerate()
        .map(|(edge, list)| {
            list.iter()
                .filter(|opening| opening.center_y > band.bottom && opening.center_y < band.top)
                .map(|opening| FacadeOpening {
                    along: opening.along + band.start_extensions[edge],
                    ..*opening
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn add_building_walls(
    walls: &mut BTreeMap<WallKind, MeshData>,
    timber: &mut MeshData,
    building: &Building,
    dominant: WallKind,
    base_y: f32,
    eave_y: f32,
    tint: [f32; 3],
    openings: &[Vec<FacadeOpening>],
) {
    let hash = stable_hash(&building.id);
    let tinted = |walls: &mut BTreeMap<WallKind, MeshData>,
                  kind: WallKind,
                  polygon: &[[f32; 2]],
                  bottom: f32,
                  top: f32,
                  edge_openings: &[Vec<FacadeOpening>]| {
        let mesh = walls.entry(kind).or_default();
        mesh.set_brush(tint);
        add_extruded_walls(mesh, polygon, bottom, top, base_y, edge_openings);
        mesh.reset_brush();
    };
    match dominant {
        WallKind::HalfTimber if base_y < 0.1 => {
            let stone_top = (base_y + 3.0).min(eave_y);
            tinted(
                walls,
                WallKind::Fieldstone,
                &building.polygon,
                base_y,
                stone_top,
                openings,
            );
            if stone_top < eave_y {
                // Plain plaster carries the storeys; the frame is geometry.
                tinted(
                    walls,
                    WallKind::Plaster,
                    &building.polygon,
                    stone_top,
                    eave_y,
                    openings,
                );
                add_timber_framing(
                    timber,
                    &building.polygon,
                    stone_top,
                    eave_y,
                    base_y,
                    openings,
                    hash,
                    tint,
                    true,
                    true,
                );
            }
        }
        WallKind::Plaster if base_y < 0.1 => {
            let plinth_top = (base_y + 0.65).min(eave_y);
            tinted(
                walls,
                WallKind::Fieldstone,
                &building.polygon,
                base_y,
                plinth_top,
                openings,
            );
            tinted(
                walls,
                WallKind::Plaster,
                &building.polygon,
                plinth_top,
                eave_y,
                openings,
            );
        }
        _ => {
            // Elevated half-timber shells (the bridge upper storeys, the
            // malt-house) frame their whole height over plaster.
            let kind = if dominant == WallKind::HalfTimber {
                WallKind::Plaster
            } else {
                dominant
            };
            tinted(walls, kind, &building.polygon, base_y, eave_y, openings);
            if dominant == WallKind::HalfTimber {
                add_timber_framing(
                    timber,
                    &building.polygon,
                    base_y,
                    eave_y,
                    base_y,
                    openings,
                    hash,
                    tint,
                    true,
                    true,
                );
            }
        }
    }
}

/// Walls for a jettied building: fieldstone ground floor on the cadastral
/// line, then half-timber storeys stepping out over the street, each step
/// closed underneath by a soffit ring and faced with a bressummer beam.
#[allow(clippy::too_many_arguments)]
fn add_jettied_walls(
    walls: &mut BTreeMap<WallKind, MeshData>,
    frames: &mut MeshData,
    timber: &mut MeshData,
    hash: u32,
    bands: &[StoreyBand],
    tint: [f32; 3],
    base_y: f32,
    openings: &[Vec<FacadeOpening>],
) {
    for (index, band) in bands.iter().enumerate() {
        let kind = if index == 0 {
            WallKind::Fieldstone
        } else {
            WallKind::Plaster
        };
        let scoped = band_openings(openings, band);
        let mesh = walls.entry(kind).or_default();
        mesh.set_brush(tint);
        add_extruded_walls(mesh, &band.polygon, band.bottom, band.top, base_y, &scoped);
        mesh.reset_brush();

        if index == 0 {
            continue;
        }
        let below = &bands[index - 1];
        let stepped = band.offset > below.offset + 0.01;
        // The frame of the storey: where the band steps out, the bressummer
        // below stands in for the bottom rail.
        add_timber_framing(
            timber,
            &band.polygon,
            band.bottom,
            band.top,
            base_y,
            &scoped,
            hash.wrapping_add(index as u32),
            tint,
            !stepped,
            index + 1 == bands.len(),
        );
        if !stepped {
            continue;
        }
        // Soffit: the visible underside of the cantilever, joist-dark.
        let mesh = walls.entry(WallKind::Plaster).or_default();
        mesh.set_brush(tint);
        let inner = &below.polygon;
        let outer = &band.polygon;
        let count = inner.len();
        for i in 0..count {
            let j = (i + 1) % count;
            let quad = [
                Vec3::new(outer[i][0], band.bottom, outer[i][1]),
                Vec3::new(outer[j][0], band.bottom, outer[j][1]),
                Vec3::new(inner[j][0], band.bottom, inner[j][1]),
                Vec3::new(inner[i][0], band.bottom, inner[i][1]),
            ];
            let first = mesh.positions.len() as u32;
            for point in quad {
                mesh.vertex_shaded(
                    point,
                    Vec3::NEG_Y,
                    Vec2::new(point.x / 7.0, point.z / 7.0),
                    0.5,
                );
            }
            mesh.indices.extend_from_slice(&[
                first,
                first + 1,
                first + 2,
                first,
                first + 2,
                first + 3,
            ]);
        }
        mesh.reset_brush();

        // Bressummer: the beam that carries the overhung wall.
        for (a, b) in outer.iter().zip(outer.iter().cycle().skip(1)) {
            let a2 = Vec2::from_array(*a);
            let b2 = Vec2::from_array(*b);
            let length = a2.distance(b2);
            if length < 0.4 {
                continue;
            }
            let center = (a2 + b2) * 0.5;
            add_oriented_box(
                frames,
                Vec3::new(center.x, band.bottom + 0.1, center.y),
                Vec3::new(length * 0.5, 0.1, 0.085),
                (b2 - a2) / length,
            );
        }
    }
}

fn building_verticals(building: &Building) -> (f32, f32) {
    if building.use_name == "bridge" {
        return (4.25, 9.0);
    }
    if building.id == "named_malt_house" {
        return (3.8, 11.2);
    }

    let eave = match building.id.as_str() {
        // The stone shaft only: the open belfry, bell, and spire that crown it
        // are authored in `build_bellstand_belfry`.
        "named_bellstand_tower" => 23.5,
        "named_old_sluice" => 12.5,
        "named_saint_marens" => 11.2,
        id if id.starts_with("gate_") => {
            if id.starts_with("gate_reed") {
                17.0
            } else {
                20.0
            }
        }
        id if id.starts_with("reserve_church_") => 10.5,
        _ => building.levels as f32 * BUILDING_FLOOR_HEIGHT + 0.45,
    };
    (0.0, eave)
}

fn wall_kind(material: &str) -> WallKind {
    match material {
        "limestone" => WallKind::Limestone,
        "fieldstone" => WallKind::Fieldstone,
        "half_timber" | "stone_timber" => WallKind::HalfTimber,
        "plaster" => WallKind::Plaster,
        other => panic!("unknown Ombreval wall material '{other}'"),
    }
}

fn roof_kind(building: &Building) -> RoofKind {
    if matches!(
        building.use_name.as_str(),
        "ecclesiastical" | "fortification" | "civic" | "guild" | "bridge"
    ) || building.material == "limestone"
    {
        RoofKind::Slate
    } else {
        let hash = stable_hash(&building.id);
        let wall_margin = building.district == "City wall"
            || building.district.contains("Reed")
            || building.district.contains("Sluice");
        if wall_margin && building.levels <= 2 && hash.is_multiple_of(11) {
            RoofKind::Thatch
        } else {
            RoofKind::Terracotta
        }
    }
}

/// Street filth climbs about a storey up a wall and then stops; below the knee
/// every façade in the references is visibly darker than above it.
fn grime_shade(y: f32, ground: f32) -> f32 {
    0.74 + 0.26 * ((y - ground) / 2.8).clamp(0.0, 1.0)
}

/// Extrude the footprint into walls, leaving genuine holes where the façade
/// plan put openings (an empty slice keeps every face blind).
fn add_extruded_walls(
    mesh: &mut MeshData,
    polygon: &[[f32; 2]],
    bottom: f32,
    top: f32,
    ground: f32,
    openings: &[Vec<FacadeOpening>],
) {
    if top <= bottom + 0.01 {
        return;
    }
    let orientation = plan::signed_area(polygon).signum();
    for (edge_index, (a, b)) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let a2 = Vec2::from_array(*a);
        let b2 = Vec2::from_array(*b);
        let edge = b2 - a2;
        let length = edge.length();
        if length < 0.01 {
            continue;
        }
        let mut normal = Vec3::new(edge.y, 0.0, -edge.x).normalize();
        if orientation < 0.0 {
            normal = -normal;
        }
        let edge_openings = openings.get(edge_index).map(Vec::as_slice).unwrap_or(&[]);
        add_wall_face_with_holes(
            mesh,
            a2,
            edge / length,
            normal,
            length,
            bottom,
            top,
            ground,
            edge_openings,
        );
    }
}

/// Emit one wall face as the rectangle complement of its openings: horizontal
/// bands where nothing opens, vertical piers between openings where they do.
/// The scanline is over the y-extents of the openings clipped to this band.
#[allow(clippy::too_many_arguments)]
fn add_wall_face_with_holes(
    mesh: &mut MeshData,
    origin: Vec2,
    direction: Vec2,
    normal: Vec3,
    length: f32,
    bottom: f32,
    top: f32,
    ground: f32,
    openings: &[FacadeOpening],
) {
    // Clip openings to this band and keep only the ones that actually cut it.
    let mut cuts: Vec<(f32, f32, f32, f32)> = openings
        .iter()
        .filter(|opening| opening.max_y() > bottom + 0.01 && opening.min_y() < top - 0.01)
        .map(|opening| {
            (
                opening.along - opening.width * 0.5,
                opening.along + opening.width * 0.5,
                opening.min_y().max(bottom),
                opening.max_y().min(top),
            )
        })
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut emit = |x0: f32, x1: f32, y0: f32, y1: f32| {
        if x1 - x0 < 0.005 || y1 - y0 < 0.005 {
            return;
        }
        let shade0 = grime_shade(y0, ground);
        let shade1 = grime_shade(y1, ground);
        let p0 = origin + direction * x0;
        let p1 = origin + direction * x1;
        let first = mesh.vertex_shaded(
            Vec3::new(p0.x, y0, p0.y),
            normal,
            Vec2::new(x0 / 7.0, y0 / 7.0),
            shade0,
        );
        mesh.vertex_shaded(
            Vec3::new(p1.x, y0, p1.y),
            normal,
            Vec2::new(x1 / 7.0, y0 / 7.0),
            shade0,
        );
        mesh.vertex_shaded(
            Vec3::new(p1.x, y1, p1.y),
            normal,
            Vec2::new(x1 / 7.0, y1 / 7.0),
            shade1,
        );
        mesh.vertex_shaded(
            Vec3::new(p0.x, y1, p0.y),
            normal,
            Vec2::new(x0 / 7.0, y1 / 7.0),
            shade1,
        );
        mesh.indices
            .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    };

    if cuts.is_empty() {
        // Blind wall: still split at the grime knee so the gradient keeps its
        // elbow instead of smearing evenly to the eaves.
        let knee = (bottom + 2.8).min(top);
        emit(0.0, length, bottom, knee);
        emit(0.0, length, knee, top);
        return;
    }

    // Scanline over the distinct y-levels the openings introduce.
    let mut levels: Vec<f32> = vec![bottom, top];
    for &(_, _, y0, y1) in &cuts {
        levels.push(y0);
        levels.push(y1);
    }
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 0.005);

    for pair in levels.windows(2) {
        let (y0, y1) = (pair[0], pair[1]);
        let mid = (y0 + y1) * 0.5;
        let mut cursor = 0.0_f32;
        for &(x0, x1, cy0, cy1) in &cuts {
            if cy0 > mid || cy1 < mid {
                continue;
            }
            emit(cursor, x0, y0, y1);
            cursor = cursor.max(x1);
        }
        emit(cursor, length, y0, y1);
    }
}

/// How far each roof plane continues past the wall face, following its own
/// pitch. The shadow line this casts under the eaves is one of the strongest
/// "real building" cues there is.
const EAVES_OVERHANG: f32 = 0.55;
/// The verge: how far the roof oversails the gable ends along the ridge.
const VERGE_OVERHANG: f32 = 0.32;

fn add_building_roof(
    roof: &mut MeshData,
    gable_wall: &mut MeshData,
    polygon: &[[f32; 2]],
    eave_y: f32,
) -> (f32, Option<[Vec3; 2]>) {
    if polygon.len() != 4 {
        add_polygon_surface(roof, polygon, eave_y + 0.08, 7.0);
        return (0.16, None);
    }

    let p = [
        Vec2::from_array(polygon[0]),
        Vec2::from_array(polygon[1]),
        Vec2::from_array(polygon[2]),
        Vec2::from_array(polygon[3]),
    ];
    let edge_01 = p[0].distance(p[1]);
    let edge_12 = p[1].distance(p[2]);
    let roof_height = edge_01.min(edge_12).mul_add(0.32, 0.65).clamp(1.25, 4.2);
    let y_ridge = eave_y + roof_height;

    // The ridge spans the midpoints of the two short edges; each plane's eave
    // pair is listed (near, far) relative to the `ridge_a` end, and the gables
    // fill the short walls up to the ridge.
    let (ridge_a, ridge_b, eave_pairs, gables) = if edge_01 <= edge_12 {
        let a = (p[0] + p[1]) * 0.5;
        let b = (p[2] + p[3]) * 0.5;
        (
            a,
            b,
            [(p[0], p[3]), (p[1], p[2])],
            [(p[0], p[1], a), (p[3], p[2], b)],
        )
    } else {
        let a = (p[1] + p[2]) * 0.5;
        let b = (p[3] + p[0]) * 0.5;
        (
            a,
            b,
            [(p[1], p[0]), (p[2], p[3])],
            [(p[1], p[2], a), (p[0], p[3], b)],
        )
    };

    let ridge_dir = (ridge_b - ridge_a).normalize_or_zero();
    let mid_ridge = (ridge_a + ridge_b) * 0.5;
    for (near, far) in eave_pairs {
        // Push the eave edge out along the plane's own pitch so the overhang
        // droops instead of floating flat, and oversail both gable ends.
        let out = (near + far) * 0.5 - mid_ridge;
        let half_span = out.length().max(0.05);
        let out = out / half_span;
        let drop = EAVES_OVERHANG * roof_height / half_span;
        let eave_low = eave_y - drop;
        let e_near = near + out * EAVES_OVERHANG - ridge_dir * VERGE_OVERHANG;
        let e_far = far + out * EAVES_OVERHANG + ridge_dir * VERGE_OVERHANG;
        let r_near = ridge_a - ridge_dir * VERGE_OVERHANG;
        let r_far = ridge_b + ridge_dir * VERGE_OVERHANG;
        let points = [
            Vec3::new(e_near.x, eave_low, e_near.y),
            Vec3::new(e_far.x, eave_low, e_far.y),
            Vec3::new(r_far.x, y_ridge, r_far.y),
            Vec3::new(r_near.x, y_ridge, r_near.y),
        ];
        let mut normal = (points[1] - points[0])
            .cross(points[3] - points[0])
            .normalize_or(Vec3::Y);
        if normal.y < 0.0 {
            normal = -normal;
        }
        // Tile courses run with the pitch: u along the eave, v up the slope —
        // the old top-down planar map stretched tiles on every steep roof.
        let eave_dir = (e_far - e_near).normalize_or_zero();
        let slope_len =
            ((half_span + EAVES_OVERHANG).powi(2) + (roof_height + drop).powi(2)).sqrt();
        let u = |point: Vec2| (point - e_near).dot(eave_dir) / 7.0;
        roof.quad(
            points,
            normal,
            [
                Vec2::new(u(e_near), 0.0),
                Vec2::new(u(e_far), 0.0),
                Vec2::new(u(r_far), slope_len / 7.0),
                Vec2::new(u(r_near), slope_len / 7.0),
            ],
        );
    }

    // A half-round ridge cap: two pitched strips meeting a touch above the
    // ridge line, sailing the same verge as the planes.
    let cap_a = ridge_a - ridge_dir * VERGE_OVERHANG;
    let cap_b = ridge_b + ridge_dir * VERGE_OVERHANG;
    let cap_side = Vec2::new(ridge_dir.y, -ridge_dir.x);
    for side in [-1.0, 1.0] {
        let skirt = cap_side * side * 0.20;
        let points = [
            Vec3::new(cap_a.x, y_ridge + 0.09, cap_a.y),
            Vec3::new(cap_b.x, y_ridge + 0.09, cap_b.y),
            Vec3::new(cap_b.x + skirt.x, y_ridge - 0.05, cap_b.y + skirt.y),
            Vec3::new(cap_a.x + skirt.x, y_ridge - 0.05, cap_a.y + skirt.y),
        ];
        let mut normal = (points[1] - points[0])
            .cross(points[3] - points[0])
            .normalize_or(Vec3::Y);
        if normal.y < 0.0 {
            normal = -normal;
        }
        roof.quad(
            points,
            normal,
            [
                Vec2::ZERO,
                Vec2::new(cap_a.distance(cap_b) / 7.0, 0.0),
                Vec2::new(cap_a.distance(cap_b) / 7.0, 0.05),
                Vec2::new(0.0, 0.05),
            ],
        );
    }

    for (a, b, ridge) in gables {
        gable_wall.triangle(
            [
                Vec3::new(a.x, eave_y, a.y),
                Vec3::new(b.x, eave_y, b.y),
                Vec3::new(ridge.x, y_ridge, ridge.y),
            ],
            [Vec2::ZERO, Vec2::X, Vec2::new(0.5, roof_height / 4.0)],
            false,
        );
    }

    debug_assert!(ridge_a.distance(ridge_b) > 0.1);
    (
        roof_height,
        Some([
            Vec3::new(ridge_a.x, y_ridge, ridge_a.y),
            Vec3::new(ridge_b.x, y_ridge, ridge_b.y),
        ]),
    )
}

/// Chimneys are what a skyline is made of. One or two fieldstone stacks per
/// gabled building, planted on the ridge at a stable per-building spot.
fn add_chimneys(mesh: &mut MeshData, building: &Building, ridge: [Vec3; 2]) {
    if building.use_name == "bridge" {
        return;
    }
    let [ridge_a, ridge_b] = ridge;
    let ridge_len = ridge_a.distance(ridge_b);
    if ridge_len < 2.5 {
        return;
    }
    let hash = stable_hash(&building.id);
    let along = Vec2::new(ridge_b.x - ridge_a.x, ridge_b.z - ridge_a.z) / ridge_len;
    let count = if ridge_len > 15.0 && hash % 3 == 0 {
        2
    } else {
        1
    };
    for index in 0..count {
        let t = if count == 2 {
            0.26 + 0.48 * index as f32
        } else {
            0.28 + (hash % 45) as f32 / 100.0
        };
        let base = ridge_a.lerp(ridge_b, t);
        // Stack sunk into the ridge, flaring into a cap slab above.
        add_oriented_box(
            mesh,
            base + Vec3::Y * 0.25,
            Vec3::new(0.36, 1.0, 0.36),
            along,
        );
        add_oriented_box(
            mesh,
            base + Vec3::Y * 1.3,
            Vec3::new(0.48, 0.08, 0.48),
            along,
        );
    }
}

/// An axis-defined box written straight into a batched mesh: `along` is the
/// local +X direction in the ground plane, `half` the half-extents.
fn add_oriented_box(mesh: &mut MeshData, center: Vec3, half: Vec3, along: Vec2) {
    let ax = Vec3::new(along.x, 0.0, along.y);
    let az = Vec3::new(-along.y, 0.0, along.x);
    let ay = Vec3::Y;
    for (normal, right, up, half_n, half_r, half_u) in [
        (ax, az, ay, half.x, half.z, half.y),
        (-ax, -az, ay, half.x, half.z, half.y),
        (az, -ax, ay, half.z, half.x, half.y),
        (-az, ax, ay, half.z, half.x, half.y),
        (ay, ax, az, half.y, half.x, half.z),
        (-ay, -ax, az, half.y, half.x, half.z),
    ] {
        let face_center = center + normal * half_n;
        let points = [
            face_center - right * half_r - up * half_u,
            face_center + right * half_r - up * half_u,
            face_center + right * half_r + up * half_u,
            face_center - right * half_r + up * half_u,
        ];
        mesh.quad(
            points,
            normal,
            [
                Vec2::ZERO,
                Vec2::new(half_r / 3.5, 0.0),
                Vec2::new(half_r / 3.5, half_u / 3.5),
                Vec2::new(0.0, half_u / 3.5),
            ],
        );
    }
}

/// One timber member on a wall face, in edge-local coordinates: from
/// `(along, y)` `a` to `(along, y)` `b`, `half_width` across the member in the
/// face plane, standing `proud` of the wall with its back buried. Front and
/// side faces only unless `ends` — the buried back face is never emitted.
#[allow(clippy::too_many_arguments)]
fn add_face_member(
    mesh: &mut MeshData,
    origin: Vec2,
    direction: Vec2,
    normal2: Vec2,
    a: Vec2,
    b: Vec2,
    half_width: f32,
    proud: f32,
    ends: bool,
) {
    let to_world = |p: Vec2| {
        let flat = origin + direction * p.x;
        Vec3::new(flat.x, p.y, flat.y)
    };
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    let axis = (to_world(b) - to_world(a)).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let across = normal.cross(axis).normalize_or(Vec3::Y);
    // The back sits a touch inside the wall so no seam ever opens.
    let depth_half = (proud + 0.04) * 0.5;
    let shift = normal * (proud - depth_half);
    let start = to_world(a) + shift;
    let end = to_world(b) + shift;
    let face = |mesh: &mut MeshData, points: [Vec3; 4], face_normal: Vec3| {
        let u = points[0].distance(points[1]) / 3.5;
        let v = points[1].distance(points[2]) / 3.5;
        mesh.quad(
            points,
            face_normal,
            [
                Vec2::ZERO,
                Vec2::new(u, 0.0),
                Vec2::new(u, v),
                Vec2::new(0.0, v),
            ],
        );
    };
    let w = across * half_width;
    let d = normal * depth_half;
    face(
        mesh,
        [start - w + d, end - w + d, end + w + d, start + w + d],
        normal,
    );
    face(
        mesh,
        [start - w - d, end - w - d, end - w + d, start - w + d],
        -across,
    );
    face(
        mesh,
        [start + w + d, end + w + d, end + w - d, start + w - d],
        across,
    );
    if ends {
        face(
            mesh,
            [start - w - d, start - w + d, start + w + d, start + w - d],
            -axis,
        );
        face(
            mesh,
            [end - w - d, end + w - d, end + w + d, end - w + d],
            axis,
        );
    }
}

/// The structural skeleton of a half-timber storey band, drawn as real
/// geometry instead of a painted grid: corner posts, rails on the storey
/// lines, hash-jittered studs that step around the openings, and a diagonal
/// brace where a corner leaves room for one.
#[allow(clippy::too_many_arguments)]
fn add_timber_framing(
    timber: &mut MeshData,
    polygon: &[[f32; 2]],
    bottom: f32,
    top: f32,
    ground: f32,
    openings: &[Vec<FacadeOpening>],
    hash: u32,
    tint: [f32; 3],
    bottom_rail: bool,
    top_rail: bool,
) {
    let height = top - bottom;
    if height < 0.4 {
        return;
    }
    timber.set_brush(tint);
    let orientation = plan::signed_area(polygon).signum();
    for (edge_index, (a, b)) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let a2 = Vec2::from_array(*a);
        let b2 = Vec2::from_array(*b);
        let edge = b2 - a2;
        let length = edge.length();
        if length < 0.6 {
            continue;
        }
        let direction = edge / length;
        let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
        if orientation < 0.0 {
            normal2 = -normal2;
        }
        let edge_hash = hash ^ (edge_index as u32).wrapping_mul(0x9E37_79B9);

        // Corner post on the shared vertex, proud of both meeting faces.
        add_oriented_box(
            timber,
            Vec3::new(a2.x, (bottom + top) * 0.5, a2.y),
            Vec3::new(0.11, height * 0.5, 0.11),
            direction,
        );

        // The openings that actually pierce this band on this edge.
        let cuts: Vec<&FacadeOpening> = openings
            .get(edge_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter(|opening| opening.max_y() > bottom + 0.05 && opening.min_y() < top - 0.05)
            .collect();

        // Rails: the sill beam over the masonry, one on every storey line the
        // band crosses, and the wall plate under the eaves. The storey lines
        // thread between the window rows by construction, so rails never cross
        // glass.
        let mut rail_ys = Vec::new();
        if bottom_rail {
            rail_ys.push(bottom + 0.09);
        }
        let mut line = (((bottom - ground) / BUILDING_FLOOR_HEIGHT).floor() + 1.0)
            * BUILDING_FLOOR_HEIGHT
            + ground;
        while line < top - 0.45 {
            if line > bottom + 0.45 {
                rail_ys.push(line);
            }
            line += BUILDING_FLOOR_HEIGHT;
        }
        if top_rail {
            rail_ys.push(top - 0.10);
        }
        for y in rail_ys {
            add_face_member(
                timber,
                a2,
                direction,
                normal2,
                Vec2::new(0.10, y),
                Vec2::new(length - 0.10, y),
                0.075,
                0.13,
                false,
            );
        }

        if height < 1.1 {
            continue;
        }

        // Studs: jittered 1.2–1.8 m spacing, skipped where a window interrupts.
        let mut along = 0.85 + (edge_hash % 37) as f32 / 60.0;
        let mut stud_index = 0u32;
        while along < length - 0.7 {
            let clear = cuts
                .iter()
                .all(|opening| (along - opening.along).abs() > opening.width * 0.5 + 0.17);
            if clear {
                add_face_member(
                    timber,
                    a2,
                    direction,
                    normal2,
                    Vec2::new(along, bottom + 0.06),
                    Vec2::new(along, top - 0.06),
                    0.055,
                    0.12,
                    false,
                );
            }
            let step_hash = edge_hash ^ stud_index.wrapping_mul(0x85EB_CA6B);
            along += 1.2 + (step_hash % 61) as f32 / 100.0;
            stud_index += 1;
        }

        // A diagonal brace off a corner post — the classic K/Z patterns, two
        // orientations picked by hash, only where no opening blocks the run.
        let rise = (height - 0.3).min(2.35);
        let run = rise.mul_add(0.55, 0.35);
        let variants = [
            (0.16, bottom + 0.12, 0.16 + run, bottom + 0.12 + rise),
            (
                length - 0.16,
                bottom + 0.12,
                length - 0.16 - run,
                bottom + 0.12 + rise,
            ),
        ];
        let pick = (edge_hash >> 6) % 3;
        for (variant, (x0, y0, x1, y1)) in variants.into_iter().enumerate() {
            if pick != 2 && pick != variant as u32 {
                continue;
            }
            let (lo, hi) = (x0.min(x1), x0.max(x1));
            let clear = lo > 0.1
                && hi < length - 0.1
                && cuts.iter().all(|opening| {
                    opening.along + opening.width * 0.5 + 0.15 < lo
                        || opening.along - opening.width * 0.5 - 0.15 > hi
                });
            if clear {
                add_face_member(
                    timber,
                    a2,
                    direction,
                    normal2,
                    Vec2::new(x0, y0),
                    Vec2::new(x1, y1),
                    0.065,
                    0.115,
                    true,
                );
            }
        }
    }
    timber.reset_brush();
}

/// How deep every opening sits behind the wall face. The reveal this exposes
/// is the difference between a window and a sticker.
const OPENING_DEPTH: f32 = 0.15;

/// Line the holes the wall builder left: glass, reveals, sills, lintels,
/// shutters and door leaves. Purely decorative — the openings themselves were
/// cut by `add_wall_face_with_holes`. Works on whichever footprint the walls
/// actually used (a jettied storey's ring, or the cadastral polygon).
fn add_facade_openings_on(
    windows: &mut MeshData,
    doors: &mut MeshData,
    frames: &mut MeshData,
    polygon: &[[f32; 2]],
    openings: &[Vec<FacadeOpening>],
) {
    let orientation = plan::signed_area(polygon).signum();
    for (edge_index, (a, b)) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let edge_openings = match openings.get(edge_index) {
            Some(list) if !list.is_empty() => list,
            _ => continue,
        };
        let a = Vec2::from_array(*a);
        let b = Vec2::from_array(*b);
        let edge = b - a;
        let length = edge.length();
        if length < 0.01 {
            continue;
        }
        let direction = edge / length;
        let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
        if orientation < 0.0 {
            normal2 = -normal2;
        }
        for opening in edge_openings {
            let wall_point = a + direction * opening.along;
            match opening.kind {
                OpeningKind::Window { shutters } => add_window_module(
                    windows,
                    doors,
                    frames,
                    wall_point,
                    opening.center_y,
                    direction,
                    normal2,
                    opening.width,
                    opening.height,
                    opening.hash,
                    shutters,
                ),
                OpeningKind::Door => add_door_module(
                    doors,
                    frames,
                    wall_point,
                    opening.min_y(),
                    direction,
                    normal2,
                ),
            }
        }
    }
}

/// One real window: glass sunk behind the wall face, reveal returns, a
/// projecting sill and lintel, a mullion cross, and sometimes open shutters.
#[allow(clippy::too_many_arguments)]
fn add_window_module(
    windows: &mut MeshData,
    doors: &mut MeshData,
    frames: &mut MeshData,
    wall_point: Vec2,
    center_y: f32,
    direction: Vec2,
    normal2: Vec2,
    width: f32,
    height: f32,
    hash: u32,
    shutters_allowed: bool,
) {
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    // The glass slightly overlaps the hole so no slit into the hollow shell
    // survives at the reveal borders.
    let glass_center = wall_point - normal2 * OPENING_DEPTH;
    add_facade_panel(
        windows,
        glass_center,
        center_y,
        direction,
        normal,
        width + 0.06,
        height + 0.06,
    );
    add_reveal(
        frames,
        wall_point,
        center_y,
        direction,
        normal2,
        width,
        height,
        OPENING_DEPTH,
        false,
    );

    // Mullion cross on the glass plane.
    let mullion_center = wall_point - normal2 * (OPENING_DEPTH - 0.03);
    add_facade_panel(
        frames,
        mullion_center,
        center_y,
        direction,
        normal,
        0.055,
        height - 0.08,
    );
    add_facade_panel(
        frames,
        mullion_center,
        center_y,
        direction,
        normal,
        width - 0.08,
        0.055,
    );

    // Sill: a slab proud of the wall below the opening; lintel above.
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, center_y - height * 0.5 - 0.04, wall_point.y) + normal * 0.05,
        Vec3::new(width * 0.5 + 0.08, 0.045, 0.09),
        direction,
    );
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, center_y + height * 0.5 + 0.05, wall_point.y) + normal * 0.03,
        Vec3::new(width * 0.5 + 0.06, 0.055, 0.07),
        direction,
    );

    // Shutters folded back against the wall — one leaf or both.
    if shutters_allowed {
        let leaf_width = width * 0.52;
        let sides: &[f32] = if hash % 5 < 3 { &[-1.0, 1.0] } else { &[1.0] };
        for side in sides {
            let leaf_center =
                wall_point + direction * side * (width * 0.5 + leaf_width * 0.5 + 0.04);
            add_facade_panel(
                doors,
                leaf_center + normal2 * 0.045,
                center_y,
                direction,
                normal,
                leaf_width,
                height - 0.04,
            );
        }
    }
}

/// The door: leaf recessed behind the face, reveal returns, a proud lintel and
/// a worn threshold slab at the foot.
fn add_door_module(
    doors: &mut MeshData,
    frames: &mut MeshData,
    wall_point: Vec2,
    base_y: f32,
    direction: Vec2,
    normal2: Vec2,
) {
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    let width = 1.35;
    let height = 2.5;
    let center_y = base_y + height * 0.5;
    add_facade_panel(
        doors,
        wall_point - normal2 * (OPENING_DEPTH - 0.03),
        center_y,
        direction,
        normal,
        width + 0.06,
        height + 0.04,
    );
    add_reveal(
        frames,
        wall_point,
        center_y,
        direction,
        normal2,
        width,
        height,
        OPENING_DEPTH - 0.03,
        true,
    );
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, base_y + height + 0.07, wall_point.y) + normal * 0.04,
        Vec3::new(width * 0.5 + 0.12, 0.07, 0.09),
        direction,
    );
    // Threshold: a step slab proud of the wall at ground level.
    add_oriented_box(
        frames,
        Vec3::new(wall_point.x, base_y + 0.045, wall_point.y) + normal * 0.14,
        Vec3::new(width * 0.5 + 0.05, 0.05, 0.24),
        direction,
    );
}

/// The four (or three, for a door) return faces connecting the wall plane to a
/// recessed opening. This is what makes an opening read as a hole.
#[allow(clippy::too_many_arguments)]
fn add_reveal(
    frames: &mut MeshData,
    wall_point: Vec2,
    center_y: f32,
    direction: Vec2,
    normal2: Vec2,
    width: f32,
    height: f32,
    depth: f32,
    skip_bottom: bool,
) {
    let normal = Vec3::new(normal2.x, 0.0, normal2.y);
    let along = Vec3::new(direction.x, 0.0, direction.y);
    let center = Vec3::new(wall_point.x, center_y, wall_point.y);
    let half_w = along * (width * 0.5);
    let half_h = Vec3::Y * (height * 0.5);
    let inward = -normal * depth;

    // Side returns face each other across the opening.
    for side in [-1.0, 1.0] {
        let outer_top = center + half_w * side + half_h;
        let outer_bottom = center + half_w * side - half_h;
        frames.quad(
            [
                outer_bottom,
                outer_top,
                outer_top + inward,
                outer_bottom + inward,
            ],
            along * -side,
            [
                Vec2::ZERO,
                Vec2::new(height / 7.0, 0.0),
                Vec2::new(height / 7.0, depth / 7.0),
                Vec2::new(0.0, depth / 7.0),
            ],
        );
    }
    // Head return faces down; sill return faces up.
    let head_a = center - half_w + half_h;
    let head_b = center + half_w + half_h;
    frames.quad(
        [head_a, head_b, head_b + inward, head_a + inward],
        Vec3::NEG_Y,
        [
            Vec2::ZERO,
            Vec2::new(width / 7.0, 0.0),
            Vec2::new(width / 7.0, depth / 7.0),
            Vec2::new(0.0, depth / 7.0),
        ],
    );
    if !skip_bottom {
        let foot_a = center - half_w - half_h;
        let foot_b = center + half_w - half_h;
        frames.quad(
            [foot_a, foot_b, foot_b + inward, foot_a + inward],
            Vec3::Y,
            [
                Vec2::ZERO,
                Vec2::new(width / 7.0, 0.0),
                Vec2::new(width / 7.0, depth / 7.0),
                Vec2::new(0.0, depth / 7.0),
            ],
        );
    }
}

fn add_facade_panel(
    mesh: &mut MeshData,
    center: Vec2,
    center_y: f32,
    along: Vec2,
    normal: Vec3,
    width: f32,
    height: f32,
) {
    let horizontal = Vec3::new(along.x, 0.0, along.y) * (width * 0.5);
    let vertical = Vec3::Y * (height * 0.5);
    let center = Vec3::new(center.x, center_y, center.y);
    mesh.quad(
        [
            center - horizontal - vertical,
            center + horizontal - vertical,
            center + horizontal + vertical,
            center - horizontal + vertical,
        ],
        normal,
        [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
    );
}

/// Register the visible footprint itself as collision geometry.
///
/// Almost every cadastral building is a convex quadrilateral. The two concave
/// non-cathedral footprints are decomposed into triangles so their passages
/// remain open instead of being filled by an oversized convex hull.
fn add_footprint_colliders(
    collision_world: &mut CollisionWorld,
    polygon: &[[f32; 2]],
    min_y: f32,
    max_y: f32,
) {
    if polygon_is_convex(polygon) {
        collision_world.add_convex_prism(polygon, min_y, max_y);
        return;
    }

    for triangle in triangulate_polygon(polygon) {
        collision_world.add_convex_prism(&triangle.map(|vertex| polygon[vertex]), min_y, max_y);
    }
}

fn build_fixtures(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    fixtures: &[Fixture],
    collision_world: &mut CollisionWorld,
) {
    for (index, fixture) in fixtures.iter().enumerate() {
        if fixture.kind == "statue" {
            continue;
        }
        let position = Vec3::new(fixture.position[0], 0.0, fixture.position[1]);
        let angle = fixture.angle_deg.to_radians();
        let name = fixture
            .label
            .clone()
            .unwrap_or_else(|| format!("{} [{}]", fixture.kind, fixture.id));
        match fixture.kind.as_str() {
            "stall" => spawn_market_stall(
                commands,
                meshes,
                materials,
                collision_world,
                position,
                Vec2::from_array(fixture.size),
                angle,
                index,
                name,
            ),
            "stone_stack" => {
                spawn_rotated_box_named(
                    commands,
                    meshes,
                    &materials.limestone,
                    position + Vec3::Y * 0.65,
                    Vec3::new(fixture.size[0], 1.3, fixture.size[1]),
                    angle,
                    name,
                );
                add_rotated_box_collider(
                    collision_world,
                    position,
                    Vec2::from_array(fixture.size),
                    angle,
                    1.3,
                );
            }
            "smoke_rack" => spawn_smoke_rack(commands, meshes, materials, position, angle, &name),
            "well" | "chain_well" | "three_curb_well" | "lodge_well" | "cistern"
            | "step_cistern" | "fire_tanks" => water::spawn_water_fixture(
                commands,
                meshes,
                materials,
                collision_world,
                &fixture.id,
                &fixture.kind,
                position,
                Vec2::from_array(fixture.size),
                angle,
            ),
            "stone" => {
                spawn_mesh_named(
                    commands,
                    &meshes.sphere,
                    &materials.fieldstone,
                    Transform::from_translation(position + Vec3::Y * 0.55).with_scale(Vec3::new(
                        fixture.size[0] * 0.38,
                        0.55,
                        fixture.size[1] * 0.38,
                    )),
                    name,
                );
                add_rotated_box_collider(
                    collision_world,
                    position,
                    Vec2::new(fixture.size[0] * 0.65, fixture.size[1] * 0.65),
                    0.0,
                    1.2,
                );
            }
            "platform" => {
                spawn_rotated_box_named(
                    commands,
                    meshes,
                    &materials.limestone,
                    position + Vec3::Y * 0.4,
                    Vec3::new(fixture.size[0], 0.8, fixture.size[1]),
                    angle,
                    name,
                );
                add_rotated_box_collider(
                    collision_world,
                    position,
                    Vec2::from_array(fixture.size),
                    angle,
                    0.8,
                );
            }
            "weighbeam" => spawn_weighbeam(commands, meshes, materials, position, angle, &name),
            "tracing" => spawn_rotated_box_named(
                commands,
                meshes,
                &materials.paving,
                position + Vec3::Y * 0.055,
                Vec3::new(fixture.size[0], 0.1, fixture.size[1]),
                angle,
                name,
            ),
            "crane" => spawn_yard_crane(commands, meshes, materials, position, angle, &name),
            other => warn!("unrendered Ombreval fixture kind: {other}"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_market_stall(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    position: Vec3,
    size: Vec2,
    angle: f32,
    variant: usize,
    name: String,
) {
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.timber,
        position + Vec3::Y * 0.65,
        Vec3::new(size.x, 1.3, size.y),
        angle,
        name,
    );
    let canopy = if variant.is_multiple_of(2) {
        &materials.cloth_ochre
    } else {
        &materials.cloth_russet
    };
    spawn_rotated_box_named(
        commands,
        meshes,
        canopy,
        position + Vec3::Y * 2.45,
        Vec3::new(size.x + 0.55, 0.18, size.y + 0.55),
        angle,
        "Stall awning",
    );
    add_rotated_box_collider(collision_world, position, size, angle, 1.35);
}

fn spawn_smoke_rack(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
    name: &str,
) {
    let right = Quat::from_rotation_y(angle) * Vec3::X;
    for side in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.dark_wood,
            position + right * side * 2.0 + Vec3::Y * 1.5,
            Vec3::new(0.18, 3.0, 0.18),
            name,
        );
    }
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        position + Vec3::Y * 2.85,
        Vec3::new(4.4, 0.18, 0.18),
        angle,
        "Smoke rack beam",
    );
}

fn spawn_weighbeam(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
    name: &str,
) {
    let right = Quat::from_rotation_y(angle) * Vec3::X;
    for side in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            position + right * side * 5.8 + Vec3::Y * 2.6,
            Vec3::new(0.45, 5.2, 0.45),
            name,
        );
    }
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        position + Vec3::Y * 5.0,
        Vec3::new(14.0, 0.5, 0.5),
        angle,
        "Tallage weighing beam",
    );
}

fn spawn_yard_crane(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
    name: &str,
) {
    spawn_box_named(
        commands,
        meshes,
        &materials.timber,
        position + Vec3::Y * 4.5,
        Vec3::new(0.75, 9.0, 0.75),
        name,
    );
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.timber,
        position + Vec3::Y * 8.0,
        Vec3::new(8.0, 0.55, 0.55),
        angle - 0.2,
        "Yard crane arm",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        position + Quat::from_rotation_y(angle - 0.2) * Vec3::X * 3.7 + Vec3::Y * 6.3,
        Vec3::new(0.08, 3.4, 0.08),
        "Yard crane chain",
    );
}

fn build_named_details(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    build_bellstand_stair(commands, meshes, materials, collision_world);
    build_bellstand_belfry(commands, meshes, materials, collision_world);
    build_saint_maren_tower(commands, meshes, materials, collision_world);
    build_parish_towers(commands, meshes, materials, plan, collision_world);
    build_old_sluice_face(commands, meshes, materials);
    build_charnel_and_ilvane_details(commands, meshes, materials);
    build_bridge_supports(commands, meshes, materials, plan, collision_world);
    build_ropewalk(commands, meshes, materials);
    build_osanne_stall(commands, meshes, materials, collision_world);
    build_wharf_cranes(commands, meshes, materials);
}

/// A cast bronze bell assembled from primitives, mouth down, hung from a
/// headstock beam. `scale` 1.0 is the great Bellstand bell (~2.6 m mouth).
fn spawn_bell(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    center: Vec3,
    scale: f32,
    name: &str,
) {
    // Headstock the bell swings from.
    spawn_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        center + Vec3::Y * 2.6 * scale,
        Vec3::new(5.2 * scale, 0.5 * scale, 0.55 * scale),
        format!("{name} headstock"),
    );
    // Crown, shoulder, waist, flare, lip — the classic profile, coarsely.
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.bronze,
        Transform::from_translation(center + Vec3::Y * 2.15 * scale)
            .with_scale(Vec3::splat(0.5 * scale)),
        format!("{name} crown"),
    );
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.bronze,
        Transform::from_translation(center + Vec3::Y * 1.55 * scale).with_scale(Vec3::new(
            1.02 * scale,
            0.85 * scale,
            1.02 * scale,
        )),
        format!("{name} shoulder"),
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        center + Vec3::Y * 0.85 * scale,
        0.95 * scale,
        1.5 * scale,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        center + Vec3::Y * 0.18 * scale,
        1.22 * scale,
        0.45 * scale,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        center + Vec3::Y * 0.02 * scale,
        1.3 * scale,
        0.16 * scale,
    );
    // Clapper, just visible under the lip.
    spawn_cylinder(
        commands,
        meshes,
        &materials.iron,
        center + Vec3::Y * 0.55 * scale,
        0.09 * scale,
        1.4 * scale,
    );
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.iron,
        Transform::from_translation(center - Vec3::Y * 0.18 * scale)
            .with_scale(Vec3::splat(0.22 * scale)),
        format!("{name} clapper"),
    );
}

/// The open bell stage crowning the Bellstand tower: piers, parapet,
/// entablature, corner pinnacles, the great bell, and a slate spire — the
/// silhouette `the_bellstand_001.png` promises. The stage floor is walkable.
fn build_bellstand_belfry(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center = Vec2::new(64.0, -270.0);
    let (half_x, half_z) = (11.0, 12.5);
    let floor_y = 23.5;
    let stage_top = 31.2;

    // Stage floor caps the shaft; you can land and stand on it.
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new(center.x, floor_y + 0.25, center.y),
        Vec3::new(half_x * 2.0, 0.5, half_z * 2.0),
        "Bellstand stage floor",
    );
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y - half_z),
        Vec3::new(center.x + half_x, floor_y + 0.5, center.y + half_z),
    );

    // Corner and mid-face piers carry the entablature.
    let pier_height = stage_top - floor_y;
    let pier_y = floor_y + pier_height * 0.5;
    let corner_inset = 1.1;
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            let position = Vec3::new(
                center.x + sx * (half_x - corner_inset),
                pier_y,
                center.y + sz * (half_z - corner_inset),
            );
            spawn_box_named(
                commands,
                meshes,
                &materials.limestone,
                position,
                Vec3::new(1.8, pier_height, 1.8),
                "Bellstand corner pier",
            );
            collision_world.add_box(
                position - Vec3::new(0.9, pier_height * 0.5, 0.9),
                position + Vec3::new(0.9, pier_height * 0.5, 0.9),
            );
        }
    }
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x, pier_y, center.y + sz * (half_z - 0.8)),
            Vec3::new(1.5, pier_height, 1.6),
            "Bellstand mid pier",
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x + sx * (half_x - 0.8), pier_y, center.y),
            Vec3::new(1.6, pier_height, 1.5),
            "Bellstand mid pier",
        );
    }

    // Waist-high parapet between the piers.
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x, floor_y + 1.0, center.y + sz * (half_z - 0.55)),
            Vec3::new(half_x * 2.0 - 1.4, 1.1, 0.5),
            "Bellstand parapet",
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x + sx * (half_x - 0.55), floor_y + 1.0, center.y),
            Vec3::new(0.5, 1.1, half_z * 2.0 - 1.4),
            "Bellstand parapet",
        );
    }
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y - half_z),
        Vec3::new(center.x + half_x, floor_y + 1.55, center.y - half_z + 0.6),
    );
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y + half_z - 0.6),
        Vec3::new(center.x + half_x, floor_y + 1.55, center.y + half_z),
    );
    collision_world.add_box(
        Vec3::new(center.x - half_x, floor_y, center.y - half_z),
        Vec3::new(center.x - half_x + 0.6, floor_y + 1.55, center.y + half_z),
    );
    collision_world.add_box(
        Vec3::new(center.x + half_x - 0.6, floor_y, center.y - half_z),
        Vec3::new(center.x + half_x, floor_y + 1.55, center.y + half_z),
    );

    // Entablature ring above the arcade.
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x, stage_top + 0.5, center.y + sz * (half_z - 0.7)),
            Vec3::new(half_x * 2.0 + 0.7, 1.0, 1.7),
            "Bellstand entablature",
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(center.x + sx * (half_x - 0.7), stage_top + 0.5, center.y),
            Vec3::new(1.7, 1.0, half_z * 2.0 + 0.7),
            "Bellstand entablature",
        );
    }
    // Corner pinnacles for the skyline.
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            spawn_mesh_named(
                commands,
                &meshes.pyramid,
                &materials.slate,
                Transform::from_xyz(
                    center.x + sx * (half_x - corner_inset),
                    stage_top + 1.9,
                    center.y + sz * (half_z - corner_inset),
                )
                .with_scale(Vec3::new(1.5, 1.8, 1.5)),
                "Bellstand pinnacle",
            );
        }
    }

    // The great bell, hung from the middle of the stage.
    spawn_bell(
        commands,
        meshes,
        materials,
        Vec3::new(center.x, 26.4, center.y),
        1.0,
        "The Bellstand watch-bell",
    );

    // Slate spire and finial.
    spawn_mesh_named(
        commands,
        &meshes.pyramid,
        &materials.slate,
        Transform::from_xyz(center.x, stage_top + 1.0 + 4.5, center.y)
            .with_scale(Vec3::new(15.5, 9.0, 17.5)),
        "Bellstand spire",
    );
    collision_world.add_box(
        Vec3::new(center.x - 7.0, stage_top + 1.0, center.y - 8.0),
        Vec3::new(center.x + 7.0, stage_top + 8.0, center.y + 8.0),
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        Vec3::new(center.x, stage_top + 10.6, center.y),
        0.07,
        2.6,
    );
    spawn_mesh_named(
        commands,
        &meshes.sphere,
        &materials.bronze,
        Transform::from_xyz(center.x, stage_top + 12.0, center.y).with_scale(Vec3::splat(0.38)),
        "Bellstand finial",
    );
}

fn build_bellstand_stair(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let bottom = Vec2::new(55.0, -248.0);
    let top = Vec2::new(64.0, -270.0);
    let direction = (top - bottom).normalize();
    let angle = direction.x.atan2(direction.y);
    for step in 0..14 {
        let t = step as f32 / 13.0;
        let point = bottom.lerp(top, t);
        let y = 3.15 + t * 9.8;
        let center = Vec3::new(point.x, y, point.y);
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.limestone,
            center,
            Vec3::new(4.1, 0.32, 2.2),
            angle,
            "Bellstand external stair",
        );
        add_rotated_box_collider_at(collision_world, center, Vec3::new(4.1, 0.32, 2.2), angle);
    }
    for side in [-1.0, 1.0] {
        let right = Vec2::new(direction.y, -direction.x) * side * 2.05;
        let center2 = bottom.lerp(top, 0.5) + right;
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.timber,
            Vec3::new(center2.x, 8.4, center2.y),
            Vec3::new(0.22, 1.1, bottom.distance(top)),
            angle,
            "Bellfoot stair rail",
        );
    }
}

fn build_saint_maren_tower(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center = Vec3::new(-253.0, 8.5, -402.0);
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        center,
        Vec3::new(8.5, 17.0, 8.5),
        "Saint Maren's modest bell tower",
    );
    add_open_bell_stage(
        commands,
        meshes,
        materials,
        Vec2::new(center.x, center.z),
        17.0,
        4.25,
        0.62,
        "Saint Maren's",
    );
    collision_world.add_box(
        Vec3::new(center.x - 4.25, 0.0, center.z - 4.25),
        Vec3::new(center.x + 4.25, 24.5, center.z + 4.25),
    );
}

/// A small open lantern for the parish landmarks: corner posts, a visible
/// swinging bell, and the slate pyramid lifted back on top.
#[allow(clippy::too_many_arguments)]
fn add_open_bell_stage(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    center: Vec2,
    base_y: f32,
    half_width: f32,
    bell_scale: f32,
    name: &str,
) {
    let stage_height = 3.4;
    let post_inset = 0.55;
    for sx in [-1.0, 1.0] {
        for sz in [-1.0, 1.0] {
            spawn_box_named(
                commands,
                meshes,
                &materials.fieldstone,
                Vec3::new(
                    center.x + sx * (half_width - post_inset),
                    base_y + stage_height * 0.5,
                    center.y + sz * (half_width - post_inset),
                ),
                Vec3::new(0.9, stage_height, 0.9),
                format!("{name} bell-stage post"),
            );
        }
    }
    // Low rail between the posts.
    for sz in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            Vec3::new(center.x, base_y + 0.45, center.y + sz * (half_width - 0.35)),
            Vec3::new(half_width * 2.0 - 1.0, 0.9, 0.35),
            format!("{name} bell-stage rail"),
        );
    }
    for sx in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            Vec3::new(center.x + sx * (half_width - 0.35), base_y + 0.45, center.y),
            Vec3::new(0.35, 0.9, half_width * 2.0 - 1.0),
            format!("{name} bell-stage rail"),
        );
    }
    spawn_bell(
        commands,
        meshes,
        materials,
        Vec3::new(center.x, base_y + 0.9, center.y),
        bell_scale,
        name,
    );
    spawn_mesh_named(
        commands,
        &meshes.pyramid,
        &materials.slate,
        Transform::from_xyz(center.x, base_y + stage_height + 1.9, center.y).with_scale(Vec3::new(
            half_width * 2.0 - 0.6,
            4.0,
            half_width * 2.0 - 0.6,
        )),
        format!("{name} tower roof"),
    );
}

fn build_parish_towers(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    for building in plan
        .buildings
        .iter()
        .filter(|building| building.id.starts_with("reserve_church_"))
    {
        let center2 = polygon_center(&building.polygon);
        let center = Vec3::new(center2.x, 6.7, center2.y);
        let name = building.name.as_deref().unwrap_or(&building.id);
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            center,
            Vec3::new(6.5, 13.4, 6.5),
            format!("{name} parish tower reserve"),
        );
        add_open_bell_stage(
            commands,
            meshes,
            materials,
            Vec2::new(center.x, center.z),
            13.4,
            3.25,
            0.5,
            name,
        );
        collision_world.add_box(
            center + Vec3::new(-3.25, -6.7, -3.25),
            center + Vec3::new(3.25, 13.5, 3.25),
        );
    }
}

fn build_old_sluice_face(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    let face_z = -588.86;
    for x in [-318.0, -292.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.iron,
            Vec3::new(x, 3.4, face_z),
            Vec3::new(16.0, 6.2, 0.18),
            "Blocked dry arch of the Old Sluice",
        );
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(x, 7.0, face_z + 0.1),
            Vec3::new(18.0, 1.0, 0.75),
            "Old Sluice arch lintel",
        );
    }
    for x in [-329.0, -305.0, -281.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(x, 3.5, face_z + 0.1),
            Vec3::new(2.0, 7.0, 0.8),
            "Old Sluice arch pier",
        );
    }
}

fn build_charnel_and_ilvane_details(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
) {
    spawn_box_named(
        commands,
        meshes,
        &materials.dark_wood,
        Vec3::new(-284.08, 1.35, -365.0),
        Vec3::new(0.16, 2.7, 1.65),
        "Saint Maren's charnel door",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new(-284.2, 2.95, -365.0),
        Vec3::new(0.5, 0.45, 2.4),
        "Saint Maren's worn charnel lintel",
    );

    // The chapel's public openings are visibly mortared; the occupied cell's
    // tiny north-facing squint is the sole living aperture.
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        Vec3::new(174.5, 1.7, -66.45),
        Vec3::new(2.6, 3.4, 0.22),
        "Mortared Ilvane Chapel door",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.window,
        Vec3::new(198.36, 2.0, -92.0),
        Vec3::new(0.12, 0.65, 0.45),
        "Ilvane anchorhold north squint",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.timber,
        Vec3::new(198.55, 1.05, -92.0),
        Vec3::new(0.55, 0.15, 1.2),
        "Ilvane anchorhold alms shelf",
    );
}

fn build_bridge_supports(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    for building in plan
        .buildings
        .iter()
        .filter(|building| building.use_name == "bridge" || building.id == "named_malt_house")
    {
        if building.polygon.len() != 4 {
            continue;
        }
        let p = building
            .polygon
            .iter()
            .map(|point| Vec2::from_array(*point))
            .collect::<Vec<_>>();
        let edge_01 = p[0].distance(p[1]);
        let edge_12 = p[1].distance(p[2]);
        let (ends, width) = if edge_01 >= edge_12 {
            ([(p[0] + p[3]) * 0.5, (p[1] + p[2]) * 0.5], edge_12)
        } else {
            ([(p[0] + p[1]) * 0.5, (p[2] + p[3]) * 0.5], edge_01)
        };
        for end in ends {
            let size = Vec2::new(1.25, (width - 1.0).max(1.2));
            let long = (ends[1] - ends[0]).normalize_or_zero();
            let angle = long.x.atan2(long.y);
            let center = Vec3::new(end.x, 2.1, end.y);
            spawn_rotated_box_named(
                commands,
                meshes,
                if building.material == "limestone" {
                    &materials.limestone
                } else {
                    &materials.timber
                },
                center,
                Vec3::new(size.x, 4.2, size.y),
                angle,
                format!(
                    "{} support",
                    building.name.as_deref().unwrap_or(&building.id)
                ),
            );
            add_rotated_box_collider_at(
                collision_world,
                center,
                Vec3::new(size.x, 4.2, size.y),
                angle,
            );
        }
    }
}

/// Dress the underside of every space a road-goer crosses beneath a building —
/// the three bridge upper storeys and the malt-house over Malt Passage — after
/// `bellfoot_passage_001.png`: a boarded ceiling with joists, a fascia over
/// each mouth, hanging lanterns that burn day and night, posted notices on the
/// spine piers, and a worn stone doorstep strip at each end.
fn build_covered_passages(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
) {
    let mut boards = MeshData::default();
    let mut ironwork = MeshData::default();
    let mut glass = MeshData::default();
    let mut notices = MeshData::default();
    let mut steps = MeshData::default();

    for building in plan
        .buildings
        .iter()
        .filter(|building| building.use_name == "bridge" || building.id == "named_malt_house")
    {
        if building.polygon.len() != 4 {
            continue;
        }
        let (base_y, _) = building_verticals(building);
        let p: Vec<Vec2> = building
            .polygon
            .iter()
            .map(|point| Vec2::from_array(*point))
            .collect();
        let edge_01 = p[0].distance(p[1]);
        let edge_12 = p[1].distance(p[2]);
        // The passage runs the long way; the mouths are the two short edges —
        // the same reading `build_bridge_supports` stands its piers on.
        let (ends, width) = if edge_01 >= edge_12 {
            ([(p[0] + p[3]) * 0.5, (p[1] + p[2]) * 0.5], edge_12)
        } else {
            ([(p[0] + p[1]) * 0.5, (p[2] + p[3]) * 0.5], edge_01)
        };
        let long_dir = (ends[1] - ends[0]).normalize_or_zero();
        let across = Vec2::new(-long_dir.y, long_dir.x);
        let run = ends[0].distance(ends[1]);
        let hash = stable_hash(&building.id);

        // Boarded ceiling, dark as the soffits, joists slung across it.
        let ceiling_y = base_y - 0.03;
        let first = boards.positions.len() as u32;
        for corner in &p {
            boards.vertex_shaded(
                Vec3::new(corner.x, ceiling_y, corner.y),
                Vec3::NEG_Y,
                Vec2::new(corner.x / 3.5, corner.y / 3.5),
                0.45,
            );
        }
        boards.indices.extend_from_slice(&[
            first,
            first + 1,
            first + 2,
            first,
            first + 2,
            first + 3,
        ]);
        let joists = ((run - 1.6) / 1.7).floor().max(1.0) as usize;
        for index in 0..joists {
            let t = (index as f32 + 1.0) / (joists as f32 + 1.0);
            let center = ends[0].lerp(ends[1], t);
            add_oriented_box(
                &mut boards,
                Vec3::new(center.x, ceiling_y - 0.07, center.y),
                Vec3::new(width * 0.5 - 0.15, 0.075, 0.09),
                across,
            );
        }

        for (end_index, end) in ends.iter().enumerate() {
            let inward = if end_index == 0 { long_dir } else { -long_dir };
            // Fascia board across the head of the mouth.
            let fascia = *end + inward * 0.12;
            add_oriented_box(
                &mut boards,
                Vec3::new(fascia.x, base_y - 0.19, fascia.y),
                Vec3::new(width * 0.5, 0.20, 0.055),
                across,
            );
            // A worn stone doorstep strip where the covered dark begins.
            add_oriented_box(
                &mut steps,
                Vec3::new(end.x, 0.045, end.y),
                Vec3::new(width * 0.5 - 0.35, 0.045, 0.42),
                across,
            );

            // Posted notices on both faces of the spine pier at this mouth.
            let pier_half = (width - 1.0).max(1.2) * 0.5;
            for side in [-1.0, 1.0] {
                let face_normal = across * side;
                let count = 1 + (hash >> (end_index as u32 * 4 + (side as i32 + 1) as u32)) % 3;
                for notice in 0..count {
                    let notice_hash = hash
                        ^ (end_index as u32 * 41)
                        ^ ((side as i32 + 2) as u32 * 97)
                        ^ notice.wrapping_mul(0x9E37_79B9);
                    let along = (notice as f32 - (count as f32 - 1.0) * 0.5)
                        * (0.62 + (notice_hash % 30) as f32 / 100.0)
                        + ((notice_hash >> 5) % 40) as f32 / 100.0
                        - 0.2;
                    let spot = *end
                        + long_dir * along.clamp(-pier_half + 0.4, pier_half - 0.4)
                        + face_normal * 0.665;
                    add_facade_panel(
                        &mut notices,
                        spot,
                        1.45 + ((notice_hash >> 7) % 50) as f32 / 100.0,
                        long_dir,
                        Vec3::new(face_normal.x, 0.0, face_normal.y),
                        0.28 + (notice_hash % 18) as f32 / 100.0,
                        0.36 + ((notice_hash >> 3) % 22) as f32 / 100.0,
                    );
                }
            }
        }

        // Lanterns down the centreline, chained to the boards.
        let lantern_count = if run > 26.0 { 3 } else { 2 };
        for index in 0..lantern_count {
            let t = (index as f32 + 1.0) / (lantern_count as f32 + 1.0);
            let drift = (((hash >> (index * 5)) % 3) as f32 - 1.0) * 0.35;
            let spot = ends[0].lerp(ends[1], t) + across * drift;
            let head_y = base_y - 0.78;
            add_oriented_box(
                &mut ironwork,
                Vec3::new(spot.x, (ceiling_y + head_y + 0.14) * 0.5, spot.y),
                Vec3::new(0.02, (ceiling_y - head_y - 0.14) * 0.5, 0.02),
                long_dir,
            );
            add_oriented_box(
                &mut ironwork,
                Vec3::new(spot.x, head_y + 0.13, spot.y),
                Vec3::new(0.10, 0.025, 0.10),
                long_dir,
            );
            add_oriented_box(
                &mut ironwork,
                Vec3::new(spot.x, head_y - 0.13, spot.y),
                Vec3::new(0.085, 0.025, 0.085),
                long_dir,
            );
            add_oriented_box(
                &mut glass,
                Vec3::new(spot.x, head_y, spot.y),
                Vec3::new(0.065, 0.105, 0.065),
                long_dir,
            );
            commands.spawn((
                Name::new(format!(
                    "Passage lantern: {}",
                    building.name.as_deref().unwrap_or(&building.id)
                )),
                PointLight {
                    color: Color::srgb(1.0, 0.62, 0.28),
                    intensity: 20_000.0,
                    range: 11.0,
                    radius: 0.1,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(spot.x, head_y - 0.25, spot.y),
            ));
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        boards,
        "Passage ceilings and boards",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.iron,
        ironwork,
        "Passage lantern ironwork",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.lantern_glass,
        glass,
        "Passage lantern panes",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_ochre,
        notices,
        "Passage posted notices",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.limestone,
        steps,
        "Passage doorsteps",
    );
}

fn build_ropewalk(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    for z in (232..=288).step_by(8) {
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            Vec3::new(-260.0, 1.35, z as f32),
            Vec3::new(0.18, 2.7, 0.18),
            "The Cut ropewalk post",
        );
    }
    for x in [-260.8, -260.25, -259.7] {
        spawn_box_named(
            commands,
            meshes,
            &materials.dark_wood,
            Vec3::new(x, 1.9, 260.0),
            Vec3::new(0.035, 0.035, 58.0),
            "The Cut ropewalk line",
        );
    }
}

fn build_osanne_stall(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    spawn_market_stall(
        commands,
        meshes,
        materials,
        collision_world,
        Vec3::new(18.0, 0.0, 350.0),
        Vec2::new(5.2, 3.0),
        -8.0_f32.to_radians(),
        1,
        "Osanne Vell's stall".into(),
    );
}

fn build_wharf_cranes(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    for (index, z) in [52.0, -176.0, -404.0].into_iter().enumerate() {
        spawn_yard_crane(
            commands,
            meshes,
            materials,
            Vec3::new(-601.0, 0.0, z),
            PI * 0.5,
            &format!("Outer Serle wharf crane {}", index + 1),
        );
    }
}

/// Timber galleries spanning the narrower streets at first-floor height — the
/// bridges full of onlookers in `the_bellstand_001.png`. A gallery only spawns
/// where a 2+-storey building actually stands on each side, so every one is
/// seated in masonry rather than floating.
fn build_street_galleries(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    let tall_building_at = |point: Vec2| {
        plan.buildings.iter().any(|building| {
            building.levels >= 2
                && building.use_name != "bridge"
                && point_in_polygon(point, &building.polygon)
        })
    };

    let mut built = 0;
    for road in &plan.roads {
        if !(2.0..=5.5).contains(&road.width_m) {
            continue;
        }
        for (segment_index, pair) in road.points.windows(2).enumerate() {
            let a = Vec2::from_array(pair[0]);
            let b = Vec2::from_array(pair[1]);
            let length = a.distance(b);
            if length < 12.0 {
                continue;
            }
            let hash = stable_hash(&format!("gallery-{}-{segment_index}", road.id));
            if hash % 2 != 0 {
                continue;
            }
            let t = 0.3 + (hash % 41) as f32 / 100.0;
            let center = a.lerp(b, t);
            let street_dir = (b - a) / length;
            let across = Vec2::new(-street_dir.y, street_dir.x);
            // Both flanks must carry a tall building right at the street edge;
            // façades sit at varying setbacks, so probe outward until one is
            // found and seat the gallery that deep into it.
            let seat_depth = |side: f32| {
                [0.7_f32, 1.6, 2.6].into_iter().find(|extra| {
                    tall_building_at(center + across * side * (road.width_m * 0.5 + extra))
                })
            };
            let (Some(seat_a), Some(seat_b)) = (seat_depth(1.0), seat_depth(-1.0)) else {
                continue;
            };

            let span = road.width_m + seat_a + seat_b + 1.6;
            let yaw = (-across.y).atan2(across.x);
            let floor_y = 4.55;
            let shifted = center + across * (seat_a - seat_b) * 0.5;
            let base = Vec3::new(shifted.x, floor_y, shifted.y);
            spawn_rotated_box_named(
                commands,
                meshes,
                &materials.timber,
                base,
                Vec3::new(span, 0.26, 2.5),
                yaw,
                format!("Street gallery over {}", road.name),
            );
            // Half-timbered parapet walls and a slate hood.
            for side in [-1.0, 1.0] {
                spawn_rotated_box_named(
                    commands,
                    meshes,
                    &materials.half_timber,
                    base + Vec3::new(street_dir.x, 0.0, street_dir.y) * side * 1.13
                        + Vec3::Y * 1.05,
                    Vec3::new(span, 1.85, 0.24),
                    yaw,
                    "Street gallery parapet",
                );
            }
            spawn_rotated_box_named(
                commands,
                meshes,
                &materials.slate,
                base + Vec3::Y * 2.35,
                Vec3::new(span + 0.6, 0.16, 3.1),
                yaw,
                "Street gallery roof",
            );
            spawn_rotated_box_named(
                commands,
                meshes,
                &materials.slate,
                base + Vec3::Y * 2.55,
                Vec3::new(span + 0.6, 0.14, 1.1),
                yaw,
                "Street gallery roof ridge",
            );
            add_rotated_box_collider_at(collision_world, base, Vec3::new(span, 0.26, 2.5), yaw);
            built += 1;
        }
    }
    info!("spanned {built} street galleries");
}

/// Ground-floor arcade strips on the buildings that front the town squares:
/// timber posts at ~2.4 m spacing carrying a beam and a slate pentice roof,
/// with the walkable colonnade between the posts and the façade. The posts are
/// scenery like the street props — the baked navigation predates them, so they
/// must not collide — and the roof rides above head height, so nothing at
/// street level changes.
fn build_square_arcades(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
) {
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();
    let mut woodwork = MeshData::default();
    let mut hoods = MeshData::default();
    let mut arcaded = 0;

    for building in &plan.buildings {
        let (base_y, eave_y) = building_verticals(building);
        // Towers, gates, churches and the overhead shells keep their bare
        // faces; the arcade belongs to the ordinary fabric around a square.
        if base_y > 0.1
            || eave_y < 6.0
            || building.use_name == "bridge"
            || building.id == "named_bellstand_tower"
            || building.id == "named_saint_marens"
            || building.id == "named_old_sluice"
            || building.id.starts_with("gate_")
            || building.id.starts_with("reserve_church_")
        {
            continue;
        }
        let tint = building_tint(building);
        let door_edge = door_edges.get(&building.id).copied();
        let orientation = plan::signed_area(&building.polygon).signum();
        for (edge_index, (a, b)) in building
            .polygon
            .iter()
            .zip(building.polygon.iter().cycle().skip(1))
            .enumerate()
        {
            let a2 = Vec2::from_array(*a);
            let b2 = Vec2::from_array(*b);
            let edge = b2 - a2;
            let length = edge.length();
            if length < 4.5 {
                continue;
            }
            let direction = edge / length;
            let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
            if orientation < 0.0 {
                normal2 = -normal2;
            }
            // The colonnade must stand on the square itself.
            let probe = a2 + direction * (length * 0.5) + normal2 * 1.6;
            if !squares
                .iter()
                .any(|square| point_in_polygon(probe, &square.polygon))
            {
                continue;
            }

            woodwork.set_brush(tint);
            hoods.set_brush(tint);
            let door_here = door_edge == Some(edge_index);
            let count = ((length - 1.6) / 2.4).floor().max(1.0) as usize;
            for index in 0..count {
                let along = length * (index as f32 + 1.0) / (count as f32 + 1.0);
                // Never plant a post in front of the doorway.
                if door_here && (along - length * 0.5).abs() < 1.6 {
                    continue;
                }
                let foot = a2 + direction * along + normal2 * 1.35;
                add_oriented_box(
                    &mut woodwork,
                    Vec3::new(foot.x, 1.62, foot.y),
                    Vec3::new(0.09, 1.62, 0.09),
                    direction,
                );
            }
            // The beam the posts carry.
            let beam_center = a2 + direction * (length * 0.5) + normal2 * 1.35;
            add_oriented_box(
                &mut woodwork,
                Vec3::new(beam_center.x, 3.33, beam_center.y),
                Vec3::new(length * 0.5 - 0.55, 0.09, 0.11),
                direction,
            );
            woodwork.reset_brush();

            // The pentice: a slate strip pitched off the façade over the walk.
            let inner_a = a2 + direction * 0.25;
            let inner_b = a2 + direction * (length - 0.25);
            let outer_a = inner_a + normal2 * 1.62;
            let outer_b = inner_b + normal2 * 1.62;
            let slope = Vec3::new(normal2.x * 1.62, -0.45, normal2.y * 1.62);
            let mut roof_normal = slope
                .cross(Vec3::new(direction.x, 0.0, direction.y))
                .normalize_or(Vec3::Y);
            if roof_normal.y < 0.0 {
                roof_normal = -roof_normal;
            }
            hoods.quad(
                [
                    Vec3::new(inner_a.x, 3.78, inner_a.y),
                    Vec3::new(inner_b.x, 3.78, inner_b.y),
                    Vec3::new(outer_b.x, 3.33, outer_b.y),
                    Vec3::new(outer_a.x, 3.33, outer_a.y),
                ],
                roof_normal,
                [
                    Vec2::ZERO,
                    Vec2::new(length / 7.0, 0.0),
                    Vec2::new(length / 7.0, 0.25),
                    Vec2::new(0.0, 0.25),
                ],
            );
            hoods.reset_brush();
            arcaded += 1;
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        woodwork,
        "Square arcade posts",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.slate,
        hoods,
        "Square arcade hoods",
    );
    info!("raised {arcaded} arcade strips on the squares");
}

/// The street-life kit: barrels, crates, sacks, firewood and hanging signs
/// hugging the façades beside doors. Everything is merged into five batched
/// meshes (one per material), and none of it collides: the baked navigation
/// predates these, so they are scenery for the eye, not walls for the feet —
/// exactly like the NPCs, which never collide with props either.
fn build_street_props(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
) {
    let mut timber = MeshData::default();
    let mut iron = MeshData::default();
    let mut dark_wood = MeshData::default();
    let mut ochre = MeshData::default();
    let mut russet = MeshData::default();
    let mut placed = 0;

    for building in &plan.buildings {
        let Some(&edge_index) = door_edges.get(&building.id) else {
            continue;
        };
        let hash = stable_hash(&building.id).rotate_left(9);
        // Two thirds of doorways get something standing outside.
        if hash % 3 == 0 {
            continue;
        }
        let polygon = &building.polygon;
        let a = Vec2::from_array(polygon[edge_index]);
        let b = Vec2::from_array(polygon[(edge_index + 1) % polygon.len()]);
        let edge = b - a;
        let length = edge.length();
        if length < 4.5 {
            continue;
        }
        let direction = edge / length;
        let orientation = plan::signed_area(polygon).signum();
        let mut normal = Vec2::new(edge.y, -edge.x).normalize() * orientation;
        // `plan` polygons wind either way; make sure the props step outward.
        if point_in_polygon(a + direction * (length * 0.5) + normal * 0.5, polygon) {
            normal = -normal;
        }
        let door = a + direction * (length * 0.5);

        // One or two clutter spots flanking the door, tight to the wall.
        let spots = if hash % 5 < 2 { 2 } else { 1 };
        for spot in 0..spots {
            let side = if (spot == 0) == (hash % 2 == 0) {
                1.0
            } else {
                -1.0
            };
            let spot_hash = hash.rotate_left(5 + spot as u32 * 7) ^ 0xA5A5_5A5A;
            let along_offset = 1.6 + (spot_hash % 90) as f32 / 100.0;
            let position2 = door + direction * side * along_offset + normal * 0.55;
            let position = Vec3::new(position2.x, 0.0, position2.y);
            match spot_hash % 4 {
                // A barrel, sometimes two.
                0 => {
                    add_barrel(&mut timber, &mut iron, position);
                    if spot_hash % 7 == 0 {
                        add_barrel(
                            &mut timber,
                            &mut iron,
                            position + Vec3::new(direction.x, 0.0, direction.y) * side * 0.72,
                        );
                    }
                }
                // Crates, stacked when the hash feels like it.
                1 => {
                    let skew = rotate2(direction, (spot_hash % 7) as f32 * 0.1);
                    add_oriented_box(
                        &mut timber,
                        position + Vec3::Y * 0.29,
                        Vec3::new(0.30, 0.29, 0.30),
                        skew,
                    );
                    if spot_hash % 3 == 0 {
                        add_oriented_box(
                            &mut timber,
                            position + Vec3::Y * 0.82,
                            Vec3::new(0.25, 0.24, 0.25),
                            rotate2(skew, 0.35),
                        );
                    }
                }
                // Sacks slumped against the wall.
                2 => {
                    let cloth = if spot_hash % 2 == 0 {
                        &mut ochre
                    } else {
                        &mut russet
                    };
                    for (offset, squash) in [(Vec2::ZERO, 0.24), (Vec2::new(0.42, 0.06), 0.19)] {
                        let sack2 = position2 + direction * offset.x + normal * offset.y;
                        add_sack(
                            cloth,
                            Vec3::new(sack2.x, squash * 0.8, sack2.y),
                            Vec3::new(0.30, squash, 0.27),
                        );
                    }
                }
                // A firewood rick: split logs against the plinth.
                _ => {
                    for row in 0..3 {
                        for column in 0..2 {
                            let log2 = position2
                                + direction * (column as f32 * 0.3 - 0.15)
                                + normal * 0.05;
                            add_log(
                                &mut dark_wood,
                                Vec3::new(log2.x, 0.14 + row as f32 * 0.24, log2.y),
                                0.115,
                                1.05,
                                direction,
                            );
                        }
                    }
                }
            }
            placed += 1;
        }

        // A hanging trade sign over some doors: bracket arm and swinging board.
        if hash % 8 == 0 {
            let arm_center = door + normal * 0.55;
            add_oriented_box(
                &mut iron,
                Vec3::new(arm_center.x, 3.35, arm_center.y),
                Vec3::new(0.03, 0.03, 0.5),
                direction,
            );
            let board2 = door + normal * 0.78;
            add_oriented_box(
                &mut dark_wood,
                Vec3::new(board2.x, 2.78, board2.y),
                Vec3::new(0.29, 0.33, 0.03),
                direction,
            );
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        timber,
        "Street props: cooperage",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.iron,
        iron,
        "Street props: ironmongery",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.dark_wood,
        dark_wood,
        "Street props: firewood and signs",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_ochre,
        ochre,
        "Street props: sacks",
    );
    spawn_batch(
        commands,
        meshes,
        &materials.cloth_russet,
        russet,
        "Street props: more sacks",
    );
    info!("scattered {placed} doorway prop clusters");
}

/// External timber stairs up to first-floor balconies — the left edge of
/// `the_bellstand_001.png` — on a hash-picked tenth of the taller ordinary
/// houses, and only where the flight provably stands in a yard: clear of every
/// other footprint, every road, every fixture, the squares and the curtain
/// wall. The flight itself is scenery (the baked navigation predates it, like
/// the props); only the landing high above the walk band gets a collider, so
/// a flying player can put down on it.
fn build_yard_stairs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    door_edges: &HashMap<String, usize>,
    collision_world: &mut CollisionWorld,
) {
    const RUN_ALONG: f32 = 6.4;
    let bounds: Vec<(Vec2, Vec2)> = plan
        .buildings
        .iter()
        .map(|building| {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in &building.polygon {
                min = min.min(Vec2::from_array(*point));
                max = max.max(Vec2::from_array(*point));
            }
            (min, max)
        })
        .collect();
    let squares: Vec<&Site> = plan
        .sites
        .iter()
        .filter(|site| site.kind == "square")
        .collect();

    let clear = |probe: Vec2, skip: usize| {
        plan.buildings.iter().enumerate().all(|(index, building)| {
            index == skip
                || probe.x < bounds[index].0.x
                || probe.y < bounds[index].0.y
                || probe.x > bounds[index].1.x
                || probe.y > bounds[index].1.y
                || !point_in_polygon(probe, &building.polygon)
        }) && plan.roads.iter().all(|road| {
            let margin = road.width_m * 0.5 + 0.8;
            road.points.windows(2).all(|pair| {
                segment_distance_squared(
                    probe,
                    Vec2::from_array(pair[0]),
                    Vec2::from_array(pair[1]),
                ) > margin * margin
            })
        }) && plan.fixtures.iter().all(|fixture| {
            let angle = fixture.angle_deg.to_radians();
            let delta = probe - Vec2::from_array(fixture.position);
            let local_x = delta.x * angle.cos() - delta.y * angle.sin();
            let local_z = delta.x * angle.sin() + delta.y * angle.cos();
            local_x.abs() > fixture.size[0] * 0.5 + 0.8
                || local_z.abs() > fixture.size[1] * 0.5 + 0.8
        }) && !squares
            .iter()
            .any(|square| point_in_polygon(probe, &square.polygon))
            && point_in_polygon(probe, &plan.wall_polygon_xz)
            && plan
                .wall_polygon_xz
                .windows(2)
                .chain(std::iter::once(
                    &[
                        *plan.wall_polygon_xz.last().unwrap(),
                        plan.wall_polygon_xz[0],
                    ][..],
                ))
                .all(|pair| {
                    segment_distance_squared(
                        probe,
                        Vec2::from_array(pair[0]),
                        Vec2::from_array(pair[1]),
                    ) > 3.2 * 3.2
                })
    };

    let mut timber = MeshData::default();
    let mut raised = 0;
    for (building_index, building) in plan.buildings.iter().enumerate() {
        let hash = stable_hash(&building.id).rotate_left(3);
        if building.named || building.levels < 2 || !hash.is_multiple_of(10) {
            continue;
        }
        let door_edge = door_edges.get(&building.id).copied();
        let orientation = plan::signed_area(&building.polygon).signum();
        let edge_count = building.polygon.len();
        let mut placed_here = false;
        for edge_offset in 0..edge_count {
            if placed_here {
                break;
            }
            let edge_index = (edge_offset + hash as usize) % edge_count;
            if door_edge == Some(edge_index) {
                continue;
            }
            let a2 = Vec2::from_array(building.polygon[edge_index]);
            let b2 = Vec2::from_array(building.polygon[(edge_index + 1) % edge_count]);
            let edge = b2 - a2;
            let length = edge.length();
            if length < RUN_ALONG + 0.6 {
                continue;
            }
            let direction = edge / length;
            let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
            if orientation < 0.0 {
                normal2 = -normal2;
            }
            let probes_clear = [0.6_f32, 2.4, 4.2, 6.2]
                .into_iter()
                .flat_map(|along| [0.6_f32, 1.7].map(|out| (along, out)))
                .all(|(along, out)| clear(a2 + direction * along + normal2 * out, building_index));
            if !probes_clear {
                continue;
            }

            add_yard_stair(
                &mut timber,
                collision_world,
                a2,
                direction,
                normal2,
                building_tint(building),
            );
            raised += 1;
            placed_here = true;
        }
    }

    spawn_batch(
        commands,
        meshes,
        &materials.timber,
        timber,
        "Yard stairs and balconies",
    );
    info!("raised {raised} yard stairs");
}

/// One straight flight against a wall — stringers, treads, handrail — up to a
/// railed landing at first-floor height, with a dark upper door behind it.
fn add_yard_stair(
    timber: &mut MeshData,
    collision_world: &mut CollisionWorld,
    a2: Vec2,
    direction: Vec2,
    normal2: Vec2,
    tint: [f32; 3],
) {
    const LANDING_Y: f32 = 3.15;
    let at = |along: f32, out: f32| a2 + direction * along + normal2 * out;
    timber.set_brush(tint);

    // Stringers on both flanks of the flight.
    for out in [0.38, 1.22] {
        add_face_member(
            timber,
            a2 + normal2 * out,
            direction,
            normal2,
            Vec2::new(0.4, 0.12),
            Vec2::new(4.6, LANDING_Y - 0.04),
            0.10,
            0.10,
            true,
        );
    }
    // Treads.
    for step in 0..11 {
        let along = 0.66 + step as f32 * 0.36;
        let rise = 0.18 + step as f32 * 0.27;
        let center = at(along, 0.8);
        add_oriented_box(
            timber,
            Vec3::new(center.x, rise, center.y),
            Vec3::new(0.19, 0.032, 0.44),
            direction,
        );
    }
    // Handrail and balusters on the open side.
    add_face_member(
        timber,
        a2 + normal2 * 1.26,
        direction,
        normal2,
        Vec2::new(0.4, 1.07),
        Vec2::new(4.6, LANDING_Y + 0.91),
        0.045,
        0.05,
        true,
    );
    for along in [1.4_f32, 2.6, 3.8] {
        let height = 0.12 + (along - 0.4) / 4.2 * (LANDING_Y - 0.16);
        let foot = at(along, 1.24);
        add_oriented_box(
            timber,
            Vec3::new(foot.x, height + 0.48, foot.y),
            Vec3::new(0.038, 0.48, 0.038),
            direction,
        );
    }

    // The landing: platform, two full-height posts, rails, and the door it
    // serves.
    let platform = at(5.5, 0.85);
    add_oriented_box(
        timber,
        Vec3::new(platform.x, LANDING_Y, platform.y),
        Vec3::new(0.92, 0.055, 0.72),
        direction,
    );
    for along in [4.72_f32, 6.27] {
        let foot = at(along, 1.46);
        add_oriented_box(
            timber,
            Vec3::new(foot.x, 2.05, foot.y),
            Vec3::new(0.07, 2.05, 0.07),
            direction,
        );
    }
    // Outer rail along the balcony edge, and the closed far end.
    add_face_member(
        timber,
        a2 + normal2 * 1.5,
        direction,
        normal2,
        Vec2::new(4.72, LANDING_Y + 0.92),
        Vec2::new(6.27, LANDING_Y + 0.92),
        0.045,
        0.05,
        true,
    );
    let end_origin = at(6.34, 0.0);
    add_face_member(
        timber,
        end_origin,
        normal2,
        direction,
        Vec2::new(0.15, LANDING_Y + 0.92),
        Vec2::new(1.45, LANDING_Y + 0.92),
        0.045,
        0.05,
        true,
    );
    // The dark upper door the stair exists for.
    let door = at(5.5, 0.05);
    timber.set_brush([tint[0] * 0.30, tint[1] * 0.28, tint[2] * 0.26]);
    add_oriented_box(
        timber,
        Vec3::new(door.x, LANDING_Y + 1.08, door.y),
        Vec3::new(0.52, 1.02, 0.05),
        direction,
    );
    timber.reset_brush();

    // Only the landing carries collision — it floats far above the walk band,
    // so the baked navigation below stays honest.
    add_rotated_box_collider_at(
        collision_world,
        Vec3::new(platform.x, LANDING_Y, platform.y),
        Vec3::new(1.84, 0.11, 1.44),
        (-direction.y).atan2(direction.x),
    );
}

/// Squared distance from `point` to the segment `a`–`b`.
fn segment_distance_squared(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let edge = b - a;
    let length_squared = edge.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance_squared(a);
    }
    let along = ((point - a).dot(edge) / length_squared).clamp(0.0, 1.0);
    point.distance_squared(a + edge * along)
}

fn rotate2(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// A coopered barrel: ten-sided drum with a lid and two iron hoops.
fn add_barrel(timber: &mut MeshData, iron: &mut MeshData, base: Vec3) {
    add_drum(timber, base + Vec3::Y * 0.42, 0.32, 0.84, true);
    add_drum(iron, base + Vec3::Y * 0.22, 0.335, 0.06, false);
    add_drum(iron, base + Vec3::Y * 0.62, 0.335, 0.06, false);
}

/// A vertical cylinder written into a batch: `cap` adds the top disc.
fn add_drum(mesh: &mut MeshData, center: Vec3, radius: f32, height: f32, cap: bool) {
    const SEGMENTS: usize = 10;
    let half = height * 0.5;
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * (PI * 2.0);
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * (PI * 2.0);
        let n0 = Vec3::new(a0.cos(), 0.0, a0.sin());
        let n1 = Vec3::new(a1.cos(), 0.0, a1.sin());
        let mid = ((n0 + n1) * 0.5).normalize_or(Vec3::X);
        mesh.quad(
            [
                center + n0 * radius - Vec3::Y * half,
                center + n1 * radius - Vec3::Y * half,
                center + n1 * radius + Vec3::Y * half,
                center + n0 * radius + Vec3::Y * half,
            ],
            mid,
            [
                Vec2::new(0.0, 0.0),
                Vec2::new(0.06, 0.0),
                Vec2::new(0.06, height / 7.0),
                Vec2::new(0.0, height / 7.0),
            ],
        );
        if cap {
            let top = center + Vec3::Y * half;
            mesh.triangle(
                [top, top + n0 * radius, top + n1 * radius],
                [Vec2::ZERO, Vec2::new(0.05, 0.0), Vec2::new(0.05, 0.05)],
                true,
            );
        }
    }
}

/// A horizontal log lying along `along`, split-firewood scale.
fn add_log(mesh: &mut MeshData, center: Vec3, radius: f32, length: f32, along: Vec2) {
    const SEGMENTS: usize = 7;
    let axis = Vec3::new(along.x, 0.0, along.y);
    let side = Vec3::new(-along.y, 0.0, along.x);
    let half = axis * (length * 0.5);
    for segment in 0..SEGMENTS {
        let a0 = segment as f32 / SEGMENTS as f32 * (PI * 2.0);
        let a1 = (segment + 1) as f32 / SEGMENTS as f32 * (PI * 2.0);
        let r0 = (side * a0.cos() + Vec3::Y * a0.sin()) * radius;
        let r1 = (side * a1.cos() + Vec3::Y * a1.sin()) * radius;
        let normal = ((r0 + r1) * 0.5).normalize_or(Vec3::Y);
        mesh.quad(
            [
                center - half + r0,
                center - half + r1,
                center + half + r1,
                center + half + r0,
            ],
            normal,
            [
                Vec2::ZERO,
                Vec2::new(0.04, 0.0),
                Vec2::new(0.04, 0.15),
                Vec2::new(0.0, 0.15),
            ],
        );
    }
    // End discs so the rick reads as cut wood.
    for (end, direction) in [(center + half, axis), (center - half, -axis)] {
        for segment in 0..SEGMENTS {
            let a0 = segment as f32 / SEGMENTS as f32 * (PI * 2.0);
            let a1 = (segment + 1) as f32 / SEGMENTS as f32 * (PI * 2.0);
            let r0 = (side * a0.cos() + Vec3::Y * a0.sin()) * radius;
            let r1 = (side * a1.cos() + Vec3::Y * a1.sin()) * radius;
            mesh.triangle(
                [end, end + r0, end + r1],
                [Vec2::ZERO, Vec2::new(0.03, 0.0), Vec2::new(0.03, 0.03)],
                false,
            );
        }
        let _ = direction;
    }
}

/// A slumped sack: a low-resolution squashed dome, batched.
fn add_sack(mesh: &mut MeshData, center: Vec3, radii: Vec3) {
    const SECTORS: usize = 8;
    const RINGS: usize = 4;
    for ring in 0..RINGS {
        let v0 = ring as f32 / RINGS as f32 * FRAC_PI_2_SACK;
        let v1 = (ring + 1) as f32 / RINGS as f32 * FRAC_PI_2_SACK;
        for sector in 0..SECTORS {
            let u0 = sector as f32 / SECTORS as f32 * (PI * 2.0);
            let u1 = (sector + 1) as f32 / SECTORS as f32 * (PI * 2.0);
            let point = |u: f32, v: f32| {
                center
                    + Vec3::new(
                        radii.x * v.cos() * u.cos(),
                        radii.y * v.sin(),
                        radii.z * v.cos() * u.sin(),
                    )
            };
            let normal = (point(u0, v0) + point(u1, v1) - center * 2.0).normalize_or(Vec3::Y);
            mesh.quad(
                [point(u0, v0), point(u1, v0), point(u1, v1), point(u0, v1)],
                normal,
                [
                    Vec2::ZERO,
                    Vec2::new(0.05, 0.0),
                    Vec2::new(0.05, 0.05),
                    Vec2::new(0.0, 0.05),
                ],
            );
        }
    }
}

const FRAC_PI_2_SACK: f32 = PI * 0.5;

fn point_in_polygon(point: Vec2, polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        if (a[1] > point.y) != (b[1] > point.y)
            && point.x < (b[0] - a[0]) * (point.y - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

fn build_fortifications(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) {
    let openings = [
        (Vec2::new(-35.0, 510.0), 18.0),
        (Vec2::new(495.0, 135.0), 28.0),
        (Vec2::new(15.0, -665.0), 18.0),
        (Vec2::new(-505.0, -135.0), 37.0),
        (Vec2::new(-455.0, -535.0), 6.0),
    ];

    for (start, end) in plan
        .wall_polygon_xz
        .iter()
        .zip(plan.wall_polygon_xz.iter().cycle().skip(1))
    {
        let start = Vec2::from_array(*start);
        let end = Vec2::from_array(*end);
        for (segment_start, segment_end) in wall_ranges_around_gates(start, end, &openings) {
            spawn_wall_segment(
                commands,
                meshes,
                materials,
                collision_world,
                segment_start,
                segment_end,
            );
        }
    }

    // The map places a tower at every wall vertex and at roughly 115 m along
    // long curtains.  Keep the same deterministic rule in 3D.
    let mut tower_points = plan
        .wall_polygon_xz
        .iter()
        .map(|point| Vec2::from_array(*point))
        .collect::<Vec<_>>();
    for (start, end) in plan
        .wall_polygon_xz
        .iter()
        .zip(plan.wall_polygon_xz.iter().cycle().skip(1))
    {
        let start = Vec2::from_array(*start);
        let end = Vec2::from_array(*end);
        let divisions = (start.distance(end) / 115.0).floor() as usize + 1;
        for step in 1..divisions {
            tower_points.push(start.lerp(end, step as f32 / divisions as f32));
        }
    }
    for (index, point) in tower_points.into_iter().enumerate() {
        let height = 18.0 + (stable_hash(&format!("wall-tower-{index}")) % 500) as f32 / 100.0;
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(point.x, height * 0.5, point.y),
            Vec3::new(12.0, height, 12.0),
            PI * 0.25,
            format!("Wall tower {:02}", index + 1),
        );
        spawn_mesh_named(
            commands,
            &meshes.pyramid,
            &materials.slate,
            Transform::from_xyz(point.x, height + 3.1, point.y)
                .with_rotation(Quat::from_rotation_y(PI * 0.25))
                .with_scale(Vec3::new(9.4, 5.2, 9.4)),
            format!("Wall tower {:02} roof", index + 1),
        );
        let half = 12.0 * 2.0_f32.sqrt() * 0.5;
        collision_world.add_box(
            Vec3::new(point.x - half, 0.0, point.y - half),
            Vec3::new(point.x + half, height + 5.7, point.y + half),
        );
    }

    build_gatehouses(commands, meshes, materials, collision_world);
}

fn wall_ranges_around_gates(start: Vec2, end: Vec2, openings: &[(Vec2, f32)]) -> Vec<(Vec2, Vec2)> {
    let edge = end - start;
    let length = edge.length();
    let mut gaps = Vec::new();
    for (point, width) in openings {
        let t = (*point - start).dot(edge) / edge.length_squared();
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let projected = start + edge * t;
        if projected.distance(*point) <= 32.0 {
            let half_t = width * 0.5 / length;
            gaps.push(((t - half_t).clamp(0.0, 1.0), (t + half_t).clamp(0.0, 1.0)));
        }
    }
    gaps.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut ranges = Vec::new();
    let mut cursor = 0.0_f32;
    for (gap_start, gap_end) in gaps {
        if gap_start > cursor + 0.001 {
            ranges.push((start + edge * cursor, start + edge * gap_start));
        }
        cursor = cursor.max(gap_end);
    }
    if cursor < 0.999 {
        ranges.push((start + edge * cursor, end));
    }
    ranges
}

fn spawn_wall_segment(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    start: Vec2,
    end: Vec2,
) {
    let edge = end - start;
    let length = edge.length();
    if length < 0.2 {
        return;
    }
    let center = (start + end) * 0.5;
    let yaw = -edge.y.atan2(edge.x);
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.limestone,
        Vec3::new(center.x, WALL_HEIGHT * 0.5, center.y),
        Vec3::new(length, WALL_HEIGHT, WALL_THICKNESS),
        yaw,
        "Ombreval city wall",
    );
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.paving,
        Vec3::new(center.x, WALL_HEIGHT + 0.12, center.y),
        Vec3::new(length, 0.24, WALL_THICKNESS + 1.5),
        yaw,
        "Ombreval wall walk",
    );

    let chunks = (length / 8.0).ceil().max(1.0) as usize;
    for chunk in 0..chunks {
        let a = start.lerp(end, chunk as f32 / chunks as f32);
        let b = start.lerp(end, (chunk + 1) as f32 / chunks as f32);
        let min = a.min(b) - Vec2::splat(WALL_THICKNESS * 0.65);
        let max = a.max(b) + Vec2::splat(WALL_THICKNESS * 0.65);
        collision_world.add_box(
            Vec3::new(min.x, 0.0, min.y),
            Vec3::new(max.x, WALL_HEIGHT + 0.5, max.y),
        );
    }
}

fn build_gatehouses(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let houses = [
        (
            "Wool Gate upper store",
            Vec3::new(-35.0, 12.0, 510.0),
            Vec3::new(58.0, 7.0, 7.0),
        ),
        (
            "Stone Gate upper store",
            Vec3::new(495.0, 12.0, 135.0),
            Vec3::new(7.0, 7.0, 68.0),
        ),
        (
            "Harne Gate upper store",
            Vec3::new(15.0, 12.0, -665.0),
            Vec3::new(58.0, 7.0, 7.0),
        ),
        (
            "River Gate upper store",
            Vec3::new(-505.0, 12.0, -135.0),
            Vec3::new(7.0, 7.0, 82.0),
        ),
    ];
    for (name, center, size) in houses {
        spawn_box_named(commands, meshes, &materials.limestone, center, size, name);
        spawn_mesh_named(
            commands,
            &meshes.pyramid,
            &materials.slate,
            Transform::from_translation(center + Vec3::Y * 5.7).with_scale(Vec3::new(
                size.x * 0.72,
                3.2,
                size.z * 0.72,
            )),
            format!("{name} roof"),
        );
        collision_world.add_box(center - size * 0.5, center + size * 0.5 + Vec3::Y * 3.5);
    }
}

fn spawn_place_markers(commands: &mut Commands, plan: &CityPlan) {
    for place in &plan.named_place_index {
        commands.spawn((
            Name::new(format!(
                "Place {:02}: {} ({})",
                place.number, place.name, place.kind
            )),
            LorePlaceNumber(place.number),
            Transform::from_xyz(place.anchor[0], 0.05, place.anchor[1]),
            Visibility::default(),
        ));
    }
}

fn add_surface_quad(
    mesh: &mut MeshData,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    y: f32,
    uv_span: f32,
) {
    mesh.quad(
        [
            Vec3::new(min_x, y, min_z),
            Vec3::new(min_x, y, max_z),
            Vec3::new(max_x, y, max_z),
            Vec3::new(max_x, y, min_z),
        ],
        Vec3::Y,
        [
            Vec2::new(min_x / uv_span, min_z / uv_span),
            Vec2::new(min_x / uv_span, max_z / uv_span),
            Vec2::new(max_x / uv_span, max_z / uv_span),
            Vec2::new(max_x / uv_span, min_z / uv_span),
        ],
    );
}

fn add_polygon_surface(mesh: &mut MeshData, polygon: &[[f32; 2]], y: f32, uv_span: f32) {
    for [a, b, c] in triangulate_polygon(polygon) {
        let points = [a, b, c].map(|index| {
            let point = polygon[index];
            Vec3::new(point[0], y, point[1])
        });
        mesh.triangle(
            points,
            points.map(|point| Vec2::new(point.x / uv_span, point.z / uv_span)),
            true,
        );
    }
}

fn add_road_ribbon(mesh: &mut MeshData, road: &Road, y: f32) {
    let half_width = road.width_m * 0.5;
    for pair in road.points.windows(2) {
        let a = Vec2::from_array(pair[0]);
        let b = Vec2::from_array(pair[1]);
        let direction = (b - a).normalize_or_zero();
        let side = Vec2::new(-direction.y, direction.x) * half_width;
        let points = [
            Vec3::new((a - side).x, y, (a - side).y),
            Vec3::new((a + side).x, y, (a + side).y),
            Vec3::new((b + side).x, y, (b + side).y),
            Vec3::new((b - side).x, y, (b - side).y),
        ];
        mesh.quad(
            points,
            Vec3::Y,
            points.map(|point| {
                Vec2::new(
                    point.x / FLOOR_TEXTURE_SPAN_METERS,
                    point.z / FLOOR_TEXTURE_SPAN_METERS,
                )
            }),
        );
    }
    for point in &road.points {
        add_disc_surface(mesh, Vec2::from_array(*point), half_width, y + 0.0002, 12);
    }
}

fn add_disc_surface(mesh: &mut MeshData, center: Vec2, radius: f32, y: f32, segments: usize) {
    for segment in 0..segments {
        let angle_a = segment as f32 * 2.0 * PI / segments as f32;
        let angle_b = (segment + 1) as f32 * 2.0 * PI / segments as f32;
        let a = center + Vec2::new(angle_a.cos(), angle_a.sin()) * radius;
        let b = center + Vec2::new(angle_b.cos(), angle_b.sin()) * radius;
        let points = [
            Vec3::new(center.x, y, center.y),
            Vec3::new(a.x, y, a.y),
            Vec3::new(b.x, y, b.y),
        ];
        mesh.triangle(
            points,
            points.map(|point| {
                Vec2::new(
                    point.x / FLOOR_TEXTURE_SPAN_METERS,
                    point.z / FLOOR_TEXTURE_SPAN_METERS,
                )
            }),
            true,
        );
    }
}

fn polygon_is_convex(polygon: &[[f32; 2]]) -> bool {
    let winding = plan::signed_area(polygon).signum();
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .zip(polygon.iter().cycle().skip(2))
        .all(|((a, b), c)| {
            let a = Vec2::from_array(*a);
            let b = Vec2::from_array(*b);
            let c = Vec2::from_array(*c);
            cross_2d(b - a, c - b) * winding >= -0.001
        })
}

fn triangulate_polygon(polygon: &[[f32; 2]]) -> Vec<[usize; 3]> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let mut remaining = if plan::signed_area(polygon) > 0.0 {
        (0..polygon.len()).collect::<Vec<_>>()
    } else {
        (0..polygon.len()).rev().collect::<Vec<_>>()
    };
    let mut triangles = Vec::with_capacity(polygon.len() - 2);
    let mut guard = polygon.len() * polygon.len();

    while remaining.len() > 3 && guard > 0 {
        guard -= 1;
        let mut clipped = false;
        for cursor in 0..remaining.len() {
            let previous = remaining[(cursor + remaining.len() - 1) % remaining.len()];
            let current = remaining[cursor];
            let next = remaining[(cursor + 1) % remaining.len()];
            let a = Vec2::from_array(polygon[previous]);
            let b = Vec2::from_array(polygon[current]);
            let c = Vec2::from_array(polygon[next]);
            if cross_2d(b - a, c - b) <= 0.0001 {
                continue;
            }
            if remaining.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle(Vec2::from_array(polygon[candidate]), a, b, c)
            }) {
                continue;
            }
            triangles.push([previous, current, next]);
            remaining.remove(cursor);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
    }
    if triangles.len() != polygon.len() - 2 {
        warn!("falling back to fan triangulation for a plan polygon");
        (1..polygon.len() - 1)
            .map(|index| [0, index, index + 1])
            .collect()
    } else {
        triangles
    }
}

fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn point_in_triangle(point: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let ab = cross_2d(b - a, point - a);
    let bc = cross_2d(c - b, point - b);
    let ca = cross_2d(a - c, point - c);
    ab >= -0.0001 && bc >= -0.0001 && ca >= -0.0001
}

fn polygon_center(polygon: &[[f32; 2]]) -> Vec2 {
    polygon
        .iter()
        .map(|point| Vec2::from_array(*point))
        .sum::<Vec2>()
        / polygon.len() as f32
}

fn stable_hash(text: &str) -> u32 {
    text.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ byte as u32).wrapping_mul(16_777_619)
    })
}

fn spawn_batch(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    data: MeshData,
    name: impl Into<String>,
) {
    if data.is_empty() {
        return;
    }
    commands.spawn((
        Name::new(name.into()),
        Mesh3d(meshes.add(data.into_mesh())),
        MeshMaterial3d(material.clone()),
        Transform::default(),
    ));
}

fn spawn_box_named(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    name: impl Into<String>,
) {
    spawn_mesh_named(
        commands,
        &meshes.cube,
        material,
        Transform::from_translation(center).with_scale(size),
        name,
    );
}

fn spawn_rotated_box_named(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    yaw: f32,
    name: impl Into<String>,
) {
    spawn_mesh_named(
        commands,
        &meshes.cube,
        material,
        Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(size),
        name,
    );
}

fn spawn_cylinder(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    radius: f32,
    height: f32,
) {
    spawn_mesh_named(
        commands,
        &meshes.cylinder,
        material,
        Transform::from_translation(center).with_scale(Vec3::new(radius, height, radius)),
        "Cylindrical city detail",
    );
}

fn spawn_mesh_named(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    transform: Transform,
    name: impl Into<String>,
) {
    commands.spawn((
        Name::new(name.into()),
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        transform,
    ));
}

fn add_rotated_box_collider(
    collision_world: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    yaw: f32,
    height: f32,
) {
    add_rotated_box_collider_at(
        collision_world,
        base + Vec3::Y * height * 0.5,
        Vec3::new(size.x, height, size.y),
        yaw,
    );
}

fn add_rotated_box_collider_at(
    collision_world: &mut CollisionWorld,
    center: Vec3,
    size: Vec3,
    yaw: f32,
) {
    let (sin, cos) = yaw.sin_cos();
    let half_x = (cos.abs() * size.x + sin.abs() * size.z) * 0.5;
    let half_z = (sin.abs() * size.x + cos.abs() * size.z) * 0.5;
    collision_world.add_box(
        center - Vec3::new(half_x, size.y * 0.5, half_z),
        center + Vec3::new(half_x, size.y * 0.5, half_z),
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;
    use crate::controller::{WALK_BAND_HI, WALK_BAND_LO};

    const NAV_JSON: &str = include_str!("../../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../../assets/world/navigation.bin");

    fn built_collision_world() -> CollisionWorld {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();
        std::mem::take(app.world_mut().resource_mut::<CollisionWorld>().as_mut())
    }

    /// Closes the loop between the two obstacle sets: the sim bakes the walkable
    /// surface from the cadastral plan, and this proves the bake agrees with what
    /// actually stops the player. No cell that an NPC may stand on is inside a
    /// `CollisionWorld` solid at walking height (02_navigation.md §2, §8).
    #[test]
    fn no_walkable_cell_is_solid() {
        let collision = built_collision_world();
        let nav = cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN)
            .expect("the committed navigation artifact loads");
        let grid = nav.grid();

        // The player is a standing AABB, not a point at WALK_Y, so a cell is solid
        // if any collider whose vertical extent overlaps the walk band
        // [WALK_BAND_LO, WALK_BAND_HI] covers its XZ. A collider that tops out
        // below WALK_Y (a water trough, the bellstand platform, a cistern rim)
        // still stops the player and so must not be walkable — the earlier
        // single-plane `contains_point(_, WALK_Y, _)` check was blind to those.
        let mut violations = Vec::new();
        for footprint in collision.solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI) {
            let min_x = footprint.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
            let max_x = footprint
                .iter()
                .map(|p| p.x)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_z = footprint.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            let max_z = footprint
                .iter()
                .map(|p| p.y)
                .fold(f32::NEG_INFINITY, f32::max);
            let col_lo = (((min_x as f64 - grid.x0) / grid.cell_m).floor()).max(0.0) as usize;
            let col_hi = (((max_x as f64 - grid.x0) / grid.cell_m).ceil() as usize).min(grid.w - 1);
            let row_lo = (((min_z as f64 - grid.z0) / grid.cell_m).floor()).max(0.0) as usize;
            let row_hi = (((max_z as f64 - grid.z0) / grid.cell_m).ceil() as usize).min(grid.h - 1);
            for row in row_lo..=row_hi {
                for col in col_lo..=col_hi {
                    let (cx, cz) = grid.centre(row, col);
                    if nav.is_walkable(cx, cz)
                        && collision.blocks_walk_band(
                            cx as f32,
                            cz as f32,
                            WALK_BAND_LO,
                            WALK_BAND_HI,
                        )
                    {
                        violations.push((cx, cz));
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "{} baked walkable cells are inside a collider, e.g. {:?}",
            violations.len(),
            &violations[..violations.len().min(12)]
        );
    }

    /// Regenerate `assets/world/collision_footprints.json` — the exact XZ
    /// footprints of everything that stops the player at walking height (walls,
    /// towers, gatehouses, buildings, fixtures, bridge piers, the ropewalk). The
    /// navigation bake subtracts these, so the walkable surface is the true
    /// complement of the collision world. Overhead structures (the bridges, the
    /// malt-house) are absent because their collider starts above head height.
    ///
    /// Run when scene collision changes:
    ///   cargo test export_collision_footprints -- --ignored --nocapture
    /// then re-run `scripts/bake_navigation.py`.
    #[test]
    #[ignore = "writes an asset; run manually when scene collision changes"]
    fn export_collision_footprints() {
        let collision = built_collision_world();
        let footprints: Vec<Vec<[f32; 2]>> = collision
            .solid_footprints_in_band(WALK_BAND_LO, WALK_BAND_HI)
            .into_iter()
            .map(|poly| poly.into_iter().map(|p| [p.x, p.y]).collect())
            .collect();
        let doc = serde_json::json!({
            "walk_band": [WALK_BAND_LO, WALK_BAND_HI],
            "note": "XZ footprints of colliders overlapping the standing player's \
                     walk band; generated by `cargo test export_collision_footprints -- --ignored`",
            "footprints": footprints,
        });
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/world/collision_footprints.json"
        );
        std::fs::write(path, serde_json::to_string(&doc).unwrap()).expect("write footprints");
        println!("wrote {} footprints to {path}", footprints.len());
    }

    /// Re-running the deterministic bake reproduces the committed artifact byte
    /// for byte, so `navigation.json` / `navigation.bin` cannot silently drift
    /// from the plan and collision export they are baked from (02_navigation.md
    /// §8). Ignored because it shells out to `uv` + Python.
    #[test]
    #[ignore = "requires uv + python; re-runs the bake and checks it is byte-stable"]
    fn bake_is_reproducible() {
        let root = env!("CARGO_MANIFEST_DIR");
        let bake = std::process::Command::new("uv")
            .args(["run", "scripts/bake_navigation.py"])
            .current_dir(root)
            .output()
            .expect("run the navigation bake");
        assert!(
            bake.status.success(),
            "bake failed: {}",
            String::from_utf8_lossy(&bake.stderr)
        );
        let clean = std::process::Command::new("git")
            .args([
                "diff",
                "--quiet",
                "--",
                "assets/world/navigation.json",
                "assets/world/navigation.bin",
            ])
            .current_dir(root)
            .status()
            .expect("run git diff");
        assert!(
            clean.success(),
            "re-baking changed the committed navigation artifact — it is not reproducible"
        );
    }

    /// The door the sim walks to and the door the player sees are the same door:
    /// every baked door sits on a render-eligible polygon edge, and its walkable
    /// node is one pace outward from that edge's midpoint — exactly where
    /// `add_facade_openings` now draws the panel (02_navigation.md §1, §8).
    #[test]
    fn the_door_you_see_is_the_door_you_walk_to() {
        let plan = plan::load();
        let nav = cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN)
            .expect("the committed navigation artifact loads");
        let by_id: HashMap<&str, &Building> =
            plan.buildings.iter().map(|b| (b.id.as_str(), b)).collect();

        for door in nav.doors() {
            let building = by_id[door.building.as_str()];

            // add_facade_openings early-returns (renders no door) for bridges and
            // the malt-house, so the bake must not emit one either — a baked door
            // here would be a phantom the player can walk to but never sees, on
            // open ground under overhead scenery.
            assert!(
                building.use_name != "bridge" && building.id != "named_malt_house",
                "{} is a phantom door: the renderer draws none for bridges or the \
                 malt-house, so the bake must skip it",
                door.building
            );

            let poly = &building.polygon;
            let n = poly.len();
            assert!(
                door.edge < n,
                "door edge index in range for {}",
                door.building
            );

            let a = Vec2::from_array(poly[door.edge]);
            let b = Vec2::from_array(poly[(door.edge + 1) % n]);
            let edge = b - a;
            let length = edge.length();
            assert!(
                length >= 3.2,
                "{} door is on a {length:.2} m edge the renderer would skip",
                door.building
            );

            let mut normal = Vec2::new(edge.y, -edge.x).normalize();
            if plan::signed_area(poly).signum() < 0.0 {
                normal = -normal;
            }
            let stand = a + edge * 0.5 + normal * 0.8;
            let node = nav.node_xz(door.node);
            let offset = (Vec2::new(node[0] as f32, node[1] as f32) - stand).length();
            assert!(
                offset < 0.5,
                "{} door node {node:?} is {offset:.2} m from its edge's threshold",
                door.building
            );
        }
    }

    #[test]
    fn city_builds_every_cadastral_building_and_named_place() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);

        app.update();

        let world = app.world_mut();
        let stats = *world.resource::<CityBuildStats>();
        assert_eq!(stats.planned_buildings, 2_566);
        // The Lanthorn is rendered by CathedralPlugin; every other footprint is
        // rendered by this plugin from the authoritative plan.
        assert_eq!(stats.rendered_plan_buildings, 2_565);
        assert_eq!(stats.named_places, 69);
        assert_eq!(stats.roads, 49);
        assert_eq!(stats.sites, 23);
        assert_eq!(stats.fixtures, 91);
        assert_eq!(stats.wharf_sheds, 15);

        let place_markers = world
            .query::<&LorePlaceNumber>()
            .iter(world)
            .map(|number| number.0)
            .collect::<Vec<_>>();
        assert_eq!(place_markers.len(), 69);
        assert!(place_markers.contains(&1));
        assert!(place_markers.contains(&69));
        assert!(world.resource::<CollisionWorld>().len() > 3_000);
    }

    /// Every water fixture in the plan is built, marked for its loop, and asks
    /// for a loop the catalog can actually synthesize — a typo here would be a
    /// silent well rather than a failed build.
    #[test]
    fn every_water_source_is_built_and_sounds_like_itself() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();

        let catalog = cathedral_sim::SoundCatalog::from_toml_str(include_str!(
            "../../assets/sounds/catalog.toml"
        ))
        .expect("the shipped catalog loads");
        let ambient_ids = catalog
            .ambients()
            .iter()
            .map(|ambient| ambient.sound_id.as_str())
            .collect::<BTreeSet<_>>();

        let world = app.world_mut();
        let sources = world
            .query::<(&water::WaterAmbience, &Transform)>()
            .iter(world)
            .map(|(ambience, transform)| {
                assert!(
                    ambient_ids.contains(ambience.sound_id),
                    "'{}' is not an [[ambients]] row in the sound catalog",
                    ambience.sound_id
                );
                assert!(ambience.audible_distance > 0.0);
                [transform.translation.x, transform.translation.z]
            })
            .collect::<Vec<_>>();

        // The nine named ward sources (Ford plus the eight of the ward network),
        // the Shambles well, and the Seven Lofts fire tanks.
        assert_eq!(sources.len(), 11);
        let plan = plan::load();
        for fixture in plan.fixtures.iter().filter(|fixture| {
            matches!(
                fixture.kind.as_str(),
                "well"
                    | "chain_well"
                    | "three_curb_well"
                    | "lodge_well"
                    | "cistern"
                    | "step_cistern"
                    | "fire_tanks"
            )
        }) {
            assert!(
                sources.contains(&fixture.position),
                "{} has no water ambience",
                fixture.id
            );
        }
    }

    #[test]
    fn batched_city_keeps_render_entity_count_bounded() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);
        app.update();

        let world = app.world_mut();
        let count = world
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(world)
            .count();
        assert!(count > 150, "expected authored details, got {count}");
        assert!(
            count < 1_500,
            "cadastral geometry should stay batched, got {count}"
        );
    }

    #[test]
    fn wall_openings_interrupt_the_curtain_at_every_gate() {
        let plan = plan::load();
        let openings = [
            (Vec2::new(-35.0, 510.0), 18.0),
            (Vec2::new(495.0, 135.0), 28.0),
            (Vec2::new(15.0, -665.0), 18.0),
            (Vec2::new(-505.0, -135.0), 37.0),
            (Vec2::new(-455.0, -535.0), 6.0),
        ];
        for (gate, width) in openings {
            let mut matched_wall = false;
            for (a, b) in plan
                .wall_polygon_xz
                .iter()
                .zip(plan.wall_polygon_xz.iter().cycle().skip(1))
            {
                let start = Vec2::from_array(*a);
                let end = Vec2::from_array(*b);
                let edge = end - start;
                let t = (gate - start).dot(edge) / edge.length_squared();
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let projected_gate = start + edge * t;
                if projected_gate.distance(gate) > 32.0 {
                    continue;
                }

                matched_wall = true;
                let ranges = wall_ranges_around_gates(start, end, &[(gate, width)]);
                assert!(
                    ranges.iter().all(|(range_start, range_end)| {
                        let range = *range_end - *range_start;
                        let range_t =
                            (projected_gate - *range_start).dot(range) / range.length_squared();
                        !(0.0..=1.0).contains(&range_t)
                            || (*range_start + range * range_t).distance(projected_gate) > 0.01
                    }),
                    "gate at {gate:?} is still covered by a wall segment"
                );
            }
            assert!(matched_wall, "gate at {gate:?} does not meet the curtain");
        }
    }

    #[test]
    fn concave_gaunt_footprint_triangulates_without_filling_its_passage() {
        let plan = plan::load();
        let gaunt = plan
            .buildings
            .iter()
            .find(|building| building.id == "named_gaunt_house")
            .unwrap();
        assert_eq!(gaunt.polygon.len(), 8);
        assert_eq!(triangulate_polygon(&gaunt.polygon).len(), 6);

        let passage = Vec2::new(-235.0, 20.0);
        assert!(!point_in_polygon_for_test(passage, &gaunt.polygon));

        let mut collision_world = CollisionWorld::default();
        add_footprint_colliders(&mut collision_world, &gaunt.polygon, 0.0, 8.0);
        assert!(
            collision_world
                .nearest_ray_hit(Vec3::new(-239.0, 1.0, 20.0), Vec3::X, 7.0)
                .is_none(),
            "the triangulated collider must not seal Gaunt House's passage"
        );
    }

    #[test]
    fn every_building_collider_starts_at_its_visible_facades() {
        let plan = plan::load();
        for building in &plan.buildings {
            if building.id == "named_lanthorn" {
                continue;
            }

            let (base_y, eave_y) = building_verticals(building);
            let mut collision_world = CollisionWorld::default();
            add_footprint_colliders(&mut collision_world, &building.polygon, base_y, eave_y);
            assert!(
                !collision_world.is_empty(),
                "{} has no collider",
                building.id
            );

            let winding = plan::signed_area(&building.polygon).signum();
            for (a, b) in building
                .polygon
                .iter()
                .zip(building.polygon.iter().cycle().skip(1))
            {
                let a = Vec2::from_array(*a);
                let b = Vec2::from_array(*b);
                let midpoint = (a + b) * 0.5;
                let edge = b - a;
                let outward = Vec2::new(edge.y, -edge.x).normalize() * winding;
                let ray_start = midpoint + outward * 0.75;
                let distance = collision_world
                    .nearest_ray_hit(
                        Vec3::new(ray_start.x, base_y + 0.5, ray_start.y),
                        Vec3::new(-outward.x, 0.0, -outward.y),
                        1.5,
                    )
                    .unwrap_or_else(|| panic!("{} has an unprotected facade", building.id));
                assert!(
                    (distance - 0.75).abs() < 0.002,
                    "{} collider is {distance:.3} m from its facade",
                    building.id
                );
            }
        }
    }

    fn point_in_polygon_for_test(point: Vec2, polygon: &[[f32; 2]]) -> bool {
        let mut inside = false;
        for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
            let a = Vec2::from_array(*a);
            let b = Vec2::from_array(*b);
            if (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                inside = !inside;
            }
        }
        inside
    }
}
