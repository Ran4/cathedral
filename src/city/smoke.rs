//! Chimney smoke: the one animated layer of the otherwise static city.
//!
//! `add_chimneys` reports every stack it plants on a ridge; a stable hash
//! picks the fraction of them whose hearths exist ([`hearth_heat`] decides
//! when each of them burns against the sim clock — lit after the Kindling,
//! covered at the Snuffing, with a scatter of bakehouses firing at the
//! Watch). Each lit stack carries a looping column of puffs — camera-facing
//! quads that rise, drift with the authoritative weather wind, swell and fade — and
//! every puff in the city is rewritten each frame into one shared mesh, so
//! the whole skyline of plumes costs a single draw call, one entity, and
//! nothing at all in navigation.

use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};

use crate::mesh_batch::{idle_batch_mesh, write_batch_mesh};

/// One stack in this many smokes; the rest are cold hearths.
const SMOKE_GATE: u32 = 4;
/// Puffs alive per plume at any moment, evenly staggered through the loop.
const PUFFS_PER_PLUME: usize = 6;
/// Seconds from leaving the flue to dissolving.
const PUFF_LIFE_S: f32 = 9.0;
/// How fast a fresh puff climbs; ageing puffs ease off as the wind takes over.
const RISE_M_PER_S: f32 = 1.0;
/// Clear fallback for isolated city tests that do not install the weather
/// projection. The game always supplies `SmoothedWeather`.
const FALLBACK_WIND: Vec2 = Vec2::new(0.8, -0.2);
/// A puff swells from flue-width to a loose cloud as it dissolves.
const PUFF_START_DIAMETER_M: f32 = 0.7;
const PUFF_END_DIAMETER_M: f32 = 3.4;
const PUFF_PEAK_ALPHA: f32 = 0.65;

/// Where a chimney flue tops out, reported by `add_chimneys` for every stack
/// whether or not it ends up smoking, plus the stack's stable hash.
pub(super) struct ChimneyAnchor {
    pub top: Vec3,
    pub seed: u32,
    /// Bakehouse-pattern hearth: fires at the Watch, hours before the Kindling.
    pub early: bool,
    /// Warehouses get a stack but never a fire; the smoking census skips them.
    pub cold: bool,
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
    /// Stable local eddy applied around the shared weather vector.
    wind_angle_jitter: f32,
    wind_speed_factor: f32,
    /// Woodsmoke runs warm-grey to blue-grey depending on the hearth.
    tint: [f32; 3],
    /// Lights at the Watch instead of the Kindling.
    early: bool,
}

/// A uniform draw from the higher bits of a stack's hash.
fn unit(seed: u32, shift: u32) -> f32 {
    ((seed >> shift) % 997) as f32 / 996.0
}

/// Whether this stack's hearth follows the bakehouse pattern — fired before
/// anyone is up. Roughly 8% of hearths, doubled where the trade wants a kiln
/// or a copper going by the Watch.
pub(super) fn early_hearth(seed: u32, use_name: &str) -> bool {
    let threshold = if matches!(use_name, "workshop" | "industrial") {
        0.16
    } else {
        0.08
    };
    unit(seed, 19) < threshold
}

