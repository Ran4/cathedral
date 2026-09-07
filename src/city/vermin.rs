//! Rats in the fish lanes and the slaughter courts.
//!
//! Roughly fifty ordinary rats in eight authored colonies — drawn and heard,
//! never simulated: no per-rat entities, no colliders, no nav rebake, no sim
//! verb, and an ordinary rat never costs a token. The chimney-smoke pattern at
//! smaller scale: one `Vermin` entity, one batched mesh rewritten per frame,
//! and per-rat routes a pure function of the clock — each rat's
//! sprint–pause–sprint waypoint loop is baked once at startup from
//! `(colony seed, rat index)` and validated against the same navigation and
//! collision the player walks, so a rat is confined by *reading* the world,
//! never by writing a collider into it (`collision_footprints.json` stays
//! byte-identical). Fixed-size cosmetic state follows the final displacement,
//! with one scatter impulse per colony: rats that ignore you are wallpaper;
//! rats that flee you are alive.
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
    mesh_batch::{BatchScratch, idle_batch_mesh, write_batch_mesh_reusing},
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
const RAT_GROUND_Y: f32 = 0.0125;
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
    motion: RatMotion,
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
    /// The colony-wide footfall impulse; cosmetic pose state lives on each rat.
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
        motion: RatMotion::default(),
    })
}

impl Rat {
    /// The budget available to a cosmetic action before the next baked departure.
    fn pause_remaining(&self, elapsed: f32) -> f32 {
        let t = (elapsed + self.phase).rem_euclid(self.period.max(0.001));
        self.legs
            .iter()
            .find(|leg| t < leg.arrive)
            .map_or(0.0, |leg| (leg.depart - t).max(0.0))
    }

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
/// derives the same colony selection — it asks [`boiling_colony_now`] the same question
/// and gets the same answer. Cosmetic pose state remains local to each rat.
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
    let _span = crate::perf::span(crate::perf::Probe::Vermin);
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
    // Both settle on the cast's size after the first frame and are never
    // allocated again; a startle reflex runs every frame, so the two `Vec`s
    // this used to build per frame were pure allocator traffic.
    mut movers: Local<Vec<Vec2>>,
    mut near: Local<Vec<Vec2>>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Vermin);
    let elapsed = time.elapsed_secs();
    let clock = clock.as_deref();
    // Only somebody standing on the ground is a footfall: a player in
    // developer flight, or crossing one of the city's overhead bridges, passes
    // over a colony without ever putting a foot near it.
    movers.clear();
    movers.extend(
        player
            .iter()
            .map(|transform| transform.translation)
            .chain(actors.iter().map(|transform| transform.translation()))
            .filter(|translation| translation.y <= SCATTER_MAX_FOOT_Y)
            .map(|translation| translation.xz()),
    );
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
            near.clear();
            near.extend(
                movers
                    .iter()
                    .copied()
                    .filter(|mover| mover.distance(colony.anchor) <= reach),
            );
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
    mut vermin: Query<(&mut Vermin, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    // Held across frames: a boiling colony's batch is the same size every frame
    // it boils, so growing five fresh `Vec`s to it each time was work with no
    // answer attached. The batch write hands them back empty.
    mut scratch: Local<BatchScratch>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Vermin);
    let Ok(camera) = camera.single() else {
        return;
    };
    let camera_position = camera.translation();
    let elapsed = time.elapsed_secs();
    let clock = clock.as_deref();
    let suppressed = rain_suppressed(weather.as_deref());

    for (mut vermin, mesh_handle) in &mut vermin {
        let vermin = &mut *vermin;
        let batch = &mut *scratch;
        let boiling = boiling_colony_now(clock, vermin.seed, vermin.colonies.len());
        for (index, colony) in vermin.colonies.iter_mut().enumerate() {
            let Some(showing) = colony_showing(colony.all_offices, boiling == Some(index), clock)
            else {
                continue;
            };
            let colony_range = VERMIN_VISIBLE_RANGE_M + colony.showing_radius_m(showing);
            if colony.anchor.distance_squared(camera_position.xz()) > colony_range * colony_range {
                continue;
            }
            let scatter = colony.scatter;
            let extra = if matches!(showing, Showing::Boil) {
                colony.boil_rats.as_mut_slice()
            } else {
                &mut []
            };
            for (rat_index, rat) in colony.rats.iter_mut().chain(extra).enumerate() {
                // Heavy rain thins the colony to a stray third, matching the
                // animals going quiet in the soundscape.
                if suppressed && rat_index % 3 != 0 {
                    continue;
                }
                let (loop_position, loop_heading, _) = rat.sample(elapsed);
                let offset = scatter_offset(
                    &vermin.nav,
                    &collision,
                    rat,
                    loop_position,
                    scatter,
                    elapsed,
                );
                let position = loop_position + offset;
                let pause_remaining = rat.pause_remaining(elapsed);
                let pose = rat.motion.update(
                    rat.seed,
                    rat.length_m,
                    position,
                    loop_heading,
                    elapsed,
                    pause_remaining,
                );
                push_rat(
                    &mut batch.positions,
                    &mut batch.normals,
                    &mut batch.uvs,
                    &mut batch.colors,
                    &mut batch.indices,
                    rat,
                    position,
                    &pose,
                );
            }
        }
        write_batch_mesh_reusing(&mut meshes, &mesh_handle.0, batch);
    }
}

// ---------------------------------------------------------------------------
// The rat itself: a continuous hull and small shaped features, written straight
// into the batch each frame like a smoke puff. There is no per-rat mesh asset
// to place, so the gait is free to reshape the body — stretch it into a
// sprint, beat the legs, whip the tail — without a rig.
// ---------------------------------------------------------------------------

const BODY_SECTORS: usize = 12;
const BODY_STATIONS: usize = 14;
const TAIL_SECTORS: usize = 6;
const TAIL_STATIONS: usize = 11;

/// Nose to rump, in metres for a 28 cm rat. The low shoulder saddle separates
/// the cranium from the rounded haunch without a seam between body parts.
const BODY_PROFILE: [(f32, f32, f32, f32); BODY_STATIONS] = [
    (0.148, 0.0045, 0.004, 0.043), // small nose above a shallow chin
    (0.140, 0.008, 0.006, 0.042),
    (0.124, 0.014, 0.010, 0.040), // muzzle and chin share the same surface
    (0.111, 0.018, 0.014, 0.044),
    (0.102, 0.026, 0.022, 0.047), // cheek below the orbital ridge
    (0.083, 0.029, 0.028, 0.054), // rounded cranium
    (0.060, 0.026, 0.025, 0.050),
    (0.035, 0.031, 0.032, 0.056),
    (-0.028, 0.043, 0.042, 0.058),
    (-0.063, 0.047, 0.047, 0.063),
    (-0.095, 0.043, 0.044, 0.059),
    (-0.123, 0.029, 0.031, 0.044),
    (-0.138, 0.017, 0.018, 0.032),
    (-0.151, 0.0065, 0.0075, 0.024), // fur narrows over the naked tail root
];

// Constant topology, including the underside of the ears and all twelve toes.
const RAT_VERTICES: usize = BODY_STATIONS * BODY_SECTORS
    + TAIL_STATIONS * TAIL_SECTORS
    + 2 * 26
    + 3 * 14
    + 2 * 26
    + 4 * (18 + 14 + 3 * 6)
    + 6 * 4;

