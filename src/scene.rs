//! Procedural architecture for the cathedral interior.
//!
//! Bevy units are metres. The west entrance is at positive Z and the high altar
//! is at negative Z, so the controller can spawn at Z=60 looking down -Z.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::{
    asset::RenderAssetUsages,
    light::{
        Atmosphere, CascadeShadowConfigBuilder, atmosphere::ScatteringMedium, light_consts::lux,
    },
    mesh::{Indices, MeshBuilder, PrimitiveTopology},
    prelude::*,
};

use crate::{
    controller::CollisionWorld,
    materials::{FLOOR_TEXTURE_SPAN_METERS, load_repeating_texture},
};

pub struct CathedralPlugin;

impl Plugin for CathedralPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (build_cathedral, build_daylight_atmosphere))
            .add_systems(Update, add_fog_to_new_cameras);
    }
}

/// Adds a physical Earth atmosphere. Besides drawing the sky, this lets the
/// camera's generated environment map provide soft skylight in shadowed lanes.
fn build_daylight_atmosphere(
    mut commands: Commands,
    mut scattering_media: ResMut<Assets<ScatteringMedium>>,
) {
    let earth = scattering_media.add(ScatteringMedium::earth(128, 128));
    commands.spawn((
        Name::new("Mediterranean daylight atmosphere"),
        Atmosphere::earth(earth),
    ));
}

#[derive(Clone)]
struct CathedralMeshes {
    cube: Handle<Mesh>,
    floor: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    sphere: Handle<Mesh>,
    pane: Handle<Mesh>,
    rose_disc: Handle<Mesh>,
    nave_arch: Handle<Mesh>,
    arcade_arch: Handle<Mesh>,
    chapel_arch: Handle<Mesh>,
    crossing_arch: Handle<Mesh>,
    rose_ring: Handle<Mesh>,
    drum_ring: Handle<Mesh>,
    dome: Handle<Mesh>,
}

#[derive(Clone)]
struct CathedralMaterials {
    floor: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
    pale_stone: Handle<StandardMaterial>,
    dark_stone: Handle<StandardMaterial>,
    marble_light: Handle<StandardMaterial>,
    marble_dark: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    bronze: Handle<StandardMaterial>,
    rose: Handle<StandardMaterial>,
    blue_glass: Handle<StandardMaterial>,
    amber_glass: Handle<StandardMaterial>,
    red_glass: Handle<StandardMaterial>,
    candle: Handle<StandardMaterial>,
    apse_mosaic: Handle<StandardMaterial>,
}

fn build_cathedral(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut collision_world: ResMut<CollisionWorld>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.58, 0.66, 0.72),
        brightness: 300.0,
        ..default()
    });

    let mesh = create_meshes(&mut meshes);
    let material = create_materials(&asset_server, &mut materials);

    build_floor(&mut commands, &mesh, &material, &mut collision_world);
    build_outer_shell(&mut commands, &mesh, &material, &mut collision_world);
    build_nave(&mut commands, &mesh, &material, &mut collision_world);
    build_side_aisles_and_chapels(&mut commands, &mesh, &material, &mut collision_world);
    build_crossing_and_dome(&mut commands, &mesh, &material, &mut collision_world);
    build_apse_and_altar(&mut commands, &mesh, &material, &mut collision_world);
    build_west_end(&mut commands, &mesh, &material, &mut collision_world);
    build_lighting(&mut commands, &mesh, &material);
}

fn create_meshes(meshes: &mut Assets<Mesh>) -> CathedralMeshes {
    let half_arch = |inner: f32, outer: f32, major_resolution: usize| {
        Torus::new(inner, outer)
            .mesh()
            .major_resolution(major_resolution)
            .minor_resolution(8)
            .angle_range(0.0..=PI)
            .build()
    };

    CathedralMeshes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        floor: meshes.add(cathedral_floor_mesh()),
        cylinder: meshes.add(Cylinder::new(1.0, 1.0)),
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(24, 16)),
        pane: meshes.add(Rectangle::new(1.0, 1.0)),
        rose_disc: meshes.add(Circle::new(1.0).mesh().resolution(64)),
        nave_arch: meshes.add(half_arch(11.45, 12.75, 32)),
        arcade_arch: meshes.add(half_arch(6.65, 7.75, 24)),
        chapel_arch: meshes.add(half_arch(3.35, 4.15, 20)),
        crossing_arch: meshes.add(half_arch(13.7, 15.25, 32)),
        rose_ring: meshes.add(Torus::new(6.8, 7.55).mesh().major_resolution(48).build()),
        drum_ring: meshes.add(
            Torus::new(21.9, 23.0)
                .mesh()
                .major_resolution(64)
                .minor_resolution(8)
                .build(),
        ),
        dome: meshes.add(hemisphere_mesh(22.5, 25.0, 64, 20)),
    }
}

