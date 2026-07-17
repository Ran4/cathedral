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
    EAT_SECONDS, FOOD_QUEUE_SHORT, HEARING_RADIUS_M, HEARTH_REFILL_PER_GAME_SECOND,
    HUNGER_DECAY_PER_GAME_SECOND, HUNGER_FAMISHED, HUNGER_HUNGRY, HUNGER_MAX,
    HUNGER_SEED_DECLARED_HUNGRY, HUNGER_SEED_FLOOR, LADDER_DECISION_MAX_SECONDS,
    LADDER_DECISION_MIN_SECONDS, PERSON_ARRIVE_RADIUS_M, PLACE_ARRIVE_RADIUS_M, PURCHASE_SECONDS,
    SOCIAL_PULL_RADIUS_M, STALL_ARRIVE_RADIUS_M, STALL_PITCH_REACH_M, STALL_SEEK_RADIUS_M,
    THIRST_MAX, THIRST_PARCHED, THIRST_THIRSTY, WALK_SPEED_MPS, WALLET_SEED_MIN, WALLET_SEED_SALT,
    WALLET_SEED_SPREAD, WATER_DRAW_SECONDS, WELL_ARRIVE_RADIUS_M,
    WELL_KEEPER_SOUND_INTERVAL_SECONDS, WELL_QUEUE_SHORT,
    character::{Character, IntentTarget, Movement, VendorListing},
    clock::{Office, WorldClock, Weekday},
    event::DomainEvent,
    homes::{HOMES_JSON, HomesDoc},
    ids::{ActorId, ItemId, PlaceId},
    item::Item,
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
/// The `--trace-food` log is bounded here: the game host never drains it, so it
/// caps rather than grows (the oldest lines drop). A market morning of sales
/// coalesces on the sheet, but every line is kept in the log for the headless
/// tracer, so this is generous.
const FOOD_LOG_CAP: usize = 4096;
/// A tavern's hearth reaches this far from its node (food & items M2, §4). Looser
/// than the home hearth ([`CENSUS_HOME_RADIUS_M`]) because a tavern is a wide
/// floor a worker mills across (the `tavern` archetype leash is 8 m), not a
/// doorstep — matched to [`CENSUS_POST_RADIUS_M`], the same "at this post" reach.
const TAVERN_HEARTH_RADIUS_M: f64 = CENSUS_POST_RADIUS_M;

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

/// The food-stall content (food & items M3, `04_the_bread_round.md`): the seven
/// pitches, their trades, the Kindling restock templates and the vendor float.
/// Embedded like [`ROUNDS_JSON`].
const FOOD_JSON: &str = include_str!("../../../assets/world/food.json");

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
/// The city's taverns, by nav-graph display name — the hearths of the evening
/// trade (`03_hunger.md` §4). An actor within [`TAVERN_HEARTH_RADIUS_M`] of one
/// during a meal office is fed exactly as one at home: the `tavern` archetype's
/// legs (`rounds.json`) keep its workers at the workplace right through the
/// evening, and the hearth makes those legs *feed*. These are the tavern
/// archetype's work nodes — resolved to points from the nav graph at seed, never
/// hardcoded coordinates. (The Wickmarket, where an entertainer may also perform,
/// is a market square, not a hearth, and is deliberately not one of them.)
const TAVERNS: &[&str] = &["The Hungry Ox", "The Bellstand"];
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

// --------------------------------------------------------------------------- //
// The food-stall content, straight off `food.json`
// --------------------------------------------------------------------------- //
#[derive(Debug, Deserialize)]
struct FoodDoc {
    /// The float a vendor's wallet resets to each morning — change for a market
    /// day ([`04_the_bread_round.md`] §3).
    vendor_float_sparks: u32,
    /// Trade → its eligible occupations and the morning stock template.
    trades: HashMap<String, TradeSpec>,
    stalls: Vec<StallSpec>,
}

#[derive(Debug, Deserialize)]
struct TradeSpec {
    /// The occupations that may keep a stall of this trade (`04` §2).
    occupations: Vec<String>,
    /// The template conjured onto the vendor each Kindling — empty for the pot,
    /// whose bowl is conjured per serving instead.
    #[serde(default)]
    stock: Vec<StockSpec>,
    /// The kind conjured fresh at every sale (the never-scraped pot's `stew`),
    /// in place of a depleting stock stack.
    #[serde(default)]
    conjure_per_serving: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StockSpec {
    kind: String,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
struct StallSpec {
    name: String,
    /// The square or tavern the stall belongs to, resolved against the nav graph
    /// by display name exactly as a work leg's anchor is.
    site: String,
    /// A key into [`FoodDoc::trades`].
    trade: String,
    /// A stride off the site node so two stalls at one square do not overlap —
    /// small enough that the vendor the round delivers to the square still
    /// keeps the pitch ([`STALL_PITCH_REACH_M`]).
    #[serde(default)]
    pitch_offset: [f64; 2],
    /// The authored keeper this stall prefers, bound ahead of nearest-by-base
    /// when they are in the cast and routed here (`04` §2: Renna Tapster at the
    /// Ladle, Bertran of the Ox at the Hungry Ox).
    #[serde(default)]
    preferred_vendor: Option<String>,
    open: OpenSpec,
}

/// A stall's open hours: a predicate on the clock, not state (`04` §1). A stall
/// outside these offices — or a bound vendor away from the pitch — is closed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct OpenSpec {
    /// The offices during which the stall trades.
    offices: Vec<Office>,
    /// The weekdays it trades, or `None` for every day (a market-square stall
    /// trades during market hours any day, busiest on its market day).
    #[serde(default)]
    weekdays: Option<Vec<Weekday>>,
}

impl OpenSpec {
    /// Whether the stall trades at this office on this weekday.
    fn is_open(&self, office: Office, weekday: Weekday) -> bool {
        self.offices.contains(&office)
            && self.weekdays.as_ref().is_none_or(|days| days.contains(&weekday))
    }

    /// Whether the stall trades at all today (any office), so a vendor is only
    /// bound to a stall whose day it is.
    fn open_today(&self, weekday: Weekday) -> bool {
        self.weekdays.as_ref().is_none_or(|days| days.contains(&weekday))
    }
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

/// One food stall (food & items M3, `04_the_bread_round.md`): a trade pitched at
/// a square or tavern, the vendor bound to it for the day, and the FIFO queue of
/// buyers waiting to buy. The loaves-in-it twin of [`WaterSource`] — plus the one
/// thing water never needed, stock that runs out and the confessed magic that
/// refills it.
#[derive(Debug, Clone, PartialEq)]
pub struct FoodStall {
    /// The stall's name, for the trace and the census.
    pub name: String,
    /// The site's nav display name — the square or tavern — for binding and the
    /// census (`"The Wickmarket"`).
    pub site: String,
    /// The walkable point the queue forms at and the `coin_clink` comes from.
    pub pitch: Vec3,
    /// A key into [`Round::food_trades`] (`"bread"`, `"pot"`, …).
    trade: String,
    /// The townsperson keeping the stall today (bound each morning to the
    /// nearest routed keeper), or `None` for a stall nobody staffs — which then
    /// sells nothing, exactly as an unkept well draws no water.
    pub vendor: Option<ActorId>,
    /// Ordered turns: the front buys next. A plain FIFO — markets have no vessel
    /// classes (`04` §4).
    pub queue: Vec<ActorId>,
    /// The buyer at the head and the real-clock time their purchase resolves.
    serving: Option<(ActorId, f64)>,
    /// The stock ids conjured onto the vendor this morning, so leftover stock is
    /// removed cleanly at the next restock and the nightly ledger.
    stock_ids: Vec<ItemId>,
    /// The authored keeper bound ahead of nearest-by-base, if in the cast (`04`
    /// §2). `None` for the market stalls, which have no authored keeper.
    preferred: Option<ActorId>,
    /// The open-hours predicate on the clock.
    open: OpenSpec,
    /// Next real-clock time this stall cries its wares while open and staffed.
    cry_next: f64,
}

/// A trade resolved from `food.json` for the stalls to share: who may keep it,
/// its morning stock, and the pot's per-serving conjure.
#[derive(Debug, Clone, PartialEq)]
struct ResolvedTrade {
    occupations: Vec<String>,
    stock: Vec<StockSpec>,
    per_serving: Option<String>,
}

/// A buyer's errand at a food stall (M3), parallel to the water `source`/`Phase`
/// machinery: a townsperson can be walking to a stall, standing in its queue, or
/// eating at the pitch while the rest of their round waits.
#[derive(Debug, Clone, PartialEq)]
struct FoodErrand {
    /// Index into [`Round::stalls`].
    stall: usize,
    phase: FoodPhase,
}

#[derive(Debug, Clone, PartialEq)]
enum FoodPhase {
    /// Walking to the stall's pitch.
    Approaching,
    /// Standing in the queue.
    Queued,
    /// Standing at the pitch eating what was bought — the bought item and the
    /// real-clock time the meal ends (satiety applies then, not at the buy).
    Eating { item: ItemId, until: f64 },
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
    /// A live errand at a food stall (M3): walking to buy, queued, or eating at
    /// the pitch. Independent of the water `phase` — everyone can buy food, so
    /// this is not gated on an occupation the way `source` is.
    food: Option<FoodErrand>,
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
    /// The taverns' hearths (food & items M2), resolved from the nav graph at
    /// seed: an actor within [`TAVERN_HEARTH_RADIUS_M`] of one during a meal
    /// office is fed like one at home (`03_hunger.md` §4). Empty without a nav
    /// graph, so a world with no round feeds nobody and the fixtures stay inert.
    taverns: Vec<Vec3>,
    /// The food stalls (food & items M3, `04_the_bread_round.md`): a source with
    /// a keeper, a FIFO queue, a timed service, a need snapped full — plus stock
    /// that runs out. Empty without a nav graph, so the frozen fixtures stay
    /// inert exactly as the water sources do.
    stalls: Vec<FoodStall>,
    /// The trades the stalls share, resolved from `food.json` once at seed.
    food_trades: BTreeMap<String, ResolvedTrade>,
    /// The float a vendor's wallet resets to each morning.
    vendor_float: u32,
    /// Each enrolled townsperson's seeded wallet level in sparks, so the nightly
    /// Watch ledger can refill a buyer to exactly what they started with — Ilse's
    /// authored single spark included, which a recompute would overwrite with the
    /// 2–7 spread (`02_the_spark_standard.md` §4).
    seed_wallets: BTreeMap<ActorId, u32>,
    /// A bounded log of the morning's restock, sales and the nightly ledger,
    /// drained by the host for `--trace-food`. The game host never reads it, so
    /// it is capped ([`FOOD_LOG_CAP`]) and never grows without bound.
    food_log: Vec<String>,
    /// Real `now` at the last office-crossing check, so the Kindling restock and
    /// the Watch ledger each fire exactly once per crossing, no matter the debug
    /// time-scale (`WorldClock::offices_crossed`).
    last_office_now: f64,
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