/// Local attachment points in the frozen 28 cm appearance.
const RAT_LEGS: [(f32, f32, f32); 4] = [
    (0.050, 0.024, 0.032),
    (0.050, -0.024, 0.032),
    (-0.075, 0.030, 0.038),
    (-0.075, -0.030, 0.038),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RatAction {
    #[default]
    Quiet,
    Sniff,
    Alert,
    Groom,
}

/// All dimensions in this pose are cosmetic; no geometry requests navigation.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RatPose {
    heading: Vec2,
    speed: f32,
    phase: f32,
    tail_phase: f32,
    travel: f32,
    turn: f32,
    sniff: f32,
    alert: f32,
    groom: f32,
    head_yaw: f32,
    head_bow: f32,
    nose: f32,
    ears: [f32; 2],
    tail_bend: f32,
    tail_tip: f32,
    /// World-sized offsets expressed in the visible body's forward/up/side frame.
    feet: [Vec3; 4],
    paw_curl: [f32; 4],
    action: RatAction,
}
impl Default for RatPose {
    fn default() -> Self {
        Self {
            heading: Vec2::X,
            speed: 0.0,
            phase: 0.0,
            tail_phase: 0.0,
            travel: 0.0,
            turn: 0.0,
            sniff: 0.0,
            alert: 0.0,
            groom: 0.0,
            head_yaw: 0.0,
            head_bow: 0.0,
            nose: 0.0,
            ears: [0.0; 2],
            tail_bend: 0.0,
            tail_tip: 0.0,
            feet: [Vec3::ZERO; 4],
            paw_curl: [0.0; 4],
            action: RatAction::Quiet,
        }
    }
}

/// Retained, fixed-size cosmetic state: no entities, allocations or events.
#[derive(Clone, Debug, Default)]
struct RatMotion {
    last_time: Option<f32>,
    last_position: Vec2,
    tail_heading: Vec2,
    phase: f32,
    tail_phase: f32,
    feet: [Vec2; 4],
    swing_from: [Vec2; 4],
    foot_height: [f32; 4],
    was_stance: [bool; 4],
    wash_offset: [Vec3; 2],
    wash_curl: [f32; 2],
    idle_age: f32,
    bout: u64,
    action: RatAction,
    action_age: f32,
    action_duration: f32,
    next_action: f32,
    pose: RatPose,
}

fn rat_smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// End an ease at a visually negligible boundary. Otherwise f32 subtraction
/// can stall at the smallest subnormal and make every subsequent vertex costly.
fn rat_snap(value: f32, epsilon: f32) -> f32 {
    if value.abs() < epsilon { 0.0 } else { value }
}

fn rat_angle(from: Vec2, to: Vec2) -> f32 {
    from.perp_dot(to).atan2(from.dot(to))
}

