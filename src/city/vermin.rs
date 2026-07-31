//! Rats in the fish lanes and the slaughter courts.
//!
//! Roughly fifty ordinary rats in eight authored colonies — drawn and heard,
//! never simulated: no per-rat entities, no colliders, no nav rebake, no sim
//! verb, and an ordinary rat never costs a token. The chimney-smoke pattern at
//! smaller scale: one `Vermin` entity, one batched mesh rewritten per frame,
//! and per-rat motion a pure function of the clock — each rat's
//! sprint–pause–sprint waypoint loop is baked once at startup from
//! `(colony seed, rat index)` and validated against the same navigation and
//! collision the player walks, so a rat is confined by *reading* the world,
//! never by writing a collider into it (`collision_footprints.json` stays
//! byte-identical). The only mutable state is one scatter impulse per colony:
//! rats that ignore you are wallpaper; rats that flee you are alive.

use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};
use cathedral_sim::{NavData, WeatherKind};

use crate::{
    config::VerminSettings,
    controller::{CollisionWorld, PlayerController},
    mesh_batch::{idle_batch_mesh, write_batch_mesh},
    smart_actors::{WorldClockState, actors::ActorView},
    weather::WorldWeatherState,
};

/// Rats past this range are skipped entirely: a 25 cm body is sub-pixel long
/// before the smoke's 450 m, so the frame cost is a few dozen quads near the
/// player and the idle triangle everywhere else.
const VERMIN_VISIBLE_RANGE_M: f32 = 60.0;
/// A player or puppet inside this range of a rat kicks its colony's scatter.
const SCATTER_TRIGGER_M: f32 = 2.5;
/// Feet above this are not feet on the ground: the developer flying over the
/// Shambles is not standing in it, and a scatter is a reaction to a footfall.
/// The player transform is eye height (~1.6 m, the `tp` vantage), so this
/// clears a standing body and stops well short of any flight or bridge deck.
const SCATTER_MAX_FOOT_Y: f32 = 2.5;
/// Rats farther than this from the impulse point ignore it.
const SCATTER_REACH_M: f32 = 4.0;
/// The whole impulse: a fast dart out, a hold, and a wary creep back.
const SCATTER_TOTAL_S: f32 = 3.0;
/// How far the dart carries at full effect.
const SCATTER_FLEE_M: f32 = 1.7;
/// Where a rat's feet sit: just above the 0.012 m road and site surfaces.
const RAT_GROUND_Y: f32 = 0.03;
/// The height colliders are probed at when a waypoint is validated — inside a
/// wall or crate, above kerbs and thresholds a rat may cross.
const RAT_PROBE_Y: f32 = 0.15;

/// One authored colony. This table is the whole population: no reproduction,
/// no migration, no procedural placement — the same spirit as the fixed
/// hand-authored cast. The three all-office colonies are exactly the three
/// authored `FliesAtWaste` piles: the flies and the rats mark the same filth.
struct ColonySpec {
    name: &'static str,
    anchor: Vec2,
    rats: usize,
    radius_m: f32,
    /// `true` runs all offices; `false` is the `WarmDayWaste` inverse —
    /// visible only while the city is dark, the Snuffing to the Kindling.
    all_offices: bool,
}

const COLONIES: [ColonySpec; 8] = [
    ColonySpec {
        name: "the Shambles",
        anchor: Vec2::new(-294.0, 220.0),
        rats: 10,
        radius_m: 14.0,
        all_offices: true,
    },
    ColonySpec {
        name: "Maren's Green, landing edge",
        anchor: Vec2::new(-214.0, -255.0),
        rats: 8,
        radius_m: 12.0,
        all_offices: true,
    },
    ColonySpec {
        name: "Tanners' Slip",
        anchor: Vec2::new(-296.0, -229.0),
        rats: 6,
        radius_m: 10.0,
        all_offices: true,
    },
    ColonySpec {
        name: "Eelback Alley",
        anchor: Vec2::new(-275.0, -330.0),
        rats: 6,
        radius_m: 9.0,
        all_offices: false,
    },
    ColonySpec {
        name: "the Old Sluice",
        // The feature table's (−213, −427) is the *middle of the sluice's
        // shell* — `areas.json: old_sluice` is a solid block from z −448 to
        // z −406 and there is no walkable ground anywhere inside it. The
        // colony belongs where the doc says it does, at the dry grate: the
        // blocked dry arches face north at `z −405.86`
        // (`build_old_sluice_face`), so the anchor sits just off them
        // on the Cut's own centreline (`CUT_CENTRE_X`), in the street's
        // southernmost laid reach.
        anchor: Vec2::new(-213.5, -400.0),
        rats: 6,
        radius_m: 10.0,
        all_offices: false,
    },
    ColonySpec {
        name: "Gaunt Passage",
        anchor: Vec2::new(-155.0, 17.0),
        rats: 4,
        radius_m: 8.0,
        all_offices: false,
    },
    ColonySpec {
        name: "the Seven Lofts skirts",
        anchor: Vec2::new(252.0, 234.0),
        rats: 4,
        radius_m: 8.0,
        all_offices: false,
    },
    ColonySpec {
        name: "the Wickmarket",
        anchor: Vec2::new(-14.0, 249.0),
        rats: 6,
        radius_m: 12.0,
        all_offices: false,
    },
];