fn create_materials(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> CathedralMaterials {
    let limestone = asset_server.load("textures/limestone.png");
    let floor_texture = load_repeating_texture(asset_server, "textures/cathedral_floor.png");
    let rose_texture = asset_server.load("textures/rose_window.png");

    CathedralMaterials {
        floor: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(floor_texture),
            perceptual_roughness: 0.62,
            reflectance: 0.46,
            ..default()
        }),
        stone: materials.add(StandardMaterial {
            base_color: Color::srgb(0.83, 0.75, 0.61),
            base_color_texture: Some(limestone.clone()),
            perceptual_roughness: 0.76,
            reflectance: 0.32,
            ..default()
        }),
        pale_stone: materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.79, 0.64),
            base_color_texture: Some(limestone),
            perceptual_roughness: 0.67,
            reflectance: 0.38,
            ..default()
        }),
        dark_stone: materials.add(StandardMaterial {
            base_color: Color::srgb(0.19, 0.17, 0.16),
            perceptual_roughness: 0.84,
            // Also used on the dome shell, which must be visible from below.
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        marble_light: materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.69, 0.63),
            perceptual_roughness: 0.29,
            reflectance: 0.62,
            ..default()
        }),
        marble_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.075, 0.085, 0.095),
            perceptual_roughness: 0.24,
            reflectance: 0.7,
            ..default()
        }),
        wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.055, 0.024),
            perceptual_roughness: 0.43,
            reflectance: 0.28,
            ..default()
        }),
        gold: materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.47, 0.10),
            metallic: 0.92,
            perceptual_roughness: 0.25,
            reflectance: 0.78,
            ..default()
        }),
        bronze: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.095, 0.045),
            metallic: 0.86,
            perceptual_roughness: 0.42,
            ..default()
        }),
        rose: materials.add(StandardMaterial {
            base_color_texture: Some(rose_texture.clone()),
            emissive_texture: Some(rose_texture),
            emissive: LinearRgba::rgb(2.8, 2.4, 2.0),
            emissive_exposure_weight: 0.35,
            perceptual_roughness: 0.24,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        blue_glass: glass_material(
            materials,
            Color::srgba(0.09, 0.28, 0.58, 0.84),
            LinearRgba::rgb(0.15, 0.55, 2.4),
        ),
        amber_glass: glass_material(
            materials,
            Color::srgba(0.76, 0.30, 0.055, 0.84),
            LinearRgba::rgb(2.5, 0.72, 0.11),
        ),
        red_glass: glass_material(
            materials,
            Color::srgba(0.48, 0.035, 0.045, 0.84),
            LinearRgba::rgb(2.3, 0.13, 0.10),
        ),
        candle: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.55, 0.15),
            emissive: LinearRgba::rgb(15.0, 4.0, 0.45),
            perceptual_roughness: 0.2,
            ..default()
        }),
        apse_mosaic: materials.add(StandardMaterial {
            base_color: Color::srgb(0.075, 0.18, 0.28),
            emissive: LinearRgba::rgb(0.03, 0.12, 0.23),
            metallic: 0.16,
            perceptual_roughness: 0.36,
            ..default()
        }),
    }
}

fn glass_material(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    emissive: LinearRgba,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        emissive,
        emissive_exposure_weight: 0.35,
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.18,
        reflectance: 0.66,
        double_sided: true,
        cull_mode: None,
        ..default()
    })
}

fn build_floor(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    // Solid backing prevents light leaks beneath a single, world-UV-mapped top
    // surface. The transept backing is split into wings so no coplanar faces
    // overlap and shimmer below the textured surface.
    spawn_box(
        commands,
        mesh,
        &material.marble_light,
        Vec3::new(0.0, -0.36, -10.0),
        Vec3::new(90.0, 0.7, 180.0),
        Quat::IDENTITY,
    );
    for x in [-56.5, 56.5] {
        spawn_box(
            commands,
            mesh,
            &material.marble_light,
            Vec3::new(x, -0.36, -23.0),
            Vec3::new(23.0, 0.7, 31.0),
            Quat::IDENTITY,
        );
    }
    spawn_mesh(commands, &mesh.floor, &material.floor, Transform::IDENTITY);
    collision_world.add_box(Vec3::new(-45.0, -1.0, -105.0), Vec3::new(45.0, 0.0, 80.0));
    collision_world.add_box(Vec3::new(-68.0, -1.0, -39.0), Vec3::new(68.0, 0.0, -7.0));

    // A dark processional runner broken by pale diamonds gives the long nave a
    // readable axis and catches the low, warm lighting.
    spawn_box(
        commands,
        mesh,
        &material.marble_dark,
        Vec3::new(0.0, 0.025, 9.0),
        Vec3::new(7.6, 0.05, 137.0),
        Quat::IDENTITY,
    );
    for z in (-72..=72).step_by(16) {
        spawn_box(
            commands,
            mesh,
            &material.marble_light,
            Vec3::new(0.0, 0.058, z as f32),
            Vec3::new(4.2, 0.04, 4.2),
            Quat::from_rotation_y(FRAC_PI_2 / 2.0),
        );
    }

    // Radial compass below the dome.
    spawn_cylinder(
        commands,
        mesh,
        &material.marble_dark,
        Vec3::new(0.0, 0.055, -23.0),
        10.2,
        0.07,
    );
    spawn_cylinder(
        commands,
        mesh,
        &material.marble_light,
        Vec3::new(0.0, 0.10, -23.0),
        7.7,
        0.06,
    );
    spawn_cylinder(
        commands,
        mesh,
        &material.gold,
        Vec3::new(0.0, 0.14, -23.0),
        2.0,
        0.065,
    );
    for i in 0..12 {
        let angle = i as f32 * TAU / 12.0;
        spawn_box(
            commands,
            mesh,
            &material.marble_dark,
            Vec3::new(angle.sin() * 5.0, 0.145, -23.0 + angle.cos() * 5.0),
            Vec3::new(0.35, 0.055, 7.2),
            Quat::from_rotation_y(angle),
        );
    }
}