impl RatMotion {
    /// Follow the *final* validated position, including scatter attack/hold/return.
    /// The caller evaluates navigation once; this method only observes its result.
    fn update(
        &mut self,
        seed: u64,
        length_m: f32,
        position: Vec2,
        heading_hint: Vec2,
        elapsed: f32,
        pause_remaining: f32,
    ) -> RatPose {
        let scale = length_m / 0.28;
        let dt = self.last_time.map_or(f32::INFINITY, |last| elapsed - last);
        if dt == 0.0 {
            return self.pose;
        }
        let displacement = position - self.last_position;
        // A newly visible rat must not turn a hidden minute into one enormous stride.
        if !dt.is_finite() || !(0.0..=0.25).contains(&dt) || displacement.length() > 1.0 {
            *self = Self::default();
            self.last_time = Some(elapsed);
            self.last_position = position;
            self.pose.heading = heading_hint.normalize_or(Vec2::X);
            self.tail_heading = self.pose.heading;
            self.next_action = 0.22 + unit(seed, 701) * 0.23;
            let side = self.pose.heading.perp();
            for (index, (along, flank, _)) in RAT_LEGS.into_iter().enumerate() {
                let rest = Vec2::new(
                    along + if index < 2 { 0.008 } else { -0.006 },
                    flank + flank.signum() * 0.007,
                ) * scale;
                self.feet[index] = position + self.pose.heading * rest.x + side * rest.y;
                self.swing_from[index] = self.feet[index];
                self.was_stance[index] = true;
                self.pose.feet[index] = Vec3::new(rest.x, 0.0, rest.y);
            }
            return self.pose;
        }
        self.last_time = Some(elapsed);
        self.last_position = position;
        let distance = displacement.length();
        let speed = distance / dt;
        let moving = speed > 0.025;
        let yaw_error = if moving {
            rat_angle(self.pose.heading, displacement / distance)
        } else {
            0.0
        };
        let heading = if moving {
            let target = displacement / distance;
            let turn = rat_angle(self.pose.heading, target).clamp(-26.0 * dt, 26.0 * dt);
            Vec2::from_angle(turn)
                .rotate(self.pose.heading)
                .normalize_or(target)
        } else {
            self.pose.heading
        };
        let tail_turn = rat_angle(self.tail_heading, heading);
        self.tail_heading = if tail_turn.abs() < 0.0001 {
            heading
        } else {
            Vec2::from_angle(tail_turn * (1.0 - (-5.5 * dt).exp()))
                .rotate(self.tail_heading)
                .normalize_or(heading)
        };
        let side = heading.perp();
        let stride = 0.18 * scale;
        if moving {
            self.phase = (self.phase + distance / stride).rem_euclid(1.0);
            self.tail_phase = (self.tail_phase + distance / stride * 0.5).rem_euclid(1.0);
        }
        let travel_target = if moving { (speed / 0.8).min(1.0) } else { 0.0 };
        let travel = rat_snap(
            self.pose.travel + (travel_target - self.pose.travel) * (1.0 - (-22.0 * dt).exp()),
            0.0001,
        );
        let turn_target = (yaw_error.abs() / 1.3).min(1.0);
        let turn = rat_snap(
            self.pose.turn + (turn_target - self.pose.turn) * (1.0 - (-20.0 * dt).exp()),
            0.0001,
        );
        if moving {
            if self.idle_age > 0.0 {
                self.bout = self.bout.wrapping_add(1);
            }
            self.idle_age = 0.0;
            self.action = RatAction::Quiet;
            self.action_age = 0.0;
            self.next_action = 0.18 + unit(seed, 701 + self.bout * 17) * 0.22;
        } else {
            self.idle_age += dt;
            self.action_age += dt;
            if self.action != RatAction::Quiet && self.action_age >= self.action_duration {
                self.action = RatAction::Quiet;
                self.next_action = self.idle_age + 0.22 + unit(seed, 705 + self.bout * 17) * 0.45;
                self.bout = self.bout.wrapping_add(1);
            }
            if self.action == RatAction::Quiet
                && self.idle_age >= self.next_action
                && pause_remaining > 0.38
            {
                let choice = mix(seed ^ self.bout.wrapping_mul(31) ^ 0xace5) % 5;
                let (action, duration) = if choice == 0 && pause_remaining > 1.82 {
                    (
                        RatAction::Groom,
                        (1.58 + unit(seed, 710 + self.bout) * 0.16).min(pause_remaining - 0.12),
                    )
                } else if choice == 1 && pause_remaining > 0.75 {
                    (RatAction::Alert, 0.58 + unit(seed, 711 + self.bout) * 0.16)
                } else {
                    (
                        RatAction::Sniff,
                        (0.62 + unit(seed, 712 + self.bout) * 0.34).min(pause_remaining - 0.12),
                    )
                };
                self.action = action;
                self.action_age = 0.0;
                self.action_duration = duration;
            }
        }
        let envelope = rat_smooth(self.action_age / 0.14)
            * rat_smooth((self.action_duration - self.action_age) / 0.18)
            * rat_smooth(pause_remaining / 0.13);
        let blend = 1.0 - (-28.0 * dt).exp();
        let weight = |old: f32, action: RatAction| {
            rat_snap(
                old + ((if self.action == action { envelope } else { 0.0 }) - old) * blend,
                0.0001,
            )
        };
        let sniff = weight(self.pose.sniff, RatAction::Sniff);
        let alert = weight(self.pose.alert, RatAction::Alert);
        let groom = weight(self.pose.groom, RatAction::Groom);
        let clock = elapsed + unit(seed, 719) * 9.0;
        let nose = sniff * (clock * 34.0).sin() * (0.6 + 0.4 * (clock * 11.0).sin());
        let wash_time = (self.action_age - 0.22).max(0.0);
        let wash_cycle = (wash_time / 0.54).fract();
        let wash_half = (wash_time / 0.54).floor() as usize;
        let sweep =
            rat_smooth((wash_cycle - 0.28) / 0.37) * (1.0 - rat_smooth((wash_cycle - 0.68) / 0.22));
        let head_bow_target = groom * mixf(-0.014, -0.026, sweep);
        let head_bow = rat_snap(
            self.pose.head_bow + (head_bow_target - self.pose.head_bow) * blend,
            0.000001,
        );
        let mut pose = RatPose {
            heading,
            speed,
            phase: self.phase,
            tail_phase: self.tail_phase,
            travel,
            turn,
            sniff,
            alert,
            groom,
            head_yaw: {
                let turn_lead = if moving {
                    rat_angle(heading, displacement.normalize()).clamp(-0.30, 0.30)
                } else {
                    0.0
                };
                let target = sniff * 0.18 * (self.action_age * 6.0 + unit(seed, 720) * 6.0).sin()
                    + alert * 0.12 * (self.action_age * 4.0).sin()
                    + groom * if wash_half % 2 == 0 { 0.10 } else { -0.10 }
                    + turn_lead;
                rat_snap(
                    self.pose.head_yaw + (target - self.pose.head_yaw) * (1.0 - (-15.0 * dt).exp()),
                    0.00001,
                )
            },
            head_bow,
            nose,
            ears: [0.0; 2],
            tail_bend: -rat_angle(self.tail_heading, heading).clamp(-1.10, 1.10),
            tail_tip: if moving {
                0.0
            } else {
                (clock * 3.7).sin() * sniff
            },
            feet: [Vec3::ZERO; 4],
            paw_curl: [0.0; 4],
            action: self.action,
        };
        for (ear, value) in pose.ears.iter_mut().enumerate() {
            let twitch = (clock * 1.7 + ear as f32 * 2.4).sin();
            *value = alert * (if ear == 0 { -0.16 } else { 0.08 })
                + (twitch - 0.93).max(0.0) * 4.0 * (clock * 42.0).sin();
        }
        for (index, (along, flank, _)) in RAT_LEGS.into_iter().enumerate() {
            let cycle = (self.phase + [0.0, 0.5, 0.5, 0.0][index]).fract();
            let brace_leg = if yaw_error < 0.0 { 3 } else { 2 };
            let brace = moving
                && index == brace_leg
                && ((yaw_error.abs() > 0.35 && turn > 0.1) || self.pose.groom > 0.2);
            let stance = cycle < 0.55 || brace;
            let target = position
                + heading * ((along + 0.0495) * scale)
                + side * ((flank + flank.signum() * 0.007) * scale);
            if moving {
                if stance {
                    if !self.was_stance[index] {
                        self.feet[index] = target - heading * (cycle * stride);
                    }
                    self.foot_height[index] = 0.0;
                    self.swing_from[index] = self.feet[index];
                } else {
                    if self.was_stance[index] {
                        self.swing_from[index] = self.feet[index];
                    }
                    let swing = (cycle - 0.55) / 0.45;
                    self.feet[index] = self.swing_from[index].lerp(target, rat_smooth(swing));
                    self.foot_height[index] = (std::f32::consts::PI * swing).sin() * 0.014 * scale;
                }
                self.was_stance[index] = stance;
            } else {
                // Finish only the vertical part of an interrupted step: no treadmill.
                self.foot_height[index] = (self.foot_height[index] - dt * 0.20 * scale).max(0.0);
            }
            // A fast escape can cross a whole stance between rendered frames.
            // Replant an overreached contact rather than stretching a limb across
            // the turning torso; the new contact uses the already validated body.
            let neutral = position
                + heading * (along * scale)
                + side * ((flank + flank.signum() * 0.007) * scale);
            if self.feet[index].distance(neutral) > 0.061 * scale {
                self.feet[index] = neutral + heading * (0.018 * scale);
                self.swing_from[index] = self.feet[index];
                self.foot_height[index] = 0.0;
                self.was_stance[index] = true;
            }
            let relative = self.feet[index] - position;
            pose.feet[index] = Vec3::new(
                relative.dot(heading),
                self.foot_height[index],
                relative.dot(side),
            );
            // Hind paws gather before the hands lift. Their staggered little
            // steps are supported by both front feet during entry and exit.
            if index >= 2 {
                let gather = rat_smooth((groom - if index == 2 { 0.0 } else { 0.35 }) / 0.40);
                pose.feet[index].x += gather * 0.028 * scale;
                pose.feet[index].y +=
                    (std::f32::consts::PI * gather).sin().max(0.0) * 0.006 * scale;
            }
            // Both hands gather at the muzzle only once the rump and hind paws
            // support the crouch. One hand trails the other slightly in the wipe.
            if index < 2 {
                let hand_cycle = ((wash_time - index as f32 * 0.025).max(0.0) / 0.54).fract();
                let hand_sweep = rat_smooth((hand_cycle - 0.28) / 0.37)
                    * (1.0 - rat_smooth((hand_cycle - 0.68) / 0.22));
                let wash = groom
                    * rat_smooth((groom - 0.88) / 0.12)
                    * rat_smooth((self.action_age - 0.22) / 0.10)
                    * rat_smooth((self.action_duration - self.action_age - 0.16) / 0.12)
                    * rat_smooth(hand_cycle / 0.16)
                    * (1.0 - rat_smooth((hand_cycle - 0.94) / 0.06));
                let sign = flank.signum();
                // Lick the paw at the muzzle, wipe toward the eye/ear, return
                // to the mouth, then plant before the opposite paw starts.
                // These are the same stations as the rendered muzzle and brow.
                // Subtract the middle toe's actual extension so the fingertips,
                // rather than the wrist, contact the moving face.
                let mouth = rat_local_station(&pose, 0.133, 0.043) + Vec3::Z * (sign * 0.012);
                let brow = rat_local_station(&pose, 0.085, 0.077) + Vec3::Z * (sign * 0.025);
                let curl = mixf(0.40, 1.10, hand_sweep);
                let toe = Vec3::new(
                    0.020 * curl.cos() - 0.00035 * curl.sin(),
                    0.020 * curl.sin() + 0.00035 * curl.cos(),
                    0.0,
                );
                let face = (mouth.lerp(brow, hand_sweep) - toe) * scale;
                let target = (face - pose.feet[index]) * wash;
                let settle = 1.0 - (-45.0 * dt).exp();
                self.wash_offset[index] += (target - self.wash_offset[index]) * settle;
                for axis in 0..3 {
                    self.wash_offset[index][axis] =
                        rat_snap(self.wash_offset[index][axis], 0.000001 * scale);
                }
                self.wash_curl[index] = rat_snap(
                    self.wash_curl[index] + (wash * curl - self.wash_curl[index]) * settle,
                    0.0001,
                );
                pose.feet[index] += self.wash_offset[index];
                pose.paw_curl[index] = self.wash_curl[index];
            }
        }
        self.pose = pose;
        pose
    }
}

/// Shared body deformation in the 28 cm model frame. The wash targets use
/// the very same moving surface as the visible head, including its bow and turn.
fn rat_local_station(pose: &RatPose, along: f32, height: f32) -> Vec3 {
    let gait = pose.phase * std::f32::consts::TAU;
    let stretch =
        1.0 + pose.travel * (0.040 + 0.025 * gait.sin()) - pose.turn * 0.035 * (1.0 - pose.groom);
    let squat = 1.0 - 0.015 * pose.travel;
    let head = ((along - 0.035) / 0.09).clamp(0.0, 1.0);
    let chest = ((along + 0.045) / 0.09).clamp(0.0, 1.0);
    let shoulder_wave = pose.travel * 0.0035 * (gait + chest * 1.8).sin();
    let seat = ((0.010 - along) / 0.10).clamp(0.0, 1.0);
    let crouch = rat_smooth(pose.groom / 0.75);
    let lift = pose.alert * 0.034 * head - pose.sniff * 0.012 * head
        + pose.head_bow * head
        + (pose.alert * 0.008 + pose.groom * 0.021) * chest
        + pose.nose * 0.0013 * head
        + shoulder_wave
        - crouch * 0.012 * seat
        - pose.turn * 0.0035 * (1.0 - crouch) * (0.45 + 0.55 * chest);
    let bob = pose.travel * 0.0035 * (0.5 + 0.5 * (gait * 2.0).cos());
    Vec3::new(
        along * stretch - crouch * 0.028 - pose.groom * (0.023 * head + 0.009 * chest)
            + pose.travel * 0.0025 * gait.sin() * chest,
        height * squat + lift + bob,
        pose.head_yaw * head * (along - 0.035).max(0.0),
    )
}