/// One dogleg of a rat's loop: sitting at `from` until `depart`, then a
/// straight sprint landing at `to` on `arrive` (the next leg's start).
struct Leg {
    depart: f32,
    arrive: f32,
    from: Vec2,
    to: Vec2,
    heading: Vec2,
}

struct Rat {
    seed: u64,
    legs: Vec<Leg>,
    period: f32,
    /// Offset into the loop, so a colony never darts in unison.
    phase: f32,
    length_m: f32,
    /// Coat brightness jitter around the shared dark brown.
    tint: f32,
}

struct Colony {
    name: &'static str,
    anchor: Vec2,
    radius_m: f32,
    all_offices: bool,
    rats: Vec<Rat>,
    /// Where somebody's foot fell and when — the one piece of mutable state.
    scatter: Option<(Vec2, f32)>,
}

/// The whole city's rats: one entity, one mesh rebuilt per frame.
#[derive(Component)]
pub(super) struct Vermin {
    colonies: Vec<Colony>,
    /// The committed navigation bake, read (like the puddles read it) to keep
    /// every waypoint and every scatter dart on ground the player can walk.
    nav: NavData,
}

/// splitmix64's finalizer: the per-rat determinism everything draws from.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// A uniform draw in `[0, 1)` from a seed and a stream index.
fn unit(seed: u64, stream: u64) -> f32 {
    (mix(seed ^ stream.wrapping_mul(0x9e37_79b9_7f4a_7c15)) >> 40) as f32 / (1u64 << 24) as f32
}

fn walkable(nav: &NavData, collision: &CollisionWorld, point: Vec2) -> bool {
    nav.is_walkable(f64::from(point.x), f64::from(point.y))
        && !collision.contains_point(Vec3::new(point.x, RAT_PROBE_Y, point.y))
}

/// Bakes one rat's loop: a home draw inside the colony disc, then a random
/// walk of sprint-sized steps, every point validated against nav + collision.
/// A rat whose colony offers no walkable ground at all is skipped, not forced.
fn bake_rat(
    nav: &NavData,
    collision: &CollisionWorld,
    anchor: Vec2,
    radius_m: f32,
    seed: u64,
) -> Option<Rat> {
    let home = (0..40).find_map(|try_index| {
        let angle = unit(seed, 100 + try_index) * std::f32::consts::TAU;
        let range = radius_m * unit(seed, 200 + try_index).sqrt();
        let candidate = anchor + Vec2::from_angle(angle) * range;
        walkable(nav, collision, candidate).then_some(candidate)
    })?;

    // 4–8 points; each step is drawn as a sprint (1.8–2.6 m/s for 0.4–1.2 s)
    // and lands wherever that carries, so the loop stays local: short darts,
    // a pause, gone again — never a trek across the colony.
    let waypoint_count = 4 + (mix(seed ^ 0x77) % 5) as usize;
    let mut waypoints = vec![home];
    for index in 1..waypoint_count {
        let base = 1000 + index as u64 * 20;
        let previous = *waypoints.last().expect("the loop starts at home");
        let step = (0..12).find_map(|try_index| {
            let stream = base + try_index;
            let angle = unit(seed, stream) * std::f32::consts::TAU;
            let speed = 1.8 + 0.8 * unit(seed, stream + 3000);
            let sprint_s = 0.4 + 0.8 * unit(seed, stream + 6000);
            let candidate = previous + Vec2::from_angle(angle) * (speed * sprint_s);
            (candidate.distance(anchor) <= radius_m && walkable(nav, collision, candidate))
                .then_some(candidate)
        });
        // A cornered rat darts home rather than through a wall.
        waypoints.push(step.unwrap_or(home));
    }

    let mut legs = Vec::with_capacity(waypoints.len());
    let mut now = 0.0_f32;
    let mut heading = Vec2::X;
    for (index, from) in waypoints.iter().enumerate() {
        let to = waypoints[(index + 1) % waypoints.len()];
        let stream = 9000 + index as u64 * 7;
        // Weighted long: most of a rat's life is the pause.
        let pause_s = 0.5 + 3.5 * unit(seed, stream).powf(0.6);
        let speed = 1.8 + 0.8 * unit(seed, stream + 1);
        let distance = from.distance(to);
        let direction = (to - *from) / distance.max(0.001);
        if distance > 0.01 {
            heading = direction;
        }
        let depart = now + pause_s;
        let arrive = depart + (distance / speed).max(0.02);
        legs.push(Leg {
            depart,
            arrive,
            from: *from,
            to,
            heading,
        });
        now = arrive;
    }

    Some(Rat {
        seed,
        legs,
        period: now,
        phase: unit(seed, 31) * now,
        length_m: 0.24 + 0.08 * unit(seed, 32),
        tint: 0.8 + 0.35 * unit(seed, 33),
    })
}

