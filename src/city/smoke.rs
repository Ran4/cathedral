//! Chimney smoke: the one animated layer of the otherwise static city.
//!
//! `add_chimneys` reports every stack it plants on a ridge; a stable hash
//! picks the fraction of them whose hearths are lit. Each lit stack carries a
//! looping column of puffs — camera-facing quads that rise, drift with the
//! prevailing wind, swell and fade — and every puff in the city is rewritten
//! each frame into one shared mesh, so the whole skyline of plumes costs a
//! single draw call, one entity, and nothing at all in navigation.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// One stack in this many smokes; the rest are cold hearths.
const SMOKE_GATE: u32 = 4;
/// Puffs alive per plume at any moment, evenly staggered through the loop.
const PUFFS_PER_PLUME: usize = 6;
/// Seconds from leaving the flue to dissolving.
const PUFF_LIFE_S: f32 = 9.0;
/// How fast a fresh puff climbs; ageing puffs ease off as the wind takes over.
const RISE_M_PER_S: f32 = 1.0;
/// The prevailing wind over the roofs, before per-plume jitter.
const WIND_HEADING_RAD: f32 = 2.3;
const WIND_SPEED_M_PER_S: f32 = 0.55;
/// A puff swells from flue-width to a loose cloud as it dissolves.
const PUFF_START_DIAMETER_M: f32 = 0.7;
const PUFF_END_DIAMETER_M: f32 = 3.4;
const PUFF_PEAK_ALPHA: f32 = 0.65;

/// Where a chimney flue tops out, reported by `add_chimneys` for every stack
/// whether or not it ends up smoking, plus the stack's stable hash.
pub(super) struct ChimneyAnchor {
    pub top: Vec3,
    pub seed: u32,
}

/// The whole city's smoke: one entity, one mesh rebuilt per frame.
#[derive(Component)]
pub(super) struct ChimneySmoke {
    plumes: Vec<Plume>,
}

struct Plume {
    top: Vec3,
    seed: u32,
    /// Offset into the puff loop, so plumes never breathe in unison.
    phase: f32,
    /// Ground-plane drift in metres per second: wind plus per-flue jitter.
    drift: Vec2,
    /// Woodsmoke runs warm-grey to blue-grey depending on the hearth.
    tint: [f32; 3],
}

/// A uniform draw from the higher bits of a stack's hash.
fn unit(seed: u32, shift: u32) -> f32 {
    ((seed >> shift) % 997) as f32 / 996.0
}

pub(super) fn build_chimney_smoke(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    anchors: &[ChimneyAnchor],
) -> usize {
    let wind = Vec2::from_angle(WIND_HEADING_RAD) * WIND_SPEED_M_PER_S;
    let plumes: Vec<Plume> = anchors
        .iter()
        .filter(|anchor| anchor.seed % SMOKE_GATE == 0)
        .map(|anchor| {
            let seed = anchor.seed;
            let cool = unit(seed, 11);
            Plume {
                top: anchor.top,
                seed,
                phase: unit(seed, 15) * PUFF_LIFE_S,
                drift: Vec2::from_angle((unit(seed, 3) - 0.5) * 0.8).rotate(wind)
                    * (0.7 + 0.6 * unit(seed, 7)),
                tint: [0.88 - 0.14 * cool, 0.86 - 0.10 * cool, 0.84 - 0.04 * cool],
            }
        })
        .collect();
    if plumes.is_empty() {
        return 0;
    }
    let count = plumes.len();

    // The mesh starts empty but with every attribute present, so it is valid
    // to draw even on a frame with no camera to billboard toward.
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));

    commands.spawn((
        Name::new("Chimney smoke"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("textures/ombreval_smoke.png")),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        // The quads chase the camera every frame, so a baked AABB would lie;
        // a few thousand vertices are cheaper than keeping one honest.
        NoFrustumCulling,
        NotShadowCaster,
        ChimneySmoke { plumes },
    ));
    count
}

