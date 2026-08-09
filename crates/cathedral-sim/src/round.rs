//! The daily round (M4) — the non-LLM behaviour layer that gets the city up in
//! the morning, sends it to work, moves the crowd on market days, and empties the
//! streets at the Snuffing (`features/implemented/movement/04_the_round.md`,
//! `features/implemented/movement/03_the_ladder.md`). It subsumes the M3 water round: water
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

use serde::{Deserialize, Serialize};

use crate::{
    EAT_SECONDS, FOOD_QUEUE_SHORT, GO_TO_BUDGET_FACTOR, GO_TO_MIN_BUDGET_SECONDS, HEARING_RADIUS_M,
    HEARTH_REFILL_PER_GAME_SECOND, HUNGER_DECAY_PER_GAME_SECOND, HUNGER_FAMISHED, HUNGER_HUNGRY,
    HUNGER_MAX, HUNGER_SEED_DECLARED_HUNGRY, HUNGER_SEED_FLOOR, LADDER_DECISION_MAX_SECONDS,
    LADDER_DECISION_MIN_SECONDS, PERSON_ARRIVE_RADIUS_M, PLACE_ARRIVE_RADIUS_M, PURCHASE_SECONDS,
    SOCIAL_PULL_RADIUS_M, STALL_ARRIVE_RADIUS_M, STALL_PITCH_REACH_M, STALL_SEEK_RADIUS_M,
    THIRST_MAX, THIRST_PARCHED, THIRST_THIRSTY, WALK_SPEED_MPS, WALLET_SEED_MIN, WALLET_SEED_SALT,
    WALLET_SEED_SPREAD, WATER_DRAW_SECONDS, WELL_ARRIVE_RADIUS_M,
    WELL_KEEPER_SOUND_INTERVAL_SECONDS, WELL_QUEUE_SHORT,
    character::{Character, EconomicClass, IntentTarget, Movement, RoundEdit, VendorListing},
    clock::{Office, Weekday, WorldClock},
    event::DomainEvent,
    homes::{HOMES_JSON, HomesDoc},
    ids::{ActorId, ItemId, PartyId, PlaceId},
    inventory::{
        ItemMatcher, MarketRequestLine, ReservedInput, SaleReceipt, StockSpec, TransformJob,
    },
    item::Item,
    lore::Significance,
    math::Vec3,
    nav::{NavData, WALK_Y},
    perception::identify_ids,
    places::PlaceRegistry,
    weather::{LightningStrike, ShelterAccess, WeatherKind, WeatherSample},
    world::{World, hash01, lane_fraction, planar_close},
};

/// Occupations that fetch water with a **household** vessel — chiefly the
/// servant, whose day is a trip to the ward well and back (the single largest
/// occupation in the city, `features/implemented/movement/README.md` §8). A household vessel
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
/// A **generated** citizen's leash, drawn per person over this band
/// (`give_the_crowd_somewhere_to_be.md` M3) instead of the flat
/// [`DEFAULT_ROUND_LEASH_M`], which was calibrated for a handful of people per
/// anchor and is the reason a workplace with forty of them reads as a scrum.
/// A band rather than a second constant so the clump has a soft edge instead of
/// a larger hard one: some of the crowd keeps to the corner, some of it is half
/// a street away, and there is no ring where everybody stops.
///
/// Only ever written in [`Round::seed`]'s enrolment branch behind
/// `LoreProfile::generated`, so the authored cast keeps exactly the leash
/// [`build_legs`] gave it — the route's own, the archetype's, or
/// [`DEFAULT_ROUND_LEASH_M`]. (Authored leashes run 0 to 24 m in
/// `rounds.json`, so this band overlaps them: a 16 m leash is not evidence of a
/// leak, and the test that guards this pins *which rule* drew the number.)
const CROWD_LEASH_MIN_M: f64 = 15.0;
const CROWD_LEASH_MAX_M: f64 = 40.0;
/// Hashed offsets [`wander_target`] tries before a mill becomes a stand. Four
/// for the authored cast (unchanged — see that function), eight for a generated
/// citizen, whose leash is up to four times the default and whose draws
/// therefore land on stone more often. Each attempt is one bitset lookup and only the
/// *rejected* ones cost anything, so the extra four are free in every sense
/// that matters to the pump.
const WANDER_ATTEMPTS: u64 = 4;
const CROWD_WANDER_ATTEMPTS: u64 = 8;
/// Census: how close counts as "at your post" / "at home".
///
/// For a generated citizen this is a *floor*, not the reach — see
/// [`Round::census`]: their post is as wide as their own leash.
const CENSUS_POST_RADIUS_M: f64 = 9.0;
const CENSUS_HOME_RADIUS_M: f64 = 5.0;
/// The `--trace-food` log is bounded here: the game host never drains it, so it
/// caps rather than grows (the oldest lines drop). A market morning of sales
/// coalesces on the sheet, but every line is kept in the log for the headless
/// tracer, so this is generous.
const FOOD_LOG_CAP: usize = 4096;
/// How long an active transform job may sit with no work accrued at all before
/// the round abandons it and releases its reservations. A full game day is
/// deliberately generous: every legitimate pause — the night, a conversation,
/// a market errand, the office closing — resolves within one cycle of the
/// round, because the Work leg that feeds the job comes past again tomorrow.
/// A job still untouched after a whole cycle has lost its worker for good (a
/// round edit that slipped past the applier's guard, route data that died, a
/// future system nobody has written yet), and since a job has no other
/// non-completing exit, its reserved inputs would otherwise be dead stock
/// forever and the chain behind the site would starve on them.
const TRANSFORM_ABANDON_GAME_DAYS: f64 = 1.0;
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

/// The food-stall and supply-chain content embedded from `food.json`.
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

/// What one tally stroke is worth, in metres of extra walk
/// (`features/implemented/chalking_the_walls.md` M4). A busy well pushes its next drawer
/// toward the neighbour; overnight the chalk washes off and the well recovers.
const TALLY_METRES_PER_STROKE: f64 = 6.0;

/// How long a chalk-refused buyer stays away from the boards, as a fraction of
/// a game day (`features/implemented/chalking_the_walls.md` M1). Half a day: long enough
/// that one refusal is one scene rather than a loop, short enough that a cross
/// scrubbed in the morning lets them eat by evening.
const CHALK_REFUSAL_GAME_DAYS: f64 = 0.5;

/// The share of the ambient cast whose evening the nightly code roll moves to a
/// tavern hearth (movement M6). Deliberately small: the payoff is that the
/// streets are not identical two nights running, not that the taverns are full.
const AMBIENT_TAVERN_FRACTION: f64 = 0.15;
/// Posts per square, rung around the square's interior node.
const LAMPS_PER_SQUARE: usize = 4;
/// The ring the posts stand on, probed inward until each lands on pavement.
const LAMP_RING_RADIUS_M: f64 = 11.0;
/// Close enough to reach the wick with the taper.
const LAMP_LIGHT_RADIUS_M: f64 = 2.5;
/// A shelter farther away than this is not a credible reaction to a shower.
/// Route length, rather than straight-line distance, keeps the choice honest in
/// the walled city's doglegs.
const SHELTER_SEEK_RADIUS_M: f64 = 130.0;
const SHELTER_RELEASE_MINUTES: f64 = 10.0;
const SHELTER_RELEASE_SPREAD_MINUTES: f64 = 10.0;
/// A cloud-to-ground origin this close in XZ is near enough to make an exposed
/// person flinch. The bodily pause is real-time like the flash, rather than
/// stretching into minutes when the debug clock is accelerated.
const LIGHTNING_REFLEX_RADIUS_M: f64 = 180.0;
const LIGHTNING_REFLEX_MIN_SECONDS: f64 = 2.5;
const LIGHTNING_REFLEX_SPREAD_SECONDS: f64 = 1.5;

/// What an actor means to do on arrival — the pathfind-then-act bridge lifted
/// from seagame's `targetState` (`features/implemented/movement/02_navigation.md` §4).
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
    #[serde(default)]
    road_parties: Vec<RoadPartySpec>,
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