fn mixf(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Stack-only writer into the city's retained batch buffers. Small append-only
/// primitives do not allocate after those buffers have reached their high water.
struct RatMesh<'a> {
    positions: &'a mut Vec<[f32; 3]>,
    normals: &'a mut Vec<[f32; 3]>,
    uvs: &'a mut Vec<[f32; 2]>,
    colors: &'a mut Vec<[f32; 4]>,
    indices: &'a mut Vec<u32>,
}

impl RatMesh<'_> {
    fn vertex(&mut self, p: Vec3, n: Vec3, color: [f32; 4]) -> u32 {
        let index = self.positions.len() as u32;
        self.positions.push(p.to_array());
        self.normals.push(n.normalize_or(Vec3::Y).to_array());
        self.uvs.push([0.0, 0.0]);
        self.colors.push(color);
        index
    }

    fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    /// Two latitude rings and two actual poles: no zero-area polar triangles.
    fn ellipsoid(&mut self, center: Vec3, axes: [Vec3; 3], radii: Vec3, color: [f32; 4]) {
        let [ahead, side, up] = axes;
        let first = self.positions.len() as u32;
        for longitudinal in [0.55_f32, -0.55] {
            for sector in 0..6 {
                let (sin, cos) = rat_circle(sector, 6);
                let local = Vec3::new(longitudinal, cos * 0.8351647, sin * 0.8351647);
                self.vertex(
                    center
                        + ahead * (local.x * radii.x)
                        + side * (local.y * radii.y)
                        + up * (local.z * radii.z),
                    ahead * (local.x / radii.x)
                        + side * (local.y / radii.y)
                        + up * (local.z / radii.z),
                    color,
                );
            }
        }
        stitch_tube(self.indices, first, 2, 6);
        let front = self.vertex(center + ahead * radii.x, ahead, color);
        let rear = self.vertex(center - ahead * radii.x, -ahead, color);
        for sector in 0..6 {
            let next = (sector + 1) % 6;
            self.triangle(front, first + next, first + sector);
            self.triangle(rear, first + 6 + sector, first + 6 + next);
        }
    }
    /// Extra silhouette resolution is reserved for the two exposed thighs.
    fn haunch(&mut self, center: Vec3, axes: [Vec3; 3], radii: Vec3, color: [f32; 4]) {
        let [ahead, side, up] = axes;
        let first = self.positions.len() as u32;
        for (longitudinal, radius) in [
            (0.70710677, 0.70710677),
            (0.0, 1.0),
            (-0.70710677, 0.70710677),
        ] {
            for sector in 0..8 {
                let (sin, cos) = rat_circle(sector, 8);
                let local = Vec3::new(longitudinal, cos * radius, sin * radius);
                self.vertex(
                    center
                        + ahead * (local.x * radii.x)
                        + side * (local.y * radii.y)
                        + up * (local.z * radii.z),
                    ahead * (local.x / radii.x)
                        + side * (local.y / radii.y)
                        + up * (local.z / radii.z),
                    color,
                );
            }
        }
        stitch_tube(self.indices, first, 3, 8);
        let front = self.vertex(center + ahead * radii.x, ahead, color);
        let rear = self.vertex(center - ahead * radii.x, -ahead, color);
        for sector in 0..8 {
            let next = (sector + 1) % 8;
            self.triangle(front, first + next, first + sector);
            self.triangle(rear, first + 16 + sector, first + 16 + next);
        }
    }
}

/// Fixed ring samples avoid trigonometry per vertex in the frame hot path.
fn rat_circle(sector: usize, sectors: usize) -> (f32, f32) {
    const RING: [(f32, f32); 12] = [
        (0.0, 1.0),
        (0.5, 0.8660254),
        (0.8660254, 0.5),
        (1.0, 0.0),
        (0.8660254, -0.5),
        (0.5, -0.8660254),
        (0.0, -1.0),
        (-0.5, -0.8660254),
        (-0.8660254, -0.5),
        (-1.0, 0.0),
        (-0.8660254, 0.5),
        (-0.5, 0.8660254),
    ];
    const OCTAGON: [(f32, f32); 8] = [
        (0.0, 1.0),
        (0.70710677, 0.70710677),
        (1.0, 0.0),
        (0.70710677, -0.70710677),
        (0.0, -1.0),
        (-0.70710677, -0.70710677),
        (-1.0, 0.0),
        (-0.70710677, 0.70710677),
    ];
    if sectors == 8 {
        OCTAGON[sector]
    } else {
        RING[sector * (12 / sectors)]
    }
}