impl Rat {
    /// Position, facing and gait on the loop at wall-clock `elapsed` — a pure
    /// function of time, like a smoke puff on its arc.
    fn sample(&self, elapsed: f32) -> (Vec2, Vec2, bool) {
        let t = (elapsed + self.phase).rem_euclid(self.period.max(0.001));
        for leg in &self.legs {
            if t < leg.depart {
                return (leg.from, leg.heading, false);
            }
            if t < leg.arrive {
                let f = (t - leg.depart) / (leg.arrive - leg.depart);
                return (leg.from.lerp(leg.to, f), leg.heading, true);
            }
        }
        let last = self.legs.last().expect("a baked rat has legs");
        (last.to, last.heading, false)
    }
}

/// Colony visibility against the sim clock: the three waste-pile colonies run
/// all offices; the rest are the soundscape's `WarmDayWaste` inverse — out
/// only while the city is dark. No clock yet (the seconds before the engine
/// speaks, a headless city test) shows everything: the same information-only
/// dimming the chimney smoke practices.
fn colony_active(all_offices: bool, clock: Option<&WorldClockState>) -> bool {
    all_offices
        || clock
            .filter(|clock| clock.present)
            .is_none_or(|clock| clock.brightness <= 0.30)
}

/// Heavy rain sends the rats under cover with the rest of the animals — the
/// same threshold `soundscape::wildlife_suppressed` holds its fauna to.
fn rain_suppressed(weather: Option<&WorldWeatherState>) -> bool {
    weather.is_some_and(|weather| {
        weather.current.precipitation >= 0.62
            || matches!(
                weather.current.kind,
                WeatherKind::Downpour | WeatherKind::Thunderstorm
            )
    })
}

