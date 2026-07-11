//! The procedural city surrounding the cathedral.
//!
//! The city covers roughly 1.2 by 1.0 kilometres. Large, repeated forms create
//! the distant skyline while street-facing details keep the walkable scale rich.

use bevy::{math::Affine2, prelude::*};

use crate::{
    controller::CollisionWorld,
    materials::{FLOOR_TEXTURE_SPAN_METERS, load_repeating_texture},
    monuments::build_approach_monuments,
};

const CITY_MIN_X: f32 = -520.0;
const CITY_MAX_X: f32 = 520.0;
const CITY_MIN_Z: f32 = -550.0;
const CITY_MAX_Z: f32 = 650.0;
const NARROW_STREET_WIDTH: f32 = 7.0;
const AVENUE_WIDTH: f32 = 14.0;
const CEREMONIAL_WIDTH: f32 = 20.0;
const LANE_WIDTH: f32 = 4.6;
const CANAL_X: f32 = -480.0;
const GRAND_FORECOURT_WIDTH: f32 = 77.0;
const GRAND_FORECOURT_DEPTH: f32 = 52.0;
const GRAND_FORECOURT_CENTER_Z: f32 = 131.0;
const GRAND_APPROACH_WIDTH: f32 = 24.0;
const GRAND_APPROACH_LENGTH: f32 = 352.0;
const GRAND_APPROACH_CENTER_Z: f32 = 339.0;

const X_ROADS: [f32; 9] = [
    -480.0, -360.0, -240.0, -120.0, 0.0, 120.0, 240.0, 360.0, 480.0,
];
const Z_ROADS: [f32; 10] = [
    -500.0, -380.0, -260.0, -140.0, -20.0, 100.0, 220.0, 340.0, 460.0, 580.0,
];

pub struct CityPlugin;

impl Plugin for CityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_city);
    }
}

#[derive(Clone)]
struct CityMeshes {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    sphere: Handle<Mesh>,
    cone: Handle<Mesh>,
    roof: Handle<Mesh>,
    window_surround_rows: [Handle<Mesh>; 5],
    window_glass_rows: [Handle<Mesh>; 5],
}

#[derive(Clone)]
struct CityMaterials {
    ground: Handle<StandardMaterial>,
    forecourt_paving: Handle<StandardMaterial>,
    cathedral_cross_street: Handle<StandardMaterial>,
    cathedral_approach: Handle<StandardMaterial>,
    limestone: Handle<StandardMaterial>,
    pale_stone: Handle<StandardMaterial>,
    ochre: Handle<StandardMaterial>,
    rose_plaster: Handle<StandardMaterial>,
    umber: Handle<StandardMaterial>,
    half_timber: Handle<StandardMaterial>,
    road: Handle<StandardMaterial>,
    marble: Handle<StandardMaterial>,
    slate: Handle<StandardMaterial>,
    terracotta: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    bronze: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    window_blue: Handle<StandardMaterial>,
    window_warm: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    leaf: Handle<StandardMaterial>,
    lamp: Handle<StandardMaterial>,
}

