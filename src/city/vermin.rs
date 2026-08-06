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
//!
//! Once a game night one colony *boils* (M2): from the Snuffing to the Kindling
//! its count triples and its reach doubles — purely more of the same rats, off a
//! complement baked beside the ordinary loops at startup. That is also the one
//! and only crossing the whole feature makes into the simulation: an
//! unattributed `rat_swarm` world sound at the colony's anchor, which an NPC
//! standing in it hears like the town bell. No sheet field, no verb, no sim
//! state — a boil is a transient event, which is what the inbox is for.

use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};
use cathedral_sim::NavData;

use crate::{
    config::VerminSettings,
    controller::{CollisionWorld, PlayerController},
    mesh_batch::{idle_batch_mesh, write_batch_mesh},
    smart_actors::{
        WorldClockState,
        actors::ActorView,
        bridge::{BridgeCommand, BridgeHandle},
        model::Position,
    },
    // Heavy rain thins the *visible* colonies on the very predicate the
    // soundscape uses to send its birds and its alley cat quiet, rather than on
    // a second copy of the same two thresholds that could drift from it.
    soundscape::wildlife_suppressed as rain_suppressed,
    weather::WorldWeatherState,
};

/// Rats past this range are skipped entirely: a 25 cm body is sub-pixel long
/// before the smoke's 450 m, so the frame cost is a handful of small lofted
/// bodies near the player and the idle triangle everywhere else.
const VERMIN_VISIBLE_RANGE_M: f32 = 60.0;
/// A player or puppet inside this range of a rat kicks its colony's scatter.
const SCATTER_TRIGGER_M: f32 = 2.5;
/// Feet above this are not feet on the ground: the developer flying over the
/// Shambles is not standing in it, and a scatter is a reaction to a footfall.
///
/// The height compared is the **body**, not the eye: the player root carries
/// the body transform (`controller::PLAYER_SPAWN.y` = 0.91 m for a standing
/// player) and the camera is a child `EYE_OFFSET` above it, and `tp` likewise
/// names the body — so §5's 1.6 m reading vantages still count as standing.
/// The margin over 0.91 is what lets a rat be startled from a kerb, a step or
/// a stall platform; flight and the city's overhead bridge decks are all well
/// above it.
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

/// The Snuffing — the curfew edge the Scold already rings — is where a boil
/// begins…
const BOIL_START_FRACTION: f64 = 21.0 / 24.0;
/// …and the Kindling, the next morning, is where it ends.
const BOIL_END_FRACTION: f64 = 5.0 / 24.0;
/// Extra rats baked per ordinary one, so a boiling colony's count triples.
const BOIL_EXTRA_RATS_PER_RAT: usize = 2;
/// …and reaches this many times further from its anchor.
const BOIL_RADIUS_SCALE: f32 = 2.0;
/// The boil complement draws from its own seed stream, so a colony's ordinary
/// loops are identical whether it boils tonight or not.
const BOIL_SEED_STREAM: u64 = 0xb011_0000_0000_0000;
/// Hard ceiling on one colony's ordinary count — defence in depth behind the
/// config sanitizer (`config::VERMIN_DENSITY_MAX`), for a `VerminSettings`
/// built by hand rather than loaded: an order of magnitude above anything a
/// sanitized config can ask for, and small enough that the startup bake stays
/// bounded however wild the density is.
const MAX_RATS_PER_COLONY: usize = 256;
/// The catalog row the boil crosses into the sim on
/// (`assets/sounds/catalog.toml`): unattributed, not actor-emittable, 12 m.
const SWARM_SOUND_ID: &str = "rat_swarm";
/// Game-minutes between swarm percepts while a boil holds, and the one number
/// in this file that is a *budget* rather than a look.
///
/// A world sound is not only a line in an inbox: `flush_sound` hands the
/// nearest hearer the priority slot, so every repeat buys a paid turn for
/// whoever is standing in the boil. A boil runs eight game-hours — 480 game
/// minutes — so the doc's suggested "every few game-minutes" would be ~96
/// nudges a night for one person, two and a half times the Night Office's whole
/// daily budget, and a drive run at 60× shows exactly that: one NPC in the
/// Wickmarket took 24 of a 28-prompt run and the rest of the cast starved.
/// Half a game-hour bounds a whole night at 16, and the inbox coalescing
/// counter (`… (3 times now)`) keeps even those from crowding a history window.
const SWARM_PERCEPT_INTERVAL_MINUTES: f64 = 30.0;

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
    /// The boil complement, baked beside the ordinary rats at startup over the
    /// doubled radius and kept apart from them so an ordinary frame — every
    /// frame but one colony's, one night in eight — skips it for free.
    boil_rats: Vec<Rat>,
    /// Where somebody's foot fell and when — the one piece of mutable state.
    scatter: Option<(Vec2, f32)>,
}

impl Colony {
    /// The rats on the ground: the ordinary loops, and while the colony boils
    /// the complement as well — which is what triples the count.
    fn showing_rats(&self, showing: Showing) -> impl Iterator<Item = &Rat> {
        self.rats.iter().chain(
            matches!(showing, Showing::Boil)
                .then_some(self.boil_rats.as_slice())
                .unwrap_or_default(),
        )
    }

    /// How far from the anchor this colony's rats can be — the boil doubles it,
    /// so the distance cull has to be told.
    fn showing_radius_m(&self, showing: Showing) -> f32 {
        match showing {
            Showing::Loops => self.radius_m,
            Showing::Boil => self.radius_m * BOIL_RADIUS_SCALE,
        }
    }
}

/// The whole city's rats: one entity, one mesh rebuilt per frame.
#[derive(Component)]
pub(super) struct Vermin {
    colonies: Vec<Colony>,
    /// The committed navigation bake, read (like the puddles read it) to keep
    /// every waypoint and every scatter dart on ground the player can walk.
    nav: NavData,
    /// `vermin.seed`, baked in rather than read back off the config resource,
    /// so every system that asks which colony boils asks the same number.
    seed: u64,
    /// `vermin.swarm_percepts`. With it false the boil is a sight and nothing
    /// more: no bridge command is ever sent, and the sim never learns of rats.
    swarm_percepts: bool,
    /// The last game night whose boil was announced, so the log line the
    /// feature is verified by lands once a night however many frames it spans.
    announced_boil_night: Option<i64>,
    /// Game-minutes (since day zero) at the last swarm percept, which is what
    /// paces the repeats. Sim time, never wall time: the `T` key's 60× must
    /// reach the repeat the same way it reaches the boil.
    last_percept_minutes: Option<f64>,
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

/// Whether the straight line `from → to` is clear along its whole length, not
/// merely at its far end: [`Rat::sample`] lerps the sprint and a scatter dart
/// slides out and back along its offset, so a thin crate or wall standing
/// *between* two walkable endpoints is something a rat would otherwise pass
/// straight through. The walkable bitset is sampled half a nav cell apart
/// (nothing wider than a clipped cell corner can hide between samples at that
/// pitch); the colliders are swept once as a ray at the endpoint probe height
/// — one pass over the `CollisionWorld` per segment instead of one per
/// sample, which is what keeps the startup bake cheap. `from` is the caller's
/// to vouch for: it is always a point already accepted.
fn segment_clear(nav: &NavData, collision: &CollisionWorld, from: Vec2, to: Vec2) -> bool {
    if !walkable(nav, collision, to) {
        return false;
    }
    let length = from.distance(to);
    if length < 1e-4 {
        return true;
    }
    let samples = (f64::from(length) / (nav.grid().cell_m * 0.5)).ceil() as usize;
    if (1..samples).any(|index| {
        let point = from.lerp(to, index as f32 / samples as f32);
        !nav.is_walkable(f64::from(point.x), f64::from(point.y))
    }) {
        return false;
    }
    collision
        .nearest_ray_hit(
            Vec3::new(from.x, RAT_PROBE_Y, from.y),
            Vec3::new(to.x - from.x, 0.0, to.y - from.y),
            length,
        )
        .is_none()
}

/// Bakes one rat's loop: a home draw inside the colony disc, then a random
/// walk of sprint-sized steps, every sprint validated against nav + collision
/// along its whole line ([`segment_clear`]), the closing leg home included.
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
            (candidate.distance(anchor) <= radius_m
                && segment_clear(nav, collision, previous, candidate))
            .then_some(candidate)
        });
        // A cornered rat darts home rather than through a wall — and when even
        // the line home is cut, it sits where it is instead of taking it.
        waypoints.push(step.unwrap_or_else(|| {
            if segment_clear(nav, collision, previous, home) {
                home
            } else {
                previous
            }
        }));
    }
    // The loop closes with a sprint from the last waypoint back to `home` that
    // no draw above ever examined; drop trailing waypoints until that closing
    // line is as clear as every other leg. (A single-point loop is legal: one
    // degenerate leg, a rat that sits at home.)
    while waypoints.len() > 1
        && !segment_clear(
            nav,
            collision,
            *waypoints.last().expect("the loop starts at home"),
            home,
        )
    {
        waypoints.pop();
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

/// What a colony has on the ground this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    /// Its ordinary loops.
    Loops,
    /// Its loops and the boil complement: three times the rats over twice the
    /// ground — one colony, one night.
    Boil,
}

