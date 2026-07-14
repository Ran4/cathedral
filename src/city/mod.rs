//! The cadastral city of Ombreval.
//!
//! `lore/places/ombreval_buildings.json` is the same plan that produces the
//! zoomable bird's-eye SVG.  The game consumes those exact parcels, routes,
//! sites, fixtures, storey counts, materials, and stable IDs instead of
//! inventing a second procedural grid here.

mod monuments;
mod plan;
pub mod water;

use std::{collections::BTreeMap, f32::consts::PI};

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

#[derive(Default)]
struct MeshData {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl MeshData {
    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn vertex(&mut self, position: Vec3, normal: Vec3, uv: Vec2) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push(position.to_array());
        self.normals.push(normal.normalize_or(Vec3::Y).to_array());
        self.uvs.push(uv.to_array());
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
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_indices(Indices::U32(self.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
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
        dark_wood: matte(materials, Color::srgb(0.075, 0.045, 0.028), 0.86),
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
            perceptual_roughness: 0.3,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        cloth_ochre: matte(materials, Color::srgb(0.45, 0.29, 0.095), 0.92),
        cloth_russet: matte(materials, Color::srgb(0.38, 0.095, 0.055), 0.92),
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

fn build_buildings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &CityMaterials,
    plan: &CityPlan,
    collision_world: &mut CollisionWorld,
) -> usize {
    let mut walls = BTreeMap::<WallKind, MeshData>::new();
    let mut roofs = BTreeMap::<RoofKind, MeshData>::new();
    let mut windows = MeshData::default();
    let mut doors = MeshData::default();
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
        add_building_walls(&mut walls, building, dominant_wall, base_y, eave_y);

        let roof_kind = roof_kind(building);
        let roof_height = add_building_roof(
            roofs.entry(roof_kind).or_default(),
            walls.entry(dominant_wall).or_default(),
            &building.polygon,
            eave_y,
        );
        add_facade_openings(&mut windows, &mut doors, building, base_y, eave_y);
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

    rendered
}

fn add_building_walls(
    walls: &mut BTreeMap<WallKind, MeshData>,
    building: &Building,
    dominant: WallKind,
    base_y: f32,
    eave_y: f32,
) {
    match dominant {
        WallKind::HalfTimber if base_y < 0.1 => {
            let stone_top = (base_y + 3.0).min(eave_y);
            add_extruded_walls(
                walls.entry(WallKind::Fieldstone).or_default(),
                &building.polygon,
                base_y,
                stone_top,
            );
            if stone_top < eave_y {
                add_extruded_walls(
                    walls.entry(WallKind::HalfTimber).or_default(),
                    &building.polygon,
                    stone_top,
                    eave_y,
                );
            }
        }
        WallKind::Plaster if base_y < 0.1 => {
            let plinth_top = (base_y + 0.65).min(eave_y);
            add_extruded_walls(
                walls.entry(WallKind::Fieldstone).or_default(),
                &building.polygon,
                base_y,
                plinth_top,
            );
            add_extruded_walls(
                walls.entry(WallKind::Plaster).or_default(),
                &building.polygon,
                plinth_top,
                eave_y,
            );
        }
        _ => add_extruded_walls(
            walls.entry(dominant).or_default(),
            &building.polygon,
            base_y,
            eave_y,
        ),
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
        "named_bellstand_tower" => 31.5,
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

fn add_extruded_walls(mesh: &mut MeshData, polygon: &[[f32; 2]], bottom: f32, top: f32) {
    if top <= bottom + 0.01 {
        return;
    }
    let orientation = plan::signed_area(polygon).signum();
    for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
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
        let points = [
            Vec3::new(a2.x, bottom, a2.y),
            Vec3::new(b2.x, bottom, b2.y),
            Vec3::new(b2.x, top, b2.y),
            Vec3::new(a2.x, top, a2.y),
        ];
        mesh.quad(
            points,
            normal,
            [
                Vec2::new(0.0, bottom / 7.0),
                Vec2::new(length / 7.0, bottom / 7.0),
                Vec2::new(length / 7.0, top / 7.0),
                Vec2::new(0.0, top / 7.0),
            ],
        );
    }
}

fn add_building_roof(
    roof: &mut MeshData,
    gable_wall: &mut MeshData,
    polygon: &[[f32; 2]],
    eave_y: f32,
) -> f32 {
    if polygon.len() != 4 {
        add_polygon_surface(roof, polygon, eave_y + 0.08, 7.0);
        return 0.16;
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

    let (ridge_a, ridge_b, planes, gables) = if edge_01 <= edge_12 {
        let a = (p[0] + p[1]) * 0.5;
        let b = (p[2] + p[3]) * 0.5;
        (
            a,
            b,
            [([p[0], p[3]], [b, a]), ([p[1], p[2]], [b, a])],
            [(p[0], p[1], a), (p[3], p[2], b)],
        )
    } else {
        let a = (p[1] + p[2]) * 0.5;
        let b = (p[3] + p[0]) * 0.5;
        (
            a,
            b,
            [([p[0], p[1]], [a, b]), ([p[3], p[2]], [a, b])],
            [(p[1], p[2], a), (p[0], p[3], b)],
        )
    };

    for (eave, ridge) in planes {
        let points = [
            Vec3::new(eave[0].x, eave_y, eave[0].y),
            Vec3::new(eave[1].x, eave_y, eave[1].y),
            Vec3::new(ridge[0].x, y_ridge, ridge[0].y),
            Vec3::new(ridge[1].x, y_ridge, ridge[1].y),
        ];
        let normal = (points[1] - points[0])
            .cross(points[3] - points[0])
            .normalize_or(Vec3::Y);
        roof.quad(
            points,
            normal,
            points.map(|point| Vec2::new(point.x / 7.0, point.z / 7.0)),
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

    // Explicitly use both values so a malformed rectangle cannot quietly
    // collapse its ridge to a point.
    debug_assert!(ridge_a.distance(ridge_b) > 0.1);
    roof_height
}

fn add_facade_openings(
    windows: &mut MeshData,
    doors: &mut MeshData,
    building: &Building,
    base_y: f32,
    eave_y: f32,
) {
    if building.use_name == "bridge" || building.id == "named_malt_house" {
        return;
    }
    let orientation = plan::signed_area(&building.polygon).signum();
    for (edge_index, (a, b)) in building
        .polygon
        .iter()
        .zip(building.polygon.iter().cycle().skip(1))
        .enumerate()
    {
        let a = Vec2::from_array(*a);
        let b = Vec2::from_array(*b);
        let edge = b - a;
        let length = edge.length();
        if length < 3.2 {
            continue;
        }
        let direction = edge / length;
        let mut normal2 = Vec2::new(edge.y, -edge.x).normalize();
        if orientation < 0.0 {
            normal2 = -normal2;
        }
        let normal = Vec3::new(normal2.x, 0.0, normal2.y);
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
                let along = length * (index as f32 + 1.0) / (count as f32 + 1.0);
                let center2 = a + direction * along + normal2 * 0.035;
                add_facade_panel(windows, center2, y, direction, normal, 1.0, 1.35);
            }
        }

        if edge_index == stable_hash(&building.id) as usize % building.polygon.len() {
            let center2 = a + direction * (length * 0.5) + normal2 * 0.045;
            add_facade_panel(doors, center2, base_y + 1.25, direction, normal, 1.35, 2.5);
        }
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
    build_saint_maren_tower(commands, meshes, materials, collision_world);
    build_parish_towers(commands, meshes, materials, plan, collision_world);
    build_old_sluice_face(commands, meshes, materials);
    build_charnel_and_ilvane_details(commands, meshes, materials);
    build_bridge_supports(commands, meshes, materials, plan, collision_world);
    build_ropewalk(commands, meshes, materials);
    build_osanne_stall(commands, meshes, materials, collision_world);
    build_wharf_cranes(commands, meshes, materials);
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
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        Vec3::new(64.0, 27.0, -270.0),
        1.15,
        1.9,
    );
}

fn build_saint_maren_tower(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center = Vec3::new(-253.0, 10.0, -402.0);
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        center,
        Vec3::new(8.5, 20.0, 8.5),
        "Saint Maren's modest bell tower",
    );
    spawn_mesh_named(
        commands,
        &meshes.pyramid,
        &materials.slate,
        Transform::from_xyz(center.x, 22.2, center.z).with_scale(Vec3::new(6.8, 4.2, 6.8)),
        "Saint Maren's tower roof",
    );
    collision_world.add_box(
        Vec3::new(center.x - 4.25, 0.0, center.z - 4.25),
        Vec3::new(center.x + 4.25, 24.5, center.z + 4.25),
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
        let center = Vec3::new(center2.x, 7.8, center2.y);
        spawn_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            center,
            Vec3::new(6.5, 15.6, 6.5),
            format!(
                "{} parish tower reserve",
                building.name.as_deref().unwrap_or(&building.id)
            ),
        );
        spawn_mesh_named(
            commands,
            &meshes.pyramid,
            &materials.slate,
            Transform::from_xyz(center.x, 17.7, center.z).with_scale(Vec3::new(5.2, 3.6, 5.2)),
            "Reserved parish tower roof",
        );
        collision_world.add_box(
            center + Vec3::new(-3.25, -7.8, -3.25),
            center + Vec3::new(3.25, 12.0, 3.25),
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