fn build_city(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut collision_world: ResMut<CollisionWorld>,
) {
    let city_meshes = create_meshes(&mut meshes);
    let city_materials = create_materials(&asset_server, &mut materials);

    build_ground_and_streets(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_city_blocks(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_dense_cathedral_quarter(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_grand_forecourt(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_approach_monuments(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut collision_world,
    );
    build_town_squares(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_landmarks(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_cathedral_exterior(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
    build_street_furniture(&mut commands, &city_meshes, &city_materials);
    build_city_walls(
        &mut commands,
        &city_meshes,
        &city_materials,
        &mut collision_world,
    );
}

fn create_meshes(meshes: &mut Assets<Mesh>) -> CityMeshes {
    let window_surround_rows = std::array::from_fn(|index| {
        meshes.add(window_row_mesh(index + 1, Vec3::new(1.55, 2.0, 0.11)))
    });
    let window_glass_rows = std::array::from_fn(|index| {
        meshes.add(window_row_mesh(index + 1, Vec3::new(1.08, 1.55, 0.08)))
    });

    CityMeshes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(1.0, 1.0)),
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(20, 12)),
        cone: meshes.add(Cone::new(1.0, 1.0)),
        roof: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4).build()),
        window_surround_rows,
        window_glass_rows,
    }
}

/// Combines a complete row into one mesh so a detailed façade costs two
/// render entities (stone surround and glass) rather than two per opening.
fn window_row_mesh(count: usize, size: Vec3) -> Mesh {
    const SPACING: f32 = 4.0;

    let x_for = |index: usize| (index as f32 - (count - 1) as f32 * 0.5) * SPACING;
    let mut row = Cuboid::new(size.x, size.y, size.z)
        .mesh()
        .build()
        .transformed_by(Transform::from_xyz(x_for(0), 0.0, 0.0));
    for index in 1..count {
        let window = Cuboid::new(size.x, size.y, size.z)
            .mesh()
            .build()
            .transformed_by(Transform::from_xyz(x_for(index), 0.0, 0.0));
        row.merge(&window)
            .expect("window row cuboids use compatible vertex layouts");
    }
    row
}

fn create_materials(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> CityMaterials {
    let limestone_texture = load_repeating_texture(asset_server, "textures/limestone.png");
    let ground_texture = load_repeating_texture(asset_server, "textures/cathedral_floor.png");
    let forecourt_texture = load_repeating_texture(asset_server, "textures/city_plaza_paving.png");
    let street_texture = load_repeating_texture(asset_server, "textures/city_street_cobbles.png");
    let plaster_texture = load_repeating_texture(asset_server, "textures/city_plaster.png");
    let half_timber_texture = load_repeating_texture(asset_server, "textures/city_half_timber.png");
    let fieldstone_texture = load_repeating_texture(asset_server, "textures/city_fieldstone.png");
    let terracotta_texture = load_repeating_texture(asset_server, "textures/city_terracotta.png");
    let slate_texture = load_repeating_texture(asset_server, "textures/city_slate.png");
    CityMaterials {
        ground: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(ground_texture),
            uv_transform: Affine2::from_scale(Vec2::new(
                (CITY_MAX_X - CITY_MIN_X) / FLOOR_TEXTURE_SPAN_METERS,
                (CITY_MAX_Z - CITY_MIN_Z) / FLOOR_TEXTURE_SPAN_METERS,
            )),
            perceptual_roughness: 0.78,
            reflectance: 0.34,
            ..default()
        }),
        forecourt_paving: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(forecourt_texture),
            uv_transform: Affine2::from_scale(Vec2::new(
                GRAND_FORECOURT_WIDTH / FLOOR_TEXTURE_SPAN_METERS,
                GRAND_FORECOURT_DEPTH / FLOOR_TEXTURE_SPAN_METERS,
            )),
            perceptual_roughness: 0.9,
            reflectance: 0.28,
            ..default()
        }),
        cathedral_cross_street: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(street_texture.clone()),
            uv_transform: Affine2::from_scale(Vec2::new(
                (CITY_MAX_X - CITY_MIN_X) / FLOOR_TEXTURE_SPAN_METERS,
                CEREMONIAL_WIDTH / FLOOR_TEXTURE_SPAN_METERS,
            )),
            perceptual_roughness: 0.92,
            reflectance: 0.24,
            ..default()
        }),
        cathedral_approach: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(street_texture),
            uv_transform: Affine2::from_scale(Vec2::new(
                GRAND_APPROACH_WIDTH / FLOOR_TEXTURE_SPAN_METERS,
                GRAND_APPROACH_LENGTH / FLOOR_TEXTURE_SPAN_METERS,
            )),
            perceptual_roughness: 0.92,
            reflectance: 0.24,
            ..default()
        }),
        limestone: materials.add(StandardMaterial {
            base_color: Color::srgb(0.96, 0.90, 0.78),
            base_color_texture: Some(limestone_texture.clone()),
            uv_transform: Affine2::from_scale(Vec2::splat(3.0)),
            perceptual_roughness: 0.86,
            ..default()
        }),
        pale_stone: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.96, 0.87),
            base_color_texture: Some(limestone_texture),
            uv_transform: Affine2::from_scale(Vec2::splat(3.0)),
            perceptual_roughness: 0.8,
            ..default()
        }),
        ochre: materials.add(StandardMaterial {
            base_color: Color::srgb(0.94, 0.80, 0.57),
            base_color_texture: Some(plaster_texture.clone()),
            uv_transform: Affine2::from_scale(Vec2::splat(2.5)),
            perceptual_roughness: 0.9,
            ..default()
        }),
        rose_plaster: materials.add(StandardMaterial {
            base_color: Color::srgb(0.84, 0.64, 0.53),
            base_color_texture: Some(plaster_texture),
            uv_transform: Affine2::from_scale(Vec2::splat(2.5)),
            perceptual_roughness: 0.9,
            ..default()
        }),
        umber: materials.add(StandardMaterial {
            base_color: Color::srgb(0.76, 0.70, 0.61),
            base_color_texture: Some(fieldstone_texture),
            uv_transform: Affine2::from_scale(Vec2::splat(3.5)),
            perceptual_roughness: 0.88,
            ..default()
        }),
        half_timber: materials.add(StandardMaterial {
            base_color: Color::srgb(0.91, 0.84, 0.72),
            base_color_texture: Some(half_timber_texture),
            uv_transform: Affine2::from_scale(Vec2::splat(2.0)),
            perceptual_roughness: 0.82,
            ..default()
        }),
        road: matte(materials, Color::srgb(0.22, 0.20, 0.17)),
        marble: materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.45, 0.40),
            perceptual_roughness: 0.35,
            reflectance: 0.55,
            ..default()
        }),
        slate: materials.add(StandardMaterial {
            base_color: Color::srgb(0.76, 0.77, 0.76),
            base_color_texture: Some(slate_texture),
            uv_transform: Affine2::from_scale(Vec2::splat(4.0)),
            metallic: 0.04,
            perceptual_roughness: 0.72,
            ..default()
        }),
        terracotta: materials.add(StandardMaterial {
            base_color: Color::srgb(0.96, 0.78, 0.63),
            base_color_texture: Some(terracotta_texture),
            uv_transform: Affine2::from_scale(Vec2::splat(4.0)),
            perceptual_roughness: 0.86,
            ..default()
        }),
        wood: matte(materials, Color::srgb(0.12, 0.045, 0.02)),
        bronze: materials.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.075, 0.035),
            metallic: 0.85,
            perceptual_roughness: 0.38,
            ..default()
        }),
        gold: materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.42, 0.08),
            metallic: 0.92,
            perceptual_roughness: 0.24,
            ..default()
        }),
        window_blue: materials.add(StandardMaterial {
            base_color: Color::srgb(0.028, 0.038, 0.040),
            perceptual_roughness: 0.28,
            metallic: 0.18,
            reflectance: 0.72,
            ..default()
        }),
        window_warm: materials.add(StandardMaterial {
            base_color: Color::srgb(0.105, 0.060, 0.027),
            emissive: LinearRgba::rgb(0.018, 0.008, 0.002),
            perceptual_roughness: 0.32,
            reflectance: 0.58,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(0.015, 0.10, 0.15, 0.92),
            metallic: 0.25,
            perceptual_roughness: 0.12,
            reflectance: 0.82,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        leaf: matte(materials, Color::srgb(0.055, 0.13, 0.075)),
        lamp: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.58, 0.18),
            emissive: LinearRgba::rgb(10.0, 3.0, 0.35),
            ..default()
        }),
    }
}

fn matte(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.82,
        ..default()
    })
}

fn build_ground_and_streets(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let city_center_z = (CITY_MIN_Z + CITY_MAX_Z) * 0.5;
    spawn_box(
        commands,
        meshes,
        &materials.ground,
        Vec3::new(0.0, -0.46, city_center_z),
        Vec3::new(CITY_MAX_X - CITY_MIN_X, 0.9, CITY_MAX_Z - CITY_MIN_Z),
    );
    collision_world.add_box(
        Vec3::new(CITY_MIN_X, -1.2, CITY_MIN_Z),
        Vec3::new(CITY_MAX_X, 0.0, CITY_MAX_Z),
    );

    for x in X_ROADS {
        if x == CANAL_X {
            continue;
        }
        let width = vertical_road_width(x);
        if x == 0.0 {
            spawn_road(commands, meshes, materials, x, -345.0, width, 410.0);
            spawn_road(commands, meshes, materials, x, 375.0, width, 550.0);
        } else {
            spawn_road(
                commands,
                meshes,
                materials,
                x,
                city_center_z,
                width,
                CITY_MAX_Z - CITY_MIN_Z,
            );
        }
    }

    for z in Z_ROADS {
        let width = horizontal_road_width(z);
        if z == -20.0 {
            spawn_horizontal_road(commands, meshes, materials, -305.0, z, 430.0, width);
            spawn_horizontal_road(commands, meshes, materials, 305.0, z, 430.0, width);
        } else {
            spawn_horizontal_road(
                commands,
                meshes,
                materials,
                0.0,
                z,
                CITY_MAX_X - CITY_MIN_X,
                width,
            );
        }
    }

    build_canal(commands, meshes, materials, collision_world);
}

