//! The daily round (M4) — the non-LLM behaviour layer that gets the city up in
//! the morning, sends it to work, moves the crowd on market days, and empties the
//! streets at the Snuffing (`features/movement/04_the_round.md`,
//! `features/movement/03_the_ladder.md`). It subsumes the M3 water round: water
//! is now two rungs (2 & 6) of one flat, first-match-wins ladder.
//!
//! Every LLM townsperson is enrolled with a **home** (baked into
//! `assets/world/homes.json`), a **workplace** (bound from `assets/world/rounds.json`'s
//! occupation → nav-place map), and a **round** — a short list of office-pegged
//! legs, taken from the 19 authored route overrides for the majors that resolve to
//! a 5-char id and from the 65 occupation templates for everyone else. The ladder
//! each decision epoch is:
//!
//! | rung | fires when | goes to |
//! |------|------------|---------|
//! | 5 curfew  | night (Snuffing/Watch), housed, not a night trade | home, to sleep |
//! | 2 parched | `thirst < THIRST_PARCHED`, a water source bound   | the well now |
//! | 6 thirsty | `thirst < THIRST_THIRSTY`, queue short            | the well |
//! | 9 round   | the current office's leg says "be at X", I am not | X |
//! | 11 social | a known, settled neighbour is near               | drift toward them |
//! | 12 wander | — (a gated hash roll)                            | mill near home |
//!
//! Everything is embedded (`include_str!`) so both hosts get it with no wiring,
//! and the whole layer is **inert without a nav graph** — the frozen golden
//! fixtures pass `nav: None`, so nobody is enrolled and nothing moves. It is
//! pure and deterministic: every "random" choice is a hash of
//! `(salt, actor_id, epoch)`, never an RNG (the `attention.rs` curiosity idiom).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Deserialize;

use crate::{
    HEARING_RADIUS_M, LADDER_DECISION_MAX_SECONDS, LADDER_DECISION_MIN_SECONDS,
    PERSON_ARRIVE_RADIUS_M, PLACE_ARRIVE_RADIUS_M, SOCIAL_PULL_RADIUS_M, THIRST_MAX,
    THIRST_PARCHED, THIRST_THIRSTY, WALK_SPEED_MPS, WATER_DRAW_SECONDS,
    WELL_ARRIVE_RADIUS_M, WELL_KEEPER_SOUND_INTERVAL_SECONDS, WELL_QUEUE_SHORT,
    character::{Character, IntentTarget, Movement},
    clock::{Office, WorldClock, Weekday},
    event::DomainEvent,
    homes::{HOMES_JSON, HomesDoc},
    ids::{ActorId, PlaceId},
    lore::Significance,
    math::Vec3,
    nav::{NavData, WALK_Y},
    perception::identify_ids,
    places::PlaceRegistry,
    world::{World, hash01, lane_fraction, planar_close},
};

/// Occupations that fetch water with a **household** vessel — chiefly the
/// servant, whose day is a trip to the ward well and back (the single largest
/// occupation in the city, `features/movement/README.md` §8). A household vessel
/// takes precedence in the queue (`lore/wells_and_water.md`).
const HOUSEHOLD_OCCUPATIONS: &[&str] = &["domestic_servant"];
/// Occupations that draw with a **trade** vessel and so queue *behind* the
/// households — the fullers, dyers and cloth-washers the cisterns were assessed
/// for (`lore/wells_and_water.md`: Tenter Cistern's "fullers, cloth washers,
/// dyers").
const TRADE_OCCUPATIONS: &[&str] = &["cloth_worker", "garment_worker"];

/// Whether an occupation fetches water, and with what vessel: `Some(true)` is a
/// household vessel, `Some(false)` a trade one, `None` is not a drawer.
fn vessel_of(occupation: Option<&str>) -> Option<bool> {
    match occupation {
        Some(occupation) if HOUSEHOLD_OCCUPATIONS.contains(&occupation) => Some(true),
        Some(occupation) if TRADE_OCCUPATIONS.contains(&occupation) => Some(false),
        _ => None,
    }
}
/// A source only takes a keeper from an NPC standing within this of its curb, so
/// nobody is dragged across the city to work a well.
const KEEPER_MAX_DIST_M: f64 = 55.0;
/// A keeper's wander leash: a stride or two off the curb, never out of reach of
/// the gear — well inside [`CENSUS_POST_RADIUS_M`], so a milling keeper still
/// censuses as at their post.
const KEEPER_LEASH_M: f64 = 4.0;
/// A mover is "at" a round leg's anchor (a square, a workshop, the moorings) once
/// within this of it. Looser than [`WELL_ARRIVE_RADIUS_M`]: a place's node is one
/// point in a wide site, not a curb.
const ROUND_ARRIVE_RADIUS_M: f64 = 6.0;
/// A follow re-lays its path once the target has drifted this far from where
/// the standing path was aimed — "re-pathed every movement tick" without
/// re-running A* for a target who is standing still.
const FOLLOW_REPATH_EPSILON_M: f64 = 1.5;
/// A mover is "at home" once within this of their door — tight, so a sleeper
/// stands on their own step rather than in the street outside a neighbour's.
const HOME_ARRIVE_RADIUS_M: f64 = 3.0;
/// Leg leash when a route/template does not name one.
const DEFAULT_ROUND_LEASH_M: f64 = 10.0;
/// Census: how close counts as "at your post" / "at home".
const CENSUS_POST_RADIUS_M: f64 = 9.0;
const CENSUS_HOME_RADIUS_M: f64 = 5.0;

/// The nine public drinking sources (`07_milestones.md` M3), by nav-graph display
/// name, each paired with its gear's sound. See the M3 notes: the Shambles work
/// well and the Seven Lofts fire tanks are deliberately excluded.
const SOURCES: &[(&str, &str)] = &[
    ("Ford Well", "draw_water"),
    ("Chain Well", "chain_windlass"),
    ("Bitter Well", "draw_water"),
    ("Three-Curb", "draw_water"),
    ("Lodge Well", "draw_water"),
    ("Slate Cistern", "pour_trough"),
    ("Tenter Cistern", "pour_trough"),
    ("Reed Cistern", "pour_trough"),
    ("Step Cistern", "pour_trough"),
];

/// The authored round content, embedded so both hosts get it with no wiring —
/// exactly as `bake_navigation.py`'s output is embedded in the game host. Static
/// data, compiled in, not read at runtime: the sim keeps its no-IO decree.
const ROUNDS_JSON: &str = include_str!("../../../assets/world/rounds.json");

/// The five town squares whose lamps the lamplighter's dusk round lights
/// (M7; `features/50_cool_suggestions.md` #21). The round itself visits them
/// nearest-next from wherever he stands, so this order only seeds the list.
const LAMP_SQUARES: &[&str] = &[
    "The Wickmarket",
    "Coswald's Yard",
    "The Tallage",
    "Maren's Green",
    "The Gradine",
];
/// Posts per square, rung around the square's interior node.
const LAMPS_PER_SQUARE: usize = 4;
/// The ring the posts stand on, probed inward until each lands on pavement.
const LAMP_RING_RADIUS_M: f64 = 11.0;
/// Close enough to reach the wick with the taper.
const LAMP_LIGHT_RADIUS_M: f64 = 2.5;

/// What an actor means to do on arrival — the pathfind-then-act bridge lifted
/// from seagame's `targetState` (`features/movement/02_navigation.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrival {
    Work,
    Trade,
    Sleep,
    Pray,
    Idle,
    DrawWater,
    Stand,
}

// --------------------------------------------------------------------------- //
// The embedded content, straight off the JSON
// --------------------------------------------------------------------------- //
#[derive(Debug, Deserialize)]
struct RoundsDoc {
    workplaces: HashMap<String, Vec<String>>,
    archetypes: HashMap<String, TemplateSpec>,
    occupations: HashMap<String, String>,
    routes: HashMap<String, RouteSpec>,
    /// The lamplighters' beats (M7): 5-char actor id → the squares whose lamps
    /// that keeper lights at dusk. Beats, not one citywide round, because the
    /// street graph is a fortified maze — adjacent squares are 1.3–2 km apart
    /// on foot, and a single walker cannot cover five of them in one night. A
    /// square no authored keeper claims (or whose keeper is missing from the
    /// cast) simply stays dark.
    #[serde(default)]
    lamp_keepers: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TemplateSpec {
    leash_m: f64,
    #[serde(default)]
    curfew_exempt: bool,
    legs: Vec<LegSpec>,
}

#[derive(Debug, Deserialize)]
struct RouteSpec {
    #[serde(default)]
    leash_m: Option<f64>,
    #[serde(default)]
    curfew_exempt: Option<bool>,
    legs: Vec<LegSpec>,
}

#[derive(Debug, Deserialize)]
struct LegSpec {
    from: Office,
    at: String,
    doing: Arrival,
    #[serde(default)]
    only_on: Option<Vec<Weekday>>,
}

/// Resolves a leg's `at` string to a world point: a nav place display name, a
/// nav site id or name, or the keywords `home` / `workplace` (handled by the
/// caller). Built once per seed.
struct PlaceResolver<'a> {
    nav: &'a NavData,
    site_by_key: HashMap<String, usize>,
}

impl<'a> PlaceResolver<'a> {
    fn new(nav: &'a NavData) -> Self {
        let mut site_by_key = HashMap::new();
        for site in nav.sites() {
            site_by_key.insert(site.name.clone(), site.node);
            site_by_key.insert(site.id.clone(), site.node);
        }
        Self { nav, site_by_key }
    }

    /// A named place/site to a walkable point, or `None` if it is not in the graph.
    fn resolve(&self, name: &str) -> Option<Vec3> {
        if let Some(place) = self.nav.place(name) {
            return Some(self.nav.node_point(place.node));
        }
        self.site_by_key.get(name).map(|&node| self.nav.node_point(node))
    }

    /// [`resolve`], but preferring the open site's interior node over the street
    /// place — the point nearest the square's centre, which is where a lamp
    /// ring should stand.
    ///
    /// [`resolve`]: PlaceResolver::resolve
    fn resolve_centre(&self, name: &str) -> Option<Vec3> {
        if let Some(&node) = self.site_by_key.get(name) {
            return Some(self.nav.node_point(node));
        }
        self.resolve(name)
    }
}