/// Whether a colony is out at all, and if so how much of it. The **one**
/// predicate [`animate_vermin`] and [`trigger_vermin_scatter`] both ask, so the
/// two can never disagree about which rats exist — a colony that is drawn is a
/// colony that can be startled, and the batch and the scatter sweep walk the
/// same population.
///
/// The three waste-pile colonies run all offices; the rest are the soundscape's
/// `WarmDayWaste` inverse — out only while the city is dark. No clock yet (the
/// seconds before the engine speaks, a headless city test) shows everything:
/// the same information-only dimming the chimney smoke practices.
fn colony_showing(
    all_offices: bool,
    boiling: bool,
    clock: Option<&WorldClockState>,
) -> Option<Showing> {
    if boiling {
        // A boil is out whatever the darkness gate would have said. The two
        // agree today — a boil runs from the Snuffing to the Kindling, which is
        // dark — and this is the half that must keep meaning what it says if
        // ever they stop.
        return Some(Showing::Boil);
    }
    let dark = clock
        .filter(|clock| clock.present)
        .is_none_or(|clock| clock.brightness <= 0.30);
    (all_offices || dark).then_some(Showing::Loops)
}

/// The game night a clock reading falls in, or `None` by day. A night spans
/// midnight, so 21:00 on day N and 04:00 on day N+1 are both night N — which is
/// what keeps one boil one boil, rather than two halves picking two colonies.
///
/// Without a clock nothing boils, deliberately unlike [`colony_showing`]: a
/// colony with no clock is only undimmed, while a boil is an event with a date,
/// and an undated one would announce itself on the first frame of every run.
fn boil_night(clock: Option<&WorldClockState>) -> Option<i64> {
    let clock = clock.filter(|clock| clock.present)?;
    if clock.fraction >= BOIL_START_FRACTION {
        Some(clock.day)
    } else if clock.fraction < BOIL_END_FRACTION {
        Some(clock.day - 1)
    } else {
        None
    }
}