    /// A one-line census of the cast's hunger for `--trace-food` (M2): how many
    /// are fed, hungry or famished, the mean gauge, and the coin in circulation.
    /// The morning climb and the High Wick collapse (the hearth feeding the
    /// dinner legs) read straight off the hungry/famished counts; the spark
    /// total holds steady, since nothing spends a wallet until M3.
    pub fn food_summary(&self, world: &World) -> String {
        let total = self.people.len();
        let mut fed = 0usize;
        let mut hungry = 0usize;
        let mut famished = 0usize;
        let mut sum = 0.0;
        for id in self.people.keys() {
            let Some(character) = world.characters.get(id) else {
                continue;
            };
            let hunger = character.needs().hunger;
            sum += hunger;
            if hunger < HUNGER_FAMISHED {
                famished += 1;
            } else if hunger < HUNGER_HUNGRY {
                hungry += 1;
            } else {
                fed += 1;
            }
        }
        let mean = if total > 0 { sum / total as f64 } else { 0.0 };
        let sparks: u32 = self
            .people
            .keys()
            .filter_map(|id| world.characters.get(id))
            .flat_map(|character| character.holds())
            .filter_map(|item_id| world.items.get(item_id))
            .filter(|item| item.kind.as_str() == "spark")
            .map(|item| item.quantity)
            .sum();
        // The stall side of the census (M3): how many stalls are staffed, how
        // many buyers are queued or being served, and the stock still on the
        // vendors' boards — so a market morning reads as stock dwindling and
        // coin gathering, the twin of the water round's `queued/drawing`.
        let time = world.current_time;
        let staffed = self
            .stalls
            .iter()
            .filter(|stall| {
                stall.vendor.as_ref().is_some_and(|v| {
                    world
                        .characters
                        .get(v)
                        .is_some_and(|c| c.position_m().distance(stall.pitch) <= STALL_PITCH_REACH_M)
                }) && time.is_some_and(|t| stall.open.is_open(t.office, t.weekday))
            })
            .count();
        let queued: usize = self.stalls.iter().map(|stall| stall.queue.len()).sum();
        let serving = self.stalls.iter().filter(|stall| stall.serving.is_some()).count();
        let stock: u32 = self
            .stalls
            .iter()
            .flat_map(|stall| stall.stock_ids.iter())
            .filter_map(|id| world.items.get(id))
            .map(|item| item.quantity)
            .sum();
        format!(
            "food: {total} enrolled | fed {fed}, hungry {hungry}, famished {famished} | mean {mean:.0} | {sparks} sparks held | \
             stalls {}/{} open, queued {queued}, serving {serving}, stock {stock}",
            staffed,
            self.stalls.len(),
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

        // Hunger and wallets, seeded together for every enrolled townsperson
        // (`03_hunger.md` §1, `02_the_spark_standard.md` §4). Both are a spread
        // off the deterministic-hash idiom, so the city neither starves nor goes
        // broke in lockstep — and both are inert without a nav graph, since a
        // world with no round enrols nobody, keeping the frozen fixtures stable.
        // This is exactly the `townsfolk` set the enrolment loop below inserts
        // into `people`, and `decay_needs` decays hunger for every one of them —
        // so nobody is seeded a hunger they never work off (`03_hunger.md` §7:
        // the un-enrolled stay full because they are never seeded low here at
        // all, seeding running only inside a nav-graph `seed`).
        for id in &townsfolk {
            // Whether the sheet declares this actor hungry — as an authored
            // `hungry` condition, or in a memory ("I am very hungry after the
            // long road here"). The condition case is the general future-proof
            // hook; the memory case carries Ilse, whose static `hungry`
            // condition M2 drops to stop it double-printing with the computed
            // one (`03_hunger.md` §6) — the memory stays and is now truthful.
            let character = &world.characters[id];
            let declares_hungry = character
                .lore()
                .is_some_and(|lore| lore.conditions.iter().any(|c| c == "hungry"))
                || character.memories().iter().any(|memory| memory_declares_hunger(memory));
            let hunger = if declares_hungry {
                // Hungry now, famished within the hour — Ilse's story made
                // mechanical, and a reliable famished demo for M4.
                HUNGER_SEED_DECLARED_HUNGRY
            } else {
                (HUNGER_MAX * hash01("hunger_seed", id, 0)).max(HUNGER_SEED_FLOOR)
            };
            world
                .characters
                .get_mut(id)
                .expect("townsperson exists")
                .state
                .needs
                .hunger = hunger;

            // A starting purse of `WALLET_SEED_MIN + floor(WALLET_SEED_SPREAD *
            // hash01)` sparks (2..=7) — unless a lore sheet already handed this
            // actor coin, which stands (Ilse keeps her authored single spark,
            // her reluctance to spend it being her character). The wallet is an
            // ordinary `spark` stack in `world.items`, so it is visible in
            // `you_hold`, offerable, and needs no special money concept.
            let holds_spark = world.characters[id].holds().iter().any(|item_id| {
                world
                    .items
                    .get(item_id)
                    .is_some_and(|item| item.kind.as_str() == "spark")
            });
            if !holds_spark {
                let sparks =
                    WALLET_SEED_MIN + (WALLET_SEED_SPREAD as f64 * hash01(WALLET_SEED_SALT, id, 0)) as u32;
                let wallet_id = ItemId::from_raw(format!("w_{}", id.as_str()));
                world.add_item(Item::stack(wallet_id.clone(), "spark", sparks));
                world
                    .characters
                    .get_mut(id)
                    .expect("townsperson exists")
                    .state
                    .holds
                    .push(wallet_id);
                self.seed_wallets.insert(id.clone(), sparks);
            } else {
                // An authored purse stands; record its level so the nightly Watch
                // ledger refills to it, not to the 2–7 spread (Ilse's one spark).
                self.seed_wallets.insert(id.clone(), wallet_sparks(world, id));
            }
        }

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

        // The taverns' hearths (food & items M2, §4): the evening trade eats
        // where it works, so an actor at a tavern during a meal office is fed
        // like one at home. Positions come from the nav graph exactly as the
        // work legs' do (`resolver.resolve`); the names are authored content, the
        // twin of `SOURCES`/`LAMP_SQUARES`. A tavern the graph lacks simply feeds
        // nobody, one diagnostic line, no panic.
        for &name in TAVERNS {
            match resolver.resolve(name) {
                Some(point) => self.taverns.push(point),
                None => diagnostics.push(format!(
                    "[smart actors] round: tavern {name:?} is missing from the graph; its hearth is cold"
                )),
            }
        }

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
                let keeper_state = &mut world.characters.get_mut(id).expect("keeper exists").state;
                keeper_state.places_known = known;
                keeper_state.daily_round = legs.iter().map(leg_line).collect();
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
                        food: None,
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

            let townsperson_state =
                &mut world.characters.get_mut(id).expect("townsperson exists").state;
            townsperson_state.places_known = known;
            townsperson_state.daily_round = legs.iter().map(leg_line).collect();
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
                    food: None,
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

        // The food stalls (M3): resolve the pitches against the same nav graph,
        // then bind today's vendors and lay in the morning's stock so the market
        // is trading from the first tick — the seed's day may not be a market
        // day, in which case the square stalls simply bind nobody and wait for
        // their weekday. Runs last: it reads the enrolled `people`'s legs to bind.
        self.last_office_now = now;
        self.seed_food(nav, &resolver, &mut diagnostics);
        let time = clock.at(now);
        self.bind_vendors(world, time.weekday);
        self.restock(world, time.day);
        if !self.stalls.is_empty() {
            let bound = self.stalls.iter().filter(|stall| stall.vendor.is_some()).count();
            diagnostics.push(format!(
                "[smart actors] round: {} food stalls, {bound} staffed on day {} ({})",
                self.stalls.len(),
                time.day,
                time.weekday.label(),
            ));
        }
        diagnostics
    }
}

// --------------------------------------------------------------------------- //
// The food stalls (M3): binding, the Kindling restock, the Watch ledger, the
// FIFO queue and the silent purchase (`04_the_bread_round.md`).
// --------------------------------------------------------------------------- //
impl Round {
    /// The food stalls, for the census and tests.
    pub fn stalls(&self) -> &[FoodStall] {
        &self.stalls
    }