fn build_outer_shell(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    // Long outer aisle walls, split at the transept openings.
    for side in [-1.0, 1.0] {
        for (center_z, length) in [(34.0, 88.0), (-69.5, 53.0)] {
            spawn_box(
                commands,
                mesh,
                &material.stone,
                Vec3::new(side * 44.0, 9.0, center_z),
                Vec3::new(2.0, 18.0, length),
                Quat::IDENTITY,
            );
            collision_world.add_box(
                Vec3::new(side * 44.0 - 1.0, 0.0, center_z - length * 0.5),
                Vec3::new(side * 44.0 + 1.0, 18.0, center_z + length * 0.5),
            );
        }

        // Transept end walls.
        spawn_box(
            commands,
            mesh,
            &material.stone,
            Vec3::new(side * 67.0, 11.0, -23.0),
            Vec3::new(2.0, 22.0, 32.0),
            Quat::IDENTITY,
        );
        collision_world.add_box(
            Vec3::new(side * 67.0 - 1.0, 0.0, -39.0),
            Vec3::new(side * 67.0 + 1.0, 22.0, -7.0),
        );
    }

    for z in [-39.0, -7.0] {
        for side in [-1.0, 1.0] {
            let inner_x = side * 44.0;
            let outer_x = side * 67.0;
            spawn_box(
                commands,
                mesh,
                &material.stone,
                Vec3::new(side * 55.5, 9.0, z),
                Vec3::new(23.0, 18.0, 2.0),
                Quat::IDENTITY,
            );
            collision_world.add_box(
                Vec3::new(inner_x.min(outer_x), 0.0, z - 1.0),
                Vec3::new(inner_x.max(outer_x), 18.0, z + 1.0),
            );
        }
    }

    // Heavy cornices visually cap the lower side aisles.
    for side in [-1.0, 1.0] {
        for (center_z, length) in [(34.0, 88.0), (-69.5, 53.0)] {
            spawn_box(
                commands,
                mesh,
                &material.pale_stone,
                Vec3::new(side * 42.7, 17.1, center_z),
                Vec3::new(3.5, 1.0, length),
                Quat::IDENTITY,
            );
        }
    }
}

fn build_nave(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    const WEST_BAYS: [f32; 6] = [72.0, 56.0, 40.0, 24.0, 8.0, -8.0];
    const CHOIR_BAYS: [f32; 4] = [-38.0, -54.0, -70.0, -82.0];

    for z in WEST_BAYS.into_iter().chain(CHOIR_BAYS) {
        for side in [-1.0, 1.0] {
            let x = side * 13.0;
            spawn_compound_column(commands, mesh, material, Vec3::new(x, 0.0, z), 1.0, 16.0);
            collision_world.add_box(
                Vec3::new(x - 1.35, 0.0, z - 1.35),
                Vec3::new(x + 1.35, 16.3, z + 1.35),
            );
        }

        // Transverse arches create the powerful repeated ribs visible along the
        // central vista.
        spawn_mesh(
            commands,
            &mesh.nave_arch,
            &material.pale_stone,
            Transform::from_xyz(0.0, 16.0, z).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        );
        spawn_box(
            commands,
            mesh,
            &material.pale_stone,
            Vec3::new(0.0, 29.0, z),
            Vec3::new(28.5, 0.55, 0.75),
            Quat::IDENTITY,
        );
    }

    // Longitudinal arcades between the major piers.
    for pair in WEST_BAYS.windows(2).chain(CHOIR_BAYS.windows(2)) {
        let midpoint = (pair[0] + pair[1]) * 0.5;
        for side in [-1.0, 1.0] {
            spawn_mesh(
                commands,
                &mesh.arcade_arch,
                &material.pale_stone,
                Transform::from_xyz(side * 13.0, 16.0, midpoint).with_rotation(
                    Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2),
                ),
            );
            // Clerestory backing and a slender divider above each arcade.
            spawn_box(
                commands,
                mesh,
                &material.stone,
                Vec3::new(side * 13.7, 24.0, midpoint),
                Vec3::new(1.2, 10.5, 14.5),
                Quat::IDENTITY,
            );
            spawn_cylinder(
                commands,
                mesh,
                &material.pale_stone,
                Vec3::new(side * 12.95, 24.0, midpoint),
                0.35,
                7.8,
            );
        }
    }

    // Faceted stone panels approximate a high barrel vault. Each segment is a
    // reusable cuboid, so the ceiling remains inexpensive despite its scale.
    for (start, end) in [(80.0, -16.0), (-30.0, -88.0)] {
        let mut z = start - 8.0;
        while z > end {
            for segment in 0..10 {
                let theta = (segment as f32 + 0.5) * PI / 10.0;
                let radius = 12.8;
                let p = Vec3::new(radius * theta.cos(), 16.0 + radius * theta.sin(), z);
                spawn_box(
                    commands,
                    mesh,
                    &material.stone,
                    p,
                    Vec3::new(4.15, 0.34, 15.7),
                    Quat::from_rotation_z(theta - FRAC_PI_2),
                );
            }
            z -= 16.0;
        }
    }

    // A second, slimmer order of columns adds the layered Baroque complexity of
    // the reference rather than leaving the structural piers as plain cylinders.
    for z in [64.0, 48.0, 32.0, 16.0, 0.0, -46.0, -62.0, -76.0] {
        for side in [-1.0, 1.0] {
            spawn_compound_column(
                commands,
                mesh,
                material,
                Vec3::new(side * 16.0, 0.0, z),
                0.52,
                12.2,
            );
        }
    }
}

