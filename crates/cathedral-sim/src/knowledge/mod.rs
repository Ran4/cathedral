//! What this city knows, who holds it, and in whose words.
//!
//! A **fact** is one proposition about the world — authored in
//! `assets/world/facts.json`, minted from an event, or (from M4) coined by a
//! mouth. Its identity is its id: the same fact is the same fact however many
//! mouths it has been through and however wrong it has got. What each person
//! *has* of it is a [`Held`]: how many removes, from whom, when, and what their
//! version says. [`render_line`] is the one place a fact becomes words, because
//! the words depend on who is reading.
//!
//! Two naming conventions bind every caller.
//!
//! 1. **`knowledge::holds` is always written fully qualified and never
//!    `use`d.** It is one letter from [`Character::holds`](crate::character::Character::holds)
//!    (the hands) and [`Custody::holds`](crate::custody::Custody::holds) (the
//!    law), and `notices::carries` already establishes the convention for this
//!    shape of predicate.
//! 2. Nothing in this module is re-exported at the crate root. Every type is
//!    reached as `cathedral_sim::knowledge::…`; only the two dense keys
//!    ([`FactKey`], [`AreaKey`]) join the `ids` line, because they are handles
//!    and not vocabulary.
//!
//! Nothing here enters [`PublicSnapshot`](crate::snapshot::PublicSnapshot) and
//! nothing here calls `touch_public_state`: this is prompt state, like
//! [`crate::notices`], and the snapshot's 160 KiB bound has little headroom
//! left.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::character::Character;
use crate::ids::{ActorId, AreaKey, FactId, FactKey};
use crate::prompt::PromptStrings;
use crate::world::World;

pub mod catalog;
pub mod salience;
pub mod source;

pub use catalog::{FactCatalog, FactCatalogError, FactSourceSpec, FactSpec};
pub use salience::{HedgeBand, SalienceError, SalienceTable};
pub use source::FactSource;

// ---------------------------------------------------------------------------
// The constants. Every value's derivation is in
// `features/knowledge_and_rumor/plan/02_numbers.md` §3; the doc comments here
// are copied from it. Only M5's final tuning pass may move a free parameter.
// ---------------------------------------------------------------------------

/// The ward's air halves every half game day. Chosen first, as the legible
/// statement: it sets how long a top-band fact is news
/// (`τ·log₂(1.00/VOLUNTEER_HEAT)` = 37.4 game hours, a game day and a half) and
/// every warm life below it. The bracket for a news life of one to two game days
/// is `τ ∈ [7.7, 15.4]`; 12 is the round number in it.
pub const AIR_HALF_LIFE_GAME_HOURS: f64 = 12.0;

/// The one free parameter, solved from the slow end: the smallest heat at which
/// an off-affinity `Craft` witness stops repeating himself **before the leg that
/// fires at the mint bell carries him over the nearest ward boundary** (82 m from
/// the Wickmarket, 0.26 game hours at walking pace) — so a spoiled batch dies in
/// its own lane. `t_warm(0.12, 0) < d_b / WALK_SPEED_MPS` gives
/// `VH > 0.12 / 2^(0.26/12) = 0.1182`, rounded up to three places. Warm life at
/// salience 0.12 is then 0.145 game hours, about nine game minutes — one poll.
/// Also what a fact must clear to be seated on a sheet on its own.
pub const VOLUNTEER_HEAT: f32 = 0.119;

/// The second free parameter, attached to the fast end: a stir edge every 30
/// game minutes. One roll per person per fact per stir, so the poll gap must stay
/// under `60 / STIRS_PER_GAME_HOUR` or a person could skip a whole window —
/// asserted by `the_poll_gap_cannot_skip_a_stir`, whose failure message says why.
pub const STIRS_PER_GAME_HOUR: f64 = 2.0;

/// A person's own turn at the air, in game minutes, jittered ×0.5..1.5 off their
/// id — six polls a game hour, at most 15 game minutes apart. **Game time, never
/// real seconds**: the `T` key's 60× and `--watch-clock` must give a played run's
/// roll count, or the measured cadence would not be the cadence a played run
/// produces.
pub const POLLEN_POLL_GAME_MINUTES: f64 = 10.0;
/// The jitter's ceiling. Composed with the host step it stays under the 30 game
/// minutes of one stir window at every clock scale this game runs at.
pub const POLLEN_POLL_MAX_GAME_MINUTES: f64 = 15.0;

/// Charged once, at the hop, so heat at hops n is a clean `HOP_LOSS^n × λ^t` and
/// there is no per-carrier cooling pass to write. Bracketed by two inequalities,
/// both asserted: `HOP_LOSS^4 > VOLUNTEER_HEAT` (a fourth-hand top-band story is
/// still repeatable, so the ladder's far rungs are reachable and a chain is
/// walkable) and `HOP_LOSS × 0.12 < VOLUNTEER_HEAT` (an off-affinity trade matter
/// travels one hop and stops, which is what "nothing to anyone but a cooper"
/// means arithmetically).
pub const HOP_LOSS: f32 = 0.85;

/// Where a re-asked cold fact comes back to, and no further — an **absolute**
/// heat, never `VOLUNTEER_HEAT / salience`, because heat and salience are separate
/// axes and dividing one by the other re-heats a dull fact hardest, which inverts
/// the whole point. A revived scandal circulates again; a revived stall quarrel is
/// merely answerable. The 1.10 is one rung above the gate: a re-heated top-band
/// fact stays above it for `12·log₂(1.10)` = 1.65 game hours, about half an office.
pub const REHEAT_TO: f32 = VOLUNTEER_HEAT * 1.10_f32;

/// Never 0.0. Exponential decay underflows to exactly zero and `0.0 < 0.0` is
/// false forever, which makes the row immortal and holds a cap slot nothing can
/// reclaim — the same bug `MarkCatalog::from_json` rejects in as many words.
pub const HEAT_GONE_BELOW: f32 = 0.01;

/// P(a masked field is wrong) per hop. Both readings the spec invites — right two
/// times in three at one hop, wrong four in five at four — land on ⅓; 0.35 is the
/// rounder number just above it, so a chain worth walking is a little more wrong
/// than the coincidence. Right 0.650 / 0.423 / 0.275 / 0.179 at hops 1–4.
pub const GARBLE_CHANCE_PER_HOP: f64 = 0.35;