    /// The current queue at a named stall, front first — for tests/tracing.
    pub fn stall_queue(&self, name: &str) -> Option<&[ActorId]> {
        self.stalls
            .iter()
            .find(|stall| stall.name == name)
            .map(|stall| stall.queue.as_slice())
    }

    /// Drain the `--trace-food` log the round has buffered since the last poll:
    /// the Kindling restock, each sale, the Watch ledger. The game host simply
    /// never calls this, so the buffer stays capped and never grows.
    pub fn drain_food_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.food_log)
    }

    /// Append one `[food]` line, dropping the oldest if the cap is reached (the
    /// game host never drains it).
    fn push_food_log(&mut self, line: String) {
        self.food_log.push(line);
        if self.food_log.len() > FOOD_LOG_CAP {
            let overflow = self.food_log.len() - FOOD_LOG_CAP;
            self.food_log.drain(..overflow);
        }
    }

    /// Resolve the stall pitches against the nav graph (like the taverns and the
    /// lamp squares), offset a stride off the site node so two stalls at one
    /// square do not overlap. A stall whose site or trade does not resolve is
    /// skipped with a diagnostic; the rest come alive.
    fn seed_food(&mut self, nav: &NavData, resolver: &PlaceResolver, diagnostics: &mut Vec<String>) {
        let doc: FoodDoc = match serde_json::from_str(FOOD_JSON) {
            Ok(doc) => doc,
            Err(error) => {
                diagnostics.push(format!("[smart actors] round: food.json did not load: {error}"));
                return;
            }
        };
        self.vendor_float = doc.vendor_float_sparks;
        for (name, spec) in doc.trades {
            self.food_trades.insert(
                name,
                ResolvedTrade {
                    occupations: spec.occupations,
                    stock: spec.stock,
                    per_serving: spec.conjure_per_serving,
                },
            );
        }
        for spec in &doc.stalls {
            if !self.food_trades.contains_key(&spec.trade) {
                diagnostics.push(format!(
                    "[smart actors] round: stall {:?} names unknown trade {:?}; skipped",
                    spec.name, spec.trade
                ));
                continue;
            }
            let Some(site) = resolver.resolve(&spec.site) else {
                diagnostics.push(format!(
                    "[smart actors] round: stall {:?} site {:?} is missing from the graph; skipped",
                    spec.name, spec.site
                ));
                continue;
            };
            let offset = Vec3::new(site.x + spec.pitch_offset[0], WALK_Y, site.z + spec.pitch_offset[1]);
            // The offset must stand on pavement, or the queue forms on stone and
            // nobody can reach it — fall back to the site node itself.
            let pitch = if nav.is_walkable(offset.x, offset.z) { offset } else { site };
            self.stalls.push(FoodStall {
                name: spec.name.clone(),
                site: spec.site.clone(),
                pitch,
                trade: spec.trade.clone(),
                vendor: None,
                queue: Vec::new(),
                serving: None,
                stock_ids: Vec::new(),
                preferred: spec.preferred_vendor.as_ref().map(|id| ActorId::from_raw(id.as_str())),
                open: spec.open.clone(),
                cry_next: 0.0,
            });
        }
    }

    /// Bind each stall open today to the nearest routed keeper — the nearest
    /// unbound townsperson whose occupation keeps this trade and whose round
    /// already delivers them to the stall's site (a leg labelled it). Rebinding
    /// a stall to a new vendor releases whoever was queued at the old one.
    fn bind_vendors(&mut self, world: &mut World, weekday: Weekday) {
        // Two-phase, so `you_sell` bookkeeping keys off the *settled* set of
        // vendors rather than a per-stall interleave. If clear-old and write-new
        // ran per stall in index order, a vendor reassigned from a higher-index
        // stall to a lower-index one in the same pass (a fish trader moved from
        // Maren's Green to Coswald's on a Highmarket day) would be bound and
        // written at the low index, then wiped when the high stall cleared its
        // now-stale previous vendor — leaving an actively-bound vendor with no
        // price list.

        // Phase 1: pick every stall's new vendor (respecting `taken`) before
        // mutating any stall, and remember who kept each stall going in.
        let previous: Vec<Option<ActorId>> =
            self.stalls.iter().map(|stall| stall.vendor.clone()).collect();
        let mut taken: BTreeSet<ActorId> = BTreeSet::new();
        let mut chosen: Vec<Option<ActorId>> = Vec::with_capacity(self.stalls.len());
        for s in 0..self.stalls.len() {
            let new = if self.stalls[s].open.open_today(weekday) {
                self.pick_vendor(world, s, &taken)
            } else {
                None
            };
            if let Some(vendor) = &new {
                taken.insert(vendor.clone());
            }
            chosen.push(new);
        }

        // Phase 2: apply the bindings — release the buyers at any stall whose
        // vendor changed, then install the new vendor.
        for s in 0..self.stalls.len() {
            if chosen[s] != self.stalls[s].vendor {
                self.release_stall(world, s);
                self.stalls[s].vendor = chosen[s].clone();
            }
        }

        // Phase 3: `you_sell` — clear it for anyone who was a vendor and now keeps
        // NO stall, then (re)write it for every current vendor. Computed only once
        // the whole set has settled, so a reassigned vendor is never wiped by the
        // stall they left. Priced off the catalog's trade template, not current
        // stock, so a sold-out baker still knows what they charge.
        let current: BTreeSet<ActorId> =
            self.stalls.iter().filter_map(|stall| stall.vendor.clone()).collect();
        for old in previous.into_iter().flatten() {
            if !current.contains(&old)
                && let Some(character) = world.characters.get_mut(&old)
            {
                character.state.you_sell.clear();
            }
        }
        for s in 0..self.stalls.len() {
            if let Some(vendor) = self.stalls[s].vendor.clone() {
                let listings = self.sell_listings(world, s);
                if let Some(character) = world.characters.get_mut(&vendor) {
                    character.state.you_sell = listings;
                }
            }
        }
    }

    /// The `you_sell` price list for stall `s`: each kind in the trade's stock
    /// template (or the pot's per-serving bowl), named and priced from the item
    /// catalog in template order (`05_the_llm_seam.md` §3). A kind the catalog
    /// does not price is skipped rather than invented.
    fn sell_listings(&self, world: &World, s: usize) -> Vec<VendorListing> {
        let Some(trade) = self.food_trades.get(&self.stalls[s].trade) else {
            return Vec::new();
        };
        let probe = |kind: &str, metadata: &BTreeMap<String, String>| -> Option<VendorListing> {
            let mut item = Item::new(ItemId::from_raw("sell_probe"), kind);
            item.metadata = metadata.clone();
            world.item_catalog.price_sparks(&item).map(|price_sparks| VendorListing {
                name: world.item_catalog.display_name(&item),
                price_sparks,
            })
        };
        let mut listings: Vec<VendorListing> = trade
            .stock
            .iter()
            .filter_map(|spec| probe(&spec.kind, &spec.metadata))
            .collect();
        // The never-scraped pot sells a bowl it conjures per serving; it belongs
        // on the sheet exactly like a stock kind so the cook quotes the list too.
        if let Some(kind) = &trade.per_serving
            && let Some(listing) = probe(kind, &BTreeMap::new())
        {
            listings.push(listing);
        }
        listings
    }