#[derive(Debug, Clone, Deserialize)]
struct LegSpec {
    from: Office,
    at: String,
    doing: Arrival,
    #[serde(default)]
    only_on: Option<Vec<Weekday>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoadPartySpec {
    id: PartyId,
    leader: ActorId,
    members: Vec<ActorId>,
    gate: String,
    only_on: Vec<Weekday>,
    stage_at: Office,
    enter_at: Office,
    return_at: Office,
    wallet_float_sparks: BTreeMap<ActorId, u32>,
    commercial_cargo: Vec<ItemMatcher>,
    manifest: Vec<StockSpec>,
    legs: Vec<LegSpec>,
}

// --------------------------------------------------------------------------- //
// The food-stall content, straight off `food.json`
// --------------------------------------------------------------------------- //
#[derive(Debug, Deserialize)]
struct FoodDoc {
    /// Trade → eligible occupations, live listings, and any explicitly
    /// unchained legacy restock template.
    trades: HashMap<String, TradeSpec>,
    stalls: Vec<StallSpec>,
    #[serde(default)]
    counters: Vec<CounterSpec>,
    #[serde(default)]
    counter_groups: Vec<CounterGroupSpec>,
    #[serde(default)]
    stock_plans: Vec<StockPlanSpec>,
    #[serde(default)]
    production_plans: Vec<ProductionPlanSpec>,
    #[serde(default)]
    working_capital: BTreeMap<ActorId, u32>,
    #[serde(default)]
    historical_stock: Vec<HistoricalStockSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterSpec {
    id: String,
    trade: String,
    site: String,
    #[serde(default)]
    anchor_actor: Option<ActorId>,
    #[serde(default)]
    pitch_offset: [f64; 2],
    preferred_actor: ActorId,
    offices: Vec<Office>,
    required_doing: Arrival,
    #[serde(default)]
    road_party: Option<PartyId>,
    #[serde(default)]
    worksite_only: bool,
    site_radius_m: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterGroupSpec {
    id: String,
    counters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StockSource {
    Counter(String),
    CounterGroup(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockPlanSpec {
    id: String,
    buyer: ActorId,
    source: StockSource,
    targets: Vec<StockTargetSpec>,
    max_spend_sparks: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockTargetSpec {
    kind: crate::item::ItemKind,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    desired_quantity: u32,
}

impl StockTargetSpec {
    fn matcher(&self) -> ItemMatcher {
        ItemMatcher {
            kind: self.kind.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductionPlanSpec {
    producer: ActorId,
    max_jobs_per_day: u32,
    transforms: Vec<TransformSpecDoc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformSpecDoc {
    id: String,
    site: String,
    #[serde(default)]
    anchor_actor: Option<ActorId>,
    consumes: Vec<StockSpec>,
    produces: Vec<StockSpec>,
    allowed_offices: Vec<Office>,
    work_minutes: u32,
    desired_output_quantity: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalStockSpec {
    owner: ActorId,
    kind: crate::item::ItemKind,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
struct TradeSpec {
    /// The occupations that may keep a stall of this trade (`04` §2).
    occupations: Vec<String>,
    /// The template conjured onto the vendor each Kindling — empty for the pot,
    /// whose bowl is conjured per serving instead.
    #[serde(default)]
    listings: Vec<ItemMatcher>,
    #[serde(default)]
    restock: Vec<StockSpec>,
    /// The kind conjured fresh at every sale (the never-scraped pot's `stew`),
    /// in place of a depleting stock stack.
    #[serde(default)]
    conjure_per_serving: Option<crate::item::ItemKind>,
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
            && self
                .weekdays
                .as_ref()
                .is_none_or(|days| days.contains(&weekday))
    }

    /// Whether the stall trades at all today (any office), so a vendor is only
    /// bound to a stall whose day it is.
    fn open_today(&self, weekday: Weekday) -> bool {
        self.weekdays
            .as_ref()
            .is_none_or(|days| days.contains(&weekday))
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
        self.site_by_key
            .get(name)
            .map(|&node| self.nav.node_point(node))
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
    /// The authored keeper bound ahead of nearest-by-base, if in the cast (`04`
    /// §2). `None` for the market stalls, which have no authored keeper.
    preferred: Option<ActorId>,
    /// The open-hours predicate on the clock.
    open: OpenSpec,
    /// Next real-clock time this stall cries its wares while open and staffed.
    cry_next: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartLoadKind {
    GrainSacks,
    WoolBales,
    ClothBolts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadCart {
    pub party_id: PartyId,
    pub leader_id: ActorId,
    pub load: Vec<CartLoadKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyPhase {
    BeyondTheWalls,
    StagedOutsideGate,
    InCity,
    Returning,
    DeparturePending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyState {
    pub phase: PartyPhase,
    pub trip_number: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct RoadParty {
    id: PartyId,
    leader: ActorId,
    members: Vec<ActorId>,
    gate: String,
    gate_point: Vec3,
    only_on: Vec<Weekday>,
    stage_at: Office,
    enter_at: Office,
    return_at: Office,
    wallet_floats: BTreeMap<ActorId, u32>,
    commercial_cargo: Vec<ItemMatcher>,
    manifest: Vec<StockSpec>,
    legs: Vec<RoundLeg>,
    state: PartyState,
    last_trigger_day: Option<i64>,
    /// Members already given the road's excuse-yourself pressure this return,
    /// mapped to the real-clock moment their one turn of grace runs out and
    /// the body may walk mid-topic — the ladder's `excused` mechanism
    /// ([`run_ladder`]) carried locally, because road members are never
    /// enrolled in `people` and so have no `excused` flag of their own.
    /// Cleared by [`Round::begin_road_return`], so every trip's return starts
    /// with the courtesy owed afresh.
    departure_excuses: BTreeMap<ActorId, f64>,
}

fn road_cart_is_visible(party: &RoadParty, world: &World) -> bool {
    matches!(
        party.state.phase,
        PartyPhase::InCity | PartyPhase::Returning | PartyPhase::DeparturePending
    ) && party.members.iter().all(|member| world.is_present(member))
}

fn road_cart_load(party: &RoadParty, world: &World) -> Vec<CartLoadKind> {
    let mut load = BTreeSet::new();
    for member in &party.members {
        let Some(actor) = world.characters.get(member) else {
            continue;
        };
        for id in actor.holds() {
            let Some(item) = world.items.get(id) else {
                continue;
            };
            if !party
                .commercial_cargo
                .iter()
                .any(|matcher| matcher.matches(item))
            {
                continue;
            }
            match item.kind.as_str() {
                "grain" => {
                    load.insert(CartLoadKind::GrainSacks);
                }
                "wool" => {
                    load.insert(CartLoadKind::WoolBales);
                }
                "cloth" => {
                    load.insert(CartLoadKind::ClothBolts);
                }
                _ => {}
            }
        }
    }
    load.into_iter().collect()
}

fn road_transition_trace(event: &str, party: &RoadParty, world: &World, day: i64) -> String {
    let members = party
        .members
        .iter()
        .map(|member| {
            let actor = &world.characters[member];
            format!(
                "{member}:{:?}@{}",
                actor.state.presence, actor.state.presence_epoch
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let cart = road_cart_is_visible(party, world).then(|| road_cart_load(party, world));
    format!(
        "{event}: party {}, day {day}, phase {:?}, trip {}, gate {}, members [{members}], cart {cart:?}",
        party.id, party.state.phase, party.state.trip_number, party.gate
    )
}

#[derive(Debug, Clone, PartialEq)]
struct Counter {
    id: String,
    trade: String,
    site: String,
    pitch: Vec3,
    seller: ActorId,
    offices: Vec<Office>,
    required_doing: Arrival,
    road_party: Option<PartyId>,
    worksite_only: bool,
    radius_m: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CounterSession {
    Daily { absolute_day: i64 },
    RoadTrip { party_id: PartyId, trip_number: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CounterBindingKey {
    pub counter_id: String,
    pub seller: ActorId,
    pub session: CounterSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketErrandPhase {
    Approaching,
    WaitingForOpen,
    AtCounter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketVisitEnd {
    TargetsSatisfied,
    BudgetExhausted,
    SourceIneligible,
    LastOfficePassed,
    NoRoute,
    TravelExpired,
    ReplacedByGoTo,
    Returning,
    UnpricedStock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketErrand {
    pub plan_id: String,
    pub selected: Option<CounterBindingKey>,
    pub bindings_seen: Vec<CounterBindingKey>,
    pub phase: MarketErrandPhase,
    pub spent_sparks: u32,
    pub last_failed_fingerprint: Option<String>,
    pub travel_deadline_real: Option<f64>,
    /// The real-clock moment a legitimate hold — a conversation, a pressing
    /// rung, the law — took the body mid-walk, or `None` while the walk is the
    /// errand's own. The travel deadline exists to catch a genuinely stuck
    /// walk, not to bill the buyer for time another system rightfully owned:
    /// the spec promises that conversation and the pressing needs *pause* the
    /// errand rather than end it (`07_the_supply_chain.md` §"never end the
    /// visit"), so on resume the deadline moves forward by exactly the span
    /// held. Without this, hunger firing mid-errand burned the deadline down
    /// and forfeited the day's whole run to `TravelExpired` — a timeout firing
    /// on state another system caused.
    pub deadline_hold_began_real: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosedMarketVisit {
    pub plan_id: String,
    pub bindings_seen: Vec<CounterBindingKey>,
    pub end_reason: MarketVisitEnd,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedProductionPlan {
    producer: ActorId,
    max_jobs_per_day: u32,
    transforms: Vec<ResolvedTransformSpec>,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedTransformSpec {
    id: String,
    site: String,
    point: Vec3,
    consumes: Vec<StockSpec>,
    produces: Vec<StockSpec>,
    allowed_offices: Vec<Office>,
    work_minutes: u32,
    desired_output_quantity: u32,
}

/// A trade resolved from `food.json` for the stalls to share: who may keep it,
/// its morning stock, and the pot's per-serving conjure.
#[derive(Debug, Clone, PartialEq)]
struct ResolvedTrade {
    occupations: Vec<String>,
    listings: Vec<ItemMatcher>,
    restock: Vec<StockSpec>,
    per_serving: Option<crate::item::ItemKind>,
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
    /// The evening leg exactly as the seed authored it, kept from the first
    /// night the ambient re-roll displaced it (movement M6). Restoring from
    /// this before each roll is what makes the roll a *nightly* choice rather
    /// than a one-way drift: a night that comes up "home" really does put them
    /// back at their own hearth. `None` for everyone the roll never touched —
    /// the whole Major and Minor cast, and the ambients whose day names no
    /// evening leg.
    evening_seed: Option<(usize, RoundLeg)>,
    /// A pressing rung (curfew, parched) came due while they were held in a
    /// conversation, and the pressure has been injected as a `system:` percept:
    /// they have had their one turn to excuse themselves, and the next pressing
    /// decision walks them regardless of what they said. Cleared once the
    /// exchange lapses.
    excused: bool,
}

/// A deterministic, resumable shelter errand.  It lives beside the ordinary
/// round instead of in an actor's LLM intent: taking cover is body policy, not
/// a provider decision.  `below_since_days` implements the 10--20 game-minute
/// release hysteresis, so a wavering front cannot make a crowd seesaw at every
/// sample.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WeatherShelterIntent {
    shelter: usize,
    target: Vec3,
    release_threshold: f64,
    below_since_days: Option<f64>,
    release_after_days: f64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdTransfer {
    pub donor: ActorId,
    pub recipient: ActorId,
    pub sparks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdRelief {
    pub actor: ActorId,
    pub before_spendable: u32,
    pub after_spendable: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdSettlementReceipt {
    pub day: i64,
    pub transfers: Vec<HouseholdTransfer>,
    pub relief: Vec<HouseholdRelief>,
    pub institutional_payroll_sparks: u32,
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

/// One producer's entry in the stranded-job watchdog
/// ([`Round::transform_stall_watch`]): the job and progress figure last
/// observed, and the game-day the figure last moved.
#[derive(Debug, Clone, PartialEq)]
struct TransformStallWatch {
    job_id: String,
    progress_work_minutes: f64,
    unchanged_since_game_days: f64,
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
    /// A bounded log of restock, sales, production, road traffic, and settlement,
    /// drained by the host for `--trace-food`. The game host never reads it, so
    /// it is capped ([`FOOD_LOG_CAP`]) and never grows without bound.
    food_log: Vec<String>,
    /// Real `now` at the last office-crossing check, so Kindling restock and
    /// Watch settlement each fire exactly once per crossing, no matter the debug
    /// time-scale (`WorldClock::offices_crossed`).
    last_office_now: f64,
    people: BTreeMap<ActorId, Townsperson>,
    /// Active deterministic weather diversions.  Kept at round level so the
    /// authored townsperson seed format and LLM-facing intent model stay clean.
    weather_shelter_intents: BTreeMap<ActorId, WeatherShelterIntent>,
    /// Real-time deadline for the short, deterministic flinch caused by a
    /// nearby strike. This remains a mechanical reaction: it never enters an
    /// inbox or spends a cognition turn.
    lightning_reflex_until: BTreeMap<ActorId, f64>,
    /// Real-clock deadline after a chalk refusal at a stall counter
    /// (`features/implemented/chalking_the_walls.md` M1), so a refused buyer stops
    /// re-queueing at the same board.
    ///
    /// Without it the loop is guaranteed, not hypothetical: the ladder's only
    /// selection filter is affordability, and a chalk-refused buyer still has
    /// the coin and the stall still has the bread — so they would rejoin the
    /// queue on every `next_decision`, forever, one inbox line a lap. Shaped
    /// like [`Self::lightning_reflex_until`] because the guard that reads it
    /// (`nearest_open_stall`) has no `now`: pruned at the top of the tick,
    /// tested with a bare `contains_key`.
    chalk_refused_until: BTreeMap<ActorId, f64>,
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
    /// Spendable working reserve protected from household redistribution.
    household_reserves: BTreeMap<ActorId, u32>,
    last_household_watch_day: Option<i64>,
    last_household_settlement_day: Option<i64>,
    unrelieved_zero_streak: BTreeMap<ActorId, u32>,
    institutional_payroll_sparks: u64,
    household_redistributed_sparks: u64,
    road_cash_in_sparks: u64,
    road_cash_out_sparks: u64,
    road_parties: BTreeMap<PartyId, RoadParty>,
    /// Last derived *presentation* loads seen by the tracer. This cache has no
    /// simulation authority: carts are still rebuilt from party inventories on
    /// every snapshot; it exists solely to emit before/after `cart_load` lines.
    observed_cart_loads: BTreeMap<PartyId, Vec<CartLoadKind>>,
    departed_this_tick: Vec<ActorId>,
    /// Named non-geometry worksites anchored to authored doors.
    worksites: BTreeMap<String, Vec3>,
    counters: BTreeMap<String, Counter>,
    counter_groups: BTreeMap<String, Vec<String>>,
    stock_plans: Vec<StockPlanSpec>,
    market_errands: BTreeMap<ActorId, MarketErrand>,
    closed_market_visits: BTreeMap<String, ClosedMarketVisit>,
    production_plans: Vec<ResolvedProductionPlan>,
    production_starts: BTreeMap<(ActorId, i64), u32>,
    production_last_game_days: f64,
    /// Physical eligibility sampled on the preceding pump for each active
    /// producer. Progress is credited only when both ends of an interval show
    /// the actor continuously available at the worksite; the office and
    /// authored Work-leg overlap inside that interval is integrated exactly.
    /// Keeping those two predicates separate lets a coarse pump just after
    /// closing retain the work completed before the bell.
    production_was_eligible: BTreeMap<ActorId, bool>,
    /// The stranded-job watchdog ([`TRANSFORM_ABANDON_GAME_DAYS`]): per
    /// producer, the active job and progress last seen and when that figure
    /// last moved. Swept every production tick over *every* live job in the
    /// world — not just the planned producers — so a job whose plan has gone
    /// is covered too, and one whose worker can never stand its site again is
    /// abandoned instead of holding its inputs committed forever.
    transform_stall_watch: BTreeMap<ActorId, TransformStallWatch>,
    /// A reused scratch buffer for [`run_ladder`]'s per-tick cast snapshot: the
    /// ids must be cloned out to iterate while the body mutates `people`, but
    /// the `Vec` itself need not be reallocated every 20 Hz tick. Always drained
    /// empty between ticks, so it never affects `Clone`/`PartialEq`.
    ladder_scratch: Vec<ActorId>,
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

    /// The social shelter currently claimed by an actor, for headless
    /// diagnostics and pure tests.  This exposes the stable data ID, never the
    /// private implementation of the ladder intent.
    pub fn weather_shelter<'a>(&self, world: &'a World, actor: &ActorId) -> Option<&'a str> {
        let intent = self.weather_shelter_intents.get(actor)?;
        world
            .shelters
            .shelters()
            .get(intent.shelter)
            .map(|shelter| shelter.id.as_str())
    }

    /// Wake the ordinary ladder for exposed actors near a lightning origin.
    /// Atomic phases and conversation holds are still enforced by
    /// [`run_ladder`]; setting `next_decision` merely lets a free walker react
    /// on the same engine pump instead of waiting for its normal cadence.
    pub fn note_lightning(&mut self, world: &World, strike: &LightningStrike, now: f64) {
        let radius_squared = LIGHTNING_REFLEX_RADIUS_M * LIGHTNING_REFLEX_RADIUS_M;
        for (id, person) in &mut self.people {
            let Some(character) = world.characters.get(id) else {
                continue;
            };
            let position = character.position_m();
            let at_home = person
                .home
                .is_some_and(|home| position.distance(home) <= HOME_ARRIVE_RADIUS_M);
            let dx = position.x - strike.origin_m[0];
            let dz = position.z - strike.origin_m[2];
            if !world.is_present(id)
                || at_home
                || world.shelters.is_sheltered(position)
                || dx * dx + dz * dz > radius_squared
            {
                continue;
            }
            let duration = LIGHTNING_REFLEX_MIN_SECONDS
                + hash01("lightning_reflex", id, strike.id) * LIGHTNING_REFLEX_SPREAD_SECONDS;
            self.lightning_reflex_until
                .entry(id.clone())
                .and_modify(|until| *until = until.max(now + duration))
                .or_insert(now + duration);
            person.next_decision = person.next_decision.min(now);
        }
    }

    /// The squares' street lamps (M7), for the engine's `Lamps` channel and tests.
    pub fn lamps(&self) -> &[Lamp] {
        &self.lamps
    }

    /// Bumped on any lamp change; the engine republishes when it moves.
    pub fn lamp_revision(&self) -> u64 {
        self.lamp_revision
    }

    pub fn last_household_watch_day(&self) -> Option<i64> {
        self.last_household_watch_day
    }

    pub fn last_household_settlement_day(&self) -> Option<i64> {
        self.last_household_settlement_day
    }

    pub fn unrelieved_zero_streak(&self, actor: &ActorId) -> u32 {
        self.unrelieved_zero_streak.get(actor).copied().unwrap_or(0)
    }

    pub fn party_state(&self, id: &PartyId) -> Option<&PartyState> {
        self.road_parties.get(id).map(|party| &party.state)
    }

    pub fn road_carts(&self, world: &World) -> Vec<RoadCart> {
        self.road_parties
            .values()
            .filter(|party| road_cart_is_visible(party, world))
            .map(|party| RoadCart {
                party_id: party.id.clone(),
                leader_id: party.leader.clone(),
                load: road_cart_load(party, world),
            })
            .collect()
    }

    fn trace_cart_load_changes(&mut self, world: &World) {
        let current = self
            .road_carts(world)
            .into_iter()
            .map(|cart| (cart.party_id, cart.load))
            .collect::<BTreeMap<_, _>>();
        let ids = self
            .observed_cart_loads
            .keys()
            .chain(current.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for id in ids {
            let before = self
                .observed_cart_loads
                .get(&id)
                .cloned()
                .unwrap_or_default();
            let after = current.get(&id).cloned().unwrap_or_default();
            if before != after
                || self.observed_cart_loads.contains_key(&id) != current.contains_key(&id)
            {
                self.push_food_log(format!(
                    "cart_load: party {id}, before {before:?}, after {after:?}"
                ));
            }
        }
        self.observed_cart_loads = current;
    }

    pub fn drain_departed(&mut self) -> Vec<ActorId> {
        std::mem::take(&mut self.departed_this_tick)
    }

    /// Whether this actor keeps a round at all — the Night Office asks before
    /// it spends a provider call on somebody whose day it could not change.
    pub fn is_enrolled(&self, actor: &ActorId) -> bool {
        self.people.contains_key(actor)
    }

    /// The ambient cast's Night Office (movement M6, `05_the_llm_seam.md` §4):
    /// **no provider call at all**. Roughly 350 people, one deterministic roll
    /// each off their id and the day, and about a seventh of them take
    /// tomorrow's evening at a tavern hearth instead of at their own.
    ///
    /// The evening leg is the one the roll may move — for every shipped
    /// archetype that is the `lamplight` leg, which is also their bed. Moving
    /// it keeps nobody out all night: the curfew rung still walks the housed
    /// home at the Snuffing, so what actually changes is the three hours
    /// between the lamps being lit and the gates shutting. That is the evening
    /// trade, and a tavern hearth feeds whoever stands at it
    /// (`03_hunger.md` §4).
    ///
    /// Night trades keep their posts (they have no evening to move) and the
    /// bedless keep the street (they have no hearth to leave). Returns how many
    /// evenings moved, for the diagnostic.
    pub fn reroll_ambient_evenings(&mut self, world: &mut World, day: i64) -> usize {
        // The two hearths, by the same authored names the seed resolves — read
        // off the wayfinding registry so the label the sheet shows and the point
        // the feet walk to are one lookup and cannot disagree.
        let taverns: Vec<(String, Vec3)> = TAVERNS
            .iter()
            .filter_map(|name| {
                world
                    .places
                    .named(name)
                    .map(|entry| (entry.name.clone(), entry.point))
            })
            .collect();

        // The chalked places of resort (`features/implemented/chalking_the_walls.md` M4),
        // by ward: a ward-sign draws its own ward's evening crowd. Gathered
        // once for the whole roll rather than per walker — the set is at most
        // eight and does not change inside the loop.
        let ward_signs: BTreeMap<String, (String, Vec3, f64)> = world
            .marks
            .iter()
            .filter(|(_, mark)| {
                mark.kind == crate::marks::MarkKind::WardSign && world.mark_catalog.is_binding(mark)
            })
            .filter_map(|(_, mark)| {
                let crate::marks::MarkAnchor::Place(name) = &mark.anchor else {
                    return None;
                };
                let entry = world.places.named(name)?;
                let ward = entry.ward.clone()?;
                Some((ward, (entry.name.clone(), entry.point, mark.strength)))
            })
            .collect();

        // The tavern half of tonight's draw is the same for everybody, so it is
        // built once for the whole roll and each walker's own ward sign is
        // pushed onto the end and popped off again. The order is load-bearing:
        // the weighted pick below walks `destinations` by index, so the sign
        // has to stay *last* or every ambient's evening silently re-rolls.
        let mut destinations: Vec<(String, Vec3, f64)> = taverns
            .iter()
            .map(|(label, point)| (label.clone(), *point, 1.0))
            .collect();
        let tavern_count = destinations.len();

        let ids: Vec<ActorId> = self.people.keys().cloned().collect();
        let mut moved = 0usize;
        for id in ids {
            let lore = world
                .characters
                .get(&id)
                .and_then(|character| character.lore());
            let ambient = lore.is_some_and(|profile| profile.significance == Significance::Ambient);
            if !ambient {
                continue;
            }
            // The walker's own ward is already in hand — the loop fetches the
            // profile to test `significance` anyway — so the sign lookup costs
            // nothing extra. Borrowed, and read only after the `ambient` guard:
            // ~170 of the enrolled cast are not ambient and never reach it.
            let own_ward = lore.map(|profile| profile.planning_ward.as_str());
            let Some(person) = self.people.get_mut(&id) else {
                continue;
            };
            if person.curfew_exempt || person.home.is_none() {
                continue;
            }

            // Put last night's choice back first, so the roll is a nightly
            // decision rather than a one-way drift: a night that comes up
            // "home" really does return them to their own hearth.
            let mut changed = false;
            if let Some((index, seed)) = person.evening_seed.take()
                && let Some(leg) = person.legs.get_mut(index)
            {
                *leg = seed;
                changed = true;
            }

            let evening = person
                .legs
                .iter()
                .position(|leg| leg.from == Office::Lamplight && leg.is_home);
            // The roll before the shortlist, not after: six ambients in seven
            // stay at their own hearth, and building a destination list for
            // them was the bulk of the whole pass. Both terms are pure, so
            // which one is asked first cannot change who moves.
            if let Some(index) = evening
                // `as u64` on a negative day wraps, which is all a hash salt
                // needs; the roll only has to be stable and vary by night.
                && hash01("night_evening", &id, day as u64) < AMBIENT_TAVERN_FRACTION
            {
                // The destinations this walker could be drawn to tonight: every
                // tavern at weight 1.0, plus their own ward's chalked place of
                // resort at `1.0 + 2.0 * strength` — a fresh sign is worth three
                // taverns, a half-washed one barely more than one.
                if let Some(ward) = own_ward
                    && let Some((label, point, strength)) = ward_signs.get(ward)
                {
                    destinations.push((label.clone(), *point, 1.0 + 2.0 * strength));
                }
                if !destinations.is_empty() {
                    // Weighted pick from the same pure hash of (id, day) — never
                    // a fresh draw. The engine polls at 60 Hz (see
                    // `attention.rs`), so a roll that was not a pure function of
                    // the night would re-decide sixty times a second.
                    let total: f64 = destinations.iter().map(|(_, _, weight)| weight).sum();
                    let mut cursor = hash01("night_tavern", &id, day as u64) * total;
                    let mut chosen = destinations.len() - 1;
                    for (index, (_, _, weight)) in destinations.iter().enumerate() {
                        if cursor < *weight {
                            chosen = index;
                            break;
                        }
                        cursor -= weight;
                    }
                    let (label, point, _) = destinations[chosen].clone();
                    person.evening_seed = Some((index, person.legs[index].clone()));
                    let leg = &mut person.legs[index];
                    leg.at = point;
                    leg.label = label;
                    // Their ease, not their bed: the curfew rung owns where they
                    // actually sleep, and "sleep at The Hungry Ox" would be a
                    // promise the ladder does not keep.
                    leg.doing = Arrival::Idle;
                    leg.is_home = false;
                    moved += 1;
                    changed = true;
                }
                // Hand the shared list back the way it was lent.
                destinations.truncate(tavern_count);
            }

            if changed {
                let lines: Vec<String> = person.legs.iter().map(leg_line).collect();
                world
                    .characters
                    .get_mut(&id)
                    .expect("the walker is in the world")
                    .state
                    .daily_round = lines;
            }
        }
        moved
    }

    /// The office this person goes to bed at (movement M6): the **earliest**
    /// unconditional `sleep` leg in the day's order, which for a night trade is
    /// the Watch and for a day worker Lamplight. `None` for anyone the round
    /// never enrolled, and for the handful whose day names no bed at all — the
    /// anchoress bricked into her wall, the homeless — whom the Night Office
    /// then reflects at the curfew like everybody else.
    ///
    /// This is what staggers the lane without a scheduler: the Round already
    /// says when each character sleeps, and they do not all sleep at once.
    pub fn bedtime(&self, actor: &ActorId) -> Option<Office> {
        let person = self.people.get(actor)?;
        let earliest_bed = |unconditional: bool| {
            person
                .legs
                .iter()
                .filter(|leg| leg.doing == Arrival::Sleep)
                .filter(|leg| leg.only_on.is_none() == unconditional)
                .map(|leg| leg.from)
                .min()
        };
        // A weekday-restricted bed is a fallback, not a bedtime: a Bellday-only
        // sleep leg must not decide the other six nights.
        earliest_bed(true).or_else(|| earliest_bed(false))
    }

    fn is_road_member(&self, actor: &ActorId) -> bool {
        self.road_parties
            .values()
            .any(|party| party.members.contains(actor))
    }

    fn seed_road_parties(
        &mut self,
        world: &mut World,
        resolver: &PlaceResolver<'_>,
        specs: Vec<RoadPartySpec>,
        time: crate::clock::WorldTime,
        diagnostics: &mut Vec<String>,
    ) {
        let mut claimed = BTreeSet::new();
        for spec in specs {
            if self.road_parties.contains_key(&spec.id) {
                diagnostics.push(format!(
                    "[smart actors] duplicate road party {}; skipped",
                    spec.id
                ));
                continue;
            }
            let member_set: BTreeSet<ActorId> = spec.members.iter().cloned().collect();
            if !spec.members.contains(&spec.leader)
                || spec.members.is_empty()
                || member_set.len() != spec.members.len()
                || spec.members.iter().any(|member| claimed.contains(member))
            {
                diagnostics.push(format!(
                    "[smart actors] road party {} has invalid or duplicate membership; skipped",
                    spec.id
                ));
                continue;
            }
            if spec.only_on.is_empty()
                || spec.only_on.iter().collect::<BTreeSet<_>>().len() != spec.only_on.len()
                || !(spec.stage_at < spec.enter_at && spec.enter_at < spec.return_at)
                || spec.commercial_cargo.is_empty()
                || spec.manifest.is_empty()
                || spec.legs.is_empty()
            {
                diagnostics.push(format!(
                    "[smart actors] road party {} has an invalid schedule or cargo declaration; skipped",
                    spec.id
                ));
                continue;
            }
            if spec.wallet_float_sparks.len() != spec.members.len()
                || spec
                    .members
                    .iter()
                    .any(|member| !spec.wallet_float_sparks.contains_key(member))
            {
                diagnostics.push(format!(
                    "[smart actors] road party {} wallet floats do not exactly match its members; skipped",
                    spec.id
                ));
                continue;
            }
            let Some(gate_point) = resolver.resolve(&spec.gate) else {
                diagnostics.push(format!(
                    "[smart actors] road party {} gate {:?} is missing; skipped",
                    spec.id, spec.gate
                ));
                continue;
            };
            if spec
                .members
                .iter()
                .any(|member| !world.characters.contains_key(member))
            {
                diagnostics.push(format!(
                    "[smart actors] road party {} names a missing character; skipped",
                    spec.id
                ));
                continue;
            }
            let invalid_cargo = spec.commercial_cargo.iter().any(|matcher| {
                let probe = matcher.to_item(ItemId::from_raw("road_matcher_probe"), 1);
                world.item_catalog.validate_seed_item(&probe).is_err()
            }) || spec.manifest.iter().any(|stock| {
                let probe = stock
                    .matcher()
                    .to_item(ItemId::from_raw("road_manifest_probe"), stock.quantity);
                world.item_catalog.validate_seed_item(&probe).is_err()
            });
            if invalid_cargo {
                diagnostics.push(format!(
                    "[smart actors] road party {} has invalid catalog cargo; skipped",
                    spec.id
                ));
                continue;
            }
            let mut legs = Vec::new();
            let mut unresolved = false;
            for leg in spec.legs {
                let Some(at) = resolver.resolve(&leg.at) else {
                    diagnostics.push(format!(
                        "[smart actors] road party {} leg site {:?} is missing; skipped",
                        spec.id, leg.at
                    ));
                    unresolved = true;
                    break;
                };
                legs.push(RoundLeg {
                    from: leg.from,
                    at,
                    label: leg.at,
                    doing: leg.doing,
                    only_on: leg.only_on,
                    is_home: false,
                });
            }
            if unresolved {
                continue;
            }
            claimed.extend(spec.members.iter().cloned());
            for member in &spec.members {
                let actor = world
                    .characters
                    .get_mut(member)
                    .expect("validated road member");
                actor.state.presence = crate::Presence::BeyondTheWalls;
                actor.state.economic_class = EconomicClass::RoadParty;
                actor.state.leaving_city = false;
                actor.state.movement = None;
                actor.state.daily_round = legs.iter().map(leg_line).collect();
            }
            let id = spec.id.clone();
            self.road_parties.insert(
                id.clone(),
                RoadParty {
                    id,
                    leader: spec.leader,
                    members: spec.members,
                    gate: spec.gate,
                    gate_point,
                    only_on: spec.only_on,
                    stage_at: spec.stage_at,
                    enter_at: spec.enter_at,
                    return_at: spec.return_at,
                    wallet_floats: spec.wallet_float_sparks,
                    commercial_cargo: spec.commercial_cargo,
                    manifest: spec.manifest,
                    legs,
                    state: PartyState {
                        phase: PartyPhase::BeyondTheWalls,
                        trip_number: 0,
                    },
                    last_trigger_day: None,
                    departure_excuses: BTreeMap::new(),
                },
            );
        }

        let ids: Vec<PartyId> = self.road_parties.keys().cloned().collect();
        for id in ids {
            let scheduled = self.road_parties[&id].only_on.contains(&time.weekday);
            if !scheduled {
                continue;
            }
            if time.office == self.road_parties[&id].stage_at
                || time.office == self.road_parties[&id].enter_at
            {
                self.trigger_road_stage(world, &id, time.day);
            }
            if time.office == self.road_parties[&id].enter_at {
                self.trigger_road_entry(world, &id, time.day);
            }
        }
        diagnostics.push(format!(
            "[smart actors] round: {} fixed road parties",
            self.road_parties.len()
        ));
    }

    fn trigger_road_stage(&mut self, world: &mut World, id: &PartyId, day: i64) {
        let mut party = self
            .road_parties
            .remove(id)
            .expect("party id came from map");
        if party.last_trigger_day == Some(day) {
            self.road_parties.insert(id.clone(), party);
            return;
        }
        party.last_trigger_day = Some(day);
        if party.state.phase != PartyPhase::BeyondTheWalls {
            self.push_food_log(road_transition_trace(
                "road_trip_missed",
                &party,
                world,
                day,
            ));
            self.road_parties.insert(id.clone(), party);
            return;
        }
        match boundary_exchange(&mut party, world) {
            Ok(receipt) => {
                for line in receipt.lines {
                    self.push_food_log(line);
                }
                self.road_cash_in_sparks = self
                    .road_cash_in_sparks
                    .checked_add(receipt.cash_in_sparks)
                    .expect("road cash-in accounting overflow");
                self.road_cash_out_sparks = self
                    .road_cash_out_sparks
                    .checked_add(receipt.cash_out_sparks)
                    .expect("road cash-out accounting overflow");
                self.push_food_log(road_transition_trace("road_stage", &party, world, day));
            }
            Err(error) => self.push_food_log(format!(
                "{}; reason {error}",
                road_transition_trace("boundary_exchange_failed", &party, world, day)
            )),
        }
        self.road_parties.insert(id.clone(), party);
    }

    fn trigger_road_entry(&mut self, world: &mut World, id: &PartyId, day: i64) {
        let mut party = self
            .road_parties
            .remove(id)
            .expect("party id came from map");
        if party.state.phase != PartyPhase::StagedOutsideGate {
            self.road_parties.insert(id.clone(), party);
            return;
        }
        let positions = party
            .members
            .iter()
            .enumerate()
            .map(|(index, member)| {
                let side = index as f64 * 0.8;
                (
                    member.clone(),
                    Vec3::new(party.gate_point.x + side, WALK_Y, party.gate_point.z),
                )
            })
            .collect::<BTreeMap<_, _>>();
        match world.transition_presence(&party.members, crate::Presence::InCity, &positions) {
            Ok(_) => {
                for member in &party.members {
                    let actor = world
                        .characters
                        .get_mut(member)
                        .expect("party member exists");
                    actor.state.needs.hunger = HUNGER_MAX;
                    actor.state.needs.thirst = THIRST_MAX;
                    actor.state.leaving_city = false;
                }
                party.state.phase = PartyPhase::InCity;
                self.push_food_log(road_transition_trace("road_in", &party, world, day));
            }
            Err(error) => self.push_food_log(format!(
                "road_in_failed: party {}, day {day}: {error}",
                party.id
            )),
        }
        self.road_parties.insert(id.clone(), party);
    }

    fn trigger_road_office(
        &mut self,
        world: &mut World,
        office: Office,
        time: crate::clock::WorldTime,
        nudges: &mut Vec<ActorId>,
    ) {
        let ids: Vec<PartyId> = self.road_parties.keys().cloned().collect();
        for id in ids {
            let scheduled = self.road_parties[&id].only_on.contains(&time.weekday);
            if office == self.road_parties[&id].stage_at && scheduled {
                self.trigger_road_stage(world, &id, time.day);
            }
            if office == self.road_parties[&id].enter_at && scheduled {
                // A coarse pump may have crossed both offices; stage is
                // idempotent and therefore safe to ensure here as well.
                self.trigger_road_stage(world, &id, time.day);
                self.trigger_road_entry(world, &id, time.day);
            }
            if office == self.road_parties[&id].return_at {
                self.begin_road_return(world, &id, time.day, nudges);
            }
        }
    }

    fn begin_road_return(
        &mut self,
        world: &mut World,
        id: &PartyId,
        day: i64,
        nudges: &mut Vec<ActorId>,
    ) {
        let mut party = self
            .road_parties
            .remove(id)
            .expect("party id came from map");
        if party.state.phase != PartyPhase::InCity {
            self.road_parties.insert(id.clone(), party);
            return;
        }
        // A fresh return owes every member the excuse-yourself courtesy anew.
        party.departure_excuses.clear();
        for member in &party.members {
            self.finish_market_errand(world, member, MarketVisitEnd::Returning);
            for source in &mut self.sources {
                source.queue.retain(|queued| queued != member);
                if source
                    .serving
                    .as_ref()
                    .is_some_and(|(actor, _)| actor == member)
                {
                    source.serving = None;
                }
            }
            for stall in &mut self.stalls {
                stall.queue.retain(|queued| queued != member);
                if stall
                    .serving
                    .as_ref()
                    .is_some_and(|(actor, _)| actor == member)
                {
                    stall.serving = None;
                }
            }
            if let Some(person) = self.people.get_mut(member) {
                person.food = None;
                person.phase = Phase::Idle;
                person.travel_target = None;
                person.travel_for_intent = false;
            }
            // The recall preempts a live `go_to` exactly as a pressing rung
            // does, and the lapse is a percept through the same door
            // ([`end_intent`]): silent abandonment would leave the mind
            // believing it is still headed somewhere the body gave up on —
            // the untruth rule (05_the_llm_seam.md §2). A member the law
            // holds is the one exception: the departure goes without them,
            // the seizure's own percepts already own their story, and "the
            // road turned you back" would be the new untruth.
            if !world.custody.holds(member)
                && let Some(destination) = intent_destination(world, member)
            {
                end_intent(
                    self,
                    world,
                    member,
                    format!(
                        "The road turned you back before you reached {destination} — the party is leaving for the gate."
                    ),
                    nudges,
                );
            }
            let actor = world
                .characters
                .get_mut(member)
                .expect("party member exists");
            actor.state.movement = None;
            actor.state.intent = None;
            actor.state.active_gesture = None;
            // A member the law holds is not leaving — the departure goes
            // without them — and the flag has teeth: `leaving_city` refuses
            // trade verbs, which a committed prisoner facing the posted gaol
            // fee may badly need.
            actor.state.leaving_city = !world.custody.holds(member);
        }
        party.state.phase = PartyPhase::Returning;
        world.touch_public_state();
        self.push_food_log(road_transition_trace("road_return", &party, world, day));
        self.road_parties.insert(id.clone(), party);
    }

    fn tick_road_parties(
        &mut self,
        world: &mut World,
        nav: &NavData,
        time: crate::clock::WorldTime,
        now: f64,
        in_conversation: &BTreeSet<ActorId>,
    ) {
        let ids: Vec<PartyId> = self.road_parties.keys().cloned().collect();
        for id in ids {
            let mut party = self
                .road_parties
                .remove(&id)
                .expect("party id came from map");
            match party.state.phase {
                PartyPhase::InCity => {
                    let target =
                        active_leg(&party.legs, time.office, time.weekday).map(|leg| leg.at);
                    if let Some(target) = target {
                        for member in &party.members {
                            let Some(actor) = world.characters.get(member) else {
                                continue;
                            };
                            // A member in the law's hands is walked by their
                            // escort, or stands at a keeper's threshold — the
                            // party's leg is not theirs until the law lets go.
                            if world.custody.holds(member) {
                                continue;
                            }
                            if in_conversation.contains(member) {
                                world
                                    .characters
                                    .get_mut(member)
                                    .expect("member exists")
                                    .state
                                    .movement = None;
                                continue;
                            }
                            // Explicit `go_to` remains above the ordinary party
                            // route while the party is trading — and it brings
                            // its own arrival radius with it: stopping a
                            // follower six metres short of the person they set
                            // out to catch is a "caught up with" [`tick_intents`]
                            // can never say, so the errand would only ever lapse.
                            let (target, arrive_radius) = actor.state.intent.as_ref().map_or(
                                (target, ROUND_ARRIVE_RADIUS_M),
                                |intent| match &intent.target {
                                    IntentTarget::Place { point, .. } => {
                                        (*point, PLACE_ARRIVE_RADIUS_M)
                                    }
                                    IntentTarget::Person { last_seen, .. } => {
                                        (*last_seen, PERSON_ARRIVE_RADIUS_M)
                                    }
                                },
                            );
                            if actor.position_m().distance(target) <= arrive_radius {
                                world
                                    .characters
                                    .get_mut(member)
                                    .expect("member exists")
                                    .state
                                    .movement = None;
                            } else if !actor.is_walking()
                                && let Some(path) =
                                    route_path_to_point(nav, member, actor.position_m(), target)
                            {
                                set_route(world, member, path);
                            }
                        }
                    }
                }
                PartyPhase::Returning => {
                    let mut public_state_changed = false;
                    for member in &party.members {
                        // Copied out, because the excuse and expiry paths
                        // below need `world` mutably before the walk is laid.
                        let Some((position, walking)) = world
                            .characters
                            .get(member)
                            .map(|actor| (actor.position_m(), actor.is_walking()))
                        else {
                            continue;
                        };
                        // A member the law took on the way out is never routed
                        // to the gate: the departure below leaves without them,
                        // and four of the eight stations *are* gates, so the
                        // arrival branch must not expire the offers of somebody
                        // who is committed at the arch rather than leaving.
                        if world.custody.holds(member) {
                            continue;
                        }
                        if in_conversation.contains(member) {
                            // The road breaks a conversation with the same
                            // courtesy the pressing rungs give ([`run_ladder`]):
                            // the `system:` pressure line first, one turn's
                            // grace to say the goodbye, and only then does the
                            // body walk — a carrier who strode off to the gate
                            // mid-topic without a word left their partner (and
                            // their own mind) believing an exchange that no
                            // longer exists. Road members are not in `people`,
                            // so the ladder's `excused` flag cannot carry
                            // this; the party remembers it instead.
                            let walk_at = *party
                                .departure_excuses
                                .entry(member.clone())
                                .or_insert_with(|| {
                                    world
                                        .characters
                                        .get_mut(member)
                                        .expect("member exists")
                                        .notify_percept(ROAD_RETURN_PRESSURE.to_string());
                                    now + decision_jitter(member, 0)
                                });
                            if now < walk_at {
                                world
                                    .characters
                                    .get_mut(member)
                                    .expect("member exists")
                                    .state
                                    .movement = None;
                                continue;
                            }
                            // Grace spent: fall through and walk, exactly as
                            // the ladder does on the pressing decision after
                            // the excuse turn.
                        }
                        if position.distance(party.gate_point) > PLACE_ARRIVE_RADIUS_M {
                            if !walking
                                && let Some(path) =
                                    route_path_to_point(nav, member, position, party.gate_point)
                            {
                                set_route(world, member, path);
                            }
                        } else {
                            world
                                .characters
                                .get_mut(member)
                                .expect("member exists")
                                .state
                                .movement = None;
                            // A promise cannot follow a traveller through the
                            // gate. Expire offers as each member arrives, not
                            // only after the slowest cart-mate gets there —
                            // and through the same courtesy machinery the
                            // distance sweep uses ([`crate::actions::lapse_offer`]):
                            // both parties are told why, and the `lapse_offer`
                            // event drives the HUD notice. A bare
                            // `offers.remove` here left the other party's arm
                            // out toward a promise that no longer existed.
                            let expiring: Vec<ItemId> = world
                                .offers
                                .iter()
                                .filter(|(_, offer)| {
                                    offer.giver_id == *member
                                        || offer.target_id.as_ref() == Some(member)
                                })
                                .map(|(item, _)| item.clone())
                                .collect();
                            for item in expiring {
                                crate::actions::lapse_offer(
                                    world,
                                    &item,
                                    &crate::actions::OfferLapse::ThroughTheGate {
                                        leaver: member.clone(),
                                    },
                                );
                                public_state_changed = true;
                                self.push_food_log(format!(
                                    "{}; member {member}, item {item}",
                                    road_transition_trace(
                                        "road_offer_expired",
                                        &party,
                                        world,
                                        time.day,
                                    )
                                ));
                            }
                        }
                    }
                    // Who can actually leave: the law keeps its own. A held
                    // crew member neither blocks the departure (they would
                    // never reach the gate) nor makes it — and a party whose
                    // *leader* is held waits instead, because the cart, the
                    // manifest and the boundary exchange are all the leader's,
                    // and the hold ceilings drain every custody in minutes.
                    let free: Vec<&ActorId> = party
                        .members
                        .iter()
                        .filter(|member| !world.custody.holds(member))
                        .collect();
                    let at_gate = !free.is_empty()
                        && !world.custody.holds(&party.leader)
                        && free.iter().all(|member| {
                            world.characters[*member]
                                .position_m()
                                .distance(party.gate_point)
                                <= PLACE_ARRIVE_RADIUS_M
                        });
                    if at_gate && free.iter().all(|member| !in_conversation.contains(*member)) {
                        party.state.phase = PartyPhase::DeparturePending;
                        public_state_changed = true;
                        self.push_food_log(road_transition_trace(
                            "road_departure_pending",
                            &party,
                            world,
                            time.day,
                        ));
                    }
                    if public_state_changed {
                        world.touch_public_state();
                    }
                }
                PartyPhase::DeparturePending => {
                    // Ask the law again at the threshold: the seizure that
                    // matters here is the one that landed after the gate check.
                    // A person the law holds must not be carried out of the
                    // world — `transition_presence` would dissolve a custody
                    // nobody released — so the held stay behind and the party
                    // leaves without them. With the leader held (or nobody
                    // free at all) there is no departure to have: back to
                    // Returning, whose gate walk re-forms once the law lets go
                    // — never a departure from wherever a release left them.
                    let (staying, departing): (Vec<ActorId>, Vec<ActorId>) = party
                        .members
                        .iter()
                        .cloned()
                        .partition(|member| world.custody.holds(member));
                    if departing.is_empty() || staying.contains(&party.leader) {
                        party.state.phase = PartyPhase::Returning;
                        self.push_food_log(road_transition_trace(
                            "road_departure_held",
                            &party,
                            world,
                            time.day,
                        ));
                    } else if departing
                        .iter()
                        .all(|member| !in_conversation.contains(member))
                    {
                        let positions = BTreeMap::new();
                        match world.transition_presence(
                            &departing,
                            crate::Presence::BeyondTheWalls,
                            &positions,
                        ) {
                            Ok(_) => {
                                for member in &departing {
                                    let actor =
                                        world.characters.get_mut(member).expect("member exists");
                                    actor.state.leaving_city = false;
                                }
                                for member in &staying {
                                    // Left behind for good: the cast is fixed
                                    // and this is a named person, so a life in
                                    // the city is theirs the moment the law is
                                    // done with them — but the roster, the
                                    // wallet float and the boundary exchange
                                    // must stop naming somebody who no longer
                                    // travels. [`Round::enrol_left_behind`] is
                                    // what actually hands that life over: the
                                    // retain below is permanent, and `seed` —
                                    // one-shot, and long since run — will never
                                    // come back for them.
                                    let actor =
                                        world.characters.get_mut(member).expect("member exists");
                                    actor.state.leaving_city = false;
                                    self.enrol_left_behind(
                                        world,
                                        nav,
                                        member,
                                        party.gate_point,
                                        now,
                                    );
                                    self.push_food_log(format!(
                                        "road_left_behind: party {}, trip {}, member {member} \
                                         is in the law's hands and stays",
                                        party.id, party.state.trip_number
                                    ));
                                }
                                party.members.retain(|member| !staying.contains(member));
                                party.state.phase = PartyPhase::BeyondTheWalls;
                                self.departed_this_tick.extend(departing.iter().cloned());
                                self.push_food_log(road_transition_trace(
                                    "road_out", &party, world, time.day,
                                ));
                            }
                            Err(error) => self.push_food_log(format!(
                                "road_out_failed: party {}, trip {}: {error}",
                                party.id, party.state.trip_number
                            )),
                        }
                    }
                }
                PartyPhase::BeyondTheWalls | PartyPhase::StagedOutsideGate => {}
            }
            self.road_parties.insert(id, party);
        }
    }

    /// Give somebody the law kept back at the gate a life in the city: enrol
    /// them in `people` exactly as [`Round::seed`] enrols anyone else it finds
    /// without a bed. Until this, being left behind meant being dropped —
    /// `run_ladder`, `decay_needs` and the census all walk `people`, so the
    /// stranded stood on one paving stone for the rest of the run while their
    /// sheet went on naming a trading leg the departed cart had taken with it.
    ///
    /// It never wants undoing. The roster `retain` above is permanent and
    /// nothing ever puts a member back, so the party's next trip neither stages
    /// nor names them: the only enrolment they can ever get is this one, and a
    /// returning cart finds a townsperson, not a crew member.
    ///
    /// Nothing here is invented — the cast is fixed and so is what the seed
    /// knows about them. The legs are `rounds.json`'s own archetype for the
    /// occupation their sheet already declares, and the bed that archetype asks
    /// for is simply skipped, because `homes.json` names no door for a road
    /// member: a stranded carter is a homeless one, still in the street at the
    /// Snuffing like every other unhoused person, which is exactly the person
    /// the watch stops. The leash centre is the gate their own party's evening
    /// leg told them to stand at — the one point in this city their seed data
    /// gives them, and a steadier one than wherever the law happened to lay
    /// hands on them.
    ///
    /// They become a [`EconomicClass::Visitor`]: no party float or boundary
    /// exchange is theirs any more, but neither is the Watch's household
    /// settlement — they are in the city without being of it, which is the case
    /// that class was written for. Needs are left where they stand: hunger now
    /// decays with everyone else's from full, the un-enrolled state `seed`'s
    /// hunger spread deliberately leaves anybody it does not seed low.
    fn enrol_left_behind(
        &mut self,
        world: &mut World,
        nav: &NavData,
        id: &ActorId,
        gate_point: Vec3,
        now: f64,
    ) {
        let Some(character) = world.characters.get(id) else {
            return;
        };
        let occupation = character.lore().and_then(|lore| lore.occupation_id.clone());
        let ward = character.lore().map(|lore| lore.planning_ward);

        let (legs, leash_m, curfew_exempt) = match serde_json::from_str::<RoundsDoc>(ROUNDS_JSON) {
            Ok(rounds) => build_legs(
                &rounds,
                &PlaceResolver::new(nav),
                &self.worksites,
                id,
                occupation.as_deref(),
                None,
                gate_point,
            ),
            // The shape `seed` degrades to when the content will not parse: no
            // legs, and the wander keeps them near the gate.
            Err(_) => (Vec::new(), DEFAULT_ROUND_LEASH_M, false),
        };

        // The wayfinding whitelist `seed` assembles, minus the homes: the coarse
        // handles, their own ward, and the stations of the day above. Extended
        // rather than assigned, because a way somebody told them on the trip in
        // is theirs to keep.
        let mut known: BTreeSet<PlaceId> = world
            .places
            .coarse()
            .map(|entry| entry.id.clone())
            .collect();
        if let Some(ward) = ward {
            known.extend(
                world
                    .places
                    .ward_places(ward.as_str())
                    .map(|entry| entry.id.clone()),
            );
        }
        for leg in &legs {
            if let Some(entry) = world.places.named(&leg.label) {
                known.insert(entry.id.clone());
            }
        }

        // Water by `seed`'s rule, so a party that one day carries a fuller
        // leaves behind somebody with a curb like anyone else of that trade.
        // No thirst spread: that spread exists to stagger a whole city's first
        // draw, and this is one person arriving alone.
        let (source, is_household) = match vessel_of(occupation.as_deref()) {
            Some(is_household) => (self.nearest_staffed_source(gate_point), is_household),
            None => (None, false),
        };

        let state = &mut world.characters.get_mut(id).expect("member exists").state;
        state.places_known.extend(known);
        state.daily_round = legs.iter().map(leg_line).collect();
        state.economic_class = EconomicClass::Visitor;
        self.people.insert(
            id.clone(),
            Townsperson {
                home: None,
                base: gate_point,
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
                evening_seed: None,
                excused: false,
            },
        );
    }

    /// The staffed water source nearest a point, ties broken by source index so
    /// an exact distance tie binds the same way every run.
    fn nearest_staffed_source(&self, base: Vec3) -> Option<usize> {
        (0..self.sources.len())
            .filter(|index| self.sources[*index].keeper.is_some())
            .min_by(|left, right| {
                let dl = self.sources[*left].draw_point.distance(base);
                let dr = self.sources[*right].draw_point.distance(base);
                dl.partial_cmp(&dr)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.cmp(right))
            })
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
                .then(|| {
                    source.and_then(|source| source.queue.iter().position(|queued| queued == id))
                })
                .flatten(),
            walk_target,
            for_intent: person.phase == Phase::Travelling && person.travel_for_intent,
        })
    }

    /// A one-line census of the water round for `--trace-water`.
    pub fn water_summary(&self, world: &World) -> String {
        let staffed = self
            .sources
            .iter()
            .filter(|source| source.keeper.is_some())
            .count();
        let queued: usize = self.sources.iter().map(|source| source.queue.len()).sum();
        let drawing = self
            .sources
            .iter()
            .filter(|source| source.serving.is_some())
            .count();
        let drawers = self
            .people
            .iter()
            .filter(|(id, person)| person.draws_water() && world.is_present(id))
            .count();
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
        let total = self.people.keys().filter(|id| world.is_present(id)).count();
        let mut fed = 0usize;
        let mut hungry = 0usize;
        let mut famished = 0usize;
        let mut sum = 0.0;
        for id in self.people.keys() {
            let Some(character) = world.characters.get(id).filter(|_| world.is_present(id)) else {
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
        let sparks_by_class = |class| -> u64 {
            world
                .characters
                .values()
                .filter(|actor| actor.state.economic_class == class)
                .map(|actor| u64::from(world.wallet_sparks(actor.id())))
                .sum()
        };
        let resident_sparks = sparks_by_class(EconomicClass::Resident);
        let visitor_sparks = sparks_by_class(EconomicClass::Visitor);
        let road_sparks = sparks_by_class(EconomicClass::RoadParty);
        let sparks = resident_sparks + visitor_sparks + road_sparks;
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
                    world.is_present(v)
                        && world.characters.get(v).is_some_and(|c| {
                            c.position_m().distance(stall.pitch) <= STALL_PITCH_REACH_M
                        })
                }) && time.is_some_and(|t| {
                    stall.open.is_open(t.office, t.weekday) && stall_weather_open(world, stall)
                })
            })
            .count();
        let queued: usize = self.stalls.iter().map(|stall| stall.queue.len()).sum();
        let serving = self
            .stalls
            .iter()
            .filter(|stall| stall.serving.is_some())
            .count();
        let stock: u32 = self
            .stalls
            .iter()
            .filter_map(|stall| stall.vendor.as_ref().map(|vendor| (stall, vendor)))
            .map(|(stall, vendor)| {
                self.food_trades
                    .get(&stall.trade)
                    .map(|trade| {
                        trade
                            .listings
                            .iter()
                            .map(|matcher| world.uncommitted_held_quantity(vendor, matcher))
                            .sum::<u32>()
                    })
                    .unwrap_or(0)
            })
            .sum();
        let chain_quantity = |kind: &str| -> u64 {
            world
                .items
                .values()
                .filter(|item| item.kind.as_str() == kind)
                .map(|item| u64::from(item.quantity))
                .sum()
        };
        let road_present = self
            .road_parties
            .values()
            .filter(|party| party.members.iter().all(|member| world.is_present(member)))
            .count();
        let resident_spendable = world
            .characters
            .values()
            .filter(|actor| actor.state.economic_class == EconomicClass::Resident)
            .map(|actor| u64::from(world.spendable_sparks(actor.id())))
            .sum::<u64>();
        let grain = ItemMatcher::new("grain");
        let seven_lofts_grain = world.held_quantity(&ActorId::from_raw("p008s"), &grain);
        let chain_kinds = ["grain", "flour", "loaf", "wool", "cloth"];
        let chain_actors = [
            ("Betriss", ActorId::from_raw("p008s")),
            ("Bertran", ActorId::from_raw("e7mil")),
            ("Averil", ActorId::from_raw("davqn")),
            ("Ewart", ActorId::from_raw("e1skl")),
        ]
        .into_iter()
        .filter_map(|(name, id)| {
            let actor = world.characters.get(&id)?;
            let ids = actor.holds().iter().filter(|item_id| {
                world.items.get(*item_id).is_some_and(|item| {
                    chain_kinds.contains(&item.kind.as_str())
                })
            });
            let mut held = 0u64;
            let mut uncommitted = 0u64;
            let mut listed = 0u64;
            let mut reserved = 0u64;
            let mut offered = 0u64;
            for item_id in ids {
                let item = &world.items[item_id];
                held += u64::from(item.quantity);
                uncommitted += u64::from(world.uncommitted_quantity(item_id));
                if commercially_listed(self, world, &id, item_id) {
                    listed += u64::from(item.quantity);
                }
                reserved += u64::from(world.transform_reserved_quantity(item_id));
                offered += u64::from(world.offered_quantity(item_id));
            }
            Some(format!(
                "{name}({id}) held {held}/free {uncommitted}/listed {listed}/reserved {reserved}/offered {offered}"
            ))
        })
        .collect::<Vec<_>>()
        .join("; ");
        let parties = self
            .road_parties
            .values()
            .map(|party| {
                let members = party
                    .members
                    .iter()
                    .map(|member| {
                        let actor = &world.characters[member];
                        format!(
                            "{member}:wallet {}/float {}/{:?}@{}",
                            world.wallet_sparks(member),
                            party.wallet_floats[member],
                            actor.state.presence,
                            actor.state.presence_epoch,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let cart = road_cart_is_visible(party, world).then(|| road_cart_load(party, world));
                format!(
                    "{} phase {:?}/trip {}/members [{}]/cart {:?}",
                    party.id, party.state.phase, party.state.trip_number, members, cart
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let errands = self
            .market_errands
            .iter()
            .map(|(buyer, errand)| {
                let plan = self
                    .stock_plans
                    .iter()
                    .find(|plan| plan.id == errand.plan_id);
                let (source, remaining) = plan.map_or_else(
                    || ("<missing>".to_string(), 0),
                    |plan| {
                        (
                            format!("{:?}", plan.source),
                            plan.max_spend_sparks.saturating_sub(errand.spent_sparks),
                        )
                    },
                );
                format!(
                    "{} buyer {buyer}/source {source}/selected {:?}/seen {:?}/phase {:?}/spent {}/remaining {remaining}/fingerprint {:?}/next {}",
                    errand.plan_id,
                    errand.selected,
                    errand.bindings_seen,
                    errand.phase,
                    errand.spent_sparks,
                    errand.last_failed_fingerprint,
                    if errand.last_failed_fingerprint.is_some() {
                        "fingerprint_change"
                    } else {
                        "next_tick"
                    },
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let jobs = world
            .transform_jobs()
            .map(|job| {
                format!(
                    "{} producer {}/spec {}/day {}/progress {:.3}/reserved [{}]/future [{}]",
                    job.job_id,
                    job.producer,
                    job.spec_id,
                    job.production_day,
                    job.progress_work_minutes,
                    reserved_inputs_trace(&job.inputs),
                    stock_specs_trace(&job.outputs),
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let completed = world
            .completed_transforms()
            .map(|completed| {
                format!(
                    "{}@{} consumed [{}] produced [{}]",
                    completed.receipt.job_id,
                    completed.completed_on_day,
                    transform_receipt_lines_trace(&completed.receipt.consumed),
                    transform_receipt_lines_trace(&completed.receipt.produced),
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let streaks = self
            .unrelieved_zero_streak
            .iter()
            .filter(|(_, streak)| **streak > 0)
            .map(|(actor, streak)| format!("{actor}:{streak}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut summary = format!(
            "food: {total} present enrolled | fed {fed}, hungry {hungry}, famished {famished} | mean {mean:.0} | \
             sparks {sparks} (resident total {resident_sparks}/spendable {resident_spendable}, visitor {visitor_sparks}, road {road_sparks}; cash_in {}, cash_out {}, redistributed {}, payroll {}) | \
             stalls {}/{} open, queued {queued}, serving {serving}, stock {stock} | \
             chain grain {}, flour {}, loaf {}, wool {}, cloth {}; Seven Lofts grain {seven_lofts_grain} | errands {}, jobs {} | road {road_present}/{} present",
            self.road_cash_in_sparks,
            self.road_cash_out_sparks,
            self.household_redistributed_sparks,
            self.institutional_payroll_sparks,
            staffed,
            self.stalls.len(),
            chain_quantity("grain"),
            chain_quantity("flour"),
            chain_quantity("loaf"),
            chain_quantity("wool"),
            chain_quantity("cloth"),
            self.market_errands.len(),
            world.transform_jobs().count(),
            self.road_parties.len(),
        );
        summary.push_str(&format!(
            " | chain actors [{chain_actors}] | parties [{parties}] | market errands [{errands}] | active jobs [{jobs}] | completed jobs [{completed}] | settlement watch {:?}/completed {:?}/zero_streaks [{streaks}]",
            self.last_household_watch_day,
            self.last_household_settlement_day,
        ));
        summary
    }

    /// A behavioural census of the enrolled cast at the current instant: how many
    /// are home, at a post, walking, or left in the street, and which posts are
    /// populated. Reads the current office/weekday off the clock so a leg's
    /// market-day and night rules are respected.
    pub fn census(&self, world: &World, clock: &WorldClock, now: f64) -> Census {
        let time = clock.at(now);
        let mut census = Census {
            total: self.people.keys().filter(|id| world.is_present(id)).count(),
            ..Census::default()
        };
        for (id, person) in &self.people {
            let Some(character) = world.characters.get(id).filter(|_| world.is_present(id)) else {
                continue;
            };
            let position = character.position_m();
            // Near their current post counts as *at* it, walking or standing — a
            // seller milling a few metres across the Wickmarket is still at the
            // Wickmarket. Check this before "walking", so a wander on the spot does
            // not read as a journey.
            //
            // How wide "near" is, is the person's own reach (M3): a generated
            // citizen holds a 15..=40 m leash, and somebody milling 25 m across
            // a market they are leashed to is at their post by construction —
            // read at the flat 9 m they would census as absent and the city
            // would look like one that never turns up for work. So the radius
            // is `max(leash, CENSUS_POST_RADIUS_M)`, not `min`: the *narrower*
            // of the two would collapse every wide leash back onto the 9 m the
            // milestone exists to widen. It is a floor, so a keeper's 4 m leash
            // still censuses at 9 m.
            //
            // Gated on `generated`, and knowingly asymmetric: `rounds.json`
            // authors leashes up to 24 m, so the same argument says the cast
            // should be read this way too, and today a mason milling 12 m
            // across his lodge censuses as "in the street". Fixing that would
            // change the census at `extra_ambient_npcs: 0`, which this feature
            // may not do. It is the census's own bug and wants its own change.
            let post_radius = match character.lore().is_some_and(|lore| lore.generated) {
                true => person.leash_m.max(CENSUS_POST_RADIUS_M),
                false => CENSUS_POST_RADIUS_M,
            };
            if let Some(leg) = active_leg(&person.legs, time.office, time.weekday)
                && position.distance(leg.at) <= post_radius
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
    pub fn seed(
        &mut self,
        world: &mut World,
        nav: &NavData,
        now: f64,
        clock: &WorldClock,
    ) -> Vec<String> {
        let mut diagnostics = Vec::new();
        if self.seeded {
            return diagnostics;
        }
        self.seeded = true;
        self.last_game_days = clock.game_days(now);

        // Road membership is authoritative before the ordinary round selects
        // keepers or seeds needs: off-map actors must not be enrolled merely
        // because their authored occupation resembles a resident trade.
        let early_rounds = serde_json::from_str::<RoundsDoc>(ROUNDS_JSON);
        let early_resolver = PlaceResolver::new(nav);
        match early_rounds {
            Ok(doc) => self.seed_road_parties(
                world,
                &early_resolver,
                doc.road_parties,
                clock.at(now),
                &mut diagnostics,
            ),
            Err(error) => diagnostics.push(format!(
                "[smart actors] round: road parties did not load: {error}"
            )),
        }

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
                    && !self.is_road_member(id)
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
                || character
                    .memories()
                    .iter()
                    .any(|memory| memory_declares_hunger(memory));
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
                let sparks = WALLET_SEED_MIN
                    + (WALLET_SEED_SPREAD as f64 * hash01(WALLET_SEED_SALT, id, 0)) as u32;
                let wallet_id = ItemId::from_raw(format!("w_{}", id.as_str()));
                world.add_item(Item::stack(wallet_id.clone(), "spark", sparks));
                world
                    .characters
                    .get_mut(id)
                    .expect("townsperson exists")
                    .state
                    .holds
                    .push(wallet_id);
            }
        }

        // A keeper each: the nearest free **ambient** townsperson to the curb,
        // pinned there so a queue has someone to form on. Keepers are enrolled
        // like everyone else below, but with the well as their round's one post,
        // so their own day never drags them off the curb.
        //
        // The generated crowd (`crate::crowd`) is excluded. This is the one
        // place in the sim that hands out an authored job on nothing but "who
        // is standing nearest", and with `extra_ambient_npcs` turned up the
        // nearest ambient body to every curb in the city is a stranger minted
        // at load. Keeping the curbs to the cast is what stops a crowd knob
        // from quietly rewriting the water round.
        let mut keepers: BTreeMap<ActorId, usize> = BTreeMap::new();
        for index in 0..self.sources.len() {
            let draw_point = self.sources[index].draw_point;
            let keeper = townsfolk
                .iter()
                .filter(|id| {
                    let character = &world.characters[*id];
                    !keepers.contains_key(*id)
                        && character.significance() == Significance::Ambient
                        && !character.lore().is_some_and(|lore| lore.generated)
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
                world
                    .characters
                    .get_mut(&keeper)
                    .expect("keeper exists")
                    .state
                    .position_m = draw_point;
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
                    diagnostics.push(format!(
                        "[smart actors] round: rounds.json did not load: {error}"
                    ));
                }
                if let Err(error) = homes {
                    diagnostics.push(format!(
                        "[smart actors] round: homes.json did not load: {error}"
                    ));
                }
                None
            }
        };
        let resolver = PlaceResolver::new(nav);
        if let Some((_, homes)) = &content
            && let Some(oven) = homes.homes.get("danqn")
        {
            self.worksites.insert(
                "Ansel Quern's common oven".to_string(),
                Vec3::new(oven.point[0], WALK_Y, oven.point[1]),
            );
        }

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
            let generated = character.lore().is_some_and(|lore| lore.generated);

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
                known.extend(
                    registry
                        .ward_places(ward.as_str())
                        .map(|entry| entry.id.clone()),
                );
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
                        evening_seed: None,
                        excused: false,
                    },
                );
                enrolled += 1;
                continue;
            }

            let (legs, leash_m, curfew_exempt) = content
                .as_ref()
                .map(|(rounds, _)| {
                    build_legs(
                        rounds,
                        &resolver,
                        &self.worksites,
                        id,
                        occupation.as_deref(),
                        home,
                        base,
                    )
                })
                .unwrap_or((Vec::new(), DEFAULT_ROUND_LEASH_M, false));

            // M3 — a leash sized for a crowd. The one place it is written, and
            // the only thing about it that is conditional: a generated citizen
            // mills over 15..=40 m of their anchor instead of the ten a person
            // with no archetype gets. `build_legs` above has already handed out the
            // authored value (the route's own, the archetype's, or the
            // default), so at `extra_ambient_npcs: 0` this line never runs and
            // nothing the fixtures pin can move.
            let leash_m = if generated {
                crowd_leash_m(id)
            } else {
                leash_m
            };

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
                    let nearest = self
                        .nearest_staffed_source(base)
                        .expect("staffed is non-empty");
                    let thirst = THIRST_MAX * hash01("water_thirst_seed", id, 0);
                    world
                        .characters
                        .get_mut(id)
                        .expect("drawer exists")
                        .state
                        .needs
                        .thirst = thirst;
                    drawers += 1;
                    (Some(nearest), is_household)
                }
                _ => (None, false),
            };

            let townsperson_state = &mut world
                .characters
                .get_mut(id)
                .expect("townsperson exists")
                .state;
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
                    evening_seed: None,
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
        self.seed_food(world, nav, &resolver, &mut diagnostics);
        let time = clock.at(now);
        self.bind_vendors(world, time.weekday);
        self.restock(world, time.day);
        if !self.stalls.is_empty() {
            let bound = self
                .stalls
                .iter()
                .filter(|stall| stall.vendor.is_some())
                .count();
            diagnostics.push(format!(
                "[smart actors] round: {} food stalls, {bound} staffed on day {} ({})",
                self.stalls.len(),
                time.day,
                time.weekday.label(),
            ));
        }

        // The Stone House's standing population (`law_and_order.md` M5b). Runs
        // last, after the registry is in `world` and after every binding pass,
        // so nothing that walks people around gets a later word than this one.
        // The eight are enrolled in the round like anybody else — they have
        // legs, a home and a water source waiting for the day they are let out —
        // and the rung-0 guard in `decide` is what keeps them off their feet
        // meanwhile.
        let inmates = crate::custody::seed_authored_inmates(world);
        if inmates.is_empty() {
            // Not a fault before M5a, and worth saying out loud rather than
            // silently seeding nobody — the same reason the tavern list logs.
            diagnostics.push(format!(
                "[smart actors] gaol: no place named \"{}\" in the registry; the city's prisoners walk free",
                crate::custody::STONE_HOUSE_PLACE_NAME
            ));
        } else {
            diagnostics.push(format!(
                "[smart actors] gaol: {} held in {}",
                inmates.len(),
                crate::custody::STONE_HOUSE_PLACE_NAME
            ));
        }
        diagnostics
    }
}

// --------------------------------------------------------------------------- //
// Food stalls and supply-chain counters: binding, retained legacy restock,
// household settlement, FIFO meals, and purpose-neutral sales.
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
    /// restock, sales, production, road transitions, and settlement. The game host
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
    fn seed_food(
        &mut self,
        world: &mut World,
        nav: &NavData,
        resolver: &PlaceResolver,
        diagnostics: &mut Vec<String>,
    ) {
        let doc: FoodDoc = match serde_json::from_str(FOOD_JSON) {
            Ok(doc) => doc,
            Err(error) => {
                diagnostics.push(format!(
                    "[smart actors] round: food.json did not load: {error}"
                ));
                return;
            }
        };
        for (name, spec) in doc.trades {
            let listings_unique =
                spec.listings.iter().collect::<BTreeSet<_>>().len() == spec.listings.len();
            let valid_listings = spec.listings.iter().all(|matcher| {
                let probe = matcher.to_item(ItemId::from_raw("listing_probe"), 1);
                world.item_catalog.validate_seed_item(&probe).is_ok()
            });
            let valid_restock = spec.restock.iter().all(|stock| {
                let probe = stock
                    .matcher()
                    .to_item(ItemId::from_raw("restock_probe"), stock.quantity);
                world.item_catalog.validate_seed_item(&probe).is_ok()
                    && spec.listings.contains(&stock.matcher())
            });
            let valid_serving = spec.conjure_per_serving.as_ref().is_none_or(|kind| {
                let probe = Item::new(ItemId::from_raw("serving_probe"), kind.clone());
                world.item_catalog.validate_seed_item(&probe).is_ok()
                    && world.item_catalog.price_sparks(&probe).is_some()
            });
            if name.trim().is_empty()
                || !listings_unique
                || !valid_listings
                || !valid_restock
                || !valid_serving
            {
                diagnostics.push(format!(
                    "[smart actors] round: trade {name:?} has invalid catalog/listing content; skipped"
                ));
                continue;
            }
            self.food_trades.insert(
                name,
                ResolvedTrade {
                    occupations: spec.occupations,
                    listings: spec.listings,
                    restock: spec.restock,
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
            let offset = Vec3::new(
                site.x + spec.pitch_offset[0],
                WALK_Y,
                site.z + spec.pitch_offset[1],
            );
            // The offset must stand on pavement, or the queue forms on stone and
            // nobody can reach it — fall back to the site node itself.
            let pitch = if nav.is_walkable(offset.x, offset.z) {
                offset
            } else {
                site
            };
            self.stalls.push(FoodStall {
                name: spec.name.clone(),
                site: spec.site.clone(),
                pitch,
                trade: spec.trade.clone(),
                vendor: None,
                queue: Vec::new(),
                serving: None,
                preferred: spec
                    .preferred_vendor
                    .as_ref()
                    .map(|id| ActorId::from_raw(id.as_str())),
                open: spec.open.clone(),
                cry_next: 0.0,
            });
        }

        let resolve_site = |site: &str, anchor: Option<&ActorId>| -> Option<Vec3> {
            resolver
                .resolve(site)
                .or_else(|| self.worksites.get(site).copied())
                .or_else(|| {
                    anchor.and_then(|actor| world.places.home_of(actor).map(|place| place.point))
                })
        };
        for spec in doc.counters {
            if self.counters.contains_key(&spec.id)
                || !self.food_trades.contains_key(&spec.trade)
                || !world.characters.contains_key(&spec.preferred_actor)
                || spec.id.trim().is_empty()
                || spec.offices.is_empty()
                || spec.offices.iter().collect::<BTreeSet<_>>().len() != spec.offices.len()
                || !spec.site_radius_m.is_finite()
                || spec.site_radius_m <= 0.0
                || spec
                    .anchor_actor
                    .as_ref()
                    .is_some_and(|actor| !world.characters.contains_key(actor))
            {
                diagnostics.push(format!(
                    "[smart actors] supply counter {:?} has duplicate/missing references; skipped",
                    spec.id
                ));
                continue;
            }
            if let Some(party_id) = &spec.road_party {
                let valid = self
                    .road_parties
                    .get(party_id)
                    .is_some_and(|party| party.members.contains(&spec.preferred_actor));
                if !valid {
                    diagnostics.push(format!(
                        "[smart actors] supply counter {:?} names an invalid road party/seller; skipped",
                        spec.id
                    ));
                    continue;
                }
            }
            let Some(site) = resolve_site(&spec.site, spec.anchor_actor.as_ref()) else {
                diagnostics.push(format!(
                    "[smart actors] supply counter {:?} site {:?} is missing; skipped",
                    spec.id, spec.site
                ));
                continue;
            };
            let offset = Vec3::new(
                site.x + spec.pitch_offset[0],
                WALK_Y,
                site.z + spec.pitch_offset[1],
            );
            let pitch = if nav.is_walkable(offset.x, offset.z) {
                offset
            } else {
                site
            };
            self.counters.insert(
                spec.id.clone(),
                Counter {
                    id: spec.id,
                    trade: spec.trade,
                    site: spec.site,
                    pitch,
                    seller: spec.preferred_actor,
                    offices: spec.offices,
                    required_doing: spec.required_doing,
                    road_party: spec.road_party,
                    worksite_only: spec.worksite_only,
                    radius_m: spec.site_radius_m,
                },
            );
        }
        let referenced_groups: BTreeSet<String> = doc
            .stock_plans
            .iter()
            .filter_map(|plan| match &plan.source {
                StockSource::CounterGroup(group) => Some(group.clone()),
                StockSource::Counter(_) => None,
            })
            .collect();
        for group in doc.counter_groups {
            if self.counter_groups.contains_key(&group.id)
                || !referenced_groups.contains(&group.id)
                || group.counters.is_empty()
                || group.counters.iter().collect::<BTreeSet<_>>().len() != group.counters.len()
                || group
                    .counters
                    .iter()
                    .any(|id| !self.counters.contains_key(id))
            {
                diagnostics.push(format!(
                    "[smart actors] invalid counter group {:?}; skipped",
                    group.id
                ));
                continue;
            }
            self.counter_groups.insert(group.id, group.counters);
        }
        let mut plan_ids = BTreeSet::new();
        for plan in doc.stock_plans {
            let source_counters: Option<Vec<&Counter>> = match &plan.source {
                StockSource::Counter(counter) => {
                    self.counters.get(counter).map(|counter| vec![counter])
                }
                StockSource::CounterGroup(group) => self
                    .counter_groups
                    .get(group)
                    .map(|ids| ids.iter().filter_map(|id| self.counters.get(id)).collect()),
            };
            let valid_targets = source_counters.as_ref().is_some_and(|counters| {
                !counters.is_empty()
                    && !plan.targets.is_empty()
                    && plan.targets.iter().all(|target| {
                        target.desired_quantity > 0
                            && counters.iter().all(|counter| {
                                self.food_trades[&counter.trade]
                                    .listings
                                    .contains(&target.matcher())
                            })
                    })
            });
            if plan.id.trim().is_empty()
                || !plan_ids.insert(plan.id.clone())
                || !world.characters.contains_key(&plan.buyer)
                || plan.max_spend_sparks == 0
                || !valid_targets
                || source_counters.as_ref().is_some_and(|counters| {
                    counters.iter().any(|counter| counter.seller == plan.buyer)
                })
            {
                diagnostics.push(format!(
                    "[smart actors] stock plan {:?} has invalid actor/source/targets; skipped",
                    plan.id
                ));
                continue;
            }
            self.stock_plans.push(plan);
        }

        let mut production_producers = BTreeSet::new();
        for plan in doc.production_plans {
            if !world.characters.contains_key(&plan.producer)
                || plan.max_jobs_per_day == 0
                || !production_producers.insert(plan.producer.clone())
            {
                diagnostics.push(format!(
                    "[smart actors] production producer {} is missing, duplicated, or has a zero cap",
                    plan.producer
                ));
                continue;
            }
            let mut transforms = Vec::new();
            let mut transform_ids = BTreeSet::new();
            for transform in plan.transforms {
                let valid_recipe = !transform.id.trim().is_empty()
                    && transform_ids.insert(transform.id.clone())
                    && !transform.consumes.is_empty()
                    && !transform.produces.is_empty()
                    && !transform.allowed_offices.is_empty()
                    && transform
                        .allowed_offices
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        == transform.allowed_offices.len()
                    && transform.work_minutes > 0
                    && transform.desired_output_quantity > 0
                    && transform
                        .consumes
                        .iter()
                        .chain(&transform.produces)
                        .all(|stock| {
                            let probe = stock
                                .matcher()
                                .to_item(ItemId::from_raw("transform_spec_probe"), stock.quantity);
                            world.item_catalog.validate_seed_item(&probe).is_ok()
                        });
                if !valid_recipe {
                    diagnostics.push(format!(
                        "[smart actors] transform {:?} has invalid recipe content; skipped",
                        transform.id
                    ));
                    continue;
                }
                let Some(point) = resolve_site(&transform.site, transform.anchor_actor.as_ref())
                else {
                    diagnostics.push(format!(
                        "[smart actors] transform {:?} site {:?} is missing; skipped",
                        transform.id, transform.site
                    ));
                    continue;
                };
                let has_recurring_work_leg = Weekday::ALL.iter().copied().any(|weekday| {
                    transform.allowed_offices.iter().copied().any(|office| {
                        self.actor_on_leg_at(
                            &plan.producer,
                            office,
                            weekday,
                            &transform.site,
                            Arrival::Work,
                        )
                    })
                });
                if !has_recurring_work_leg {
                    diagnostics.push(format!(
                        "[smart actors] transform {:?} has no recurring Work leg at {:?}; skipped",
                        transform.id, transform.site
                    ));
                    continue;
                }
                transforms.push(ResolvedTransformSpec {
                    id: transform.id,
                    site: transform.site,
                    point,
                    consumes: transform.consumes,
                    produces: transform.produces,
                    allowed_offices: transform.allowed_offices,
                    work_minutes: transform.work_minutes,
                    desired_output_quantity: transform.desired_output_quantity,
                });
            }
            if !transforms.is_empty() {
                self.production_plans.push(ResolvedProductionPlan {
                    producer: plan.producer,
                    max_jobs_per_day: plan.max_jobs_per_day,
                    transforms,
                });
            }
        }

        // Ordinary residents protect their day-zero spendable purse; named
        // chain firms receive their explicit one-time working capital instead.
        for (id, actor) in &world.characters {
            if actor.state.economic_class == EconomicClass::Resident {
                self.household_reserves
                    .insert(id.clone(), world.spendable_sparks(id));
            }
        }
        let mut seeded_inventory = false;
        for (id, minimum) in doc.working_capital {
            if !world.characters.contains_key(&id) {
                diagnostics.push(format!(
                    "[smart actors] working-capital actor {id} is missing"
                ));
                continue;
            }
            let current = world.spendable_sparks(&id);
            if current < minimum {
                if let Err(error) =
                    world.credit_sparks(&id, minimum - current, &format!("working_capital:{id}"))
                {
                    diagnostics.push(format!(
                        "[smart actors] working capital for {id} failed: {error}"
                    ));
                } else {
                    seeded_inventory = true;
                }
            }
            self.household_reserves.insert(id, minimum);
        }
        for seed in doc.historical_stock {
            let stock = StockSpec {
                kind: seed.kind,
                metadata: seed.metadata,
                quantity: seed.quantity,
            };
            match world.add_stock(
                &seed.owner,
                &stock,
                &format!("historical_stock:{}", seed.owner),
            ) {
                Ok(_) => seeded_inventory = true,
                Err(error) => diagnostics.push(format!(
                    "[smart actors] historical stock for {} failed: {error}",
                    seed.owner
                )),
            }
        }
        if seeded_inventory {
            world.touch_public_state();
        }
        self.production_last_game_days = self.last_game_days;
        self.trace_cart_load_changes(world);
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
        let previous: Vec<Option<ActorId>> = self
            .stalls
            .iter()
            .map(|stall| stall.vendor.clone())
            .collect();
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
        for (s, new_vendor) in chosen.iter().enumerate() {
            if *new_vendor != self.stalls[s].vendor {
                self.release_stall(world, s);
                self.stalls[s].vendor = new_vendor.clone();
            }
        }

        // Phase 3: `you_sell` — clear it for anyone who was a vendor and now keeps
        // NO stall, then (re)write it for every current vendor. Computed only once
        // the whole set has settled, so a reassigned vendor is never wiped by the
        // stall they left. Priced off the catalog's trade template, not current
        // stock, so a sold-out baker still knows what they charge.
        let current: BTreeSet<ActorId> = self
            .stalls
            .iter()
            .filter_map(|stall| stall.vendor.clone())
            .collect();
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
            world
                .item_catalog
                .price_sparks(&item)
                .map(|price_sparks| VendorListing {
                    name: world.item_catalog.display_name(&item),
                    price_sparks,
                })
        };
        let mut listings: Vec<VendorListing> = trade
            .listings
            .iter()
            .filter_map(|spec| probe(spec.kind.as_str(), &spec.metadata))
            .collect();
        // The never-scraped pot sells a bowl it conjures per serving; it belongs
        // on the sheet exactly like a stock kind so the cook quotes the list too.
        if let Some(kind) = &trade.per_serving
            && let Some(listing) = probe(kind.as_str(), &BTreeMap::new())
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
            if taken.contains(id) || !world.is_present(id) {
                return false;
            }
            let Some(person) = self.people.get(id) else {
                return false;
            };
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
            let better = best.as_ref().is_none_or(|(best_dist, best_id)| {
                distance < *best_dist || (distance == *best_dist && id < best_id)
            });
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
                && person.food.as_ref().is_some_and(|errand| {
                    errand.stall == s && !matches!(errand.phase, FoodPhase::Eating { .. })
                })
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

    /// Strip one enrolled person's committed bodily errands — the well walk,
    /// queue place or draw; a stall visit not yet at the eating stage; a claimed
    /// weather shelter — and hand them straight back to the ladder, through the
    /// same release paths a closing stall ([`Self::release_stall`]) or a road
    /// return ([`Self::begin_road_return`]) uses, so no queue slot or `serving`
    /// entry is ever leaked.
    ///
    /// For the escort's feet (`law_and_order.md` M4): `run_ladder`'s early
    /// skips keep these errands committed *by design* — a draw or a purchase is
    /// atomic — but an officer who seizes somebody mid-errand must not carry
    /// the whole walk-queue-draw-deliver arc to completion with a prisoner in
    /// tow (session 514's cousin: the delivery leg never lays the station walk,
    /// and the 20 m poll frees a player prisoner behind it). A meal already
    /// being eaten is left to its few-second timer — the body is standing
    /// still, which is all an escort owes.
    pub(crate) fn abandon_bodily_errands(&mut self, world: &mut World, id: &ActorId) {
        let Some(person) = self.people.get(id) else {
            return;
        };
        let well_errand = matches!(
            person.phase,
            Phase::Approaching | Phase::Queued | Phase::Drawing | Phase::Returning
        );
        let stall_errand = person
            .food
            .as_ref()
            .is_some_and(|errand| !matches!(errand.phase, FoodPhase::Eating { .. }));
        let sheltering = self.weather_shelter_intents.remove(id).is_some();
        if !well_errand && !stall_errand && !sheltering {
            return;
        }
        if well_errand {
            for source in &mut self.sources {
                source.queue.retain(|queued| queued != id);
                if source
                    .serving
                    .as_ref()
                    .is_some_and(|(actor, _)| actor == id)
                {
                    source.serving = None;
                }
            }
        }
        if stall_errand {
            for stall in &mut self.stalls {
                stall.queue.retain(|queued| queued != id);
                if stall.serving.as_ref().is_some_and(|(actor, _)| actor == id) {
                    stall.serving = None;
                }
            }
        }
        let person = self.people.get_mut(id).expect("presence checked above");
        if stall_errand {
            person.food = None;
        }
        // Halt the errand's own walk — never an intent walk that happens to be
        // under way (a station `go_to` already laid is exactly the walk an
        // escort must keep).
        if well_errand {
            person.phase = Phase::Idle;
            person.travel_target = None;
            person.travel_for_intent = false;
            if let Some(character) = world.characters.get_mut(id) {
                character.state.movement = None;
            }
        } else if sheltering && person.phase == Phase::Travelling && !person.travel_for_intent {
            person.phase = Phase::Idle;
            person.travel_target = None;
            if let Some(character) = world.characters.get_mut(id) {
                character.state.movement = None;
            }
        }
        // Re-decide at once rather than waiting out the cadence: the abandoned
        // errand's owner has somewhere to be (the station walk, or plain Stay).
        person.next_decision = 0.0;
    }

    /// The retained legacy Kindling restock: sweep only quantity shares created
    /// by the same explicitly unchained source, then add its template. Real
    /// returned stock and every chained kind persist. The pot still materializes
    /// its licensed serving at purchase time; no wallet changes here.
    fn restock(&mut self, world: &mut World, day: i64) {
        if self.stalls.is_empty() {
            return;
        }
        let mut lines: Vec<String> = Vec::new();
        let mut changed = false;
        for s in 0..self.stalls.len() {
            let source_id = format!("legacy_stall:{}", self.stalls[s].name);
            match world.sweep_legacy_restock(&source_id) {
                Ok(swept) => changed |= swept > 0,
                Err(error) => {
                    self.push_food_log(format!(
                        "restock invariant failure at {}: {error}",
                        self.stalls[s].name
                    ));
                    continue;
                }
            }
            let Some(vendor) = self.stalls[s].vendor.clone() else {
                continue;
            };
            let trade_key = self.stalls[s].trade.clone();
            let trade = self.food_trades[&trade_key].clone();
            if trade.restock.is_empty() {
                let state = if trade.per_serving.is_some() {
                    "the pot"
                } else {
                    "persistent stock only"
                };
                lines.push(format!("{} ({vendor}): {state}", self.stalls[s].name));
                continue;
            }
            let mut counts: Vec<String> = Vec::new();
            for (slot, spec) in trade.restock.iter().enumerate() {
                let probe = spec
                    .matcher()
                    .to_item(ItemId::from_raw("restock_probe"), spec.quantity);
                counts.push(format!(
                    "{}× {}",
                    spec.quantity,
                    world.item_catalog.display_plural(&probe)
                ));
                if let Err(error) = world.add_legacy_restock(
                    &vendor,
                    &source_id,
                    spec,
                    &format!("legacy:{day}:{}:{slot}", self.stalls[s].name),
                ) {
                    self.push_food_log(format!(
                        "restock failed at {}: {error}",
                        self.stalls[s].name
                    ));
                } else {
                    changed = true;
                }
            }
            lines.push(format!(
                "{} ({vendor}): {}",
                self.stalls[s].name,
                counts.join(", ")
            ));
        }
        if changed {
            world.touch_public_state();
        }
        world.assert_invariants();
        self.push_food_log(format!(
            "Kindling restock, day {day} — {}",
            lines.join(" | ")
        ));
    }

    /// Watch owns the sample and completion stamps, so a missing or failed
    /// handler remains observable on the following day.
    fn dispatch_household_settlement(&mut self, world: &mut World, day: i64) {
        if self.last_household_watch_day == Some(day) {
            return;
        }
        if let Some(previous) = self.last_household_watch_day
            && self.last_household_settlement_day != Some(previous)
        {
            self.push_food_log(format!(
                "household_settlement_missed: sampled day {previous}, last completed {:?}",
                self.last_household_settlement_day
            ));
        }
        self.last_household_watch_day = Some(day);

        let residents: Vec<ActorId> = world
            .characters
            .iter()
            .filter(|(_, actor)| actor.state.economic_class == EconomicClass::Resident)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &residents {
            let streak = self.unrelieved_zero_streak.entry(id.clone()).or_default();
            if world.spendable_sparks(id) == 0 {
                *streak = streak.saturating_add(1);
            } else {
                *streak = 0;
            }
        }

        match self.settle_households(world, day) {
            Ok(receipt) => {
                self.last_household_settlement_day = Some(day);
                self.institutional_payroll_sparks = self
                    .institutional_payroll_sparks
                    .checked_add(u64::from(receipt.institutional_payroll_sparks))
                    .expect("institutional payroll accounting overflow");
                let redistributed = receipt
                    .transfers
                    .iter()
                    .try_fold(0u64, |sum, transfer| {
                        sum.checked_add(u64::from(transfer.sparks))
                    })
                    .expect("household redistribution receipt overflow");
                self.household_redistributed_sparks = self
                    .household_redistributed_sparks
                    .checked_add(redistributed)
                    .expect("household redistribution accounting overflow");
                for relieved in &receipt.relief {
                    if relieved.after_spendable >= 4 {
                        self.unrelieved_zero_streak
                            .insert(relieved.actor.clone(), 0);
                    }
                }
                let transfers = receipt
                    .transfers
                    .iter()
                    .map(|line| format!("{}->{}:{}", line.donor, line.recipient, line.sparks))
                    .collect::<Vec<_>>()
                    .join(",");
                let relief = receipt
                    .relief
                    .iter()
                    .map(|line| {
                        format!(
                            "{}:{}->{}",
                            line.actor, line.before_spendable, line.after_spendable
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                self.push_food_log(format!(
                    "household_settlement: day {}, transfers [{transfers}], recipients [{relief}], redistributed {redistributed}, payroll_minted {}",
                    receipt.day, receipt.institutional_payroll_sparks
                ));
            }
            Err(error) => {
                self.push_food_log(format!("household_settlement_failed: day {day}: {error}"))
            }
        }
    }

    /// Deterministic resident-only redistribution followed by only the exact
    /// residual wages/alms mint. Applying it to a cloned world gives the whole
    /// multi-purse plan atomic rollback semantics.
    pub fn settle_households(
        &self,
        world: &mut World,
        day: i64,
    ) -> Result<HouseholdSettlementReceipt, crate::InventoryError> {
        let residents: Vec<ActorId> = world
            .characters
            .iter()
            .filter(|(_, actor)| actor.state.economic_class == EconomicClass::Resident)
            .map(|(id, _)| id.clone())
            .collect();
        let before: BTreeMap<ActorId, u32> = residents
            .iter()
            .map(|id| (id.clone(), world.spendable_sparks(id)))
            .collect();
        let mut needs: Vec<(ActorId, u32)> = before
            .iter()
            .filter(|(_, sparks)| **sparks < 4)
            .map(|(id, sparks)| (id.clone(), 4 - *sparks))
            .collect();
        let mut donors: Vec<(ActorId, u32)> = before
            .iter()
            .filter_map(|(id, sparks)| {
                let reserve = self
                    .household_reserves
                    .get(id)
                    .copied()
                    .unwrap_or(*sparks)
                    .max(4);
                (*sparks > reserve).then(|| (id.clone(), *sparks - reserve))
            })
            .collect();
        needs.sort_by(|left, right| left.0.cmp(&right.0));
        donors.sort_by(|left, right| left.0.cmp(&right.0));

        let mut staged = world.clone();
        let mut transfers = Vec::new();
        let mut donor_index = 0usize;
        for (recipient, need) in &mut needs {
            while *need > 0 && donor_index < donors.len() {
                let (donor, available) = &mut donors[donor_index];
                if *available == 0 {
                    donor_index += 1;
                    continue;
                }
                let amount = (*need).min(*available);
                staged.debit_sparks(donor, amount)?;
                staged.credit_sparks(
                    recipient,
                    amount,
                    &format!("settlement:{day}:{donor}:{recipient}"),
                )?;
                transfers.push(HouseholdTransfer {
                    donor: donor.clone(),
                    recipient: recipient.clone(),
                    sparks: amount,
                });
                *need -= amount;
                *available -= amount;
            }
        }
        let payroll = needs
            .iter()
            .try_fold(0u32, |sum, (_, need)| sum.checked_add(*need))
            .ok_or_else(|| {
                crate::InventoryError::new(
                    crate::InventoryErrorCode::ArithmeticOverflow,
                    "household payroll total overflow",
                )
            })?;
        for (recipient, need) in &needs {
            if *need > 0 {
                staged.credit_sparks(
                    recipient,
                    *need,
                    &format!("settlement:{day}:payroll:{recipient}"),
                )?;
            }
        }
        let relief = before
            .iter()
            .filter(|(_, amount)| **amount < 4)
            .map(|(actor, amount)| HouseholdRelief {
                actor: actor.clone(),
                before_spendable: *amount,
                after_spendable: staged.spendable_sparks(actor),
            })
            .collect::<Vec<_>>();
        if !transfers.is_empty() || payroll > 0 {
            staged.touch_public_state();
        }
        *world = staged;
        Ok(HouseholdSettlementReceipt {
            day,
            transfers,
            relief,
            institutional_payroll_sparks: payroll,
        })
    }

    fn actor_on_leg(
        &self,
        actor: &ActorId,
        time: crate::clock::WorldTime,
        site: &str,
        doing: Arrival,
    ) -> bool {
        self.actor_on_leg_at(actor, time.office, time.weekday, site, doing)
    }

    fn actor_on_leg_at(
        &self,
        actor: &ActorId,
        office: Office,
        weekday: Weekday,
        site: &str,
        doing: Arrival,
    ) -> bool {
        if let Some(person) = self.people.get(actor) {
            return active_leg(&person.legs, office, weekday)
                .is_some_and(|leg| leg.label == site && leg.doing == doing);
        }
        self.road_parties.values().any(|party| {
            party.members.contains(actor)
                && party.state.phase == PartyPhase::InCity
                && active_leg(&party.legs, office, weekday)
                    .is_some_and(|leg| leg.label == site && leg.doing == doing)
        })
    }

    fn counter_binding(
        &self,
        world: &World,
        counter_id: &str,
        time: crate::clock::WorldTime,
    ) -> Option<CounterBindingKey> {
        let counter = self.counters.get(counter_id)?;
        if counter.worksite_only
            || !counter.offices.contains(&time.office)
            || !world.is_present(&counter.seller)
            || world.characters[&counter.seller]
                .position_m()
                .distance(counter.pitch)
                > counter.radius_m
            || !self.actor_on_leg(&counter.seller, time, &counter.site, counter.required_doing)
        {
            return None;
        }
        let session = if let Some(party_id) = &counter.road_party {
            let party = self.road_parties.get(party_id)?;
            if !party.members.contains(&counter.seller) || party.state.phase != PartyPhase::InCity {
                return None;
            }
            CounterSession::RoadTrip {
                party_id: party_id.clone(),
                trip_number: party.state.trip_number,
            }
        } else {
            CounterSession::Daily {
                absolute_day: time.day,
            }
        };
        Some(CounterBindingKey {
            counter_id: counter.id.clone(),
            seller: counter.seller.clone(),
            session,
        })
    }

    fn source_binding(
        &self,
        world: &World,
        source: &StockSource,
        time: crate::clock::WorldTime,
    ) -> Option<CounterBindingKey> {
        match source {
            StockSource::Counter(id) => self.counter_binding(world, id, time),
            StockSource::CounterGroup(id) => self
                .counter_groups
                .get(id)?
                .iter()
                .find_map(|counter| self.counter_binding(world, counter, time)),
        }
    }

    fn binding_still_viable(
        &self,
        binding: &CounterBindingKey,
        time: crate::clock::WorldTime,
    ) -> bool {
        let Some(counter) = self.counters.get(&binding.counter_id) else {
            return false;
        };
        if counter.seller != binding.seller
            || !counter.offices.iter().any(|office| *office >= time.office)
        {
            return false;
        }
        match &binding.session {
            CounterSession::Daily { absolute_day } => *absolute_day == time.day,
            CounterSession::RoadTrip {
                party_id,
                trip_number,
            } => self.road_parties.get(party_id).is_some_and(|party| {
                party.state.trip_number == *trip_number
                    && party.state.phase == PartyPhase::InCity
                    && party.members.contains(&binding.seller)
            }),
        }
    }

    fn expired_binding_reason(
        &self,
        binding: &CounterBindingKey,
        time: crate::clock::WorldTime,
    ) -> MarketVisitEnd {
        let office_passed = self
            .counters
            .get(&binding.counter_id)
            .is_some_and(|counter| {
                !counter.offices.iter().any(|office| *office >= time.office)
                    || matches!(
                        binding.session,
                        CounterSession::Daily { absolute_day } if absolute_day < time.day
                    )
            });
        if office_passed {
            MarketVisitEnd::LastOfficePassed
        } else {
            MarketVisitEnd::SourceIneligible
        }
    }

    fn market_attempt_fingerprint(
        &self,
        world: &World,
        plan: &StockPlanSpec,
        binding: &CounterBindingKey,
        office: Office,
        spent: u32,
        remaining_budget: u32,
    ) -> String {
        let counter = &self.counters[&binding.counter_id];
        let trade = &self.food_trades[&counter.trade];
        let mut source = Vec::new();
        for item_id in world.characters[&binding.seller].holds() {
            let Some(item) = world.items.get(item_id) else {
                continue;
            };
            if trade.listings.iter().any(|matcher| matcher.matches(item)) {
                source.push(format!("{item_id}:{}", world.uncommitted_quantity(item_id)));
            }
        }
        source.sort();
        let buyer = plan
            .targets
            .iter()
            .map(|target| {
                format!(
                    "{}:{:?}:{}",
                    target.kind,
                    target.metadata,
                    world.held_quantity(&plan.buyer, &target.matcher())
                )
            })
            .collect::<Vec<_>>();
        format!(
            "{:?}|{office:?}|src={}|buyer={}|funds={}|spent={spent}|remaining={remaining_budget}",
            binding,
            source.join(","),
            buyer.join(","),
            world.spendable_sparks(&plan.buyer),
        )
    }

    fn stock_plan_satisfied(&self, world: &World, plan: &StockPlanSpec) -> bool {
        plan.targets.iter().all(|target| {
            world.held_quantity(&plan.buyer, &target.matcher()) >= target.desired_quantity
        })
    }

    fn stock_errand_trace(
        &self,
        plan: &StockPlanSpec,
        errand: &MarketErrand,
        result: &str,
        end_reason: Option<MarketVisitEnd>,
    ) -> String {
        let remaining = plan.max_spend_sparks.saturating_sub(errand.spent_sparks);
        let next_retry = if end_reason.is_some() {
            "none"
        } else if errand.last_failed_fingerprint.is_some() {
            "fingerprint_change"
        } else {
            "next_tick"
        };
        format!(
            "stock_errand: plan {}, buyer {}, source {:?}, selected {:?}, bindings_seen {:?}, phase {:?}, spent {}, remaining {}, result {result}, fingerprint {:?}, visit_end {:?}, next_retry {next_retry}",
            plan.id,
            plan.buyer,
            plan.source,
            errand.selected,
            errand.bindings_seen,
            errand.phase,
            errand.spent_sparks,
            remaining,
            errand.last_failed_fingerprint,
            end_reason,
        )
    }

    fn finish_market_errand(&mut self, world: &mut World, buyer: &ActorId, reason: MarketVisitEnd) {
        let Some(errand) = self.market_errands.remove(buyer) else {
            return;
        };
        let trace = self
            .stock_plans
            .iter()
            .find(|plan| plan.id == errand.plan_id)
            .map(|plan| self.stock_errand_trace(plan, &errand, "visit_ended", Some(reason)));
        self.closed_market_visits.insert(
            errand.plan_id.clone(),
            ClosedMarketVisit {
                plan_id: errand.plan_id.clone(),
                bindings_seen: errand.bindings_seen.clone(),
                end_reason: reason,
            },
        );
        let counter_pitch = errand
            .selected
            .as_ref()
            .and_then(|binding| self.counters.get(&binding.counter_id))
            .map(|counter| counter.pitch);
        let was_stock_walk = self.people.get(buyer).map_or(
            errand.phase == MarketErrandPhase::Approaching,
            |person| {
                person.phase == Phase::Travelling
                    && !person.travel_for_intent
                    && counter_pitch.is_some_and(|pitch| person.travel_target == Some(pitch))
            },
        );
        if was_stock_walk && let Some(person) = self.people.get_mut(buyer) {
            person.phase = Phase::Idle;
            person.travel_target = None;
        }
        if was_stock_walk && let Some(actor) = world.characters.get_mut(buyer) {
            actor.state.movement = None;
        }
        if let Some(trace) = trace {
            self.push_food_log(trace);
        }
    }

    /// Note the moment a legitimate hold — a conversation, a pressing rung,
    /// the law — takes the errand's body, so the travel deadline can be pushed
    /// forward by exactly the held span when the walk resumes
    /// ([`MarketErrand::deadline_hold_began_real`]). Idempotent across the
    /// ticks a hold lasts: only the first tick's `now` is kept.
    fn hold_stock_travel_deadline(&mut self, buyer: &ActorId, now: f64) {
        if let Some(errand) = self.market_errands.get_mut(buyer) {
            errand.deadline_hold_began_real.get_or_insert(now);
        }
    }

    fn tick_stock_plans(
        &mut self,
        world: &mut World,
        nav: &NavData,
        time: crate::clock::WorldTime,
        now: f64,
        in_conversation: &BTreeSet<ActorId>,
    ) {
        let plans = self.stock_plans.clone();
        for plan in plans {
            if !world.is_present(&plan.buyer) {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::SourceIneligible);
                continue;
            }
            if world.characters[&plan.buyer].state.leaving_city {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::Returning);
                continue;
            }
            // The law has the buyer (M4/M5): in charge they are walked by their
            // escort, committed they may not leave the threshold — either way
            // no counter visit survives the seizure, and no fresh one opens
            // while it stands. Finished rather than frozen, so the errand's
            // queue slot and phase go back through the ordinary door instead of
            // the plan re-targeting a body `set_route` will refuse every tick.
            if world.custody.holds(&plan.buyer) {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::SourceIneligible);
                continue;
            }
            // The errand ledger is keyed by buyer, and nothing above promises
            // one plan per buyer — the loader checks plan ids for uniqueness,
            // not buyers. Everything past the buyer-level checks above is the
            // errand-owner's own business: its satisfaction, its hold
            // markers, its travel deadline. A plan whose buyer is out on some
            // other plan's errand waits its turn here rather than finishing a
            // visit it never opened or consuming bookkeeping it does not own.
            if self
                .market_errands
                .get(&plan.buyer)
                .is_some_and(|errand| errand.plan_id != plan.id)
            {
                continue;
            }
            if in_conversation.contains(&plan.buyer) {
                self.hold_stock_travel_deadline(&plan.buyer, now);
                continue;
            }
            if self.stock_plan_satisfied(world, &plan) {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::TargetsSatisfied);
                continue;
            }
            // A player/model-authored route supersedes the mechanical errand.
            if world.characters[&plan.buyer].state.intent.is_some() {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::ReplacedByGoTo);
                continue;
            }
            // An escort opens no stock visit. Seizing sets the station intent,
            // so the branch above normally ends a live visit — but in the gap
            // where that intent is down (a burned budget, the closing chase
            // just grabbed) a fresh visit would walk the officer to a counter
            // with a prisoner on the leash. Freeze rather than finish: the
            // custody poll re-lays the intent, and the branch above then closes
            // the visit through its ordinary door.
            if world.custody.is_escorting(&plan.buyer) {
                self.hold_stock_travel_deadline(&plan.buyer, now);
                continue;
            }
            if let Some(person) = self.people.get(&plan.buyer) {
                let committed_need = person.food.is_some()
                    || matches!(
                        person.phase,
                        Phase::Approaching | Phase::Queued | Phase::Drawing | Phase::Returning
                    );
                let pressing = decide(
                    self,
                    world,
                    nav,
                    &plan.buyer,
                    person.epoch,
                    time.office,
                    time.weekday,
                )
                .1
                .is_some();
                let lightning_reflex = self
                    .lightning_reflex_until
                    .get(&plan.buyer)
                    .is_some_and(|until| now < *until);
                if committed_need || pressing || lightning_reflex {
                    self.hold_stock_travel_deadline(&plan.buyer, now);
                    continue;
                }
            }
            // The walk is the errand's own again: hand whatever span a
            // legitimate diversion held the body back to the deadline, so the
            // famished rung firing mid-walk cannot burn the visit down to
            // `TravelExpired` and forfeit the binding for the rest of the day.
            // The deadline stays meaningful — it still times exactly the
            // walking the errand itself does.
            if let Some(errand) = self.market_errands.get_mut(&plan.buyer)
                && let Some(began) = errand.deadline_hold_began_real.take()
                && let Some(deadline) = &mut errand.travel_deadline_real
            {
                *deadline += now - began;
            }
            let binding = self.source_binding(world, &plan.source, time);
            if !self.market_errands.contains_key(&plan.buyer) {
                let Some(binding) = binding.clone() else {
                    continue;
                };
                if self
                    .closed_market_visits
                    .get(&plan.id)
                    .is_some_and(|closed| closed.bindings_seen.contains(&binding))
                {
                    continue;
                }
                self.market_errands.insert(
                    plan.buyer.clone(),
                    MarketErrand {
                        plan_id: plan.id.clone(),
                        selected: Some(binding.clone()),
                        bindings_seen: vec![binding],
                        phase: MarketErrandPhase::Approaching,
                        spent_sparks: 0,
                        last_failed_fingerprint: None,
                        travel_deadline_real: None,
                        deadline_hold_began_real: None,
                    },
                );
                let trace = self.stock_errand_trace(
                    &plan,
                    &self.market_errands[&plan.buyer],
                    "visit_started",
                    None,
                );
                self.push_food_log(trace);
            }
            let prior_binding = self
                .market_errands
                .get(&plan.buyer)
                .and_then(|errand| errand.selected.as_ref().or(errand.bindings_seen.last()))
                .cloned();
            if let Some(prior) = &prior_binding
                && binding.as_ref() != Some(prior)
                && !self.binding_still_viable(prior, time)
            {
                // An explicitly interchangeable group may switch from a cart
                // that just unbound to another concrete binding without
                // manufacturing a fresh visit or budget. With no replacement,
                // the old session ends normally and is remembered closed.
                let can_reselect_group =
                    matches!(&plan.source, StockSource::CounterGroup(_)) && binding.is_some();
                if !can_reselect_group {
                    let reason = self.expired_binding_reason(prior, time);
                    self.finish_market_errand(world, &plan.buyer, reason);
                    continue;
                }
            }
            let mut selection_changed = false;
            {
                let Some(errand) = self.market_errands.get_mut(&plan.buyer) else {
                    continue;
                };
                if binding != errand.selected {
                    selection_changed = true;
                    // A seller who merely steps off the pitch clears `selected`
                    // without resetting the visit (07_the_supply_chain.md:
                    // "rebinding resumes it with the same spent budget"), and
                    // that survival must include the walk's own clock: nulling
                    // the deadline here and re-stamping it on return handed a
                    // fresh travel budget to every flicker of the seller's
                    // presence, so a genuinely stuck walk toward an oscillating
                    // seller churned all day without ever tripping
                    // `TravelExpired`. Only a truly different selection —
                    // another cart of the group, a new session — starts a new
                    // walk with a new budget; the same binding going absent and
                    // coming back keeps the deadline it had, held (below) for
                    // exactly the span the buyer stood waiting.
                    let same_binding_paused = match (&binding, &errand.selected) {
                        // Going absent: reaching here with a prior selection and
                        // no binding means `binding_still_viable` vouched for it
                        // above, else the visit would already have ended.
                        (None, Some(_)) => true,
                        // Coming back: the very key the walk was laid toward.
                        (Some(next), None) => errand.bindings_seen.last() == Some(next),
                        _ => false,
                    };
                    if !same_binding_paused {
                        errand.travel_deadline_real = None;
                        errand.deadline_hold_began_real = None;
                    }
                    errand.selected = binding.clone();
                    if let Some(binding) = &binding
                        && !errand.bindings_seen.contains(binding)
                    {
                        errand.bindings_seen.push(binding.clone());
                    }
                }
            }
            if selection_changed {
                world
                    .characters
                    .get_mut(&plan.buyer)
                    .expect("stock buyer exists")
                    .state
                    .movement = None;
                if let Some(person) = self.people.get_mut(&plan.buyer) {
                    person.phase = Phase::Idle;
                    person.travel_target = None;
                }
                if binding.is_some() {
                    let trace = self.stock_errand_trace(
                        &plan,
                        &self.market_errands[&plan.buyer],
                        "binding_changed",
                        None,
                    );
                    self.push_food_log(trace);
                }
            }
            let Some(binding) = binding else {
                let phase_changed = {
                    let errand = self
                        .market_errands
                        .get_mut(&plan.buyer)
                        .expect("errand exists");
                    let changed = errand.phase != MarketErrandPhase::WaitingForOpen;
                    errand.phase = MarketErrandPhase::WaitingForOpen;
                    // The wait is the seller's doing, not the walk's: freeze
                    // the travel clock for as long as the source is absent.
                    // Re-armed every tick because the release above pays each
                    // held slice out incrementally; the sum is the whole span
                    // stood waiting.
                    errand.deadline_hold_began_real.get_or_insert(now);
                    changed
                };
                if selection_changed || phase_changed {
                    let result = prior_binding
                        .as_ref()
                        .and_then(|prior| self.counters.get(&prior.counter_id))
                        .map_or("source_absent", |counter| {
                            if counter.offices.contains(&time.office) {
                                "source_absent"
                            } else {
                                "closed"
                            }
                        });
                    let trace = self.stock_errand_trace(
                        &plan,
                        &self.market_errands[&plan.buyer],
                        result,
                        None,
                    );
                    self.push_food_log(trace);
                }
                continue;
            };
            let counter = self.counters[&binding.counter_id].clone();
            let position = world.characters[&plan.buyer].position_m();
            if position.distance(counter.pitch) > counter.radius_m {
                let deadline = {
                    let errand = self
                        .market_errands
                        .get_mut(&plan.buyer)
                        .expect("errand exists");
                    errand.phase = MarketErrandPhase::Approaching;
                    errand.travel_deadline_real
                };
                if deadline.is_some_and(|deadline| now >= deadline) {
                    self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::TravelExpired);
                    continue;
                }
                if !world.characters[&plan.buyer].is_walking() {
                    let Some(path) = route_path_to_point(nav, &plan.buyer, position, counter.pitch)
                    else {
                        self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::NoRoute);
                        continue;
                    };
                    let metres = path
                        .windows(2)
                        .map(|pair| pair[0].distance(pair[1]))
                        .sum::<f64>();
                    self.market_errands
                        .get_mut(&plan.buyer)
                        .expect("errand exists")
                        .travel_deadline_real
                        .get_or_insert(
                            now + (GO_TO_BUDGET_FACTOR * metres / WALK_SPEED_MPS)
                                .max(GO_TO_MIN_BUDGET_SECONDS),
                        );
                    set_route(world, &plan.buyer, path);
                    if let Some(person) = self.people.get_mut(&plan.buyer) {
                        person.phase = Phase::Travelling;
                        person.travel_target = Some(counter.pitch);
                    }
                }
                continue;
            }
            world
                .characters
                .get_mut(&plan.buyer)
                .expect("buyer exists")
                .state
                .movement = None;
            self.market_errands
                .get_mut(&plan.buyer)
                .expect("errand exists")
                .phase = MarketErrandPhase::AtCounter;
            let spent_before = self.market_errands[&plan.buyer].spent_sparks;
            let remaining_budget = plan
                .max_spend_sparks
                .checked_sub(spent_before)
                .expect("a market visit never spends beyond its validated cap");
            if remaining_budget == 0 {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::BudgetExhausted);
                continue;
            }
            let Some(trade) = self.food_trades.get(&counter.trade) else {
                continue;
            };
            let requested: Vec<MarketRequestLine> = plan
                .targets
                .iter()
                .filter_map(|target| {
                    let matcher = target.matcher();
                    if !trade.listings.contains(&matcher) {
                        return None;
                    }
                    let held = world.held_quantity(&plan.buyer, &matcher);
                    (held < target.desired_quantity).then_some(MarketRequestLine {
                        matcher,
                        quantity: target.desired_quantity - held,
                    })
                })
                .collect();
            if requested.is_empty() {
                self.finish_market_errand(world, &plan.buyer, MarketVisitEnd::TargetsSatisfied);
                continue;
            }
            let fingerprint = self.market_attempt_fingerprint(
                world,
                &plan,
                &binding,
                time.office,
                spent_before,
                remaining_budget,
            );
            if self.market_errands[&plan.buyer]
                .last_failed_fingerprint
                .as_ref()
                == Some(&fingerprint)
            {
                continue;
            }
            let operation = format!(
                "stock:{}:{}:{}:{}",
                plan.id, time.day, binding.counter_id, spent_before
            );
            match world.market_sale(
                &plan.buyer,
                &binding.seller,
                &requested,
                remaining_budget,
                &operation,
            ) {
                Ok(receipt) => {
                    let new_spent = {
                        let errand = self
                            .market_errands
                            .get_mut(&plan.buyer)
                            .expect("errand exists");
                        errand.spent_sparks = spent_before
                            .checked_add(receipt.total_sparks)
                            .expect("sale cannot exceed the checked visit budget");
                        errand.last_failed_fingerprint = None;
                        errand.spent_sparks
                    };
                    self.push_food_log(sale_receipt_trace(&receipt, &binding.counter_id));
                    let trace = self.stock_errand_trace(
                        &plan,
                        &self.market_errands[&plan.buyer],
                        "sale_committed",
                        None,
                    );
                    self.push_food_log(trace);
                    if self.stock_plan_satisfied(world, &plan) {
                        self.finish_market_errand(
                            world,
                            &plan.buyer,
                            MarketVisitEnd::TargetsSatisfied,
                        );
                    } else if new_spent >= plan.max_spend_sparks {
                        self.finish_market_errand(
                            world,
                            &plan.buyer,
                            MarketVisitEnd::BudgetExhausted,
                        );
                    }
                }
                Err(error) => {
                    let should_log = self.market_errands.get(&plan.buyer).is_some_and(|errand| {
                        errand.last_failed_fingerprint.as_ref() != Some(&fingerprint)
                    });
                    if should_log {
                        self.market_errands
                            .get_mut(&plan.buyer)
                            .expect("errand exists")
                            .last_failed_fingerprint = Some(fingerprint);
                        let trace = self.stock_errand_trace(
                            &plan,
                            &self.market_errands[&plan.buyer],
                            error.code.as_str(),
                            None,
                        );
                        self.push_food_log(trace);
                    }
                    if error.code == crate::InventoryErrorCode::UnpricedStock {
                        self.finish_market_errand(
                            world,
                            &plan.buyer,
                            MarketVisitEnd::UnpricedStock,
                        );
                    } else if error.code == crate::InventoryErrorCode::BudgetExhausted {
                        self.finish_market_errand(
                            world,
                            &plan.buyer,
                            MarketVisitEnd::BudgetExhausted,
                        );
                    }
                }
            }
        }
    }

    fn production_dynamically_eligible(
        &self,
        world: &World,
        plan: &ResolvedProductionPlan,
        transform: &ResolvedTransformSpec,
        in_conversation: &BTreeSet<ActorId>,
    ) -> bool {
        world.is_present(&plan.producer)
            && !world.characters[&plan.producer].is_walking()
            && world.characters[&plan.producer]
                .position_m()
                .distance(transform.point)
                <= ROUND_ARRIVE_RADIUS_M
            && !in_conversation.contains(&plan.producer)
            && !self.market_errands.contains_key(&plan.producer)
            && self
                .people
                .get(&plan.producer)
                .is_none_or(|person| person.food.is_none())
            && world.characters[&plan.producer].state.intent.is_none()
    }

    fn production_start_eligible(
        &self,
        world: &World,
        plan: &ResolvedProductionPlan,
        transform: &ResolvedTransformSpec,
        time: crate::clock::WorldTime,
        in_conversation: &BTreeSet<ActorId>,
    ) -> bool {
        self.production_dynamically_eligible(world, plan, transform, in_conversation)
            && transform.allowed_offices.contains(&time.office)
            && self.actor_on_leg(&plan.producer, time, &transform.site, Arrival::Work)
    }

    /// Exact game minutes in `(from, to]` for which this transform's office is
    /// open and the producer's authored round says Work at this site. Civil
    /// midnight is a boundary too: the Snuffing spans it, but a weekday-only
    /// leg changes day there.
    fn production_overlap_minutes(
        &self,
        plan: &ResolvedProductionPlan,
        transform: &ResolvedTransformSpec,
        from: f64,
        to: f64,
    ) -> f64 {
        if !from.is_finite() || !to.is_finite() || to <= from {
            return 0.0;
        }
        let first_day = from.floor() as i64;
        let last_day = to.floor() as i64;
        let mut overlap_days = 0.0;
        for day in first_day..=last_day {
            let weekday = Weekday::of_day(day);
            let mut start_fraction = 0.0;
            let mut office = Office::Snuffing;
            for next_office in Office::ALL {
                let end_fraction = next_office.start_fraction();
                if transform.allowed_offices.contains(&office)
                    && self.actor_on_leg_at(
                        &plan.producer,
                        office,
                        weekday,
                        &transform.site,
                        Arrival::Work,
                    )
                {
                    let start = day as f64 + start_fraction;
                    let end = day as f64 + end_fraction;
                    overlap_days += (to.min(end) - from.max(start)).max(0.0);
                }
                start_fraction = end_fraction;
                office = next_office;
            }
            if transform.allowed_offices.contains(&office)
                && self.actor_on_leg_at(
                    &plan.producer,
                    office,
                    weekday,
                    &transform.site,
                    Arrival::Work,
                )
            {
                let start = day as f64 + start_fraction;
                let end = day as f64 + 1.0;
                overlap_days += (to.min(end) - from.max(start)).max(0.0);
            }
        }
        overlap_days * 24.0 * 60.0
    }

    fn tick_production(
        &mut self,
        world: &mut World,
        clock: &WorldClock,
        now: f64,
        in_conversation: &BTreeSet<ActorId>,
    ) {
        let current_days = clock.game_days(now);
        let previous_days = self.production_last_game_days;
        self.production_last_game_days = current_days;
        let time = clock.at(now);
        let plans = self.production_plans.clone();
        for plan in plans {
            let was_eligible = self
                .production_was_eligible
                .get(&plan.producer)
                .copied()
                .unwrap_or(false);
            if let Some(job) = world.active_transform_job(&plan.producer).cloned()
                && let Some(transform) = plan.transforms.iter().find(|spec| spec.id == job.spec_id)
            {
                let eligible_now =
                    self.production_dynamically_eligible(world, &plan, transform, in_conversation);
                if eligible_now && was_eligible {
                    let minutes = self.production_overlap_minutes(
                        &plan,
                        transform,
                        previous_days,
                        current_days,
                    );
                    if minutes > 0.0
                        && let Some(active) = world.active_transform_job_mut(&plan.producer)
                    {
                        active.progress_work_minutes += minutes;
                    }
                } else if !eligible_now && was_eligible {
                    self.push_food_log(format!(
                        "transform_pause: producer {}, spec {}, job {}, production_day {}, reserved [{}], future_outputs [{}], progress {:.3}/{:.3} work_minutes",
                        plan.producer,
                        transform.id,
                        job.job_id,
                        job.production_day,
                        reserved_inputs_trace(&job.inputs),
                        stock_specs_trace(&job.outputs),
                        job.progress_work_minutes,
                        transform.work_minutes,
                    ));
                }
                if eligible_now
                    && world
                        .active_transform_job(&plan.producer)
                        .is_some_and(|active| {
                            active.progress_work_minutes >= f64::from(transform.work_minutes)
                        })
                {
                    let completed_work_minutes = world
                        .active_transform_job(&plan.producer)
                        .map(|active| active.progress_work_minutes)
                        .unwrap_or(job.progress_work_minutes);
                    match world.complete_transform_job_by_id(&plan.producer, &job.job_id, time.day)
                    {
                        Ok(receipt) => {
                            world.touch_public_state();
                            self.push_food_log(format!(
                                "transform_finish: producer {}, spec {}, job {}, production_day {}, completion_day {}, work_minutes {:.3}/{}, consumed [{}], produced [{}]",
                                plan.producer,
                                transform.id,
                                receipt.job_id,
                                job.production_day,
                                receipt.completed_on_day,
                                completed_work_minutes,
                                transform.work_minutes,
                                transform_receipt_lines_trace(&receipt.consumed),
                                transform_receipt_lines_trace(&receipt.produced),
                            ));
                        }
                        Err(error) => self.push_food_log(format!(
                            "transform_finish_failed: producer {}, spec {}: {error}",
                            plan.producer, transform.id
                        )),
                    }
                }
            }
            if world.active_transform_job(&plan.producer).is_some() {
                let eligible_at_end = world
                    .active_transform_job(&plan.producer)
                    .and_then(|job| plan.transforms.iter().find(|spec| spec.id == job.spec_id))
                    .is_some_and(|transform| {
                        self.production_dynamically_eligible(
                            world,
                            &plan,
                            transform,
                            in_conversation,
                        )
                    });
                self.production_was_eligible
                    .insert(plan.producer.clone(), eligible_at_end);
                continue;
            }
            let starts = self
                .production_starts
                .get(&(plan.producer.clone(), time.day))
                .copied()
                .unwrap_or(0);
            if starts >= plan.max_jobs_per_day {
                self.production_was_eligible
                    .insert(plan.producer.clone(), false);
                continue;
            }
            for transform in &plan.transforms {
                if !self.production_start_eligible(world, &plan, transform, time, in_conversation) {
                    continue;
                }
                let mut batch_outputs = BTreeMap::<ItemMatcher, u32>::new();
                let batch_is_valid = transform.produces.iter().all(|output| {
                    let quantity = batch_outputs.entry(output.matcher()).or_default();
                    if let Some(total) = quantity.checked_add(output.quantity) {
                        *quantity = total;
                        true
                    } else {
                        false
                    }
                });
                let output_fits_target = batch_is_valid
                    && batch_outputs.iter().all(|(matcher, batch_quantity)| {
                        world
                            .held_quantity(&plan.producer, matcher)
                            .checked_add(world.future_output_quantity(&plan.producer, matcher))
                            .and_then(|quantity| quantity.checked_add(*batch_quantity))
                            .is_some_and(|quantity| quantity <= transform.desired_output_quantity)
                    });
                if !output_fits_target {
                    continue;
                }
                let mut reserved = Vec::new();
                let mut complete = true;
                for input in &transform.consumes {
                    let matcher = input.matcher();
                    let mut remaining = input.quantity;
                    let mut ids = world.characters[&plan.producer].holds().to_vec();
                    ids.sort();
                    for id in ids {
                        if remaining == 0 {
                            break;
                        }
                        if !world
                            .items
                            .get(&id)
                            .is_some_and(|item| matcher.matches(item))
                        {
                            continue;
                        }
                        let take = remaining.min(world.uncommitted_quantity(&id));
                        if take > 0 {
                            reserved.push(ReservedInput {
                                item_id: id,
                                quantity: take,
                            });
                            remaining -= take;
                        }
                    }
                    if remaining > 0 {
                        complete = false;
                        break;
                    }
                }
                if !complete {
                    continue;
                }
                let start_slot = starts;
                let job_id = format!(
                    "{}:{}:{}:{}",
                    plan.producer, transform.id, time.day, start_slot
                );
                let job = TransformJob {
                    job_id: job_id.clone(),
                    spec_id: transform.id.clone(),
                    producer: plan.producer.clone(),
                    production_day: time.day,
                    start_slot,
                    inputs: reserved,
                    outputs: transform.produces.clone(),
                    progress_work_minutes: 0.0,
                };
                let reserved_trace = reserved_inputs_trace(&job.inputs);
                let outputs_trace = stock_specs_trace(&job.outputs);
                match world.start_transform_job(job) {
                    Ok(()) => {
                        self.production_starts
                            .insert((plan.producer.clone(), time.day), starts + 1);
                        world.touch_public_state();
                        self.push_food_log(format!(
                            "transform_start: producer {}, spec {}, job {job_id}, production_day {}, start_slot {start_slot}, reserved [{reserved_trace}], future_outputs [{outputs_trace}], work_minutes {}",
                            plan.producer, transform.id, time.day, transform.work_minutes
                        ));
                    }
                    Err(error) => self.push_food_log(format!(
                        "transform_start_failed: producer {}, spec {}: {error}",
                        plan.producer, transform.id
                    )),
                }
                break;
            }
            let eligible_at_end = world
                .active_transform_job(&plan.producer)
                .and_then(|job| plan.transforms.iter().find(|spec| spec.id == job.spec_id))
                .is_some_and(|transform| {
                    self.production_dynamically_eligible(world, &plan, transform, in_conversation)
                });
            self.production_was_eligible
                .insert(plan.producer.clone(), eligible_at_end);
        }
        self.production_starts
            .retain(|(_, day), _| *day >= time.day.saturating_sub(1));
        self.sweep_stranded_transforms(world, current_days);
        world.prune_completed_transforms(time.day);
    }

    /// The safety net under every way a job can lose its worker: abandon any
    /// live transform whose progress figure has not moved for
    /// [`TRANSFORM_ABANDON_GAME_DAYS`], releasing its reserved inputs back to
    /// uncommitted stock ([`World::abandon_transform_job`]). Walked over the
    /// world's whole job set rather than the plan list, so a producer whose
    /// plan has vanished is still covered. The plan loop above pauses a job
    /// honestly (`transform_pause`) but can never end one — completion is the
    /// only other exit — so without this sweep a job stranded by a moved Work
    /// leg would hold its flour committed until the end of the world.
    fn sweep_stranded_transforms(&mut self, world: &mut World, current_days: f64) {
        let jobs: Vec<(ActorId, String, f64)> = world
            .transform_jobs()
            .map(|job| {
                (
                    job.producer.clone(),
                    job.job_id.clone(),
                    job.progress_work_minutes,
                )
            })
            .collect();
        // A watch whose job has gone (completed, or abandoned last sweep) must
        // not carry its stale stamp onto the producer's next job.
        self.transform_stall_watch
            .retain(|producer, _| jobs.iter().any(|(live, _, _)| live == producer));
        for (producer, job_id, progress) in jobs {
            let stalled_days = {
                let watch = self
                    .transform_stall_watch
                    .entry(producer.clone())
                    .or_insert_with(|| TransformStallWatch {
                        job_id: job_id.clone(),
                        progress_work_minutes: progress,
                        unchanged_since_game_days: current_days,
                    });
                // Any movement — accrued minutes, or a different job under the
                // same producer — restarts the window; only a figure sitting
                // exactly still is a job nobody is working.
                if watch.job_id != job_id || watch.progress_work_minutes != progress {
                    *watch = TransformStallWatch {
                        job_id,
                        progress_work_minutes: progress,
                        unchanged_since_game_days: current_days,
                    };
                    continue;
                }
                current_days - watch.unchanged_since_game_days
            };
            if stalled_days < TRANSFORM_ABANDON_GAME_DAYS {
                continue;
            }
            if let Some(job) = world.abandon_transform_job(&producer) {
                world.touch_public_state();
                self.push_food_log(format!(
                    "transform_abandoned: producer {}, spec {}, job {}, production_day {}, released [{}], progress {:.3} work_minutes, idle_game_days {stalled_days:.2}",
                    job.producer,
                    job.spec_id,
                    job.job_id,
                    job.production_day,
                    reserved_inputs_trace(&job.inputs),
                    job.progress_work_minutes,
                ));
            }
            self.transform_stall_watch.remove(&producer);
        }
    }
}

fn sale_receipt_trace(receipt: &SaleReceipt, counter: &str) -> String {
    let lines = receipt
        .lines
        .iter()
        .map(|line| {
            format!(
                "{}->{} {}{:?} qty {} unit {} total {}",
                line.source_item_id,
                line.destination_item_id,
                line.matcher.kind,
                line.matcher.metadata,
                line.quantity,
                line.unit_price_sparks,
                line.line_total_sparks,
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "sale: operation {}, counter {counter}, buyer {}, seller {}, lines [{lines}], total {}",
        receipt.operation_key, receipt.buyer, receipt.seller, receipt.total_sparks
    )
}

fn stock_specs_trace(specs: &[StockSpec]) -> String {
    specs
        .iter()
        .map(|stock| format!("{}{:?}:{}", stock.kind, stock.metadata, stock.quantity))
        .collect::<Vec<_>>()
        .join(",")
}

fn reserved_inputs_trace(inputs: &[ReservedInput]) -> String {
    inputs
        .iter()
        .map(|input| format!("{}:{}", input.item_id, input.quantity))
        .collect::<Vec<_>>()
        .join(",")
}

fn transform_receipt_lines_trace(lines: &[crate::inventory::TransformReceiptLine]) -> String {
    lines
        .iter()
        .map(|line| format!("{}:{}", line.item_id, line.quantity))
        .collect::<Vec<_>>()
        .join(",")
}

struct BoundaryExchangeReceipt {
    lines: Vec<String>,
    cash_in_sparks: u64,
    cash_out_sparks: u64,
}

fn boundary_exchange(
    party: &mut RoadParty,
    world: &mut World,
) -> Result<BoundaryExchangeReceipt, crate::InventoryError> {
    let next_trip = party.state.trip_number.checked_add(1).ok_or_else(|| {
        crate::InventoryError::new(
            crate::InventoryErrorCode::ArithmeticOverflow,
            "road trip number overflow",
        )
    })?;
    let mut staged = world.clone();
    let mut lines = Vec::new();
    let mut cash_in_sparks = 0u64;
    let mut cash_out_sparks = 0u64;

    // Member order and item-id order make the off-map delivery deterministic.
    for member in &party.members {
        let mut ids = staged.characters[member].holds().to_vec();
        ids.sort();
        for item_id in ids {
            let Some(item) = staged.items.get(&item_id).cloned() else {
                continue;
            };
            if !party
                .commercial_cargo
                .iter()
                .any(|matcher| matcher.matches(&item))
            {
                continue;
            }
            staged.consume_item_quantity(member, &item_id, item.quantity)?;
            lines.push(format!(
                "boundary_unload: party {}, trip {next_trip}, owner {member}, item {item_id}, kind {}, quantity {}",
                party.id, item.kind, item.quantity
            ));
        }
    }
    for member in &party.members {
        let target = party.wallet_floats[member];
        let (cash_in, cash_out) = staged.settle_wallet_exact(
            member,
            target,
            &format!("road:{}:{next_trip}:wallet:{member}", party.id),
        )?;
        if cash_in > 0 {
            lines.push(format!(
                "road_cash_in: party {}, trip {next_trip}, member {member}, amount {cash_in}",
                party.id
            ));
            cash_in_sparks = cash_in_sparks
                .checked_add(u64::from(cash_in))
                .expect("per-trip cash-in total fits u64");
        }
        if cash_out > 0 {
            lines.push(format!(
                "road_cash_out: party {}, trip {next_trip}, member {member}, amount {cash_out}",
                party.id
            ));
            cash_out_sparks = cash_out_sparks
                .checked_add(u64::from(cash_out))
                .expect("per-trip cash-out total fits u64");
        }
    }
    for (slot, stock) in party.manifest.iter().enumerate() {
        let item_id = staged.add_stock(
            &party.leader,
            stock,
            &format!("road:{}:{next_trip}:manifest:{slot}", party.id),
        )?;
        lines.push(format!(
            "boundary_load: party {}, trip {next_trip}, owner {}, item {item_id}, kind {}, quantity {}",
            party.id, party.leader, stock.kind, stock.quantity
        ));
    }
    staged.touch_public_state();
    *world = staged;
    party.state.trip_number = next_trip;
    party.state.phase = PartyPhase::StagedOutsideGate;
    Ok(BoundaryExchangeReceipt {
        lines,
        cash_in_sparks,
        cash_out_sparks,
    })
}

/// `"s"` unless `n == 1` — the naive pluralizer, for the sparks in a percept.
fn spark_plural(n: u32) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Eat one unit of a held food item **without** the bystander inbox lines the
/// `eat` verb delivers to nearby NPCs — the market's zero-token discipline
/// (`04` §5, `05_the_llm_seam.md` §4). A code-driven meal (eat-at-pitch, or a
/// famished actor eating what they hold) fires far too often to nudge a reaction
/// turn per bite, so it follows the well-draw pattern: the eater *remembers their
/// own meal* (askable), the stack decrements with the same commitment-aware
/// rule and satiety as the verb, and the player sees the item vanish from the snapshot — but
/// no `X ate a herring` lands in a neighbour's inbox. A **deliberate** `eat` turn
/// (an LLM or the player choosing it) still carries its terse bystander line
/// through the real verb; only the ladder's auto-eat is silenced. Returns whether
/// a unit was actually eaten (a benign miss if the item left the hand meanwhile).
fn silent_eat(world: &mut World, eater: &ActorId, item_id: &ItemId) -> bool {
    if !world
        .characters
        .get(eater)
        .is_some_and(|character| character.holds().contains(item_id))
    {
        return false;
    }
    let Some(item) = world.items.get(item_id).cloned() else {
        return false;
    };
    let Some(satiety) = world.item_catalog.satiety(&item) else {
        return false; // not food (a race, or a bad decision) — eat nothing
    };
    let quench = world.item_catalog.thirst_quench(&item).unwrap_or(0);
    let verb = if world.item_catalog.is_drink(&item) {
        "drank"
    } else {
        "ate"
    };
    let noun = world.item_catalog.display_name(&item);
    if world.consume_item_quantity(eater, item_id, 1).is_err() {
        return false;
    }
    if let Some(character) = world.characters.get_mut(eater) {
        let hunger = &mut character.state.needs.hunger;
        *hunger = (*hunger + f64::from(satiety)).min(HUNGER_MAX);
        let thirst = &mut character.state.needs.thirst;
        *thirst = (*thirst + f64::from(quench)).min(crate::THIRST_MAX);
        character.remember_percept(format!("You {verb} a {noun}."));
    }
    // A ladder meal starts the gut clock exactly like the verb does
    // (`extra_pockets.md` M3) — the round feeds most of the cast, so without
    // this the poop clock would only ever run for the handful who `eat`.
    crate::actions::queue_gut(world, eater, None);
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
    receipt: SaleReceipt,
}

/// Build a townsperson's resolved legs from their route override (the 19 authored
/// majors) or their occupation template, dropping any leg whose anchor does not
/// resolve.
fn build_legs(
    rounds: &RoundsDoc,
    resolver: &PlaceResolver,
    worksites: &BTreeMap<String, Vec3>,
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
            name => match resolver
                .resolve(name)
                .or_else(|| worksites.get(name).copied())
            {
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

/// One generated citizen's leash, drawn once and for life over
/// [`CROWD_LEASH_MIN_M`]..=[`CROWD_LEASH_MAX_M`] — the same `hash01` idiom as
/// every other per-person constant in this file, so the same crowd is the same
/// crowd in every run and every save.
fn crowd_leash_m(id: &ActorId) -> f64 {
    CROWD_LEASH_MIN_M + hash01("crowd_leash", id, 0) * (CROWD_LEASH_MAX_M - CROWD_LEASH_MIN_M)
}

/// The active leg at a given office and weekday: among the eligible legs (begun
/// by `office`, allowed today), the one with the greatest `from`, later
/// array-position winning a tie — so a market-day leg placed after the generic
/// one wins on its day and is filtered out otherwise. If nothing has begun yet
/// (deep night before the first leg), the day's tail leg carries over.
fn active_leg(legs: &[RoundLeg], office: Office, weekday: Weekday) -> Option<&RoundLeg> {
    let eligible = |leg: &&RoundLeg| {
        leg.only_on
            .as_ref()
            .is_none_or(|days| days.contains(&weekday))
    };
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
/// the errand chain (`features/implemented/movement/05_the_llm_seam.md` §3). The engine
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
    round.lightning_reflex_until.retain(|_, until| now < *until);
    round.chalk_refused_until.retain(|_, until| now < *until);
    // Before anything reads a leg: a `set_round` recorded since the last tick
    // is part of the day this tick runs, not the next one.
    apply_round_edits(round, world, now);
    tick_food_economy(round, world, clock, now, &mut nudges);
    round.tick_road_parties(world, nav, clock.at(now), now, in_conversation);
    decay_needs(round, world, clock, now);
    tick_lamps(round, clock, now);
    resolve_arrivals(round, world);
    resolve_food_arrivals(round, world);
    update_weather_shelter_intents(round, world, clock.game_days(now), now);
    tick_intents(round, world, nav, now, &mut nudges);
    service_sources(round, world, nav, clock, now, player_id, in_conversation);
    service_stalls(round, world, clock, now, player_id);
    round.tick_stock_plans(world, nav, clock.at(now), now, in_conversation);
    // Credit the interval that just elapsed before the office ladder can send
    // a worker away at the new bell.
    round.tick_production(world, clock, now, in_conversation);
    run_ladder(round, world, nav, clock, now, in_conversation, &mut nudges);
    round.trace_cart_load_changes(world);
    nudges
}

/// Advance only the *release* side of shelter behavior.  Acquisition remains a
/// ladder decision, but hysteresis must run every poll (including while an
/// actor is between decision cadences) in game time so debug acceleration and
/// coarse headless polling give the same answer.
fn update_weather_shelter_intents(round: &mut Round, world: &mut World, game_days: f64, now: f64) {
    let precipitation = world
        .current_weather
        .unwrap_or(WeatherSample::CLEAR)
        .precipitation;
    let mut release = Vec::new();
    for (id, intent) in &mut round.weather_shelter_intents {
        if !world.is_present(id) {
            release.push(id.clone());
            continue;
        }
        // Their own hearth outranks a public awning. Acquisition already
        // refuses the resident standing at home, but a claim taken *before*
        // the walk home (the famished rung sent them to the hearth mid-seek)
        // sits above the round in `decide` and would march them back out of a
        // dry house into the rain — holding a capacity slot against a soaked
        // neighbour the whole time. A claim dies at the holder's own door.
        if round
            .people
            .get(id)
            .and_then(|person| person.home)
            .is_some_and(|home| {
                world.characters.get(id).is_some_and(|character| {
                    character.position_m().distance(home) <= HOME_ARRIVE_RADIUS_M
                })
            })
        {
            release.push(id.clone());
            continue;
        }
        if precipitation + f64::EPSILON < intent.release_threshold {
            let below_since = intent.below_since_days.get_or_insert(game_days);
            if game_days - *below_since >= intent.release_after_days {
                release.push(id.clone());
            }
        } else {
            intent.below_since_days = None;
        }
    }
    for id in release {
        let Some(intent) = round.weather_shelter_intents.remove(&id) else {
            continue;
        };
        let was_weather_walk = round.people.get(&id).is_some_and(|person| {
            person.phase == Phase::Travelling && person.travel_target == Some(intent.target)
        });
        if was_weather_walk {
            if let Some(character) = world.characters.get_mut(&id) {
                character.state.movement = None;
            }
            if let Some(person) = round.people.get_mut(&id) {
                person.phase = Phase::Idle;
                person.travel_target = None;
                person.travel_for_intent = false;
                person.next_decision = person.next_decision.min(now);
            }
        }
    }
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
            let roll = hash01(
                "belwyns_lamp",
                &keeper,
                time.day as u64 ^ stable_hash(&square),
            );
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
/// Only walks are interrupted; someone Queued or Drawing at a well — or queued
/// or eating at a stall — is already standing still and keeps their place.
/// Dropping the walker to [`Phase::Idle`] hands them back to the ladder, which
/// re-decides once the exchange goes cold (the [`tick`] hold above) — so an
/// interrupted errand resumes on its own. Anyone not enrolled (a scripted
/// mover) is none of the round's business and is left alone — as is an
/// *excused* walker: their pressing errand has already outranked the
/// conversation, and a parting line must not stop them again.
pub fn interrupt_for_conversation(round: &mut Round, world: &mut World, id: &ActorId) {
    let Some(person) = round.people.get_mut(id) else {
        return;
    };
    if person.excused {
        return;
    }
    // A stall errand is a walk the water phases cannot see: [`Decision::ApproachStall`]
    // leaves the phase standing at [`Phase::Idle`] while the body crosses the square,
    // so asking the phases alone would let a buyer stroll off to the pitch mid-sentence.
    // Only the Approaching leg is a walk. The errand itself is left standing: stopped
    // short of the pitch, [`resolve_food_arrivals`] drops it next tick and the ladder
    // has them back — the same door a failed re-route leaves by.
    let walking_to_stall = matches!(
        person.food.as_ref().map(|errand| &errand.phase),
        Some(FoodPhase::Approaching)
    );
    if !walking_to_stall
        && !matches!(
            person.phase,
            Phase::Approaching | Phase::Travelling | Phase::Returning
        )
    {
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
/// happens (`features/implemented/movement/05_the_llm_seam.md` §2).
fn tick_intents(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    now: f64,
    nudges: &mut Vec<ActorId>,
) {
    // The enrolled cast *and* every road-party member. Road members are
    // deliberately absent from `people` — their feet belong to the party, not
    // to the round — but `go_to` accepts them like anyone else (it refuses only
    // `leaving_city` and the law's hands), and an intent nothing ticks never
    // has its deadline stamped, never arrives and never lapses. Nor does it lie
    // quiet: [`Round::tick_road_parties`] reads the intent *above* the trading
    // leg, so one dead errand parks a carrier at a frozen point — away from
    // their own counter — for the rest of the trip. Collected as a set, so the
    // pass keeps one deterministic id order.
    let ids: BTreeSet<ActorId> = round
        .people
        .keys()
        .cloned()
        .chain(
            round
                .road_parties
                .values()
                .flat_map(|party| party.members.iter().cloned()),
        )
        .collect();
    for id in ids {
        if !world.is_present(&id) {
            continue;
        }
        let Some(character) = world.characters.get(&id) else {
            continue;
        };
        let Some(mut intent) = character.state.intent.clone() else {
            // The intent ended outside this pass — `stop {}` is self-initiated
            // and emits no percept — but its walk may still be under way: halt
            // exactly that walk and hand the body back to the ladder. A road
            // member has no such walk to halt: `travel_for_intent` is the
            // round's own bookkeeping and they were never enrolled in it, the
            // same nothing [`end_intent`] finds to unwind for them.
            if let Some(person) = round.people.get_mut(&id)
                && person.travel_for_intent
            {
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
            } => match world
                .characters
                .get(target_id)
                .filter(|_| world.is_present(target_id))
                .map(Character::position_m)
            {
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
                    actor_id: target_id,
                    ..
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
        //
        // A road member is the one exception: the laying is not theirs, because
        // the feet are not. [`Round::tick_road_parties`] already walks them to
        // this very target — it honours the intent above the trading leg — and
        // there is no `people` entry to keep a phase in. Everything above (the
        // deadline, the arrival, the follow's last-seen) is still owed to them.
        let (target, arrive_radius) = match &intent.target {
            IntentTarget::Place { point, .. } => (*point, PLACE_ARRIVE_RADIUS_M),
            IntentTarget::Person { last_seen, .. } => (*last_seen, PERSON_ARRIVE_RADIUS_M),
        };
        let tracking = matches!(&intent.target, IntentTarget::Person { visible: true, .. });
        let Some(person) = round.people.get(&id) else {
            continue;
        };
        let lay = match person.phase {
            // A follow resuming from a stand waits for the ladder cadence,
            // exactly as a place walk does: `interrupt_for_conversation` puts
            // the walker in `Idle` so the pause after answering a line is the
            // answer's beat — a per-poll re-lay here would have the follower
            // back on their feet one movement tick later, chasing a moving
            // target out of the 20 m say radius while their reply is still
            // being written. Fresh stays immediate: "sets off after them"
            // should mean now.
            Phase::Idle => fresh || (tracking && now >= person.next_decision),
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
    // Road-party members are never enrolled in `people` — their walking is the
    // party's, not the round's — yet they issue `go_to` like anyone else, and
    // the recall ends those intents through this same door
    // ([`Round::begin_road_return`]). The percept and the nudge above are all
    // they are owed; there is no errand bookkeeping to unwind.
    let Some(person) = round.people.get_mut(id) else {
        return;
    };
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

/// Name a live intent's destination the way its owner would say it: a place by
/// its name, a person as [`identify_ids`] shows them. `None` when `id` has no
/// intent. Shared by every "…before you reached {destination}" lapse line, so
/// the ladder's preemption and the road's recall speak with one voice.
fn intent_destination(world: &World, id: &ActorId) -> Option<String> {
    let intent = world.characters.get(id)?.state.intent.as_ref()?;
    Some(match &intent.target {
        IntentTarget::Place { name, .. } => name.clone(),
        IntentTarget::Person {
            actor_id: target_id,
            ..
        } => identify_ids(world, id, target_id),
    })
}

/// Whether an office is a meal office — the hours the hearth feeds
/// (`03_hunger.md` §4): dinner at High Wick, and the supper span from the Waning
/// through Lamplight.
fn is_meal_office(office: Office) -> bool {
    matches!(
        office,
        Office::HighWick | Office::Waning | Office::Lamplight
    )
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
        if !world.is_present(id) {
            continue;
        }
        // Read before the mutable borrow below.
        let kept = world.custody.holds(id);
        let Some(character) = world.characters.get_mut(id) else {
            continue;
        };
        // **Stone House rations** (`law_and_order.md` M5b). Somebody the law is
        // holding cannot walk to a cistern or a stall — rung 0 of `decide` sees
        // to that, and `go_to` refuses them — so without this they decay to
        // nothing within minutes of the run starting and stay there, and eight
        // authored inmates spend the whole game dying of thirst against a wall.
        // The lore answered this before the code existed: Lise Skell's own sheet
        // says *"Stone House rations and food carried in by kin are your present
        // support"*, and the design's whole point is that families bring bread
        // and a blanket to the grate. So custody feeds and waters: a keeper who
        // let their prisoners die would not be a keeper, and being held is meant
        // to be a conversation, not a slow death.
        if person.draws_water() && !kept {
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
        if !kept {
            *hunger = (*hunger - hunger_drop).max(0.0);
        }
        if at_hearth || kept {
            // The gaol is a hearth, for the reason above: rations come in, and
            // they are the one thing a prisoner is reliably given.
            *hunger = (*hunger + hearth_gain).min(HUNGER_MAX);
        }
    }
}

/// Dispatch the retained Kindling restock and M5 household settlement exactly
/// once per office crossing since the last poll, no matter the debug time-scale
/// (`WorldClock::offices_crossed`, the same span the bells ride).
fn tick_food_economy(
    round: &mut Round,
    world: &mut World,
    clock: &WorldClock,
    now: f64,
    nudges: &mut Vec<ActorId>,
) {
    let crossings = clock.offices_crossed(round.last_office_now, now);
    round.last_office_now = now;
    for (instant, office) in crossings {
        let time = clock.at(instant);
        let day = time.day;
        round.trigger_road_office(world, office, time, nudges);
        match office {
            Office::Watch => round.dispatch_household_settlement(world, day),
            Office::Kindling if !round.stalls.is_empty() => {
                round.bind_vendors(world, time.weekday);
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
    // Only the walkers this pass can act on. An `ActorId` is a `String`, so
    // collecting the whole enrolled roster cost ~450 allocations on every 20 Hz
    // tick to reach the handful actually on their way to a pitch; the loop
    // below re-reads the errand anyway, so the filter is the pass's own first
    // two early-outs moved one step earlier.
    let ids: Vec<ActorId> = round
        .people
        .iter()
        .filter(|(_, person)| {
            person
                .food
                .as_ref()
                .is_some_and(|errand| matches!(errand.phase, FoodPhase::Approaching))
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        if !world.is_present(&id) {
            continue;
        }
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
            round.people.get_mut(&id).expect("person exists").food = Some(FoodErrand {
                stall: errand.stall,
                phase: FoodPhase::Queued,
            });
            world
                .characters
                .get_mut(&id)
                .expect("buyer exists")
                .state
                .movement = None;
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
fn service_stalls(
    round: &mut Round,
    world: &mut World,
    clock: &WorldClock,
    now: f64,
    player_id: &ActorId,
) {
    if round.stalls.is_empty() {
        return;
    }
    let time = clock.at(now);

    // Inventory can change while a buyer is walking or queued (a gift, a
    // transform completion, a released offer). A usable held meal satisfies
    // acquisition immediately, so cancel the stale queue slot before it can
    // commit a second purchase. Famished/hungry actors eat now unless their
    // active supper leg is still carrying them home.
    let cancelled: Vec<(ActorId, ItemId)> = round
        .people
        .iter()
        .filter_map(|(id, person)| {
            matches!(
                person.food.as_ref().map(|errand| &errand.phase),
                Some(FoodPhase::Approaching | FoodPhase::Queued)
            )
            .then(|| {
                world
                    .characters
                    .get(id)
                    .and_then(|actor| held_edible(round, world, actor))
                    .map(|item| (id.clone(), item))
            })
            .flatten()
        })
        .collect();
    for (buyer, item) in cancelled {
        for stall in &mut round.stalls {
            stall.queue.retain(|queued| queued != &buyer);
            if stall
                .serving
                .as_ref()
                .is_some_and(|(served, _)| served == &buyer)
            {
                stall.serving = None;
            }
        }
        round
            .people
            .get_mut(&buyer)
            .expect("buyer is enrolled")
            .food = None;
        if world
            .characters
            .get(&buyer)
            .is_some_and(|actor| actor.needs().hunger < HUNGER_HUNGRY)
            && !held_meal_waits_for_home(round, world, &buyer, time.office, time.weekday)
        {
            silent_eat(world, &buyer, &item);
        }
    }

    // The law has the buyer (`law_and_order.md` M4/M5). A queue slot outlives
    // the seizure — `custody::follow_escorts` walks the prisoner off a pace
    // behind the officer while the pitch keeps their place — and the head of a
    // queue is served unconditionally, so without this a prisoner halfway to
    // the Stone House completes a hand-to-hand sale with a vendor hundreds of
    // metres behind them and sits down to eat it on the road. The escort's own
    // errands are stripped by `abandon_bodily_errands` in `run_ladder`; the
    // prisoner's are nobody's, so they are stripped here.
    //
    // Dropped from the queue outright rather than merely passed over — the
    // stock plan's door when the law takes its buyer (`tick_stock_plans`).
    // They are not coming back to this pitch on this errand, and a slot its
    // holder can never use blocks everybody behind them for as long as the law
    // has them, which for a committal is the rest of the day. Unlike the water
    // twin there is nothing to let finish first: a draw is one body and a
    // bucket, a sale is two bodies now hundreds of metres apart. A meal
    // already in hand keeps its few-second timer, exactly as for the escort.
    let seized: Vec<ActorId> = round
        .people
        .iter()
        .filter_map(|(id, person)| {
            (matches!(
                person.food.as_ref().map(|errand| &errand.phase),
                Some(FoodPhase::Approaching | FoodPhase::Queued)
            ) && world.custody.holds(id))
            .then(|| id.clone())
        })
        .collect();
    for buyer in seized {
        for stall in &mut round.stalls {
            stall.queue.retain(|queued| queued != &buyer);
            if stall
                .serving
                .as_ref()
                .is_some_and(|(served, _)| served == &buyer)
            {
                stall.serving = None;
            }
        }
        round
            .people
            .get_mut(&buyer)
            .expect("buyer is enrolled")
            .food = None;
    }

    // Open = the hours predicate holds *and* the bound vendor is at the pitch —
    // the seller the round delivered to the square (no pin). Computed first, so
    // the mutable pass below is free of the world borrow.
    let availability: Vec<(bool, bool)> = round
        .stalls
        .iter()
        .map(|stall| {
            let ordinary_open = stall.open.is_open(time.office, time.weekday)
                && stall.vendor.as_ref().is_some_and(|vendor| {
                    world.is_present(vendor)
                        && world.characters.get(vendor).is_some_and(|character| {
                            character.position_m().distance(stall.pitch) <= STALL_PITCH_REACH_M
                        })
                });
            let weather_open = stall_weather_open(world, stall);
            (ordinary_open && weather_open, !weather_open)
        })
        .collect();

    let mut finished: Vec<(usize, ActorId)> = Vec::new();
    let mut to_release: Vec<usize> = Vec::new();
    // (sound_id, position) player-only world sounds to emit this tick: a cry per
    // open stall on its slow rhythm, plus a clink per sale (pushed below).
    let mut sounds: Vec<(&'static str, Vec3)> = Vec::new();
    for (s, (is_open, weather_paused)) in availability.iter().copied().enumerate() {
        round.stalls[s]
            .queue
            .retain(|actor| world.is_present(actor));
        if round.stalls[s]
            .serving
            .as_ref()
            .is_some_and(|(actor, _)| !world.is_present(actor))
        {
            round.stalls[s].serving = None;
        }
        if !is_open {
            // A sale whose timer already started is an atomic handoff. If
            // weather closes the bare board mid-count, finish that one hurried
            // exchange instead of deleting it; only the waiting queue is sent
            // away. Ordinary office/vendor closure retains its old behavior.
            if weather_paused && let Some((buyer, _)) = round.stalls[s].serving.take() {
                if round.stalls[s].queue.first() == Some(&buyer) {
                    round.stalls[s].queue.remove(0);
                } else {
                    round.stalls[s].queue.retain(|id| id != &buyer);
                }
                finished.push((s, buyer));
            }
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
                // `World::market_sale` emitted the one purpose-neutral `sale`
                // event used for the presentation-only hand-over. It has no
                // recipients, so the market's zero-token discipline holds.
                let carry = should_carry(round, &buyer, time.office, time.weekday);
                if carry {
                    round.people.get_mut(&buyer).expect("buyer exists").food = None;
                } else {
                    round.people.get_mut(&buyer).expect("buyer exists").food = Some(FoodErrand {
                        stall: s,
                        phase: FoodPhase::Eating {
                            item: sale.bought_id,
                            until: now + EAT_SECONDS,
                        },
                    });
                }
                if let Some(character) = world.characters.get_mut(&buyer) {
                    character.state.movement = None;
                }
                world.assert_invariants();
                round.push_food_log(format!(
                    "{}; stall {}, item {}, posted_price {}, stock_left {}, disposition {}",
                    sale_receipt_trace(&sale.receipt, &format!("stall:{}", sale.stall_name)),
                    sale.stall_name,
                    sale.item_display,
                    sale.price,
                    sale.stock_left,
                    if carry {
                        "carried_home"
                    } else {
                        "eat_at_pitch"
                    },
                ));
            }
            None => {
                // Could not pay (spent it mid-queue), nothing affordable is
                // left, or — since M1 — the cross on their door: a graceful
                // no-sale, the buyer leaves and re-evaluates.
                //
                // The chalk case is re-asked here rather than threaded out of
                // `try_purchase` because that function has no `now` and no
                // clock, and both the stamp and the trace want one. It is the
                // same predicate over the same authoritative source, so the
                // two answers cannot disagree.
                let chalked = crate::marks::binding_mark_about(
                    world,
                    crate::marks::MarkKind::ChalkCross,
                    &buyer,
                );
                // …and only once per stamp period. The `nearest_open_stall`
                // guard already keeps the ladder from re-queueing them, but
                // this is the belt to that pair of braces: whatever route puts
                // a stamped buyer back at a counter — a queue they were
                // already standing in, a stall bound before the refusal — it
                // is the same refusal, not a new one, and it must not write a
                // second line or take a second turn's worth of attention.
                let already_refused = round.chalk_refused_until.contains_key(&buyer);
                if let Some(mark_id) = chalked.filter(|_| !already_refused) {
                    let stall_name = round.stalls[s].name.clone();
                    // An inbox line, and deliberately **no nudge**: a refusal
                    // is not worth a paid turn (§2.8). They will mention it on
                    // whatever turn they were going to take anyway.
                    if let Some(character) = world.characters.get_mut(&buyer) {
                        character.notify_percept(format!(
                            "You reached the counter at {stall_name} and were refused: \
                             there is a chalk cross on your door, and the vendor \
                             will not sell to a household that owes."
                        ));
                    }
                    // …and a trace line beside the sale traces, so `--trace-food`
                    // distinguishes a chalk refusal from "spent it mid-queue",
                    // which the bare `None` arm never could.
                    round.push_food_log(format!(
                        "refused_on_chalk; stall {stall_name}, buyer {buyer}, mark {mark_id}"
                    ));
                    // The anti-re-queue stamp. Without it they rejoin this very
                    // queue on the next `next_decision` and are refused again,
                    // forever, one inbox line a lap.
                    let until = now + CHALK_REFUSAL_GAME_DAYS * clock.seconds_per_day();
                    round
                        .chalk_refused_until
                        .entry(buyer.clone())
                        .and_modify(|deadline| *deadline = deadline.max(until))
                        .or_insert(until);
                }
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
            Some(FoodErrand {
                phase: FoodPhase::Eating { item, until },
                ..
            }) if now >= *until => Some((id.clone(), item.clone())),
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
                Some(pos) if pos.distance(position) <= sound.audible_distance => {
                    vec![player_id.clone()]
                }
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
    // The chalk at the counter (`features/implemented/chalking_the_walls.md` M1). A buyer
    // with a live, still-binding cross on their door is refused here, at the
    // head of the queue, and not at stall selection — a buyer who walks across
    // the square, queues, reaches the counter and is *then* turned away is a
    // scene the player can stand next to and watch. A buyer who silently never
    // sets out is nothing.
    //
    // This reads `world.marks` and **never `world.notices`**. That partition is
    // the whole feature: if the refusal consulted the notice instead, scrubbing
    // the cross would change nothing and forging one would be placebo. The
    // notice's only role is to be the reason a hand chalked, once, on the
    // ward's daily beat (`notices::chalk_the_debtors`).
    if crate::marks::binding_mark_about(world, crate::marks::MarkKind::ChalkCross, buyer).is_some()
    {
        return None;
    }
    let vendor = round.stalls[s].vendor.clone()?;
    let trade_key = round.stalls[s].trade.clone();
    let trade = round.food_trades.get(&trade_key)?.clone();
    let buyer_sparks = world.spendable_sparks(buyer);

    // Choose what to sell: the pot materialises a fresh bowl inside the same
    // staged transaction; every stocked counter scans the vendor's live,
    // uncommitted holds rather than a stale side list.
    let (matcher, price, conjure): (ItemMatcher, u32, bool) = if let Some(kind) = &trade.per_serving
    {
        let bowl = Item::new(ItemId::from_raw("stew_probe"), kind.as_str());
        let price = world.item_catalog.price_sparks(&bowl)?;
        if buyer_sparks < price {
            return None;
        }
        (ItemMatcher::new(kind.clone()), price, true)
    } else {
        let mut best: Option<(u32, u32, ItemId, Item)> = None;
        for stock_id in world.characters.get(&vendor)?.holds() {
            let Some(item) = world.items.get(stock_id) else {
                continue;
            };
            if world.uncommitted_quantity(stock_id) == 0
                || !world.item_catalog.is_edible(item)
                || !trade.listings.iter().any(|matcher| matcher.matches(item))
            {
                continue;
            }
            let Some(item_price) = world.item_catalog.price_sparks(item) else {
                continue;
            };
            if item_price > buyer_sparks {
                continue;
            }
            // Cheapest wins; at the same price the most filling wins (a spark
            // buys a herring, not an egg, off a mixed provisions board), then
            // the id tie-break keeps it deterministic.
            let satiety = world.item_catalog.satiety(item).unwrap_or(0);
            let better = best
                .as_ref()
                .is_none_or(|(best_price, best_satiety, best_id, _)| {
                    (item_price, std::cmp::Reverse(satiety), stock_id)
                        < (*best_price, std::cmp::Reverse(*best_satiety), best_id)
                });
            if better {
                best = Some((item_price, satiety, stock_id.clone(), item.clone()));
            }
        }
        let (price, _satiety, _stock_id, item) = best?;
        let matcher = ItemMatcher {
            kind: item.kind.clone(),
            metadata: item.metadata.clone(),
        };
        (matcher, price, false)
    };

    let operation = format!(
        "meal:{}:{}:{}",
        round.stalls[s].name,
        buyer,
        world.event_sequence + 1
    );
    let mut staged = world.clone();
    if conjure {
        staged
            .add_stock(
                &vendor,
                &StockSpec {
                    kind: matcher.kind.clone(),
                    metadata: matcher.metadata.clone(),
                    quantity: 1,
                },
                &format!("{operation}:pot"),
            )
            .ok()?;
    }
    let receipt = staged
        .market_sale(
            buyer,
            &vendor,
            &[MarketRequestLine {
                matcher: matcher.clone(),
                quantity: 1,
            }],
            price,
            &operation,
        )
        .ok()?;
    let bought_id = receipt.lines.first()?.destination_item_id.clone();
    *world = staged;

    let stock_left: u32 = trade
        .listings
        .iter()
        .map(|listed| world.uncommitted_held_quantity(&vendor, listed))
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

    Some(Sale {
        stall_name,
        item_display,
        price,
        stock_left,
        bought_id,
        pitch,
        receipt,
    })
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

/// Severe rain pauses only an *exposed* pitch.  A canvas/slate market shelter
/// in the shared social-cover data keeps trading; a bare board releases its
/// queue but retains every stock item and binding, so it resumes normally once
/// the authoritative sample clears.
fn stall_weather_open(world: &World, stall: &FoodStall) -> bool {
    let severe = world.current_weather.is_some_and(|weather| {
        matches!(
            weather.kind,
            WeatherKind::Downpour | WeatherKind::Thunderstorm
        )
    });
    !severe || world.shelters.is_sheltered(stall.pitch)
}

/// Add a stroke to the tally on a source's curb, creating it at one.
///
/// Saturates at [`TALLY_STROKES_MAX`]: a well everybody used all day reads
/// "very busy", not an unbounded number, and the reader's penalty stays
/// bounded with it.
fn notch_the_tally(round: &Round, world: &mut World, source_index: usize, game_days: f64) {
    if !world.mark_kind_enabled(crate::marks::MarkKind::WellTally) {
        return;
    }
    let Some(source) = round.sources.get(source_index) else {
        return;
    };
    let anchor = crate::marks::MarkAnchor::Place(source.name.clone());
    let Some(drawn) = crate::marks::draw_or_refresh(
        world,
        crate::marks::MarkKind::WellTally,
        anchor,
        // The well's own hand: no author, and no reader may ask anyway.
        None,
        game_days,
    ) else {
        return;
    };
    if let Some(mark) = world.marks.get_mut(drawn.id) {
        // A refresh returns strength to full, which is right — the chalk is
        // being written on right now — but the *count* is what a reader wants.
        mark.strokes = if drawn.fresh {
            1
        } else {
            (mark.strokes + 1).min(crate::marks::TALLY_STROKES_MAX)
        };
    }
}

/// The staffed source nearest `base`, with each candidate's distance charged
/// [`TALLY_METRES_PER_STROKE`] for every stroke chalked on its curb.
///
/// **This is a per-trip re-pick, and it has to be.** The spec proposed adding
/// the penalty inside `Round::nearest_staffed_source`, but that function has
/// only two callers and both are enrolment-time: `Townsperson.source` is
/// written once, in a struct literal, for an actor's whole life, and the
/// per-trip site reads the already-bound index. A penalty inside it would be
/// evaluated at world-seed t=0, when no chalk exists, and would therefore move
/// nobody, ever (`features/implemented/chalking_the_walls.md` §0 C7).
///
/// Ties still break by source index, so an exact tie binds identically every
/// run — and with no chalk anywhere this returns exactly what
/// `nearest_staffed_source` would.
fn tallied_source(round: &Round, world: &World, base: Vec3) -> Option<usize> {
    let charge = |index: usize| -> f64 {
        let Some(source) = round.sources.get(index) else {
            return 0.0;
        };
        let anchor = crate::marks::MarkAnchor::Place(source.name.clone());
        world
            .marks
            .find(crate::marks::MarkKind::WellTally, &anchor)
            .filter(|(_, mark)| world.mark_catalog.is_binding(mark))
            .map_or(0.0, |(_, mark)| {
                f64::from(mark.strokes) * TALLY_METRES_PER_STROKE
            })
    };
    (0..round.sources.len())
        .filter(|index| round.sources[*index].keeper.is_some())
        .min_by(|left, right| {
            let dl = round.sources[*left].draw_point.distance(base) + charge(*left);
            let dr = round.sources[*right].draw_point.distance(base) + charge(*right);
            dl.partial_cmp(&dr)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.cmp(right))
        })
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
    if round
        .stalls
        .iter()
        .any(|stall| stall.vendor.as_ref() == Some(id))
    {
        return None;
    }
    // Somebody turned away from a counter for the cross on their door is not
    // going straight back to the board (M1). The stamp expires on its own, so
    // this is a pause and not a ban — and it is the only thing standing
    // between a chalked buyer and an infinite re-queue, since the filters
    // below only ever reject the *poor*.
    if round.chalk_refused_until.contains_key(id) {
        return None;
    }
    // Counting the purse means a whole-cast pocket walk per spark stack
    // (`World::uncommitted_quantity`), and the code's own note above says the
    // whole cast is famished by dawn — but most of them are nowhere near a
    // board, and the distance filter below throws every stall out before the
    // money is ever the question. So it is counted on the first stall that gets
    // as far as asking, and not at all for the rest.
    let mut purse: Option<u32> = None;
    let mut best: Option<(f64, usize)> = None;
    for (s, stall) in round.stalls.iter().enumerate() {
        if !stall.open.is_open(office, weekday) || !stall_weather_open(world, stall) {
            continue;
        }
        let staffed = stall.vendor.as_ref().is_some_and(|vendor| {
            world.is_present(vendor)
                && world.characters.get(vendor).is_some_and(|character| {
                    character.position_m().distance(stall.pitch) <= STALL_PITCH_REACH_M
                })
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
        let sparks = *purse.get_or_insert_with(|| world.spendable_sparks(id));
        if !stall_has_affordable(round, world, stall, sparks) {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(best_dist, _)| distance < *best_dist)
        {
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
        return world
            .item_catalog
            .price_sparks(&bowl)
            .is_some_and(|price| price <= sparks);
    }
    let Some(vendor) = &stall.vendor else {
        return false;
    };
    world.characters[vendor].holds().iter().any(|id| {
        world.items.get(id).is_some_and(|item| {
            // Cheapest question first, dearest last. All four are pure, so the
            // order cannot change the answer — but `uncommitted_quantity` walks
            // every character's pockets, and asking it of every stack on the
            // board paid for that walk ten times over to reject stacks the
            // listing does not even name.
            trade.listings.iter().any(|matcher| matcher.matches(item))
                && world.item_catalog.is_edible(item)
                && world
                    .item_catalog
                    .price_sparks(item)
                    .is_some_and(|price| price <= sparks)
                && world.uncommitted_quantity(id) > 0
        })
    })
}

/// Move arrivals through the state machine: an approacher who reached the curb
/// joins the queue; a returner or traveller who reached their anchor falls idle.
fn resolve_arrivals(round: &mut Round, world: &mut World) {
    // Only the phases the `match` below has an arm for. Cloning all ~450 ids
    // on every 20 Hz tick to reach the few people mid-walk was pure allocator
    // churn; the pass writes nothing for anyone else, and nothing it does write
    // is another person's phase, so the shortlist cannot go stale inside the
    // loop.
    let ids: Vec<ActorId> = round
        .people
        .iter()
        .filter(|(_, person)| {
            matches!(
                person.phase,
                Phase::Approaching | Phase::Returning | Phase::Travelling
            )
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        if !world.is_present(&id) {
            continue;
        }
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
                    world
                        .characters
                        .get_mut(&id)
                        .expect("drawer exists")
                        .state
                        .movement = None;
                } else {
                    round.people.get_mut(&id).expect("person exists").phase = Phase::Idle;
                }
            }
            Phase::Returning | Phase::Travelling => {
                round.people.get_mut(&id).expect("person exists").phase = Phase::Idle;
                world
                    .characters
                    .get_mut(&id)
                    .expect("mover exists")
                    .state
                    .movement = None;
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
            .position(|id| {
                !round
                    .people
                    .get(id)
                    .is_some_and(|person| person.is_household)
            })
            .unwrap_or(round.sources[source_idx].queue.len());
        round.sources[source_idx].queue.insert(insert_at, actor);
    } else {
        round.sources[source_idx].queue.push(actor);
    }
}

/// Work each curb: finish a completed draw (refill, remember it, send home),
/// start the next one, and clank the gear while anybody is waiting.
///
/// **The well sound is a clock, not an event** (`features/implemented/movement/05_the_llm_seam.md`
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
    // (drawer, source index) so the tally knows which curb was used.
    let mut finished: Vec<(ActorId, usize)> = Vec::new();
    let mut started: Vec<ActorId> = Vec::new();
    let mut emissions: Vec<(&'static str, Vec3)> = Vec::new();

    for (source_index, source) in round.sources.iter_mut().enumerate() {
        source.queue.retain(|actor| world.is_present(actor));
        if source
            .serving
            .as_ref()
            .is_some_and(|(actor, _)| !world.is_present(actor))
        {
            source.serving = None;
        }
        if let Some((drawer, ends_at)) = source.serving.clone()
            && now >= ends_at
        {
            source.serving = None;
            if source.queue.first() == Some(&drawer) {
                source.queue.remove(0);
            } else {
                source.queue.retain(|id| id != &drawer);
            }
            finished.push((drawer, source_index));
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
    for (drawer, source_index) in finished {
        if let Some(character) = world.characters.get_mut(&drawer) {
            character.state.needs.thirst = THIRST_MAX; // a full vessel
        }
        // One stroke on the curb for one draw (`features/implemented/chalking_the_walls.md`
        // M4). Nobody is told and nobody spends a turn: this is the well
        // keeping its own count, in chalk, where the next thirsty body can
        // read it.
        notch_the_tally(round, world, source_index, clock.game_days(now));
        // Mid-exchange (with the player or a neighbour): stand at the curb
        // instead of walking off; the ladder takes over once it goes cold. The
        // same stand for anybody the law took while they queued — the delivery
        // walk is not theirs to make, and their vessel keeps its water.
        if in_conversation.contains(&drawer) || world.custody.holds(&drawer) {
            world
                .characters
                .get_mut(&drawer)
                .expect("drawer exists")
                .state
                .movement = None;
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
                    world
                        .characters
                        .get_mut(&drawer)
                        .expect("drawer exists")
                        .state
                        .movement = None;
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
            Some(pos) if pos.distance(position) <= sound.audible_distance => {
                vec![player_id.clone()]
            }
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
/// cadence has come round. First match wins (`features/implemented/movement/03_the_ladder.md`
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
    // Reuse the scratch allocation across ticks: iterating the cast while the
    // body mutates `round.people`/`world` needs the ids cloned into a buffer,
    // but the buffer's own allocation is reused (drained empty and restored),
    // not remade each 20 Hz tick. `mem::take` decouples it from `round` so the
    // body borrows `round` freely.
    let mut ids = std::mem::take(&mut round.ladder_scratch);
    ids.clear();
    ids.extend(round.people.keys().cloned());
    for id in ids.drain(..) {
        if !world.is_present(&id) {
            continue;
        }
        // The escort's feet come before the committed-errand skips below: an
        // officer who took somebody in charge *mid-errand* — halfway to the
        // well, standing in a stall queue — would otherwise carry the whole
        // walk-queue-draw-deliver arc to completion, the skips never letting
        // `decide` see the fresh station intent, while the 20 m poll freed a
        // player prisoner behind him (session 514's cousin). Abandoning here
        // rather than in the seize verb also covers a custody the verb never
        // made — the drive-mode stand-in seizes through the same act but the
        // action layer holds no `Round` to clean.
        if !world.custody.is_empty() && world.custody.is_escorting(&id) {
            round.abandon_bodily_errands(world, &id);
        }
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
            (
                person.phase,
                person.epoch,
                person.next_decision,
                person.excused,
            )
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

        let (decision, pressure) = decide(round, world, nav, &id, epoch, time.office, time.weekday);

        // A live stock visit is the resumable rung immediately above the
        // ordinary round and convenient hunger. It keeps its route through
        // conversation and ordinary re-decisions; only a pressing rung below
        // is allowed to divert the body, without resetting the visit budget.
        if round.market_errands.contains_key(&id) && pressure.is_none() {
            continue;
        }

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
            let destination = intent_destination(world, &id).expect("checked above");
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
        if held
            && !excused
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
                Decision::SeekShelter(intent) => under_way != Some(intent.target),
                Decision::WalkToLamp(index) => under_way != Some(round.lamps[*index].position),
                Decision::EatHeld(_)
                | Decision::ApproachWell
                | Decision::ApproachStall(_)
                | Decision::LightLamp(_)
                | Decision::WeatherPause => true,
                Decision::Wander(_) | Decision::Stay => false,
            };
            if !diverts {
                continue;
            }
        }

        apply_decision(round, world, nav, &id, decision);
    }
    // `drain(..)` left it empty; hand the allocation back for the next tick.
    round.ladder_scratch = ids;
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
    /// Weather reaction: claim a reachable social shelter and walk to its
    /// stable spread point.  This is deliberately not an LLM intent.
    SeekShelter(WeatherShelterIntent),
    /// A short involuntary stop after nearby lightning. It is below immediate
    /// bodily needs but above chosen errands and routine work.
    WeatherPause,
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
/// (`features/implemented/movement/05_the_llm_seam.md`) that buys the LLM one turn to
/// excuse itself before the body walks.
const CURFEW_PRESSURE: &str = "system: night is falling and the watch clears the streets — you need to be home; excuse yourself.";
const PARCHED_PRESSURE: &str =
    "system: your thirst is pressing — excuse yourself and get to the well.";
const FAMISHED_PRESSURE: &str =
    "system: you are famished; your feet are taking you to food — excuse yourself.";
/// The road's own pressing rung, in the same voice: a returning party member
/// caught mid-conversation is told before the body walks to the gate
/// ([`Round::tick_road_parties`]), never after.
const ROAD_RETURN_PRESSURE: &str =
    "system: the road party is leaving — you must go to the gate; excuse yourself.";

/// Minimum rain intensity this actor waits out once they have taken cover.
/// The threshold is chosen from semantic severity and occupation, while the
/// drizzle choice itself is a stable actor/revision roll.  Essential outdoor
/// trades continue through ordinary weather; nonessential townsfolk react at
/// steady rain and above.
fn weather_shelter_threshold(
    character: &Character,
    id: &ActorId,
    weather: Option<WeatherSample>,
) -> Option<f64> {
    let weather = weather?;
    let occupation = character
        .lore()
        .and_then(|lore| lore.occupation_id.as_deref());
    let essential = occupation.is_some_and(|occupation| {
        matches!(
            occupation,
            "watchman_and_keeper"
                | "lamplighter"
                | "healer"
                | "bell_ringer"
                | "cargo_worker"
                | "boatworker"
                | "messenger"
                | "domestic_servant"
                | "water_and_bath_worker"
                | "sanitation_worker"
        )
    });
    match weather.kind {
        WeatherKind::Drizzle if !essential => {
            let genteel = occupation.is_some_and(|occupation| {
                matches!(
                    occupation,
                    "candor_cleric"
                        | "civic_officer"
                        | "court_officer"
                        | "custody_clerk"
                        | "fine_metalworker"
                        | "freight_broker"
                        | "merchant"
                        | "money_dealer"
                        | "scholar"
                        | "scribe_and_clerk"
                )
            });
            (weather.precipitation >= 0.08
                && (genteel
                    || hash01("weather_drizzle_shelter", id, weather.semantic_revision) < 0.18))
                .then_some(0.08)
        }
        WeatherKind::Rain if !essential && weather.precipitation >= 0.22 => Some(0.22),
        WeatherKind::Downpour | WeatherKind::Thunderstorm
            if !essential && weather.precipitation >= 0.22 =>
        {
            Some(0.22)
        }
        _ => None,
    }
}

/// Pick a public, open, reachable shelter without overfilling it.  Route
/// distance is the primary score; a small stable actor/shelter salt breaks
/// equal-distance ties and spreads a crowd between adjacent awnings.
fn choose_weather_shelter(
    round: &Round,
    world: &World,
    nav: &NavData,
    id: &ActorId,
    position: Vec3,
    office: Office,
    release_threshold: f64,
) -> Option<WeatherShelterIntent> {
    let mut best: Option<(f64, WeatherShelterIntent)> = None;
    for (index, shelter) in world.shelters.shelters().iter().enumerate() {
        if shelter.access != ShelterAccess::Public
            || !shelter.is_open(office)
            || shelter.route_node >= nav.node_count()
        {
            continue;
        }
        let occupants_here = world
            .characters
            .iter()
            .filter(|(actor_id, actor)| {
                world.is_present(actor_id) && shelter.contains(actor.position_m())
            })
            .count();
        let occupants_en_route = round
            .weather_shelter_intents
            .iter()
            .filter(|(actor_id, intent)| {
                intent.shelter == index
                    && world
                        .characters
                        .get(*actor_id)
                        .is_none_or(|actor| !shelter.contains(actor.position_m()))
            })
            .count();
        if occupants_here + occupants_en_route >= shelter.capacity {
            continue;
        }
        let Some(route) = nav.route_between(position, nav.node_point(shelter.route_node)) else {
            continue;
        };
        if route.length_m > SHELTER_SEEK_RADIUS_M {
            continue;
        }
        let Some(target) = shelter_spread_target(nav, shelter, id, index) else {
            continue;
        };
        let score = route.length_m + hash01("weather_shelter_choice", id, index as u64) * 2.0;
        let intent = WeatherShelterIntent {
            shelter: index,
            target,
            release_threshold,
            below_since_days: None,
            release_after_days: (SHELTER_RELEASE_MINUTES
                + hash01("weather_shelter_release", id, index as u64)
                    * SHELTER_RELEASE_SPREAD_MINUTES)
                / (24.0 * 60.0),
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score < *best_score)
        {
            best = Some((score, intent));
        }
    }
    best.map(|(_, intent)| intent)
}

/// A walkable actor-specific point inside the authored covered polygon.  The
/// route node establishes graph reachability; this final stride is what keeps
/// five people from visibly sharing that one node.
fn shelter_spread_target(
    nav: &NavData,
    shelter: &crate::weather::Shelter,
    id: &ActorId,
    shelter_index: usize,
) -> Option<Vec3> {
    let count = shelter.polygon_xz.len() as f64;
    let centre = shelter.polygon_xz.iter().fold([0.0, 0.0], |sum, point| {
        [sum[0] + point[0], sum[1] + point[1]]
    });
    let centre = Vec3::new(centre[0] / count, WALK_Y, centre[1] / count);
    let base_angle =
        hash01("weather_shelter_angle", id, shelter_index as u64) * std::f64::consts::TAU;
    let base_radius = shelter.spread_radius_m
        * (0.25 + 0.75 * hash01("weather_shelter_radius", id, shelter_index as u64));
    for attempt in 0..12 {
        let angle = base_angle + attempt as f64 * std::f64::consts::TAU / 12.0;
        // Tighten toward the centre after a full half-turn, making even narrow
        // passage polygons find a valid point without abandoning stable spread.
        let radius = base_radius * (1.0 - attempt as f64 / 14.0);
        let candidate = Vec3::new(
            centre.x + angle.cos() * radius,
            WALK_Y,
            centre.z + angle.sin() * radius,
        );
        if shelter.contains(candidate) && nav.is_walkable(candidate.x, candidate.z) {
            return Some(candidate);
        }
    }
    if shelter.contains(centre) && nav.is_walkable(centre.x, centre.z) {
        return Some(centre);
    }
    let node = nav.node_point(shelter.route_node);
    (shelter.contains(node) && nav.is_walkable(node.x, node.z)).then_some(node)
}

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
    // Rung 0 — the law's hands (`law_and_order.md` M4b′/M5). Above every rung
    // there is, the body's needs included: an inmate whose thirst drops must not
    // set off for the nearest cistern and walk straight through the gaol wall,
    // and someone being walked to a station is moved by their escort
    // ([`crate::custody::follow_escorts`]), not by this ladder.
    //
    // One guard here covers curfew routing, parched, famished, stall-seeking,
    // the social pull, the wander and the round legs, because every one of them
    // is decided in this function. The verb needs its own guard — see `go_to`.
    if world.custody.holds(id) {
        return (Decision::Stay, None);
    }

    // Rung 0, the escort's side: delivering a prisoner outranks the body.
    // While somebody is merely in charge, their escort's curfew, thirst and
    // hunger all wait for the commit — a famished officer who turned aside for
    // the tavern (session 514) walked off the leash, and the 20 m poll freed
    // the prisoner behind him. The wait is bounded: the hold ceiling and the
    // dead-man timer both end an escort in minutes, and the moment the
    // prisoner is committed (or breaks free) the pressing rungs fire again.
    // The `go_to` aiming the officer at the station is rung 8, below all of
    // these, so it needs the whole flight of them gated.
    let escorting = world.custody.is_escorting(id);

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
    if night
        && !escorting
        && !person.curfew_exempt
        && let Some(home) = person.home
    {
        if position.distance(home) > HOME_ARRIVE_RADIUS_M {
            return (Decision::Travel(home), Some(CURFEW_PRESSURE));
        }
        // Home already, so the curfew is asking for nothing but standing still —
        // and eating what is in your hand *is* standing still. Rung 3's meal is
        // therefore lifted over this rung, and only the meal: everything else
        // the famished rung does is a walk out of the door, which is the one
        // thing the curfew exists to stop. Without it the night swallows the
        // invariant whole — the hearth refill is gated on `is_meal_office`, so
        // the Snuffing and the Watch are eight game hours of pure decay, and a
        // sleeper who carried supper home would cross famished in the small
        // hours and hold the loaf uneaten until the Kindling lifted the rung.
        if character.needs().hunger < HUNGER_FAMISHED
            && let Some(item_id) = held_edible(round, world, character)
        {
            return (Decision::EatHeld(item_id), None);
        }
        return (Decision::Stay, None);
    }

    // Rung 2 — parched: drop everything and go to the well now. Below curfew, so
    // a housed drawer waits out the night at home and sets off at the Kindling;
    // the homeless and the night trades still draw at any hour.
    if !escorting
        && let Some((thirst, _)) = water
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
    if !escorting && character.needs().hunger < HUNGER_FAMISHED {
        // Eat what you hold, standing — for anyone, the night trades included,
        // at any hour: a famished actor with food in hand always eats it (but a
        // commercially listed food is only a last preference, not protected
        // from its hungry owner). "At any hour" is why the curfew rung above
        // carries this one branch too: the housed reach it only through their
        // own door.
        if let Some(item_id) = held_edible(round, world, character) {
            return (Decision::EatHeld(item_id), None);
        }
        // The market first (M3, the bread round, `04_the_bread_round.md`): a
        // famished actor with empty hands buys at the nearest open, staffed,
        // affordable stall within reach. It ranks above the hearth — an open
        // market is the right place to feed — and rides the pressure percept, so
        // an on-stage actor gets one turn to excuse itself before the body walks,
        // exactly as parched does, and only when the rung actually diverts.
        if let Some(stall) = nearest_open_stall(round, world, id, position, office, weekday, false)
        {
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
    if !escorting
        && let Some((thirst, queue_len)) = water
        && thirst < THIRST_THIRSTY
        && queue_len < WELL_QUEUE_SHORT
    {
        return (Decision::ApproachWell, None);
    }

    // Rung 7 — hungry: use real held food before seeking another unit. During
    // the supper span a person whose active leg is home carries it there; once
    // home (or at any other post) they eat it immediately. Otherwise join the
    // nearest open, staffed, affordable stall whose queue is short
    // (`FOOD_QUEUE_SHORT`). Quiet, like thirsty — no pressure percept. Its place
    // in the ladder is fixed: after thirsty (6), before the `go_to` errand (8).
    if !escorting && character.needs().hunger < HUNGER_HUNGRY {
        if let Some(item_id) = held_edible(round, world, character) {
            if !held_meal_waits_for_home(round, world, id, office, weekday) {
                return (Decision::EatHeld(item_id), None);
            }
        } else if let Some(stall) =
            nearest_open_stall(round, world, id, position, office, weekday, true)
        {
            return (Decision::ApproachStall(stall), None);
        }
    }

    // Nearby thunderstorm lightning: a short mechanical flinch, not an LLM
    // decision. It sits below urgent needs but above errands and the daily
    // round. The conversation hold and atomic-phase skips in `run_ladder`
    // remain authoritative, so nobody walks out mid-sentence or mid-handoff.
    if round.lightning_reflex_until.contains_key(id) {
        return (Decision::WeatherPause, None);
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

    // An escort below rung 8 stands. The needs gates above cover the pressing
    // rungs, but an escort's station `go_to` can die mid-delivery — the budget
    // burns down through conversation holds, and the closing chase overwrites
    // it with a Person-follow that ends at the grab — and then every mover from
    // here down (the lamp beat, the shelters, the round leg, the social pull,
    // the wander) belongs to somebody with a prisoner on an 8 m leash. A
    // sergeant walking his Gradine leg drags an NPC prisoner across town, or —
    // the player being tethered by a poll, not a slave — quietly frees them at
    // 20 m (session 514's other half). The engine's custody poll re-lays the
    // station walk the moment the intent is empty; standing is the whole of an
    // escort's duty until it does. The lamplighter is gated with the rest: a
    // keeper who is somehow also an escort keeps the prisoner and lets the
    // quarter stay dark, which is the cheaper wrong.
    if escorting {
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

    // Weather shelter — below needs, an explicit `go_to`, and the
    // lamplighter's essential dusk act, but above the ordinary work/idle round.
    // `run_ladder`'s conversation hold treats it as deferrable, and its early
    // phase skips keep well draws, purchases, meals and other atomic work
    // committed until they complete.
    if let Some(intent) = round.weather_shelter_intents.get(id).copied() {
        let sheltered = world
            .shelters
            .shelters()
            .get(intent.shelter)
            .is_some_and(|shelter| shelter.contains(position));
        return if sheltered {
            (Decision::Stay, None)
        } else {
            (Decision::SeekShelter(intent), None)
        };
    }
    if person
        .home
        .is_some_and(|home| position.distance(home) <= HOME_ARRIVE_RADIUS_M)
    {
        // A resident already at home needs neither a visible doorway pile nor
        // a cognition turn to remain dry.
    } else if !world.shelters.is_sheltered(position)
        && let Some(release_threshold) =
            weather_shelter_threshold(character, id, world.current_weather)
        && let Some(intent) =
            choose_weather_shelter(round, world, nav, id, position, office, release_threshold)
    {
        return (Decision::SeekShelter(intent), None);
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
        let radius = if leg.is_home {
            HOME_ARRIVE_RADIUS_M
        } else {
            ROUND_ARRIVE_RADIUS_M
        };
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
    let generated = character.lore().is_some_and(|lore| lore.generated);

    // Rung 10½ — the leash is a leash (M3). Every *aim* below is already inside
    // it (`clamp_to_leash`, and `wander_target` draws inside it), but the
    // *walking* leaks: [`route_path`] snaps both endpoints to the nearest nav
    // node, and M1 stands the crowd up to 12 m off the graph, so a target 8 m
    // away can route out of the lane and back — and a `Wander`/`Stay` never
    // diverts a walk already under way. Measured before this rung at
    // `--extra-ambient 400`: the median legless generated citizen ended the day
    // 6.7 m from where they were stood, but p90 was 61 m and the worst 122 m,
    // and 39% finished outside their leash. A wider leash multiplies that, so
    // M3 **bounds** the leak rather than leaving it: one distance check per
    // poll, and anybody who has drifted off their patch walks back onto it.
    //
    // Bounded, not fixed. The snapping itself is untouched — the honest repair
    // is for a wander to walk to its own target rather than to the nearest node
    // of it, and the only tool for that ([`route_path_to_point`]) appends an
    // unvalidated final stride that would put a body through a wall corner
    // across a 40 m leash. That is a navigation change with its own risks and
    // its own verification, and it does not belong inside a milestone about how
    // far people stand from their post.
    //
    // Deliberately narrowed to citizens with **no legs at all** — the cohort the
    // leak was measured on. A generated tradesman is already recalled by rung 9
    // while their leg is live, and at night rung 9 stands down on purpose ("the
    // homeless linger rather than march to a workshop at 2 a.m."); recalling
    // them to their spawn point every Snuffing would invent the nightly tide M4
    // is for.
    //
    // The walk back always exists, which is why this cannot pin anybody where
    // it found them: a drifter is standing on a nav node (route endpoints are
    // nodes), and M1 stands every base within 12 m of *its* node, so a body
    // more than 15 m — the narrowest leash — from base is never on base's own
    // nearest node, and [`route_path`] therefore returns a real path.
    if generated && person.legs.is_empty() && position.distance(person.base) > person.leash_m {
        return (Decision::Travel(person.base), None);
    }

    // Rung 11 — the social pull: drift toward a known, settled neighbour.
    if let Some(friend) = nearest_known_settled(world, id, position) {
        let toward = drift_target(anchor, position, friend, person.leash_m);
        if nav.is_walkable(toward.x, toward.z) {
            return (Decision::Wander(toward), None);
        }
    }
    // Rung 12 — wander: mill near the post, but not every time — people mostly
    // stand and work.
    let attempts = if generated {
        CROWD_WANDER_ATTEMPTS
    } else {
        WANDER_ATTEMPTS
    };
    if hash01("round_wander_gate", id, epoch) < 0.35
        && let Some(target) = wander_target(nav, id, epoch, anchor, person.leash_m, attempts)
    {
        return (Decision::Wander(target), None);
    }
    (Decision::Stay, None)
}

/// Carry out a decision: set the walk and the phase, or stand.
fn apply_decision(
    round: &mut Round,
    world: &mut World,
    nav: &NavData,
    id: &ActorId,
    decision: Decision,
) {
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
            let bound = round.people[id]
                .source
                .expect("a well decision has a source");
            // Re-pick for *this trip* against the chalk (M4). The bound source
            // is the fallback, so a world with no tallies anywhere behaves
            // exactly as before. Written back onto the person because
            // `enqueue`, the arrival and the queue all read `person.source`
            // again later — walking to one curb and queueing at another would
            // be worse than not re-picking at all.
            let base = world.characters[id].position_m();
            let source = tallied_source(round, world, base).unwrap_or(bound);
            if source != bound
                && let Some(person) = round.people.get_mut(id)
            {
                person.source = Some(source);
            }
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
                    world
                        .characters
                        .get_mut(id)
                        .expect("drawer exists")
                        .state
                        .movement = None;
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
                    round.people.get_mut(id).expect("person exists").food = Some(FoodErrand {
                        stall,
                        phase: FoodPhase::Approaching,
                    });
                }
                None => {
                    // Already at the pitch: join the queue now.
                    if !round.stalls[stall].queue.contains(id) {
                        round.stalls[stall].queue.push(id.clone());
                    }
                    round.people.get_mut(id).expect("person exists").food = Some(FoodErrand {
                        stall,
                        phase: FoodPhase::Queued,
                    });
                    world
                        .characters
                        .get_mut(id)
                        .expect("buyer exists")
                        .state
                        .movement = None;
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
        Decision::SeekShelter(intent) => {
            round.weather_shelter_intents.insert(id.clone(), intent);
            let position = world.characters[id].position_m();
            if let Some(path) = route_path_to_point(nav, id, position, intent.target) {
                set_route(world, id, path);
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Travelling;
                person.travel_target = Some(intent.target);
                person.travel_for_intent = false;
            } else {
                // Selection already established reachability. `None` therefore
                // means the actor is at its spread point (or within the same
                // snapped node); standing is the correct completion.
                if let Some(character) = world.characters.get_mut(id) {
                    character.state.movement = None;
                }
                let person = round.people.get_mut(id).expect("person exists");
                person.phase = Phase::Idle;
                person.travel_target = None;
                person.travel_for_intent = false;
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
        Decision::WeatherPause => {
            if let Some(character) = world.characters.get_mut(id) {
                character.state.movement = None;
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
    ["i am", "i'm", "i feel"]
        .iter()
        .any(|lead| lower[..hungry_at].contains(lead))
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
/// what actually feeds. Personal food sorts before commercially listed stock,
/// but a board unit is still edible when it is the only uncommitted meal.
fn held_edible(round: &Round, world: &World, character: &Character) -> Option<ItemId> {
    let mut edible: Vec<ItemId> = character
        .holds()
        .iter()
        .filter(|item_id| {
            world.uncommitted_quantity(item_id) > 0
                && world
                    .items
                    .get(*item_id)
                    .and_then(|item| world.item_catalog.satiety(item))
                    .is_some_and(|satiety| satiety > 0)
        })
        .cloned()
        .collect();
    edible.sort_by_key(|item_id| {
        (
            commercially_listed(round, world, character.id(), item_id),
            item_id.clone(),
        )
    });
    edible.into_iter().next()
}

fn commercially_listed(round: &Round, world: &World, owner: &ActorId, item_id: &ItemId) -> bool {
    let Some(item) = world.items.get(item_id) else {
        return false;
    };
    let stall_stock = round.stalls.iter().any(|stall| {
        (stall.vendor.as_ref() == Some(owner) || stall.preferred.as_ref() == Some(owner))
            && round
                .food_trades
                .get(&stall.trade)
                .is_some_and(|trade| trade.listings.iter().any(|matcher| matcher.matches(item)))
    });
    stall_stock
        || round.counters.values().any(|counter| {
            &counter.seller == owner
                && round
                    .food_trades
                    .get(&counter.trade)
                    .is_some_and(|trade| trade.listings.iter().any(|matcher| matcher.matches(item)))
        })
}

fn held_meal_waits_for_home(
    round: &Round,
    world: &World,
    actor: &ActorId,
    office: Office,
    weekday: Weekday,
) -> bool {
    if !should_carry(round, actor, office, weekday) {
        return false;
    }
    let Some(person) = round.people.get(actor) else {
        return false;
    };
    let Some(home) = person.home else {
        return false;
    };
    world
        .characters
        .get(actor)
        .is_some_and(|character| character.position_m().distance(home) > HOME_ARRIVE_RADIUS_M)
}

/// The nearest LLM neighbour within the social radius that this actor knows by
/// name and that has stopped moving.
fn nearest_known_settled(world: &World, id: &ActorId, position: Vec3) -> Option<Vec3> {
    let me = world.characters.get(id)?;
    // Borrowed ids: this answers on the first neighbour who qualifies, or not at
    // all, and a ladder tick can ask it for dozens of walkers at once — cloning
    // the whole crowd around each of them was the bulk of the call.
    for neighbour_id in world.characters_within_refs(position, SOCIAL_PULL_RADIUS_M, Some(id)) {
        let neighbour = &world.characters[neighbour_id];
        if neighbour.control().is_llm()
            && neighbour.is_settled()
            && me.knows().contains(neighbour_id)
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
///
/// `attempts` is [`WANDER_ATTEMPTS`] for the authored cast and
/// [`CROWD_WANDER_ATTEMPTS`] for a generated citizen (M3). Measured over 2,000
/// crowd stands on the shipped graph × 12 epochs each, the give-up rate at four
/// draws is 2.7% at a 10 m leash and 3.8% at 30 m; eight draws take that to 0.4%
/// and 0.3%. The far half of a wide leash was the worry, and it turns out to be
/// a mild one — the share of accepted targets beyond half the leash moves 44.6%
/// → 46.3% at 30 m, because a rejected draw redraws the *radius* too rather than
/// retrying the same one. The cast keeps its four regardless: a fifth draw would
/// turn ~2% of its wander polls from standing into walking, and nothing may
/// change at `extra_ambient_npcs: 0`.
fn wander_target(
    nav: &NavData,
    id: &ActorId,
    epoch: u64,
    base: Vec3,
    leash_m: f64,
    attempts: u64,
) -> Option<Vec3> {
    for attempt in 0..attempts {
        let angle = hash01("round_wander_angle", id, epoch ^ attempt) * std::f64::consts::TAU;
        let radius = hash01("round_wander_radius", id, epoch.wrapping_add(attempt)) * leash_m;
        let target = Vec3::new(
            base.x + angle.cos() * radius,
            WALK_Y,
            base.z + angle.sin() * radius,
        );
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

/// Carry out the `set_round` edits the Night Office recorded on characters
/// (movement M6). One leg moves; everything else about the day stands.
///
/// It runs here rather than in the verb because the resolved legs are the
/// round's and the action layer holds only a [`World`] — the same split
/// `go_to`'s intent takes. An edit whose place has since gone, whose leg number
/// no longer exists, or whose author the round never enrolled (a road-party
/// member, who walks to the gate under the party's orders) is simply dropped:
/// the verb has already told its author the leg moved, and re-telling them at
/// midnight would be a percept nobody asked for. An edit the author's *trade*
/// forbids ([`round_edit_refusal`]) is different — that one is dropped **and
/// explained**, because the author believes the leg moved and would otherwise
/// walk their livelihood into the ground without ever learning why.
fn apply_round_edits(round: &mut Round, world: &mut World, now: f64) {
    // Taken from the whole cast, not just the enrolled, so an edit recorded on
    // somebody the round cannot serve is cleared rather than left to sit.
    let mut edits: Vec<(ActorId, RoundEdit)> = Vec::new();
    for (id, character) in world.characters.iter_mut() {
        if let Some(edit) = character.state.round_edit.take() {
            edits.push((id.clone(), edit));
        }
    }
    for (id, edit) in edits {
        let Some(entry) = world.places.get(&edit.place_id) else {
            continue;
        };
        let (label, point) = (entry.name.clone(), entry.point);
        // Their own door is still "home" on the sheet and to the hearth, so a
        // leg pointed back at it reads as one.
        let is_home = world
            .places
            .home_of(&id)
            .is_some_and(|home| home.id == edit.place_id);
        if let Some(reason) = round_edit_refusal(round, world, &id, edit.leg, &label) {
            world
                .characters
                .get_mut(&id)
                .expect("the author is in the world")
                .notify_percept(reason);
            continue;
        }
        let Some(person) = round.people.get_mut(&id) else {
            continue;
        };
        let Some(leg) = person.legs.get_mut(edit.leg) else {
            continue;
        };
        leg.at = point;
        leg.label = label;
        leg.is_home = is_home;
        // What they are *doing* there is untouched: a bed moved to the Bell and
        // Ladle's woodstore is still a bed, and Ede of the Needle already
        // sleeps in one.
        let lines: Vec<String> = person.legs.iter().map(leg_line).collect();
        // Decide again at once, so an edit to the leg already under way moves
        // their feet tonight instead of tomorrow.
        person.next_decision = person.next_decision.min(now);
        person.epoch = person.epoch.wrapping_add(1);
        world
            .characters
            .get_mut(&id)
            .expect("the author is in the world")
            .state
            .daily_round = lines;
    }
}

/// The one refusal in [`apply_round_edits`]: the reason this edit must be
/// dropped, or `None` when it is free to land.
///
/// Two systems key off a leg's *label*, and a `set_round` that rewrites it can
/// silently sever both. A half-done [`TransformJob`] accrues work only while
/// its producer's round says Work at the transform's site
/// ([`Round::production_overlap_minutes`]), so renaming that leg strands the
/// reserved inputs; and a stall or counter keeps its keeper only while a leg
/// still delivers them to the site ([`Round::pick_vendor`],
/// [`Round::counter_binding`]), so renaming *that* one shuts the board at dawn
/// with nobody told why. In both cases the trade holds the person: the edit is
/// refused and the author hears the reason in the same second-person register
/// every deferred refusal uses — which is exactly the self-correction seam the
/// Night Office reads, so tomorrow's reflection can pick a different leg.
///
/// The check lives here rather than in the verb because the verb's layer holds
/// only a [`World`], and everything being defended — the stall roster, the
/// counter set, the production plans — belongs to the round.
///
/// Only a binding that holds *before* the edit and would be gone *after* it
/// refuses: moving a leg **to** the trade's site, a duplicate leg that still
/// covers it, or a keeper the site never actually bound all edit freely.
fn round_edit_refusal(
    round: &Round,
    world: &World,
    id: &ActorId,
    leg_index: usize,
    new_label: &str,
) -> Option<String> {
    let person = round.people.get(id)?;
    if leg_index >= person.legs.len() {
        return None;
    }
    // Whether a leg still serves `site` (doing `required`, where the binding
    // cares) — `edited` reads the sheet as the edit would leave it. The edit
    // touches only the label, never the `doing`, so the same leg is probed
    // under both names.
    let bound = |site: &str, required: Option<Arrival>, edited: bool| -> bool {
        person.legs.iter().enumerate().any(|(index, leg)| {
            let label = if edited && index == leg_index {
                new_label
            } else {
                leg.label.as_str()
            };
            label == site && required.is_none_or(|doing| leg.doing == doing)
        })
    };
    let removes = |site: &str, required: Option<Arrival>| {
        bound(site, required, false) && !bound(site, required, true)
    };

    // A live production site: the active job's spec names where the work
    // happens, and the reserved inputs are only ever released by finishing it
    // there (or by the stranded-job sweep, a whole day later).
    if let Some(job) = world.active_transform_job(id)
        && let Some(site) = round
            .production_plans
            .iter()
            .filter(|plan| plan.producer == *id)
            .flat_map(|plan| &plan.transforms)
            .find(|transform| transform.id == job.spec_id)
            .map(|transform| transform.site.as_str())
        && removes(site, Some(Arrival::Work))
    {
        return Some(format!(
            "Your half-worked batch at {site} holds you there; your round keeps that leg."
        ));
    }
    // The stall this actor keeps today: eligibility is any leg labelled the
    // site, whatever it is for, so the guard asks exactly that.
    for stall in &round.stalls {
        if stall.vendor.as_ref() == Some(id) && removes(&stall.site, None) {
            return Some(format!(
                "Your stall at {} has no other keeper; your round keeps that leg.",
                stall.site
            ));
        }
    }
    // A counter sold in this actor's name: the binding needs the leg to say
    // what the counter's spec says it is for.
    for counter in round.counters.values() {
        if counter.seller == *id && removes(&counter.site, Some(counter.required_doing)) {
            return Some(format!(
                "Your counter at {} trades on your being there; your round keeps that leg.",
                counter.site
            ));
        }
    }
    None
}

/// Give a mover a fresh walk with no patrol, keeping their gait phase seamless.
///
/// Refuses anybody in the law's hands (`law_and_order.md` M4/M5). Every route
/// the round lays comes through here, and the movers that re-lay on their own
/// clock — the road party's leg walk, the stock plan's counter approach, the
/// finished draw's delivery — would otherwise out-shout the per-tick clear in
/// `custody::follow_escorts`: the clear runs *before* `round::tick`, so a route
/// re-laid after it always gets the next movement slice, and a committed
/// prisoner creeps out of the gaol at full walking speed until the roam poll
/// brands them with M4d's unanswerable escape notice for a walk they never
/// chose. `decide`'s rung 0 already stands a held body, but it only covers the
/// ladder; this is the one choke point the mechanical movers share. The guard
/// is about the *prisoner's* feet alone: the escort's station walk, the closing
/// chase and every keeper's round belong to people the law is not holding, and
/// a slaved body is moved by `follow_escorts` placing it, never through here.
fn set_route(world: &mut World, id: &ActorId, path: Vec<Vec3>) {
    if world.custody.holds(id) {
        return;
    }
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