/// ±1 per garbled hop, clamped here. A **guard, not a shape**: three
/// same-direction day garbles by hops 3 is `(0.35/2)³` ≈ 0.5%, so the clamp almost
/// never binds and exists so a deep chain cannot wander a week and fall off the
/// day phrase's vocabulary.
pub const DAY_OFFSET_MAX: i8 = 3;

/// `AreaMap::nearest_areas`' search radius for the garbled-place pool. Over 71
/// areas in a 727 × 828 m city, 120 m returns two to five neighbours — enough that
/// the swap is not always the same place, small enough that "an adjacent area"
/// stays adjacent.
pub const GARBLE_AREA_RADIUS_M: f64 = 120.0;

/// A person is not a newspaper. Six rows are ~850 B per actor at the cap, so
/// 17.4 MB at `--extra-ambient 20000`. Evicted coldest first, then most hops, then
/// `FactKey` — a total order, so eviction is reproducible.
pub const HOLDINGS_MAX: usize = 6;

/// Smaller than [`crate::notices::NOTICES_SHEET_MAX`] (4), as the spec requires.
/// Three bullets plus the header and the note is ≈ 1.14 KB on a 13.6 KB sheet
/// against a 64 KiB bound.
pub const KNOWN_SHEET_MAX: usize = 3;

/// Eight wards × 24 rows × ~104 B ≈ 20 KB of air in total. Comfortably more than
/// any one ward's ear can hold at once, and a hard bound on an LLM spamming
/// `raise_word`.
pub const AIR_PER_WARD_MAX: usize = 24;

/// 256 × ~700 B ≈ 180 KB — well past the base game's authored handful plus a game
/// day of mints plus a quest pack.
pub const FACTS_MAX_LIVE: usize = 256;

/// A chain is a reconstruction, not a log; this bounds it against a merge bug that
/// points two holdings at each other inside a prompt render. Heat alone caps hops
/// at `⌊ln(VOLUNTEER_HEAT / (s·λ^t)) / ln(HOP_LOSS)⌋` = 13 for a fresh top-band
/// fact; the practical bound is the pickup rate — about one hop per 4.2 stirs at
/// the cast's mean curiosity — so eight is past any chain a game day produces.
pub const CHAIN_MAX_LINKS: usize = 8;

/// The player has no lore sheet, so `attention::curiosity_of` would return
/// `CURIOSITY_WITHOUT_LORE` (1.0) and hand them every fact in the air on their
/// first roll. Roughly three times the cast's measured mean (0.119): an attentive
/// outsider, not a firehose.
pub const PLAYER_CURIOSITY: f64 = 0.35;

/// The same cadence as the cast's, unjittered — there is one player.
pub const PLAYER_POLL_GAME_MINUTES: f64 = 10.0;

/// Below the measured 1.2748 m minimum separation between any two of the 413 baked
/// doors, so a false positive is impossible and door equality fires only for
/// generated citizens sharing one `nav::Door` under the occupancy cap.
pub const HOUSEHOLD_EPSILON_M: f64 = 0.5;

/// Shorter than the shortest office (2 game hours), so an un-offered occasion
/// cannot survive a bell. It is deliberately **not** "longer than any inter-turn
/// gap": under the `T` key's 60× a game hour is 2.5 real seconds, which is why an
/// occasion rendered onto a sheet is `offered` and lives until that exchange's
/// reply lands or fails.
pub const OCCASION_LIFE_GAME_HOURS: f64 = 1.0;

/// A pre-filter, **not** the gate (the gate is a real `holds()` lookup). 24 bytes
/// is past "Aye." and "What of it?" and short of any real assertion.
pub const OCCASION_MIN_ASSERTION_CHARS: usize = 24;

/// One raise per actor per office.
pub const RAISES_PER_OFFICE: u8 = 1;

/// Layer 2's own scan is O(N) (`world.rs`'s `neighbours_by_distance` has no
/// spatial index), so it is gated in **real** seconds — it is a *legibility*
/// cadence for what the player can watch, not a sim cadence — and capped at the
/// nearest few carriers. 0.5 scans/s × 20,000 is 10,000 distance tests a second
/// against a pump already at 179 ms/frame.
pub const STAGE_HOP_SECONDS: f64 = 2.0;
/// The nearest eight carriers, so a busy square hops a handful of times rather
/// than N².
pub const STAGE_HOP_MAX_PAIRS: usize = 8;
/// Layer 2 reaches exactly as far as a voice does.
pub const STAGE_HOP_RADIUS_M: f64 = crate::HEARING_RADIUS_M;

/// Whole-percent heat. One cooling step multiplies heat by ~0.944 — twelve orders
/// of magnitude above `f32::EPSILON` — so a raw-`f32` comparison is true on every
/// sweep and churns any change test forever. Every ordering, change test and
/// predicate goes through this, following `marks::published_strength_pct`.
pub fn heat_pct(heat: f32) -> u8 {
    let clamped = if heat.is_nan() {
        0.0
    } else {
        heat.clamp(0.0, 1.0)
    };
    (f64::from(clamped) * 100.0).round() as u8
}

/// `λ = 0.5^(1/AIR_HALF_LIFE_GAME_HOURS)` — the per-game-hour cooling factor.
fn cooling_lambda() -> f64 {
    0.5_f64.powf(1.0 / AIR_HALF_LIFE_GAME_HOURS)
}

/// `heat_at_learn × λ^t`, `t` in game hours since learning — the one formula
/// behind [`Held::heat`] and the store's eviction ranking, so the sheet and the
/// store cannot disagree about what "coldest" means. `heat_at_learn` comes back
/// untouched when either side is `None`.
fn cooled(heat_at_learn: f32, learned_on: Option<f64>, game_days: Option<f64>) -> f32 {
    let (Some(now), Some(then)) = (game_days, learned_on) else {
        return heat_at_learn;
    };
    let hours = ((now - then) * 24.0).max(0.0);
    (f64::from(heat_at_learn) * cooling_lambda().powf(hours)) as f32
}

// ---------------------------------------------------------------------------
// The types
// ---------------------------------------------------------------------------