fn build_side_aisles_and_chapels(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    // Low aisle ceilings and transverse ribs emphasize the vertical jump into
    // the nave and hide the outer wall tops from ground level.
    for side in [-1.0, 1.0] {
        spawn_box(
            commands,
            mesh,
            &material.dark_stone,
            Vec3::new(side * 28.3, 17.5, 32.0),
            Vec3::new(29.5, 0.55, 88.0),
            Quat::IDENTITY,
        );
        spawn_box(
            commands,
            mesh,
            &material.dark_stone,
            Vec3::new(side * 28.3, 17.5, -69.0),
            Vec3::new(29.5, 0.55, 54.0),
            Quat::IDENTITY,
        );

        for z in [72.0, 56.0, 40.0, 24.0, 8.0, -8.0, -40.0, -56.0, -72.0] {
            spawn_box(
                commands,
                mesh,
                &material.pale_stone,
                Vec3::new(side * 28.5, 16.9, z),
                Vec3::new(30.0, 0.65, 0.8),
                Quat::IDENTITY,
            );
        }
    }

    let chapel_z = [60.0, 36.0, 12.0, -50.0, -74.0];
    for (index, z) in chapel_z.into_iter().enumerate() {
        for side in [-1.0, 1.0] {
            let x = side * 42.85;
            let rotation = if side > 0.0 {
                Quat::from_rotation_y(-FRAC_PI_2)
            } else {
                Quat::from_rotation_y(FRAC_PI_2)
            };
            let glass = if (index + (side > 0.0) as usize).is_multiple_of(3) {
                &material.blue_glass
            } else if index % 2 == 0 {
                &material.amber_glass
            } else {
                &material.red_glass
            };

            spawn_mesh(
                commands,
                &mesh.pane,
                glass,
                Transform::from_xyz(x - side * 0.06, 10.4, z)
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(5.3, 10.0, 1.0)),
            );
            spawn_mesh(
                commands,
                &mesh.chapel_arch,
                &material.pale_stone,
                Transform::from_xyz(x - side * 0.25, 7.0, z).with_rotation(
                    Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2),
                ),
            );

            // Small chapel altar, reredos, and gilded devotional figure.
            spawn_box(
                commands,
                mesh,
                &material.marble_dark,
                Vec3::new(side * 38.8, 0.55, z),
                Vec3::new(3.2, 1.1, 5.2),
                Quat::IDENTITY,
            );
            collision_world.add_box(
                Vec3::new(side * 38.8 - 1.6, 0.0, z - 2.6),
                Vec3::new(side * 38.8 + 1.6, 1.1, z + 2.6),
            );
            spawn_box(
                commands,
                mesh,
                &material.gold,
                Vec3::new(side * 39.3, 4.3, z),
                Vec3::new(0.35, 6.2, 3.1),
                Quat::IDENTITY,
            );
            spawn_cylinder(
                commands,
                mesh,
                &material.gold,
                Vec3::new(side * 38.8, 3.1, z),
                0.42,
                3.0,
            );
            spawn_mesh(
                commands,
                &mesh.sphere,
                &material.gold,
                Transform::from_xyz(side * 38.8, 5.0, z).with_scale(Vec3::splat(0.62)),
            );
        }
    }

    // Paired outer-wall pilasters make the chapels feel recessed rather than
    // pasted onto a featureless wall.
    for z in [76.0, 52.0, 28.0, 4.0, -38.0, -62.0, -86.0] {
        for side in [-1.0, 1.0] {
            spawn_compound_column(
                commands,
                mesh,
                material,
                Vec3::new(side * 41.8, 0.0, z),
                0.48,
                13.2,
            );
        }
    }
}