/// Continuous coat hull, cupped ears, embedded eyes and a naked tapered tail.
/// The persistent pose deforms the hull and its attached features together.
#[allow(clippy::too_many_arguments)]
fn push_rat(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    rat: &Rat,
    position: Vec2,
    pose: &RatPose,
) {
    use std::f32::consts::{PI, TAU};
    let first_vertex = positions.len();
    let scale = rat.length_m / 0.28;
    let center = Vec3::new(position.x, RAT_GROUND_Y, position.y);
    let heading = pose.heading;
    let ahead = Vec3::new(heading.x, 0.0, heading.y);
    let side = Vec3::new(-heading.y, 0.0, heading.x);
    let axes = [ahead, side, Vec3::Y];
    let gait = pose.phase * TAU;
    let stretch =
        1.0 + pose.travel * (0.040 + 0.025 * gait.sin()) - pose.turn * 0.035 * (1.0 - pose.groom);
    let squat = 1.0 - 0.015 * pose.travel;
    let station = |along: f32, height: f32| {
        let local = rat_local_station(pose, along, height) * scale;
        center + ahead * local.x + side * local.z + Vec3::Y * local.y
    };
    let tint = rat.tint;
    let grey = unit(rat.seed, 34);
    let coat = [
        mixf(0.095, 0.073, grey) * tint,
        mixf(0.068, 0.065, grey) * tint,
        mixf(0.044, 0.056, grey) * tint,
    ];
    let body_color = |up: f32, along: f32| {
        let belly = (-up).max(0.0) * 0.38;
        let back = up.max(0.0) * 0.12;
        // Broad, restrained color variation follows anatomy, never random faces.
        let muzzle = ((along - 0.08) / 0.06).clamp(0.0, 1.0) * 0.12;
        [
            coat[0] * (1.0 + belly + muzzle - back),
            coat[1] * (1.0 + belly * 1.25 + muzzle - back),
            coat[2] * (1.0 + belly * 1.6 + muzzle - back),
            1.0,
        ]
    };
    let naked = [0.18 * tint, 0.105 * tint, 0.085 * tint, 1.0];
    let feet = [0.21 * tint, 0.125 * tint, 0.10 * tint, 1.0];
    let eye = [0.009, 0.007, 0.006, 1.0];
    let mut mesh = RatMesh {
        positions,
        normals,
        uvs,
        colors,
        indices,
    };

    let hull_first = mesh.positions.len() as u32;
    for (index, &(along, width, height, spine)) in BODY_PROFILE.iter().enumerate() {
        let fore = BODY_PROFILE[index.saturating_sub(1)];
        let aft = BODY_PROFILE[(index + 1).min(BODY_STATIONS - 1)];
        for sector in 0..BODY_SECTORS {
            let (sin, cos) = rat_circle(sector, BODY_SECTORS);
            let brow = (1.0 - ((along - 0.097) / 0.020).abs()).max(0.0) * sin.max(0.0) * 0.002;
            let pad = (1.0 - ((along - 0.130) / 0.018).abs()).max(0.0) * (1.0 - sin.abs()) * 0.0025;
            let point = station(along, spine)
                + side * (cos * (width + brow + pad) * scale)
                + Vec3::Y * (sin * (height + brow) * squat * scale);
            let tangent = station(fore.0, fore.3 + sin * fore.2)
                - station(aft.0, aft.3 + sin * aft.2)
                + side * (cos * (fore.1 - aft.1) * scale);
            let around = -side * (sin * width) + Vec3::Y * (cos * height * squat);
            mesh.vertex(point, tangent.cross(around), body_color(sin, along));
        }
    }
    stitch_tube(mesh.indices, hull_first, BODY_STATIONS, BODY_SECTORS);

    let tail_first = mesh.positions.len() as u32;
    let tail_length = rat.length_m * 0.9;
    let swirl = unit(rat.seed, 51) * TAU;
    let root_height = rat_local_station(pose, -0.151, BODY_PROFILE[BODY_STATIONS - 1].3).y;
    for index in 0..TAIL_STATIONS {
        let fraction = index as f32 / (TAIL_STATIONS - 1) as f32;
        let resting_curve = (swirl + fraction * 3.2).sin() * 0.014 * fraction;
        let scurry_curve = (pose.tail_phase * TAU - fraction * 4.0).sin() * 0.009 * fraction;
        let lateral = mixf(resting_curve, scurry_curve, pose.travel);
        // The resting curve is retained anatomy; the travel wave blends into
        // it while the tail's arc follows the previous heading through a turn.
        let lateral = lateral
            + 0.035 * (fraction * PI * 1.35).sin() * fraction
            + pose.tail_tip * 0.006 * fraction.powi(4);
        let droop = fraction * fraction * (3.0 - 2.0 * fraction);
        let radius = mixf(0.0065, 0.00055, fraction) * scale;
        let height = mixf(root_height * scale, 0.0011 * scale, droop).max(radius + 0.0004 * scale);
        let bend = pose.tail_bend * fraction;
        let tail_axis = ahead * bend.cos() + side * bend.sin();
        let tangent =
            -ahead * (bend.cos() - bend * bend.sin()) - side * (bend.sin() + bend * bend.cos());
        let tail_side = Vec3::Y.cross(tangent).normalize_or(side);
        let ring_center =
            center - ahead * (0.147 * stretch * scale) - tail_axis * (fraction * tail_length)
                + side * (lateral * scale)
                + Vec3::Y * height;
        for sector in 0..TAIL_SECTORS {
            let (sin, cos) = rat_circle(sector, TAIL_SECTORS);
            let shade = 0.92 + 0.08 * sin;
            mesh.vertex(
                ring_center + tail_side * (cos * radius) + Vec3::Y * (sin * radius),
                tail_side * cos + Vec3::Y * sin,
                [naked[0] * shade, naked[1] * shade, naked[2] * shade, 1.0],
            );
        }
    }
    stitch_tube(mesh.indices, tail_first, TAIL_STATIONS, TAIL_SECTORS);

    // Thick round pinnae. An inset inner ring slopes into the concha; separate
    // back vertices retain the outer skin and a crisp but rounded rolled rim.
    for sign in [1.0_f32, -1.0] {
        let ear_center = station(0.065, 0.077) + side * (sign * 0.024 * scale);
        let ear_angle = sign * (0.6435 + pose.ears[usize::from(sign < 0.0)]);
        let normal = ahead * ear_angle.cos() + side * ear_angle.sin();
        let across = side * ear_angle.cos() - ahead * ear_angle.sin();
        let first = mesh.positions.len() as u32;
        for layer in 0..3 {
            let (size, depth, color) = match layer {
                0 => (1.0, 0.0, naked),
                1 => (
                    0.66,
                    -0.0032,
                    [0.145 * tint, 0.068 * tint, 0.055 * tint, 1.0],
                ),
                _ => (1.0, -0.0018, body_color(0.4, 0.067)),
            };
            for sector in 0..8 {
                let angle = sector as f32 * TAU / 8.0;
                let (sin, cos) = angle.sin_cos();
                let radial = across * cos + Vec3::Y * sin;
                let p = ear_center
                    + across * (cos * 0.0115 * size * scale)
                    + Vec3::Y * (sin * 0.017 * size * scale)
                    + normal * (depth * scale);
                let n = if layer == 2 {
                    -normal + radial * 0.7
                } else {
                    normal + radial * 0.5
                };
                mesh.vertex(p, n, color);
            }
        }
        let inner = mesh.vertex(
            ear_center - normal * (0.0045 * scale),
            normal,
            [0.115 * tint, 0.048 * tint, 0.039 * tint, 1.0],
        );
        let back = mesh.vertex(
            ear_center - normal * (0.005 * scale),
            -normal,
            body_color(0.3, 0.067),
        );
        for sector in 0..8_u32 {
            let next = (sector + 1) % 8;
            mesh.triangle(inner, first + 8 + next, first + 8 + sector);
            mesh.triangle(first + sector, first + 8 + next, first + next);
            mesh.triangle(first + sector, first + 8 + sector, first + 8 + next);
            mesh.triangle(back, first + 16 + sector, first + 16 + next);
            mesh.triangle(first + sector, first + next, first + 16 + next);
            mesh.triangle(first + sector, first + 16 + next, first + 16 + sector);
        }
    }

    for sign in [1.0_f32, -1.0] {
        let bead = station(0.102, 0.057) + side * (sign * 0.0232 * scale);
        mesh.ellipsoid(bead, axes, Vec3::new(0.0046, 0.0022, 0.0042) * scale, eye);
    }
    mesh.ellipsoid(
        station(0.148, 0.043),
        axes,
        Vec3::new(0.0032, 0.0043, 0.0032) * scale,
        [0.13 * tint, 0.067 * tint, 0.056 * tint, 1.0],
    );

    // Feet are planted in world space during stance. Paired face washing uses
    // a low rump-supported crouch, rather than the cage reference's support.
    for (leg_index, (along, flank, hip_height)) in RAT_LEGS.into_iter().enumerate() {
        let hip = station(along, hip_height)
            + side * (flank * scale)
            + ahead * (pose.groom * 0.008 * scale * if leg_index < 2 { 1.0 } else { 0.0 });
        let local_foot = pose.feet[leg_index];
        let foot = center + ahead * local_foot.x + side * local_foot.z + Vec3::Y * local_foot.y;
        let rear = leg_index >= 2;
        let paw_angle = pose.paw_curl[leg_index];
        let finger = ahead * paw_angle.cos() + Vec3::Y * paw_angle.sin();
        let palm_up = Vec3::Y * paw_angle.cos() - ahead * paw_angle.sin();
        let paw_axes = [finger, side, palm_up];
        if rear {
            // The upper half is buried in the rump. Its long axis leans from
            // the rear hip down toward the forward knee; denser rings round
            // the exposed contour without lowering it into a hanging lobe.
            let thigh_axes = [
                ahead * 0.9 + Vec3::Y * 0.4358899,
                side,
                Vec3::Y * 0.9 - ahead * 0.4358899,
            ];
            mesh.haunch(
                hip + ahead * (0.006 * scale) - Vec3::Y * (0.001 * scale),
                thigh_axes,
                Vec3::new(0.026, 0.018, 0.026) * scale,
                body_color(-0.1, along),
            );
        }
        let wrist = foot + palm_up * (0.007 * scale) - finger * (0.002 * scale);
        let elbow = hip.lerp(wrist, 0.53) - ahead * (if rear { -0.006 } else { 0.005 } * scale);
        let limb_first = mesh.positions.len() as u32;
        let limb_down = (wrist - hip).normalize_or(-Vec3::Y);
        let limb_side = (side - limb_down * side.dot(limb_down)).normalize_or(side);
        let limb_round = limb_down.cross(limb_side);
        // Rings advance down the limb; side/ahead ordering keeps its skin out.
        for (point, radius, color) in [
            (
                hip,
                if rear { 0.013 } else { 0.009 },
                body_color(-0.2, along),
            ),
            (
                elbow,
                if rear { 0.007 } else { 0.005 },
                body_color(-0.6, along),
            ),
            (wrist, 0.0032, feet),
        ] {
            for sector in 0..6 {
                let (sin, cos) = rat_circle(sector, 6);
                let radial = limb_side * cos + limb_round * sin;
                mesh.vertex(point + radial * (radius * scale), radial, color);
            }
        }
        stitch_tube(mesh.indices, limb_first, 3, 6);
        let paw = foot + finger * (0.004 * scale) + palm_up * (0.003 * scale);
        mesh.ellipsoid(
            paw,
            paw_axes,
            Vec3::new(if rear { 0.010 } else { 0.008 }, 0.0065, 0.003) * scale,
            feet,
        );
        for toe in -1..=1 {
            let root = foot + finger * (0.009 * scale) + side * (toe as f32 * 0.0034 * scale);
            let tip = root
                + finger * ((if toe == 0 { 0.011 } else { 0.0085 }) * scale)
                + side * (toe as f32 * 0.001 * scale)
                + palm_up * (0.00035 * scale);
            let first = mesh.positions.len() as u32;
            // Three-sided toes taper to a small blunt end, not a single spike.
            let points = [
                tip + side * (0.0008 * scale),
                tip + palm_up * (0.0018 * scale),
                tip - side * (0.0008 * scale),
                root + side * (0.0015 * scale),
                root + palm_up * (0.0036 * scale),
                root - side * (0.0015 * scale),
            ];
            let toe_center = points.iter().copied().sum::<Vec3>() / 6.0;
            for p in points {
                mesh.vertex(p, p - toe_center, feet);
            }
            stitch_tube(mesh.indices, first, 2, 3);
            mesh.triangle(first, first + 2, first + 1);
            mesh.triangle(first + 3, first + 4, first + 5);
        }
    }

    // Three fine whiskers per cheek. Their span stays inside the broad rump,
    // so neither the silhouette nor the batch bounds become a wire brush.
    for sign in [1.0_f32, -1.0] {
        for row in 0..3 {
            let root = station(0.135 - row as f32 * 0.004, 0.043) + side * (sign * 0.015 * scale);
            let tip = root
                + side * (sign * (0.020 + row as f32 * 0.003) * scale)
                + ahead * ((0.013 - row as f32 * 0.013 + pose.nose * 0.007) * scale)
                + Vec3::Y * ((row as f32 - 1.0) * 0.003 * scale);
            let thickness = Vec3::Y * (0.00018 * scale);
            push_quad(
                mesh.positions,
                mesh.normals,
                mesh.uvs,
                mesh.colors,
                mesh.indices,
                [
                    root - thickness,
                    tip - thickness * 0.2,
                    tip + thickness * 0.2,
                    root + thickness,
                ],
                [0.075, 0.060, 0.049, 1.0],
            );
        }
    }
    debug_assert_eq!(mesh.positions.len() - first_vertex, RAT_VERTICES);
}

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