/// What a fact is *about* — and therefore how far it travels, how its hedges
/// erode, and whose ear it catches.
///
/// A property of the proposition, so it is invariant across every mouth: garbling
/// moves the subject, the place and the day, and never moves this. The authored
/// spelling of [`Topic::Talk`] is `"talk"`, **never `"word"`**: `word` is the
/// city's prose register (`word_in_the_ward`, `raise_word`) and must not double as
/// a machine tag.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Bed,
    Blood,
    Law,
    Omen,
    Stranger,
    Coin,
    Bread,
    Craft,
    Talk,
}

impl Topic {
    /// Declaration order, which is the order `salience.json` is validated in and
    /// the order `--trace-pollen` prints.
    pub const ALL: [Topic; 9] = [
        Topic::Bed,
        Topic::Blood,
        Topic::Law,
        Topic::Omen,
        Topic::Stranger,
        Topic::Coin,
        Topic::Bread,
        Topic::Craft,
        Topic::Talk,
    ];

    /// The nine authored spellings, for a loader error that names them all.
    pub const AUTHORED_LIST: &'static str =
        "bed, blood, law, omen, stranger, coin, bread, craft, talk";

    pub fn as_str(self) -> &'static str {
        match self {
            Topic::Bed => "bed",
            Topic::Blood => "blood",
            Topic::Law => "law",
            Topic::Omen => "omen",
            Topic::Stranger => "stranger",
            Topic::Coin => "coin",
            Topic::Bread => "bread",
            Topic::Craft => "craft",
            Topic::Talk => "talk",
        }
    }

    /// The authored tag, or `None`. The loader wants the `None` so it can name the
    /// nine legal spellings in its error.
    pub fn parse(value: &str) -> Option<Self> {
        Topic::ALL.into_iter().find(|topic| topic.as_str() == value)
    }

    /// An unrecognised tag lands on `Talk`, the dullest band: a mis-tagged fact
    /// that under-spreads is a shrug, and one that becomes a citywide scandal is
    /// the bug that would make `raise_word` unshippable. The failure direction is
    /// downward on purpose.
    pub fn parse_or_talk(value: &str) -> Self {
        Topic::parse(value).unwrap_or(Topic::Talk)
    }
}

/// Which fields a hop may move. The rest is load-bearing truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GarbleMask {
    pub subject: bool,
    pub place: bool,
    pub day: bool,
}

impl GarbleMask {
    pub const NONE: Self = Self {
        subject: false,
        place: false,
        day: false,
    };

    pub const ALL: Self = Self {
        subject: true,
        place: true,
        day: true,
    };

    /// `"none"` | `"place,day"` | `"subject,place,day"` — any order. An unknown
    /// token is a load error naming the three legal ones, because a silently
    /// ignored token is a mask that does not do what it says — and so is `none`
    /// beside a field, or an empty string: both read as one thing and mean
    /// another.
    pub fn parse(value: &str) -> Result<Self, FactCatalogError> {
        if value.trim() == "none" {
            return Ok(Self::NONE);
        }
        let mut mask = Self::NONE;
        for token in value.split(',') {
            match token.trim() {
                "subject" => mask.subject = true,
                "place" => mask.place = true,
                "day" => mask.day = true,
                "none" => {
                    return Err(FactCatalogError {
                        message: format!(
                            "garble '{value}' that mixes none with a field — none means nothing \
                             may move, so it cannot sit beside a field that may"
                        ),
                    });
                }
                other => {
                    return Err(FactCatalogError {
                        message: format!(
                            "unknown garble field '{other}'; expected none, subject, place or day"
                        ),
                    });
                }
            }
        }
        Ok(mask)
    }

    /// Always `subject,place,day` order, so a round trip is byte-stable.
    pub fn as_authored(self) -> String {
        let mut parts = Vec::new();
        if self.subject {
            parts.push("subject");
        }
        if self.place {
            parts.push("place");
        }
        if self.day {
            parts.push("day");
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(",")
        }
    }

    /// What a raised word gives a claim of this topic before the template rule
    /// narrows it: [`Self::ALL`], for every one of the nine. A claim cannot seal
    /// itself against drift, and what it can garble is whatever its sentence
    /// names — M4's `mint_claim` keeps `subject` only when it put a `{subject}`
    /// in, and `place`/`day` are never in a free-text claim. One line, no table.
    pub fn default_for(_topic: Topic) -> Self {
        Self::ALL
    }

    pub fn any(self) -> bool {
        self.subject || self.place || self.day
    }

    pub fn is_none(self) -> bool {
        !self.any()
    }
}

/// One proposition about the world. Authored, minted from an event, or coined by a
/// mouth. Its identity is the id: the same fact is the same fact however many
/// mouths it has been through and however wrong it has got.
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    pub id: FactId,
    /// Dense handle. Never authored, never rendered, never serialised.
    pub key: FactKey,
    /// Mint order, and the garble seed's first term — so a chain is
    /// reconstructible from [`Held::from`] rather than logged.
    pub sequence: i64,
    pub subject: Vec<ActorId>,
    pub place: Option<AreaKey>,
    pub day: Option<i64>,
    /// A **template**, never a sentence: `{subject}`, `{place}`, `{day}`.
    /// [`render_line`] is the only thing that turns it into words, because the
    /// words depend on who is reading.
    pub said: String,
    /// First-person templates for the people who did it or were there. The same
    /// fact, in their own mouth — which is what stops a ward of holders saying one
    /// sentence.
    pub own: BTreeMap<ActorId, String>,
    /// hops-0 holders: authored, or everyone within earshot at mint.
    pub seeded: BTreeSet<ActorId>,
    pub garble: GarbleMask,
    /// Authored standing facts do not cool. Event-minted news does, and so does
    /// everything a mouth coins.
    pub decays: bool,
    pub topic: Topic,
    pub minted_game_days: Option<f64>,
    source: FactSource,
}

impl Fact {
    /// The one reader of provenance outside `source.rs` is
    /// [`invalidate_stale`]; everything else goes through
    /// [`is_claimed`](Self::is_claimed) and [`claimant`](Self::claimant).
    pub(crate) fn source(&self) -> &FactSource {
        &self.source
    }

    /// Whether an LLM made it. Safe to log; the payload is not.
    pub fn is_claimed(&self) -> bool {
        self.source.is_claimed()
    }

    /// The mouth to walk back to, for the chain and for invalidation.
    pub fn claimant(&self) -> Option<&ActorId> {
        self.source.claimant()
    }