// --------------------------------------------------------------------------- //
// The public model
// --------------------------------------------------------------------------- //
/// One public well or cistern: where its water is drawn, what it sounds like, who
/// keeps it, and the queue waiting at the curb.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterSource {
    /// The nav place display name, e.g. `"Chain Well"`.
    pub name: String,
    /// The walkable node the queue forms at and the sound comes from.
    pub draw_point: Vec3,
    /// The catalog sound the keeper makes working the gear.
    pub draw_sound: &'static str,
    /// The actor who works the curb, or `None` for a source with nobody nearby
    /// to keep it (which then draws no water and makes no sound).
    pub keeper: Option<ActorId>,
    /// Ordered turns: the front draws next. Household vessels are inserted ahead
    /// of trade vessels (`lore/wells_and_water.md`).
    pub queue: Vec<ActorId>,
    /// The drawer currently at the curb and the real-clock time their draw ends.
    serving: Option<(ActorId, f64)>,
    /// Next real-clock time the keeper works the gear while the source is busy.
    keeper_next_sound: f64,
}

/// One street lamp in a town square (M7). The sim owns the set — positions are
/// derived from the nav graph at seed, the lit state from the lamplighters'
/// dusk beats — and the host mirrors it into lantern props and point lights via
/// `EngineMessage::Lamps`. Nobody lights a lamp but its keeper.
#[derive(Debug, Clone, PartialEq)]
pub struct Lamp {
    /// The square's display name, e.g. `"The Wickmarket"`.
    pub square: String,
    /// Where the post stands, on the square's pavement.
    pub position: Vec3,
    pub lit: bool,
    /// The lamplighter whose beat this square is, or `None` for a square with
    /// no keeper in the cast — its lamps then stay dark.
    pub keeper: Option<ActorId>,
}

/// One leg of a resolved daily round: the office it begins at, where it puts your
/// feet, what you do there, and which weekdays it applies to.
#[derive(Debug, Clone, PartialEq)]
struct RoundLeg {
    from: Office,
    at: Vec3,
    /// The place name for the census (`"Coswald's Yard"`, or `"home"`).
    label: String,
    doing: Arrival,
    only_on: Option<Vec<Weekday>>,
    is_home: bool,
}

/// Where an enrolled townsperson is in their errand. Public for the developer
/// character sheet's [`Round::errand_debug`] view; nothing outside the round
/// ever writes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Standing at some anchor; the ladder may send them somewhere.
    Idle,
    /// Walking to their assigned well.
    Approaching,
    /// Standing in the queue at the curb.
    Queued,
    /// At the front, drawing.
    Drawing,
    /// Carrying the full vessel to its delivery point (home for a household
    /// vessel, the current post for a trade one) after drawing.
    Returning,
    /// Walking to a round leg's anchor (or home, at curfew).
    Travelling,
}

/// One enrolled townsperson: where they sleep, their day, their well (if any),
/// and where they are in it right now.
#[derive(Debug, Clone, PartialEq)]
struct Townsperson {
    /// Their bed, or `None` for the homeless — who are then still in the street
    /// at the Snuffing, exactly as the lore intends.
    home: Option<Vec3>,
    /// Home if housed, else spawn: the point the wander leash and the walk back
    /// from the well are measured from, so even the homeless have somewhere to be.
    base: Vec3,
    /// The office-pegged legs of the day (2–4), anchors already resolved.
    legs: Vec<RoundLeg>,
    leash_m: f64,
    /// A night trade (tavern, watch, lamplighter) that ignores the curfew rung.
    curfew_exempt: bool,
    /// The water source they draw from, or `None` for the non-drawing majority.
    source: Option<usize>,
    is_household: bool,
    phase: Phase,
    /// The destination of the current [`Phase::Travelling`] walk, so a mid-walk
    /// re-decision can tell a genuinely new destination from the journey already
    /// under way (the routed path ends at a snapped nav node, not the target,
    /// so the path itself cannot be compared).
    travel_target: Option<Vec3>,
    /// Whether the current walk serves the character's `go_to` intent (M5), so
    /// a `stop {}` — or the intent ending any other way — halts exactly this
    /// walk and never a round errand that happens to be under way.
    travel_for_intent: bool,
    /// Real-clock time of this idle actor's next ladder evaluation.
    next_decision: f64,
    /// Bumped each decision; the salt that makes the deterministic choices vary.
    epoch: u64,
    /// A pressing rung (curfew, parched) came due while they were held in a
    /// conversation, and the pressure has been injected as a `system:` percept:
    /// they have had their one turn to excuse themselves, and the next pressing
    /// decision walks them regardless of what they said. Cleared once the
    /// exchange lapses.
    excused: bool,
}

impl Townsperson {
    /// A drawer bound to a water source (M3 rungs 2 & 6 apply).
    fn draws_water(&self) -> bool {
        self.source.is_some()
    }
}

/// One enrolled townsperson's errand, reduced for the developer character
/// sheet: the phase, their well, their standing in its queue, where the current
/// walk ends, and whether that walk serves the character's `go_to` intent. A
/// read-only projection of [`Townsperson`]; never handed back to the ladder.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrandDebug {
    pub phase: Phase,
    /// The assigned water source's display name, for the drawing minority.
    pub well: Option<String>,
    /// People ahead of them in the well queue (0 = next up); `Some` only while
    /// [`Phase::Queued`].
    pub ahead_in_queue: Option<usize>,
    /// Where the current walk ends: the draw point while approaching, the
    /// decided target while travelling. `None` in the standing phases (and for
    /// [`Phase::Returning`], whose delivery point is not recorded — the walk's
    /// own last waypoint is the fallback there).
    pub walk_target: Option<Vec3>,
    /// True while a [`Phase::Travelling`] walk serves the `go_to` intent.
    pub for_intent: bool,
}

/// A one-tick behavioural census of the enrolled cast, for `--census-by-area`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Census {
    pub total: usize,
    pub walking: usize,
    pub at_home: usize,
    pub at_post: usize,
    pub in_street: usize,
    /// Populated posts, place → head count, for the "squares should be full at
    /// High Wick" check.
    pub by_place: BTreeMap<String, usize>,
}

impl Census {
    /// A one-line summary for the headless tracer.
    pub fn summary(&self) -> String {
        let mut posts: Vec<(&String, &usize)> = self.by_place.iter().collect();
        posts.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let top: Vec<String> = posts
            .iter()
            .take(6)
            .map(|(place, count)| format!("{place} {count}"))
            .collect();
        format!(
            "{} enrolled | home {} | post {} | walking {} | street {} || {}",
            self.total,
            self.at_home,
            self.at_post,
            self.walking,
            self.in_street,
            top.join(", ")
        )
    }
}

/// The whole daily round: the staffed water sources and every enrolled
/// townsperson. Inert until [`Round::seed`] runs (only when the host supplies a
/// nav graph), so a world without nav has an empty round and no behaviour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Round {
    sources: Vec<WaterSource>,
    people: BTreeMap<ActorId, Townsperson>,
    /// Game-days at the last tick, so thirst decays by the game clock (and speeds
    /// up with the debug time-scale), not by wall-clock.
    last_game_days: f64,
    seeded: bool,
    /// The squares' street lamps (M7), seeded from the nav graph.
    lamps: Vec<Lamp>,
    /// Bumped on any change to the lamp set (including the seed), so the engine
    /// republishes `EngineMessage::Lamps` exactly when something changed.
    lamp_revision: u64,
    /// The game day whose dusk round is under way (stamped at the Lamplight
    /// bell), so each night picks its Belwyn's lamps once.
    lamp_night_day: Option<i64>,
    /// The post each keeper is committed to right now. Chosen nearest-first but
    /// then **held until lit**: re-running "nearest unlit" from a moving body
    /// flips between far posts mid-journey and walks the whole night away
    /// (found in the M7 bring-up), so the greedy choice is made standing, once.
    lamp_targets: BTreeMap<ActorId, usize>,
    /// Tonight's deliberately dark lamp in each square — Belwyn's, left unlit
    /// by rote at a different post each night, one per square
    /// (`lore/places/04_routes_and_sightlines.md`: "Belwyn's lamp rotates
    /// among the lamps in each square"; Tobin Vell's own sheet carries the
    /// ritual). Keyed square name → global lamp index.
    belwyn: BTreeMap<String, usize>,
}

impl Round {
    pub fn new() -> Self {
        Self::default()
    }

    /// The staffed sources, for diagnostics and tests.
    pub fn sources(&self) -> &[WaterSource] {
        &self.sources
    }

    /// The current queue at a named source, front first — for tests/tracing.
    pub fn queue_at(&self, name: &str) -> Option<&[ActorId]> {
        self.sources
            .iter()
            .find(|source| source.name == name)
            .map(|source| source.queue.as_slice())
    }

    /// Whether a drawer is currently drawing at a named source — for tests.
    pub fn is_drawing_at(&self, name: &str) -> bool {
        self.sources
            .iter()
            .find(|source| source.name == name)
            .is_some_and(|source| source.serving.is_some())
    }

    /// The number of enrolled townsfolk, for diagnostics and tests.
    pub fn enrolled(&self) -> usize {
        self.people.len()
    }

    /// The squares' street lamps (M7), for the engine's `Lamps` channel and tests.
    pub fn lamps(&self) -> &[Lamp] {
        &self.lamps
    }

    /// Bumped on any lamp change; the engine republishes when it moves.
    pub fn lamp_revision(&self) -> u64 {
        self.lamp_revision
    }

    /// The errand view of one enrolled townsperson, for the developer character
    /// sheet — or `None` for anyone the round never enrolled (the player, test
    /// fixtures).
    pub fn errand_debug(&self, id: &ActorId) -> Option<ErrandDebug> {
        let person = self.people.get(id)?;
        let source = person.source.map(|index| &self.sources[index]);
        let walk_target = match person.phase {
            Phase::Approaching => source.map(|source| source.draw_point),
            Phase::Travelling => person.travel_target,
            _ => None,
        };
        Some(ErrandDebug {
            phase: person.phase,
            well: source.map(|source| source.name.clone()),
            ahead_in_queue: (person.phase == Phase::Queued)
                .then(|| source.and_then(|source| source.queue.iter().position(|queued| queued == id)))
                .flatten(),
            walk_target,
            for_intent: person.phase == Phase::Travelling && person.travel_for_intent,
        })
    }