#[allow(clippy::too_many_arguments)]
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

    fn motion_pose(rat: &Rat, heading: Vec2, moving: bool, elapsed: f32) -> RatPose {
        let mut motion = RatMotion::default();
        let mut pose = motion.update(
            rat.seed,
            rat.length_m,
            Vec2::ZERO,
            heading,
            0.0,
            f32::INFINITY,
        );
        let frames = (elapsed * 60.0).ceil() as usize;
        for frame in 1..=frames {
            let t = (frame as f32 / 60.0).min(elapsed);
            let position = heading * if moving { t * 1.8 } else { 0.0 };
            pose = motion.update(
                rat.seed,
                rat.length_m,
                position,
                heading,
                t,
                if moving { 0.0 } else { f32::INFINITY },
            );
        }
        pose
    }

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
            commands
                .try_iter()
                .enumerate()
                .map(|(i, command)| {
                    crate::smart_actors::bridge::expect_host_command(command, i as u64 + 1)
                })
                .collect()
        };

        // Density 0 empties every colony but keeps all the authored records.
        let (sender, commands) = bounded(8);
        let mut empty = built_app_with(Some(VerminSettings {
            density: 0.0,
            ..Default::default()
        }));
        empty.insert_resource(BridgeHandle::new(sender, std::path::PathBuf::from("/tmp")));
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
        app.insert_resource(BridgeHandle::new(sender, std::path::PathBuf::from("/tmp")));
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
    fn rat_motion_follows_distance_and_retains_stance_contacts() {
        let run = |hz: usize| {
            let mut motion = RatMotion::default();
            let mut old = motion.update(7, 0.28, Vec2::ZERO, Vec2::X, 0.0, 0.0);
            let mut old_position = Vec2::ZERO;
            let mut planted = 0;
            for frame in 1..=hz {
                let t = frame as f32 / hz as f32;
                let position = Vec2::X * t * 0.72;
                let pose = motion.update(7, 0.28, position, Vec2::Y, t, 0.0);
                for foot in 0..4 {
                    if t > 0.25 && old.feet[foot].y == 0.0 && pose.feet[foot].y == 0.0 {
                        let previous_world =
                            old_position + Vec2::new(old.feet[foot].x, old.feet[foot].z);
                        let current_world =
                            position + Vec2::new(pose.feet[foot].x, pose.feet[foot].z);
                        assert!(
                            previous_world.distance(current_world) < 0.00001,
                            "stance slid: {foot}"
                        );
                        planted += 1;
                    }
                }
                old = pose;
                old_position = position;
            }
            assert!(
                planted > hz,
                "travel must have measurable supporting contacts"
            );
            old
        };
        let a = run(60);
        let b = run(120);
        assert!(rat_angle(a.heading, Vec2::X).abs() < 0.001);
        assert!(
            (a.phase - b.phase)
                .abs()
                .min(1.0 - (a.phase - b.phase).abs())
                < 0.0001
        );
        assert!((a.speed - 0.72).abs() < 0.0001);
    }

    #[test]
    fn rat_zero_dt_cull_resume_and_groom_interruption_are_safe() {
        let mut motion = RatMotion::default();
        let mut previous = motion.update(7, 0.28, Vec2::ZERO, Vec2::X, 0.0, 10.0);
        let mut groom_seen = false;
        for frame in 1..=2400 {
            let t = frame as f32 / 60.0;
            let pose = motion.update(7, 0.28, Vec2::ZERO, Vec2::Y, t, 10.0);
            assert_eq!(
                motion.update(7, 0.28, Vec2::ONE, -Vec2::X, t, 0.0),
                pose,
                "paused frames must be inert"
            );
            assert_eq!(pose.phase, 0.0, "idle feet must not treadmill");
            assert_eq!(
                pose.heading,
                Vec2::X,
                "outgoing route hint must not spin a stationary rat"
            );
            if pose.groom > 0.7 && pose.feet.iter().any(|foot| foot.y > 0.025) {
                assert!(
                    pose.feet[2].y < 0.0001 && pose.feet[3].y < 0.0001,
                    "both hind paws support the seated wash"
                );
                let next = motion.update(7, 0.28, Vec2::X * 0.006, Vec2::X, t + 1.0 / 60.0, 0.0);
                assert!(next.groom < pose.groom && next.groom > 0.0);
                for foot in 0..2 {
                    assert!(
                        (next.feet[foot].y - pose.feet[foot].y).abs() < 0.04,
                        "wash must settle into escape"
                    );
                }
                groom_seen = true;
                break;
            }
            previous = pose;
        }
        assert!(
            groom_seen,
            "ordinary deterministic quiet time must include an actual paw-to-face wash"
        );
        let reset = motion.update(7, 0.28, Vec2::splat(200.0), Vec2::Y, 100.0, 4.0);
        assert_eq!(reset.speed, 0.0);
        assert!(reset.feet.iter().all(|p| p.is_finite() && p.y == 0.0));
        assert_ne!(previous.heading, reset.heading);
    }

    #[test]
    fn rat_actions_deform_the_actual_mesh_with_ground_support() {
        let rat = Rat {
            seed: 7,
            legs: Vec::new(),
            period: 10.0,
            phase: 0.0,
            length_m: 0.28,
            tint: 1.0,
            motion: RatMotion::default(),
        };
        let (mut p, mut n, mut u, mut c, mut i) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut seen = [false; 3];
        let mut contact_low = f32::INFINITY;
        let mut contact_high = f32::NEG_INFINITY;
        let mut contact_count = 0;
        for length in [0.24, 0.28, 0.32] {
            let mut motion = RatMotion::default();
            for frame in 0..1800 {
                let t = frame as f32 / 60.0;
                let pose = motion.update(rat.seed, length, Vec2::ZERO, Vec2::X, t, 4.0);
                assert!(
                    [
                        pose.travel,
                        pose.turn,
                        pose.sniff,
                        pose.alert,
                        pose.groom,
                        pose.head_bow,
                        pose.head_yaw,
                        pose.tail_bend,
                        pose.tail_tip,
                        pose.nose
                    ]
                    .into_iter()
                    .chain(pose.paw_curl)
                    .chain(pose.ears)
                    .chain(pose.feet.into_iter().flat_map(|foot| foot.to_array()))
                    .all(|value| value == 0.0 || value.is_normal()),
                    "quiet easing residues must not feed subnormals into geometry: {pose:?}"
                );
                seen[0] |= pose.sniff > 0.8;
                seen[1] |= pose.alert > 0.8;
                seen[2] |= pose.groom > 0.8;
                p.clear();
                n.clear();
                u.clear();
                c.clear();
                i.clear();
                let sized = Rat {
                    length_m: length,
                    ..Rat {
                        seed: rat.seed,
                        legs: Vec::new(),
                        period: 10.0,
                        phase: 0.0,
                        length_m: length,
                        tint: 1.0,
                        motion: RatMotion::default(),
                    }
                };
                push_rat(
                    &mut p,
                    &mut n,
                    &mut u,
                    &mut c,
                    &mut i,
                    &sized,
                    Vec2::ZERO,
                    &pose,
                );
                assert_eq!(p.len(), RAT_VERTICES);
                assert_eq!(i.len() / 3, 996);
                if pose.feet[0].y.min(pose.feet[1].y) > 0.02 * (length / 0.28) {
                    let rump_floor = p[10 * BODY_SECTORS..13 * BODY_SECTORS]
                        .iter()
                        .map(|p| p[1])
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        rump_floor < RAT_GROUND_Y + 0.0025 * (length / 0.28),
                        "paired wash must sit on its rump: {rump_floor}"
                    );
                    assert!(
                        pose.feet[2].y.max(pose.feet[3].y) < 0.0001,
                        "paired wash needs both hind contacts"
                    );
                }
                if pose.groom > 0.8 {
                    // The middle toe's three tip vertices, on each actual front paw.
                    for first in [366, 416] {
                        let fingertip = p[first..first + 3]
                            .iter()
                            .copied()
                            .map(Vec3::from_array)
                            .sum::<Vec3>()
                            / 3.0;
                        let gap = p[..7 * BODY_SECTORS]
                            .iter()
                            .copied()
                            .map(Vec3::from_array)
                            .map(|point| point.distance(fingertip))
                            .fold(f32::INFINITY, f32::min);
                        if gap < 0.011 * (length / 0.28) {
                            let height = (fingertip.y - RAT_GROUND_Y) / (length / 0.28);
                            contact_low = contact_low.min(height);
                            contact_high = contact_high.max(height);
                            contact_count += 1;
                        }
                    }
                }
                let floor = p.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                assert!(
                    (0.012..0.014).contains(&floor),
                    "action digs through road: {floor} {pose:?}"
                );
                assert!(p.iter().flatten().all(|v| v.is_finite()));
                for tri in i.chunks_exact(3) {
                    let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
                    let face = (Vec3::from_array(p[b]) - Vec3::from_array(p[a]))
                        .cross(Vec3::from_array(p[c]) - Vec3::from_array(p[a]));
                    let normal =
                        Vec3::from_array(n[a]) + Vec3::from_array(n[b]) + Vec3::from_array(n[c]);
                    assert!(
                        face.length() > 1e-11 && face.normalize().dot(normal.normalize()) > 0.025,
                        "invalid action triangle {a},{b},{c}, frame={frame}, pose={pose:?}"
                    );
                }
            }
        }
        assert!(
            contact_count > 20 && contact_high - contact_low > 0.020,
            "washing fingertips must traverse the actual mouth-to-brow surface: {contact_count} contacts over {} m",
            contact_high - contact_low
        );
        assert!(
            seen.into_iter().all(|seen| seen),
            "all three short actions must appear"
        );
    }

    #[test]
    fn rat_tail_vertices_remain_continuous_through_stride_wraps_and_stops() {
        let rat = Rat {
            seed: 37,
            legs: Vec::new(),
            period: 10.0,
            phase: 0.0,
            length_m: 0.28,
            tint: 1.0,
            motion: RatMotion::default(),
        };
        let mut motion = RatMotion::default();
        let (mut p, mut n, mut u, mut c, mut i) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut previous = Vec::new();
        for frame in 0..180 {
            let t = frame as f32 / 120.0;
            let position = Vec2::X * t.min(0.83) * 1.8;
            let pose = motion.update(
                rat.seed,
                rat.length_m,
                position,
                Vec2::X,
                t,
                if t < 0.83 { 0.0 } else { 0.3 },
            );
            p.clear();
            n.clear();
            u.clear();
            c.clear();
            i.clear();
            push_rat(
                &mut p,
                &mut n,
                &mut u,
                &mut c,
                &mut i,
                &rat,
                Vec2::ZERO,
                &pose,
            );
            let tail = &p[BODY_STATIONS * BODY_SECTORS
                ..BODY_STATIONS * BODY_SECTORS + TAIL_STATIONS * TAIL_SECTORS];
            if !previous.is_empty() {
                for (before, after) in previous.iter().zip(tail) {
                    assert!(
                        Vec3::from_array(*before).distance(Vec3::from_array(*after)) < 0.006,
                        "tail vertex jumped across stride/stop at {t}"
                    );
                }
            }
            previous.clear();
            previous.extend_from_slice(tail);
        }
    }

    /// Optional capture input is produced by the actual route and scatter functions.
    /// With no env setting this is a normal assertion-only regression test.
    #[test]
    fn rat_actual_route_and_stationary_scatter_trace() {
        use std::fmt::Write as _;
        let nav = committed_nav();
        let collision = CollisionWorld::default();
        let anchor = Vec2::new(-294.0, 220.0);
        let destination = std::env::var_os("CATHEDRAL_RAT_TRACE_DIR").map(std::path::PathBuf::from);
        for case in 0..4 {
            let interrupt_groom = case == 3;
            let stationary = case == 1 || interrupt_groom;
            let combined = case == 2;
            let mut rat = bake_rat(
                &nav,
                &collision,
                anchor,
                14.0,
                if interrupt_groom { 37 } else { 7 },
            )
            .unwrap();
            rat.phase = if combined {
                (rat.legs[0].depart - 0.6).max(0.0)
            } else {
                0.0
            };
            let home = rat.sample(0.0).0;
            let attack_start = if interrupt_groom { 1.0 } else { 0.25 };
            if stationary {
                rat.legs = vec![Leg {
                    depart: 10.0,
                    arrive: 10.02,
                    from: home,
                    to: home,
                    heading: Vec2::X,
                }];
                rat.period = 10.02;
            }
            let duration = if stationary || combined {
                if interrupt_groom { 4.5 } else { 4.0 }
            } else {
                rat.period
            };
            let mut motion = RatMotion::default();
            let mut csv = destination.as_ref().map(|_| {
                String::from("time_seconds,x_m,z_m,heading_x,heading_z,pause_remaining_seconds\n")
            });
            let mut hold_phase = None;
            let mut return_seen = false;
            let mut combined_hold_travel = false;
            let mut last_position = rat.sample(0.0).0;
            let mut max_speed = 0.0_f32;
            let mut paired_before_escape = false;
            let mut leaving_crouch_seen = false;
            let (mut p, mut n, mut u, mut c, mut i) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for frame in 0..=(duration * 60.0).ceil() as usize {
                let t = frame as f32 / 60.0;
                let (base, hint, _) = rat.sample(t);
                let impulse =
                    (stationary || combined).then_some((home + Vec2::new(0.4, 0.0), attack_start));
                let offset = scatter_offset(&nav, &collision, &rat, base, impulse, t);
                let position = base + offset;
                assert!(walkable(&nav, &collision, position));
                let remaining = rat.pause_remaining(t);
                let pose = motion.update(rat.seed, rat.length_m, position, hint, t, remaining);
                p.clear();
                n.clear();
                u.clear();
                c.clear();
                i.clear();
                push_rat(
                    &mut p,
                    &mut n,
                    &mut u,
                    &mut c,
                    &mut i,
                    &rat,
                    Vec2::ZERO,
                    &pose,
                );
                assert!(p.iter().flatten().all(|v| v.is_finite()));
                let floor = p.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                assert!(
                    (0.012..0.014).contains(&floor),
                    "escape geometry loses the road at {t}: {floor}"
                );
                assert_eq!(i.len() / 3, 996);
                for triangle in i.chunks_exact(3) {
                    let [a, b, c] = [
                        triangle[0] as usize,
                        triangle[1] as usize,
                        triangle[2] as usize,
                    ];
                    let face = (Vec3::from_array(p[b]) - Vec3::from_array(p[a]))
                        .cross(Vec3::from_array(p[c]) - Vec3::from_array(p[a]));
                    let normal =
                        Vec3::from_array(n[a]) + Vec3::from_array(n[b]) + Vec3::from_array(n[c]);
                    assert!(
                        face.length() > 1e-11 && face.normalize().dot(normal.normalize()) > 0.025,
                        "invalid escape triangle {a},{b},{c} at {t}"
                    );
                }
                paired_before_escape |= interrupt_groom
                    && t < attack_start
                    && pose.feet[0].y.min(pose.feet[1].y) > 0.020 * (rat.length_m / 0.28);
                leaving_crouch_seen |= interrupt_groom
                    && t > attack_start
                    && t < attack_start + 0.3
                    && pose.groom > 0.0
                    && pose.speed > 0.2;
                max_speed = max_speed.max(pose.speed);
                for (index, foot) in pose.feet.iter().enumerate() {
                    let (along, flank, _) = RAT_LEGS[index];
                    let neutral =
                        Vec2::new(along, flank + flank.signum() * 0.007) * (rat.length_m / 0.28);
                    assert!(
                        Vec2::new(foot.x, foot.z).distance(neutral) < 0.080 * (rat.length_m / 0.28),
                        "actual escape/turn overextends foot {index} at {t}: {foot:?}"
                    );
                }
                if combined && (0.85..1.4).contains(&t) && pose.speed > 0.5 {
                    let delta = position - last_position;
                    assert!((pose.speed - delta.length() * 60.0).abs() < 0.003);
                    combined_hold_travel = true;
                }
                last_position = position;
                if stationary && (attack_start + 0.55..attack_start + 1.35).contains(&t) {
                    assert!(pose.speed < 0.001, "scatter hold has no translation");
                    if let Some(phase) = hold_phase {
                        assert_eq!(pose.phase, phase, "scatter hold cannot treadmill");
                    }
                    hold_phase = Some(pose.phase);
                    if pose.groom < 0.001 {
                        assert!(
                            pose.feet.iter().all(|p| p.y < 0.001),
                            "settled non-washing paws stay grounded"
                        );
                    }
                }
                if stationary
                    && (attack_start + 1.75..attack_start + 2.75).contains(&t)
                    && pose.speed > 0.03
                {
                    assert!(
                        pose.heading.dot((-offset).normalize()) > 0.95,
                        "return must face home"
                    );
                    return_seen = true;
                }
                if let Some(csv) = &mut csv {
                    writeln!(
                        csv,
                        "{t:.8},{:.8},{:.8},{:.8},{:.8},{remaining:.8}",
                        position.x, position.y, hint.x, hint.y
                    )
                    .unwrap();
                }
            }
            if stationary {
                assert!(return_seen && hold_phase.is_some());
                assert!(
                    max_speed > 3.0,
                    "regression must include actual escape speed"
                );
            }
            if interrupt_groom {
                assert!(
                    paired_before_escape && leaving_crouch_seen,
                    "trace must interrupt a real paired crouch"
                );
            }
            if combined {
                assert!(
                    combined_hold_travel,
                    "scatter hold still moves when the baked base moves"
                );
            }
            if let (Some(directory), Some(csv)) = (&destination, csv) {
                std::fs::create_dir_all(directory).unwrap();
                let name = if interrupt_groom {
                    "groom_interrupt_scatter"
                } else if stationary {
                    "stationary_scatter"
                } else if combined {
                    "moving_base_scatter"
                } else {
                    "baked_route"
                };
                std::fs::write(directory.join(format!("{name}.csv")), csv).unwrap();
                let metadata = serde_json::json!({ "seed":rat.seed, "length_m":rat.length_m, "tint":rat.tint,
                    "phase":rat.phase, "period":rat.period, "sample_hz":60, "duration_seconds":duration, "scatter_start_seconds":if stationary || combined {Some(attack_start)} else {None},
                    "navigation":"assets/world/navigation.json + assets/world/navigation.bin (committed bake)",
                    "limitations":"Actual Rat.sample and scatter_offset with committed NavData, default empty test CollisionWorld; does not include full runtime colliders. Stationary trace replaces route with one legal degenerate home leg. The paired-groom interrupt uses seed 37 and a 1.0-second impulse; the other scatter traces start the actual impulse at 0.25 seconds; moving_base_scatter shifts the baked phase to depart at 0.6 seconds. Route trace has no scatter." });
                std::fs::write(
                    directory.join(format!("{name}.json")),
                    serde_json::to_vec_pretty(&metadata).unwrap(),
                )
                .unwrap();
            }
        }
    }

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
                &motion_pose(&rat, heading, false, 0.0),
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
                &motion_pose(&rat, Vec2::X, moving, elapsed),
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

    /// The road is at 12 mm: comparing only to RAT_GROUND_Y would permit
    /// an accidentally elevated origin to make every rat float together.
    #[test]
    fn rat_geometry_has_a_bounded_budget_and_clears_the_actual_road() {
        let mut rat = Rat {
            seed: 37,
            legs: Vec::new(),
            period: 10.0,
            phase: 0.0,
            length_m: 0.28,
            tint: 1.0,
            motion: RatMotion::default(),
        };
        let (mut p, mut n, mut u, mut c, mut i) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for length in [0.24, 0.28, 0.32] {
            rat.length_m = length;
            for moving in [false, true] {
                for frame in 0..24 {
                    p.clear();
                    n.clear();
                    u.clear();
                    c.clear();
                    i.clear();
                    push_rat(
                        &mut p,
                        &mut n,
                        &mut u,
                        &mut c,
                        &mut i,
                        &rat,
                        Vec2::ZERO,
                        &motion_pose(&rat, Vec2::X, moving, frame as f32 / 24.0),
                    );
                    assert!(i.len() / 3 <= 1000, "one rat exceeds its geometry budget");
                    assert!(p.iter().flatten().all(|v| v.is_finite()));
                    let floor = p.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
                    assert!(
                        (0.012..=0.014).contains(&floor),
                        "rat feet must meet the road: {floor}"
                    );
                    let belly = p[..10 * BODY_SECTORS]
                        .iter()
                        .map(|v| v[1])
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        belly >= 0.017,
                        "the belly ahead of the seated rump must clear the road: {belly}"
                    );
                    for triangle in i.chunks_exact(3) {
                        let [a, b, d] = [
                            triangle[0] as usize,
                            triangle[1] as usize,
                            triangle[2] as usize,
                        ];
                        let face = (Vec3::from_array(p[b]) - Vec3::from_array(p[a]))
                            .cross(Vec3::from_array(p[d]) - Vec3::from_array(p[a]));
                        let shading = Vec3::from_array(n[a])
                            + Vec3::from_array(n[b])
                            + Vec3::from_array(n[d]);
                        assert!(
                            face.length() > 1e-11
                                && face.normalize().dot(shading.normalize()) > 0.05,
                            "inverted or collapsed triangle {a},{b},{d}; moving={moving}, frame={frame}"
                        );
                    }
                }
            }
        }
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