    /// Whether this proposition is about this person. The subject is never told
    /// about themselves in the third person, and never carries it as news.
    pub fn is_about(&self, actor: &ActorId) -> bool {
        self.subject.contains(actor)
    }
}

/// A carrier's version, as **deltas from the fact, never as text**.
///
/// At 20,000 people a rendered `String` per holding is a megabyte of garbled
/// sentences the renderer would rebuild anyway — and deltas are what make the
/// chain reconstructible instead of merely logged. Deliberately carries no
/// `topic`, so "topic is invariant under garbling" is *unrepresentable* rather
/// than merely asserted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FactView {
    pub subject: Option<ActorId>,
    pub place: Option<AreaKey>,
    pub day_offset: i8,
}

impl FactView {
    /// Whether this version is the fact itself, undrifted.
    pub fn is_pristine(&self) -> bool {
        self.subject.is_none() && self.place.is_none() && self.day_offset == 0
    }
}

/// What one person currently has of it. `heat` is **derived**, not stored, so it
/// cannot drift with the poll rate and a clock-less world does not age.
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub hops: u8,
    /// Who they had it from. `None` for a seeded holder — they had it from being
    /// there. This is the walk-the-chain link.
    pub from: Option<ActorId>,
    pub learned_on: Option<f64>,
    /// The fact as *this* person has it, after per-hop garbling.
    pub view: FactView,
    heat_at_learn: f32,
}

impl Held {
    /// `heat_at_learn × λ^t`, `t` in game hours since learning, with
    /// `λ = 0.5^(1/AIR_HALF_LIFE_GAME_HOURS)`.
    ///
    /// `heat_at_learn` is returned untouched when either side is `None` — the same
    /// "no clock means nothing ages" tolerance `notices::expire` takes, which is
    /// what keeps the clock-less golden world stable. The hop loss is charged
    /// once, at the hop, so it is already inside `heat_at_learn`.
    pub fn heat(&self, game_days: Option<f64>) -> f32 {
        cooled(self.heat_at_learn, self.learned_on, game_days)
    }

    pub fn is_first_hand(&self) -> bool {
        self.hops == 0
    }

    /// A hops-0 holder who was there: heat 1.0, no chain link, no drift, and no
    /// learning stamp at all, so nothing about them can age. M2 gives this the
    /// `&Fact` it needs to stamp news and leave standing truth alone.
    fn seeded() -> Self {
        Self {
            hops: 0,
            from: None,
            learned_on: None,
            view: FactView::default(),
            heat_at_learn: 1.0,
        }
    }

    fn carried(
        hops: u8,
        from: Option<ActorId>,
        heat: f32,
        on: Option<f64>,
        view: FactView,
    ) -> Self {
        Self {
            hops,
            from,
            learned_on: on,
            view,
            heat_at_learn: heat,
        }
    }
}

/// What arrives, before the merge decides what to do with it.
///
/// Named `Telling` and not `Arrival` because [`crate::round::Arrival`] already
/// exists and is already re-exported at the crate root — the second `Arrival`
/// could not be re-exported beside it. It reads better anyway.
#[derive(Debug, Clone, PartialEq)]
pub struct Telling {
    pub hops: u8,
    pub from: Option<ActorId>,
    pub heat: f32,
    pub view: FactView,
}

/// What the merge did. Returned so a test asserts the rule directly instead of
/// inferring it from state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Learned {
    Fresh,
    Corrected,
    Warmed,
    Unchanged,
    Refused,
}

/// The stored row. Interned and flattened to 88 bytes (4 + 1 + 4 + 16 + 24 + 32 =
/// 81, and the 8-aligned `learned_on`/`from`/`view` round it up), so six of them
/// are ~530 B rather than ~1.6 KB.
#[derive(Debug, Clone, PartialEq)]
struct Holding {
    key: FactKey,
    hops: u8,
    heat_at_learn: f32,
    learned_on: Option<f64>,
    from: Option<ActorId>,
    view: FactView,
}

impl Holding {
    /// The row's current heat, the same derivation [`Held::heat`] makes — what
    /// the eviction ranks on, so "coldest" means coldest *now* and not coldest
    /// at the moment it was learned.
    fn heat(&self, game_days: Option<f64>) -> f32 {
        cooled(self.heat_at_learn, self.learned_on, game_days)
    }

    fn held(&self) -> Held {
        Held {
            hops: self.hops,
            from: self.from.clone(),
            learned_on: self.learned_on,
            view: self.view.clone(),
            heat_at_learn: self.heat_at_learn,
        }
    }

    fn of(key: FactKey, held: Held) -> Self {
        Self {
            key,
            hops: held.hops,
            heat_at_learn: held.heat_at_learn,
            learned_on: held.learned_on,
            from: held.from,
            view: held.view,
        }
    }
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// What this city knows and who holds it.
///
/// Lives on [`World`] beside `notices` and `marks`, and like them it is **prompt
/// state, never carriage state**: it never calls `touch_public_state` and never
/// enters `PublicSnapshot`, whose 160 KiB bound has little headroom left.
/// `holdings` is behind an [`Arc`] because `World::market_sale` does
/// `let mut staged = self.clone()` on **every** catalog sale, so a bare map would
/// deep-copy the whole store per transaction. Write through `Arc::make_mut`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Knowledge {
    live: BTreeMap<FactKey, Fact>,
    by_id: BTreeMap<FactId, FactKey>,
    next_key: u32,
    next_sequence: i64,
    holdings: Arc<BTreeMap<ActorId, Vec<Holding>>>,
}

impl Knowledge {
    pub fn fact(&self, key: FactKey) -> Option<&Fact> {
        self.live.get(&key)
    }

    pub fn fact_by_id(&self, id: &FactId) -> Option<&Fact> {
        self.by_id.get(id).and_then(|key| self.live.get(key))
    }

    pub fn key_of(&self, id: &FactId) -> Option<FactKey> {
        self.by_id.get(id).copied()
    }