    /// A one-line census of the water round for `--trace-water`.
    pub fn water_summary(&self, world: &World) -> String {
        let staffed = self.sources.iter().filter(|source| source.keeper.is_some()).count();
        let queued: usize = self.sources.iter().map(|source| source.queue.len()).sum();
        let drawing = self.sources.iter().filter(|source| source.serving.is_some()).count();
        let drawers = self.people.values().filter(|person| person.draws_water()).count();
        let thirsty = self
            .people
            .iter()
            .filter(|(id, person)| {
                person.draws_water()
                    && world
                        .characters
                        .get(*id)
                        .is_some_and(|character| character.needs().thirst < THIRST_THIRSTY)
            })
            .count();
        format!(
            "water: {} sources / {staffed} kept | {drawers} drawers, {thirsty} thirsty | queued {queued}, drawing {drawing}",
            self.sources.len(),
        )
    }

    /// A behavioural census of the enrolled cast at the current instant: how many
    /// are home, at a post, walking, or left in the street, and which posts are
    /// populated. Reads the current office/weekday off the clock so a leg's
    /// market-day and night rules are respected.
    pub fn census(&self, world: &World, clock: &WorldClock, now: f64) -> Census {
        let time = clock.at(now);
        let mut census = Census {
            total: self.people.len(),
            ..Census::default()
        };
        for (id, person) in &self.people {
            let Some(character) = world.characters.get(id) else {
                continue;
            };
            let position = character.position_m();
            // Near their current post counts as *at* it, walking or standing — a
            // seller milling a few metres across the Wickmarket is still at the
            // Wickmarket. Check this before "walking", so a wander on the spot does
            // not read as a journey.
            if let Some(leg) = active_leg(&person.legs, time.office, time.weekday)
                && position.distance(leg.at) <= CENSUS_POST_RADIUS_M
            {
                if leg.is_home || leg.doing == Arrival::Sleep {
                    census.at_home += 1;
                } else {
                    census.at_post += 1;
                    *census.by_place.entry(leg.label.clone()).or_insert(0) += 1;
                }
                continue;
            }
            if person
                .home
                .is_some_and(|home| position.distance(home) <= CENSUS_HOME_RADIUS_M)
            {
                census.at_home += 1;
            } else if character.is_walking() {
                census.walking += 1;
            } else {
                census.in_street += 1;
            }
        }
        census
    }

    /// Enrol the round. Resolves the water sources and their keepers exactly as
    /// M3 did, then signs up every LLM townsperson with a home, a workplace and a
    /// resolved daily round. Returns one human line per thing that did not
    /// resolve, so the caller can log and carry on — it never panics.
    pub fn seed(&mut self, world: &mut World, nav: &NavData, now: f64, clock: &WorldClock) -> Vec<String> {
        let mut diagnostics = Vec::new();
        if self.seeded {
            return diagnostics;
        }
        self.seeded = true;
        self.last_game_days = clock.game_days(now);

        // Resolve the water sources present in this nav graph.
        for &(name, sound) in SOURCES {
            match nav.place(name) {
                Some(place) => self.sources.push(WaterSource {
                    name: name.to_string(),
                    draw_point: nav.node_point(place.node),
                    draw_sound: sound,
                    keeper: None,
                    queue: Vec::new(),
                    serving: None,
                    keeper_next_sound: now,
                }),
                None => diagnostics.push(format!(
                    "[smart actors] round: nav place {name:?} is missing; source skipped"
                )),
            }
        }

        // Every LLM townsperson except the player, in deterministic roster order.
        let townsfolk: Vec<ActorId> = world
            .roster
            .iter()
            .filter(|id| {
                world
                    .characters
                    .get(*id)
                    .is_some_and(|character| character.control().is_llm())
            })
            .cloned()
            .collect();

        // A keeper each: the nearest free **ambient** townsperson to the curb,
        // pinned there so a queue has someone to form on. Keepers are enrolled
        // like everyone else below, but with the well as their round's one post,
        // so their own day never drags them off the curb.
        let mut keepers: BTreeMap<ActorId, usize> = BTreeMap::new();
        for index in 0..self.sources.len() {
            let draw_point = self.sources[index].draw_point;
            let keeper = townsfolk
                .iter()
                .filter(|id| {
                    !keepers.contains_key(*id)
                        && world.characters[*id].significance() == Significance::Ambient
                })
                .filter_map(|id| {
                    let distance = world.characters[id].position_m().distance(draw_point);
                    (distance <= KEEPER_MAX_DIST_M).then_some((distance, id))
                })
                .min_by(|left, right| {
                    left.0
                        .partial_cmp(&right.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left.1.cmp(right.1))
                })
                .map(|(_, id)| id.clone());
            if let Some(keeper) = keeper {
                keepers.insert(keeper.clone(), index);
                world.characters.get_mut(&keeper).expect("keeper exists").state.position_m = draw_point;
                self.sources[index].keeper = Some(keeper);
            }
        }

        let staffed: Vec<usize> = (0..self.sources.len())
            .filter(|index| self.sources[*index].keeper.is_some())
            .collect();

        // The embedded content. A parse failure disables the daily round but not
        // the water sources already resolved above.
        let content: Option<(RoundsDoc, HomesDoc)> = match (
            serde_json::from_str::<RoundsDoc>(ROUNDS_JSON),
            serde_json::from_str::<HomesDoc>(HOMES_JSON),
        ) {
            (Ok(rounds), Ok(homes)) => Some((rounds, homes)),
            (rounds, homes) => {
                if let Err(error) = rounds {
                    diagnostics.push(format!("[smart actors] round: rounds.json did not load: {error}"));
                }
                if let Err(error) = homes {
                    diagnostics.push(format!("[smart actors] round: homes.json did not load: {error}"));
                }
                None
            }
        };
        let resolver = PlaceResolver::new(nav);

        // The wayfinding registry (M5): the baked places and ward anchors, plus
        // one home entry per housed townsperson — "Tam Rud's house" — so
        // `places_you_know` has handles to hold and `tell_way` something to
        // share. Every home is registered BEFORE anyone's whitelist is seeded:
        // an actor early in the roster may know somebody housed later in it.
        let mut registry = match PlaceRegistry::from_embedded(nav) {
            Ok(registry) => registry,
            Err(error) => {
                diagnostics.push(format!(
                    "[smart actors] round: places.json did not load: {error}"
                ));
                PlaceRegistry::default()
            }
        };
        if let Some((_, homes)) = &content {
            for id in &townsfolk {
                if let Some(entry) = homes.homes.get(id.as_str()) {
                    let point = Vec3::new(entry.point[0], WALK_Y, entry.point[1]);
                    registry.add_home(id, world.characters[id].name(), point);
                }
            }
        }

        // The lamps of the five squares (M7; `features/50_cool_suggestions.md`
        // #21): a ring of posts on each square's pavement, all dark until its
        // keeper's dusk beat. The beats are authored in `rounds.json:
        // lamp_keepers` — one keeper per square, several squares only for
        // Tobin's central pair — because the fortified street maze puts
        // adjacent squares 1.3–2 km apart on foot: one walker cannot light
        // five squares in a night, and four lamplighters are already in the
        // cast.
        let square_keeper: HashMap<&str, ActorId> = content
            .as_ref()
            .map(|(rounds, _)| {
                let mut map = HashMap::new();
                for (keeper, squares) in &rounds.lamp_keepers {
                    let Some(id) = townsfolk.iter().find(|id| id.as_str() == keeper) else {
                        diagnostics.push(format!(
                            "[smart actors] round: lamp keeper {keeper:?} is not in the cast; their squares stay dark"
                        ));
                        continue;
                    };
                    for square in squares {
                        map.insert(square.as_str(), id.clone());
                    }
                }
                map
            })
            .unwrap_or_default();
        for &name in LAMP_SQUARES {
            match resolver.resolve_centre(name) {
                Some(centre) => {
                    let keeper = square_keeper.get(name).cloned();
                    for position in lamp_ring(nav, centre) {
                        self.lamps.push(Lamp {
                            square: name.to_string(),
                            position,
                            lit: false,
                            keeper: keeper.clone(),
                        });
                    }
                }
                None => diagnostics.push(format!(
                    "[smart actors] round: lamp square {name:?} is missing from the graph"
                )),
            }
        }
        if !self.lamps.is_empty() {
            // Revision 1: the first poll publishes the (dark) set, so the host
            // can stand the posts up before dusk.
            self.lamp_revision = 1;
            let kept: BTreeSet<&str> = self
                .lamps
                .iter()
                .filter(|lamp| lamp.keeper.is_some())
                .map(|lamp| lamp.square.as_str())
                .collect();
            diagnostics.push(format!(
                "[smart actors] round: {} lamps across {} squares, {} squares kept",
                self.lamps.len(),
                LAMP_SQUARES.len(),
                kept.len(),
            ));
        }

        let mut enrolled = 0usize;
        let mut housed = 0usize;
        let mut drawers = 0usize;
        for id in &townsfolk {
            let character = &world.characters[id];
            let spawn = character.position_m();
            let occupation = character.lore().and_then(|lore| lore.occupation_id.clone());

            let home = content
                .as_ref()
                .and_then(|(_, homes)| homes.homes.get(id.as_str()))
                .map(|entry| Vec3::new(entry.point[0], WALK_Y, entry.point[1]));
            if home.is_some() {
                housed += 1;
            }
            let base = home.unwrap_or(spawn);

            // The wayfinding whitelist, assembled per actor (05_the_llm_seam.md
            // §3): the coarse destinations everyone holds (the major squares
            // and the wards, so getting somewhere always has a legal first
            // step), the places of their own ward, their own home, and the
            // homes of the people they know. The branch below adds what each
            // kind of day touches — the keeper's well, the worker's legs. It
            // is also, quietly, characterisation: which ids someone holds is
            // who they are.
            let mut known: BTreeSet<PlaceId> =
                registry.coarse().map(|entry| entry.id.clone()).collect();
            if let Some(ward) = character.lore().map(|lore| lore.planning_ward) {
                known.extend(registry.ward_places(ward.as_str()).map(|entry| entry.id.clone()));
            }
            if let Some(entry) = registry.home_of(id) {
                known.insert(entry.id.clone());
            }
            for friend in character.knows() {
                if let Some(entry) = registry.home_of(friend) {
                    known.insert(entry.id.clone());
                }
            }

            // A keeper's round *is* their well: at the curb from the Kindling,
            // home to sleep at the Lamplight like any day worker — and the
            // curfew rung sends the housed home at night regardless (keepers
            // are not on `04_the_round.md` §6's list of who stays out). They
            // are never water-bound themselves: the curb is theirs to work,
            // not to queue at. The homeless among them behave like any other
            // homeless actor — still at the curb at the Snuffing.
            if let Some(&source_idx) = keepers.get(id) {
                let source = &self.sources[source_idx];
                if let Some(entry) = registry.named(&source.name) {
                    known.insert(entry.id.clone());
                }
                let mut legs = vec![RoundLeg {
                    from: Office::Kindling,
                    at: source.draw_point,
                    label: source.name.clone(),
                    doing: Arrival::Work,
                    only_on: None,
                    is_home: false,
                }];
                if let Some(home) = home {
                    legs.push(RoundLeg {
                        from: Office::Lamplight,
                        at: home,
                        label: "home".to_string(),
                        doing: Arrival::Sleep,
                        only_on: None,
                        is_home: true,
                    });
                }
                world
                    .characters
                    .get_mut(id)
                    .expect("keeper exists")
                    .state
                    .places_known = known;
                self.people.insert(
                    id.clone(),
                    Townsperson {
                        home,
                        base,
                        legs,
                        leash_m: KEEPER_LEASH_M,
                        curfew_exempt: false,
                        source: None,
                        is_household: false,
                        phase: Phase::Idle,
                        travel_target: None,
                        travel_for_intent: false,
                        next_decision: now + decision_jitter(id, 0),
                        epoch: 0,
                        excused: false,
                    },
                );
                enrolled += 1;
                continue;
            }

            let (legs, leash_m, curfew_exempt) = content
                .as_ref()
                .map(|(rounds, _)| build_legs(rounds, &resolver, id, occupation.as_deref(), home, base))
                .unwrap_or((Vec::new(), DEFAULT_ROUND_LEASH_M, false));

            // The day's own stations — the workplace and every named leg — are
            // ways this person necessarily knows.
            for leg in &legs {
                if leg.is_home {
                    continue;
                }
                if let Some(entry) = registry.named(&leg.label) {
                    known.insert(entry.id.clone());
                }
            }

            // Water binding: only the drawing occupations get a source, bound to
            // the nearest staffed well; their thirst is spread so the curbs get
            // busy at once rather than all later.
            let (source, is_household) = match vessel_of(occupation.as_deref()) {
                Some(is_household) if !staffed.is_empty() => {
                    let nearest = *staffed
                        .iter()
                        .min_by(|left, right| {
                            let dl = self.sources[**left].draw_point.distance(base);
                            let dr = self.sources[**right].draw_point.distance(base);
                            dl.partial_cmp(&dr)
                                .unwrap_or(std::cmp::Ordering::Equal)
                                // Explicit tie-break by source index for determinism.
                                .then_with(|| left.cmp(right))
                        })
                        .expect("staffed is non-empty");
                    let thirst = THIRST_MAX * hash01("water_thirst_seed", id, 0);
                    world.characters.get_mut(id).expect("drawer exists").state.needs.thirst = thirst;
                    drawers += 1;
                    (Some(nearest), is_household)
                }
                _ => (None, false),
            };

            world
                .characters
                .get_mut(id)
                .expect("townsperson exists")
                .state
                .places_known = known;
            self.people.insert(
                id.clone(),
                Townsperson {
                    home,
                    base,
                    legs,
                    leash_m,
                    curfew_exempt,
                    source,
                    is_household,
                    phase: Phase::Idle,
                    travel_target: None,
                    travel_for_intent: false,
                    next_decision: now + decision_jitter(id, 0),
                    epoch: 0,
                    excused: false,
                },
            );
            enrolled += 1;
        }

        diagnostics.push(format!(
            "[smart actors] round: {} water sources, {} staffed | {enrolled} enrolled ({} well keepers), {housed} housed, {drawers} water drawers",
            self.sources.len(),
            staffed.len(),
            keepers.len(),
        ));
        diagnostics.push(format!(
            "[smart actors] wayfinding: {} places in the registry ({housed} homes)",
            registry.len(),
        ));
        world.places = registry;
        diagnostics
    }
}