/// Spawns the one vermin batch. Registered after `build_city` so the baked
/// waypoints validate against the fully populated `CollisionWorld`; with
/// `vermin.enabled: false` (or `CATHEDRAL_NO_VERMIN`) nothing spawns and the
/// per-frame rebuild never runs.
pub(super) fn spawn_vermin(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    collision: Res<CollisionWorld>,
    settings: Option<Res<VerminSettings>>,
) {
    let settings = settings.as_deref().cloned().unwrap_or_default();
    if !settings.enabled {
        return;
    }
    let nav = NavData::from_parts(
        include_str!("../../assets/world/navigation.json"),
        include_bytes!("../../assets/world/navigation.bin"),
    )
    .expect("the committed navigation bake already validates at startup");

    let mut total = 0_usize;
    let colonies: Vec<Colony> = COLONIES
        .iter()
        .enumerate()
        .map(|(colony_index, spec)| {
            let count = (spec.rats as f32 * settings.density).round().max(0.0) as usize;
            let rats: Vec<Rat> = (0..count)
                .filter_map(|rat_index| {
                    let seed = mix(settings
                        .seed
                        .wrapping_add((colony_index as u64) << 32)
                        .wrapping_add(rat_index as u64));
                    bake_rat(&nav, &collision, spec.anchor, spec.radius_m, seed)
                })
                .collect();
            if rats.is_empty() && count > 0 {
                warn!(
                    "[vermin] {} offers no walkable ground around {:?}",
                    spec.name, spec.anchor
                );
            }
            total += rats.len();
            Colony {
                name: spec.name,
                anchor: spec.anchor,
                radius_m: spec.radius_m,
                all_offices: spec.all_offices,
                rats,
                scatter: None,
            }
        })
        .collect();

    for colony in &colonies {
        debug!("[vermin] {}: {} rats", colony.name, colony.rats.len());
    }
    info!(
        "[vermin] {total} rats settled across {} colonies",
        colonies.len()
    );
    commands.spawn((
        Name::new("Vermin colonies"),
        Mesh3d(meshes.add(idle_batch_mesh())),
        // Flat dark-brown vertex colour, no texture: at 25 cm and 2 m/s the
        // rat reads by motion, not by texel.
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            // A deliberate deviation from §2.2's "alpha-tested", which was
            // written for a textured rat: there is no texture and every vertex
            // alpha is 1.0, so a mask has nothing to discard and only costs the
            // batch a trip through the alpha-mask pipeline.
            alpha_mode: AlphaMode::Opaque,
            perceptual_roughness: 1.0,
            reflectance: 0.02,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        // The batch spans the city and rewrites itself every frame; a baked
        // AABB would lie.
        NoFrustumCulling,
        NotShadowCaster,
        Vermin { colonies, nav },
    ));
}

/// Anyone's feet — the player's or a puppet's — on the ground and inside
/// [`SCATTER_TRIGGER_M`] of a rat kicks that colony's scatter impulse. One
/// impulse at a time per colony: while it plays out nothing re-arms, so a
/// visitor standing their ground sets off a fresh dart every few seconds
/// rather than a shiver. Colonies the clock has put away are skipped whole.
pub(super) fn trigger_vermin_scatter(
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    player: Query<&Transform, With<PlayerController>>,
    actors: Query<&GlobalTransform, With<ActorView>>,
    mut vermin: Query<&mut Vermin>,
) {
    let elapsed = time.elapsed_secs();
    let clock = clock.as_deref();
    // Only somebody standing on the ground is a footfall: a player in
    // developer flight, or crossing one of the city's overhead bridges, passes
    // over a colony without ever putting a foot near it.
    let movers: Vec<Vec2> = player
        .iter()
        .map(|transform| transform.translation)
        .chain(actors.iter().map(|transform| transform.translation()))
        .filter(|translation| translation.y <= SCATTER_MAX_FOOT_Y)
        .map(|translation| translation.xz())
        .collect();
    if movers.is_empty() {
        return;
    }
    for mut vermin in &mut vermin {
        for colony in &mut vermin.colonies {
            // A colony nobody can see is a colony nobody can startle; skipping
            // it also spares the `rat.sample()` sweep below.
            if !colony_active(colony.all_offices, clock) {
                colony.scatter = None;
                continue;
            }
            if let Some((_, started)) = colony.scatter {
                if elapsed - started < SCATTER_TOTAL_S {
                    continue;
                }
                colony.scatter = None;
            }
            let near: Vec<Vec2> = movers
                .iter()
                .copied()
                .filter(|mover| mover.distance(colony.anchor) <= colony.radius_m + SCATTER_REACH_M)
                .collect();
            if near.is_empty() {
                continue;
            }
            'rats: for rat in &colony.rats {
                let (position, _, _) = rat.sample(elapsed);
                for mover in &near {
                    if mover.distance_squared(position) <= SCATTER_TRIGGER_M * SCATTER_TRIGGER_M {
                        colony.scatter = Some((*mover, elapsed));
                        break 'rats;
                    }
                }
            }
        }
    }
}

/// The dart-and-creep-back envelope: a fast attack, a hold, then a slow
/// release as the rat steals back to its loop.
fn scatter_envelope(age: f32) -> f32 {
    let smooth = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    smooth(age / 0.35) * (1.0 - smooth((age - 1.5) / (SCATTER_TOTAL_S - 1.5)))
}