/// Rewrites the shared smoke mesh: every live puff placed on its plume's arc,
/// billboarded toward the camera, sorted back-to-front so the blend holds.
pub(super) fn animate_chimney_smoke(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    smoke: Query<(&ChimneySmoke, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let camera_position = camera.translation();
    let elapsed = time.elapsed_secs();

    struct Puff {
        center: Vec3,
        half: f32,
        alpha: f32,
        tint: [f32; 3],
        cell: [f32; 2],
        distance_sq: f32,
    }

    for (smoke, mesh_handle) in &smoke {
        let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };

        let mut puffs = Vec::with_capacity(smoke.plumes.len() * PUFFS_PER_PLUME);
        for plume in &smoke.plumes {
            let across = plume.drift.perp().normalize_or_zero();
            for index in 0..PUFFS_PER_PLUME {
                let slot = index as f32 * (PUFF_LIFE_S / PUFFS_PER_PLUME as f32);
                let age = (elapsed + plume.phase + slot).rem_euclid(PUFF_LIFE_S);
                let f = age / PUFF_LIFE_S;
                // Straight up out of the flue, then the wind takes it: the
                // climb eases as the drift grows quadratically into a bend.
                let rise = RISE_M_PER_S * age * (1.0 - 0.22 * f);
                let bend = plume.drift * (age * f);
                let sway = across
                    * ((elapsed * 0.7 + plume.phase * 3.1 + index as f32 * 1.9).sin() * 0.35 * f);
                let center =
                    plume.top + Vec3::Y * rise + Vec3::new(bend.x + sway.x, 0.0, bend.y + sway.y);
                let fade_in = (f / 0.12).min(1.0);
                let fade_out = (1.0 - f) * (1.0 - f).sqrt();
                let diameter = PUFF_START_DIAMETER_M
                    + (PUFF_END_DIAMETER_M - PUFF_START_DIAMETER_M) * f.powf(0.7);
                let cell_index = (plume.seed >> (2 * index as u32)) & 3;
                puffs.push(Puff {
                    center,
                    half: diameter * 0.5,
                    alpha: PUFF_PEAK_ALPHA * fade_in * fade_out,
                    tint: plume.tint,
                    cell: [
                        0.5 * (cell_index & 1) as f32,
                        0.5 * (cell_index >> 1) as f32,
                    ],
                    distance_sq: center.distance_squared(camera_position),
                });
            }
        }
        puffs.sort_unstable_by(|a, b| b.distance_sq.total_cmp(&a.distance_sq));

        let mut positions = Vec::with_capacity(puffs.len() * 4);
        let mut normals = Vec::with_capacity(puffs.len() * 4);
        let mut uvs = Vec::with_capacity(puffs.len() * 4);
        let mut colors = Vec::with_capacity(puffs.len() * 4);
        let mut indices = Vec::with_capacity(puffs.len() * 6);
        for puff in &puffs {
            let to_camera = (camera_position - puff.center).normalize_or(Vec3::Z);
            let right_dir = Vec3::Y.cross(to_camera).normalize_or(Vec3::X);
            let up_dir = to_camera.cross(right_dir).normalize_or(Vec3::Y);
            let right = right_dir * puff.half;
            let up = up_dir * puff.half;
            let first = positions.len() as u32;
            let [u, v] = puff.cell;
            for (corner, uv) in [
                (puff.center - right - up, [u, v + 0.5]),
                (puff.center + right - up, [u + 0.5, v + 0.5]),
                (puff.center + right + up, [u + 0.5, v]),
                (puff.center - right + up, [u, v]),
            ] {
                positions.push(corner.to_array());
                normals.push(to_camera.to_array());
                uvs.push(uv);
                colors.push([puff.tint[0], puff.tint[1], puff.tint[2], puff.alpha]);
            }
            indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;

    use super::*;
    use crate::controller::CollisionWorld;

    fn built_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, super::super::build_city)
            .add_systems(Update, animate_chimney_smoke);
        app.update();
        app
    }

    /// The hash gate lights a stable minority of the skyline — enough plumes
    /// to read as a lived-in city, far too few to read as a fire.
    #[test]
    fn a_hash_picked_subset_of_chimneys_smokes() {
        let mut app = built_app();
        let world = app.world_mut();
        let smoke = world
            .query::<&ChimneySmoke>()
            .single(world)
            .expect("the city spawns exactly one smoke batch");
        let plumes = smoke.plumes.len();
        assert!(
            (150..1_200).contains(&plumes),
            "smoking-chimney subset out of range: {plumes}"
        );
    }

    /// With a camera present the animator fills the one shared mesh with a
    /// camera-facing quad per live puff.
    #[test]
    fn puffs_fill_one_batched_mesh_of_camera_facing_quads() {
        let mut app = built_app();
        // MinimalPlugins has no transform propagation, so the camera's
        // GlobalTransform is provided by hand.
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(Vec3::new(0.0, 40.0, 120.0)),
        ));
        app.update();

        let world = app.world_mut();
        let (smoke, mesh_handle) = world
            .query::<(&ChimneySmoke, &Mesh3d)>()
            .single(world)
            .expect("the city spawns exactly one smoke batch");
        let expected_vertices = smoke.plumes.len() * PUFFS_PER_PLUME * 4;
        let handle = mesh_handle.0.clone();
        let mesh = world
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .expect("the smoke mesh asset exists");
        assert_eq!(mesh.count_vertices(), expected_vertices);
        let alphas: Vec<f32> = match mesh
            .attribute(Mesh::ATTRIBUTE_COLOR)
            .expect("smoke quads carry vertex colours")
        {
            bevy::mesh::VertexAttributeValues::Float32x4(colors) => {
                colors.iter().map(|color| color[3]).collect()
            }
            other => panic!("unexpected colour format: {other:?}"),
        };
        assert!(
            alphas.iter().all(|alpha| (0.0..=1.0).contains(alpha)),
            "puff alphas must stay inside the blendable range"
        );
        assert!(
            alphas.iter().any(|alpha| *alpha > 0.05),
            "at least some puffs are visibly mid-life"
        );
    }
}