/// Build a townsperson's resolved legs from their route override (the 19 authored
/// majors) or their occupation template, dropping any leg whose anchor does not
/// resolve.
fn build_legs(
    rounds: &RoundsDoc,
    resolver: &PlaceResolver,
    id: &ActorId,
    occupation: Option<&str>,
    home: Option<Vec3>,
    base: Vec3,
) -> (Vec<RoundLeg>, f64, bool) {
    // The workplace: the nearest of the occupation's candidate places to home.
    let (work_point, work_label) = occupation
        .and_then(|occupation| rounds.workplaces.get(occupation))
        .into_iter()
        .flatten()
        .filter_map(|name| resolver.resolve(name).map(|point| (point, name.clone())))
        .min_by(|left, right| {
            left.0
                .distance(base)
                .partial_cmp(&right.0.distance(base))
                .unwrap_or(std::cmp::Ordering::Equal)
                // Explicit tie-break by place name, so an exact distance tie is
                // resolved the same way every run (matching the keeper code).
                .then_with(|| left.1.cmp(&right.1))
        })
        .map_or((None, None), |(point, label)| (Some(point), Some(label)));

    // Route override for the majors, else the occupation's archetype.
    let (leg_specs, leash_m, curfew_exempt): (&[LegSpec], f64, bool) =
        if let Some(route) = rounds.routes.get(id.as_str()) {
            (
                &route.legs,
                route.leash_m.unwrap_or(DEFAULT_ROUND_LEASH_M),
                route.curfew_exempt.unwrap_or(false),
            )
        } else if let Some(template) = occupation
            .and_then(|occupation| rounds.occupations.get(occupation))
            .and_then(|archetype| rounds.archetypes.get(archetype))
        {
            (&template.legs, template.leash_m, template.curfew_exempt)
        } else {
            (&[], DEFAULT_ROUND_LEASH_M, false)
        };

    let mut legs = Vec::with_capacity(leg_specs.len());
    for spec in leg_specs {
        let (at, label, is_home) = match spec.at.as_str() {
            "home" => match home {
                Some(home) => (home, "home".to_string(), true),
                None => continue, // the homeless have no bed to walk to
            },
            "workplace" => match (work_point, &work_label) {
                (Some(point), Some(label)) => (point, label.clone(), false),
                _ => continue, // no workplace resolved for this trade
            },
            name => match resolver.resolve(name) {
                Some(point) => (point, name.to_string(), false),
                None => continue, // a place not in this graph
            },
        };
        legs.push(RoundLeg {
            from: spec.from,
            at,
            label,
            doing: spec.doing,
            only_on: spec.only_on.clone(),
            is_home,
        });
    }
    (legs, leash_m, curfew_exempt)
}

/// The active leg at a given office and weekday: among the eligible legs (begun
/// by `office`, allowed today), the one with the greatest `from`, later
/// array-position winning a tie — so a market-day leg placed after the generic
/// one wins on its day and is filtered out otherwise. If nothing has begun yet
/// (deep night before the first leg), the day's tail leg carries over.
fn active_leg(legs: &[RoundLeg], office: Office, weekday: Weekday) -> Option<&RoundLeg> {
    let eligible = |leg: &&RoundLeg| leg.only_on.as_ref().is_none_or(|days| days.contains(&weekday));
    let pick = |filter: &dyn Fn(&&RoundLeg) -> bool| -> Option<&RoundLeg> {
        let mut best: Option<&RoundLeg> = None;
        for leg in legs.iter().filter(|leg| filter(leg)) {
            if best.is_none_or(|current| leg.from >= current.from) {
                best = Some(leg);
            }
        }
        best
    };
    pick(&|leg| leg.from <= office && eligible(leg)).or_else(|| pick(&eligible))
}

/// Advance the round one poll: decay thirst, resolve arrivals, drive the `go_to`
/// intents, work the well queues, and run the ladder. A no-op until
/// [`Round::seed`] has run. `player_id` is who the well sounds play *for*.
///
/// `in_conversation` is everyone currently in a warm exchange — the player's
/// partner and every warm NPC↔NPC pair: their rounds are on hold — the ladder's
/// deferrable rungs skip them and a finished draw does not send them home —
/// because nobody walks off mid-conversation. The pressing rungs (curfew,
/// parched) still break the hold, after one turn to excuse themselves
/// ([`run_ladder`]) — and a `go_to` errand never waits for it at all: leaving
/// is the character's own decision, made in the same reply as the goodbye. The
/// hold ends when the exchange goes cold and the caller stops naming them.
///
/// Returns the actors owed a **priority nudge**: a `go_to` arrival or lapse
/// grants the same handoff an addressed `say` does, because off stage the idle
/// rotation never runs — an arrival percept nobody renders would silently kill
/// the errand chain (`features/movement/05_the_llm_seam.md` §3). The engine
/// feeds them to the scheduler; the sim stays scheduler-free.
pub fn tick(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
    player_id: &ActorId,
    in_conversation: &BTreeSet<ActorId>,
) -> Vec<ActorId> {
    let mut nudges: Vec<ActorId> = Vec::new();
    if !round.seeded {
        return nudges;
    }
    decay_thirst(round, world, clock, now);
    tick_lamps(round, clock, now);
    resolve_arrivals(round, world);
    tick_intents(round, world, nav, now, &mut nudges);
    service_sources(round, world, nav, clock, now, player_id, in_conversation);
    run_ladder(round, world, nav, clock, now, in_conversation, &mut nudges);
    nudges
}

/// The offices a lit lamp belongs to: dusk through the small hours. The
/// Kindling snuffs the whole set at once — first light does the honest work,
/// and nobody is on the street to watch a snuffing round at 6 a.m.
fn lamp_window(office: Office) -> bool {
    matches!(office, Office::Lamplight | Office::Snuffing | Office::Watch)
}