    /// Ascending [`FactKey`], i.e. mint order — stable for every prompt and
    /// golden.
    pub fn facts(&self) -> impl Iterator<Item = (FactKey, &Fact)> {
        self.live.iter().map(|(key, fact)| (*key, fact))
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn holdings_len(&self, actor: &ActorId) -> usize {
        self.holdings.get(actor).map_or(0, Vec::len)
    }

    /// The stored row for this actor, if any — the private-map read
    /// [`holds_key`] performs, named so the store-before-seeded order reads as
    /// one lookup.
    fn stored(&self, actor: &ActorId, key: FactKey) -> Option<Held> {
        self.holdings
            .get(actor)?
            .iter()
            .find(|row| row.key == key)
            .map(Holding::held)
    }

    /// What the store actually costs, for the `--extra-ambient 20000` bound. An
    /// estimate of the heap, not a `size_of`: the point is the order of magnitude
    /// a cost guard asserts against.
    pub fn footprint_bytes(&self) -> usize {
        let fact_bytes: usize = self
            .live
            .values()
            .map(|fact| {
                std::mem::size_of::<Fact>()
                    + fact.id.as_str().len()
                    + fact.said.len()
                    + fact
                        .own
                        .iter()
                        .map(|(who, line)| who.as_str().len() + line.len() + 48)
                        .sum::<usize>()
                    + fact
                        .seeded
                        .iter()
                        .map(|who| who.as_str().len() + 48)
                        .sum::<usize>()
                    + fact
                        .subject
                        .iter()
                        .map(|who| who.as_str().len() + 24)
                        .sum::<usize>()
                    + 48
            })
            .sum();
        let holding_bytes: usize = self
            .holdings
            .iter()
            .map(|(actor, rows)| {
                actor.as_str().len()
                    + 48
                    + std::mem::size_of::<Vec<Holding>>()
                    + rows.len() * std::mem::size_of::<Holding>()
            })
            .sum();
        std::mem::size_of::<Self>()
            + fact_bytes
            + holding_bytes
            + self
                .by_id
                .keys()
                .map(|id| id.as_str().len() + 48)
                .sum::<usize>()
    }

    /// The only way a fact enters the world. `None` on a duplicate id, on a
    /// duplicate key (a `Fact` built without [`Self::next_handles`] and installed
    /// under a handle already live would otherwise overwrite that fact while
    /// `by_id` kept pointing the old id at it), and `None` when the store is full
    /// at [`FACTS_MAX_LIVE`].
    ///
    /// M1 evicts nothing: ranking `live` for eviction needs the ward heat, which
    /// is M2's. M2 replaces the flat refusal with a ranking that never evicts a
    /// standing fact.
    pub fn install(&mut self, fact: Fact) -> Option<FactKey> {
        if self.by_id.contains_key(&fact.id) || self.live.contains_key(&fact.key) {
            return None;
        }
        if self.live.len() >= FACTS_MAX_LIVE {
            return None;
        }
        let key = fact.key;
        self.by_id.insert(fact.id.clone(), key);
        self.live.insert(key, fact);
        Some(key)
    }

    /// Drops it from `live`, from `by_id` and from every holding.
    pub fn invalidate(&mut self, key: FactKey) -> Option<Fact> {
        let fact = self.live.remove(&key)?;
        self.by_id.remove(&fact.id);
        let holdings = Arc::make_mut(&mut self.holdings);
        holdings.retain(|_, rows| {
            rows.retain(|row| row.key != key);
            !rows.is_empty()
        });
        Some(fact)
    }

    /// Allocates the next `(FactKey, sequence)` pair. The sequence is the garble
    /// seed's first term, so it is monotone and never reused.
    pub(crate) fn next_handles(&mut self) -> (FactKey, i64) {
        let key = FactKey(self.next_key);
        self.next_key = self.next_key.saturating_add(1);
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        (key, sequence)
    }
}

// ---------------------------------------------------------------------------
// The free functions
// ---------------------------------------------------------------------------

/// **`None` means they have never heard of it at all.**
///
/// Checks the fact's `seeded` set first — hops 0, heat 1.0, no garble,
/// `from: None` — and returns without reading the carrier store, so a stray row
/// can never be read as a garbled first-hand holding. Every consumer goes through
/// this and nothing else.
///
/// Always call it fully qualified, `knowledge::holds`, and never `use` it: it is
/// one letter from [`Character::holds`](crate::character::Character::holds) (the
/// hands) and [`Custody::holds`](crate::custody::Custody::holds) (the law).
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held> {
    let key = world.knowledge.key_of(fact)?;
    holds_key(world, actor, key)
}

/// [`holds`], by dense handle.
pub fn holds_key(world: &World, actor: &ActorId, key: FactKey) -> Option<Held> {
    let fact = world.knowledge.fact(key)?;
    if fact.seeded.contains(actor) {
        return Some(Held::seeded());
    }
    world.knowledge.stored(actor, key)
}

/// Everything this person has, seeded rows and carried rows together, ascending
/// [`FactKey`].
pub fn holdings_of(world: &World, actor: &ActorId) -> Vec<(FactKey, Held)> {
    world
        .knowledge
        .facts()
        .filter_map(|(key, _)| holds_key(world, actor, key).map(|held| (key, held)))
        .collect()
}

/// The merge rule, all four rows, in one place — and the store's only writer of a
/// carried holding:
///
/// | arrival | effect |
/// |---|---|
/// | fewer hops than held | **replaces** `view`, `from`, `hops`; heat takes the max → [`Learned::Corrected`] |
/// | equal hops | keeps the held `view`; heat takes the max → `Warmed`, or `Unchanged` if the max did not move a whole percent |
/// | more hops | heat takes the max and nothing else changes → `Warmed` / `Unchanged` |
/// | held at hops 0 | ignored entirely → `Refused`. A witness cannot be talked out of what they saw |
///
/// Nothing held: `Fresh`. Heat comparisons go through [`heat_pct`], so an arrival
/// that is warmer only in the seventh decimal is `Unchanged`. Over
/// [`HOLDINGS_MAX`] it evicts coldest first, then most hops, then highest
/// `FactKey` — a total order, so eviction is reproducible.
///
/// One consequence of that order, stated because a caller counting pickups will
/// meet it: an arrival colder than everything already held at a full store is
/// **its own** eviction victim, so `Fresh` names the merge's decision and not the
/// row's survival. `knowledge::holds` afterwards is the only authority on whether
/// they have it (`a_cold_arrival_at_a_full_store_is_its_own_victim`).
///
/// **Nothing in production calls this in M1.** M2's pickup does. It ships here
/// because a store nobody can write is a store nobody can test, and because the
/// four rows are the definition of [`Held`]'s invariants.
pub fn learn(
    world: &mut World,
    actor: &ActorId,
    key: FactKey,
    telling: Telling,
    game_days: Option<f64>,
) -> Learned {
    // An unknown key is nothing to learn — never a silent insert of a row whose
    // fact does not exist, which `holdings_of` would then skip forever.
    if world.knowledge.fact(key).is_none() {
        return Learned::Refused;
    }
    let existing = holds_key(world, actor, key);

    let Some(held) = existing else {
        let fresh = Held::carried(
            telling.hops,
            telling.from,
            telling.heat,
            game_days,
            telling.view,
        );
        insert_holding(
            &mut world.knowledge,
            actor,
            Holding::of(key, fresh),
            game_days,
        );
        return Learned::Fresh;
    };

    // Row four, and it wins over the other three: a hops-0 holder was there.
    if held.hops == 0 {
        return Learned::Refused;
    }

    let held_heat = held.heat(game_days);
    let warmer = heat_pct(telling.heat) > heat_pct(held_heat);
    let closer = telling.hops < held.hops;

    let holdings = Arc::make_mut(&mut world.knowledge.holdings);
    let Some(row) = holdings
        .get_mut(actor)
        .and_then(|rows| rows.iter_mut().find(|row| row.key == key))
    else {
        // `holds_key` answered from `seeded`, which the hops-0 arm already
        // refused, so this is unreachable; treat it as nothing to change.
        return Learned::Unchanged;
    };

    // Heat always takes the maximum, and nothing else about the row moves unless
    // the arrival is closer — so `Unchanged` really changes nothing.
    if warmer {
        row.heat_at_learn = telling.heat;
        row.learned_on = game_days;
    }

    if closer {
        row.hops = telling.hops;
        row.from = telling.from;
        row.view = telling.view;
        return Learned::Corrected;
    }

    if warmer {
        Learned::Warmed
    } else {
        Learned::Unchanged
    }
}

/// Insert a fresh row, keeping the vector ascending by key and honouring
/// [`HOLDINGS_MAX`].
///
/// The eviction ranks on the **derived** heat at `game_days`, never on
/// `heat_at_learn`: a row learned hot a game day ago has cooled past one learned
/// lukewarm this minute, and ranking on the stored number would keep six dead
/// rows and throw out the only warm arrival — a carrier who once caught six hot
/// stories could then never take anything cooler than their historical maximum.
fn insert_holding(
    knowledge: &mut Knowledge,
    actor: &ActorId,
    row: Holding,
    game_days: Option<f64>,
) {
    let holdings = Arc::make_mut(&mut knowledge.holdings);
    let rows = holdings.entry(actor.clone()).or_default();
    let at = rows
        .binary_search_by(|existing| existing.key.cmp(&row.key))
        .unwrap_or_else(|position| position);
    rows.insert(at, row);
    if rows.len() > HOLDINGS_MAX {
        // Coldest first, then most hops, then highest key — a total order, so the
        // eviction is reproducible run to run.
        let victim = rows
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                heat_pct(left.heat(game_days))
                    .cmp(&heat_pct(right.heat(game_days)))
                    .then_with(|| right.hops.cmp(&left.hops))
                    .then_with(|| right.key.cmp(&left.key))
            })
            .map(|(index, _)| index);
        if let Some(index) = victim {
            rows.remove(index);
        }
    }
}