fn build_crossing_and_dome(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    const CENTER_Z: f32 = -23.0;

    // Four oversized crossing piers carry the drum and provide an unmistakable
    // transition from nave to transept.
    for x in [-16.0, 16.0] {
        for z in [-39.0, -7.0] {
            spawn_box(
                commands,
                mesh,
                &material.stone,
                Vec3::new(x, 10.0, z),
                Vec3::new(4.2, 20.0, 4.2),
                Quat::IDENTITY,
            );
            spawn_compound_column(commands, mesh, material, Vec3::new(x, 0.0, z), 1.45, 20.0);
            collision_world.add_box(
                Vec3::new(x - 2.2, 0.0, z - 2.2),
                Vec3::new(x + 2.2, 20.5, z + 2.2),
            );
        }
    }

    for z in [-39.0, -7.0] {
        spawn_mesh(
            commands,
            &mesh.crossing_arch,
            &material.pale_stone,
            Transform::from_xyz(0.0, 20.0, z).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        );
    }
    for x in [-16.0, 16.0] {
        spawn_mesh(
            commands,
            &mesh.crossing_arch,
            &material.pale_stone,
            Transform::from_xyz(x, 20.0, CENTER_Z).with_rotation(
                Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_x(-FRAC_PI_2),
            ),
        );
    }

    // Repeated wall pieces leave deep dark seams around the drum, creating the
    // impression of a ring of windows without requiring boolean mesh cuts.
    let drum_segments = 20;
    for i in 0..drum_segments {
        let angle = i as f32 * TAU / drum_segments as f32;
        let radial = Vec3::new(angle.cos(), 0.0, angle.sin());
        let position = Vec3::new(0.0, 34.0, CENTER_Z) + radial * 22.25;
        spawn_box(
            commands,
            mesh,
            &material.stone,
            position,
            Vec3::new(5.8, 9.0, 1.25),
            Quat::from_rotation_y(-angle + FRAC_PI_2),
        );
        spawn_cylinder(
            commands,
            mesh,
            &material.pale_stone,
            Vec3::new(0.0, 34.0, CENTER_Z) + radial * 21.45,
            0.42,
            9.4,
        );

        if i % 2 == 0 {
            spawn_mesh(
                commands,
                &mesh.pane,
                if i % 4 == 0 {
                    &material.blue_glass
                } else {
                    &material.amber_glass
                },
                Transform::from_translation(Vec3::new(0.0, 34.3, CENTER_Z) + radial * 21.58)
                    .with_rotation(Quat::from_rotation_y(-angle + FRAC_PI_2))
                    .with_scale(Vec3::new(3.2, 5.8, 1.0)),
            );
        }
    }

    for y in [29.3, 38.9] {
        spawn_mesh(
            commands,
            &mesh.drum_ring,
            &material.pale_stone,
            Transform::from_xyz(0.0, y, CENTER_Z),
        );
    }

    // The dome shell is a genuine open hemisphere with inward-visible material,
    // avoiding the lower half of a stock sphere intruding into the crossing.
    spawn_mesh(
        commands,
        &mesh.dome,
        &material.dark_stone,
        Transform::from_xyz(0.0, 39.0, CENTER_Z),
    );

    // Sixteen segmented ribs follow the ellipsoid closely. Pale stone ribs over
    // the blue-black shell are legible even from the entrance sixty metres away.
    for rib in 0..16 {
        let azimuth = rib as f32 * TAU / 16.0;
        let (sin_azimuth, cos_azimuth) = azimuth.sin_cos();
        for step in 0..8 {
            let a0 = step as f32 * FRAC_PI_2 / 8.0;
            let a1 = (step + 1) as f32 * FRAC_PI_2 / 8.0;
            let dome_point = |angle: f32| {
                let horizontal = 22.18 * angle.cos();
                Vec3::new(
                    horizontal * cos_azimuth,
                    38.8 + 24.72 * angle.sin(),
                    CENTER_Z + horizontal * sin_azimuth,
                )
            };
            spawn_beam_between(
                commands,
                mesh,
                &material.pale_stone,
                dome_point(a0),
                dome_point(a1),
                0.24,
            );
        }
    }

    // Oculus, pendant, and lantern-like inner crown.
    spawn_mesh(
        commands,
        &mesh.rose_ring,
        &material.gold,
        Transform::from_xyz(0.0, 63.0, CENTER_Z).with_scale(Vec3::splat(0.33)),
    );
    spawn_cylinder(
        commands,
        mesh,
        &material.bronze,
        Vec3::new(0.0, 57.8, CENTER_Z),
        0.12,
        9.4,
    );
    spawn_mesh(
        commands,
        &mesh.sphere,
        &material.gold,
        Transform::from_xyz(0.0, 52.5, CENTER_Z).with_scale(Vec3::splat(0.75)),
    );

    // Transept colonnades and end-window landmarks.
    for side in [-1.0, 1.0] {
        for x_abs in [31.0, 47.0, 61.0] {
            spawn_compound_column(
                commands,
                mesh,
                material,
                Vec3::new(side * x_abs, 0.0, -10.0),
                0.7,
                15.0,
            );
            spawn_compound_column(
                commands,
                mesh,
                material,
                Vec3::new(side * x_abs, 0.0, -36.0),
                0.7,
                15.0,
            );
        }
        spawn_mesh(
            commands,
            &mesh.rose_disc,
            if side < 0.0 {
                &material.blue_glass
            } else {
                &material.red_glass
            },
            Transform::from_xyz(side * 65.9, 13.0, CENTER_Z)
                .with_rotation(if side < 0.0 {
                    Quat::from_rotation_y(FRAC_PI_2)
                } else {
                    Quat::from_rotation_y(-FRAC_PI_2)
                })
                .with_scale(Vec3::splat(6.0)),
        );
    }
}