/// The lamp housekeeping (M7): stamp a fresh dusk — choosing tonight's
/// Belwyn's lamp in each kept square, the one left dark by rote — and snuff
/// everything at first light. The *lighting* is each keeper's own ladder rung;
/// nothing here flips a lamp on.
fn tick_lamps(round: &mut Round, clock: &WorldClock, now: f64) {
    if round.lamps.is_empty() {
        return;
    }
    let time = clock.at(now);
    if !lamp_window(time.office) {
        round.lamp_targets.clear();
        if round.lamps.iter().any(|lamp| lamp.lit) {
            for lamp in &mut round.lamps {
                lamp.lit = false;
            }
            round.lamp_revision += 1;
        }
        return;
    }
    // A fresh dusk (the Watch after midnight continues the *previous* day's
    // night, so only the Lamplight bell opens one; a run started mid-night
    // stays dark until its first dusk).
    if time.office == Office::Lamplight && round.lamp_night_day != Some(time.day) {
        round.lamp_night_day = Some(time.day);
        round.lamp_targets.clear();
        round.belwyn.clear();
        // One Belwyn's lamp per kept square, rolled off the keeper and the day.
        let mut posts: BTreeMap<String, (ActorId, Vec<usize>)> = BTreeMap::new();
        for (index, lamp) in round.lamps.iter().enumerate() {
            if let Some(keeper) = &lamp.keeper {
                posts
                    .entry(lamp.square.clone())
                    .or_insert_with(|| (keeper.clone(), Vec::new()))
                    .1
                    .push(index);
            }
        }
        for (square, (keeper, indices)) in posts {
            let roll = hash01("belwyns_lamp", &keeper, time.day as u64 ^ stable_hash(&square));
            let pick = indices[(roll * indices.len() as f64) as usize % indices.len()];
            round.belwyn.insert(square, pick);
        }
    }
}

/// A small stable hash of a square name, folded into the Belwyn roll so two
/// squares on one keeper's beat pick independently.
fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The nearest unlit lamp on `id`'s beat, skipping each square's Belwyn's
/// lamp. `None` once the beat is done (or for anyone who keeps no lamps).
fn next_unlit_lamp(round: &Round, id: &ActorId, position: Vec3) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (index, lamp) in round.lamps.iter().enumerate() {
        if lamp.lit
            || lamp.keeper.as_ref() != Some(id)
            || round.belwyn.get(&lamp.square) == Some(&index)
        {
            continue;
        }
        let distance = position.distance(lamp.position);
        if best.is_none_or(|(nearest, _)| distance < nearest) {
            best = Some((distance, index));
        }
    }
    best.map(|(_, index)| index)
}

/// A ring of lamp posts around a square's interior node, each probed inward
/// until it stands on pavement (an off-pavement angle is simply skipped, and a
/// collapsed duplicate dropped).
fn lamp_ring(nav: &NavData, centre: Vec3) -> Vec<Vec3> {
    let mut posts: Vec<Vec3> = Vec::with_capacity(LAMPS_PER_SQUARE);
    for k in 0..LAMPS_PER_SQUARE {
        let angle = (k as f64 + 0.5) / LAMPS_PER_SQUARE as f64 * std::f64::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        for step in 0..=8 {
            let radius = LAMP_RING_RADIUS_M - step as f64 * 1.25;
            if radius <= 0.0 {
                break;
            }
            let candidate = Vec3::new(centre.x + cos * radius, WALK_Y, centre.z + sin * radius);
            if nav.is_walkable(candidate.x, candidate.z)
                && !posts.iter().any(|post| planar_close(*post, candidate))
            {
                posts.push(candidate);
                break;
            }
        }
    }
    posts
}

/// Stop `id`'s round errand where they stand: they have just exchanged a line
/// (or an item) with another character, and nobody keeps walking away from a
/// conversation. Called the moment the exchange is registered — before the
/// next movement slice — so the partner does not drift out of interaction
/// range while answering.
///
/// Only walks are interrupted; someone Queued or Drawing at a well is already
/// standing still and keeps their place. Dropping the walker to [`Phase::Idle`]
/// hands them back to the ladder, which re-decides once the exchange goes cold
/// (the [`tick`] hold above) — so an interrupted errand resumes on its own.
/// Anyone not enrolled (a scripted mover) is none of the round's business and
/// is left alone — as is an *excused* walker: their pressing errand has already
/// outranked the conversation, and a parting line must not stop them again.
pub fn interrupt_for_conversation(round: &mut Round, world: &mut World, id: &ActorId) {
    let Some(person) = round.people.get_mut(id) else {
        return;
    };
    if person.excused {
        return;
    }
    if !matches!(
        person.phase,
        Phase::Approaching | Phase::Travelling | Phase::Returning
    ) {
        return;
    }
    person.phase = Phase::Idle;
    person.travel_for_intent = false;
    if let Some(character) = world.characters.get_mut(id) {
        character.state.movement = None;
    }
}

/// The per-poll intent pass (M5): stamp fresh deadlines — and set a fresh
/// errand walking the same tick — detect arrival, track a followed person,
/// and lapse what expired. Runs every poll rather than on the ladder cadence,
/// because "sets off for the Wickmarket" should mean now, a follow re-paths
/// against a moving target, and an arrival percept should land the tick it
/// happens (`features/movement/05_the_llm_seam.md` §2).
fn tick_intents(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    now: f64,
    nudges: &mut Vec<ActorId>,
) {
    let ids: Vec<ActorId> = round.people.keys().cloned().collect();
    for id in ids {
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        let Some(mut intent) = character.state.intent.clone() else {
            // The intent ended outside this pass — `stop {}` is self-initiated
            // and emits no percept — but its walk may still be under way: halt
            // exactly that walk and hand the body back to the ladder.
            let person = round.people.get_mut(&id).expect("person exists");
            if person.travel_for_intent {
                person.travel_for_intent = false;
                if person.phase == Phase::Travelling {
                    person.phase = Phase::Idle;
                    person.travel_target = None;
                    world
                        .characters
                        .get_mut(&id)
                        .expect("mover exists")
                        .state
                        .movement = None;
                }
            }
            continue;
        };
        let position = character.position_m();
        // The verb has no clock, so the first tick that sees the intent stamps
        // its expiry — and, below, sets the feet moving in the same breath.
        let fresh = intent.deadline.is_none();
        let deadline = *intent.deadline.get_or_insert(now + intent.budget_seconds);

        // The endings, in order: arrival first (an errand that arrives on its
        // last second arrived), then expiry. `ending` clears the intent with a
        // percept and the nudge; `notice` is a percept alone (losing sight
        // degrades the follow, it does not end it).
        let mut ending: Option<String> = None;
        let mut notice: Option<String> = None;
        match &mut intent.target {
            IntentTarget::Place { name, point, .. } => {
                if position.distance(*point) <= PLACE_ARRIVE_RADIUS_M {
                    ending = Some(format!("You have arrived at {name}."));
                }
            }
            IntentTarget::Person {
                actor_id: target_id,
                last_seen,
                visible,
            } => match world.characters.get(target_id).map(Character::position_m) {
                // Characters are never removed; fail soft if one ever is.
                None => ending = Some("Your errand has lapsed.".to_string()),
                Some(target_position) => {
                    if position.distance(target_position) <= HEARING_RADIUS_M {
                        *last_seen = target_position;
                        *visible = true;
                        if position.distance(target_position) <= PERSON_ARRIVE_RADIUS_M {
                            ending = Some(format!(
                                "You have caught up with {}.",
                                identify_ids(world, &id, target_id)
                            ));
                        }
                    } else if *visible {
                        // The sight gate: the follow degrades to the last-seen
                        // position rather than tracking through walls.
                        *visible = false;
                        notice = Some(format!(
                            "You have lost sight of {}.",
                            identify_ids(world, &id, target_id)
                        ));
                    } else if position.distance(*last_seen) <= PLACE_ARRIVE_RADIUS_M {
                        ending = Some(format!(
                            "You reach the spot where you last saw {}, but they are gone.",
                            identify_ids(world, &id, target_id)
                        ));
                    }
                }
            },
        }

        if let Some(line) = ending {
            end_intent(round, world, &id, line, nudges);
            continue;
        }
        if now >= deadline {
            let line = match &intent.target {
                IntentTarget::Place { name, .. } => {
                    format!("Your errand to {name} lapsed before you arrived.")
                }
                IntentTarget::Person {
                    actor_id: target_id, ..
                } => format!(
                    "You never caught up with {}; the errand has lapsed.",
                    identify_ids(world, &id, target_id)
                ),
            };
            end_intent(round, world, &id, line, nudges);
            continue;
        }
        if let Some(line) = notice {
            world
                .characters
                .get_mut(&id)
                .expect("the follower exists")
                .notify_percept(line);
        }
        // Persist the bookkeeping (the deadline stamp, the follow's last-seen).
        world
            .characters
            .get_mut(&id)
            .expect("the walker exists")
            .state
            .intent = Some(intent.clone());

        // Lay the walk. Two cases run here, per poll, instead of waiting for
        // the ladder's 1–6 s cadence:
        //
        //   * a **fresh** intent sets off THIS tick — "sets off for the
        //     Wickmarket" should mean now, not after a hesitation beat
        //     (playtest: even the cadence read as standing around);
        //   * a **follow** whose target is visible tracks them, re-laid as
        //     they move.
        //
        // Everything else — resuming after a conversation interrupt, walking
        // to a lost target's last-seen spot — goes through the ladder rung on
        // its cadence: the pause after answering a line is the answer's beat,
        // and the rung order is what lets the needs outrank the errand. Only
        // the free phases lay anything; a committed well errand keeps its
        // queue place, and the rung picks the intent up when it resolves.
        let (target, arrive_radius) = match &intent.target {
            IntentTarget::Place { point, .. } => (*point, PLACE_ARRIVE_RADIUS_M),
            IntentTarget::Person { last_seen, .. } => (*last_seen, PERSON_ARRIVE_RADIUS_M),
        };
        let tracking = matches!(
            &intent.target,
            IntentTarget::Person { visible: true, .. }
        );
        let person = &round.people[&id];
        let lay = match person.phase {
            Phase::Idle => fresh || tracking,
            Phase::Travelling => {
                (fresh || tracking)
                    && person
                        .travel_target
                        .is_none_or(|aimed| aimed.distance(target) > FOLLOW_REPATH_EPSILON_M)
            }
            _ => false,
        };
        if lay
            && position.distance(target) > arrive_radius
            && let Some(path) = route_path_to_point(nav, &id, position, target)
        {
            set_route(world, &id, path);
            let person = round.people.get_mut(&id).expect("person exists");
            person.phase = Phase::Travelling;
            person.travel_target = Some(target);
            person.travel_for_intent = true;
        }
    }
}