fn vertical_road_width(x: f32) -> f32 {
    if x == 0.0 {
        CEREMONIAL_WIDTH
    } else if x.abs() == 360.0 {
        AVENUE_WIDTH
    } else if x == CANAL_X {
        20.0
    } else {
        NARROW_STREET_WIDTH
    }
}

fn horizontal_road_width(z: f32) -> f32 {
    if z == 100.0 {
        CEREMONIAL_WIDTH
    } else if z == -140.0 || z == 340.0 {
        AVENUE_WIDTH
    } else {
        NARROW_STREET_WIDTH
    }
}

fn spawn_road(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    x: f32,
    z: f32,
    width: f32,
    length: f32,
) {
    spawn_box(
        commands,
        meshes,
        &materials.road,
        Vec3::new(x, 0.018, z),
        Vec3::new(width, 0.036, length),
    );
    spawn_box(
        commands,
        meshes,
        &materials.marble,
        Vec3::new(x, 0.041, z),
        Vec3::new(0.22, 0.025, length),
    );
}

fn spawn_horizontal_road(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    x: f32,
    z: f32,
    length: f32,
    width: f32,
) {
    let material = if z == 100.0 {
        &materials.cathedral_cross_street
    } else {
        &materials.road
    };
    spawn_box(
        commands,
        meshes,
        material,
        Vec3::new(x, 0.02, z),
        Vec3::new(length, 0.04, width),
    );
}

fn build_canal(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    spawn_box(
        commands,
        meshes,
        &materials.water,
        Vec3::new(CANAL_X, 0.055, 35.0),
        Vec3::new(16.0, 0.08, 1080.0),
    );

    for pair in Z_ROADS.windows(2) {
        let start = pair[0] + horizontal_road_width(pair[0]) * 0.62;
        let end = pair[1] - horizontal_road_width(pair[1]) * 0.62;
        let center = (start + end) * 0.5;
        let length = end - start;
        for bank_x in [CANAL_X - 9.0, CANAL_X + 9.0] {
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(bank_x, 0.52, center),
                Vec3::new(0.75, 1.04, length),
            );
            collision_world.add_box(
                Vec3::new(bank_x - 0.38, 0.0, start),
                Vec3::new(bank_x + 0.38, 1.05, end),
            );
        }
    }

    for z in Z_ROADS {
        let bridge_width = horizontal_road_width(z);
        spawn_box(
            commands,
            meshes,
            &materials.marble,
            Vec3::new(CANAL_X, 0.11, z),
            Vec3::new(30.0, 0.20, bridge_width + 2.0),
        );
        for rail_z in [z - bridge_width * 0.48, z + bridge_width * 0.48] {
            spawn_box(
                commands,
                meshes,
                &materials.bronze,
                Vec3::new(CANAL_X, 0.62, rail_z),
                Vec3::new(30.0, 0.12, 0.12),
            );
        }
    }
}

fn build_city_blocks(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    for (x_index, x_pair) in X_ROADS.windows(2).enumerate() {
        for (z_index, z_pair) in Z_ROADS.windows(2).enumerate() {
            let center = Vec2::new((x_pair[0] + x_pair[1]) * 0.5, (z_pair[0] + z_pair[1]) * 0.5);
            if is_reserved_block(center) || is_town_square(center) {
                continue;
            }

            let min_x = x_pair[0]
                + vertical_road_width(x_pair[0]) * 0.5
                + 0.5
                + facade_jitter(x_index as i32, z_index as i32, 1);
            let max_x = x_pair[1] - vertical_road_width(x_pair[1]) * 0.5 - 0.5
                + facade_jitter(x_index as i32, z_index as i32, 2);
            let min_z = z_pair[0]
                + horizontal_road_width(z_pair[0]) * 0.5
                + 0.5
                + facade_jitter(x_index as i32, z_index as i32, 3);
            let max_z = z_pair[1] - horizontal_road_width(z_pair[1]) * 0.5 - 0.5
                + facade_jitter(x_index as i32, z_index as i32, 4);
            spawn_city_block(
                commands,
                meshes,
                materials,
                collision_world,
                Vec2::new(min_x, min_z),
                Vec2::new(max_x, max_z),
                x_index as i32,
                z_index as i32,
            );
        }
    }
}

fn is_reserved_block(center: Vec2) -> bool {
    center.x.abs() < 130.0 && center.y > -160.0 && center.y < 235.0
}

fn square_centers() -> [Vec2; 5] {
    [
        Vec2::new(-300.0, 160.0),
        Vec2::new(300.0, 280.0),
        Vec2::new(-300.0, -320.0),
        Vec2::new(300.0, -200.0),
        Vec2::new(-180.0, 520.0),
    ]
}

fn is_town_square(center: Vec2) -> bool {
    square_centers()
        .into_iter()
        .any(|square| center.distance_squared(square) < 1.0)
}

