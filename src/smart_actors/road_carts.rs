//! Presentation-only road carts derived from the sim snapshot.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use super::{
    actors::ActorView,
    model::{ActorId, RoadCartSnapshot, WorldMirror},
};

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RoadCartView {
    pub party_id: String,
    load: Vec<cathedral_sim::CartLoadKind>,
}

/// Semantic marker on each cargo mesh. The cart remains presentation-only,
/// but keeping the category on the child makes the projection inspectable and
/// prevents its visual vocabulary drifting from the sim snapshot.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadCartCargo(pub cathedral_sim::CartLoadKind);

#[derive(Resource)]
pub struct RoadCartAssets {
    bed: Handle<Mesh>,
    rail: Handle<Mesh>,
    wheel: Handle<Mesh>,
    sack: Handle<Mesh>,
    bale: Handle<Mesh>,
    bolt: Handle<Mesh>,
    wood: Handle<StandardMaterial>,
    dark_wood: Handle<StandardMaterial>,
    grain: Handle<StandardMaterial>,
    wool: Handle<StandardMaterial>,
    cloth: Handle<StandardMaterial>,
}

pub fn setup_road_cart_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let matte = |color: Color| StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.88,
        ..default()
    };
    commands.insert_resource(RoadCartAssets {
        bed: meshes.add(Cuboid::new(2.0, 0.22, 3.1)),
        rail: meshes.add(Cuboid::new(0.12, 0.62, 3.0)),
        wheel: meshes.add(Torus::new(0.42, 0.52)),
        sack: meshes.add(Sphere::new(0.42).mesh().uv(12, 8)),
        bale: meshes.add(Cuboid::new(0.72, 0.48, 0.55)),
        bolt: meshes.add(Cylinder::new(0.22, 0.9)),
        wood: materials.add(matte(Color::srgb(0.30, 0.17, 0.08))),
        dark_wood: materials.add(matte(Color::srgb(0.12, 0.075, 0.04))),
        grain: materials.add(matte(Color::srgb(0.66, 0.52, 0.27))),
        wool: materials.add(matte(Color::srgb(0.72, 0.68, 0.57))),
        cloth: materials.add(matte(Color::srgb(0.27, 0.34, 0.47))),
    });
}

#[allow(clippy::type_complexity)]
pub fn reconcile_road_carts(
    mut commands: Commands,
    mirror: Res<WorldMirror>,
    assets: Res<RoadCartAssets>,
    leaders: Query<(&ActorId, &Transform), (With<ActorView>, Without<RoadCartView>)>,
    mut carts: Query<(Entity, &mut RoadCartView, &mut Transform), Without<ActorView>>,
) {
    let desired: HashMap<&str, &RoadCartSnapshot> = mirror
        .road_carts()
        .map(|cart| (cart.party_id.as_str(), cart))
        .collect();
    let leader_transforms: HashMap<&ActorId, &Transform> = leaders.iter().collect();
    let mut existing = HashSet::new();
    for (entity, view, mut transform) in &mut carts {
        let Some(cart) = desired.get(view.party_id.as_str()) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(leader) = leader_transforms.get(&cart.leader_id) else {
            commands.entity(entity).despawn();
            continue;
        };
        if view.load != cart.load {
            commands.entity(entity).despawn();
            continue;
        }
        existing.insert(view.party_id.clone());
        *transform = cart_transform(leader);
    }
    for cart in desired.values() {
        if existing.contains(&cart.party_id) {
            continue;
        }
        let Some(leader) = leader_transforms.get(&cart.leader_id) else {
            continue;
        };
        spawn_cart(&mut commands, &assets, cart, cart_transform(leader));
    }
}

fn cart_transform(leader: &Transform) -> Transform {
    let mut translation = leader.translation + leader.rotation * Vec3::new(0.0, 0.0, 2.7);
    translation.y = 0.0;
    Transform::from_translation(translation).with_rotation(leader.rotation)
}

fn spawn_cart(
    commands: &mut Commands,
    assets: &RoadCartAssets,
    cart: &RoadCartSnapshot,
    transform: Transform,
) {
    commands
        .spawn((
            Name::new(format!("Road cart: {}", cart.party_id)),
            RoadCartView {
                party_id: cart.party_id.clone(),
                load: cart.load.clone(),
            },
            transform,
            Visibility::default(),
        ))
        .with_children(|root| {
            root.spawn((
                Mesh3d(assets.bed.clone()),
                MeshMaterial3d(assets.wood.clone()),
                Transform::from_xyz(0.0, 0.68, 0.0),
            ));
            for x in [-1.02, 1.02] {
                root.spawn((
                    Mesh3d(assets.rail.clone()),
                    MeshMaterial3d(assets.wood.clone()),
                    Transform::from_xyz(x, 1.02, 0.0),
                ));
            }
            for x in [-1.06, 1.06] {
                for z in [-1.0, 1.0] {
                    root.spawn((
                        Mesh3d(assets.wheel.clone()),
                        MeshMaterial3d(assets.dark_wood.clone()),
                        Transform::from_xyz(x, 0.5, z)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ));
                }
            }
            for (slot, load) in cart.load.iter().enumerate() {
                let x = -0.62 + (slot % 3) as f32 * 0.62;
                let z = -0.65 + (slot / 3) as f32 * 0.75;
                match load {
                    cathedral_sim::CartLoadKind::GrainSacks => {
                        for dx in [0.0, 0.42] {
                            root.spawn((
                                RoadCartCargo(*load),
                                Mesh3d(assets.sack.clone()),
                                MeshMaterial3d(assets.grain.clone()),
                                Transform::from_xyz(x + dx, 1.15, z)
                                    .with_scale(Vec3::new(0.8, 1.05, 0.7)),
                            ));
                        }
                    }
                    cathedral_sim::CartLoadKind::WoolBales => {
                        root.spawn((
                            RoadCartCargo(*load),
                            Mesh3d(assets.bale.clone()),
                            MeshMaterial3d(assets.wool.clone()),
                            Transform::from_xyz(x, 1.18, z),
                        ));
                    }
                    cathedral_sim::CartLoadKind::ClothBolts => {
                        root.spawn((
                            RoadCartCargo(*load),
                            Mesh3d(assets.bolt.clone()),
                            MeshMaterial3d(assets.cloth.clone()),
                            Transform::from_xyz(x, 1.17, z)
                                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                        ));
                    }
                }
            }
        });
}