/// End a `go_to` intent: the percept that says why, the halt of the intent's
/// own walk, and the **priority nudge** an addressed `say` gets — an arrival
/// turn is how "the NPC narrates their own arrival" without anybody scripting
/// it, on stage or off (05_the_llm_seam.md §2–3).
fn end_intent(
    round: &mut Round,
    world: &mut World,
    id: &ActorId,
    line: String,
    nudges: &mut Vec<ActorId>,
) {
    if let Some(character) = world.characters.get_mut(id) {
        character.state.intent = None;
        character.notify_percept(line);
    }
    nudges.push(id.clone());
    let person = round.people.get_mut(id).expect("person exists");
    if person.travel_for_intent {
        person.travel_for_intent = false;
        if person.phase == Phase::Travelling {
            person.phase = Phase::Idle;
            person.travel_target = None;
            if let Some(character) = world.characters.get_mut(id) {
                character.state.movement = None;
            }
        }
    }
}

/// Thirst falls by the game clock, so it keeps pace with the sun and the debug
/// time-scale. Only water-drawers decay — the rest of the cast has no thirst gauge
/// in play.
fn decay_thirst(round: &mut Round, world: &mut World, clock: &WorldClock, now: f64) {
    let game_days = clock.game_days(now);
    let delta_days = (game_days - round.last_game_days).max(0.0);
    round.last_game_days = game_days;
    let drop = delta_days * 86_400.0 * crate::THIRST_DECAY_PER_GAME_SECOND;
    if drop <= 0.0 {
        return;
    }
    for (id, person) in &round.people {
        if !person.draws_water() {
            continue;
        }
        if let Some(character) = world.characters.get_mut(id) {
            character.state.needs.thirst = (character.state.needs.thirst - drop).max(0.0);
        }
    }
}

/// Move arrivals through the state machine: an approacher who reached the curb
/// joins the queue; a returner or traveller who reached their anchor falls idle.
fn resolve_arrivals(round: &mut Round, world: &mut World) {
    let ids: Vec<ActorId> = round.people.keys().cloned().collect();
    for id in ids {
        let (phase, source_idx) = {
            let person = &round.people[&id];
            (person.phase, person.source)
        };
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        if character.is_walking() {
            continue; // still on the way
        }
        let position = character.position_m();

        match phase {
            Phase::Approaching => {
                let source_idx = source_idx.expect("an approacher has a water source");
                let at_curb =
                    position.distance(round.sources[source_idx].draw_point) <= WELL_ARRIVE_RADIUS_M;
                if at_curb {
                    enqueue(round, source_idx, id.clone());
                    round.people.get_mut(&id).expect("person exists").phase = Phase::Queued;
                    world.characters.get_mut(&id).expect("drawer exists").state.movement = None;
                } else {
                    round.people.get_mut(&id).expect("person exists").phase = Phase::Idle;
                }
            }
            Phase::Returning | Phase::Travelling => {
                round.people.get_mut(&id).expect("person exists").phase = Phase::Idle;
                world.characters.get_mut(&id).expect("mover exists").state.movement = None;
            }
            _ => {}
        }
    }
}

/// Insert `actor` into the source's queue, household vessels ahead of trade ones,
/// preserving arrival order within each class (`lore/wells_and_water.md`).
fn enqueue(round: &mut Round, source_idx: usize, actor: ActorId) {
    let is_household = round.people[&actor].is_household;
    if is_household {
        let insert_at = round.sources[source_idx]
            .queue
            .iter()
            .position(|id| !round.people.get(id).is_some_and(|person| person.is_household))
            .unwrap_or(round.sources[source_idx].queue.len());
        round.sources[source_idx].queue.insert(insert_at, actor);
    } else {
        round.sources[source_idx].queue.push(actor);
    }
}