/// 0.0 cold .. 1.0 full burn, for this hearth at this day fraction.
///
/// The day of a hearth, all in sim time (offices per
/// `cathedral_sim::Office::start_fraction`): ordinary hearths light 0–90
/// minutes after the Kindling (05:00) and ramp up over ~20 minutes; early
/// hearths fire 0–45 minutes after the Watch (02:00). Everyone burns flat
/// through the working day — banked, never dead — and covers 0–40 minutes
/// after the Snuffing (21:00, curfew: *couvre-feu*), ramping out over ~15
/// minutes. The night between is cold. All jitter is drawn from the stack's
/// seed, so the schedule is deterministic per hearth forever.
fn hearth_heat(day_fraction: f64, seed: u32, early: bool) -> f32 {
    const SIM_MINUTE: f64 = 1.0 / (24.0 * 60.0);
    let light_start = if early {
        2.0 / 24.0 + f64::from(unit(seed, 2)) * 45.0 * SIM_MINUTE
    } else {
        5.0 / 24.0 + f64::from(unit(seed, 6)) * 90.0 * SIM_MINUTE
    };
    let light_end = light_start + 20.0 * SIM_MINUTE;
    let douse_start = 21.0 / 24.0 + f64::from(unit(seed, 10)) * 40.0 * SIM_MINUTE;
    let douse_end = douse_start + 15.0 * SIM_MINUTE;

    // The light and douse windows never touch midnight (latest douse-end is
    // 21:55, earliest light 02:00), so only the cold gap wraps — and the
    // outside-both-ramps test below covers it on the wrapped fraction.
    let fraction = day_fraction.rem_euclid(1.0);
    let smooth = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        (t * t * (3.0 - 2.0 * t)) as f32
    };
    if fraction < light_start || fraction >= douse_end {
        0.0
    } else if fraction < light_end {
        smooth((fraction - light_start) / (light_end - light_start))
    } else if fraction < douse_start {
        1.0
    } else {
        1.0 - smooth((fraction - douse_start) / (douse_end - douse_start))
    }
}