/// Drop every fact the world no longer bears out, and every holding of it.
/// Returns what died, for a diagnostic.
///
/// M5 calls it from `pollen::sweep` on every stir beat (half a game hour); in M1
/// only tests do. A dropped fact leaves every sheet on the next turn with no
/// `forget`, no LLM cooperation and no drift.
pub fn invalidate_stale(world: &mut World) -> Vec<FactId> {
    let doomed: Vec<FactKey> = {
        let reading: &World = world;
        reading
            .knowledge
            .facts()
            .filter(|(_, fact)| !fact.source().still_true(reading))
            .map(|(key, _)| key)
            .collect()
    };
    let mut dead = Vec::new();
    for key in doomed {
        if let Some(fact) = world.knowledge.invalidate(key) {
            dead.push(fact.id);
        }
    }
    dead
}

/// Whether this person can ever carry this fact as news: the subject cannot —
/// they hold it at hops 0 because they were there, or not at all.
///
/// A **rule and not a number**, so it sits outside the salience product and
/// neither measurement lever (`--pollen-flat`, `--pollen-no-salience`) can remove
/// it. M2's `pickup_chance` returns 0.0 on it; [`volunteers`] returns false on it;
/// M3's `hop_on_stage` skips a listener on it.
pub(crate) fn may_carry(fact: &Fact, who: &ActorId) -> bool {
    !fact.subject.contains(who)
}

/// Whether this holding is warm enough to be said unasked — and therefore whether
/// it may take a heat seat on the sheet, and which register it renders in.
///
/// `heat × salience`, exactly the product that drives pickup in M2, so a person
/// repeats a thing on the same terms they caught it. Never for the subject
/// ([`may_carry`]), and **never for a standing fact** (`!fact.decays`): authored
/// truth is answerable through relevance, never volunteered — with the M2 model
/// as first written a `decays: false` holder sat at heat 1.0 forever and deposited
/// on every poll, which is the opposite of "travels almost nowhere".
///
/// One function, one home, three callers by M3 (Layer 1's deposit, Layer 2's
/// carrier test, [`render_line`]'s cold row). M1 uses the topic's **base** band;
/// M2 replaces `world.salience.base(fact.topic)` with
/// `salience::salience_for(world, fact, holder)` at this one site and changes
/// nothing else.
pub(crate) fn volunteers(
    world: &World,
    fact: &Fact,
    holder: &ActorId,
    held: &Held,
    game_days: Option<f64>,
) -> bool {
    if !fact.decays || !may_carry(fact, holder) {
        return false;
    }
    f64::from(held.heat(game_days)) * world.salience.base(fact.topic) > f64::from(VOLUNTEER_HEAT)
}