/// Work each curb: finish a completed draw (refill, remember it, send home),
/// start the next one, and clank the gear while anybody is waiting.
///
/// **The well sound is a clock, not an event** (`features/movement/05_the_llm_seam.md`
/// §5.2, exactly as the bell is): an unattributed world sound heard only by the
/// player, so it reaches no NPC inbox and never nudges a reaction turn. The
/// drawer instead *remembers their own draw*, so the person the player asks can
/// still say what they are doing.
fn service_sources(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
    player_id: &ActorId,
    in_conversation: &BTreeSet<ActorId>,
) {
    let time = clock.at(now);
    let mut finished: Vec<ActorId> = Vec::new();
    let mut started: Vec<ActorId> = Vec::new();
    let mut emissions: Vec<(&'static str, Vec3)> = Vec::new();

    for source in &mut round.sources {
        if let Some((drawer, ends_at)) = source.serving.clone()
            && now >= ends_at
        {
            source.serving = None;
            if source.queue.first() == Some(&drawer) {
                source.queue.remove(0);
            } else {
                source.queue.retain(|id| id != &drawer);
            }
            finished.push(drawer);
        }
        if source.serving.is_none()
            && source.keeper.is_some()
            && let Some(front) = source.queue.first().cloned()
        {
            source.serving = Some((front.clone(), now + WATER_DRAW_SECONDS));
            started.push(front);
        }
        let busy = source.serving.is_some() || !source.queue.is_empty();
        if busy && source.keeper.is_some() && now >= source.keeper_next_sound {
            source.keeper_next_sound = now + WELL_KEEPER_SOUND_INTERVAL_SECONDS;
            emissions.push((source.draw_sound, source.draw_point));
        }
    }

    for drawer in started {
        let self_line = round
            .people
            .get(&drawer)
            .and_then(|person| person.source)
            .map(|source| round.sources[source].draw_sound)
            .and_then(|sound_id| world.sound_catalog.get(sound_id))
            .and_then(|sound| sound.seen.as_ref())
            .map(|seen| seen.replace("{actor}", "You"));
        if let Some(person) = round.people.get_mut(&drawer) {
            person.phase = Phase::Drawing;
        }
        if let (Some(line), Some(character)) = (self_line, world.characters.get_mut(&drawer)) {
            character.remember_percept(line);
        }
    }
    for drawer in finished {
        if let Some(character) = world.characters.get_mut(&drawer) {
            character.state.needs.thirst = THIRST_MAX; // a full vessel
        }
        // Mid-exchange (with the player or a neighbour): stand at the curb
        // instead of walking off; the ladder takes over once it goes cold.
        if in_conversation.contains(&drawer) {
            world.characters.get_mut(&drawer).expect("drawer exists").state.movement = None;
            round.people.get_mut(&drawer).expect("person exists").phase = Phase::Idle;
            continue;
        }
        // Carry the full vessel where its water is owed: a household vessel to
        // the home, a trade vessel to the workshop the current leg names — so a
        // fuller resumes their post instead of trudging home first. The ladder
        // resumes on arrival either way (`delivery_point`).
        let target = round
            .people
            .get(&drawer)
            .map(|person| delivery_point(person, time.office, time.weekday));
        let position = world.characters.get(&drawer).map(Character::position_m);
        if let (Some(target), Some(position)) = (target, position) {
            match route_path(nav, &drawer, position, target) {
                Some(path) => set_route(world, &drawer, path),
                None => {
                    world.characters.get_mut(&drawer).expect("drawer exists").state.movement = None;
                }
            }
        }
        if let Some(person) = round.people.get_mut(&drawer) {
            person.phase = Phase::Returning;
        }
    }

    // The player-audible ambient: an unattributed world sound at the curb.
    let player_pos = world.characters.get(player_id).map(Character::position_m);
    for (sound_id, position) in emissions {
        if !world.sounds_enabled {
            continue;
        }
        let Some(sound) = world.sound_catalog.get(sound_id).cloned() else {
            continue;
        };
        let recipients = match player_pos {
            Some(pos) if pos.distance(position) <= sound.audible_distance => vec![player_id.clone()],
            _ => Vec::new(),
        };
        world.emit(DomainEvent::sound(
            sound.sound_class.clone(),
            None, // a world sound, like the bell — never attributed, never nudges
            sound.sound_id.clone(),
            sound.audible_distance,
            position,
            recipients,
            Vec::new(),
        ));
    }
}

/// Where a finished drawer carries the full vessel. A household vessel is water
/// for the home, so it goes to `base` (home if housed, else spawn); a trade
/// vessel is water for the workshop, so it goes to the active round leg's
/// anchor — the fuller "arrives late" at his post rather than walking home
/// first (`04_the_round.md` §8) — falling back to `base` when no leg is active.
/// At night the non-exempt deliver homeward (`base` *is* home for the housed),
/// so the committed return leg never marches against the curfew rung that runs
/// on arrival.
fn delivery_point(person: &Townsperson, office: Office, weekday: Weekday) -> Vec3 {
    let night = matches!(office, Office::Snuffing | Office::Watch);
    if person.is_household || (night && !person.curfew_exempt) {
        return person.base;
    }
    active_leg(&person.legs, office, weekday).map_or(person.base, |leg| leg.at)
}

/// The behaviour ladder, run for idle people whose walk has ended and whose
/// cadence has come round. First match wins (`features/movement/03_the_ladder.md`
/// §4); M4 adds the curfew (5) and round (9) rungs to M3's water (2, 6), social
/// (11) and wander (12).
///
/// A [`Phase::Travelling`] walk is re-decided on the same cadence, so a higher
/// rung — curfew at the Snuffing, a new schedule leg at an office change, parched
/// thirst — *preempts* the journey instead of waiting for the obsolete one to
/// finish (`04_the_round.md` §5: the higher rungs all preempt the round). The
/// traveller is only ever re-aimed, never stopped: a wander/stay outcome does not
/// interrupt them, and a decision for the destination already under way changes
/// nothing. The well errand's own phases (Approaching/Queued/Drawing/Returning)
/// stay committed as before.
///
/// A warm conversation (`in_conversation`) holds its members — but only against
/// the rungs below curfew: the round (9), the social pull (11) and the wander
/// (12) wait for the exchange to lapse, while curfew (5) and parched (2) break
/// the hold — a long chat must not leave both of them in the street at the
/// Snuffing, *exactly the person the watch stops* (`04_the_round.md` §6). Even
/// then the body is not marched off mid-sentence: the first pressing decision
/// is converted into a `system:` percept on the standard self-correction
/// channel, so the LLM gets one turn to excuse itself; the next one walks them
/// regardless of what it said.
fn run_ladder(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    clock: &WorldClock,
    now: f64,
    in_conversation: &BTreeSet<ActorId>,
    nudges: &mut Vec<ActorId>,
) {
    let time = clock.at(now);
    let ids: Vec<ActorId> = round.people.keys().cloned().collect();
    for id in ids {
        let held = in_conversation.contains(&id);
        let (phase, epoch, next_decision, excused) = {
            let person = &round.people[&id];
            (person.phase, person.epoch, person.next_decision, person.excused)
        };
        // The exchange has lapsed: the next pressing rung under a *new* hold is
        // owed a fresh excuse-yourself turn.
        if !held && excused {
            round.people.get_mut(&id).expect("person exists").excused = false;
        }
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        if now < next_decision {
            continue;
        }
        match phase {
            // Standing at an anchor: wait out any leftover walk slice, then decide.
            Phase::Idle if character.is_walking() => continue,
            Phase::Idle => {}
            // Mid-journey: re-decide, so curfew, a new leg or parched thirst can
            // preempt the walk (see the doc comment above).
            Phase::Travelling => {}
            // The well errand's phases stay committed until it resolves.
            _ => continue,
        }

        let (decision, pressure) =
            decide(round, world, nav, &id, epoch, time.office, time.weekday);

        // A pressing rung preempts a live `go_to` errand, and the lapse is a
        // percept: silent abandonment would leave the mind believing it is
        // still headed somewhere the body gave up on, the exact untruth the
        // refusal codes exist to prevent (05_the_llm_seam.md §2).
        if let Some(pressure) = pressure
            && world
                .characters
                .get(&id)
                .is_some_and(|character| character.state.intent.is_some())
        {
            let cause = if pressure == CURFEW_PRESSURE { "The curfew" } else { "Thirst" };
            let destination = {
                let intent = world.characters[&id]
                    .state
                    .intent
                    .as_ref()
                    .expect("checked above");
                match &intent.target {
                    IntentTarget::Place { name, .. } => name.clone(),
                    IntentTarget::Person {
                        actor_id: target_id, ..
                    } => identify_ids(world, &id, target_id),
                }
            };
            end_intent(
                round,
                world,
                &id,
                format!("{cause} turned you back before you reached {destination}."),
                nudges,
            );
        }

        // The hold: mid-conversation, the deferrable rungs wait. The cadence is
        // left untouched — exactly like the old player-only skip — so the first
        // tick after the lapse re-decides at once. The errand rung is exempt:
        // a `go_to` is the character's own will — the model says its goodbye in
        // the same reply — and holding it pinned every "meet me there" walker
        // for the exchange's whole 30 s memory. A fresh addressed line still
        // stops them for the answer (`interrupt_for_conversation`); the walk
        // resumes on the next cadence, and `stop {}` is how the mind chooses to
        // stay instead.
        let self_willed = matches!(decision, Decision::TravelIntent(_));
        if held && pressure.is_none() && !self_willed {
            continue;
        }
        // Urgency beats chat — but gets one turn's grace: inject the pressure
        // as a percept, and only the *next* pressing decision moves the body.
        if held && !excused
            && let Some(pressure) = pressure
        {
            {
                let person = round.people.get_mut(&id).expect("person exists");
                person.excused = true;
                person.epoch = epoch.wrapping_add(1);
                person.next_decision = now + decision_jitter(&id, person.epoch);
            }
            if let Some(character) = world.characters.get_mut(&id) {
                character.notify_percept(pressure);
            }
            continue;
        }

        // Schedule the next evaluation and advance the salt.
        {
            let person = round.people.get_mut(&id).expect("person exists");
            person.epoch = epoch.wrapping_add(1);
            person.next_decision = now + decision_jitter(&id, person.epoch);
        }

        // A traveller is only re-aimed by a genuinely different destination:
        // travelling to where we are already headed changes nothing (no re-route
        // thrash), and a wander/stay never interrupts a committed journey.
        if phase == Phase::Travelling {
            let under_way = round.people[&id].travel_target;
            let diverts = match &decision {
                Decision::Travel(target) | Decision::TravelIntent(target) => {
                    under_way != Some(*target)
                }
                Decision::WalkToLamp(index) => under_way != Some(round.lamps[*index].position),
                Decision::ApproachWell | Decision::LightLamp(_) => true,
                Decision::Wander(_) | Decision::Stay => false,
            };
            if !diverts {
                continue;
            }
        }

        apply_decision(round, world, nav, &id, decision);
    }
}

/// The outcome of one idle person's ladder pass.
#[derive(Debug)]
enum Decision {
    /// Rungs 2 & 6: set off for the assigned well.
    ApproachWell,
    /// Rungs 5 & 9: walk to a round anchor (or home, at curfew).
    Travel(Vec3),
    /// Rung 8: walk toward the `go_to` intent's target (M5) — the same walk as
    /// [`Decision::Travel`], but flagged so a `stop {}` halts exactly it.
    TravelIntent(Vec3),
    /// The lamplighter's rung (M7), walking: to the next unlit post. Lamps
    /// stand off the graph on the square's pavement, so this walk appends the
    /// final off-graph stride exactly as an intent walk does — a plain
    /// [`Decision::Travel`] would strand the taper at the nearest node.
    WalkToLamp(usize),
    /// The lamplighter's rung (M7): standing at an unlit post with the taper —
    /// light it.
    LightLamp(usize),
    /// Rungs 11 & 12: drift toward a friend, or mill about near home.
    Wander(Vec3),
    /// Stand where you are this cadence.
    Stay,
}

/// The `system:` line a pressing rung injects when it is about to break a
/// conversation hold — the nudge through the existing self-correction seam
/// (`features/movement/05_the_llm_seam.md`) that buys the LLM one turn to
/// excuse itself before the body walks.
const CURFEW_PRESSURE: &str =
    "system: night is falling and the watch clears the streets — you need to be home; excuse yourself.";
const PARCHED_PRESSURE: &str =
    "system: your thirst is pressing — excuse yourself and get to the well.";

/// The ladder pass for one idle person: the decision, plus — when it came off a
/// *pressing* rung (curfew, parched) that outranks a conversation hold — the
/// pressure line [`run_ladder`] injects before releasing the hold. `None` for
/// the deferrable rungs, and for a pressing rung already satisfied (standing at
/// home during curfew is a `Stay` that presses nobody).
fn decide(
    round: &Round,
    world: &World,
    nav: &NavData,
    id: &ActorId,
    epoch: u64,
    office: Office,
    weekday: Weekday,
) -> (Decision, Option<&'static str>) {
    let person = &round.people[id];
    let character = &world.characters[id];
    let position = character.position_m();
    let night = matches!(office, Office::Snuffing | Office::Watch);
    let water = person
        .source
        .map(|source| (character.needs().thirst, round.sources[source].queue.len()));

    // Rung 5 — curfew: at night, the housed go home (unless a night trade), even
    // parched — the well can wait until the Kindling; the watch cannot
    // (`07_milestones.md` M4: the ladder is curfew → parched → thirsty). The
    // homeless have nowhere to go and fall through — the parched rung below still
    // works for them, and the rest linger in the street, which is exactly the
    // person the watch stops (`04_the_round.md` §6).
    if night && !person.curfew_exempt
        && let Some(home) = person.home
    {
        return if position.distance(home) <= HOME_ARRIVE_RADIUS_M {
            (Decision::Stay, None)
        } else {
            (Decision::Travel(home), Some(CURFEW_PRESSURE))
        };
    }

    // Rung 2 — parched: drop everything and go to the well now. Below curfew, so
    // a housed drawer waits out the night at home and sets off at the Kindling;
    // the homeless and the night trades still draw at any hour.
    if let Some((thirst, _)) = water
        && thirst < THIRST_PARCHED
    {
        return (Decision::ApproachWell, Some(PARCHED_PRESSURE));
    }

    // Rung 6 — thirsty: the well, but only if its queue is short.
    if let Some((thirst, queue_len)) = water
        && thirst < THIRST_THIRSTY
        && queue_len < WELL_QUEUE_SHORT
    {
        return (Decision::ApproachWell, None);
    }

    // Rung 8 — the errand: an LLM-issued `go_to` sits between thirsty (6) and
    // the round (9) — it outranks the day's routine, never the body's needs,
    // and curfew preempting it is deliberate (05_the_llm_seam.md §2). Arrival,
    // expiry and the follow's per-tick tracking live in [`tick_intents`]; the
    // rung only aims the feet. Deferrable: a conversation holds it, and the
    // clock on the intent keeps running while it does.
    if let Some(intent) = &character.state.intent {
        let (target, radius) = match &intent.target {
            IntentTarget::Place { point, .. } => (*point, PLACE_ARRIVE_RADIUS_M),
            IntentTarget::Person { last_seen, .. } => (*last_seen, PERSON_ARRIVE_RADIUS_M),
        };
        if position.distance(target) > radius {
            return (Decision::TravelIntent(target), None);
        }
        return (Decision::Stay, None);
    }

    // Rung 8½ — the lamplighter's dusk beat (M7; `features/50_cool_suggestions.md`
    // #21): while unlit posts remain on this keeper's beat in the lighting
    // window, they walk nearest-next and light them one by one. Above the
    // round (the archetype's post can wait), below the needs — and
    // *deferrable*, so a conversation holds them mid-beat: delay a lamplighter
    // with talk and their whole quarter stays dark longer, which is the
    // feature. Each square's Belwyn's lamp is skipped by rote.
    if lamp_window(office)
        && round.lamp_night_day.is_some()
        // Committed post first (nearest-unlit is re-evaluated only standing at
        // a lit one — from a moving body it flips between far posts and walks
        // the night away), then the nearest unlit on the beat.
        && let Some(index) = round
            .lamp_targets
            .get(id)
            .copied()
            .filter(|&index| {
                let lamp = &round.lamps[index];
                !lamp.lit
                    && lamp.keeper.as_ref() == Some(id)
                    && round.belwyn.get(&lamp.square) != Some(&index)
            })
            .or_else(|| next_unlit_lamp(round, id, position))
    {
        let lamp = &round.lamps[index];
        if position.distance(lamp.position) <= LAMP_LIGHT_RADIUS_M {
            return (Decision::LightLamp(index), None);
        }
        return (Decision::WalkToLamp(index), None);
    }

    // Rung 9 — the round: be where the current leg says. Skipped at night for the
    // non-exempt (curfew already sent the housed home; the homeless linger rather
    // than march to a workshop at 2 a.m.).
    let leg = if night && !person.curfew_exempt {
        None
    } else {
        active_leg(&person.legs, office, weekday)
    };
    if let Some(leg) = leg {
        let radius = if leg.is_home { HOME_ARRIVE_RADIUS_M } else { ROUND_ARRIVE_RADIUS_M };
        if position.distance(leg.at) > radius {
            return (Decision::Travel(leg.at), None);
        }
    }
    // A zero leash is a pin: the anchoress is bricked into her cell
    // (`04_the_round.md` §1 — zero legs, `route: none`), and neither the social
    // pull nor the wander may take a single step. Without this, `Wander(base)`
    // would still route her to the *nearest nav node*, metres off her squint.
    if person.leash_m <= 0.0 {
        return (Decision::Stay, None);
    }

    // The leash centre is the *post* once we have reached it — not home, or the
    // social pull and the wander would march a working mason all the way back to
    // his own door. With no active leg (a homeless idler), fall back to base.
    let anchor = leg.map_or(person.base, |leg| leg.at);

    // Rung 11 — the social pull: drift toward a known, settled neighbour.
    if let Some(friend) = nearest_known_settled(world, id, position) {
        let toward = drift_target(anchor, position, friend, person.leash_m);
        if nav.is_walkable(toward.x, toward.z) {
            return (Decision::Wander(toward), None);
        }
    }
    // Rung 12 — wander: mill near the post, but not every time — people mostly
    // stand and work.
    if hash01("round_wander_gate", id, epoch) < 0.35
        && let Some(target) = wander_target(nav, id, epoch, anchor, person.leash_m)
    {
        return (Decision::Wander(target), None);
    }
    (Decision::Stay, None)
}

/// Carry out a decision: set the walk and the phase, or stand.
fn apply_decision(round: &mut Round, world: &mut World, nav: &NavData, id: &ActorId, decision: Decision) {
    match decision {
        Decision::ApproachWell => {
            let source = round.people[id].source.expect("a well decision has a source");
            let draw_point = round.sources[source].draw_point;
            let position = world.characters[id].position_m();
            match route_path(nav, id, position, draw_point) {
                Some(path) => {
                    set_route(world, id, path);
                    round.people.get_mut(id).expect("person exists").phase = Phase::Approaching;
                }
                None => {
                    // Already at the curb: join the queue now.
                    enqueue(round, source, id.clone());
                    round.people.get_mut(id).expect("person exists").phase = Phase::Queued;
                    world.characters.get_mut(id).expect("drawer exists").state.movement = None;
                }
            }
        }
        Decision::Travel(target) => {
            let position = world.characters[id].position_m();
            if let Some(path) = route_path(nav, id, position, target) {
                set_route(world, id, path);
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Travelling;
                person.travel_target = Some(target);
                person.travel_for_intent = false;
            }
        }
        Decision::TravelIntent(target) => {
            let position = world.characters[id].position_m();
            if let Some(path) = route_path_to_point(nav, id, position, target) {
                set_route(world, id, path);
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Travelling;
                person.travel_target = Some(target);
                person.travel_for_intent = true;
            }
        }
        Decision::Wander(target) => {
            let position = world.characters[id].position_m();
            if let Some(path) = route_path(nav, id, position, target) {
                set_route(world, id, path);
                // A wander is a short errand like any other: mark it Travelling so
                // resolve_arrivals clears the (now empty) `movement` on arrival,
                // rather than leaving a stale `Some(path: [])` behind.
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Travelling;
                person.travel_target = Some(target);
                person.travel_for_intent = false;
            }
        }
        Decision::WalkToLamp(index) => {
            round.lamp_targets.insert(id.clone(), index);
            let target = round.lamps[index].position;
            let position = world.characters[id].position_m();
            if let Some(path) = route_path_to_point(nav, id, position, target) {
                set_route(world, id, path);
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Travelling;
                person.travel_target = Some(target);
                person.travel_for_intent = false;
            }
        }
        Decision::LightLamp(index) => {
            // The act, exactly as a draw at the well: the state flips, and the
            // keeper *remembers his own act* (no sound, no nudge — M3's
            // pattern), so the player who stops him can ask what he is doing.
            round.lamps[index].lit = true;
            round.lamp_revision += 1;
            round.lamp_targets.remove(id);
            let square = round.lamps[index].square.clone();
            if let Some(character) = world.characters.get_mut(id) {
                character.state.movement = None;
                character.remember_percept(format!("You light the lamp at {square}."));
            }
            let person = round.people.get_mut(id).expect("person exists");
            person.phase = Phase::Idle;
            person.travel_target = None;
            person.travel_for_intent = false;
        }
        Decision::Stay => {}
    }
}

/// The nearest LLM neighbour within the social radius that this actor knows by
/// name and that has stopped moving.
fn nearest_known_settled(world: &World, id: &ActorId, position: Vec3) -> Option<Vec3> {
    let me = world.characters.get(id)?;
    for neighbour_id in world.characters_within(position, SOCIAL_PULL_RADIUS_M, Some(id)) {
        let neighbour = &world.characters[&neighbour_id];
        if neighbour.control().is_llm() && neighbour.is_settled() && me.knows().contains(&neighbour_id)
        {
            return Some(neighbour.position_m());
        }
    }
    None
}

/// A point a stride short of `friend`, clamped to the leash around `base`.
fn drift_target(base: Vec3, position: Vec3, friend: Vec3, leash_m: f64) -> Vec3 {
    let toward = friend - position;
    let length = toward.length();
    let target = if length > 1.5 {
        position + toward / length * (length - 1.0)
    } else {
        position
    };
    clamp_to_leash(base, target, leash_m)
}

/// A deterministic walkable point within `leash_m` of base, or `None` if a few
/// hashed tries all land on stone.
fn wander_target(nav: &NavData, id: &ActorId, epoch: u64, base: Vec3, leash_m: f64) -> Option<Vec3> {
    for attempt in 0..4 {
        let angle = hash01("round_wander_angle", id, epoch ^ (attempt as u64)) * std::f64::consts::TAU;
        let radius = hash01("round_wander_radius", id, epoch.wrapping_add(attempt as u64)) * leash_m;
        let target = Vec3::new(base.x + angle.cos() * radius, WALK_Y, base.z + angle.sin() * radius);
        if nav.is_walkable(target.x, target.z) {
            return Some(target);
        }
    }
    None
}

/// Pull `target` back to the edge of the leash circle around `base` if it strays.
fn clamp_to_leash(base: Vec3, target: Vec3, leash_m: f64) -> Vec3 {
    let offset = Vec3::new(target.x - base.x, 0.0, target.z - base.z);
    let length = offset.length();
    if length <= leash_m {
        Vec3::new(target.x, WALK_Y, target.z)
    } else {
        let edge = offset / length * leash_m;
        Vec3::new(base.x + edge.x, WALK_Y, base.z + edge.z)
    }
}

/// Route `from` → `to`, shifted into the walker's stable lane (M7 — the offset
/// keeps to their right of the crown, budgeted by the traversed corridor), and
/// trim the leading node when it is where we already stand. `None` means
/// already there or no route.
fn route_path(nav: &NavData, id: &ActorId, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
    let route = nav.route_between(from, to)?;
    let trim = route
        .points
        .first()
        .is_some_and(|point| planar_close(*point, from));
    let mut path = nav.offset_route(&route, lane_fraction(id));
    if trim {
        path.remove(0);
    }
    if path.is_empty() { None } else { Some(path) }
}

/// [`route_path`], plus the final off-graph stride: the street graph ends at a
/// node, but a followed person — or the spot they were last seen at — usually
/// stands a few metres off it, and a follow that stalls on the nearest node
/// never closes to conversation distance. The target itself is appended when
/// it is walkable ground; an unwalkable target keeps the node-only path and
/// the intent's expiry ends the errand honestly. `None` still means "already
/// there or unreachable".
fn route_path_to_point(nav: &NavData, id: &ActorId, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
    let to = Vec3::new(to.x, WALK_Y, to.z);
    let mut path = route_path(nav, id, from, to).unwrap_or_default();
    let tail_missing = !path.last().is_some_and(|point| planar_close(*point, to));
    if tail_missing && !planar_close(from, to) && nav.is_walkable(to.x, to.z) {
        path.push(to);
    }
    if path.is_empty() { None } else { Some(path) }
}

/// Give a mover a fresh walk with no patrol, keeping their gait phase seamless.
fn set_route(world: &mut World, id: &ActorId, path: Vec<Vec3>) {
    let Some(character) = world.characters.get_mut(id) else {
        return;
    };
    let gait_phase = character
        .state
        .movement
        .as_ref()
        .map_or(0.0, |movement| movement.gait_phase);
    character.state.movement = Some(Movement {
        path,
        speed: WALK_SPEED_MPS,
        gait_phase,
        patrol: None,
        choke_wait: 0.0,
    });
}

/// A jittered 1–6 s cadence (real seconds), staggered across the cast by the
/// per-actor hash so the ladder needs no scheduler.
fn decision_jitter(id: &ActorId, epoch: u64) -> f64 {
    LADDER_DECISION_MIN_SECONDS
        + (LADDER_DECISION_MAX_SECONDS - LADDER_DECISION_MIN_SECONDS)
            * hash01("round_decision", id, epoch)
}

#[cfg(test)]
mod tests;