/// Which colony boils tonight: `hash(night, seed) % colonies`, so the answer is
/// a pure function every system can recompute instead of a fact one of them has
/// to own and publish.
fn boiling_colony(night: i64, seed: u64, colonies: usize) -> Option<usize> {
    (colonies > 0).then(|| {
        let hash = mix(seed ^ (night as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        (hash % colonies as u64) as usize
    })
}

/// The colony boiling right now, if any — [`boil_night`] and
/// [`boiling_colony`] in one, since nobody ever wants only half.
fn boiling_colony_now(
    clock: Option<&WorldClockState>,
    seed: u64,
    colonies: usize,
) -> Option<usize> {
    boiling_colony(boil_night(clock)?, seed, colonies)
}

/// Sim minutes since day zero — monotone across the midnight a boil spans,
/// which the clock's own `fraction` is not.
fn game_minutes(clock: &WorldClockState) -> f64 {
    (clock.day as f64 + clock.fraction) * 24.0 * 60.0
}

/// Whether the boil is due to be heard (again): on entry, and at most every
/// [`SWARM_PERCEPT_INTERVAL_MINUTES`] game-minutes after. A clock that jumps
/// backwards re-arms rather than falling silent until it has caught up.
fn percept_due(last_minutes: Option<f64>, minutes: f64) -> bool {
    last_minutes
        .is_none_or(|last| !(0.0..SWARM_PERCEPT_INTERVAL_MINUTES).contains(&(minutes - last)))
}

/// One colony's `(ordinary, boil)` counts at this density.
///
/// `load_config` already clamps the player's file (LE-05), but this system
/// takes whatever `VerminSettings` was inserted — so the arithmetic itself
/// must stay bounded: the float-to-`usize` cast saturates (`NaN` to zero) and
/// lands on [`MAX_RATS_PER_COLONY`] rather than a `usize::MAX` bake loop, and
/// the boil multiply saturates rather than overflowing.
fn colony_counts(rats: usize, density: f32) -> (usize, usize) {
    let count = ((rats as f32 * density).round().max(0.0) as usize).min(MAX_RATS_PER_COLONY);
    (count, count.saturating_mul(BOIL_EXTRA_RATS_PER_RAT))
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
    let mut boil_total = 0_usize;
    let colonies: Vec<Colony> = COLONIES
        .iter()
        .enumerate()
        .map(|(colony_index, spec)| {
            let (count, boil_count) = colony_counts(spec.rats, settings.density);
            let rats: Vec<Rat> = (0..count)
                .filter_map(|rat_index| {
                    let seed = mix(settings
                        .seed
                        .wrapping_add((colony_index as u64) << 32)
                        .wrapping_add(rat_index as u64));
                    bake_rat(&nav, &collision, spec.anchor, spec.radius_m, seed)
                })
                .collect();
            // The boil is baked here, with the rest, rather than the night it
            // happens: waypoint validation reads the whole `CollisionWorld`,
            // and the frame the Snuffing lands on is not the frame to do it.
            let boil_rats: Vec<Rat> = (0..boil_count)
                .filter_map(|rat_index| {
                    let seed = mix(BOIL_SEED_STREAM
                        .wrapping_add(settings.seed)
                        .wrapping_add((colony_index as u64) << 32)
                        .wrapping_add(rat_index as u64));
                    // Over a doubled radius most of a pinched street — Gaunt
                    // Passage, the Seven Lofts skirts — is building, and a
                    // draw can miss the ground forty times running. A boil
                    // rat that finds no room out there settles in the colony's
                    // own disc instead, where the ordinary rats have already
                    // proved there is some: the boil crowds what ground it
                    // has, rather than turning up short-handed.
                    bake_rat(
                        &nav,
                        &collision,
                        spec.anchor,
                        spec.radius_m * BOIL_RADIUS_SCALE,
                        seed,
                    )
                    .or_else(|| bake_rat(&nav, &collision, spec.anchor, spec.radius_m, seed))
                })
                .collect();
            if rats.is_empty() && count > 0 {
                warn!(
                    "[vermin] {} offers no walkable ground around {:?}",
                    spec.name, spec.anchor
                );
            }
            total += rats.len();
            boil_total += boil_rats.len();
            Colony {
                name: spec.name,
                anchor: spec.anchor,
                radius_m: spec.radius_m,
                all_offices: spec.all_offices,
                rats,
                boil_rats,
                scatter: None,
            }
        })
        .collect();

    for colony in &colonies {
        debug!(
            "[vermin] {}: {} rats (+{} when it boils)",
            colony.name,
            colony.rats.len(),
            colony.boil_rats.len()
        );
    }
    info!(
        "[vermin] {total} rats settled across {} colonies, and {boil_total} more waiting on a boil",
        colonies.len()
    );
    commands.spawn((
        Name::new("Vermin colonies"),
        Mesh3d(meshes.add(idle_batch_mesh())),
        // Per-vertex colour, no texture: the brown-to-grey coat, the lighter
        // belly and the naked pink-grey tail are painted into the batch, and
        // at 25 cm and 2 m/s the rest reads by silhouette and motion, not by
        // texel.
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
        Vermin {
            colonies,
            nav,
            seed: settings.seed,
            swarm_percepts: settings.swarm_percepts,
            announced_boil_night: None,
            last_percept_minutes: None,
        },
    ));
}

/// The boil's bookkeeping half (`features/rats.md` M2): the once-a-night log
/// line the feature is verified by, and the one crossing the whole feature
/// makes into the simulation. The drawing half is [`animate_vermin`], which
/// carries no state at all — it asks [`boiling_colony_now`] the same question
/// and gets the same answer.
///
/// The percept is an unattributed world sound at the colony's anchor, so an NPC
/// standing in the boil hears it on their next turn exactly as they hear the
/// town bell; nobody is nudged from across the ward (the catalog row is 12 m),
/// nothing is attributed (a swarm has no author), and no sim state is created.
/// With `vermin.swarm_percepts` false, or with no engine at all (a city test,
/// `CATHEDRAL_NO_ACTORS`), not one command is sent and the boil stays a sight.
pub(super) fn announce_vermin_boil(
    clock: Option<Res<WorldClockState>>,
    handle: Option<Res<BridgeHandle>>,
    mut vermin: Query<&mut Vermin>,
) {
    let clock = clock.as_deref();
    for mut vermin in &mut vermin {
        let Some(night) = boil_night(clock) else {
            continue;
        };
        let Some(index) = boiling_colony(night, vermin.seed, vermin.colonies.len()) else {
            continue;
        };
        // A colony with nothing on the ground has no swarm to hear: density 0
        // — a visual dial, "the authored counts and nothing else" — bakes
        // every record empty, and an anchor the bake could not settle leaves
        // one empty at any density. Neither may log a boil nor cross into the
        // sim, where the percept buys whoever hears it a paid turn (LE-04).
        if vermin.colonies[index]
            .showing_rats(Showing::Boil)
            .next()
            .is_none()
        {
            continue;
        }
        // Keyed on the night, not on an edge: a clock hovering either side of
        // the Snuffing re-enters the same night, and re-entering is not news.
        if vermin.announced_boil_night != Some(night) {
            info!("[vermin] boil: {}", vermin.colonies[index].name);
            vermin.announced_boil_night = Some(night);
            // A fresh boil is heard at once; the interval only paces repeats.
            vermin.last_percept_minutes = None;
        }
        if !vermin.swarm_percepts {
            continue;
        }
        let (Some(handle), Some(clock)) = (handle.as_deref(), clock) else {
            continue;
        };
        let minutes = game_minutes(clock);
        if !percept_due(vermin.last_percept_minutes, minutes) {
            continue;
        }
        let anchor = vermin.colonies[index].anchor;
        let Ok(position_m) = Position::try_from(Vec3::new(anchor.x, RAT_GROUND_Y, anchor.y)) else {
            continue;
        };
        // Stamped whether or not the queue takes it: a full bridge is a busy
        // engine, and retrying a percept at it every frame is the one thing
        // this interval exists to prevent.
        vermin.last_percept_minutes = Some(minutes);
        if let Err(error) = handle.try_send(BridgeCommand::WorldSound {
            sound_id: SWARM_SOUND_ID.to_string(),
            position_m,
        }) {
            debug!("[vermin] swarm percept not sent: {error}");
        }
    }
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
        let boiling = boiling_colony_now(clock, vermin.seed, vermin.colonies.len());
        for (index, colony) in vermin.colonies.iter_mut().enumerate() {
            // A colony nobody can see is a colony nobody can startle; skipping
            // it also spares the `rat.sample()` sweep below.
            let Some(showing) = colony_showing(colony.all_offices, boiling == Some(index), clock)
            else {
                colony.scatter = None;
                continue;
            };
            if let Some((_, started)) = colony.scatter {
                if elapsed - started < SCATTER_TOTAL_S {
                    continue;
                }
                colony.scatter = None;
            }
            let reach = colony.showing_radius_m(showing) + SCATTER_REACH_M;
            let near: Vec<Vec2> = movers
                .iter()
                .copied()
                .filter(|mover| mover.distance(colony.anchor) <= reach)
                .collect();
            if near.is_empty() {
                continue;
            }
            // The boil's rats flee like any other: they are the same rats.
            let struck = colony.showing_rats(showing).find_map(|rat| {
                let (position, _, _) = rat.sample(elapsed);
                near.iter().copied().find(|mover| {
                    mover.distance_squared(position) <= SCATTER_TRIGGER_M * SCATTER_TRIGGER_M
                })
            });
            if let Some(mover) = struck {
                colony.scatter = Some((mover, elapsed));
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
    // A dart that would carry into — or through — a wall or a crate pulls up
    // short instead. The rat slides out and back along this whole line as the
    // envelope rises and falls, so the line must be clear, not just its peak.
    for fraction in [1.0, 0.5, 0.25] {
        let offset = direction * reach * fraction;
        if segment_clear(nav, collision, position, position + offset) {
            return offset;
        }
    }
    Vec2::ZERO
}

/// Rewrites the shared vermin mesh: every visible rat placed on its loop (or
/// its scatter dart) as a small lofted body oriented to its heading —
/// deliberately not camera-facing billboards, because a ground creature seen
/// from a bridge or in developer flight must not turn to face down.
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

        let boiling = boiling_colony_now(clock, vermin.seed, vermin.colonies.len());
        for (index, colony) in vermin.colonies.iter().enumerate() {
            let Some(showing) = colony_showing(colony.all_offices, boiling == Some(index), clock)
            else {
                continue;
            };
            let colony_range = VERMIN_VISIBLE_RANGE_M + colony.showing_radius_m(showing);
            if colony.anchor.distance_squared(camera_position.xz()) > colony_range * colony_range {
                continue;
            }
            for (rat_index, rat) in colony.showing_rats(showing).enumerate() {
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

// ---------------------------------------------------------------------------
// The rat itself: one lofted hull and a few authored quads, written straight
// into the batch each frame like a smoke puff. There is no per-rat mesh asset
// to place, so the gait is free to reshape the body — stretch it into a
// sprint, beat the legs, whip the tail — without a rig.
// ---------------------------------------------------------------------------

/// Cross-sections around the body hull. Six smooth-shaded sectors read as a
/// rounded barrel at rat scale, and the flat face they leave under the belly
/// sits close over the ground like a crouch.
const BODY_SECTORS: usize = 6;
/// Stations along the body hull, nose tip to tail root.
const BODY_STATIONS: usize = 8;
/// The tail is a triangular tube — the cheapest closed form that reads from
/// street level and from a bridge overhead alike.
const TAIL_SECTORS: usize = 3;
const TAIL_STATIONS: usize = 5;

/// The body hull for the 0.28 m reference rat, `(along, half_width,
/// half_height, spine_height)` in metres; `Rat::length_m` scales the whole
/// frame. The profile is the rat silhouette itself: a pointed muzzle, a low
/// head, the back arched over the haunches, the rump falling away to the tail
/// root. The two end stations are a few millimetres wide and left open rather
/// than capped — at 25 cm a 4 mm hole never covers a pixel, and the tail tube
/// plugs the rump's.
const BODY_PROFILE: [(f32, f32, f32, f32); BODY_STATIONS] = [
    (0.140, 0.004, 0.004, 0.030),  // nose tip
    (0.116, 0.012, 0.011, 0.034),  // muzzle
    (0.086, 0.024, 0.022, 0.044),  // head — the eyes and ears sit here
    (0.044, 0.032, 0.030, 0.051),  // shoulders
    (-0.006, 0.038, 0.036, 0.055), // mid-back
    (-0.068, 0.042, 0.040, 0.058), // haunches, the arch's peak
    (-0.116, 0.028, 0.026, 0.044), // rump
    (-0.140, 0.010, 0.009, 0.031), // tail root
];

/// Vertices one rat writes into the batch: the hull, the tail tube, two ears,
/// two eyes and four legs. The gait bends the body but never its topology, so
/// the counting tests multiply by this whatever the clock says.
const RAT_VERTICES: usize =
    BODY_STATIONS * BODY_SECTORS + TAIL_STATIONS * TAIL_SECTORS + 2 * 4 + 2 * 4 + 4 * 4;

fn mixf(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// One rat: a smooth-shaded loft from nose tip to tail root — pointed muzzle,
/// back arched over the haunches — with pricked ears, dark eye beads, four
/// stub legs scurrying in diagonal pairs, and a full-length naked tail.
/// Sprinting stretches the body long and low, beats the legs and whips the
/// tail; a paused rat sits on a still S-curved tail, nothing moving but an
/// occasional sniffing lift of the nose.
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
    use std::f32::consts::{PI, TAU};

    let first_vertex = positions.len();
    let scale = rat.length_m / 0.28;
    let center = Vec3::new(position.x, RAT_GROUND_Y, position.y);
    let ahead = Vec3::new(heading.x, 0.0, heading.y);
    let side = Vec3::new(-heading.y, 0.0, heading.x);

    // One clock drives the whole gait — the legs' beat, a bob riding two to a
    // stride, the tail's whip — phased per rat so a colony never marches.
    let gait = elapsed * 21.0 + rat.phase * 7.0;
    let (stretch, squat, bob) = if moving {
        // A sprinting rat runs long and low.
        (1.08, 0.94, 0.006 * (gait * 2.0).sin() * scale)
    } else {
        (1.0, 1.0, 0.0)
    };
    // The sniff: a paused rat now and then lifts its nose and works it. A slow
    // per-rat sine gates a faster one, so the twitch comes in bouts with still
    // stretches between — wallpaper until it moves.
    let sniff = if moving {
        0.0
    } else {
        let bout = (elapsed * 0.6 + unit(rat.seed, 50) * TAU).sin();
        ((bout - 0.35) * 4.0).clamp(0.0, 1.0) * (elapsed * 9.0 + rat.phase * 3.0).sin()
    };
    // How much of the sniff's nose-lift a station at `along` inherits.
    let lift = |along: f32| sniff * 0.008 * ((along - 0.05) / 0.09).clamp(0.0, 1.0);
    let station = |along: f32, height: f32| {
        center
            + ahead * (along * stretch * scale)
            + Vec3::Y * ((height * squat + lift(along)) * scale + bob)
    };

    // Coat: per-rat brightness (`tint`) over a per-rat brown-to-grey blend —
    // kept dark: full sun on a mid vertex colour reads as bone, and a rat is
    // vermin, not a lab mouse. Per vertex the spine darkens and the belly
    // lightens, the shading that makes a rounded back read as a body rather
    // than a lump.
    let tint = rat.tint;
    let grey = unit(rat.seed, 34);
    let coat = [
        mixf(0.062, 0.050, grey) * tint,
        mixf(0.048, 0.048, grey) * tint,
        mixf(0.036, 0.046, grey) * tint,
    ];
    let body_color = |up_factor: f32| {
        let back = up_factor.max(0.0) * 0.30;
        let belly = ((-up_factor) * 1.3 - 0.15).clamp(0.0, 1.0) * 0.6;
        [
            coat[0] * (1.0 + 0.75 * belly) * (1.0 - back),
            coat[1] * (1.0 + 0.85 * belly) * (1.0 - back),
            coat[2] * (1.0 + belly) * (1.0 - back),
            1.0,
        ]
    };
    // The naked parts in the pink-grey of bare skin; the eyes a wet dark bead
    // outside the tint, so a pale rat never gets pale eyes.
    let naked = [0.115 * tint, 0.082 * tint, 0.072 * tint, 1.0];
    let feet = [0.095 * tint, 0.070 * tint, 0.062 * tint, 1.0];
    let ear_color = [
        (coat[0] + naked[0]) * 0.5,
        (coat[1] + naked[1]) * 0.5,
        (coat[2] + naked[2]) * 0.5,
        1.0,
    ];
    let eye = [0.015, 0.011, 0.010, 1.0];

    // -- The body hull ------------------------------------------------------
    // Smooth-shaded rings stitched into a tube. A ring normal is the ellipse's
    // own, tilted along the spine by the taper (`radial - ahead * slope`), so
    // the nose cone lights like a cone and not like a cylinder butt.
    let hull_first = positions.len() as u32;
    for (index, &(along, half_width, half_height, spine)) in BODY_PROFILE.iter().enumerate() {
        let fore = BODY_PROFILE[index.saturating_sub(1)];
        let aft = BODY_PROFILE[(index + 1).min(BODY_STATIONS - 1)];
        let mean = |ring: (f32, f32, f32, f32)| (ring.1 + ring.2) * 0.5;
        let slope = (mean(fore) - mean(aft)) / (fore.0 - aft.0).max(1e-4);
        let ring_center = station(along, spine);
        for sector in 0..BODY_SECTORS {
            let theta = sector as f32 / BODY_SECTORS as f32 * TAU;
            let (sin, cos) = theta.sin_cos();
            let point = ring_center
                + side * (cos * half_width * scale)
                + Vec3::Y * (sin * half_height * squat * scale);
            let radial =
                (side * (cos * half_height) + Vec3::Y * (sin * half_width)).normalize_or(Vec3::Y);
            let normal = (radial - ahead * slope).normalize_or(radial);
            positions.push(point.to_array());
            normals.push(normal.to_array());
            uvs.push([
                sector as f32 / BODY_SECTORS as f32,
                index as f32 / (BODY_STATIONS - 1) as f32,
            ]);
            colors.push(body_color(sin));
        }
    }
    stitch_tube(indices, hull_first, BODY_STATIONS, BODY_SECTORS);

    // -- The tail -----------------------------------------------------------
    // The other half of a rat: the body's length again, drooping from the
    // root to drag its tip on the ground — whipped side to side on the gait
    // clock at a sprint, lying in a per-rat S-curve at rest.
    let tail_first = positions.len() as u32;
    let tail_length = rat.length_m * 0.9;
    let swirl = unit(rat.seed, 51) * TAU;
    let root_height = BODY_PROFILE[BODY_STATIONS - 1].3;
    for index in 0..TAIL_STATIONS {
        let fraction = index as f32 / (TAIL_STATIONS - 1) as f32;
        let lateral = if moving {
            (gait * 0.5 - fraction * 4.0).sin() * 0.020 * fraction
        } else {
            (swirl + fraction * 3.2).sin() * 0.014 * fraction
        };
        let droop = fraction * fraction * (3.0 - 2.0 * fraction);
        let height = mixf(root_height * squat * scale + bob, 0.006 * scale, droop);
        let ring_center = center
            + ahead * (-(0.140 * stretch) * scale - fraction * tail_length)
            + side * (lateral * scale)
            + Vec3::Y * height;
        let radius = mixf(0.0055, 0.0018, fraction) * scale;
        for sector in 0..TAIL_SECTORS {
            // A vertex at the top, a flat face resting toward the ground.
            let theta = PI * 0.5 + sector as f32 / TAIL_SECTORS as f32 * TAU;
            let (sin, cos) = theta.sin_cos();
            let point = ring_center + side * (cos * radius) + Vec3::Y * (sin * radius);
            positions.push(point.to_array());
            normals.push((side * cos + Vec3::Y * sin).to_array());
            uvs.push([sector as f32 / TAIL_SECTORS as f32, fraction]);
            colors.push(naked);
        }
    }
    stitch_tube(indices, tail_first, TAIL_STATIONS, TAIL_SECTORS);

    // -- Ears ---------------------------------------------------------------
    // Two flaps pricked up and out from the crown, raked a little back. The
    // corner order mirrors between the sides so both computed normals face
    // the sky, not one sky and one street.
    for sign in [1.0_f32, -1.0] {
        let base = station(0.068, 0.056) + side * (sign * 0.013 * scale);
        let out = side * (sign * 0.75) + Vec3::Y * 0.66;
        let tip = base + out * (0.020 * scale) - ahead * (0.005 * scale);
        let half = ahead * (0.008 * scale);
        let corners = if sign > 0.0 {
            [
                base + half,
                base - half,
                tip - half * 0.55,
                tip + half * 0.55,
            ]
        } else {
            [
                base - half,
                base + half,
                tip + half * 0.55,
                tip - half * 0.55,
            ]
        };
        push_quad(positions, normals, uvs, colors, indices, corners, ear_color);
    }

    // -- Eyes ---------------------------------------------------------------
    // Two dark beads just proud of the head's flanks — sub-pixel past a few
    // metres, alive when a scatter sends one darting past your boot.
    for sign in [1.0_f32, -1.0] {
        let bead = station(0.094, 0.047) + side * (sign * 0.0208 * scale);
        let across = ahead * (0.0032 * scale);
        let up = Vec3::Y * (0.0024 * scale);
        let corners = if sign > 0.0 {
            [
                bead - across - up,
                bead + across - up,
                bead + across + up,
                bead - across + up,
            ]
        } else {
            [
                bead + across - up,
                bead - across - up,
                bead - across + up,
                bead + across + up,
            ]
        };
        push_quad(positions, normals, uvs, colors, indices, corners, eye);
    }

    // -- Legs ---------------------------------------------------------------
    // Four stubs from hip to ground. Sprinting they scurry in diagonal pairs
    // on the gait clock, lifting through the forward swing; paused they
    // stand, the front pair planted a little ahead like a rat about to bolt.
    let legs: [(f32, f32, f32, f32); 4] = [
        (0.050, 0.024, 0.030, 0.0),   // front right
        (0.050, -0.024, 0.030, PI),   // front left
        (-0.075, 0.030, 0.032, PI),   // rear right — diagonal with front left
        (-0.075, -0.030, 0.032, 0.0), // rear left
    ];
    for (along, flank, hip_height, phase) in legs {
        let sign = flank.signum();
        let beat = (gait + phase).sin();
        let (swing, foot_lift) = if moving {
            (beat * 0.030, beat.max(0.0) * 0.010)
        } else if along > 0.0 {
            (0.008, 0.0)
        } else {
            (-0.006, 0.0)
        };
        let hip = center
            + ahead * (along * stretch * scale)
            + side * (flank * scale)
            + Vec3::Y * (hip_height * squat * scale + bob);
        let foot = center
            + ahead * ((along * stretch + swing) * scale)
            + side * ((flank + sign * 0.007) * scale)
            + Vec3::Y * (foot_lift * scale);
        let half_hip = ahead * (0.005 * scale);
        let half_foot = ahead * (0.0035 * scale);
        let corners = if sign > 0.0 {
            [
                hip + half_hip,
                hip - half_hip,
                foot - half_foot,
                foot + half_foot,
            ]
        } else {
            [
                hip - half_hip,
                hip + half_hip,
                foot + half_foot,
                foot - half_foot,
            ]
        };
        push_quad(positions, normals, uvs, colors, indices, corners, feet);
    }

    // The promise the counting tests spend: whatever the gait is doing, a rat
    // is always exactly this many vertices.
    debug_assert_eq!(positions.len() - first_vertex, RAT_VERTICES);
}

/// Stitches consecutive rings of `sectors` vertices into a closed tube, wound
/// so every face's computed normal agrees with the outward ring normals —
/// the invariant `a_rat_is_wound_with_its_normals_out` pins, since the
/// double-sided material would shade an inside-out tube identically.
fn stitch_tube(indices: &mut Vec<u32>, first: u32, stations: usize, sectors: usize) {
    for station in 0..stations - 1 {
        for sector in 0..sectors {
            let next = (sector + 1) % sectors;
            let a = first + (station * sectors + sector) as u32;
            let b = first + (station * sectors + next) as u32;
            let c = first + ((station + 1) * sectors + sector) as u32;
            let d = first + ((station + 1) * sectors + next) as u32;
            indices.extend_from_slice(&[a, d, c, a, b, d]);
        }
    }
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
    use cathedral_sim::WeatherKind;

    use super::*;
    use crate::mesh_batch::IDLE_BATCH_VERTICES;

    fn built_app() -> App {
        built_app_with(None)
    }

    /// The city, and the whole vermin chain in the order `CityPlugin` runs it —
    /// scatter included, so the sweep that arms an impulse is exercised by the
    /// same frames the batch is.
    fn built_app_with(settings: Option<VerminSettings>) -> App {
        let mut app = App::new();
        if let Some(settings) = settings {
            app.insert_resource(settings);
        }
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<crate::materials::WindowGlassMaterial>()
            .init_resource::<CollisionWorld>()
            .add_systems(Startup, (super::super::build_city, spawn_vermin).chain())
            .add_systems(
                Update,
                (announce_vermin_boil, trigger_vermin_scatter, animate_vermin).chain(),
            );
        app.update();
        app
    }

    /// The seed and density `built_app` runs on: no `VerminSettings` resource
    /// means the shipped defaults, which is what `config.ron` ships too.
    fn default_seed() -> u64 {
        VerminSettings::default().seed
    }

    /// A clock at `hour` on `day`, dark enough for the night colonies.
    fn night_clock(day: i64, hour: f64) -> WorldClockState {
        WorldClockState {
            present: true,
            day,
            fraction: hour / 24.0,
            brightness: 0.05,
            ..Default::default()
        }
    }

    fn committed_nav() -> NavData {
        NavData::from_parts(
            include_str!("../../assets/world/navigation.json"),
            include_bytes!("../../assets/world/navigation.bin"),
        )
        .expect("the committed navigation bake parses")
    }

    /// The one colony's scatter impulse, if it is armed.
    fn scatter_of(app: &mut App, colony: usize) -> Option<(Vec2, f32)> {
        let world = app.world_mut();
        world
            .query::<&Vermin>()
            .single(world)
            .expect("one vermin batch")
            .colonies[colony]
            .scatter
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
    /// Every baked sprint — sampled sub-cell along its whole line, not merely
    /// at its endpoints, because `Rat::sample` lerps the lot (LE-03) — stays
    /// on the player's walkable surface and inside no collider; and the colony
    /// table actually settles its population, which is the canary for an
    /// anchor drifting off walkable ground.
    #[test]
    fn every_waypoint_is_walkable_and_uncollided() {
        let mut app = built_app();
        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("the city spawns exactly one vermin batch");
        let collision = world.resource::<CollisionWorld>();
        let pitch = vermin.nav.grid().cell_m * 0.5;
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
                    let samples = (f64::from(leg.from.distance(leg.to)) / pitch)
                        .ceil()
                        .max(1.0) as usize;
                    for index in 0..=samples {
                        let point = leg.from.lerp(leg.to, index as f32 / samples as f32);
                        assert!(
                            walkable(&vermin.nav, collision, point),
                            "{}: sprint point {point:?} is off the walkable surface",
                            colony.name
                        );
                    }
                    for point in [leg.from, leg.to] {
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
    /// `WarmDayWaste` inverse, and a missing clock dims nothing. A boiling
    /// colony is out whatever the darkness gate would have said.
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
        assert_eq!(
            colony_showing(true, false, Some(&day)),
            Some(Showing::Loops)
        );
        assert_eq!(
            colony_showing(true, false, Some(&night)),
            Some(Showing::Loops)
        );
        assert_eq!(colony_showing(false, false, Some(&day)), None);
        assert_eq!(
            colony_showing(false, false, Some(&night)),
            Some(Showing::Loops)
        );
        assert_eq!(colony_showing(false, false, None), Some(Showing::Loops));
        assert_eq!(
            colony_showing(
                false,
                false,
                Some(&WorldClockState {
                    present: false,
                    brightness: 1.0,
                    ..Default::default()
                })
            ),
            Some(Showing::Loops)
        );
        // The boil overrides the gate in both directions.
        assert_eq!(colony_showing(false, true, Some(&day)), Some(Showing::Boil));
        assert_eq!(
            colony_showing(true, true, Some(&night)),
            Some(Showing::Boil)
        );
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
            .map(|colony| colony.rats.len() * RAT_VERTICES)
            .sum();
        assert!(expected > 0, "the Shambles colony is in range by day");
        assert_eq!(vermin_mesh_vertices(&mut app), expected);

        // Gaunt Passage is a night colony: empty batch at High Wick, rats
        // once the city is dark. 20:00 is dark and before the Snuffing, so
        // this reads the clock gate alone, with no boil under it.
        let gaunt = Vec3::new(-155.0, 6.0, 17.0);
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<&mut GlobalTransform, With<Camera3d>>();
        *cameras.single_mut(world).expect("one camera") = GlobalTransform::from_translation(gaunt);
        app.update();
        assert_eq!(vermin_mesh_vertices(&mut app), IDLE_BATCH_VERTICES);

        app.insert_resource(night_clock(0, 20.0));
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
            .map(|colony| colony.rats.len() * RAT_VERTICES)
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

    /// LE-03: endpoints are not trajectories. `Rat::sample` lerps the whole
    /// sprint and a scatter dart slides along its whole offset, so a thin
    /// solid standing *between* two walkable endpoints — invisible to an
    /// endpoint-only check — must refuse the leg and pull the dart up short.
    #[test]
    fn a_thin_obstacle_between_endpoints_refuses_the_sprint_and_the_dart() {
        let nav = committed_nav();
        let open = CollisionWorld::default();
        let anchor = Vec2::new(-294.0, 220.0);
        let rat = bake_rat(&nav, &open, anchor, 14.0, 41).expect("the Shambles is walkable");
        let pitch = nav.grid().cell_m * 0.5;

        // A 12 cm crate over the longest sprint's midpoint: both endpoints
        // stay clear, the line between them does not.
        let leg = rat
            .legs
            .iter()
            .max_by(|a, b| {
                a.from
                    .distance_squared(a.to)
                    .total_cmp(&b.from.distance_squared(b.to))
            })
            .expect("a baked rat has legs");
        let length = leg.from.distance(leg.to);
        assert!(length > 0.5, "an open bake sprints: {length}");
        let mid = leg.from.lerp(leg.to, 0.5);
        let mut crated = CollisionWorld::default();
        crated.add_box(
            Vec3::new(mid.x - 0.06, 0.0, mid.y - 0.06),
            Vec3::new(mid.x + 0.06, 1.0, mid.y + 0.06),
        );
        assert!(
            walkable(&nav, &crated, leg.from) && walkable(&nav, &crated, leg.to),
            "the crate must cut only the line, never an endpoint"
        );
        assert!(
            !segment_clear(&nav, &crated, leg.from, leg.to),
            "a sprint through the crate is refused"
        );

        // Re-baked against the crate, no accepted sprint crosses it: every
        // sub-cell sample of every leg stays outside.
        let rebaked = bake_rat(&nav, &crated, anchor, 14.0, 41).expect("still walkable");
        for leg in &rebaked.legs {
            let samples = (f64::from(leg.from.distance(leg.to)) / pitch)
                .ceil()
                .max(1.0) as usize;
            for index in 0..=samples {
                let point = leg.from.lerp(leg.to, index as f32 / samples as f32);
                assert!(
                    !crated.contains_point(Vec3::new(point.x, RAT_PROBE_Y, point.y)),
                    "a re-baked sprint cuts the crate at {point:?}"
                );
            }
        }

        // The same trick at a dart's midpoint: the full dart's line crosses
        // the crate with its endpoint clear, so an endpoint check would take
        // it straight through — the taken dart must stop short of the crate.
        let (position, _, _) = rat.sample(0.0);
        let impulse = Some((position + Vec2::new(0.4, 0.0), 10.0));
        let open_dart = scatter_offset(&nav, &open, &rat, position, impulse, 10.8);
        assert!(
            open_dart.length() > 0.2,
            "open ground lets the rat run: {open_dart:?}"
        );
        let dart_mid = position + open_dart * 0.5;
        let mut dart_crated = CollisionWorld::default();
        dart_crated.add_box(
            Vec3::new(dart_mid.x - 0.06, 0.0, dart_mid.y - 0.06),
            Vec3::new(dart_mid.x + 0.06, 1.0, dart_mid.y + 0.06),
        );
        assert!(
            walkable(&nav, &dart_crated, position + open_dart),
            "the full dart's endpoint is clear — only the line is cut"
        );
        let short = scatter_offset(&nav, &dart_crated, &rat, position, impulse, 10.8);
        assert!(
            short.length() < open_dart.length() * 0.5,
            "the dart pulls up before the crate: {short:?} vs {open_dart:?}"
        );
        let samples = (f64::from(short.length()) / pitch).ceil().max(1.0) as usize;
        for index in 0..=samples {
            let point = position.lerp(position + short, index as f32 / samples as f32);
            assert!(
                !dart_crated.contains_point(Vec3::new(point.x, RAT_PROBE_Y, point.y)),
                "the taken dart cuts the crate at {point:?}"
            );
        }
    }

    /// The sweep that *arms* an impulse, not just the envelope it plays out:
    /// somebody standing on a rat startles its colony, and the same body
    /// overhead — developer flight, or one of the city's bridge decks — does
    /// not. The height compared is the player root's, which carries the body
    /// (`controller::PLAYER_SPAWN.y`), not the child camera's eye.
    #[test]
    fn a_footfall_arms_the_scatter_and_a_flight_over_it_does_not() {
        let mut app = built_app();
        let elapsed = app.world().resource::<Time>().elapsed_secs();
        let standing_on = {
            let world = app.world_mut();
            let vermin = world
                .query::<&Vermin>()
                .single(world)
                .expect("one vermin batch");
            vermin.colonies[0].rats[0].sample(elapsed).0
        };
        let player = app
            .world_mut()
            .spawn((
                PlayerController::default(),
                Transform::from_xyz(standing_on.x, 0.91, standing_on.y),
            ))
            .id();

        app.update();
        assert!(
            scatter_of(&mut app, 0).is_some(),
            "a foot on a rat startles the colony"
        );

        // Lift the same body into flight over the same ground: nothing re-arms.
        {
            let world = app.world_mut();
            let mut vermin = world
                .query::<&mut Vermin>()
                .single_mut(world)
                .expect("one vermin batch");
            vermin.colonies[0].scatter = None;
        }
        app.world_mut()
            .get_mut::<Transform>(player)
            .expect("the player is still there")
            .translation
            .y = SCATTER_MAX_FOOT_Y + 4.0;
        app.update();
        assert_eq!(
            scatter_of(&mut app, 0),
            None,
            "flying over a colony is not standing in it"
        );
    }

    /// Heavy rain thins the rats that are *drawn*, not merely the predicate:
    /// every third one stays out, matching the animals going quiet.
    #[test]
    fn heavy_rain_thins_the_drawn_batch() {
        let mut app = built_app();
        let shambles = Vec3::new(-294.0, 6.0, 220.0);
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(shambles),
        ));
        app.update();
        let dry = vermin_mesh_vertices(&mut app);
        assert!(dry > IDLE_BATCH_VERTICES, "the Shambles is in range");

        let mut downpour = WorldWeatherState::default();
        downpour.current.kind = WeatherKind::Downpour;
        app.insert_resource(downpour);
        app.update();
        let wet = vermin_mesh_vertices(&mut app);

        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("one vermin batch");
        let expected: usize = vermin
            .colonies
            .iter()
            .filter(|colony| {
                colony.anchor.distance(shambles.xz()) <= VERMIN_VISIBLE_RANGE_M + colony.radius_m
            })
            .map(|colony| colony.rats.len().div_ceil(3) * RAT_VERTICES)
            .sum();
        assert_eq!(wet, expected, "a stray third stays out in a downpour");
        assert!(wet < dry, "and that is fewer than the dry count");
    }

    /// §2.5's two dials. `enabled: false` — which is all `CATHEDRAL_NO_VERMIN`
    /// does — must spawn *nothing*, not an idling batch; `density` scales the
    /// authored per-colony counts and nothing else.
    #[test]
    fn the_ablation_switch_spawns_nothing_and_density_scales_the_counts() {
        let mut off = built_app_with(Some(VerminSettings {
            enabled: false,
            ..Default::default()
        }));
        let world = off.world_mut();
        assert!(
            world.query::<&Vermin>().iter(world).next().is_none(),
            "an ablated feature costs no entity and no per-frame rebuild"
        );

        // Density 0 is not the ablation switch: the batch exists and parks.
        let mut empty = built_app_with(Some(VerminSettings {
            density: 0.0,
            ..Default::default()
        }));
        {
            let world = empty.world_mut();
            let vermin = world
                .query::<&Vermin>()
                .single(world)
                .expect("one vermin batch");
            assert!(vermin.colonies.iter().all(|colony| colony.rats.is_empty()));
            assert!(
                vermin
                    .colonies
                    .iter()
                    .all(|colony| colony.boil_rats.is_empty())
            );
        }
        assert_eq!(vermin_mesh_vertices(&mut empty), IDLE_BATCH_VERTICES);

        // Doubling keeps every rat a single density already settled (the seed
        // is keyed on the rat's index, not on the count) and adds more.
        let counts = |app: &mut App| -> Vec<usize> {
            let world = app.world_mut();
            world
                .query::<&Vermin>()
                .single(world)
                .expect("one vermin batch")
                .colonies
                .iter()
                .map(|colony| colony.rats.len())
                .collect()
        };
        let single = counts(&mut built_app());
        let mut doubled_app = built_app_with(Some(VerminSettings {
            density: 2.0,
            ..Default::default()
        }));
        let doubled = counts(&mut doubled_app);
        for (index, (one, two)) in single.iter().zip(doubled.iter()).enumerate() {
            assert!(
                two >= one && *two <= one * 2,
                "{}: {one} at 1.0 but {two} at 2.0",
                COLONIES[index].name
            );
        }
        assert!(
            doubled.iter().sum::<usize>() > single.iter().sum::<usize>(),
            "a denser city has more rats in it"
        );
    }

    /// LE-05 defence in depth: the config sanitizer only guards the loaded
    /// file, so the count arithmetic itself must stay bounded for any
    /// `VerminSettings` — a huge finite density saturates the cast onto the
    /// per-colony cap instead of a `usize::MAX` bake loop, the boil multiply
    /// cannot overflow, and non-finite or negative values settle nothing.
    #[test]
    fn a_wild_density_cannot_unbound_the_bake() {
        let capped = (
            MAX_RATS_PER_COLONY,
            MAX_RATS_PER_COLONY * BOIL_EXTRA_RATS_PER_RAT,
        );
        assert_eq!(colony_counts(10, 1e20), capped);
        assert_eq!(colony_counts(10, f32::INFINITY), capped);
        assert_eq!(colony_counts(10, f32::MAX), capped);
        // NaN and negatives read as an empty colony, not a panic or a wrap.
        assert_eq!(colony_counts(10, f32::NAN), (0, 0));
        assert_eq!(colony_counts(10, f32::NEG_INFINITY), (0, 0));
        assert_eq!(colony_counts(10, -5.0), (0, 0));
        // And the sane path is untouched: rounding, not truncation, and the
        // supported ceiling (`config::VERMIN_DENSITY_MAX`) sits well under
        // the cap.
        assert_eq!(colony_counts(10, 1.0), (10, 20));
        assert_eq!(colony_counts(10, 0.26), (3, 6));
        assert_eq!(
            colony_counts(10, crate::config::VERMIN_DENSITY_MAX),
            (40, 80)
        );
    }

    /// A boil runs the Snuffing to the Kindling, which straddles midnight — so
    /// 21:00 on day N and 04:00 on day N+1 must be the *same* night, or one
    /// boil would become two and pick two colonies at the stroke of twelve.
    #[test]
    fn a_boil_night_spans_midnight() {
        assert_eq!(boil_night(Some(&night_clock(7, 21.0))), Some(7));
        assert_eq!(boil_night(Some(&night_clock(7, 23.5))), Some(7));
        assert_eq!(boil_night(Some(&night_clock(8, 0.0))), Some(7));
        assert_eq!(boil_night(Some(&night_clock(8, 4.0))), Some(7));
        assert_eq!(boil_night(Some(&night_clock(8, 21.0))), Some(8));
        // The Kindling closes it; the Snuffing has not yet opened the next.
        assert_eq!(boil_night(Some(&night_clock(8, 5.0))), None);
        assert_eq!(boil_night(Some(&night_clock(8, 13.0))), None);
        assert_eq!(boil_night(Some(&night_clock(8, 20.99))), None);
        // No clock is no date, and an undated boil would fire on every frame
        // of the seconds before the engine first speaks.
        assert_eq!(boil_night(None), None);
        assert_eq!(
            boil_night(Some(&WorldClockState {
                present: false,
                fraction: 23.0 / 24.0,
                ..Default::default()
            })),
            None
        );
    }

    /// The pick is a pure function of (night, seed) — every system may ask it
    /// instead of one owning it — and over a season it walks the whole table
    /// rather than favouring a corner of the city.
    #[test]
    fn the_boil_walks_the_whole_colony_table() {
        let seed = default_seed();
        let count = COLONIES.len();
        assert_eq!(
            boiling_colony(3, seed, count),
            boiling_colony(3, seed, count)
        );
        assert_eq!(boiling_colony(0, seed, 0), None, "no colonies, no boil");

        let mut nights_per_colony = vec![0_usize; count];
        for night in -30..200 {
            let index = boiling_colony(night, seed, count).expect("the table is not empty");
            assert!(index < count);
            nights_per_colony[index] += 1;
        }
        for (index, nights) in nights_per_colony.iter().enumerate() {
            assert!(
                *nights > 0,
                "{} never boils in 230 nights",
                COLONIES[index].name
            );
        }
        // A different seed is a different year of boils.
        let other: Vec<_> = (0..40)
            .map(|night| boiling_colony(night, seed ^ 0x5eed, count))
            .collect();
        let ours: Vec<_> = (0..40)
            .map(|night| boiling_colony(night, seed, count))
            .collect();
        assert_ne!(ours, other);
    }

    /// The boil complement is baked with the rest, over twice the radius, and
    /// answers to §2.1 exactly as the ordinary loops do — a rat is a rat.
    #[test]
    fn the_boil_complement_is_walkable_inside_twice_the_reach() {
        let mut app = built_app();
        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("one vermin batch");
        let collision = world.resource::<CollisionWorld>();
        for (colony, spec) in vermin.colonies.iter().zip(COLONIES.iter()) {
            assert_eq!(
                colony.boil_rats.len(),
                spec.rats * BOIL_EXTRA_RATS_PER_RAT,
                "{} settled a short boil",
                colony.name
            );
            // …which, since every colony settles its authored count, is what
            // makes the count on the ground exactly triple.
            assert_eq!(
                colony.boil_rats.len(),
                colony.rats.len() * BOIL_EXTRA_RATS_PER_RAT,
                "{} does not triple",
                colony.name
            );
            let reach = colony.radius_m * BOIL_RADIUS_SCALE;
            for rat in &colony.boil_rats {
                for leg in &rat.legs {
                    for point in [leg.from, leg.to] {
                        assert!(
                            walkable(&vermin.nav, collision, point),
                            "{}: boil waypoint {point:?} is off the walkable surface",
                            colony.name
                        );
                        assert!(
                            point.distance(colony.anchor) <= reach + 0.001,
                            "{}: boil waypoint {point:?} left the doubled radius",
                            colony.name
                        );
                    }
                }
            }
        }
    }

    /// The night the Snuffing lands, the chosen colony's batch triples — the
    /// same technique `the_clock_and_the_cull_gate_the_batch` uses, pointed at
    /// whichever colony tonight's hash picked.
    #[test]
    fn a_boiling_colony_triples_its_batch() {
        let mut app = built_app();
        let night = 3_i64;
        let index = boiling_colony(night, default_seed(), COLONIES.len()).expect("a colony boils");
        let anchor = COLONIES[index].anchor;
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::from_translation(Vec3::new(anchor.x, 6.0, anchor.y)),
        ));

        // 20:00 the same evening: dark, but the boil has not opened yet.
        app.insert_resource(night_clock(night, 20.0));
        app.update();
        let before = vermin_mesh_vertices(&mut app);

        app.insert_resource(night_clock(night, 21.5));
        app.update();
        let during = vermin_mesh_vertices(&mut app);

        let world = app.world_mut();
        let vermin = world
            .query::<&Vermin>()
            .single(world)
            .expect("one vermin batch");
        let colony = &vermin.colonies[index];
        assert!(colony.rats.len() >= 4, "{} settled rats", colony.name);
        // Every other colony is >60 m away, so the difference is this one's.
        assert_eq!(
            during - before,
            colony.boil_rats.len() * RAT_VERTICES,
            "{} should be pouring",
            colony.name
        );
        assert_eq!(during, before * 3, "three times the rats");

        // …and it is over by the Kindling.
        app.insert_resource(night_clock(night + 1, 5.5));
        app.update();
        assert_eq!(vermin_mesh_vertices(&mut app), before);
    }

    /// A boiling colony is drawn even where the darkness gate alone would have
    /// put it away — the predicate the batch and the scatter share says so, and
    /// this is the case that would silently rot if they ever diverged.
    #[test]
    fn a_boil_outranks_the_darkness_gate() {
        let night = 11_i64;
        let index = boiling_colony(night, default_seed(), COLONIES.len()).expect("a colony boils");
        let daylit = WorldClockState {
            present: true,
            day: night,
            fraction: 21.5 / 24.0,
            brightness: 1.0,
            ..Default::default()
        };
        assert_eq!(
            colony_showing(COLONIES[index].all_offices, true, Some(&daylit)),
            Some(Showing::Boil)
        );
    }

    /// The `[vermin] boil: <colony>` line the §5 verification greps for lands
    /// once a game night, not once a frame — checked on the component's own
    /// state, so no log capture is needed.
    #[test]
    fn the_boil_is_announced_once_a_night() {
        let mut app = built_app();
        let announced = |app: &mut App| {
            let world = app.world_mut();
            world
                .query::<&Vermin>()
                .single(world)
                .expect("one vermin batch")
                .announced_boil_night
        };

        app.insert_resource(night_clock(4, 20.0));
        app.update();
        assert_eq!(
            announced(&mut app),
            None,
            "nothing boils before the Snuffing"
        );

        app.insert_resource(night_clock(4, 21.5));
        app.update();
        assert_eq!(announced(&mut app), Some(4));

        // Later the same night — including past midnight — is the same boil.
        for (day, hour) in [(4_i64, 23.0), (5, 1.0), (5, 4.5)] {
            app.insert_resource(night_clock(day, hour));
            app.update();
            assert_eq!(announced(&mut app), Some(4), "still night 4");
        }

        // The Kindling ends it, and the next Snuffing is news again.
        app.insert_resource(night_clock(5, 12.0));
        app.update();
        assert_eq!(announced(&mut app), Some(4), "the last boil is remembered");
        app.insert_resource(night_clock(5, 21.5));
        app.update();
        assert_eq!(announced(&mut app), Some(5));
    }

    /// The swarm percept is paced on the sim clock, so the `T` key's 60×
    /// reaches the repeat the way it reaches the boil — and a clock that jumps
    /// backwards re-arms instead of going quiet until it has caught up.
    #[test]
    fn a_swarm_percept_is_paced_on_the_sim_clock() {
        assert!(percept_due(None, 0.0), "the first frame of a boil is heard");
        let start = game_minutes(&night_clock(4, 21.0));
        assert!(!percept_due(Some(start), start));
        assert!(!percept_due(
            Some(start),
            start + SWARM_PERCEPT_INTERVAL_MINUTES - 0.01
        ));
        assert!(percept_due(
            Some(start),
            start + SWARM_PERCEPT_INTERVAL_MINUTES
        ));
        assert!(
            percept_due(Some(start), start - 1.0),
            "a rewound clock re-arms"
        );

        // Game-minutes are monotone across the midnight a boil spans.
        assert!(game_minutes(&night_clock(5, 0.5)) > game_minutes(&night_clock(4, 23.5)));
        assert!(
            (game_minutes(&night_clock(5, 0.5)) - game_minutes(&night_clock(4, 23.5)) - 60.0).abs()
                < 1e-6
        );
    }

    /// LE-04: `density` is a visual dial — "the authored counts and nothing
    /// else" — so a colony record it left empty must boil in silence: no
    /// `[vermin] boil:` line, and above all no `rat_swarm` crossing into the
    /// sim, where every percept buys whoever hears it a paid turn. Asserted
    /// against a real bridge receiver, the very endpoint the game wires.
    #[test]
    fn an_empty_colony_boils_unannounced_and_unheard() {
        use crossbeam_channel::{Receiver, bounded};

        let announced = |app: &mut App| {
            let world = app.world_mut();
            world
                .query::<&Vermin>()
                .single(world)
                .expect("one vermin batch")
                .announced_boil_night
        };
        let drain = |commands: &Receiver<BridgeCommand>| -> Vec<BridgeCommand> {
            commands.try_iter().collect()
        };

        // Density 0 empties every colony but keeps all the authored records.
        let (sender, commands) = bounded(8);
        let mut empty = built_app_with(Some(VerminSettings {
            density: 0.0,
            ..Default::default()
        }));
        empty.insert_resource(BridgeHandle::new(
            sender,
            std::path::PathBuf::from("/tmp"),
        ));
        empty.insert_resource(night_clock(4, 21.5));
        empty.update();
        assert_eq!(announced(&mut empty), None, "a boil of nobody is not news");
        assert!(
            drain(&commands).is_empty(),
            "density 0 must never reach the bridge"
        );

        // The same guard at full density, for a lone colony whose ground
        // settled nobody: its night is silent, and a later populated pick
        // still announces and is heard.
        let (sender, commands) = bounded(8);
        let mut app = built_app();
        app.insert_resource(BridgeHandle::new(
            sender,
            std::path::PathBuf::from("/tmp"),
        ));
        let night = 4_i64;
        let index = boiling_colony(night, default_seed(), COLONIES.len()).expect("a colony boils");
        {
            let world = app.world_mut();
            let mut vermin = world
                .query::<&mut Vermin>()
                .single_mut(world)
                .expect("one vermin batch");
            let colony = &mut vermin.colonies[index];
            colony.rats.clear();
            colony.boil_rats.clear();
        }
        app.insert_resource(night_clock(night, 21.5));
        app.update();
        assert_eq!(
            announced(&mut app),
            None,
            "an uninhabited colony's night is silent"
        );
        assert!(drain(&commands).is_empty());

        let other_night = (night + 1..night + 40)
            .find(|&n| boiling_colony(n, default_seed(), COLONIES.len()) != Some(index))
            .expect("some other colony boils inside a season");
        app.insert_resource(night_clock(other_night, 21.5));
        app.update();
        assert_eq!(
            announced(&mut app),
            Some(other_night),
            "a populated colony still announces"
        );
        let sent = drain(&commands);
        assert_eq!(sent.len(), 1, "…and is heard exactly once on entry");
        assert!(matches!(
            &sent[0],
            BridgeCommand::WorldSound { sound_id, .. } if sound_id == SWARM_SOUND_ID
        ));
    }

    /// Every face of a rat is wound so its computed normal agrees with the
    /// authored outward normals — the hull's off the body, the flanks off
    /// their own sides, the crown of the arch at the sky.
    ///
    /// `double_sided: true` + `cull_mode: None` hides the opposite of this — the
    /// shader flips the normal on a back face, so an inside-out hull shades
    /// exactly like a correct one from every angle, and nothing on screen ever
    /// says otherwise. That is how `bb616d1` ("every batched box was wound
    /// inside out, and nothing showed it") happened, so the winding is asserted
    /// off the attributes rather than trusted to the picture. Checked at three
    /// headings, since the whole body is built out of `ahead`/`side`.
    #[test]
    fn a_rat_is_wound_with_its_normals_out() {
        let nav = committed_nav();
        let rat = bake_rat(
            &nav,
            &CollisionWorld::default(),
            Vec2::new(-294.0, 220.0),
            14.0,
            7,
        )
        .expect("the Shambles is walkable");

        for heading in [Vec2::X, Vec2::Y, Vec2::new(-0.6, 0.8).normalize()] {
            let (mut positions, mut normals) = (Vec::new(), Vec::new());
            let (mut uvs, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
            push_rat(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                &rat,
                Vec2::ZERO,
                heading,
                false,
                0.0,
            );
            assert_eq!(positions.len(), RAT_VERTICES);
            assert_eq!(normals.len(), RAT_VERTICES);
            assert_eq!(colors.len(), RAT_VERTICES);

            // Winding against lighting: each face's computed normal must agree
            // with the stored normals of its own corners, or the face is wound
            // inside out however right it looks under the material.
            for triangle in indices.chunks(3) {
                let [a, b, c] = [
                    triangle[0] as usize,
                    triangle[1] as usize,
                    triangle[2] as usize,
                ];
                let edge = |from: usize, to: usize| {
                    Vec3::from_array(positions[to]) - Vec3::from_array(positions[from])
                };
                let face = edge(a, b).cross(edge(a, c));
                assert!(
                    face.length() > 1e-10,
                    "triangle {a},{b},{c} is degenerate at {heading:?}"
                );
                let stored = Vec3::from_array(normals[a])
                    + Vec3::from_array(normals[b])
                    + Vec3::from_array(normals[c]);
                assert!(
                    face.normalize().dot(stored.normalize()) > 0.1,
                    "triangle {a},{b},{c} is wound against its normals at {heading:?}"
                );
            }

            // And the stored normals themselves point out, not in: the widest
            // vertex of each flank looks off its own side, the highest point
            // of the arch looks at the sky.
            let side = Vec3::new(-heading.y, 0.0, heading.x);
            let extreme = |towards: Vec3| {
                (0..positions.len())
                    .max_by(|a, b| {
                        let reach = |v: &usize| Vec3::from_array(positions[*v]).dot(towards);
                        reach(a).total_cmp(&reach(b))
                    })
                    .expect("a rat has vertices")
            };
            for (label, towards) in [("right flank", side), ("left flank", -side)] {
                let normal = Vec3::from_array(normals[extreme(towards)]);
                assert!(
                    normal.dot(towards) > 0.5,
                    "the {label} faces out at {heading:?}: {normal:?}"
                );
            }
            let crown = Vec3::from_array(normals[extreme(Vec3::Y)]);
            assert!(
                crown.y > 0.5,
                "the arch's crown faces the sky at {heading:?}: {crown:?}"
            );
        }
    }

    /// The gait reshapes a rat but never its topology: sprinting or paused,
    /// every frame writes exactly [`RAT_VERTICES`] vertices over the same
    /// indices — which is what entitles the batch-counting tests to multiply —
    /// while a sprint actually moves the body between instants, the feet stay
    /// on the ground plane, and the full-length tail keeps the whole animal
    /// about twice its body length nose to tip.
    #[test]
    fn the_gait_bends_a_rat_but_never_its_topology() {
        let nav = committed_nav();
        let rat = bake_rat(
            &nav,
            &CollisionWorld::default(),
            Vec2::new(-294.0, 220.0),
            14.0,
            7,
        )
        .expect("the Shambles is walkable");

        let build = |moving: bool, elapsed: f32| {
            let (mut positions, mut normals) = (Vec::new(), Vec::new());
            let (mut uvs, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new());
            push_rat(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                &rat,
                Vec2::ZERO,
                Vec2::X,
                moving,
                elapsed,
            );
            (positions, indices)
        };

        let (paused, paused_indices) = build(false, 0.0);
        let (sprint, sprint_indices) = build(true, 0.35);
        let (sprint_later, _) = build(true, 0.55);
        assert_eq!(paused.len(), RAT_VERTICES);
        assert_eq!(sprint.len(), RAT_VERTICES);
        assert_eq!(
            paused_indices, sprint_indices,
            "the gait never re-plumbs the mesh"
        );
        assert!(
            paused_indices
                .iter()
                .all(|index| (*index as usize) < RAT_VERTICES),
            "every index points into the rat"
        );
        assert_ne!(sprint, sprint_later, "a sprinting rat is never a statue");

        for (label, vertices) in [("paused", &paused), ("sprinting", &sprint)] {
            let low = vertices.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
            let high = vertices
                .iter()
                .map(|v| v[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                low >= RAT_GROUND_Y - 1e-4,
                "a {label} rat digs under the street: {low}"
            );
            assert!(
                low <= RAT_GROUND_Y + 0.002,
                "a {label} rat's feet reach the ground: {low}"
            );
            assert!(
                high <= RAT_GROUND_Y + 0.13,
                "a {label} rat is taller than a rat: {high}"
            );
        }

        // Nose to tail tip along the heading (`Vec2::X`): the body plus the
        // 0.9-length tail, so a rat that lost its tail would fail here.
        let reach = |vertices: &Vec<[f32; 3]>| {
            let max = vertices
                .iter()
                .map(|v| v[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let min = vertices.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min);
            max - min
        };
        let length = rat.length_m;
        assert!(
            (1.6 * length..=2.1 * length).contains(&reach(&paused)),
            "the tail is the other half of a rat: {} of {length}",
            reach(&paused)
        );
    }

    /// Heavy rain thins the visible count, matching the animals going quiet —
    /// on `soundscape::wildlife_suppressed` itself, which the vermin layer now
    /// shares with the birds rather than restating.
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