/// The relevance limb of sheet selection, on its own: the facts whose id segments,
/// subject name or place label appear in `since` or `recent`, in ascending
/// [`FactKey`], with the self-subject filter already applied.
///
/// **The** definition — the sheet calls it, and (M4) the re-heat calls it with the
/// same two vectors, so the thing that gets warmed is exactly the thing that got
/// seated. Two implementations of "relevant" would drift.
///
/// "The hottest thing this actor carries" is a gossip rule and the wrong rule for
/// an interrogation: ask about the bale while the ward is loud about an arrest and
/// the one fact you came for is off the sheet.
pub(crate) fn relevance_seated(
    world: &World,
    actor: &Character,
    since: &[&str],
    recent: &[&str],
) -> Vec<FactKey> {
    if !world.knowledge_enabled {
        return Vec::new();
    }
    let reader = actor.id();
    let haystack: String = since
        .iter()
        .chain(recent.iter())
        .map(|line| line.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    let mut seated = Vec::new();
    for (key, _held) in holdings_of(world, reader) {
        let Some(fact) = world.knowledge.fact(key) else {
            continue;
        };
        // The self-subject filter, before relevance and before heat. A subject who
        // *does* hold an `own` line keeps it: if they hold it, they hold their own
        // line or nothing.
        if fact.is_about(reader) && !fact.own.contains_key(reader) {
            continue;
        }
        if relevance_tokens(world, fact)
            .iter()
            .any(|token| haystack.contains(token.as_str()))
        {
            seated.push(key);
        }
    }
    seated
}

/// The words an asker will use for this fact: its id's own segments, its
/// subjects' names, and its place's label.
///
/// The id's dot-separated segments are the load-bearing half — `ashe.salt.short`
/// answers "who holds the salt contract now?", which is the adjacent-ask dead end
/// M0 measured, moved from prose into selection. Always the **canonical**
/// `fact.subject`, never a garbled view: matching on a swapped subject would seat
/// the wrong fact.
fn relevance_tokens(world: &World, fact: &Fact) -> Vec<String> {
    let mut tokens = Vec::new();
    for segment in fact.id.as_str().split('.') {
        if segment.chars().count() >= 4 {
            tokens.push(segment.to_lowercase());
        }
    }
    for subject in &fact.subject {
        // `get`, never `[]`: `FactCatalog::seed` drops ids this world lacks, and
        // an index would panic on a quest pack naming somebody absent.
        if let Some(character) = world.characters.get(subject) {
            for word in character.name().split_whitespace() {
                if word.chars().count() >= 4 {
                    tokens.push(word.to_lowercase());
                }
            }
        }
    }
    if let Some(label) = fact
        .place
        .and_then(|place| world.area_map.label_of_key(place))
    {
        for word in label.split_whitespace() {
            // The length rule drops "The" without a stop list.
            if word.chars().count() >= 5 {
                tokens.push(word.to_lowercase());
            }
        }
    }
    tokens
}

/// Which rung of the frozen ladder a telling sits on. Seven per band, all
/// measured (`m0_evidence/strings_draft.toml`); M3 adds none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rung {
    Own,
    Cold,
    Hops0,
    Hops1,
    Hops2,
    Hops3,
    Hops4,
}

/// The one precedence rule for the whole feature.
///
/// The own line wins whatever the heat — a cold subject-witness rendered in the
/// `cold` register would be told `said` about themselves — and that is the one
/// departure from the measured rig, which checked cold first even over
/// `hops0_own`. After it, cold beats every hop count including 0: a hops-0 holder
/// of a decaying fact does go cold, and the measured rig rendered them faded.
///
/// `band` is deliberately not an input: the precedence rule is the same in every
/// column, and the band only chooses which of the three strings a rung renders.
pub(crate) fn rung_for(hops: u8, cold: bool, has_own: bool) -> Rung {
    if has_own {
        return Rung::Own;
    }
    if cold {
        return Rung::Cold;
    }
    match hops {
        0 => Rung::Hops0,
        1 => Rung::Hops1,
        2 => Rung::Hops2,
        3 => Rung::Hops3,
        _ => Rung::Hops4,
    }
}

/// The 21-cell lookup, by key name. **No band-shift**: `top`/hops 2 is
/// `know_hedge_top_hops2` ("They say:"), never `default`/hops 1 — a band-shifted
/// layout would put a one-remove claim in front of a top-band telling at four
/// removes, which is a provenance lie the prompt itself would be making.
pub(crate) fn hedge_of(strings: &PromptStrings, band: HedgeBand, rung: Rung) -> &str {
    match (band, rung) {
        (HedgeBand::Default, Rung::Own) => &strings.know_hedge_default_hops0_own,
        (HedgeBand::Default, Rung::Hops0) => &strings.know_hedge_default_hops0,
        (HedgeBand::Default, Rung::Hops1) => &strings.know_hedge_default_hops1,
        (HedgeBand::Default, Rung::Hops2) => &strings.know_hedge_default_hops2,
        (HedgeBand::Default, Rung::Hops3) => &strings.know_hedge_default_hops3,
        (HedgeBand::Default, Rung::Hops4) => &strings.know_hedge_default_hops4,
        (HedgeBand::Default, Rung::Cold) => &strings.know_hedge_default_cold,
        (HedgeBand::Top, Rung::Own) => &strings.know_hedge_top_hops0_own,
        (HedgeBand::Top, Rung::Hops0) => &strings.know_hedge_top_hops0,
        (HedgeBand::Top, Rung::Hops1) => &strings.know_hedge_top_hops1,
        (HedgeBand::Top, Rung::Hops2) => &strings.know_hedge_top_hops2,
        (HedgeBand::Top, Rung::Hops3) => &strings.know_hedge_top_hops3,
        (HedgeBand::Top, Rung::Hops4) => &strings.know_hedge_top_hops4,
        (HedgeBand::Top, Rung::Cold) => &strings.know_hedge_top_cold,
        (HedgeBand::Low, Rung::Own) => &strings.know_hedge_low_hops0_own,
        (HedgeBand::Low, Rung::Hops0) => &strings.know_hedge_low_hops0,
        (HedgeBand::Low, Rung::Hops1) => &strings.know_hedge_low_hops1,
        (HedgeBand::Low, Rung::Hops2) => &strings.know_hedge_low_hops2,
        (HedgeBand::Low, Rung::Hops3) => &strings.know_hedge_low_hops3,
        (HedgeBand::Low, Rung::Hops4) => &strings.know_hedge_low_hops4,
        (HedgeBand::Low, Rung::Cold) => &strings.know_hedge_low_cold,
    }
}