/// Where the scatter impulse pushes this rat right now: away from the foot,
/// jittered per rat, faded by distance, and never off the walkable surface.
/// A dart is a position a rat occupies, so it answers to the whole of §2.1 —
/// nav *and* collision — exactly as the baked waypoints do.
fn scatter_offset(
    nav: &NavData,
    collision: &CollisionWorld,
    rat: &Rat,
    position: Vec2,
    impulse: Option<(Vec2, f32)>,
    elapsed: f32,
) -> Vec2 {
    let Some((from, started)) = impulse else {
        return Vec2::ZERO;
    };
    let age = elapsed - started;
    if !(0.0..SCATTER_TOTAL_S).contains(&age) {
        return Vec2::ZERO;
    }
    let distance = position.distance(from);
    if distance > SCATTER_REACH_M {
        return Vec2::ZERO;
    }
    let envelope = scatter_envelope(age) * (1.0 - distance / SCATTER_REACH_M).sqrt();
    if envelope <= 0.001 {
        return Vec2::ZERO;
    }
    let away = (position - from)
        .normalize_or(Vec2::from_angle(unit(rat.seed, 40) * std::f32::consts::TAU));
    let jitter = (unit(rat.seed, 41) - 0.5) * 0.9;
    let direction = Vec2::from_angle(jitter).rotate(away);
    let reach = SCATTER_FLEE_M * (0.75 + 0.5 * unit(rat.seed, 42)) * envelope;
    // A dart that would carry into a wall or a crate pulls up short instead.
    for fraction in [1.0, 0.5, 0.25] {
        let offset = direction * reach * fraction;
        if walkable(nav, collision, position + offset) {
            return offset;
        }
    }
    Vec2::ZERO
}