fn build_apse_and_altar(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    let center_z = -82.0;
    let radius = 23.0;
    let segment_count = 12;

    for i in 0..segment_count {
        let theta = (i as f32 + 0.5) * PI / segment_count as f32;
        let position = Vec3::new(radius * theta.cos(), 11.0, center_z - radius * theta.sin());
        spawn_box(
            commands,
            mesh,
            if i % 2 == 0 {
                &material.apse_mosaic
            } else {
                &material.stone
            },
            position,
            Vec3::new(6.2, 22.0, 1.4),
            Quat::from_rotation_y(FRAC_PI_2 + theta),
        );

        let boundary_angle = i as f32 * PI / segment_count as f32;
        spawn_compound_column(
            commands,
            mesh,
            material,
            Vec3::new(
                radius * boundary_angle.cos() * 0.985,
                0.0,
                center_z - radius * boundary_angle.sin() * 0.985,
            ),
            0.62,
            17.0,
        );
    }

    // A coarse rear boundary follows the outside silhouette while keeping the
    // collision representation deliberately simple and stable.
    collision_world.add_box(Vec3::new(-24.0, 0.0, -106.0), Vec3::new(24.0, 22.0, -104.0));

    // Rising altar steps.
    for i in 0..4 {
        let width = 17.0 - i as f32 * 2.1;
        let depth = 12.0 - i as f32 * 1.55;
        let height = 0.28 * (i + 1) as f32;
        spawn_box(
            commands,
            mesh,
            if i % 2 == 0 {
                &material.marble_dark
            } else {
                &material.marble_light
            },
            Vec3::new(0.0, height * 0.5, -82.5 - i as f32 * 0.35),
            Vec3::new(width, height, depth),
            Quat::IDENTITY,
        );
    }
    collision_world.add_box(Vec3::new(-8.5, 0.0, -89.0), Vec3::new(8.5, 1.15, -76.5));

    // Gilded baldachin: four dark columns, entablature, canopy, and cross.
    for x in [-4.4, 4.4] {
        for z in [-85.5, -78.2] {
            spawn_compound_column(commands, mesh, material, Vec3::new(x, 1.1, z), 0.62, 10.0);
            spawn_cylinder(
                commands,
                mesh,
                &material.gold,
                Vec3::new(x, 7.2, z),
                0.72,
                0.28,
            );
        }
    }
    spawn_box(
        commands,
        mesh,
        &material.gold,
        Vec3::new(0.0, 11.25, -81.85),
        Vec3::new(11.3, 1.0, 10.1),
        Quat::IDENTITY,
    );
    spawn_mesh(
        commands,
        &mesh.sphere,
        &material.gold,
        Transform::from_xyz(0.0, 12.0, -81.85).with_scale(Vec3::new(5.3, 2.1, 4.7)),
    );
    spawn_box(
        commands,
        mesh,
        &material.gold,
        Vec3::new(0.0, 15.0, -81.85),
        Vec3::new(0.28, 4.3, 0.28),
        Quat::IDENTITY,
    );
    spawn_box(
        commands,
        mesh,
        &material.gold,
        Vec3::new(0.0, 15.6, -81.85),
        Vec3::new(2.4, 0.28, 0.28),
        Quat::IDENTITY,
    );

    // Altar table and luminous tabernacle.
    spawn_box(
        commands,
        mesh,
        &material.marble_light,
        Vec3::new(0.0, 2.15, -76.1),
        Vec3::new(7.2, 1.2, 2.3),
        Quat::IDENTITY,
    );
    spawn_box(
        commands,
        mesh,
        &material.gold,
        Vec3::new(0.0, 3.35, -76.2),
        Vec3::new(1.8, 1.65, 1.1),
        Quat::IDENTITY,
    );
}