    /// The keeper for stall `s`: the authored preferred keeper first (Renna at
    /// the Ladle, Bertran at the Ox, `04` §2), else the nearest unbound, eligible
    /// townsperson the round routes here. `None` if the trade has no keeper routed
    /// here today.
    fn pick_vendor(&self, world: &World, s: usize, taken: &BTreeSet<ActorId>) -> Option<ActorId> {
        let stall = &self.stalls[s];
        let trade = self.food_trades.get(&stall.trade)?;
        // Whether a townsperson may keep this stall: an unbound person whose
        // occupation keeps the trade and whose round already delivers them to the
        // site (a leg labelled it) — independent of the hour, so the Kindling
        // restock can hand them their board before they have walked in.
        let eligible = |id: &ActorId| -> bool {
            if taken.contains(id) {
                return false;
            }
            let Some(person) = self.people.get(id) else { return false };
            let occupation = world
                .characters
                .get(id)
                .and_then(|character| character.lore())
                .and_then(|lore| lore.occupation_id.as_deref());
            occupation.is_some_and(|occupation| trade.occupations.iter().any(|o| o == occupation))
                && person.legs.iter().any(|leg| leg.label == stall.site)
        };

        // The authored keeper first, when they are in the cast and routed here.
        if let Some(preferred) = &stall.preferred
            && eligible(preferred)
        {
            return Some(preferred.clone());
        }

        // Else the nearest by base, deterministic tie-break by id.
        let mut best: Option<(f64, ActorId)> = None;
        for id in self.people.keys() {
            if !eligible(id) {
                continue;
            }
            let distance = self.people[id].base.distance(stall.pitch);
            let better = best
                .as_ref()
                .is_none_or(|(best_dist, best_id)| distance < *best_dist || (distance == *best_dist && id < best_id));
            if better {
                best = Some((distance, id.clone()));
            }
        }
        best.map(|(_, id)| id)
    }

    /// Release the buyers queued (or being served) at stall `s`: their errand is
    /// cleared and their walk halted, handing them back to the ladder. Eaters at
    /// the pitch have already left the queue and keep their meal.
    fn release_stall(&mut self, world: &mut World, s: usize) {
        let queued: Vec<ActorId> = self.stalls[s].queue.clone();
        for id in queued {
            if let Some(person) = self.people.get_mut(&id)
                && person
                    .food
                    .as_ref()
                    .is_some_and(|errand| errand.stall == s && !matches!(errand.phase, FoodPhase::Eating { .. }))
            {
                person.food = None;
                if let Some(character) = world.characters.get_mut(&id) {
                    character.state.movement = None;
                }
            }
        }
        self.stalls[s].queue.clear();
        self.stalls[s].serving = None;
    }

    /// The Kindling restock (`04` §3): sweep each vendor's leftover stock and
    /// conjure the trade's morning template onto them, with deterministic ids per
    /// `(vendor, day, slot)`. The pot conjures nothing here — its bowl is made
    /// per serving. Vendor wallets are *not* touched (the ledger does that, at
    /// the Watch), so the only spark mint/burn is the nightly ledger and
    /// purchases conserve the total in between.
    fn restock(&mut self, world: &mut World, day: i64) {
        if self.stalls.is_empty() {
            return;
        }
        let mut lines: Vec<String> = Vec::new();
        for s in 0..self.stalls.len() {
            self.clear_stock(world, s);
            let Some(vendor) = self.stalls[s].vendor.clone() else { continue };
            let trade_key = self.stalls[s].trade.clone();
            let trade = self.food_trades[&trade_key].clone();
            if trade.stock.is_empty() {
                lines.push(format!("{} ({vendor}): the pot", self.stalls[s].name));
                continue;
            }
            let mut new_ids: Vec<ItemId> = Vec::new();
            let mut counts: Vec<String> = Vec::new();
            for (slot, spec) in trade.stock.iter().enumerate() {
                let id = ItemId::from_raw(format!("fs_{}_{day}_{slot}", vendor.as_str()));
                let mut item = Item::stack(id.clone(), spec.kind.as_str(), spec.quantity);
                for (key, value) in &spec.metadata {
                    item.metadata.insert(key.clone(), value.clone());
                }
                counts.push(format!("{}× {}", spec.quantity, world.item_catalog.display_plural(&item)));
                world.add_item(item);
                world
                    .characters
                    .get_mut(&vendor)
                    .expect("bound vendor exists")
                    .state
                    .holds
                    .push(id.clone());
                new_ids.push(id);
            }
            self.stalls[s].stock_ids = new_ids;
            lines.push(format!("{} ({vendor}): {}", self.stalls[s].name, counts.join(", ")));
        }
        world.assert_invariants();
        self.push_food_log(format!("Kindling restock, day {day} — {}", lines.join(" | ")));
    }

    /// The Watch ledger (`02_the_spark_standard.md` §4): buyer wallets refill to
    /// their seeded level, today's vendors reset to the float, and all unsold
    /// stock is swept ("spent it on flour and rent"). The one confessed
    /// non-conservative beat — between two of them, purchases move sparks around
    /// but never mint or burn.
    fn close_books(&mut self, world: &mut World) {
        if self.stalls.is_empty() {
            return;
        }
        let vendors: BTreeSet<ActorId> = self.stalls.iter().filter_map(|stall| stall.vendor.clone()).collect();
        for s in 0..self.stalls.len() {
            self.clear_stock(world, s);
        }
        let ids: Vec<ActorId> = self.people.keys().cloned().collect();
        for id in &ids {
            let target = if vendors.contains(id) {
                self.vendor_float
            } else {
                self.seed_wallets.get(id).copied().unwrap_or(0)
            };
            set_wallet(world, id, target);
        }
        world.assert_invariants();
        self.push_food_log(format!(
            "Watch ledger — {} buyers refilled to seed, {} vendors floated to {}, stock swept",
            ids.len().saturating_sub(vendors.len()),
            vendors.len(),
            self.vendor_float,
        ));
    }