pub(super) fn build_chimney_smoke(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    anchors: &[ChimneyAnchor],
) -> usize {
    let plumes: Vec<Plume> = anchors
        .iter()
        .filter(|anchor| !anchor.cold && anchor.seed % SMOKE_GATE == 0)
        .map(|anchor| {
            let seed = anchor.seed;
            let cool = unit(seed, 11);
            Plume {
                top: anchor.top,
                seed,
                phase: unit(seed, 15) * PUFF_LIFE_S,
                wind_angle_jitter: (unit(seed, 3) - 0.5) * 0.34,
                wind_speed_factor: 0.72 + 0.56 * unit(seed, 7),
                tint: [0.88 - 0.14 * cool, 0.86 - 0.10 * cool, 0.84 - 0.04 * cool],
                early: anchor.early,
            }
        })
        .collect();
    if plumes.is_empty() {
        return 0;
    }
    let count = plumes.len();

    commands.spawn((
        Name::new("Chimney smoke"),
        // The batch starts on the idle triangle: every attribute the live
        // layout has, and something to draw even on a frame with no camera to
        // billboard toward.
        Mesh3d(meshes.add(idle_batch_mesh())),
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
    clock: Option<Res<crate::smart_actors::WorldClockState>>,
    weather: Option<Res<crate::weather::SmoothedWeather>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    smoke: Query<(&ChimneySmoke, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let camera_position = camera.translation();
    let elapsed = time.elapsed_secs();
    let (weather_wind, weather_gust, precipitation) = weather
        .as_deref()
        .map_or((FALLBACK_WIND, 0.0, 0.0), |weather| {
            (weather.wind, weather.gust, weather.precipitation)
        });
    // The hearth schedule needs the sim clock — a read-only projection, absent
    // in headless city tests and silent until the engine's first message. In
    // either case every hearth burns at full, exactly the pre-clock skyline:
    // the sky is only ever dimmed by information, never by its absence.
    let clock = clock
        .filter(|clock| clock.present)
        .map(|clock| (clock.fraction, clock.scale / clock.seconds_per_day.max(1.0)));

    struct Puff {
        center: Vec3,
        half: f32,
        alpha: f32,
        tint: [f32; 3],
        cell: [f32; 2],
        distance_sq: f32,
    }

    for (smoke, mesh_handle) in &smoke {
        let mut puffs = Vec::with_capacity(smoke.plumes.len() * PUFFS_PER_PLUME);
        for plume in &smoke.plumes {
            let drift = plume_drift(weather_wind, weather_gust, elapsed, plume);
            let across = drift.perp().normalize_or_zero();
            for index in 0..PUFFS_PER_PLUME {
                let slot = index as f32 * (PUFF_LIFE_S / PUFFS_PER_PLUME as f32);
                let age = (elapsed + plume.phase + slot).rem_euclid(PUFF_LIFE_S);
                // Heat is sampled when the puff left the flue, not now: a
                // doused fire doesn't delete its airborne smoke, the plume
                // runs out bottom-up as the last puffs live out their nine
                // seconds — and lights flue-first at the Kindling.
                let heat = match clock {
                    None => 1.0,
                    Some((day_fraction, day_per_wall_second)) => hearth_heat(
                        day_fraction - f64::from(age) * day_per_wall_second,
                        plume.seed,
                        plume.early,
                    ),
                };
                if heat <= 0.0 {
                    continue;
                }
                let f = age / PUFF_LIFE_S;
                // Straight up out of the flue, then the wind takes it: the
                // climb eases as the drift grows quadratically into a bend.
                let rise = RISE_M_PER_S * age * (1.0 - 0.22 * f);
                let bend = drift * (age * f);
                let sway = across
                    * ((elapsed * (0.7 + weather_gust * 1.4)
                        + plume.phase * 3.1
                        + index as f32 * 1.9)
                        .sin()
                        * (0.35 + weather_gust * 0.42)
                        * f);
                let center =
                    plume.top + Vec3::Y * rise + Vec3::new(bend.x + sway.x, 0.0, bend.y + sway.y);
                let fade_in = (f / 0.12).min(1.0);
                let fade_out = (1.0 - f) * (1.0 - f).sqrt();
                let diameter = (PUFF_START_DIAMETER_M
                    + (PUFF_END_DIAMETER_M - PUFF_START_DIAMETER_M) * f.powf(0.7))
                    * (1.0 + weather_gust * 0.32 * f);
                let cell_index = (plume.seed >> (2 * index as u32)) & 3;
                puffs.push(Puff {
                    center,
                    half: diameter * 0.5,
                    alpha: PUFF_PEAK_ALPHA
                        * fade_in
                        * fade_out
                        * heat
                        * (1.0 - precipitation * 0.28),
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
        // Past the Snuffing the whole city is cold and this frame has no
        // quads at all; the batch parks on its idle triangle until dawn.
        write_batch_mesh(
            &mut meshes,
            &mesh_handle.0,
            positions,
            normals,
            uvs,
            colors,
            indices,
        );
    }
}

/// Shared wind direction with a small, stable flue eddy and a common gust
/// pulse. The dot product against `weather_wind` remains positive, which is the
/// invariant rain, clouds and smoke rely on to move in the same direction.
fn plume_drift(weather_wind: Vec2, gust: f32, elapsed: f32, plume: &Plume) -> Vec2 {
    let base =
        Vec2::from_angle(plume.wind_angle_jitter).rotate(weather_wind) * plume.wind_speed_factor;
    let pulse = 1.0
        + gust
            * (0.25 + 0.35 * (elapsed * 0.83 + plume.phase + plume.seed as f32 * 0.000_013).sin());
    base * pulse.max(0.55)
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;

    use super::*;
    use crate::{controller::CollisionWorld, mesh_batch::IDLE_BATCH_VERTICES};

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

    fn smoke_mesh_vertices(app: &mut App) -> usize {
        let world = app.world_mut();
        let (_, mesh_handle) = world
            .query::<(&ChimneySmoke, &Mesh3d)>()
            .single(world)
            .expect("the city spawns exactly one smoke batch");
        let handle = mesh_handle.0.clone();
        world
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .expect("the smoke mesh asset exists")
            .count_vertices()
    }

    /// The schedule from the feature table: cold night, lit morning, flat
    /// working day, covered after the Snuffing — early hearths hours ahead,
    /// everything deterministic per seed and safe across the midnight wrap.
    #[test]
    fn hearth_heat_follows_the_day_of_a_hearth() {
        let minute = 1.0 / (24.0 * 60.0);
        for seed in [0u32, 0xdead_beef, 0x1234_5678, u32::MAX] {
            for early in [false, true] {
                // Midnight is cold, High Wick is full burn, and past the last
                // possible douse-end (21:55) the night is cold again.
                assert_eq!(hearth_heat(0.0, seed, early), 0.0);
                assert_eq!(hearth_heat(0.5, seed, early), 1.0);
                assert_eq!(hearth_heat(22.0 / 24.0, seed, early), 0.0);
                // Wrapping: a back-dated birth before midnight lands in the
                // previous evening, a fraction past 1.0 in the next morning.
                assert_eq!(
                    hearth_heat(-0.5 / 24.0, seed, early),
                    hearth_heat(23.5 / 24.0, seed, early)
                );
                assert_eq!(hearth_heat(1.5, seed, early), hearth_heat(0.5, seed, early));
                // Determinism: the same seed draws the same schedule forever.
                assert_eq!(
                    hearth_heat(5.75 / 24.0, seed, early),
                    hearth_heat(5.75 / 24.0, seed, early)
                );
            }
            // At 03:30 an early hearth is already at full burn (latest light
            // 02:45 plus the 20-minute ramp) while an ordinary one is cold.
            assert_eq!(hearth_heat(3.5 / 24.0, seed, true), 1.0);
            assert_eq!(hearth_heat(3.5 / 24.0, seed, false), 0.0);
            // Through the ordinary light window (05:00 → 06:50) the ramp is
            // monotonic, ends at full burn, and passes through the middle.
            let samples: Vec<f32> = (0..=110)
                .map(|m| hearth_heat(5.0 / 24.0 + m as f64 * minute, seed, false))
                .collect();
            assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
            assert_eq!(*samples.last().unwrap(), 1.0);
            assert!(
                samples
                    .iter()
                    .any(|heat| (0.0..1.0).contains(heat) && *heat > 0.0)
            );
        }
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
        // A scattered minority of the census fires at the Watch — enough for
        // a pre-dawn skyline, nowhere near enough to read as morning.
        let early = smoke.plumes.iter().filter(|plume| plume.early).count();
        assert!(
            early * 100 >= plumes * 2 && early * 100 <= plumes * 30,
            "early-hearth share out of range: {early} of {plumes}"
        );
    }

    #[test]
    fn local_eddies_never_reverse_the_authoritative_wind() {
        let mut app = built_app();
        let world = app.world_mut();
        let smoke = world
            .query::<&ChimneySmoke>()
            .single(world)
            .expect("the city spawns smoke");
        let wind = Vec2::new(5.0, -2.5);
        for plume in smoke.plumes.iter().take(256) {
            let drift = plume_drift(wind, 1.0, 37.0, plume);
            assert!(drift.dot(wind) > 0.0, "{drift:?} reversed {wind:?}");
        }
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

    /// The clock gates the skyline: full at High Wick, out at 23:00 — every
    /// hearth covered by 21:55, and at the default pace a nine-second puff
    /// life back-dates births by under four sim-minutes, still far past the
    /// douse — and back to the familiar full burn when no clock has spoken.
    #[test]
    fn the_snuffing_empties_the_smoke_mesh() {
        use crate::smart_actors::WorldClockState;

        let mut app = built_app();
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(Vec3::new(0.0, 40.0, 120.0)),
        ));
        app.insert_resource(WorldClockState {
            present: true,
            fraction: 0.5,
            ..Default::default()
        });
        app.update();
        let world = app.world_mut();
        let smoke = world
            .query::<&ChimneySmoke>()
            .single(world)
            .expect("the city spawns exactly one smoke batch");
        let daytime_vertices = smoke.plumes.len() * PUFFS_PER_PLUME * 4;
        assert_eq!(smoke_mesh_vertices(&mut app), daytime_vertices);

        app.insert_resource(WorldClockState {
            present: true,
            fraction: 23.0 / 24.0,
            ..Default::default()
        });
        app.update();
        // Not zero: a doused city parks the batch on the idle triangle, which
        // the mesh allocator can actually allocate (see `crate::mesh_batch`).
        assert_eq!(smoke_mesh_vertices(&mut app), IDLE_BATCH_VERTICES);

        app.world_mut().remove_resource::<WorldClockState>();
        app.update();
        assert_eq!(smoke_mesh_vertices(&mut app), daytime_vertices);
    }
}