fn build_west_end(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    collision_world: &mut CollisionWorld,
) {
    // Interior west facade. The central doors stand open so the nave connects
    // directly to the forecourt and the surrounding city.
    spawn_box(
        commands,
        mesh,
        &material.stone,
        Vec3::new(-25.0, 15.0, 80.0),
        Vec3::new(40.0, 30.0, 2.0),
        Quat::IDENTITY,
    );
    spawn_box(
        commands,
        mesh,
        &material.stone,
        Vec3::new(25.0, 15.0, 80.0),
        Vec3::new(40.0, 30.0, 2.0),
        Quat::IDENTITY,
    );
    spawn_box(
        commands,
        mesh,
        &material.stone,
        Vec3::new(0.0, 26.0, 80.0),
        Vec3::new(12.0, 8.0, 2.0),
        Quat::IDENTITY,
    );
    collision_world.add_box(Vec3::new(-45.0, 0.0, 79.0), Vec3::new(-5.5, 30.0, 81.0));
    collision_world.add_box(Vec3::new(5.5, 0.0, 79.0), Vec3::new(45.0, 30.0, 81.0));
    collision_world.add_box(Vec3::new(-5.5, 22.0, 79.0), Vec3::new(5.5, 30.0, 81.0));

    for side in [-1.0, 1.0] {
        spawn_box(
            commands,
            mesh,
            &material.wood,
            Vec3::new(side * 5.3, 5.0, 76.1),
            Vec3::new(0.45, 10.0, 5.8),
            Quat::IDENTITY,
        );
        for y in [2.0, 5.0, 8.0] {
            spawn_box(
                commands,
                mesh,
                &material.bronze,
                Vec3::new(side * 5.05, y, 76.1),
                Vec3::new(0.12, 0.16, 4.9),
                Quat::IDENTITY,
            );
        }
    }

    // The supplied rose window is the west-end color landmark.
    spawn_mesh(
        commands,
        &mesh.rose_disc,
        &material.rose,
        Transform::from_xyz(0.0, 21.5, 78.83).with_scale(Vec3::splat(7.0)),
    );
    spawn_mesh(
        commands,
        &mesh.rose_ring,
        &material.pale_stone,
        Transform::from_xyz(0.0, 21.5, 78.72).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    );

    // Organ loft, pipes, and balustrade below the rose.
    spawn_box(
        commands,
        mesh,
        &material.wood,
        Vec3::new(0.0, 11.1, 73.5),
        Vec3::new(24.0, 1.0, 8.0),
        Quat::IDENTITY,
    );
    for i in -8_i32..=8 {
        let height = 3.8 + (8 - i.abs()) as f32 * 0.55;
        spawn_cylinder(
            commands,
            mesh,
            &material.bronze,
            Vec3::new(i as f32 * 0.66, 13.0 + height * 0.5, 77.8),
            0.18,
            height,
        );
    }
    for x in -10..=10 {
        spawn_cylinder(
            commands,
            mesh,
            &material.pale_stone,
            Vec3::new(x as f32, 12.5, 69.7),
            0.11,
            2.4,
        );
    }
    spawn_box(
        commands,
        mesh,
        &material.pale_stone,
        Vec3::new(0.0, 13.55, 69.7),
        Vec3::new(22.0, 0.25, 0.25),
        Quat::IDENTITY,
    );
}

fn build_lighting(commands: &mut Commands, mesh: &CathedralMeshes, material: &CathedralMaterials) {
    commands.spawn((
        DirectionalLight {
            // Raw sunlight is filtered by the atmosphere into a warm, late
            // afternoon key light and a cooler sky fill.
            color: Color::WHITE,
            illuminance: lux::RAW_SUNLIGHT,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-420.0, 560.0, 300.0).looking_at(Vec3::new(0.0, 0.0, 40.0), Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 24.0,
            maximum_distance: 520.0,
            ..default()
        }
        .build(),
    ));

    // A cool shaft from the oculus and a warm focus on the high altar.
    commands.spawn((
        SpotLight {
            color: Color::srgb(0.62, 0.76, 1.0),
            intensity: 1_200_000.0,
            range: 75.0,
            radius: 2.5,
            inner_angle: 0.18,
            outer_angle: 0.43,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-2.0, 60.0, -21.0).looking_at(Vec3::new(2.0, 0.0, -25.0), Vec3::Z),
    ));
    commands.spawn((
        SpotLight {
            color: Color::srgb(1.0, 0.52, 0.20),
            intensity: 850_000.0,
            range: 42.0,
            radius: 1.2,
            inner_angle: 0.35,
            outer_angle: 0.72,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 18.0, -70.0).looking_at(Vec3::new(0.0, 2.0, -82.0), Vec3::Y),
    ));

    // Sparse chandeliers; only alternating lights cast shadows to keep the large
    // procedural scene affordable.
    for (index, z) in [56.0, 32.0, 8.0, -52.0, -72.0].into_iter().enumerate() {
        spawn_cylinder(
            commands,
            mesh,
            &material.bronze,
            Vec3::new(0.0, 21.5, z),
            0.08,
            8.0,
        );
        spawn_mesh(
            commands,
            &mesh.sphere,
            &material.candle,
            Transform::from_xyz(0.0, 17.3, z).with_scale(Vec3::splat(0.24)),
        );
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.56, 0.24),
                intensity: 55_000.0,
                range: 23.0,
                radius: 0.45,
                shadow_maps_enabled: index % 2 == 0,
                ..default()
            },
            Transform::from_xyz(0.0, 17.2, z),
        ));
    }

    for side in [-1.0, 1.0] {
        for z in [60.0, 36.0, 12.0, -50.0, -74.0] {
            commands.spawn((
                PointLight {
                    color: if side < 0.0 {
                        Color::srgb(0.38, 0.55, 1.0)
                    } else {
                        Color::srgb(1.0, 0.42, 0.12)
                    },
                    intensity: 18_000.0,
                    range: 12.0,
                    radius: 0.25,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(side * 39.0, 7.0, z),
            ));
        }
    }
}

fn add_fog_to_new_cameras(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<DistanceFog>)>,
) {
    for entity in &cameras {
        commands.entity(entity).insert(DistanceFog {
            color: Color::srgba(0.58, 0.68, 0.73, 0.22),
            directional_light_color: Color::srgba(1.0, 0.78, 0.52, 0.32),
            directional_light_exponent: 24.0,
            // The old density made most of this 1.2 km city converge on
            // near-black. This leaves only a restrained coastal haze.
            falloff: FogFalloff::from_visibility_squared(1_800.0),
        });
    }
}