#[allow(clippy::too_many_arguments)]
fn spawn_city_block(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    min: Vec2,
    max: Vec2,
    block_x: i32,
    block_z: i32,
) {
    let center = (min + max) * 0.5;
    let block_seed = city_hash(block_x, block_z, 0x9e37);
    let width = max.x - min.x;
    let depth = max.y - min.y;
    let row_depth = (depth - LANE_WIDTH * 2.0) / 3.0;
    let max_shift = (width * 0.5 - LANE_WIDTH * 0.5 - 13.0).clamp(8.0, 18.0);
    let bend = (10.0 + (block_seed % 700) as f32 / 100.0).min(max_shift);
    let offsets = dogleg_offsets(block_seed, bend);
    let gap_centers = offsets.map(|offset| center.x + offset);

    // Lateral lanes join the side streets. The north-south route changes its
    // alignment at each one, forcing a dogleg through the block.
    for boundary in 1..=2 {
        let z = min.y + boundary as f32 * row_depth + (boundary as f32 - 0.5) * LANE_WIDTH;
        spawn_box(
            commands,
            meshes,
            &materials.road,
            Vec3::new(center.x, 0.045, z),
            Vec3::new(width, 0.055, LANE_WIDTH),
        );
    }

    for (row, gap_center) in gap_centers.into_iter().enumerate() {
        let row_min_z = min.y + row as f32 * (row_depth + LANE_WIDTH);
        let row_max_z = row_min_z + row_depth;
        let row_center_z = (row_min_z + row_max_z) * 0.5;
        let row_seed = city_hash(block_x * 31 + row as i32, block_z * 37, 0x71a9);

        spawn_box(
            commands,
            meshes,
            &materials.road,
            Vec3::new(gap_center, 0.048, row_center_z),
            Vec3::new(LANE_WIDTH, 0.06, row_depth + 0.18),
        );

        let parcels = [
            (min.x, gap_center - LANE_WIDTH * 0.5, -1.0),
            (gap_center + LANE_WIDTH * 0.5, max.x, 1.0),
        ];
        for (side_index, (parcel_min_x, parcel_max_x, street_x)) in parcels.into_iter().enumerate()
        {
            let street_z = if row == 0 {
                -1.0
            } else if row == 2 {
                1.0
            } else if row_seed.is_multiple_of(2) {
                -1.0
            } else {
                1.0
            };

            // Medieval blocks read as a collection of narrow, independently
            // altered houses, not a single modern megastructure. Divide each
            // parcel into two or three deep town-house plots, with only the
            // outermost unit opening onto the vertical perimeter road.
            let parcel_width = parcel_max_x - parcel_min_x;
            let unit_count = if parcel_width > 47.0 { 3 } else { 2 };
            let unit_span = parcel_width / unit_count as f32;
            for unit in 0..unit_count {
                let building_center = Vec2::new(
                    parcel_min_x + unit_span * (unit as f32 + 0.5),
                    (row_min_z + row_max_z) * 0.5,
                );
                let footprint = Vec2::new(unit_span - 0.28, row_max_z - row_min_z - 0.14);
                let seed = city_hash(
                    block_x * 211 + row as i32 * 17 + unit,
                    block_z * 223 + side_index as i32 * 19,
                    0xc17d,
                );
                let core_bonus = (1.0 - building_center.length() / 760.0).max(0.0) * 9.0;
                let height = 13.0 + (seed % 1_600) as f32 / 100.0 + core_bonus;
                let opens_to_x_street =
                    (street_x < 0.0 && unit == 0) || (street_x > 0.0 && unit + 1 == unit_count);

                spawn_building(
                    commands,
                    meshes,
                    materials,
                    collision_world,
                    building_center,
                    footprint,
                    height,
                    if opens_to_x_street { street_x } else { 0.0 },
                    street_z,
                    seed,
                );
            }
        }

        if !row_seed.is_multiple_of(3) {
            spawn_box(
                commands,
                meshes,
                if row_seed.is_multiple_of(2) {
                    &materials.half_timber
                } else {
                    &materials.pale_stone
                },
                Vec3::new(gap_center, 6.2, row_center_z),
                Vec3::new(LANE_WIDTH + 1.25, 2.8, 6.2),
            );
        }
    }

    for boundary in 0..2 {
        let z = min.y + (boundary + 1) as f32 * row_depth + (boundary as f32 + 0.5) * LANE_WIDTH;
        let x = (gap_centers[boundary] + gap_centers[boundary + 1]) * 0.5;
        let bridge_seed = city_hash(block_x, block_z + boundary as i32, 0xb41d);
        if !bridge_seed.is_multiple_of(3) {
            spawn_box(
                commands,
                meshes,
                if bridge_seed.is_multiple_of(2) {
                    &materials.wood
                } else {
                    &materials.half_timber
                },
                Vec3::new(x, 6.0, z),
                Vec3::new(6.0, 2.7, LANE_WIDTH + 1.2),
            );
        }
    }

    let court_boundary = (block_seed as usize) % 2;
    let court_z = min.y
        + (court_boundary + 1) as f32 * row_depth
        + (court_boundary as f32 + 0.5) * LANE_WIDTH;
    let court_x = (gap_centers[court_boundary] + gap_centers[court_boundary + 1]) * 0.5;
    let lamp_side = if block_seed.is_multiple_of(2) {
        -1.0
    } else {
        1.0
    };
    spawn_lamp(
        commands,
        meshes,
        materials,
        Vec3::new(court_x + lamp_side * 1.45, 0.0, court_z),
        false,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_building(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    center: Vec2,
    footprint: Vec2,
    height: f32,
    street_x: f32,
    street_z: f32,
    seed: u32,
) {
    let facade = facade_material(materials, seed);
    let roof = if seed.is_multiple_of(4) {
        &materials.slate
    } else {
        &materials.terracotta
    };
    let windows = if seed.is_multiple_of(9) {
        &materials.window_warm
    } else {
        &materials.window_blue
    };

    spawn_box(
        commands,
        meshes,
        facade,
        Vec3::new(center.x, height * 0.5, center.y),
        Vec3::new(footprint.x, height, footprint.y),
    );
    if !seed.is_multiple_of(4) {
        let upper_height = (height - 8.5).clamp(4.5, height * 0.52);
        spawn_box(
            commands,
            meshes,
            facade,
            Vec3::new(center.x, height - upper_height * 0.5, center.y),
            Vec3::new(footprint.x + 0.65, upper_height, footprint.y + 0.65),
        );
    }
    spawn_box(
        commands,
        meshes,
        &materials.umber,
        Vec3::new(center.x, 0.34, center.y),
        Vec3::new(footprint.x + 0.28, 0.68, footprint.y + 0.28),
    );
    spawn_box(
        commands,
        meshes,
        &materials.pale_stone,
        Vec3::new(center.x, height - 0.38, center.y),
        Vec3::new(footprint.x + 0.52, 0.42, footprint.y + 0.52),
    );
    spawn_box(
        commands,
        meshes,
        roof,
        Vec3::new(center.x, height + 0.04, center.y),
        Vec3::new(footprint.x + 0.9, 0.28, footprint.y + 0.9),
    );
    let has_pitched_roof = !seed.is_multiple_of(6);
    if has_pitched_roof {
        spawn_mesh(
            commands,
            &meshes.roof,
            roof,
            Transform::from_xyz(center.x, height + 2.05, center.y)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                .with_scale(Vec3::new(footprint.x * 0.72, 3.9, footprint.y * 0.72)),
        );
    } else {
        // Low parapets give the occasional flat-roofed Maltese house a proper
        // silhouette instead of leaving a bare cuboid top.
        for side in [-1.0, 1.0] {
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(center.x + side * footprint.x * 0.5, height + 0.58, center.y),
                Vec3::new(0.34, 1.15, footprint.y + 0.75),
            );
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(center.x, height + 0.58, center.y + side * footprint.y * 0.5),
                Vec3::new(footprint.x + 0.75, 1.15, 0.34),
            );
        }
    }

    let floor_count = (((height - 4.0) / 4.0) as usize).clamp(2, 7);
    for floor in 0..floor_count {
        let y = 3.4 + floor as f32 * 3.7;
        if street_x != 0.0 {
            spawn_window_row_x(
                commands,
                meshes,
                materials,
                windows,
                center,
                footprint,
                y,
                street_x,
                seed.wrapping_add(floor as u32 * 41),
            );
        }
        if street_z != 0.0 {
            spawn_window_row_z(
                commands,
                meshes,
                materials,
                windows,
                center,
                footprint,
                y,
                street_z,
                seed.wrapping_add(floor as u32 * 53),
            );
        }
    }

    spawn_door(
        commands, meshes, materials, center, footprint, street_x, street_z, seed,
    );

    if !seed.is_multiple_of(3) {
        let chimney_x =
            center.x + ((seed >> 5) % 5) as f32 / 5.0 * footprint.x * 0.5 - footprint.x * 0.25;
        let chimney_z =
            center.y + ((seed >> 9) % 5) as f32 / 5.0 * footprint.y * 0.42 - footprint.y * 0.21;
        spawn_box(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(chimney_x, height + 2.35, chimney_z),
            Vec3::new(0.75, 3.4, 0.75),
        );
    }

    collision_world.add_box(
        Vec3::new(
            center.x - footprint.x * 0.5,
            0.0,
            center.y - footprint.y * 0.5,
        ),
        Vec3::new(
            center.x + footprint.x * 0.5,
            height + 0.6,
            center.y + footprint.y * 0.5,
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_window_row_x(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    glass: &Handle<StandardMaterial>,
    center: Vec2,
    footprint: Vec2,
    y: f32,
    side: f32,
    seed: u32,
) {
    let count = (((footprint.y - 2.2) / 4.0).floor() as usize).clamp(1, 5);
    let x = center.x + side * (footprint.x * 0.5 + 0.075);
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    spawn_mesh(
        commands,
        &meshes.window_surround_rows[count - 1],
        &materials.pale_stone,
        Transform::from_xyz(x, y, center.y).with_rotation(rotation),
    );
    spawn_mesh(
        commands,
        &meshes.window_glass_rows[count - 1],
        glass,
        Transform::from_xyz(x + side * 0.065, y, center.y).with_rotation(rotation),
    );

    if seed.is_multiple_of(3) {
        let index = seed as usize % count;
        let z = center.y - (index as f32 - (count - 1) as f32 * 0.5) * 4.0;
        for shutter_side in [-1.0, 1.0] {
            spawn_box(
                commands,
                meshes,
                &materials.wood,
                Vec3::new(x + side * 0.075, y, z + shutter_side * 0.76),
                Vec3::new(0.09, 1.65, 0.28),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_window_row_z(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    glass: &Handle<StandardMaterial>,
    center: Vec2,
    footprint: Vec2,
    y: f32,
    side: f32,
    seed: u32,
) {
    let count = (((footprint.x - 2.2) / 3.4).floor() as usize).clamp(1, 5);
    let z = center.y + side * (footprint.y * 0.5 + 0.075);
    spawn_mesh(
        commands,
        &meshes.window_surround_rows[count - 1],
        &materials.pale_stone,
        Transform::from_xyz(center.x, y, z),
    );
    spawn_mesh(
        commands,
        &meshes.window_glass_rows[count - 1],
        glass,
        Transform::from_xyz(center.x, y, z + side * 0.065),
    );

    if seed.is_multiple_of(3) {
        let index = seed as usize % count;
        let x = center.x + (index as f32 - (count - 1) as f32 * 0.5) * 4.0;
        for shutter_side in [-1.0, 1.0] {
            spawn_box(
                commands,
                meshes,
                &materials.wood,
                Vec3::new(x + shutter_side * 0.76, y, z + side * 0.075),
                Vec3::new(0.28, 1.65, 0.09),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_door(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    center: Vec2,
    footprint: Vec2,
    street_x: f32,
    street_z: f32,
    seed: u32,
) {
    let offset = ((seed % 3) as f32 - 1.0) * 1.8;
    if street_z != 0.0 {
        let x = (center.x + offset).clamp(
            center.x - footprint.x * 0.5 + 1.5,
            center.x + footprint.x * 0.5 - 1.5,
        );
        let z = center.y + street_z * (footprint.y * 0.5 + 0.09);
        spawn_box(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(x, 1.65, z),
            Vec3::new(2.25, 3.3, 0.16),
        );
        spawn_box(
            commands,
            meshes,
            &materials.wood,
            Vec3::new(x, 1.48, z + street_z * 0.095),
            Vec3::new(1.55, 2.95, 0.12),
        );
    } else if street_x != 0.0 {
        let z = (center.y + offset).clamp(
            center.y - footprint.y * 0.5 + 1.5,
            center.y + footprint.y * 0.5 - 1.5,
        );
        let x = center.x + street_x * (footprint.x * 0.5 + 0.09);
        spawn_box(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(x, 1.65, z),
            Vec3::new(0.16, 3.3, 2.25),
        );
        spawn_box(
            commands,
            meshes,
            &materials.wood,
            Vec3::new(x + street_x * 0.095, 1.48, z),
            Vec3::new(0.12, 2.95, 1.55),
        );
    }
}

fn facade_material(materials: &CityMaterials, seed: u32) -> &Handle<StandardMaterial> {
    match seed % 10 {
        0..=3 => &materials.limestone,
        4..=6 => &materials.pale_stone,
        7..=8 => &materials.ochre,
        _ => &materials.rose_plaster,
    }
}

fn build_dense_cathedral_quarter(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    // Two staggered rows hug each side of the cathedral. They form a narrow
    // service lane beside the buttresses and a second, still tighter back alley.
    let inner_z = [-93.0, -72.0, -51.0, 8.0, 29.0, 50.0, 71.0];
    let outer_z = [-82.0, -61.0, 18.0, 39.0, 60.0];
    for side in [-1.0, 1.0] {
        for (index, z) in inner_z.into_iter().enumerate() {
            let seed = city_hash(side as i32 * 41, index as i32, 0xc411);
            let bend = [-1.4, 0.5, 1.5, -0.6][index % 4];
            spawn_building(
                commands,
                meshes,
                materials,
                collision_world,
                Vec2::new(side * (62.0 + bend), z),
                Vec2::new(24.0, 17.0),
                18.0 + (seed % 900) as f32 / 100.0,
                -side,
                if index.is_multiple_of(2) { -1.0 } else { 1.0 },
                seed,
            );
            if !index.is_multiple_of(3) {
                spawn_box(
                    commands,
                    meshes,
                    if index.is_multiple_of(2) {
                        &materials.wood
                    } else {
                        &materials.pale_stone
                    },
                    Vec3::new(side * (47.0 + bend * 0.5), 6.4, z),
                    Vec3::new(6.5, 2.8, 5.0),
                );
            }
        }
        for (index, z) in outer_z.into_iter().enumerate() {
            let seed = city_hash(side as i32 * 67, index as i32, 0x5a17);
            let bend = [-1.0, 1.3, 0.2, -1.4, 0.8][index % 5];
            spawn_building(
                commands,
                meshes,
                materials,
                collision_world,
                Vec2::new(side * (90.0 + bend), z),
                Vec2::new(24.0, 17.0),
                16.0 + (seed % 1100) as f32 / 100.0,
                -side,
                if index.is_multiple_of(2) { 1.0 } else { -1.0 },
                seed,
            );
            spawn_box(
                commands,
                meshes,
                if index.is_multiple_of(2) {
                    &materials.umber
                } else {
                    &materials.pale_stone
                },
                Vec3::new(side * (76.0 + bend * 0.5), 6.0, z),
                Vec3::new(5.2, 3.0, 5.5),
            );
        }

        // Merchant houses frame the forecourt without turning it into another
        // empty field. The three-metre arcade behind the columns remains open.
        for (index, z) in [117.0, 142.0].into_iter().enumerate() {
            let seed = city_hash(side as i32 * 83, index as i32, 0xf041);
            let bend = [-1.2, 0.8, 1.3, -0.5][index];
            spawn_building(
                commands,
                meshes,
                materials,
                collision_world,
                Vec2::new(side * (51.0 + bend), z),
                Vec2::new(22.0, 19.0),
                17.0 + (seed % 700) as f32 / 100.0,
                -side,
                -1.0,
                seed,
            );
        }
    }

    // A crooked row behind the apse creates a close, dramatic reveal of the
    // dome when emerging from these northern lanes.
    for (index, x) in [-72.0, -48.0, -24.0, 0.0, 24.0, 48.0, 72.0]
        .into_iter()
        .enumerate()
    {
        let seed = city_hash(index as i32, -101, 0xa95e);
        spawn_building(
            commands,
            meshes,
            materials,
            collision_world,
            Vec2::new(x, -120.0 - (index % 2) as f32 * 1.5),
            Vec2::new(20.0, 17.0),
            16.0 + (seed % 1000) as f32 / 100.0,
            if x < 0.0 { 1.0 } else { -1.0 },
            1.0,
            seed,
        );
    }
}

fn build_grand_forecourt(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    spawn_box(
        commands,
        meshes,
        &materials.forecourt_paving,
        Vec3::new(0.0, 0.045, GRAND_FORECOURT_CENTER_Z),
        Vec3::new(GRAND_FORECOURT_WIDTH, 0.08, GRAND_FORECOURT_DEPTH),
    );
    spawn_box(
        commands,
        meshes,
        &materials.cathedral_approach,
        Vec3::new(0.0, 0.092, GRAND_APPROACH_CENTER_Z),
        Vec3::new(GRAND_APPROACH_WIDTH, 0.05, GRAND_APPROACH_LENGTH),
    );

    for side in [-1.0, 1.0] {
        for z in (111..=153).step_by(6) {
            spawn_cylinder(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(side * 34.0, 4.2, z as f32),
                0.75,
                8.4,
            );
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(side * 34.0, 8.5, z as f32),
                Vec3::new(2.1, 0.45, 2.1),
            );
        }
        spawn_box(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(side * 34.0, 9.0, 132.0),
            Vec3::new(2.0, 0.65, 50.0),
        );
        spawn_fountain(
            commands,
            meshes,
            materials,
            collision_world,
            Vec3::new(side * 17.5, 0.0, 131.5),
            1.15,
        );
    }

    // Flush bands suggest the broad ceremonial stair without impeding movement.
    for z in [84.0, 88.0, 92.0, 96.0] {
        spawn_box(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(0.0, 0.07, z),
            Vec3::new(44.0 + (z - 84.0) * 2.0, 0.08, 1.4),
        );
    }
}

fn build_town_squares(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    for (index, center) in square_centers().into_iter().enumerate() {
        spawn_box(
            commands,
            meshes,
            if index.is_multiple_of(2) {
                &materials.marble
            } else {
                &materials.pale_stone
            },
            Vec3::new(center.x, 0.055, center.y),
            Vec3::new(92.0, 0.09, 92.0),
        );
        spawn_fountain(
            commands,
            meshes,
            materials,
            collision_world,
            Vec3::new(center.x, 0.0, center.y),
            0.9,
        );

        for angle_index in 0_usize..8 {
            let angle = angle_index as f32 * std::f32::consts::TAU / 8.0;
            let position = Vec3::new(
                center.x + angle.cos() * 34.0,
                0.0,
                center.y + angle.sin() * 34.0,
            );
            if angle_index.is_multiple_of(2) {
                spawn_tree(commands, meshes, materials, position, 0.9);
            } else {
                spawn_market_stall(commands, meshes, materials, position, angle_index);
            }
        }
    }
}

fn spawn_fountain(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    center: Vec3,
    scale: f32,
) {
    spawn_cylinder(
        commands,
        meshes,
        &materials.marble,
        center + Vec3::Y * 0.35,
        5.2 * scale,
        0.7,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.water,
        center + Vec3::Y * 0.73,
        4.55 * scale,
        0.08,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.pale_stone,
        center + Vec3::Y * 2.0,
        0.58 * scale,
        3.2,
    );
    spawn_mesh(
        commands,
        &meshes.sphere,
        &materials.gold,
        Transform::from_translation(center + Vec3::Y * 4.0).with_scale(Vec3::splat(0.65 * scale)),
    );
    collision_world.add_box(
        center + Vec3::new(-5.2 * scale, 0.0, -5.2 * scale),
        center + Vec3::new(5.2 * scale, 4.7, 5.2 * scale),
    );
}

fn spawn_market_stall(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    center: Vec3,
    variant: usize,
) {
    spawn_box(
        commands,
        meshes,
        &materials.wood,
        center + Vec3::Y * 0.65,
        Vec3::new(4.5, 1.3, 2.8),
    );
    spawn_box(
        commands,
        meshes,
        if variant.is_multiple_of(3) {
            &materials.rose_plaster
        } else {
            &materials.ochre
        },
        center + Vec3::Y * 2.5,
        Vec3::new(5.2, 0.25, 3.5),
    );
}

fn build_landmarks(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let landmarks = [
        (Vec2::new(-300.0, 128.0), 56.0, 0),
        (Vec2::new(300.0, 248.0), 68.0, 1),
        (Vec2::new(-300.0, -352.0), 52.0, 2),
        (Vec2::new(300.0, -232.0), 61.0, 3),
        (Vec2::new(-180.0, 488.0), 72.0, 4),
    ];
    for (center, height, style) in landmarks {
        spawn_landmark_tower(
            commands,
            meshes,
            materials,
            collision_world,
            center,
            height,
            style,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_landmark_tower(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    center: Vec2,
    height: f32,
    style: usize,
) {
    let body = match style % 3 {
        0 => &materials.pale_stone,
        1 => &materials.ochre,
        _ => &materials.rose_plaster,
    };
    spawn_box(
        commands,
        meshes,
        body,
        Vec3::new(center.x, height * 0.38, center.y),
        Vec3::new(22.0, height * 0.76, 25.0),
    );
    spawn_box(
        commands,
        meshes,
        &materials.pale_stone,
        Vec3::new(center.x, height * 0.73, center.y),
        Vec3::new(25.0, 2.0, 28.0),
    );
    spawn_cylinder(
        commands,
        meshes,
        body,
        Vec3::new(center.x, height * 0.85, center.y),
        8.0,
        height * 0.22,
    );
    spawn_mesh(
        commands,
        &meshes.sphere,
        &materials.slate,
        Transform::from_xyz(center.x, height, center.y).with_scale(Vec3::new(8.5, 5.2, 8.5)),
    );
    spawn_mesh(
        commands,
        &meshes.cone,
        &materials.gold,
        Transform::from_xyz(center.x, height + 9.0, center.y).with_scale(Vec3::new(1.2, 8.0, 1.2)),
    );

    for side in [-1.0, 1.0] {
        spawn_cylinder(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(center.x + side * 7.0, height * 0.38, center.y - 13.5),
            1.0,
            height * 0.65,
        );
    }
    spawn_box(
        commands,
        meshes,
        &materials.wood,
        Vec3::new(center.x, 3.0, center.y - 12.58),
        Vec3::new(5.0, 6.0, 0.18),
    );
    collision_world.add_box(
        Vec3::new(center.x - 11.0, 0.0, center.y - 12.5),
        Vec3::new(center.x + 11.0, height, center.y + 12.5),
    );
}

fn build_cathedral_exterior(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    for side in [-1.0, 1.0] {
        let x = side * 34.0;
        spawn_box(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(x, 25.0, 87.0),
            Vec3::new(13.0, 50.0, 14.0),
        );
        for y in [8.0, 20.0, 32.0, 44.0] {
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(x, y, 87.0),
                Vec3::new(15.0, 1.1, 16.0),
            );
        }
        spawn_cylinder(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(x, 57.0, 87.0),
            6.3,
            14.0,
        );
        spawn_mesh(
            commands,
            &meshes.sphere,
            &materials.slate,
            Transform::from_xyz(x, 66.0, 87.0).with_scale(Vec3::new(6.8, 5.0, 6.8)),
        );
        spawn_mesh(
            commands,
            &meshes.cone,
            &materials.gold,
            Transform::from_xyz(x, 76.0, 87.0).with_scale(Vec3::new(1.0, 9.0, 1.0)),
        );
        collision_world.add_box(
            Vec3::new(x - 6.5, 0.0, 81.0),
            Vec3::new(x + 6.5, 50.0, 94.0),
        );
    }

    for x in [-20.0, -12.0, 12.0, 20.0] {
        spawn_cylinder(
            commands,
            meshes,
            &materials.pale_stone,
            Vec3::new(x, 10.5, 82.2),
            0.9,
            21.0,
        );
        collision_world.add_box(
            Vec3::new(x - 0.9, 0.0, 81.3),
            Vec3::new(x + 0.9, 21.0, 83.1),
        );
    }
    spawn_box(
        commands,
        meshes,
        &materials.pale_stone,
        Vec3::new(0.0, 22.0, 82.0),
        Vec3::new(48.0, 1.2, 3.0),
    );

    // Lantern and beacon complete the central dome from the city skyline.
    spawn_cylinder(
        commands,
        meshes,
        &materials.pale_stone,
        Vec3::new(0.0, 68.0, -23.0),
        3.8,
        10.0,
    );
    spawn_mesh(
        commands,
        &meshes.sphere,
        &materials.gold,
        Transform::from_xyz(0.0, 74.0, -23.0).with_scale(Vec3::new(4.2, 2.7, 4.2)),
    );
    spawn_mesh(
        commands,
        &meshes.cone,
        &materials.gold,
        Transform::from_xyz(0.0, 82.0, -23.0).with_scale(Vec3::new(0.8, 8.0, 0.8)),
    );

    for side in [-1.0, 1.0] {
        for z in [-88.0, -72.0, -56.0, -40.0, 0.0, 16.0, 32.0, 48.0, 64.0] {
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(side * 45.2, 10.0, z),
                Vec3::new(3.0, 20.0, 2.0),
            );
        }
    }
}

fn build_street_furniture(commands: &mut Commands, meshes: &CityMeshes, materials: &CityMaterials) {
    for (x_index, x) in X_ROADS.into_iter().enumerate() {
        for (z_index, z) in Z_ROADS.into_iter().enumerate() {
            if x.abs() < 130.0 && z > -160.0 && z < 235.0 {
                continue;
            }
            let seed = city_hash(x_index as i32, z_index as i32, 0x51f2);
            let x_offset = vertical_road_width(x) * 0.5 + 0.8;
            let z_offset = horizontal_road_width(z) * 0.5 + 0.8;
            let position = Vec3::new(x + x_offset, 0.0, z + z_offset);
            spawn_lamp(
                commands,
                meshes,
                materials,
                position,
                seed.is_multiple_of(11),
            );
            if !seed.is_multiple_of(3) {
                spawn_tree(
                    commands,
                    meshes,
                    materials,
                    Vec3::new(x - x_offset, 0.0, z - z_offset),
                    0.72 + (seed % 20) as f32 * 0.01,
                );
            }
        }
    }
}

fn spawn_lamp(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    foot: Vec3,
    real_light: bool,
) {
    spawn_cylinder(
        commands,
        meshes,
        &materials.bronze,
        foot + Vec3::Y * 2.6,
        0.09,
        5.2,
    );
    spawn_mesh(
        commands,
        &meshes.sphere,
        &materials.lamp,
        Transform::from_translation(foot + Vec3::Y * 5.35).with_scale(Vec3::splat(0.28)),
    );
    if real_light {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.52, 0.18),
                intensity: 12_000.0,
                range: 15.0,
                radius: 0.2,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(foot + Vec3::Y * 5.25),
        ));
    }
}

fn spawn_tree(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    foot: Vec3,
    scale: f32,
) {
    spawn_cylinder(
        commands,
        meshes,
        &materials.wood,
        foot + Vec3::Y * 1.8 * scale,
        0.28 * scale,
        3.6 * scale,
    );
    for offset in [
        Vec3::ZERO,
        Vec3::new(0.7, -0.25, 0.2),
        Vec3::new(-0.55, -0.2, -0.45),
    ] {
        spawn_mesh(
            commands,
            &meshes.sphere,
            &materials.leaf,
            Transform::from_translation(foot + Vec3::Y * 4.4 * scale + offset * scale)
                .with_scale(Vec3::new(1.8, 2.2, 1.8) * scale),
        );
    }
}

fn build_city_walls(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center_z = (CITY_MIN_Z + CITY_MAX_Z) * 0.5;
    for x in [CITY_MIN_X, CITY_MAX_X] {
        spawn_box(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(x, 7.0, center_z),
            Vec3::new(4.0, 14.0, CITY_MAX_Z - CITY_MIN_Z),
        );
        collision_world.add_box(
            Vec3::new(x - 2.0, 0.0, CITY_MIN_Z),
            Vec3::new(x + 2.0, 20.0, CITY_MAX_Z),
        );
    }
    for z in [CITY_MIN_Z, CITY_MAX_Z] {
        spawn_box(
            commands,
            meshes,
            &materials.limestone,
            Vec3::new(0.0, 7.0, z),
            Vec3::new(CITY_MAX_X - CITY_MIN_X, 14.0, 4.0),
        );
        collision_world.add_box(
            Vec3::new(CITY_MIN_X, 0.0, z - 2.0),
            Vec3::new(CITY_MAX_X, 20.0, z + 2.0),
        );
    }

    for x in (-500..=500).step_by(20) {
        for z in [CITY_MIN_Z, CITY_MAX_Z] {
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(x as f32, 15.2, z),
                Vec3::new(6.0, 2.4, 6.0),
            );
        }
    }
    for z in (-530..=630).step_by(20) {
        for x in [CITY_MIN_X, CITY_MAX_X] {
            spawn_box(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(x, 15.2, z as f32),
                Vec3::new(6.0, 2.4, 6.0),
            );
        }
    }

    for x in [CITY_MIN_X, CITY_MAX_X] {
        for z in [CITY_MIN_Z, CITY_MAX_Z] {
            spawn_cylinder(
                commands,
                meshes,
                &materials.pale_stone,
                Vec3::new(x, 12.0, z),
                9.0,
                24.0,
            );
            spawn_mesh(
                commands,
                &meshes.cone,
                &materials.slate,
                Transform::from_xyz(x, 29.0, z).with_scale(Vec3::new(10.0, 11.0, 10.0)),
            );
        }
    }
}

fn city_hash(x: i32, z: i32, salt: u32) -> u32 {
    let mut value =
        (x as u32).wrapping_mul(0x85eb_ca6b) ^ (z as u32).wrapping_mul(0xc2b2_ae35) ^ salt;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value.wrapping_mul(0x846c_a68b) ^ (value >> 16)
}

fn facade_jitter(block_x: i32, block_z: i32, edge: u32) -> f32 {
    let normalized = (city_hash(block_x, block_z, 0xface + edge) % 1001) as f32 / 1000.0;
    (normalized * 2.0 - 1.0) * 1.5
}

fn dogleg_offsets(seed: u32, bend: f32) -> [f32; 3] {
    match seed % 4 {
        0 => [-bend, bend * 0.25, bend],
        1 => [bend, -bend * 0.4, -bend],
        2 => [-bend * 0.2, bend, -bend * 0.7],
        _ => [bend * 0.7, -bend, bend * 0.15],
    }
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &CityMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
) {
    spawn_mesh(
        commands,
        &meshes.cube,
        material,
        Transform::from_translation(center).with_scale(size),
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
    spawn_mesh(
        commands,
        &meshes.cylinder,
        material,
        Transform::from_translation(center).with_scale(Vec3::new(radius, height, radius)),
    );
}

fn spawn_mesh(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    transform: Transform,
) {
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        transform,
    ));
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;

    #[test]
    fn city_builds_headlessly_at_large_scale() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_city);

        app.update();

        let world = app.world_mut();
        let mesh_entity_count = world
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(world)
            .count();
        assert!(
            mesh_entity_count > 2_000,
            "expected a dense city, got {mesh_entity_count} mesh entities"
        );
        assert!(
            mesh_entity_count < 40_000,
            "façade details should remain batched, got {mesh_entity_count} mesh entities"
        );
        assert!(world.resource::<CollisionWorld>().len() > 250);
    }

    #[test]
    fn cathedral_core_blocks_remain_open() {
        for x in [-60.0, 60.0] {
            for z in [-80.0, 40.0, 160.0] {
                assert!(is_reserved_block(Vec2::new(x, z)));
            }
        }
    }

    #[test]
    fn grand_forecourt_is_compact_and_still_meets_the_cathedral_approach() {
        assert_eq!(GRAND_FORECOURT_WIDTH, 77.0);
        assert_eq!(GRAND_FORECOURT_DEPTH, 52.0);

        let forecourt_near_edge = GRAND_FORECOURT_CENTER_Z - GRAND_FORECOURT_DEPTH * 0.5;
        let forecourt_far_edge = GRAND_FORECOURT_CENTER_Z + GRAND_FORECOURT_DEPTH * 0.5;
        let approach_near_edge = GRAND_APPROACH_CENTER_Z - GRAND_APPROACH_LENGTH * 0.5;

        assert_eq!(forecourt_near_edge, 105.0);
        assert_eq!(approach_near_edge - forecourt_far_edge, 6.0);
    }

    #[test]
    fn backstreets_are_narrow_and_doglegged() {
        assert!((LANE_WIDTH - 4.6).abs() < f32::EPSILON);
        assert_eq!(NARROW_STREET_WIDTH, 7.0);

        for pattern in 0..4 {
            let offsets = dogleg_offsets(pattern, 10.0);
            assert!((offsets[1] - offsets[0]).abs() >= 6.0);
            assert!((offsets[2] - offsets[1]).abs() >= 6.0);
        }
    }

    #[test]
    fn facade_offsets_create_street_pinch_points() {
        for block_x in 0..8 {
            for block_z in 0..9 {
                for edge in 1..=4 {
                    assert!(facade_jitter(block_x, block_z, edge).abs() <= 1.5);
                }
            }
        }
    }
}
