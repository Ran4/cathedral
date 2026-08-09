//! Street lamps (M7): the render mirror of the sim's lamp set.
//!
//! The sim owns the lamps — positions seeded from the nav graph, lit state
//! flipped by the lamplighter's dusk round (`cathedral-sim/src/round.rs`) — and
//! republishes the whole set on `EngineMessage::Lamps` whenever anything
//! changes. This module stands the posts up once and, on each update, swaps the
//! head glass between its dark and burning materials and gates each post's
//! point light. Nothing here decides when a lamp burns; delaying the
//! lamplighter in conversation visibly delays these lights, which is the
//! feature (`features/50_cool_suggestions.md` #21).

use bevy::prelude::*;

/// Lamp glow reaches this far; well under the chandelier's 23 m so the light
/// budget stays sane with four posts to a square.
const LAMP_LIGHT_RANGE_M: f32 = 18.0;
/// A burning wick behind horn panes — deliberately dimmer than the cathedral's
/// chandeliers (55 k): the squares should read as pools of warmth in a dark
/// city, not floodlit.
const LAMP_LIGHT_INTENSITY: f32 = 30_000.0;

/// The projected lamp set, written by the bridge drain and consumed by
/// [`sync_lamp_props`]. `revision` bumps per received message so the sync does
/// nothing on quiet frames.
#[derive(Resource, Default)]
pub struct CityLamps {
    pub lamps: Vec<LampState>,
    pub revision: u64,
}

pub struct LampState {
    pub position: Vec3,
    pub lit: bool,
    pub square: String,
}

/// A spawned post's head glass, indexed into [`CityLamps::lamps`].
#[derive(Component)]
pub struct LampHead(usize);

/// A spawned post's point light, same index.
#[derive(Component)]
pub struct LampGlow(usize);

/// Shared handles, built on the first spawn.
pub struct LampAssets {
    pub dark_glass: Handle<StandardMaterial>,
    pub lit_glass: Handle<StandardMaterial>,
}

/// Stand the posts up on the first (dark) set, then mirror lit-state changes
/// into the head materials and the point lights.
#[allow(clippy::too_many_arguments)]
pub fn sync_lamp_props(
    mut commands: Commands,
    lamps: Res<CityLamps>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut heads: Query<(&LampHead, &mut MeshMaterial3d<StandardMaterial>)>,
    mut glows: Query<(&LampGlow, &mut PointLight)>,
    mut assets: Local<Option<LampAssets>>,
    mut synced: Local<u64>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Lamps);
    if lamps.revision == *synced || lamps.lamps.is_empty() {
        return;
    }
    *synced = lamps.revision;

    let shared = assets.get_or_insert_with(|| LampAssets {
        dark_glass: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.17, 0.20),
            perceptual_roughness: 0.35,
            ..default()
        }),
        lit_glass: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.55),
            emissive: LinearRgba::rgb(7.0, 3.6, 1.1),
            perceptual_roughness: 0.3,
            ..default()
        }),
    });

    // First non-empty set: spawn the rigs. The set's size and positions are
    // fixed for the session (the sim seeds them once), so later messages only
    // flip lit-state.
    if heads.is_empty() {
        let iron = materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.10, 0.11),
            perceptual_roughness: 0.85,
            metallic: 0.4,
            ..default()
        });
        let post_mesh = meshes.add(Cylinder::new(0.055, 3.1));
        let head_mesh = meshes.add(Cuboid::new(0.30, 0.36, 0.30));
        let cap_mesh = meshes.add(Cuboid::new(0.38, 0.06, 0.38));
        for (index, lamp) in lamps.lamps.iter().enumerate() {
            let base = Vec3::new(lamp.position.x, 0.0, lamp.position.z);
            commands
                .spawn((
                    Name::new(format!("Street lamp: {}", lamp.square)),
                    Transform::from_translation(base),
                    Visibility::default(),
                ))
                .with_children(|rig| {
                    rig.spawn((
                        Mesh3d(post_mesh.clone()),
                        MeshMaterial3d(iron.clone()),
                        Transform::from_xyz(0.0, 1.55, 0.0),
                    ));
                    rig.spawn((
                        LampHead(index),
                        Mesh3d(head_mesh.clone()),
                        MeshMaterial3d(if lamp.lit {
                            shared.lit_glass.clone()
                        } else {
                            shared.dark_glass.clone()
                        }),
                        Transform::from_xyz(0.0, 3.28, 0.0),
                    ));
                    rig.spawn((
                        Mesh3d(cap_mesh.clone()),
                        MeshMaterial3d(iron.clone()),
                        Transform::from_xyz(0.0, 3.49, 0.0),
                    ));
                    rig.spawn((
                        LampGlow(index),
                        PointLight {
                            color: Color::srgb(1.0, 0.62, 0.28),
                            intensity: if lamp.lit { LAMP_LIGHT_INTENSITY } else { 0.0 },
                            range: LAMP_LIGHT_RANGE_M,
                            radius: 0.15,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 3.2, 0.0),
                    ));
                });
        }
        return;
    }

    for (head, mut material) in heads.iter_mut() {
        if let Some(lamp) = lamps.lamps.get(head.0) {
            let wanted = if lamp.lit {
                &shared.lit_glass
            } else {
                &shared.dark_glass
            };
            if material.0 != *wanted {
                material.0 = wanted.clone();
            }
        }
    }
    for (glow, mut light) in glows.iter_mut() {
        if let Some(lamp) = lamps.lamps.get(glow.0) {
            light.intensity = if lamp.lit { LAMP_LIGHT_INTENSITY } else { 0.0 };
        }
    }
}