fn spawn_compound_column(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &CathedralMaterials,
    foot: Vec3,
    scale: f32,
    height: f32,
) {
    let shaft_height = height - 2.2 * scale;
    spawn_box(
        commands,
        mesh,
        &material.pale_stone,
        foot + Vec3::Y * 0.35 * scale,
        Vec3::new(2.7, 0.7, 2.7) * scale,
        Quat::IDENTITY,
    );
    spawn_cylinder(
        commands,
        mesh,
        &material.pale_stone,
        foot + Vec3::Y * 1.0 * scale,
        1.28 * scale,
        0.65 * scale,
    );
    spawn_cylinder(
        commands,
        mesh,
        &material.stone,
        foot + Vec3::Y * (1.32 * scale + shaft_height * 0.5),
        0.82 * scale,
        shaft_height,
    );

    // Two attached shafts make each support read as a compound pier.
    for z_offset in [-0.84, 0.84] {
        spawn_cylinder(
            commands,
            mesh,
            &material.pale_stone,
            foot + Vec3::new(0.0, 1.28 * scale + shaft_height * 0.5, z_offset * scale),
            0.26 * scale,
            shaft_height * 0.94,
        );
    }
    spawn_cylinder(
        commands,
        mesh,
        &material.pale_stone,
        foot + Vec3::Y * (height - 0.75 * scale),
        1.18 * scale,
        0.85 * scale,
    );
    spawn_box(
        commands,
        mesh,
        &material.pale_stone,
        foot + Vec3::Y * (height - 0.2 * scale),
        Vec3::new(2.45, 0.55, 2.45) * scale,
        Quat::IDENTITY,
    );
}

fn spawn_box(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    rotation: Quat,
) {
    spawn_mesh(
        commands,
        &mesh.cube,
        material,
        Transform {
            translation: center,
            rotation,
            scale: size,
        },
    );
}

fn spawn_cylinder(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    radius: f32,
    height: f32,
) {
    spawn_mesh(
        commands,
        &mesh.cylinder,
        material,
        Transform::from_translation(center).with_scale(Vec3::new(radius, height, radius)),
    );
}

fn spawn_beam_between(
    commands: &mut Commands,
    mesh: &CathedralMeshes,
    material: &Handle<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    radius: f32,
) {
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    spawn_mesh(
        commands,
        &mesh.cylinder,
        material,
        Transform {
            translation: (start + end) * 0.5,
            rotation: Quat::from_rotation_arc(Vec3::Y, delta / length),
            scale: Vec3::new(radius, length, radius),
        },
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

fn hemisphere_mesh(radius: f32, height: f32, sectors: u32, stacks: u32) -> Mesh {
    let row = sectors + 1;
    let mut positions = Vec::with_capacity(((sectors + 1) * (stacks + 1)) as usize);
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::with_capacity((sectors * stacks * 6) as usize);

    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let latitude = v * FRAC_PI_2;
        let ring_radius = radius * latitude.cos();
        let y = height * latitude.sin();

        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let longitude = u * TAU;
            let (sin_longitude, cos_longitude) = longitude.sin_cos();
            let x = ring_radius * cos_longitude;
            let z = ring_radius * sin_longitude;
            let normal = Vec3::new(
                x / (radius * radius),
                y / (height * height),
                z / (radius * radius),
            )
            .normalize_or_zero();

            positions.push([x, y, z]);
            normals.push(normal.to_array());
            uvs.push([u, 1.0 - v]);
        }
    }

    for stack in 0..stacks {
        for sector in 0..sectors {
            let a = stack * row + sector;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

/// A non-overlapping union of the nave and transept floor. UVs derive from
/// world X/Z coordinates, keeping every slab the same size across all wings.
fn cathedral_floor_mesh() -> Mesh {
    let rectangles = [
        (-45.0, 45.0, -100.0, 80.0),
        (-68.0, -45.0, -38.5, -7.5),
        (45.0, 68.0, -38.5, -7.5),
    ];
    let mut positions = Vec::with_capacity(rectangles.len() * 4);
    let mut normals = Vec::with_capacity(rectangles.len() * 4);
    let mut uvs = Vec::with_capacity(rectangles.len() * 4);
    let mut indices = Vec::with_capacity(rectangles.len() * 6);

    for (min_x, max_x, min_z, max_z) in rectangles {
        let first = positions.len() as u32;
        for (x, z) in [
            (min_x, min_z),
            (min_x, max_z),
            (max_x, max_z),
            (max_x, min_z),
        ] {
            positions.push([x, 0.0, z]);
            normals.push(Vec3::Y.to_array());
            uvs.push([x / FLOOR_TEXTURE_SPAN_METERS, z / FLOOR_TEXTURE_SPAN_METERS]);
        }
        indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;

    #[test]
    fn cathedral_builds_headlessly_with_architecture_and_collision() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, build_cathedral);

        app.update();

        let world = app.world_mut();
        let mesh_entity_count = world
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(world)
            .count();
        assert!(
            mesh_entity_count > 300,
            "expected a detailed scene, got {mesh_entity_count} mesh entities"
        );

        let collision_world = world.resource::<CollisionWorld>();
        assert!(!collision_world.is_empty());
        assert!(
            collision_world.len() > 40,
            "expected a navigable structural shell, got {} colliders",
            collision_world.len()
        );
    }
}