/// Rewrites the shared vermin mesh: every visible rat placed on its loop (or
/// its scatter dart), an oriented tent of two quads and a tail — deliberately
/// not camera-facing billboards, because a ground creature seen from a bridge
/// or in developer flight must not turn to face down.
pub(super) fn animate_vermin(
    time: Res<Time>,
    clock: Option<Res<WorldClockState>>,
    weather: Option<Res<WorldWeatherState>>,
    collision: Res<CollisionWorld>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    vermin: Query<(&Vermin, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let camera_position = camera.translation();
    let elapsed = time.elapsed_secs();
    let clock = clock.as_deref();
    let suppressed = rain_suppressed(weather.as_deref());

    for (vermin, mesh_handle) in &vermin {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();

        for colony in &vermin.colonies {
            if !colony_active(colony.all_offices, clock) {
                continue;
            }
            let colony_range = VERMIN_VISIBLE_RANGE_M + colony.radius_m;
            if colony.anchor.distance_squared(camera_position.xz()) > colony_range * colony_range {
                continue;
            }
            for (rat_index, rat) in colony.rats.iter().enumerate() {
                // Heavy rain thins the colony to a stray third, matching the
                // animals going quiet in the soundscape.
                if suppressed && rat_index % 3 != 0 {
                    continue;
                }
                let (loop_position, loop_heading, sprinting) = rat.sample(elapsed);
                let offset = scatter_offset(
                    &vermin.nav,
                    &collision,
                    rat,
                    loop_position,
                    colony.scatter,
                    elapsed,
                );
                let fleeing = offset.length_squared() > 0.0025;
                let position = loop_position + offset;
                let heading = if fleeing {
                    offset.normalize_or(loop_heading)
                } else {
                    loop_heading
                };
                push_rat(
                    &mut positions,
                    &mut normals,
                    &mut uvs,
                    &mut colors,
                    &mut indices,
                    rat,
                    position,
                    heading,
                    sprinting || fleeing,
                    elapsed,
                );
            }
        }
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

/// One rat: a body ridge ~0.28 m long and ~0.09 m high as a two-quad tent
/// rotated to its heading, plus a single tail quad trailing low behind.
#[allow(clippy::too_many_arguments)]
fn push_rat(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    rat: &Rat,
    position: Vec2,
    heading: Vec2,
    moving: bool,
    elapsed: f32,
) {
    let center = Vec3::new(position.x, RAT_GROUND_Y, position.y);
    let ahead = Vec3::new(heading.x, 0.0, heading.y);
    let side = Vec3::new(-heading.y, 0.0, heading.x);
    let half_length = rat.length_m * 0.5;
    let half_width = 0.05 * (rat.length_m / 0.28);
    // A slight body bob while sprinting — the scurry gait, nothing more.
    let bob = if moving {
        1.0 + 0.16 * (elapsed * 21.0 + rat.phase * 7.0).sin()
    } else {
        1.0
    };
    let height = 0.09 * bob;

    let tint = rat.tint;
    let coat = [0.105 * tint, 0.082 * tint, 0.064 * tint, 1.0];
    let tail_color = [0.125 * tint, 0.094 * tint, 0.082 * tint, 1.0];

    let nose = center + ahead * half_length;
    let rump = center - ahead * half_length;
    let flank = side * half_width;
    // The ridge sits back of the nose and high over the hindquarters.
    let ridge_front = center + ahead * (half_length * 0.55) + Vec3::Y * (height * 0.78);
    let ridge_back = center - ahead * (half_length * 0.72) + Vec3::Y * height;

    push_quad(
        positions,
        normals,
        uvs,
        colors,
        indices,
        [nose - flank, rump - flank, ridge_back, ridge_front],
        coat,
    );
    push_quad(
        positions,
        normals,
        uvs,
        colors,
        indices,
        [nose + flank, rump + flank, ridge_back, ridge_front],
        coat,
    );

    let tail_root = rump + Vec3::Y * 0.024;
    let tail_tip = rump - ahead * (rat.length_m * 0.46) + Vec3::Y * 0.006;
    push_quad(
        positions,
        normals,
        uvs,
        colors,
        indices,
        [
            tail_root - side * 0.012,
            tail_root + side * 0.012,
            tail_tip + side * 0.004,
            tail_tip - side * 0.004,
        ],
        tail_color,
    );
}

fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    corners: [Vec3; 4],
    color: [f32; 4],
) {
    let normal = (corners[1] - corners[0])
        .cross(corners[3] - corners[0])
        .normalize_or(Vec3::Y);
    let first = positions.len() as u32;
    positions.extend(corners.map(|corner| corner.to_array()));
    normals.extend([normal.to_array(); 4]);
    uvs.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    colors.extend([color; 4]);
    indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;

    use super::*;
    use crate::mesh_batch::IDLE_BATCH_VERTICES;

    fn built_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<crate::materials::WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, (super::super::build_city, spawn_vermin).chain())
            .add_systems(Update, animate_vermin);
        app.update();
        app
    }

    fn committed_nav() -> NavData {
        NavData::from_parts(
            include_str!("../../assets/world/navigation.json"),
            include_bytes!("../../assets/world/navigation.bin"),
        )
        .expect("the committed navigation bake parses")
    }

    fn vermin_mesh_vertices(app: &mut App) -> usize {
        let world = app.world_mut();
        let (_, mesh_handle) = world
            .query::<(&Vermin, &Mesh3d)>()
            .single(world)
            .expect("the city spawns exactly one vermin batch");
        let handle = mesh_handle.0.clone();
        world
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .expect("the vermin mesh asset exists")
            .count_vertices()
    }

    /// The binding decision in §2.1: rats are confined by reading the world.
    /// Every baked waypoint stands on the player's walkable surface and inside
    /// no collider — and the colony table actually settles its population,
    /// which is the canary for an anchor drifting off walkable ground.
    #[test]
    fn every_waypoint_is_walkable_and_uncollided() {
        let mut app = built_app();
        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("the city spawns exactly one vermin batch");
        let collision = world.resource::<CollisionWorld>();
        let mut total = 0;
        for colony in &vermin.colonies {
            assert!(
                !colony.rats.is_empty(),
                "{} settled no rats — anchor off walkable ground?",
                colony.name
            );
            for rat in &colony.rats {
                total += 1;
                assert!(!rat.legs.is_empty());
                assert!(rat.period > 0.0);
                for leg in &rat.legs {
                    for point in [leg.from, leg.to] {
                        assert!(
                            walkable(&vermin.nav, collision, point),
                            "{}: waypoint {point:?} is off the walkable surface",
                            colony.name
                        );
                        assert!(
                            point.distance(colony.anchor) <= colony.radius_m + 0.001,
                            "{}: waypoint {point:?} left the colony",
                            colony.name
                        );
                    }
                }
            }
        }
        // The authored table sums to 50; a few rejected draws are tolerable,
        // a hollowed-out population is not.
        assert!(
            (40..=50).contains(&total),
            "population out of range: {total}"
        );
    }

    /// A rat's loop is a pure function of time: deterministic per seed, and
    /// periodic — the same clock reading always finds the same rat in the
    /// same place, which is what lets the batch carry no per-rat state.
    #[test]
    fn a_rat_loop_is_deterministic_and_periodic() {
        let nav = committed_nav();
        let collision = CollisionWorld::default();
        let anchor = Vec2::new(-294.0, 220.0);
        let first = bake_rat(&nav, &collision, anchor, 14.0, 41).expect("the Shambles is walkable");
        let second = bake_rat(&nav, &collision, anchor, 14.0, 41).expect("same seed, same rat");
        assert_eq!(first.legs.len(), second.legs.len());
        for t in [0.0_f32, 1.7, 13.4, 200.2] {
            assert_eq!(first.sample(t).0, second.sample(t).0);
            let (position, _, _) = first.sample(t);
            let (wrapped, _, _) = first.sample(t + first.period);
            assert!(position.distance(wrapped) < 1e-3);
        }
        // Sprint-pause texture: some instants move, some sit.
        let samples: Vec<bool> = (0..200).map(|i| first.sample(i as f32 * 0.1).2).collect();
        assert!(samples.iter().any(|moving| *moving));
        assert!(samples.iter().any(|moving| !*moving));
    }

    /// The three waste-pile colonies run all offices; the rest are the
    /// `WarmDayWaste` inverse, and a missing clock dims nothing.
    #[test]
    fn night_colonies_follow_the_dark() {
        let day = WorldClockState {
            present: true,
            brightness: 1.0,
            ..Default::default()
        };
        let night = WorldClockState {
            present: true,
            brightness: 0.05,
            ..Default::default()
        };
        assert!(colony_active(true, Some(&day)));
        assert!(colony_active(true, Some(&night)));
        assert!(!colony_active(false, Some(&day)));
        assert!(colony_active(false, Some(&night)));
        assert!(colony_active(false, None));
        assert!(colony_active(
            false,
            Some(&WorldClockState {
                present: false,
                brightness: 1.0,
                ..Default::default()
            })
        ));
    }

    /// With a camera over the Shambles the batch holds exactly the in-range
    /// active colonies' rats — three quads each — by day; over a night colony
    /// at High Wick it parks on the idle triangle until dark.
    #[test]
    fn the_clock_and_the_cull_gate_the_batch() {
        let mut app = built_app();
        let shambles = Vec3::new(-294.0, 6.0, 220.0);
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(shambles),
        ));
        app.insert_resource(WorldClockState {
            present: true,
            fraction: 0.5,
            brightness: 1.0,
            ..Default::default()
        });
        app.update();
        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("one vermin batch");
        let expected: usize = vermin
            .colonies
            .iter()
            .filter(|colony| {
                colony.all_offices
                    && colony.anchor.distance(shambles.xz())
                        <= VERMIN_VISIBLE_RANGE_M + colony.radius_m
            })
            .map(|colony| colony.rats.len() * 3 * 4)
            .sum();
        assert!(expected > 0, "the Shambles colony is in range by day");
        assert_eq!(vermin_mesh_vertices(&mut app), expected);

        // Gaunt Passage is a night colony: empty batch at High Wick, rats
        // once the city is dark.
        let gaunt = Vec3::new(-155.0, 6.0, 17.0);
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<&mut GlobalTransform, With<Camera3d>>();
        *cameras.single_mut(world).expect("one camera") = GlobalTransform::from_translation(gaunt);
        app.update();
        assert_eq!(vermin_mesh_vertices(&mut app), IDLE_BATCH_VERTICES);

        app.insert_resource(WorldClockState {
            present: true,
            fraction: 23.0 / 24.0,
            brightness: 0.05,
            ..Default::default()
        });
        app.update();
        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("one vermin batch");
        let expected_night: usize = vermin
            .colonies
            .iter()
            .filter(|colony| {
                colony.anchor.distance(gaunt.xz()) <= VERMIN_VISIBLE_RANGE_M + colony.radius_m
            })
            .map(|colony| colony.rats.len() * 3 * 4)
            .sum();
        assert!(expected_night > 0, "Gaunt Passage wakes at night");
        assert_eq!(vermin_mesh_vertices(&mut app), expected_night);
    }

    /// The scatter impulse: nothing before the foot falls, a dart that peaks
    /// mid-impulse and dies away by the end, always onto walkable ground, and
    /// inert for rats beyond its reach.
    #[test]
    fn a_scatter_impulse_darts_and_dies_away() {
        let nav = committed_nav();
        let collision = CollisionWorld::default();
        let anchor = Vec2::new(-294.0, 220.0);
        let rat = bake_rat(&nav, &collision, anchor, 14.0, 7).expect("the Shambles is walkable");
        let (position, _, _) = rat.sample(0.0);
        let impulse = Some((position + Vec2::new(0.4, 0.0), 10.0));

        assert_eq!(
            scatter_offset(&nav, &collision, &rat, position, None, 10.5),
            Vec2::ZERO
        );
        assert_eq!(
            scatter_offset(&nav, &collision, &rat, position, impulse, 9.9),
            Vec2::ZERO,
            "an impulse cannot act before it happens"
        );
        let mid = scatter_offset(&nav, &collision, &rat, position, impulse, 10.0 + 0.8);
        assert!(
            mid.length() > 0.2,
            "mid-impulse the rat is well away: {mid:?}"
        );
        let target = position + mid;
        assert!(nav.is_walkable(f64::from(target.x), f64::from(target.y)));
        let late = scatter_offset(
            &nav,
            &collision,
            &rat,
            position,
            impulse,
            10.0 + SCATTER_TOTAL_S - 0.01,
        );
        assert!(late.length() < 0.1, "the dart dies away: {late:?}");
        assert_eq!(
            scatter_offset(
                &nav,
                &collision,
                &rat,
                position,
                impulse,
                10.0 + SCATTER_TOTAL_S + 0.1
            ),
            Vec2::ZERO
        );
        let far = position + Vec2::new(SCATTER_REACH_M + 1.0, 0.0);
        assert_eq!(
            scatter_offset(&nav, &collision, &rat, far, impulse, 10.8),
            Vec2::ZERO,
            "a rat beyond the reach never notices"
        );
    }

    /// §2.1 binds *both* halves — walkable ground and no collider — to every
    /// position a rat occupies, and a scatter dart is such a position.
    ///
    /// The bake erodes every exported footprint out of the walkable surface, so
    /// on the shipped city `is_walkable` almost always implies "no collider"
    /// and a nav-only dart check looks perfectly correct. Only a solid the
    /// navigation does not know about tells the two apart: stand one over the
    /// rat and the dart must be refused, not driven into it.
    #[test]
    fn a_scatter_dart_answers_to_collision_as_well_as_nav() {
        let nav = committed_nav();
        let open = CollisionWorld::default();
        let anchor = Vec2::new(-294.0, 220.0);
        let rat = bake_rat(&nav, &open, anchor, 14.0, 7).expect("the Shambles is walkable");
        let (position, _, _) = rat.sample(0.0);
        let impulse = Some((position + Vec2::new(0.4, 0.0), 10.0));

        let open_dart = scatter_offset(&nav, &open, &rat, position, impulse, 10.8);
        assert!(
            open_dart.length() > 0.2,
            "open ground lets the rat run: {open_dart:?}"
        );

        // A crate the navigation bake never saw, standing over everywhere this
        // dart could carry — including the quarter- and half-length pull-ups.
        let mut crated = CollisionWorld::default();
        crated.add_box(
            Vec3::new(position.x - 6.0, 0.0, position.y - 6.0),
            Vec3::new(position.x + 6.0, 2.0, position.y + 6.0),
        );
        assert!(
            walkable(&nav, &open, position + open_dart),
            "the ground itself is fine"
        );
        assert!(
            !walkable(&nav, &crated, position + open_dart),
            "the crate is the only thing that changed"
        );
        assert_eq!(
            scatter_offset(&nav, &crated, &rat, position, impulse, 10.8),
            Vec2::ZERO,
            "a dart into a collider is refused, not taken"
        );
    }

    /// Heavy rain thins the visible count, matching the animals going quiet.
    #[test]
    fn heavy_rain_sends_most_rats_under_cover() {
        assert!(!rain_suppressed(None));
        let mut weather = WorldWeatherState::default();
        assert!(!rain_suppressed(Some(&weather)));
        weather.current.kind = WeatherKind::Thunderstorm;
        assert!(rain_suppressed(Some(&weather)));
        weather.current.kind = WeatherKind::Rain;
        weather.current.precipitation = 0.7;
        assert!(rain_suppressed(Some(&weather)));
    }
}