    /// Remove stall `s`'s conjured stock from the world and whoever holds it,
    /// then forget the ids. Idempotent — a stack already sold to a buyer (a fresh
    /// id) is untouched; only the vendor's own leftover board is swept.
    fn clear_stock(&mut self, world: &mut World, s: usize) {
        let leftover: Vec<ItemId> = std::mem::take(&mut self.stalls[s].stock_ids);
        for id in leftover {
            prune_offer_on_removal(world, &id);
            if world.items.remove(&id).is_some() {
                for character in world.characters.values_mut() {
                    character.state.holds.retain(|held| held != &id);
                }
            }
        }
    }
}

/// The seed of a walker's spark holding, summed across every spark stack they
/// hold (there is only ever one, by the merge invariant).
fn wallet_sparks(world: &World, id: &ActorId) -> u32 {
    world
        .characters
        .get(id)
        .map(|character| character.holds())
        .unwrap_or(&[])
        .iter()
        .filter_map(|item_id| world.items.get(item_id))
        .filter(|item| item.kind.as_str() == "spark")
        .map(|item| item.quantity)
        .sum()
}

/// The spark stack ids `id` holds (usually one).
fn spark_stack_ids(world: &World, id: &ActorId) -> Vec<ItemId> {
    world
        .characters
        .get(id)
        .map(|character| character.holds().to_vec())
        .unwrap_or_default()
        .into_iter()
        .filter(|item_id| world.items.get(item_id).is_some_and(|item| item.kind.as_str() == "spark"))
        .collect()
}

/// Take `amount` sparks off `id`, removing the stack at zero. Returns whether
/// they could pay — the whole debit is atomic (checked before any mutation).
fn debit_sparks(world: &mut World, id: &ActorId, amount: u32) -> bool {
    if amount == 0 {
        return true;
    }
    let stacks = spark_stack_ids(world, id);
    let total: u32 = stacks.iter().filter_map(|sid| world.items.get(sid)).map(|item| item.quantity).sum();
    if total < amount {
        return false;
    }
    let mut remaining = amount;
    for sid in stacks {
        if remaining == 0 {
            break;
        }
        let held = world.items.get(&sid).map(|item| item.quantity).unwrap_or(0);
        let take = held.min(remaining);
        remaining -= take;
        if take == held {
            prune_offer_on_removal(world, &sid);
            world.items.remove(&sid);
            if let Some(character) = world.characters.get_mut(id) {
                character.state.holds.retain(|item_id| item_id != &sid);
            }
        } else {
            world.items.get_mut(&sid).expect("stack exists").quantity -= take;
        }
    }
    true
}

/// Add `amount` sparks to `id`, folding into their existing purse or minting a
/// `w_<id>` wallet if they hold none.
fn credit_sparks(world: &mut World, id: &ActorId, amount: u32) {
    if amount == 0 {
        return;
    }
    if let Some(sid) = spark_stack_ids(world, id).first() {
        world.items.get_mut(sid).expect("stack exists").quantity += amount;
        return;
    }
    let wallet_id = ItemId::from_raw(format!("w_{}", id.as_str()));
    if world.items.contains_key(&wallet_id) {
        // A stray unheld wallet id: fold into it rather than dup-panic.
        world.items.get_mut(&wallet_id).expect("present").quantity += amount;
    } else {
        world.items.insert(wallet_id.clone(), Item::stack(wallet_id.clone(), "spark", amount));
    }
    if let Some(character) = world.characters.get_mut(id)
        && !character.holds().contains(&wallet_id)
    {
        character.state.holds.push(wallet_id);
    }
}

/// Set `id`'s spark holding to exactly `amount` (the nightly ledger): collapse
/// onto their first spark stack, drop any spare, and create a purse if they hold
/// none and `amount > 0`.
fn set_wallet(world: &mut World, id: &ActorId, amount: u32) {
    let stacks = spark_stack_ids(world, id);
    if amount == 0 {
        for sid in stacks {
            prune_offer_on_removal(world, &sid);
            world.items.remove(&sid);
            if let Some(character) = world.characters.get_mut(id) {
                character.state.holds.retain(|item_id| item_id != &sid);
            }
        }
        return;
    }
    if let Some(first) = stacks.first().cloned() {
        world.items.get_mut(&first).expect("stack exists").quantity = amount;
        for spare in stacks.into_iter().skip(1) {
            prune_offer_on_removal(world, &spare);
            world.items.remove(&spare);
            if let Some(character) = world.characters.get_mut(id) {
                character.state.holds.retain(|item_id| item_id != &spare);
            }
        }
    } else {
        credit_sparks(world, id, amount);
    }
}

/// `"s"` unless `n == 1` — the naive pluralizer, for the sparks in a percept.
fn spark_plural(n: u32) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Drop any live offer of a stack about to be **silently removed** under the
/// ladder — a sold-out board stack, an emptied purse, a swept wallet — with the
/// same cleanup `eat` does when its last unit goes (`actions.rs:1037`): no
/// `retract_offer` event (the HUD learns from the snapshot), and the offer's
/// target is told it lapsed if they are a nearby hearer. Without this a stack an
/// LLM had offered ("one stock, two doors") is removed while `world.offers` still
/// names it, and the next `assert_invariants` panics ("offer giver does not hold
/// item"). Only *removal* needs this — a stack that merely **shrinks** below an
/// offer's quantity is caught gracefully at accept time. Call BEFORE the item
/// leaves `world.items`, so the withdrawn noun and the giver's position are still
/// there to read.
fn prune_offer_on_removal(world: &mut World, item_id: &ItemId) {
    let Some(offer) = world.offers.remove(item_id) else {
        return;
    };
    let Some(target_id) = offer.target_id else {
        return; // a broadcast offer notifies nobody in particular
    };
    if !world.characters.contains_key(&target_id) {
        return;
    }
    let within_earshot = world
        .characters
        .get(&offer.giver_id)
        .map(Character::position_m)
        .zip(world.characters.get(&target_id).map(Character::position_m))
        .is_some_and(|(giver, target)| giver.distance(target) <= HEARING_RADIUS_M);
    if !within_earshot {
        return; // a distant target reads the removal off the snapshot, as `eat` does
    }
    let Some(noun) = world.items.get(item_id).map(|item| world.item_catalog.display_name(item)) else {
        return;
    };
    let withdrawer = crate::cap_first(&identify_ids(world, &target_id, &offer.giver_id));
    world
        .characters
        .get_mut(&target_id)
        .expect("target exists")
        .notify(format!("{withdrawer} withdrew the offered {noun} (id {item_id})"));
}

/// Eat one unit of a held food item **without** the bystander inbox lines the
/// `eat` verb delivers to nearby NPCs — the market's zero-token discipline
/// (`04` §5, `05_the_llm_seam.md` §4). A code-driven meal (eat-at-pitch, or a
/// famished actor eating what they hold) fires far too often to nudge a reaction
/// turn per bite, so it follows the well-draw pattern: the eater *remembers their
/// own meal* (askable), the stack decrements with the same offer cleanup and
/// satiety as the verb, and the player sees the item vanish from the snapshot — but
/// no `X ate a herring` lands in a neighbour's inbox. A **deliberate** `eat` turn
/// (an LLM or the player choosing it) still carries its terse bystander line
/// through the real verb; only the ladder's auto-eat is silenced. Returns whether
/// a unit was actually eaten (a benign miss if the item left the hand meanwhile).
fn silent_eat(world: &mut World, eater: &ActorId, item_id: &ItemId) -> bool {
    if !world.characters.get(eater).is_some_and(|character| character.holds().contains(item_id)) {
        return false;
    }
    let Some(item) = world.items.get(item_id).cloned() else {
        return false;
    };
    let Some(satiety) = world.item_catalog.satiety(&item) else {
        return false; // not food (a race, or a bad decision) — eat nothing
    };
    let noun = world.item_catalog.display_name(&item);
    if item.quantity <= 1 {
        prune_offer_on_removal(world, item_id);
        world.items.remove(item_id);
        if let Some(character) = world.characters.get_mut(eater) {
            character.state.holds.retain(|held| held != item_id);
        }
    } else {
        world.items.get_mut(item_id).expect("the eater holds the stack").quantity -= 1;
    }
    if let Some(character) = world.characters.get_mut(eater) {
        let hunger = &mut character.state.needs.hunger;
        *hunger = (*hunger + f64::from(satiety)).min(HUNGER_MAX);
        character.remember_percept(format!("You ate a {noun}."));
    }
    world.touch_public_state();
    true
}

/// A resolved sale, for the trace line and the eat-or-carry decision.
struct Sale {
    stall_name: String,
    item_display: String,
    price: u32,
    stock_left: u32,
    bought_id: ItemId,
    pitch: Vec3,
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
    tick_food_ledger(round, world, clock, now);
    decay_needs(round, world, clock, now);
    tick_lamps(round, clock, now);
    resolve_arrivals(round, world);
    resolve_food_arrivals(round, world);
    tick_intents(round, world, nav, now, &mut nudges);
    service_sources(round, world, nav, clock, now, player_id, in_conversation);
    service_stalls(round, world, clock, now, player_id);
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

/// Whether an office is a meal office — the hours the hearth feeds
/// (`03_hunger.md` §4): dinner at High Wick, and the supper span from the Waning
/// through Lamplight.
fn is_meal_office(office: Office) -> bool {
    matches!(office, Office::HighWick | Office::Waning | Office::Lamplight)
}

/// The needs fall by the game clock, so they keep pace with the sun and the
/// debug time-scale. **Thirst** decays only for the bound water-drawers;
/// **hunger** decays for *every* enrolled townsperson — everyone eats
/// (`03_hunger.md` §1, README §8.1) — and climbs back at the hearth for those
/// the round has home during a meal office (§4: no items, no coins).
fn decay_needs(round: &mut Round, world: &mut World, clock: &WorldClock, now: f64) {
    let game_days = clock.game_days(now);
    let delta_days = (game_days - round.last_game_days).max(0.0);
    round.last_game_days = game_days;
    let game_seconds = delta_days * 86_400.0;
    if game_seconds <= 0.0 {
        return;
    }
    let thirst_drop = game_seconds * crate::THIRST_DECAY_PER_GAME_SECOND;
    let hunger_drop = game_seconds * HUNGER_DECAY_PER_GAME_SECOND;
    let hearth_gain = game_seconds * HEARTH_REFILL_PER_GAME_SECOND;
    // Read the office once for the whole cast — the hearth feeds only at a meal
    // office (the round's `home`/`sleep` legs then become supper).
    let meal_office = is_meal_office(clock.at(now).office);
    for (id, person) in &round.people {
        let Some(character) = world.characters.get_mut(id) else {
            continue;
        };
        if person.draws_water() {
            let thirst = &mut character.state.needs.thirst;
            *thirst = (*thirst - thirst_drop).max(0.0);
        }
        // Physical presence at a hearth is the test, so a straggler the famished
        // rung sent home is fed exactly like a diner the round did — computed
        // before the mutable borrow of the gauge below. A hearth is the actor's
        // own home *or any tavern* (`03_hunger.md` §4): the ~39 tavern trades
        // (brewer/cook/tavern_worker/entertainer/sex_worker) work straight
        // through the meal offices and are never home, so without the tavern
        // branch they would decay to nothing forever.
        let position = character.position_m();
        let at_hearth = meal_office
            && (person
                .home
                .is_some_and(|home| position.distance(home) <= CENSUS_HOME_RADIUS_M)
                || round
                    .taverns
                    .iter()
                    .any(|tavern| position.distance(*tavern) <= TAVERN_HEARTH_RADIUS_M)
                // A stationary actor never walks to a hearth — the anchoress is
                // bricked into her cell (`stationary` archetype: zero legs), with
                // no homes.json house and no tavern — so where she stands *is*
                // her hearth, or she decays to nothing forever (`03_hunger.md`
                // §4). General to any legless enrolled actor: her base (her spawn,
                // since a legless actor is never housed here) feeds her.
                || (person.legs.is_empty()
                    && position.distance(person.base) <= CENSUS_HOME_RADIUS_M));
        let hunger = &mut character.state.needs.hunger;
        *hunger = (*hunger - hunger_drop).max(0.0);
        if at_hearth {
            *hunger = (*hunger + hearth_gain).min(HUNGER_MAX);
        }
    }
}

/// The food ledger (M3): fire the Kindling restock and the Watch reset exactly
/// once per office crossing since the last poll, no matter the debug time-scale
/// (`WorldClock::offices_crossed`, the same span the bells ride). At the Watch
/// the books close (wallets reset, stock swept); at the Kindling today's vendors
/// are bound and the morning stock is conjured onto them.
fn tick_food_ledger(round: &mut Round, world: &mut World, clock: &WorldClock, now: f64) {
    if round.stalls.is_empty() {
        round.last_office_now = now;
        return;
    }
    let crossings = clock.offices_crossed(round.last_office_now, now);
    round.last_office_now = now;
    for (instant, office) in crossings {
        let day = clock.at(instant).day;
        match office {
            Office::Watch => round.close_books(world),
            Office::Kindling => {
                round.bind_vendors(world, clock.at(instant).weekday);
                round.restock(world, day);
            }
            _ => {}
        }
    }
}

/// A food buyer who reached the pitch joins its queue; one who stopped short
/// abandons the errand and is handed back to the ladder. Runs after the water
/// [`resolve_arrivals`], which has already dropped a finished round walk to Idle.
fn resolve_food_arrivals(round: &mut Round, world: &mut World) {
    if round.stalls.is_empty() {
        return;
    }
    let ids: Vec<ActorId> = round.people.keys().cloned().collect();
    for id in ids {
        let Some(errand) = round.people[&id].food.clone() else {
            continue;
        };
        if !matches!(errand.phase, FoodPhase::Approaching) {
            continue;
        }
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        if character.is_walking() {
            continue; // still on the way
        }
        let pitch = round.stalls[errand.stall].pitch;
        if character.position_m().distance(pitch) <= STALL_ARRIVE_RADIUS_M {
            if !round.stalls[errand.stall].queue.contains(&id) {
                round.stalls[errand.stall].queue.push(id.clone());
            }
            round.people.get_mut(&id).expect("person exists").food =
                Some(FoodErrand { stall: errand.stall, phase: FoodPhase::Queued });
            world.characters.get_mut(&id).expect("buyer exists").state.movement = None;
        } else {
            // Stopped short (a conversation interrupt, a re-route that failed):
            // drop the errand and let the ladder re-decide next cadence.
            round.people.get_mut(&id).expect("person exists").food = None;
        }
    }
}

/// Work each stall: close what is shut (releasing its queue), finish a completed
/// sale, start the next, resolve the atomic purchase, run the eat-at-pitch timer,
/// and clink a coin for the player. The market twin of [`service_sources`] — the
/// sale is a silent, self-perceived act, and the only sound is a player-only
/// world sound, so thirty sales an hour never schedule an LLM turn (`04` §5).
fn service_stalls(round: &mut Round, world: &mut World, clock: &WorldClock, now: f64, player_id: &ActorId) {
    if round.stalls.is_empty() {
        return;
    }
    let time = clock.at(now);

    // Open = the hours predicate holds *and* the bound vendor is at the pitch —
    // the seller the round delivered to the square (no pin). Computed first, so
    // the mutable pass below is free of the world borrow.
    let open: Vec<bool> = round
        .stalls
        .iter()
        .map(|stall| {
            stall.open.is_open(time.office, time.weekday)
                && stall.vendor.as_ref().is_some_and(|vendor| {
                    world
                        .characters
                        .get(vendor)
                        .is_some_and(|character| character.position_m().distance(stall.pitch) <= STALL_PITCH_REACH_M)
                })
        })
        .collect();

    let mut finished: Vec<(usize, ActorId)> = Vec::new();
    let mut to_release: Vec<usize> = Vec::new();
    // (sound_id, position) player-only world sounds to emit this tick: a cry per
    // open stall on its slow rhythm, plus a clink per sale (pushed below).
    let mut sounds: Vec<(&'static str, Vec3)> = Vec::new();
    for s in 0..round.stalls.len() {
        if !open[s] {
            if !round.stalls[s].queue.is_empty() || round.stalls[s].serving.is_some() {
                to_release.push(s);
            }
            continue;
        }
        // Cry the wares on the slow rhythm — a square sounds like a market before
        // it looks like one (the flourish, `04` §5).
        if now >= round.stalls[s].cry_next {
            round.stalls[s].cry_next = now + crate::MARKET_CRY_INTERVAL_SECONDS;
            sounds.push(("market_cry", round.stalls[s].pitch));
        }
        // Finish a completed sale: the buyer leaves the head of the queue.
        if let Some((buyer, ends_at)) = round.stalls[s].serving.clone()
            && now >= ends_at
        {
            round.stalls[s].serving = None;
            if round.stalls[s].queue.first() == Some(&buyer) {
                round.stalls[s].queue.remove(0);
            } else {
                round.stalls[s].queue.retain(|id| id != &buyer);
            }
            finished.push((s, buyer));
        }
        // Start the next: the front buys.
        if round.stalls[s].serving.is_none()
            && let Some(front) = round.stalls[s].queue.first().cloned()
        {
            round.stalls[s].serving = Some((front, now + PURCHASE_SECONDS));
        }
    }
    for s in to_release {
        round.release_stall(world, s);
    }

    // Resolve each completed sale, then send the buyer to eat at the pitch or
    // carry it home, and clink a coin the player can hear.
    for (s, buyer) in finished {
        match try_purchase(round, world, s, &buyer) {
            Some(sale) => {
                sounds.push(("coin_clink", sale.pitch));
                let carry = should_carry(round, &buyer, time.office, time.weekday);
                if carry {
                    round.people.get_mut(&buyer).expect("buyer exists").food = None;
                } else {
                    round.people.get_mut(&buyer).expect("buyer exists").food = Some(FoodErrand {
                        stall: s,
                        phase: FoodPhase::Eating { item: sale.bought_id, until: now + EAT_SECONDS },
                    });
                }
                if let Some(character) = world.characters.get_mut(&buyer) {
                    character.state.movement = None;
                }
                world.touch_public_state();
                world.assert_invariants();
                round.push_food_log(format!(
                    "sale: {buyer} bought a {} for {} spark{} at {} — {} left{}",
                    sale.item_display,
                    sale.price,
                    spark_plural(sale.price),
                    sale.stall_name,
                    sale.stock_left,
                    if carry { ", carried home" } else { "" },
                ));
            }
            None => {
                // Could not pay (spent it mid-queue) or nothing affordable is
                // left: a graceful no-sale, the buyer leaves and re-evaluates.
                round.people.get_mut(&buyer).expect("buyer exists").food = None;
                if let Some(character) = world.characters.get_mut(&buyer) {
                    character.state.movement = None;
                }
            }
        }
    }

    // The eat-at-pitch timer: when a meal ends, apply the real `eat` (the stack
    // decrement and the satiety), standing where they bought — satiety on the
    // actual eat, not the buy (`04` §5).
    let eaters: Vec<(ActorId, ItemId)> = round
        .people
        .iter()
        .filter_map(|(id, person)| match &person.food {
            Some(FoodErrand { phase: FoodPhase::Eating { item, until }, .. }) if now >= *until => {
                Some((id.clone(), item.clone()))
            }
            _ => None,
        })
        .collect();
    for (id, item) in eaters {
        // The silent, self-perceived meal — no bystander inbox lines in a busy
        // square (`04` §5). Satiety applies here, on the actual eat, not the buy.
        silent_eat(world, &id, &item);
        round.people.get_mut(&id).expect("eater exists").food = None;
    }

    // The player-audible market: the cry and the clink are unattributed world
    // sounds at the pitch, heard only by the player, so they reach no NPC inbox
    // and never nudge a reaction turn (the windlass pattern).
    if !sounds.is_empty() && world.sounds_enabled {
        let player_pos = world.characters.get(player_id).map(Character::position_m);
        for (sound_id, position) in sounds {
            let Some(sound) = world.sound_catalog.get(sound_id).cloned() else {
                continue;
            };
            let recipients = match player_pos {
                Some(pos) if pos.distance(position) <= sound.audible_distance => vec![player_id.clone()],
                _ => Vec::new(),
            };
            world.emit(DomainEvent::sound(
                sound.sound_class.clone(),
                None, // a world sound, never attributed, never nudges
                sound.sound_id.clone(),
                sound.audible_distance,
                position,
                recipients,
                Vec::new(),
            ));
        }
    }
}

/// The atomic swap at the head of a stall's queue (`04` §5): the buyer's coin and
/// the vendor's food move together, or not at all. Priced off the catalog, the
/// famished buyer takes the cheapest edible stock they can afford (a herring if a
/// spark is all they hold — Ilse's exact arithmetic). Returns `None` for a
/// no-sale (nothing affordable, or the vendor's board is bare).
fn try_purchase(round: &mut Round, world: &mut World, s: usize, buyer: &ActorId) -> Option<Sale> {
    let vendor = round.stalls[s].vendor.clone()?;
    let trade_key = round.stalls[s].trade.clone();
    let trade = round.food_trades.get(&trade_key)?.clone();
    let buyer_sparks = wallet_sparks(world, buyer);

    // Choose what to sell: the pot conjures a fresh bowl; a stock stall sells the
    // cheapest affordable edible on the board.
    let (template, price, from_stock): (Item, u32, Option<ItemId>) = if let Some(kind) = &trade.per_serving {
        let bowl = Item::new(ItemId::from_raw("stew_probe"), kind.as_str());
        let price = world.item_catalog.price_sparks(&bowl)?;
        if buyer_sparks < price {
            return None;
        }
        (bowl, price, None)
    } else {
        let mut best: Option<(u32, ItemId, Item)> = None;
        for stock_id in &round.stalls[s].stock_ids {
            let Some(item) = world.items.get(stock_id) else { continue };
            if item.quantity == 0 || !world.item_catalog.is_edible(item) {
                continue;
            }
            let Some(item_price) = world.item_catalog.price_sparks(item) else { continue };
            if item_price > buyer_sparks {
                continue;
            }
            let better = best
                .as_ref()
                .is_none_or(|(best_price, best_id, _)| item_price < *best_price || (item_price == *best_price && stock_id < best_id));
            if better {
                best = Some((item_price, stock_id.clone(), item.clone()));
            }
        }
        let (price, stock_id, item) = best?;
        (item, price, Some(stock_id))
    };

    // The swap, atomic: pay first (checked affordable above, so it cannot fail),
    // then move one unit. Sparks are conserved — the debug assert is the standing
    // guard the risk ledger asks for.
    let before = wallet_sparks(world, buyer) + wallet_sparks(world, &vendor);
    if !debit_sparks(world, buyer, price) {
        return None;
    }
    credit_sparks(world, &vendor, price);
    debug_assert_eq!(
        wallet_sparks(world, buyer) + wallet_sparks(world, &vendor),
        before,
        "a purchase must conserve sparks"
    );
    let bought_id = give_food_unit(world, &vendor, buyer, &template, from_stock.as_ref());

    let stock_left: u32 = round.stalls[s]
        .stock_ids
        .iter()
        .filter_map(|id| world.items.get(id))
        .map(|item| item.quantity)
        .sum();
    let item_display = world.item_catalog.display_name(&world.items[&bought_id]);
    let stall_name = round.stalls[s].name.clone();
    let pitch = round.stalls[s].pitch;

    // Self-percepts, both parties — the act is askable for zero tokens, and the
    // consecutive-repeat dedup collapses a busy morning into one line each.
    let vendor_name = identify_ids(world, buyer, &vendor);
    if let Some(character) = world.characters.get_mut(buyer) {
        character.remember_percept(format!(
            "You bought a {item_display} from {vendor_name} for {price} spark{}.",
            spark_plural(price)
        ));
    }
    if let Some(character) = world.characters.get_mut(&vendor) {
        character.remember_percept(format!(
            "You sold a {item_display} for {price} spark{}.",
            spark_plural(price)
        ));
    }

    Some(Sale { stall_name, item_display, price, stock_left, bought_id, pitch })
}

/// Move one unit of `template` from the vendor to the buyer: decrement the stock
/// stack (removing it at zero), then fold the unit into the buyer's same-stuff
/// stack or mint it a fresh, deterministic id. Returns the id the buyer now holds.
fn give_food_unit(
    world: &mut World,
    vendor: &ActorId,
    buyer: &ActorId,
    template: &Item,
    from_stock: Option<&ItemId>,
) -> ItemId {
    if let Some(stock_id) = from_stock {
        let held = world.items.get(stock_id).map(|item| item.quantity).unwrap_or(0);
        if held <= 1 {
            prune_offer_on_removal(world, stock_id);
            world.items.remove(stock_id);
            if let Some(character) = world.characters.get_mut(vendor) {
                character.state.holds.retain(|item_id| item_id != stock_id);
            }
        } else {
            world.items.get_mut(stock_id).expect("stock stack exists").quantity -= 1;
        }
    }

    let mut unit = template.clone();
    unit.quantity = 1;
    let stackable = world.item_catalog.stackable(&unit);
    let merge_target: Option<ItemId> = if stackable {
        world.characters[buyer]
            .holds()
            .iter()
            .find(|held| world.items.get(*held).is_some_and(|other| other.same_stuff_as(&unit)))
            .cloned()
    } else {
        None
    };
    if let Some(target) = merge_target {
        world.items.get_mut(&target).expect("buyer stack exists").quantity += 1;
        return target;
    }

    // A fresh, deterministic id — parent is the stock stack, or a per-vendor
    // handle for the pot's bowl; the event-sequence salt keeps runs reproducible.
    let parent = from_stock
        .cloned()
        .unwrap_or_else(|| ItemId::from_raw(format!("pot_{}", vendor.as_str())));
    let mut salt = world.event_sequence + 1;
    let mut id = crate::world::mint_item_id(&parent, salt);
    while world.items.contains_key(&id) {
        salt = salt.wrapping_add(1);
        id = crate::world::mint_item_id(&parent, salt);
    }
    unit.id = id.clone();
    world.add_item(unit);
    if let Some(character) = world.characters.get_mut(buyer) {
        character.state.holds.push(id.clone());
    }
    id
}

/// Whether a fed buyer carries their food home rather than eating at the pitch:
/// only when their round is **actually taking them home** — their active leg is a
/// home leg — within the supper span (the Waning through Lamplight), so rung 3's
/// eat-what-you-hold finishes the meal at the hearth (`04` §5). Otherwise they eat
/// where they bought, in view. The active-leg test is what stops a buyer who is
/// still at their market post from pocketing the loaf, hearth-refilling for free,
/// and hoarding food the famished rung would never make them eat.
fn should_carry(round: &Round, buyer: &ActorId, office: Office, weekday: Weekday) -> bool {
    if !matches!(office, Office::Waning | Office::Lamplight) {
        return false;
    }
    let Some(person) = round.people.get(buyer) else {
        return false;
    };
    active_leg(&person.legs, office, weekday).is_some_and(|leg| leg.is_home)
}

/// The nearest open, staffed, affordable stall within [`STALL_SEEK_RADIUS_M`] of
/// `position` — rung 3 (famished) and rung 7 (hungry, `short` too) join it. `None`
/// keeps a famished worker at their post (or the hearth) rather than marching a
/// kilometre for a loaf.
fn nearest_open_stall(
    round: &Round,
    world: &World,
    id: &ActorId,
    position: Vec3,
    office: Office,
    weekday: Weekday,
    short: bool,
) -> Option<usize> {
    // A bound vendor is keeping their post, not shopping: they never queue (least
    // of all at their own stall, which would sell to themselves and double-count
    // the board). They are fed at the hearth like the rest of the evening trade.
    if round.stalls.iter().any(|stall| stall.vendor.as_ref() == Some(id)) {
        return None;
    }
    let sparks = wallet_sparks(world, id);
    let mut best: Option<(f64, usize)> = None;
    for (s, stall) in round.stalls.iter().enumerate() {
        if !stall.open.is_open(office, weekday) {
            continue;
        }
        let staffed = stall.vendor.as_ref().is_some_and(|vendor| {
            world
                .characters
                .get(vendor)
                .is_some_and(|character| character.position_m().distance(stall.pitch) <= STALL_PITCH_REACH_M)
        });
        if !staffed {
            continue;
        }
        if short && stall.queue.len() >= FOOD_QUEUE_SHORT {
            continue;
        }
        let distance = position.distance(stall.pitch);
        if distance > STALL_SEEK_RADIUS_M {
            continue;
        }
        if !stall_has_affordable(round, world, stall, sparks) {
            continue;
        }
        if best.as_ref().is_none_or(|(best_dist, _)| distance < *best_dist) {
            best = Some((distance, s));
        }
    }
    best.map(|(_, s)| s)
}

/// Whether the stall has something the buyer can pay for — the pot's bowl, or an
/// edible on the board within their purse.
fn stall_has_affordable(round: &Round, world: &World, stall: &FoodStall, sparks: u32) -> bool {
    let Some(trade) = round.food_trades.get(&stall.trade) else {
        return false;
    };
    if let Some(kind) = &trade.per_serving {
        let bowl = Item::new(ItemId::from_raw("stew_probe"), kind.as_str());
        return world.item_catalog.price_sparks(&bowl).is_some_and(|price| price <= sparks);
    }
    stall.stock_ids.iter().any(|id| {
        world.items.get(id).is_some_and(|item| {
            item.quantity > 0
                && world.item_catalog.is_edible(item)
                && world.item_catalog.price_sparks(item).is_some_and(|price| price <= sparks)
        })
    })
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
        // A live food errand is committed exactly like a well errand's own phases:
        // walking to a stall, standing in its queue, or eating at the pitch, the
        // ladder does not re-decide them. Buying food *is* the hunger rung acting;
        // a closing stall releases them (`service_stalls`) back to the ladder.
        if round.people[&id].food.is_some() {
            continue;
        }
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
            let cause = if pressure == CURFEW_PRESSURE {
                "The curfew"
            } else if pressure == FAMISHED_PRESSURE {
                "Hunger"
            } else {
                "Thirst"
            };
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
                Decision::EatHeld(_)
                | Decision::ApproachWell
                | Decision::ApproachStall(_)
                | Decision::LightLamp(_) => true,
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
    /// Rung 3: a famished actor eats a food item it already holds, standing —
    /// the market is for empty hands (`03_hunger.md` §3). The stack decrement
    /// and satiety are the `eat` action's, routed through it in [`apply_decision`].
    EatHeld(ItemId),
    /// Rungs 2 & 6: set off for the assigned well.
    ApproachWell,
    /// Rungs 3 (famished, go buy) & 7 (hungry, when convenient): set off for a
    /// food stall's pitch to join its queue (the bread round, `04` §4/§5).
    ApproachStall(usize),
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
const FAMISHED_PRESSURE: &str =
    "system: you are famished; your feet are taking you to food — excuse yourself.";

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

    // Rung 3 — famished: drop everything. Eat what you hold first — the market is
    // for empty hands (`03_hunger.md` §3) — else, while the hearth is serving,
    // head home to it. The pressure percept gives an on-stage actor one turn to
    // excuse itself before the body walks (the `excused` flag), exactly as
    // parched does — and rides only with the divert, so it injects only when the
    // rung actually acts.
    if character.needs().hunger < HUNGER_FAMISHED {
        // Eat what you hold, standing — for anyone, the night trades included,
        // at any hour: a famished actor with food in hand always eats it (but a
        // vendor never eats their own stall board).
        if let Some(item_id) = held_edible(round, world, character) {
            return (Decision::EatHeld(item_id), None);
        }
        // The market first (M3, the bread round, `04_the_bread_round.md`): a
        // famished actor with empty hands buys at the nearest open, staffed,
        // affordable stall within reach. It ranks above the hearth — an open
        // market is the right place to feed — and rides the pressure percept, so
        // an on-stage actor gets one turn to excuse itself before the body walks,
        // exactly as parched does, and only when the rung actually diverts.
        if let Some(stall) = nearest_open_stall(round, world, id, position, office, weekday, false) {
            return (Decision::ApproachStall(stall), Some(FAMISHED_PRESSURE));
        }
        // Else head home to the hearth, but *only while a meal office is
        // serving* (`03_hunger.md` §3/§4). Outside one the hearth is cold, so
        // diverting there would abandon the day's work to sit at a dead grate —
        // and since the whole cast is famished by dawn (a 10 h gauge, supper at
        // 18:00), an ungated divert empties the morning city. Gated, a famished
        // worker keeps working through the morning (medieval and correct) and is
        // sent home only once High Wick opens the hearth — which, with the
        // round's own dinner leg, is what makes the noon collapse land at noon. A
        // night trade keeps its post (curfew-exempt — the lamps must be lit and
        // the wall kept), fed at the tavern instead (§4); the homeless have no
        // hearth to make for; both fall through to the lesser rungs. Night needs
        // no branch here — the curfew rung already has the non-exempt home.
        if is_meal_office(office)
            && !person.curfew_exempt
            && let Some(home) = person.home
        {
            return if position.distance(home) <= HOME_ARRIVE_RADIUS_M {
                (Decision::Stay, None)
            } else {
                (Decision::Travel(home), Some(FAMISHED_PRESSURE))
            };
        }
    }

    // Rung 6 — thirsty: the well, but only if its queue is short.
    if let Some((thirst, queue_len)) = water
        && thirst < THIRST_THIRSTY
        && queue_len < WELL_QUEUE_SHORT
    {
        return (Decision::ApproachWell, None);
    }

    // Rung 7 — hungry: seek food when convenient (M3, the bread round). Join the
    // nearest open, staffed, affordable stall whose queue is short
    // (`FOOD_QUEUE_SHORT`). Quiet, like thirsty — no pressure percept. Its place
    // in the ladder is fixed: after thirsty (6), before the `go_to` errand (8).
    if character.needs().hunger < HUNGER_HUNGRY
        && let Some(stall) = nearest_open_stall(round, world, id, position, office, weekday, true)
    {
        return (Decision::ApproachStall(stall), None);
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
        Decision::EatHeld(item_id) => {
            // A code-driven meal standing where they are (`04_the_bread_round.md`
            // §5, minus the bench), through [`silent_eat`] so the stack decrement,
            // satiety and self-percept are the verb's but no bystander inbox line
            // nudges a neighbour — the market's zero-token discipline. `held_edible`
            // already proved the hold, so a miss here is a benign race and dropped.
            silent_eat(world, id, &item_id);
        }
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
        Decision::ApproachStall(stall) => {
            let pitch = round.stalls[stall].pitch;
            let position = world.characters[id].position_m();
            // Idle + a food errand: the water phase is left standing (they draw
            // no water) and `run_ladder` skips them while the errand lives.
            {
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Idle;
                person.travel_target = None;
                person.travel_for_intent = false;
            }
            match route_path(nav, id, position, pitch) {
                Some(path) => {
                    set_route(world, id, path);
                    round.people.get_mut(id).expect("person exists").food =
                        Some(FoodErrand { stall, phase: FoodPhase::Approaching });
                }
                None => {
                    // Already at the pitch: join the queue now.
                    if !round.stalls[stall].queue.contains(id) {
                        round.stalls[stall].queue.push(id.clone());
                    }
                    round.people.get_mut(id).expect("person exists").food =
                        Some(FoodErrand { stall, phase: FoodPhase::Queued });
                    world.characters.get_mut(id).expect("buyer exists").state.movement = None;
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

/// Whether a memory line is a first-person declaration of *present* hunger — the
/// low-hunger seed hook for a character written hungry (Ilse: "I am very hungry
/// after the long road here", `03_hunger.md` §1/§6).
///
/// Deliberately narrow. A bare case-insensitive `contains("hungry")` — or even a
/// plain word-boundary match — would also fire on third-person lore like "the
/// winter everyone went hungry", silently seeding an unrelated character
/// famished. So this requires the standalone word `hungry` **and** a first-person
/// present-tense subject before it ("I am" / "I'm" / "I feel"): Ilse's memory has
/// both; that stray sentence has neither. The general, future-proof hook is the
/// authored `hungry` *condition* — this only carries Ilse, whose static condition
/// M2 drops to stop it double-printing with the computed one.
fn memory_declares_hunger(memory: &str) -> bool {
    let lower = memory.to_ascii_lowercase();
    // The standalone word `hungry`, not a substring of a larger token.
    let Some(hungry_at) = word_index(&lower, "hungry") else {
        return false;
    };
    // A first-person present-tense subject somewhere before it.
    ["i am", "i'm", "i feel"].iter().any(|lead| lower[..hungry_at].contains(lead))
}

/// The byte index of the first standalone-word occurrence of `word` in `haystack`
/// (both already lowercase), or `None`. "Standalone" = not flanked by ASCII
/// alphanumerics, so `hungry` hits "hungry." and "very hungry" but not
/// "hungrycrake"; later occurrences are tried if the first is embedded.
fn word_index(haystack: &str, word: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(word) {
        let start = from + rel;
        let end = start + word.len();
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return Some(start);
        }
        from = start + 1;
    }
    None
}

/// The id of the first real food item this character holds (positive catalog
/// satiety), or `None`. A spark is not food (its kind is un-edible); an ad-hoc
/// test kind carries no satiety and is skipped too — the famished rung only eats
/// what actually feeds. A bound vendor's **stall stock is skipped**: the board is
/// for selling, not eating — without this a famished baker eats their own bread
/// and the stock leaks a loaf nobody bought (found in the M3 bring-up).
fn held_edible(round: &Round, world: &World, character: &Character) -> Option<ItemId> {
    character
        .holds()
        .iter()
        .find(|item_id| {
            !round.stalls.iter().any(|stall| stall.stock_ids.contains(item_id))
                && world
                    .items
                    .get(*item_id)
                    .and_then(|item| world.item_catalog.satiety(item))
                    .is_some_and(|satiety| satiety > 0)
        })
        .cloned()
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

/// One leg as its own walker knows it — a sheet `your_round` line, in the same
/// register as the percepts ("at Dayspring, on Bellday only: prayers at The
/// Lanthorn"). Composed once at seed; [`CharacterState::daily_round`] holds
/// the finished lines.
///
/// [`CharacterState::daily_round`]: crate::character::CharacterState::daily_round
fn leg_line(leg: &RoundLeg) -> String {
    let doing = match (leg.doing, leg.is_home) {
        (Arrival::Sleep, true) => "home to sleep".to_string(),
        (Arrival::Idle, true) => "your ease at home".to_string(),
        (Arrival::Work, _) => format!("work at {}", leg.label),
        (Arrival::Trade, _) => format!("trade at {}", leg.label),
        (Arrival::Sleep, false) => format!("sleep at {}", leg.label),
        (Arrival::Pray, _) => format!("prayers at {}", leg.label),
        (Arrival::Idle, false) => format!("your ease at {}", leg.label),
        (Arrival::DrawWater, _) => format!("drawing water at {}", leg.label),
        (Arrival::Stand, _) => format!("standing at {}", leg.label),
    };
    let days = match &leg.only_on {
        None => String::new(),
        Some(days) => {
            let names: Vec<&str> = days.iter().map(|day| day.label()).collect();
            format!(", on {} only", names.join(" and "))
        }
    };
    format!("at {}{days}: {doing}", leg.from.label())
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