/// Two to seven, in words. A digit never reaches a sheet: a number in front of a
/// model is a number it says back.
const DAYS_IN_WORDS: [&str; 6] = ["two", "three", "four", "five", "six", "seven"];

/// Turn one holding into the sentence **this reader** would say.
///
/// The only place in the crate that turns a fact into words, which is what makes
/// the unknown-people rule a property of the type rather than a promise: there is
/// no baked name to leak, garbling has something to move, and the role hint
/// answers the real bite of the rule (no NPC ever learns a name at runtime, so
/// without it essentially every third-party subject would render as one
/// undifferentiated stranger).
///
/// `None` when the fact is about the reader and they have no `own` line — nobody
/// is told about themselves in the third person.
pub fn render_line(
    world: &World,
    reader: &ActorId,
    key: FactKey,
    held: &Held,
    strings: &PromptStrings,
    game_days: Option<f64>,
) -> Option<String> {
    let fact = world.knowledge.fact(key)?;
    // The **fact's** band, never the reader's affinity: a topic is a property of
    // the proposition.
    let band = world.salience.hedge_band(fact.topic);
    let has_own = fact.own.contains_key(reader);
    let cold = !volunteers(world, fact, reader, held, game_days);
    let rung = rung_for(held.hops, cold, has_own);

    // The effective subject: this reader's version if a hop moved it, else the
    // fact's own. Resolved before the self test, because a swapped subject that
    // lands on the reader would otherwise render their own name to them in the
    // third person. M3's `view_for` must never pick the holder; this is the
    // renderer's own guard, so the rule does not depend on it.
    let effective_subject = held.view.subject.as_ref().or_else(|| fact.subject.first());

    let template = if rung == Rung::Own {
        // `own` keys are validated as a subset of `seeded` and `holds` answers
        // `seeded` first, so an `own` holder is always hops 0. The own template
        // renders alone inside `*_hops0_own`'s `%s`, with no other wrapper: a
        // narrator in front of a witness's own first-person words swaps person
        // mid-sentence, and that wrapper was rejected in all three M0 variants.
        fact.own.get(reader)?.clone()
    } else if fact.is_about(reader) || effective_subject == Some(reader) {
        return None;
    } else {
        fact.said.clone()
    };

    let subject_word = effective_subject.map(|who| person_word(world, reader, who, strings));
    let place_word = held
        .view
        .place
        .or(fact.place)
        .and_then(|place| world.area_map.label_of_key(place).map(str::to_string))
        .unwrap_or_else(|| strings.place_unknown.clone());
    let day_word = day_word(world, fact, held, strings);

    let mut sentence = template;
    if let Some(subject) = subject_word {
        sentence = sentence.replace("{subject}", &subject);
    }
    sentence = sentence.replace("{place}", &place_word);
    sentence = sentence.replace("{day}", &day_word);

    // The rung is substituted last and exactly once, which is why the loader
    // refuses a template naming `%s`.
    Some(hedge_of(strings, band, rung).replacen("%s", &sentence, 1))
}

/// The one way an [`ActorId`] becomes a word on a sheet.
///
/// The observer-aware ladder, and the whole of the unknown-people rule: their own
/// name, the real name if the reader has been told it, else the trade-and-ward
/// role, else the nameless fallback. M3's `known_from` and M4's journal resolve a
/// teller through this same function; nothing else may turn an id into a word.
pub(crate) fn person_word(
    world: &World,
    reader: &ActorId,
    who: &ActorId,
    strings: &PromptStrings,
) -> String {
    let subject = world.characters.get(who);
    let told = who == reader
        || world
            .characters
            .get(reader)
            .is_some_and(|character| character.knows().contains(who));
    if let Some(character) = subject.filter(|_| told) {
        return character.name().to_string();
    }
    if let Some((profile, display)) = subject.and_then(Character::lore).and_then(|profile| {
        profile
            .occupation_display
            .as_deref()
            .map(|display| (profile, display))
    }) {
        let trade = lower_first(display);
        let role = strings
            .unknown_person_role
            .replacen("%s", &trade, 1)
            .replacen("%s", &crate::prompt::ward_label(profile.planning_ward), 1);
        // Five of the 65 occupation displays begin with a vowel ("Anchoress",
        // "Instrument maker", "Executioner", "Animal worker", "Entertainer"), so
        // "a anchoress" would otherwise reach a sheet.
        return match role.strip_prefix("a ") {
            Some(rest) if starts_with_vowel(&trade) => format!("an {rest}"),
            _ => role,
        };
    }
    // No profile, no `occupation_display` (the no-trade quarter), or an actor this
    // world does not have.
    strings.unknown_person_name.clone()
}

/// Only the first character lowered — all 65 occupation displays in
/// `lore/core_lore/occupations.json` are common-noun phrases, so this is
/// mechanical case-folding and not prose.
fn lower_first(display: &str) -> String {
    let mut characters = display.chars();
    match characters.next() {
        Some(first) => first.to_lowercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn starts_with_vowel(word: &str) -> bool {
    matches!(
        word.chars().next(),
        Some('a') | Some('e') | Some('i') | Some('o') | Some('u')
    )
}

/// `{day}`, relative to the world clock and never as a raw number.
///
/// `day_long_ago` covers the clock-less case (every golden fixture, every hermetic
/// world), the undated fact and eight days and beyond; a future-dated delta clamps
/// to `day_today`. Saturating arithmetic, because `day` is authored data reachable
/// from a `--facts` pack and an absurd value must render a phrase, never panic a
/// sheet.
fn day_word(world: &World, fact: &Fact, held: &Held, strings: &PromptStrings) -> String {
    match (world.current_time, fact.day) {
        (Some(now), Some(day)) => match now
            .day
            .saturating_sub(day.saturating_add(i64::from(held.view.day_offset)))
        {
            days if days <= 0 => strings.day_today.clone(),
            1 => strings.day_yesterday.clone(),
            days @ 2..=7 => {
                strings
                    .day_days_past
                    .replacen("%s", DAYS_IN_WORDS[(days - 2) as usize], 1)
            }
            _ => strings.day_long_ago.clone(),
        },
        _ => strings.day_long_ago.clone(),
    }
}

#[cfg(test)]
mod tests;
