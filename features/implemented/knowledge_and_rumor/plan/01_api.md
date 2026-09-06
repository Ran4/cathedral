# 01 — The API

Paste-ready skeleton. Every signature is final; a milestone adds bodies, not shapes. Doc comments are
in the crate's own voice (state the rule and the reason it is that rule, not the mechanics).

Read `00_decisions.md` first — the decisions this file implements are numbered there and cited as
`(D-n)`.

## Module layout

```
crates/cathedral-sim/src/knowledge/          (the milestone that creates each file)
    mod.rs       M1  Fact, Held, FactView, Telling, Learned, Topic, GarbleMask, Knowledge, holds(),
                     holds_key(), holdings_of(), learn(), render_line(), volunteers(), may_carry(),
                     relevance_seated(), invalidate_stale(), heat_pct(), the constants
                 M2  frozen_ears(); Fact gains quiet_among/craft_ear; Knowledge gains air + the sweep beat
                 M3  Rung, rung_for(), chain()
                 M4  Occasion, LearnedHow, reheat(), render_plain(), when_phrase(), standing_lines(),
                     the occasion store and the receipt store on Knowledge
                 M5  holds_about(), door_is_shut(), raise_hearsay_words()
    source.rs    M1  FactSource — the sealed provenance field (D11), still_true() with every arm's body
    catalog.rs   M1  FactCatalog, FactSpec, facts.json loading + validation, quest packs
    salience.rs  M1  SalienceTable, HedgeBand, salience.json, flat()      M2  salience(), salience_for()
    pollen.rs    M2  Drift, WardGrid, AreaAdjacency, poll_person, poll_player, sweep, picks_up,
                     pickup_chance, PollenCensus, TopicRow, CrossingTally, census, debug_seed_air
                 M3  hop_on_stage, picks_up_from                          M5  amplify, ward_centroids
    garble.rs    M3  view_for(), same_ward_or_trade()          (does not exist before M3 — M3 creates it)
    mint.rs      M2  MintKind, MINT_KINDS, install_fact(), mint(), mint_commitment(), mint_from_notice(),
                     plant_for_measurement(), mint_knell() stub
                 M4  mint_claim(), collides_in_air(), note_assertion(), note_unminted_event()
                 M5  mint_knell() body, mint_stranger_deed(), peal_topic()
```

`lib.rs`: `pub mod knowledge;` (between `item` and `lore`, `lib.rs:31-32`), and **no re-export of
anything from `knowledge`** (D56). The two dense keys join the ids line:

```rust
// lib.rs
pub mod knowledge;
pub use ids::{ActorId, AreaId, AreaKey, DogId, FactId, FactKey, InvalidId, ItemId, PartyId, PlaceId,
              RequestId, SpeechEventId};
// `JournalEntry` (M4) and `CivicRope` (M5) ship beside `EngineMessage`, so they join
// `pub use engine::{…}`. Everything else is `cathedral_sim::knowledge::…`.
// Verified free of collisions repo-wide: `Arrival` is the only name in this
// feature that was already taken.
```

---

## ids.rs

```rust
id_newtype!(FactId,  "a proposition the city repeats");
id_newtype!(AreaId,  "a named area");
```
Both get `new()` → `Result<Self, InvalidId>` and `from_raw()` for seed data (`ids.rs:51`, `:61`).
Use `from_raw` at the `AreaMap` boundary (D5).

---

## knowledge/mod.rs — the constants

Every value and its derivation are in `02_numbers.md`. Copy the doc comments from there; do not
re-derive.

```rust
/// The ward's air halves every half game day. Sets how long a top-band fact is
/// news (37.4 game hours — a game day and a half) and every warm life below it.
pub const AIR_HALF_LIFE_GAME_HOURS: f64 = 12.0;

/// The one free parameter, solved from the slow end (`02_numbers.md` §3, D53):
/// the smallest heat at which an off-affinity `Craft` witness stops repeating
/// himself **before the leg that fires at the mint bell carries him over the
/// nearest ward boundary** (82 m from the Wickmarket, 0.26 game hours at
/// walking pace) — so a spoiled batch dies in its own lane. Warm life at 0.12
/// is 0.145 gh, about nine game minutes. Also what a fact must clear to be
/// seated on a sheet on its own.
pub const VOLUNTEER_HEAT: f32 = 0.119;

/// The second free parameter, attached to the fast end. One roll per person
/// per fact per stir; the poll gap must stay under `60 / STIRS_PER_GAME_HOUR`
/// or a person can skip a whole window (`the_poll_gap_cannot_skip_a_stir`).
pub const STIRS_PER_GAME_HOUR: f64 = 2.0;

/// A person's own turn at the air, in game minutes, jittered ×0.5..1.5 off
/// their id. Game time, never real seconds: the `T` key's 60× and
/// `--watch-clock` must give a played run's roll count (D22).
pub const POLLEN_POLL_GAME_MINUTES: f64 = 10.0;
pub const POLLEN_POLL_MAX_GAME_MINUTES: f64 = 15.0;

/// Charged once, at the hop, so heat at hops n is a clean `HOP_LOSS^n × λ^t`.
/// Bracketed by two inequalities, both asserted: `HOP_LOSS^4 > VOLUNTEER_HEAT`
/// (a fourth-hand top-band story is still repeatable, so the ladder's far
/// rungs are reachable) and `HOP_LOSS × 0.12 < VOLUNTEER_HEAT` (an
/// off-affinity trade matter travels one hop and stops).
pub const HOP_LOSS: f32 = 0.85;

/// Where a re-asked cold fact comes back to, and no further — an absolute
/// heat, never `VOLUNTEER_HEAT / salience`, because heat and salience are
/// separate axes and dividing one by the other re-heats a dull fact hardest,
/// which inverts the whole point. A revived scandal circulates again; a
/// revived stall quarrel is merely answerable. The 1.10 is one rung above the
/// gate: a re-heated top-band fact stays above it for `12·log₂(1.10)` = 1.65
/// game hours (about half an office), and a re-heat deposit yields ≈ 0.7
/// expected new `Bed` carriers per ask in the Wickmarket's ward.
pub const REHEAT_TO: f32 = VOLUNTEER_HEAT * 1.10_f32;  // 0.1309

/// Never 0.0. Exponential decay underflows to exactly zero and `0.0 < 0.0` is
/// false forever, which makes the row immortal and holds a cap slot nothing
/// can reclaim (`marks.rs:505`).
pub const HEAT_GONE_BELOW: f32 = 0.01;

/// P(a masked field is wrong) per hop. Both readings the spec invites — right
/// two times in three at one hop, wrong four in five at four — land on ⅓;
/// 0.35 is the rounder number just above it. Right 0.650 / 0.423 / 0.275 /
/// 0.179 at hops 1–4.
pub const GARBLE_CHANCE_PER_HOP: f64 = 0.35;
/// ±1 per garbled hop, clamped here. A guard, not a shape: three same-direction
/// day garbles by hops 3 is (0.35/2)³ ≈ 0.5%, so the clamp almost never binds
/// and exists so a deep chain cannot wander a week.
pub const DAY_OFFSET_MAX: i8 = 3;
pub const GARBLE_AREA_RADIUS_M: f64 = 120.0;

/// A person is not a newspaper. Evicted coldest first, then most hops, then
/// `FactKey` — a total order, so eviction is reproducible.
pub const HOLDINGS_MAX: usize = 6;
/// Smaller than `notices::NOTICES_SHEET_MAX` (4), as the spec requires.
pub const KNOWN_SHEET_MAX: usize = 3;
pub const AIR_PER_WARD_MAX: usize = 24;
pub const FACTS_MAX_LIVE: usize = 256;
/// A chain is a reconstruction, not a log; this bounds it against a merge bug
/// that points two holdings at each other inside a prompt render. Heat alone
/// caps hops at ⌊ln(VOLUNTEER_HEAT / (s·λ^t)) / ln(HOP_LOSS)⌋ = 13 for a fresh
/// top-band fact; the practical bound is the pickup rate — about one hop per
/// 1/(S·c̄·s) ≈ 4.2 stirs at the cast's mean curiosity — so eight is past any
/// chain a game day produces.
pub const CHAIN_MAX_LINKS: usize = 8;

/// The player has no lore sheet, so `attention::curiosity_of` would return
/// `CURIOSITY_WITHOUT_LORE` (1.0) and hand them every fact in the air on the
/// first roll. Roughly three times the cast's measured mean: an attentive
/// outsider, not a firehose.
pub const PLAYER_CURIOSITY: f64 = 0.35;
pub const PLAYER_POLL_GAME_MINUTES: f64 = 10.0;

/// Below the 1.27 m minimum separation between any two of the 413 baked doors,
/// so this can only ever fire for generated citizens sharing one `nav::Door`.
pub const HOUSEHOLD_EPSILON_M: f64 = 0.5;

pub const OCCASION_LIFE_GAME_HOURS: f64 = 1.0;
pub const OCCASION_MIN_ASSERTION_CHARS: usize = 24;
pub const RAISES_PER_OFFICE: u8 = 1;

/// Layer 2's own scan is O(N) (`world.rs:478` has no spatial index), so it is
/// gated in real seconds — it is a *rendering* concern, not a sim cadence —
/// and capped at the nearest few carriers.
pub const STAGE_HOP_SECONDS: f64 = 2.0;
pub const STAGE_HOP_MAX_PAIRS: usize = 8;
pub const STAGE_HOP_RADIUS_M: f64 = crate::HEARING_RADIUS_M;
```

Later milestones append to the same block, each with its derivation in its own file:
`WARD_CELL_M = 8.0` (M2 step 2); `GARBLE_SUBJECT_POOL_MAX = 24` (M3 step 1); `PLAYER_RECEIPTS_MAX = 64`,
`JOURNAL_ENTRIES_MAX = 24`, `JOURNAL_PUBLISH_SECONDS = 1.0`, `WHEN_DAYS_MAX = 7` (M4 A1);
`KNELL_CARRY_M = 300.0`, `DOOR_SHUT_REACH_M = 10.0` (M5 steps 1d and 7a, D59). None is a probability.

---

## knowledge/source.rs — the sealed field (D11)

```rust
/// Why a fact is true. **Never rendered anywhere** — not a prompt, a
/// projection, a log line, a journal entry or a `Debug` string.
///
/// Sealed three ways rather than one, because a projection-walking test cannot
/// see `Diagnostic(format!("{fact:?}"))` written by somebody who has not read
/// this comment: the payload is private to this module, there is no
/// `Serialize`/`Deserialize`/`Display` to reach for, and `Debug` prints a
/// placeholder. A fact says *what*; this says why it is so, and in a quest that
/// is usually the answer the player is looking for.
#[derive(Clone, PartialEq, Eq)]
pub struct FactSource(Provenance);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Provenance {
    /// An author bound it to nothing; it is simply so.
    Authored,
    /// Somebody said it. The only thing an LLM can make, and the whole safety
    /// argument for `raise_word` in one variant.
    Claimed(ActorId),
    /// True while the law holds this person.
    Custody(ActorId),
    /// True while this item is still in this pair of hands. Items have no
    /// position in this sim (`item.rs:132`): an item is wherever its holder
    /// stands, so "the item moved" means "it changed hands or left the world".
    ItemWith { item: ItemId, holder: ActorId },
    /// True while this quest phase stands.
    QuestPhase { quest: String, phase: u8 },
    /// Minted from an event and never re-checked.
    Event { kind: String, sequence: i64 },
}

impl std::fmt::Debug for FactSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FactSource(<sealed>)")
    }
}

impl FactSource {
    pub fn authored() -> Self;
    pub fn claimed(by: ActorId) -> Self;
    pub fn custody(of: ActorId) -> Self;
    pub fn item_with(item: ItemId, holder: ActorId) -> Self;
    pub fn quest_phase(quest: impl Into<String>, phase: u8) -> Self;
    pub fn event(kind: impl Into<String>, sequence: i64) -> Self;

    /// A model can mint claims; it can never mint truths. The one bit of
    /// provenance anything outside this module may read.
    pub fn is_claimed(&self) -> bool;
    /// The mouth a raised word walks back to. `None` for every other route.
    pub fn claimant(&self) -> Option<&ActorId>;

    /// Whether the world still bears this out. `false` drops the fact from
    /// `live` and from every holding on the next sweep, which clears it off
    /// every sheet on the next turn with no `forget` and no LLM cooperation.
    /// Every arm has its body from M1 (step 4): `Authored`/`Event`/`QuestPhase`
    /// → true, `Claimed(who)` → the mouth still exists, `Custody(who)` →
    /// `world.custody.holds(who)`, `ItemWith` → the holder still holds it.
    /// Called by `knowledge::invalidate_stale`, which M5 wires into
    /// `pollen::sweep`'s stir beat.
    pub(crate) fn still_true(&self, world: &World) -> bool;
}
```
**Must not derive or implement:** `Serialize`, `Deserialize`, `Display`, `Debug` (derived), `Default`.
`FactSource` must appear in no `EngineMessage`, no `PublicSnapshot` field, and no `format!` outside
this module.

---

## knowledge/mod.rs — the types

```rust
/// What a fact is *about* — and therefore how far it travels, how its hedges
/// erode, and whose ear it catches. A property of the proposition, so it is
/// invariant across every mouth: garbling moves the subject, the place and the
/// day, and never moves this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic { Bed, Blood, Law, Omen, Stranger, Coin, Bread, Craft, Talk }

impl Topic {
    /// Declaration order, which is the order `salience.json` is validated in
    /// and the order `--trace-pollen` prints.
    pub const ALL: [Topic; 9];
    pub fn as_str(self) -> &'static str;
    /// An unrecognised tag lands on `Talk`, the dullest band: a mis-tagged fact
    /// that under-spreads is a shrug, and one that becomes a citywide scandal
    /// is the bug that would make `raise_word` unshippable.
    pub fn parse_or_talk(value: &str) -> Self;
}

/// Which fields a hop may move. The rest is load-bearing truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GarbleMask { pub subject: bool, pub place: bool, pub day: bool }

impl GarbleMask {
    pub const NONE: Self;
    /// `"none"` | `"place,day"` | `"subject,place,day"` — any order; an unknown
    /// token is a load error naming the three legal ones.
    pub fn parse(value: &str) -> Result<Self, FactCatalogError>;
    /// Always `subject,place,day` order, so a round-trip is byte-stable.
    pub fn as_authored(self) -> String;
    /// What `raise_word` gives a claim of this topic before the template rule
    /// narrows it: `ALL`, for every one of the nine — a claim cannot seal
    /// itself against drift, and what it can garble is whatever its sentence
    /// names (`mint_claim` keeps `subject` only when it put a `{subject}` in;
    /// `place`/`day` are never in a free-text claim). One line, no table (D58).
    pub fn default_for(topic: Topic) -> Self;
    pub const ALL: Self;
    pub fn any(self) -> bool;
    pub fn is_none(self) -> bool;
}

/// One proposition about the world. Authored, minted from an event, or coined
/// by a mouth. Its identity is the id: the same fact is the same fact however
/// many mouths it has been through and however wrong it has got.
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    pub id: FactId,
    /// Dense handle. Never authored, never rendered, never serialised.
    pub key: FactKey,
    /// Mint order, and the garble seed's first term — so a chain is
    /// reconstructible from `Held::from` rather than logged.
    pub sequence: i64,
    pub subject: Vec<ActorId>,
    pub place: Option<AreaKey>,
    pub day: Option<i64>,
    /// A **template**, never a sentence: `{subject}`, `{place}`, `{day}`.
    /// `render_line` is the only thing that turns it into words, because the
    /// words depend on who is reading (D16).
    pub said: String,
    /// First-person templates for the people who did it or were there. The
    /// same fact, in their own mouth — which is what stops a ward of holders
    /// saying one sentence.
    pub own: BTreeMap<ActorId, String>,
    /// hops-0 holders: authored, or everyone within earshot at mint.
    pub seeded: BTreeSet<ActorId>,
    pub garble: GarbleMask,
    /// Authored standing facts do not cool. Event-minted news does, and so
    /// does everything a mouth coins.
    pub decays: bool,
    pub topic: Topic,
    pub minted_game_days: Option<f64>,
    /// The subject, their kin and anyone behind the subject's own door —
    /// frozen at mint because both inputs are seed-time facts in this crate
    /// (D38). One `contains` in the innermost roll instead of a per-listener
    /// scan.
    pub quiet_among: BTreeSet<ActorId>,
    /// The subject's own `occupation_id` at mint: `Craft`'s ×2.0 ear.
    pub craft_ear: Option<String>,
    source: FactSource,
}

impl Fact {
    pub(crate) fn source(&self) -> &FactSource;
    /// Whether an LLM made it. Safe to log; the payload is not.
    pub fn is_claimed(&self) -> bool;
    /// The mouth to walk back to, for the chain and for invalidation.
    pub fn claimant(&self) -> Option<&ActorId>;
}

/// A carrier's version, as **deltas from the fact, never as text**. At 20,000
/// people a rendered String per holding is a megabyte of garbled sentences the
/// renderer would rebuild anyway — and deltas are what makes the chain
/// reconstructible instead of merely logged. Deliberately carries no `topic`,
/// so "topic is invariant under garbling" is unrepresentable (D8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FactView {
    pub subject: Option<ActorId>,
    pub place: Option<AreaKey>,
    pub day_offset: i8,
}

impl FactView { pub fn is_pristine(&self) -> bool; }

/// What one person currently has of it. `heat` is **derived**, not stored, so
/// it cannot drift with the poll rate and a clock-less world does not age
/// (D9).
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub hops: u8,
    /// Who they had it from. `None` for a seeded holder — they had it from
    /// being there. This is the walk-the-chain link.
    pub from: Option<ActorId>,
    pub learned_on: Option<f64>,
    /// The fact as *this* person has it, after per-hop garbling.
    pub view: FactView,
    heat_at_learn: f32,
}

impl Held {
    /// `heat_at_learn × λ^((game_days − learned_on) × 24)`, and
    /// `heat_at_learn` untouched when either side is `None` — the same "no
    /// clock means nothing ages" tolerance `notices::expire` takes.
    pub fn heat(&self, game_days: Option<f64>) -> f32;
    pub fn is_first_hand(&self) -> bool;      // hops == 0
    /// A witness's clock is the fact's own mint stamp (`learned_on =
    /// fact.minted_game_days` when `fact.decays`, `None` for a standing fact),
    /// so a witness of news cools and a holder of authored truth does not.
    /// M1 declares it `seeded()`; M2 step 9 gives it the `&Fact` — M4 and M5
    /// call the M2 form.
    fn seeded(fact: &Fact) -> Self;                                          // private
    fn carried(hops: u8, from: Option<ActorId>, heat: f32,
               on: Option<f64>, view: FactView) -> Self;                    // private
}

/// What arrives, before the merge decides what to do with it.
///
/// Named `Telling` and not `Arrival` because `round::Arrival` already exists and
/// is already re-exported at the crate root (`round.rs:252`, `lib.rs`'s
/// `pub use round::{Arrival, …}`) — the second `Arrival` could not be
/// re-exported beside it.
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
pub enum Learned { Fresh, Corrected, Warmed, Unchanged, Refused }

/// The stored row. Interned and flattened: **88 bytes** (4 + 1 + 4 + 16 + 24 +
/// 32 = 81, and the 8-aligned `learned_on`/`from`/`view` round it to 88), so
/// six of them are ~530 B rather than ~1.6 KB. `a_holding_stores_no_sentence`
/// (M3 T23) asserts `size_of::<Holding>() <= 88`.
#[derive(Debug, Clone, PartialEq)]
struct Holding {
    key: FactKey,
    hops: u8,
    heat_at_learn: f32,
    learned_on: Option<f64>,
    from: Option<ActorId>,
    view: FactView,
}

/// **M4 (A1b).** Standing permission to `raise_word`, put there by the sim and
/// spent by a successful raise. One slot per actor, overwritten rather than
/// queued, and an hour old at most unless it is on a sheet in flight — so "how
/// does the model know when?" is answered by the verb not being on the sheet
/// otherwise.
#[derive(Debug, Clone, PartialEq)]
pub struct Occasion {
    /// Whoever the assertion was about, when it named somebody the sim could
    /// resolve. `None` for a witnessed event that minted nothing.
    pub subject: Option<ActorId>,
    /// The mouth that made the assertion — the chain link a claim raised off
    /// this occasion records as its `from`. `None` for limb 2.
    pub from: Option<ActorId>,
    pub at_game_days: f64,
    /// Set when a rendered sheet carried the verb; cleared when that exchange's
    /// reply lands or fails. An offered occasion is live whatever its age (D34).
    pub offered: bool,
}

/// **M4 (A3).** What a carried fact looks like from the outside — the player's
/// receipt, and the journal's whole content.
#[derive(Debug, Clone, PartialEq)]
pub struct LearnedHow {
    /// The sentence as the player heard it, rendered once at arrival by
    /// `render_plain` and never re-rendered — the holding behind it can be
    /// evicted at `HOLDINGS_MAX`, and a journal entry that goes blank is worse
    /// than no journal.
    pub word: String,
    pub at: Option<f64>,
    pub place: Option<AreaKey>,
    /// Who said it. `None` when the player witnessed it themselves.
    pub from: Option<ActorId>,
    pub hops: u8,
    /// How many separate mouths have told the player this, and in how many
    /// wards. The cheapest possible way to make a wave visible: two integers.
    pub tellings: u16,
    pub wards: u8,
    /// A `PlanningWard` bitset, so `wards` counts each ward once.
    wards_seen: u8,
}

/// Whole-percent heat, the way `marks::published_strength_pct` quantises
/// strength: one cooling step moves heat twelve orders of magnitude above
/// epsilon, so a raw-`f32` change test would churn forever.
pub fn heat_pct(heat: f32) -> u8;
```

---

## knowledge/mod.rs — the store

```rust
/// What this city knows, who holds it, and what its wards are saying.
///
/// Lives on `World` beside `notices` and `marks`, and like them it is **prompt
/// state, never carriage state**: it never calls `touch_public_state` and never
/// enters `PublicSnapshot`, whose 160 KiB bound has little headroom left. The
/// two large maps are behind `Arc` because `World::market_sale` clones the
/// whole world on every catalog sale (`inventory.rs:1298`).
#[derive(Debug, Clone, PartialEq)]                   // hand-written Default from M2 (NEG_INFINITY)
pub struct Knowledge {
    // ---- M1 ----
    live: BTreeMap<FactKey, Fact>,
    by_id: BTreeMap<FactId, FactKey>,
    next_key: u32,
    next_sequence: i64,
    holdings: Arc<BTreeMap<ActorId, Vec<Holding>>>,
    // ---- M2 ----
    air: Arc<BTreeMap<(PlanningWard, FactKey), Drift>>,
    /// `f64::NEG_INFINITY` at `Default`, so the first sweep always runs.
    last_sweep_game_days: f64,
    // ---- M4 ----
    occasions: BTreeMap<ActorId, Occasion>,
    /// `(game day, office, count)`, so the per-office cap needs no sweep.
    raises: BTreeMap<ActorId, (i64, Office, u8)>,
    /// The player's side. Serialised nowhere; projected on its own hot channel.
    pub player_learned: BTreeMap<FactId, LearnedHow>,
    receipts_revision: u64,
    /// What relevance seated on this actor's last rendered sheet (one slot: at
    /// most one turn-lane prompt is in flight).
    seated: Option<(ActorId, Vec<FactKey>)>,
    // ---- M5 ----
    last_hearsay_beat_game_days: f64,
    hearsay_raised: BTreeSet<(FactKey, ActorId)>,
}

impl Knowledge {
    // ---- reads (M1 unless marked) ----
    pub fn fact(&self, key: FactKey) -> Option<&Fact>;
    pub fn fact_by_id(&self, id: &FactId) -> Option<&Fact>;
    pub fn key_of(&self, id: &FactId) -> Option<FactKey>;
    /// Ascending `FactKey`, i.e. mint order — stable for every prompt and
    /// golden.
    pub fn facts(&self) -> impl Iterator<Item = (FactKey, &Fact)>;
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn holdings_len(&self, actor: &ActorId) -> usize;
    /// The stored row for this actor, if any — the private-map read `holds_key`
    /// performs, named so the store-before-seeded order (M4 A4) reads one lookup.
    fn stored(&self, actor: &ActorId, key: FactKey) -> Option<Held>;
    /// What the store actually costs, for the `--extra-ambient 20000` bound.
    pub fn footprint_bytes(&self) -> usize;
    pub fn drift(&self, ward: PlanningWard, key: FactKey) -> Option<&Drift>;              // M2
    /// One ward's air, as a `BTreeMap::range` and not a filter — which is the
    /// whole reason the key is `Copy`.
    pub fn ward_air(&self, ward: PlanningWard) -> impl Iterator<Item = (FactKey, &Drift)>; // M2
    pub fn air_entries(&self) -> usize;                                                    // M2
    pub fn occasion(&self, actor: &ActorId) -> Option<&Occasion>;                          // M4
    pub fn raises_left(&self, actor: &ActorId, day: i64, office: Office) -> u8;            // M4
    pub fn receipts_revision(&self) -> u64;                                                // M4

    // ---- writes: every one that touches `holdings` or `air` `Arc::make_mut`s ----
    /// M1: `None` on a duplicate id and at `FACTS_MAX_LIVE`. M2: at the cap,
    /// evicts the coldest **decaying** fact (lowest peak `heat_pct`, then lowest
    /// `sequence`); `None` when every live fact is a standing one.
    pub fn install(&mut self, fact: Fact) -> Option<FactKey>;
    /// Drops it from `live`, from every holding and (M2) from every ward's air.
    pub fn invalidate(&mut self, key: FactKey) -> Option<Fact>;
    pub(crate) fn next_handles(&mut self) -> (FactKey, i64);                              // M1
    // M2 (every row a deposit or a re-heat creates takes `stir = pollen::stage_stir(game_days)`, D28)
    pub fn deposit(&mut self, ward: PlanningWard, key: FactKey, hops: u8, heat: f32, via: &ActorId,
                   game_days: f64) -> bool;
    /// The one re-heat that bumps `stir` (D28): `reheat` (a standing fact, or
    /// one already in that ward's air) and `amplify`. Creates the row when absent.
    pub fn stir_up(&mut self, ward: PlanningWard, key: FactKey, to: f32, game_days: f64) -> bool;
    pub(crate) fn seed_air(&mut self, ward: PlanningWard, key: FactKey, heat: f32, game_days: f64);
    pub(crate) fn air_mut(&mut self) -> &mut BTreeMap<(PlanningWard, FactKey), Drift>;
    pub(crate) fn trim_air_to_cap(&mut self) -> bool;
    /// `marks::sweep`'s self-gate: `true` once per half game hour, and the
    /// clock stays current on the empty early-out so the first fact minted
    /// into a long-running world is not charged for the whole run.
    pub fn take_stir_beat(&mut self, game_days: f64) -> bool;
    pub(crate) fn last_sweep_game_days(&self) -> f64;
    #[doc(hidden)] pub fn rewind_sweep_clock(&mut self, game_days: f64);
    // M4 (A1b, A6)
    pub fn note_occasion(&mut self, actor: &ActorId, subject: Option<ActorId>,
                         from: Option<ActorId>, game_days: f64);
    /// `true` if there was one to spend. Called only on a **successful** raise.
    pub fn spend_occasion(&mut self, actor: &ActorId) -> bool;
    /// Drops un-offered occasions older than `OCCASION_LIFE_GAME_HOURS`.
    pub fn expire_occasions(&mut self, game_days: f64);
    pub fn offer_occasion(&mut self, actor: &ActorId);
    pub fn withdraw_offer(&mut self, actor: &ActorId);
    pub fn note_raise(&mut self, actor: &ActorId, day: i64, office: Office);
    fn bump_receipts_revision(&mut self);
    pub fn note_seated(&mut self, actor: &ActorId, keys: Vec<FactKey>);
    pub fn take_seated(&mut self, actor: &ActorId) -> Vec<FactKey>;
    /// Writes `heat_at_learn`/`learned_on` on the stored row, **inserting a
    /// hops-0 stored row first when the actor is seeded and has none** — so a
    /// witness of a 36-hour-old arrest can be re-heated too.
    pub fn set_heat_at(&mut self, actor: &ActorId, key: FactKey, heat: f32, on: Option<f64>);
    pub fn seat_claimant(&mut self, speaker: &ActorId, key: FactKey, from: Option<ActorId>,
                         game_days: Option<f64>);
    // M5 (8f)
    pub fn take_hearsay_beat(&mut self, game_days: f64) -> bool;
    #[doc(hidden)] pub fn rewind_hearsay_beat(&mut self, game_days: f64);
}
```

## knowledge/mod.rs — the free functions

```rust
/// **`None` means they have never heard of it at all.** Checks the fact's
/// `seeded` set first (hops 0, heat 1.0, no garble, `from: None`), then the
/// carrier store. Every consumer — the sheet, the journal, a quest, a systemic
/// reading — goes through this and nothing else.
///
/// Always call it fully qualified, `knowledge::holds`, and never `use` it: it
/// is one letter from `Character::holds` (the hands, `character.rs:653`) and
/// `Custody::holds` (the law, `custody.rs:305`).
pub fn holds(world: &World, actor: &ActorId, fact: &FactId) -> Option<Held>;
pub fn holds_key(world: &World, actor: &ActorId, key: FactKey) -> Option<Held>;
/// Everything this person has, ascending `FactKey`.
pub fn holdings_of(world: &World, actor: &ActorId) -> Vec<(FactKey, Held)>;

/// The merge rule, all four rows, in one place: fewer hops **replaces** the
/// view, the chain and the count; equal hops keeps the held view (a person
/// does not flip-flop between two equally distant versions); more hops only
/// warms; and a hops-0 holder is immovable, because a witness cannot be talked
/// out of what they saw. Heat always takes the maximum.
pub fn learn(world: &mut World, actor: &ActorId, key: FactKey, telling: Telling,
             game_days: Option<f64>) -> Learned;

/// **M1.** Whether this person can ever carry this fact as news: the subject
/// cannot (D51). A rule, outside the salience product, so neither measurement
/// lever removes it. Called first in `pickup_chance`, `volunteers` and
/// `hop_on_stage`.
pub(crate) fn may_carry(fact: &Fact, who: &ActorId) -> bool;

/// **M1** (body completed in **M2**). The deposit gate, and the sheet's own seat
/// gate: `heat × salience > VOLUNTEER_HEAT`, and never for the subject
/// (`may_carry`) or for a standing fact (`!fact.decays`, D52). M1's body uses
/// `world.salience.base(fact.topic)`; M2 replaces that with
/// `salience::salience_for(world, fact, holder)` at this one site. One
/// function, one home (`knowledge/mod.rs`), three callers: Layer 1's deposit,
/// Layer 2's carrier test, `render_line`'s cold row.
pub(crate) fn volunteers(world: &World, fact: &Fact, holder: &ActorId, held: &Held,
                         game_days: Option<f64>) -> bool;

/// **M1.** The relevance limb of sheet selection on its own (D57): the facts
/// whose id segments, subject name or place label appear in `since` or
/// `recent`, ascending `FactKey`. `what_you_know_lines` calls it for the sheet
/// and (M4) `render_prompt_and_drain` for the re-heat, so what gets warmed is
/// exactly what got seated. M5 widens it with `present` (the greeting register).
pub(crate) fn relevance_seated(world: &World, actor: &Character, since: &[&str],
                               recent: &[&str]) -> Vec<FactKey>;

/// **M1.** Drop every fact the world no longer bears out (`still_true`), and
/// every holding of it. Returns what died, for a diagnostic. M5 calls it from
/// `pollen::sweep` on every stir beat; before that only tests do.
pub fn invalidate_stale(world: &mut World) -> Vec<FactId>;

/// **M4.** Lift a cold fact back to just above the volunteer gate and no
/// further, so a revived story circulates without ever being as loud as fresh
/// news. Called when relevance seats a fact on a speaking turn — warming is a
/// consequence, not a decision, which is why it is a rule and not a verb.
pub fn reheat(world: &mut World, actor: &ActorId, key: FactKey, game_days: Option<f64>);

/// **M1.** Turn one holding into the sentence *this* reader would say. The only
/// place a fact becomes words: it substitutes `{subject}`/`{place}`/`{day}`,
/// applies the unknown-people rule to the subject, and wraps the result in the
/// band × rung hedge (D18). `None` when the fact is about the reader and they
/// have no `own` line — nobody is told about themselves in the third person.
pub fn render_line(world: &World, reader: &ActorId, key: FactKey, held: &Held,
                   strings: &PromptStrings, game_days: Option<f64>) -> Option<String>;

/// **M3.** Walk `Held::from` back, newest mouth first, capped at
/// `CHAIN_MAX_LINKS`. A reconstruction, not a log — which is why garbling had
/// to be a pure function of `(sequence, carrier, hops)`.
pub fn chain(world: &World, actor: &ActorId, key: FactKey) -> Vec<ActorId>;

// M4 (A5, A16) and M5 (6a, 7a, 8f) add, beside these:
//   pub fn render_plain(world, reader, key, held, game_days) -> Option<String>
//   pub fn when_phrase(at: Option<f64>, now: Option<f64>) -> String
//   pub fn standing_lines(world, player) -> Vec<String>
//   pub fn holds_about(world, holder, about, topic: Option<Topic>, game_days) -> Option<(FactKey, Held)>
//   pub fn door_is_shut(world, householder, caller) -> bool
//   pub fn raise_hearsay_words(world, game_days) -> Vec<String>
```

---

## knowledge/catalog.rs

```rust
const FACTS_JSON: &str = include_str!("../../../../assets/world/facts.json");

/// One authored row, before a world exists to resolve it against.
#[derive(Debug, Clone, PartialEq)]
pub struct FactSpec {
    pub id: FactId,
    pub topic: Topic,
    pub said: String,
    pub own: BTreeMap<ActorId, String>,
    pub subject: Vec<ActorId>,
    pub seeded: BTreeSet<ActorId>,
    pub place: Option<AreaId>,
    pub day: Option<i64>,
    pub decays: bool,
    pub garble: GarbleMask,
    /// Named here so a quest can bind invalidation without a Rust type;
    /// resolved to a `FactSource` by `seed`.
    pub source: FactSourceSpec,
}

/// The authored spelling of provenance — the *only* thing outside `source.rs`
/// that names a variant, and it names them by string, not by payload.
#[derive(Debug, Clone, PartialEq)]
pub enum FactSourceSpec {
    Authored,
    Custody(ActorId),
    Item(ItemId),
    QuestPhase { quest: String, phase: u8 },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FactCatalog { specs: Vec<FactSpec> }

#[derive(Debug, Clone, PartialEq)]
pub struct FactCatalogError { pub message: String }
impl std::fmt::Display for FactCatalogError { /* write_str(&self.message) */ }

impl Default for FactCatalog {
    /// The embedded catalog. Panics only if the *compiled-in* asset is
    /// malformed, which is a build-time fact — the same bargain `marks.rs` and
    /// `item.rs` make.
    fn default() -> Self;   // from_embedded().expect("the embedded fact catalog must parse and validate")
}

impl FactCatalog {
    pub fn from_embedded() -> Result<Self, FactCatalogError>;
    pub fn from_json(json: &str) -> Result<Self, FactCatalogError>;
    /// A quest pack, host-supplied. Returns how many rows were added; a
    /// duplicate id is an error, not a silent overwrite.
    pub fn extend_from_json(&mut self, json: &str) -> Result<usize, FactCatalogError>;
    pub fn specs(&self) -> &[FactSpec];
    /// Install every row into a live world: resolve `place` against the
    /// `AreaMap`, compute `quiet_among` and `craft_ear`, drop rows naming an
    /// actor the world does not have. Returns diagnostics — never a panic,
    /// because a hermetic test world legitimately lacks the cast.
    pub fn seed(&self, world: &mut World) -> Vec<String>;
}
```

---

## knowledge/salience.rs

```rust
const SALIENCE_JSON: &str = include_str!("../../../../assets/world/salience.json");

/// Which rung of the hedge ladder a topic's tellings sit on. Authored per
/// topic rather than derived from the base number, so `flat()` stays purely
/// arithmetic and cannot silently promote all nine topics to the top band
/// mid-measurement (D19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HedgeBand { Top, Default, Low }

/// The designer's whole tuning surface: nine base bands and the trades that
/// hear each one differently. A float per fact would be five hundred floats
/// nobody can reason about; a topic is a classification with an external check.
#[derive(Debug, Clone, PartialEq)]
pub struct SalienceTable { /* private */ }

#[derive(Debug, Clone, PartialEq)]
pub struct SalienceError { pub message: String }

impl Default for SalienceTable { fn default() -> Self; /* from_embedded().expect(…) */ }

impl SalienceTable {
    pub fn from_embedded() -> Result<Self, SalienceError>;
    pub fn from_json(json: &str) -> Result<Self, SalienceError>;
    /// Every base and every affinity 1.0, hedge bands untouched. The identity
    /// run: the roll becomes `curiosity × heat` again, which is the model
    /// before salience existed, so the pre-salience cadence numbers must
    /// reproduce exactly (`02_numbers.md`, the flat-table identity). Sets
    /// `household = 1.0`, which the loader rejects — this constructor bypasses
    /// the loader on purpose and says so.
    pub fn flat() -> Self;
    pub fn base(&self, topic: Topic) -> f64;
    pub fn hedge_band(&self, topic: Topic) -> HedgeBand;
    pub fn ear_of(&self, topic: Topic) -> (&[String], f64);
    pub fn craft_own(&self) -> f64;
    pub fn craft_other(&self) -> f64;
    pub fn no_trade(&self) -> f64;
    pub fn household(&self) -> f64;
}

/// **M2.** `base(topic) × affinity(listener) × household damping` — the pure
/// table product and nothing else (D51: the self-subject rule is `may_carry`,
/// outside this function, so `flat()` is exactly a multiplication by one).
/// Never decays — heat answers *is this current*, this answers *is this worth
/// repeating at all*, and what falls out of multiplying them is that a cold
/// scandal out-travels a fresh squabble. The player's affinity is 1.0 on every
/// topic (D26): `PLAYER_CURIOSITY` is the whole of their roll.
pub fn salience(world: &World, fact: &Fact, listener: &ActorId) -> f64;

/// **M2.** `salience`, honouring `--pollen-no-salience` — the **one** place that
/// lever is read: `if world.pollen_no_salience { 1.0 } else { salience(…) }`.
pub(crate) fn salience_for(world: &World, fact: &Fact, who: &ActorId) -> f64;
```

---

## knowledge/pollen.rs

```rust
/// The word in one ward's air: what is being said there, how loudly, at how
/// few removes, and by whose mouth it last got in.
#[derive(Debug, Clone, PartialEq)]
pub struct Drift {
    pub heat: f32,
    /// The fewest hops any depositing carrier holds it at. A pickup lands at
    /// `hops + 1`.
    pub hops: u8,
    /// The last mouth to deposit at that hop count — the chain link a pickup
    /// records as its `from`, which is what keeps walk-the-chain walkable.
    /// Makes `Drift` non-`Copy`, so every sweep is two-phase (D27).
    pub via: Option<ActorId>,
    /// One pickup roll per person per fact per stir — never a fresh draw per
    /// poll. Bumped by the cooling sweep on a fixed half-game-hour grid and by
    /// `Knowledge::stir_up`; **never by a deposit** (D28). Seeded from
    /// `stage_stir(game_days)` at creation, never from 0.
    pub stir: u32,
}

/// Position → standing ward, exactly, in one array index most of the time.
///
/// An **accelerator, not an approximation**: a cell holds a ward ordinal only
/// where its centre and all four corners agree under the exact nearest-mark
/// search, and `0xFF` otherwise. An ambiguous cell — and any point outside the
/// city box — falls through to that same exact search, so the grid's answer is
/// identical to it everywhere and there is no ground that answers `None`
/// wrongly. `crowd.rs` is routed through this too, so housing and pollen
/// cannot disagree (D23).
#[derive(Debug, Clone, PartialEq)]
pub struct WardGrid { /* private: Vec<u8>, min, cell_m, cols, rows, marks */ }

/// The grid, baked once per process. 8 m cells, 91 × 104 = 9,464 bytes.
pub fn ward_grid() -> &'static WardGrid;

impl WardGrid {
    pub fn bake() -> Self;
    /// **The** definition of where a person is standing.
    pub fn at(&self, point: Vec3) -> Option<PlanningWard>;
    /// The exact nearest-mark search over `crowd::ward_map()`, and the only
    /// caller of it: the baker, and `at`'s fallback.
    pub fn exact_at(&self, point: Vec3) -> Option<PlanningWard>;
    pub fn cells(&self) -> usize;
    pub fn ambiguous_cells(&self) -> usize;
}

/// Which areas a garbled place may become: `AreaMap::nearest_areas` minus the
/// area itself, precomputed once. Empty in every world with an empty
/// `AreaMap` — the goldens and the hermetic tests — so a place garble is a
/// no-op there, which is stated rather than discovered.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AreaAdjacency { /* Vec<Vec<AreaKey>> */ }

impl AreaAdjacency {
    pub fn build(map: &AreaMap) -> Self;
    pub fn neighbours(&self, area: AreaKey) -> &[AreaKey];
}

/// One person's turn at the air: deposit what they are warm enough to say,
/// then roll for everything in this ward they do not already hold closer.
///
/// One ward lookup and at most `AIR_PER_WARD_MAX` rolls. The roll is a pure
/// function of `stir`, so it is idempotent within a stir window and needs no
/// per-(actor, fact) memo — recomputing gives the same answer, and a
/// successful pickup short-circuits through `holds`. Writes the store and
/// nothing else: **a pickup is not a percept** (D25).
pub fn poll_person(world: &mut World, actor: &ActorId, game_days: f64);

/// The player's own seat at the air. They are not in `round.people` and
/// `notify_percept` no-ops for them, so this is called from `Engine::poll` and
/// rolls against `PLAYER_CURIOSITY` — never `curiosity_of`, which returns 1.0
/// for a body with no lore sheet and would hand them the whole ward at once.
pub fn poll_player(world: &mut World, player_id: &ActorId, game_days: f64);

/// Cool the air by `λ^(1/2)`, bump `stir`, evict below `HEAT_GONE_BELOW`, and
/// hold each ward to `AIR_PER_WARD_MAX` (coldest out). Self-gating on the
/// half-game-hour grid, `marks::sweep`'s shape including its keep-the-clock
/// early-out. Two-phase borrow: collect keys, then `get_mut`.
pub fn sweep(world: &mut World, game_days: f64) -> bool;

/// Layer 2: mouth to mouth where the player can see it, so the wave reads as a
/// wave instead of turning up already known one ward over.
///
/// It runs its **own** scan rather than riding `attention::on_stage`'s, for
/// two reasons that must not be optimised away: `night::stage_occupied`
/// (`night.rs:864`) calls `on_stage` purely as an emptiness question, so a
/// side-effecting version would double-fire; and `on_stage` is empty under the
/// default `IdleCognitionMode::All`. The scan is O(N) (`world.rs:478` has no
/// spatial index), hence the real-seconds self-gate and the pair cap.
pub fn hop_on_stage(world: &mut World, player_id: &ActorId, now: f64, game_days: f64);

/// M5: a civic peal re-heats matching air within earshot rather than minting —
/// so acoustics physically extend rumour range and the false-bell prank gains
/// an epistemic consequence.
pub fn amplify(world: &mut World, at: Vec3, radius_m: f64, topic: Option<Topic>, game_days: f64);

/// The one roll, and it is a hash of stable inputs and never a fresh draw: the
/// engine polls at 60 Hz and a re-drawn 1-in-20 is a certainty within a frame
/// (`attention.rs` learned this the hard way). The `notices::carries` idiom,
/// `notices.rs:419`.
pub fn picks_up(fact: &Fact, actor: &ActorId, stir: u32, chance: f64) -> bool;

/// **M3.** `picks_up` with the mouth in the hash, for Layer 2: eight carriers
/// beside one listener must be eight independent chances (M3 step 5).
pub fn picks_up_from(fact: &Fact, listener: &ActorId, carrier: &ActorId, stir: u32,
                     chance: f64) -> bool;

/// **M2.** The one place the roll's probability is computed:
/// `curiosity × heat × salience_for`, `PLAYER_CURIOSITY` for the player, and
/// 0.0 when `!may_carry`.
pub(crate) fn pickup_chance(world: &World, fact: &Fact, listener: &ActorId, heat: f32) -> f64;

/// **M2.** The stir grid's ordinal at an instant — the half-game-hour grid the
/// sweep bumps `stir` on. New air rows start here, never at 0 (D28); Layer 2
/// (M3) rolls on it.
pub(crate) fn stage_stir(game_days: f64) -> u32;

/// **M4 (A17).** One `Drift` row written for `seed-fact … -> <ward>`, respecting
/// `AIR_PER_WARD_MAX`. `#[doc(hidden)]`, like `rewind_sweep_clock`.
#[doc(hidden)]
pub fn debug_seed_air(world: &mut World, ward: PlanningWard, key: FactKey, game_days: Option<f64>);
```

### The measurement

```rust
/// What the wave is actually doing, per topic — the cadence band's own
/// instrument. Printed by `--trace-pollen`; the assertions in
/// `tests/pollen_cadence.rs` read these fields and nothing else.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PollenCensus {
    pub facts: usize,
    pub holdings: usize,
    pub air_entries: usize,
    pub store_bytes: usize,
    pub rolls_per_game_hour: f64,
    /// Measured X, city mean: ward changes per person per game day. Filled by
    /// the caller from a `CrossingTally`; `0.0` from a single instant.
    pub crossings_per_person_per_game_day: f64,
    /// Measured X **for each ward**: exits of that ward ÷ person-game-hours
    /// spent in it, × 24. The expectation for a fact uses its **mint ward's**
    /// row, never the city mean (D54).
    pub ward_exit_rate_per_game_day: BTreeMap<PlanningWard, f64>,
    /// Per-ward, never a city mean: the standing wards are 7:1 lopsided
    /// (BellAndSluice 192, Cinder 26), so a mean hides both ends.
    pub ward_population: BTreeMap<PlanningWard, usize>,
    pub ward_mean_curiosity: BTreeMap<PlanningWard, f64>,
    pub topics: Vec<TopicRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopicRow {
    pub topic: Topic,
    pub fact: FactId,
    pub mint_ward: Option<PlanningWard>,
    /// **The number of wards whose air holds a row for this fact** — a count
    /// of deposits, never of holders' wards (D54). This is what every realised
    /// band backstop reads: a cold holder walking home is not "the word being
    /// said" there.
    pub wards_reached: u8,
    pub carriers: usize,
    /// How many of those carriers are warm enough to repeat it — the deposit
    /// gate's own population, which is what the slow end turns on.
    pub volunteering: usize,
    /// Expected crossings out of the mint ward so far:
    /// `warm_mint_ward_game_hours × ward_exit_rate(mint_ward) / 24`. Filled by
    /// the caller from a `CrossingTally`.
    pub expected_crossings: f64,
    /// Realised exits of the mint ward by holders, warm or cold. A printed
    /// diagnostic only — never asserted (D54).
    pub holder_exits: f64,
    pub holder_exits_same_trade: f64,
    pub mean_hops: f64,
    pub max_heat: f32,
    pub age_game_hours: f64,
    pub carriers_by_ward: BTreeMap<PlanningWard, usize>,
}

/// The one thing no instant can answer. Held by whoever is sampling, never by
/// `World`. See M2 step 17 for the sampling rule.
pub struct CrossingTally { /* private */ }

impl CrossingTally {
    pub fn new(watching: &[FactKey]) -> Self;
    pub fn sample(&mut self, world: &World, game_days: f64);
    /// A copy of every accumulator at this instant, so a test reads the office
    /// boundary and the game day off one run.
    pub fn snapshot(&self) -> TallySnapshot;
}

#[derive(Debug, Clone, PartialEq)]
pub struct TallySnapshot {
    pub game_days: f64,
    pub span_game_days: f64,
    /// City mean: total ward changes ÷ bodies ÷ elapsed game days.
    pub per_person_per_game_day: f64,
    /// Per ward: exits ÷ person-game-hours × 24.
    pub ward_exit_rate_per_game_day: BTreeMap<PlanningWard, f64>,
    /// Per watched fact: Σ over samples of (holders warm on it **and standing
    /// in its mint ward**) × the sample gap in game hours.
    pub warm_mint_ward_game_hours: BTreeMap<FactKey, f64>,
    pub holder_exits: BTreeMap<FactKey, f64>,
    pub holder_exits_same_trade: BTreeMap<FactKey, f64>,
}

impl PollenCensus {
    pub fn summary(&self) -> String;
    /// One `[pollen]` line per topic, stable order (`Topic::ALL`).
    pub fn topic_lines(&self) -> Vec<String>;
    /// Fill the tally-derived fields from a snapshot.
    pub fn fill(&mut self, tally: &TallySnapshot);
}

pub fn census(world: &World, clock: &WorldClock, now: f64) -> PollenCensus;
```

---

## knowledge/garble.rs

```rust
/// A pure function of `(fact.sequence, carrier, hops)` — which is why a chain
/// is reconstructible from `Held::from` and never has to be logged. Bounded to
/// a fixed vocabulary: the subject becomes another **named** actor of the same
/// ward or trade, the place an adjacent area, the day ±1 clamped to
/// `DAY_OFFSET_MAX`. It never invents a person; `no-procedural-characters`
/// holds here as everywhere.
pub fn view_for(world: &World, fact: &Fact, carrier: &ActorId, hops: u8) -> FactView;

/// The substitution pool for a garbled subject: named actors sharing the
/// subject's **lore** ward (`LoreProfile.planning_ward` — never the standing
/// ward, which moves every leg and would make the same link recompute to a
/// different name) or `occupation_id`, in roster order, capped at
/// `GARBLE_SUBJECT_POOL_MAX`. Empty is fine — the subject then simply does not
/// move.
pub fn same_ward_or_trade(world: &World, subject: &ActorId) -> Vec<ActorId>;
```

---

## knowledge/mint.rs

```rust
/// One row of the whitelist. Stated **once**: the occasion gate's second limb
/// reads this same list, so "a percept that minted nothing" cannot drift out
/// of agreement with the mints (D33). `said` is the row's template — it lives
/// here and not in `strings.toml` because `actions::raise_notice` has no
/// `PromptStrings` in hand (M2 step 16).
pub struct MintKind { pub kind: &'static str, pub topic: Topic, pub garble: GarbleMask,
                      pub said: &'static str }

/// M2: the custody commitment and `raise_notice` (2 rows). M5: the knell and
/// the two stranger deeds (5 rows). A large accepted sale is declined (D32). A
/// coded mint needs no classifier — a seizure is `Law` because seizures are.
pub const MINT_KINDS: &[MintKind];

/// The one installer (M2, `pub(crate)`): allocate the handles, freeze the
/// salience inputs, resolve the place, stamp the clock, and put the telling in
/// the minting ward's air. `own` is empty for every coded mint and carries the
/// speaker's line for a claim (M4 `mint_claim` routes through here too, D58).
#[allow(clippy::too_many_arguments)]
pub(crate) fn install_fact(world: &mut World, id: FactId, topic: Topic, said: String,
                           own: BTreeMap<ActorId, String>, subject: Vec<ActorId>,
                           seeded: BTreeSet<ActorId>, at: Vec3, garble: GarbleMask, decays: bool,
                           source: FactSource, game_days: Option<f64>) -> Option<FactKey>;

/// `install_fact` with `seeded` computed from earshot
/// (`characters_within(at, HEARING_RADIUS_M, None)`) — the ordinary coded mint.
#[allow(clippy::too_many_arguments)]
pub fn mint(world: &mut World, id: FactId, topic: Topic, said: String, subject: Vec<ActorId>,
            at: Vec3, garble: GarbleMask, decays: bool, source: FactSource,
            game_days: Option<f64>) -> Option<FactKey>;

/// Called from `Engine::announce_commitment` (`engine.rs:3748`), immediately
/// above the `confiscate_the_taking` call — **not** from `World::emit`: the
/// `"commit"` event is `if gaol`-gated with empty `recipient_ids`
/// (`engine.rs:3825`), so it misses every gate-arch commitment, and `emit`
/// itself has no clock. Earshot is computed here.
pub fn mint_commitment(world: &mut World, prisoner: &ActorId, officer: Option<&ActorId>,
                       station: &str, at: Vec3, game_days: Option<f64>) -> Option<FactKey>;

/// Called from `actions::raise_notice` after its `world_event`
/// (`actions.rs:3145`). The notice already carries authored prose, a place and
/// a clock stamp, so this is the cheapest second kind in the tree.
pub fn mint_from_notice(world: &mut World, notice_id: u64, raiser: &ActorId,
                        game_days: Option<f64>) -> Option<FactKey>;

/// M5, behind `EngineCommand::Knell` — the knell has no sim seam today. A stub
/// returning `None` in M2.
pub fn mint_knell(world: &mut World, at: Vec3, years: u32, game_days: f64) -> Option<FactKey>;

/// **M4.** The **only** constructor for a claimed fact (D35, D58): substitutes
/// the resolved subject's name with `{subject}`, narrows the mask to the
/// placeholders present, seeds the speaker alone, forces `decays`, keeps the
/// speaker's `own` line, routes through `install_fact`, then `seat_claimant`.
pub fn mint_claim(world: &mut World, speaker: &ActorId, topic: Topic, said: String,
                  subject: Vec<ActorId>, from: Option<ActorId>, game_days: Option<f64>)
                  -> Option<FactKey>;

/// **M4.** Whether this ward's air already carries the same
/// `(topic, subject, place, day)` — structural, never a text comparison.
pub fn collides_in_air(world: &World, ward: PlanningWard, topic: Topic, subject: Option<&ActorId>,
                       place: Option<AreaKey>, day: Option<i64>) -> bool;

/// **M4.** The occasion gate's second limb: a `WorldEvent` whose `kind` is not
/// in `MINT_KINDS` stamps an occasion on every LLM actor within
/// `HEARING_RADIUS_M`. They saw something the whitelist does not cover, so a
/// word about it is theirs to raise.
pub fn note_unminted_event(world: &mut World, event: &DomainEvent, game_days: f64);

/// **M4.** The occasion gate's first limb, called from `actions::say` for the
/// addressee of the line. A cheap pre-filter on the text, then a real `holds()`
/// lookup over every live fact whose subject the line names: they were told a
/// thing and they do not have it (D34).
pub fn note_assertion(world: &mut World, speaker: &ActorId, hearer: &ActorId, text: &str,
                      game_days: f64);

/// **M5.** The STRANGER token's mint: `draw_mark`/`scrub_mark` by the player.
pub fn mint_stranger_deed(world: &mut World, event: &DomainEvent, player_id: &ActorId,
                          game_days: f64) -> Option<FactKey>;
/// **M5.** What the city hears in a rope; stated once.
pub fn peal_topic(rope: crate::engine::CivicRope) -> Option<Topic>;
```

---

## The `World` fields (`world.rs`, the struct at `:77` and `impl Default` at `:203`)

```rust
// M1
pub knowledge: crate::knowledge::Knowledge,
pub fact_catalog: Arc<crate::knowledge::FactCatalog>,
pub salience: Arc<crate::knowledge::SalienceTable>,
/// `config.ron: smart_actors.knowledge.enabled`, `CATHEDRAL_NO_KNOWLEDGE`.
/// Gates readers **and** writers, like `marks_enabled` (`world.rs:174`), so an
/// ablation run is a real ablation.
pub knowledge_enabled: bool,
// M2
pub area_adjacency: Arc<crate::knowledge::AreaAdjacency>,
/// Written once by `Round::seed` and by nothing else, so `quiet_among` can be
/// frozen at mint (D38). Empty in every round-less world.
pub household_doors: Arc<BTreeMap<ActorId, Vec3>>,
/// Measurement lever, never in `config.ron`: the salience factor deleted from
/// the roll (`--pollen-no-salience`).
pub pollen_no_salience: bool,
```
```rust
impl World {
    /// M1 (D37). Seed a character's standing intention the way world creation
    /// does. A **seed**, not an override: their own `set_goal` must win
    /// afterwards, or they stop being a character. Deliberately no memories
    /// parameter — a seeded memory is erasable by `forget` on the first turn,
    /// so anything quest-critical is a one-person fact instead.
    pub fn arm_actor(&mut self, id: &ActorId, goal: Option<String>);
    /// M2 (D23). Where this point stands, and the only answer to that question
    /// anywhere in the crate — a one-line delegate to `pollen::ward_grid()`.
    pub fn ward_at(&self, point: Vec3) -> Option<PlanningWard>;
}
```
`Default` initialises `knowledge: Knowledge::default()`, `fact_catalog: Arc::new(FactCatalog::default())`,
`salience: Arc::new(SalienceTable::default())`, `knowledge_enabled: true` (M1);
`area_adjacency: Arc::new(AreaAdjacency::default())`, `household_doors: Arc::new(BTreeMap::new())`,
`pollen_no_salience: false` (M2).
**Not serialised. Not in `PublicSnapshot`.**

---

## The `Round` field and the pass (`round.rs`)

```rust
struct Round {
    // …
    /// A person's next turn at the ward's air, in **game-days** — never real
    /// seconds, or the `T` key's 60× would change the roll count and the
    /// measured cadence would not be the played one (D22).
    next_pollen: BTreeMap<ActorId, f64>,
    /// M5: a vendor who refused credit on a held fact, so the ladder stops
    /// re-queueing them at the same board. Its own map, not
    /// `chalk_refused_until`, so the trace line names one reason.
    knowledge_refused_until: BTreeMap<ActorId, f64>,
}

/// Deposit, pick up and cool for the whole cast, each on their own game-time
/// deadline.
///
/// A separate pass rather than a rung of `run_ladder`, because `run_ladder`
/// `continue`s past the market crowd, the well queue and everybody standing
/// talking — the most gossip-shaped places in the city — before it ever
/// reaches its throttle. Shaped like `decay_needs` (`round.rs:6259`), which is
/// the proven whole-cast borrow, and living in `round.rs` because
/// `Round.people` is private.
fn tick_pollen(round: &mut Round, world: &mut World, clock: &WorldClock, now: f64);
```
Called from `round::tick` immediately after `decay_needs(round, world, clock, now);`
(**`round.rs:5741`**). `knowledge_refused_until` is pruned beside `chalk_refused_until`
(`round.rs:5735`).

---

## The prompt seam (`prompt/mod.rs`)

```rust
// Sheet, declared immediately after `marks_here` (prompt/mod.rs:292) and BEFORE
// `word_in_the_ward` (:306) — declaration order is render order (prompt/mod.rs:222-224):
    /// What this actor has been told or saw of the city's doings, capped at
    /// `KNOWN_SHEET_MAX` and seated by relevance before heat. Omitted entirely
    /// when empty — the universal case and every frozen fixture's case.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    what_you_know: Vec<KnownLine>,

/// One `what_you_know` bullet. The sentence and nothing else: no id, no hop
/// count, no number — no verb names a fact, so a handle here could only be
/// misused.
#[derive(Serialize)]
struct KnownLine { word: String }

/// Relevance first, heat second, and the self-subject filter before both (D57).
///
/// "The hottest thing this actor carries" is a gossip rule and the wrong rule
/// for an interrogation: ask about the bale while the ward is loud about an
/// arrest and the one fact you came for is off the sheet. So a fact whose id
/// segments, subject name or place label appear in `since_your_last_turn` or
/// `recent_history` is seated first whatever its heat, including a faded one
/// (`knowledge::relevance_seated`); heat fills what is left, tie-broken by
/// fewest hops then `fact.id`, so the sheet is stable across runs and goldens.
/// `since` and `recent` are `build_sheet`'s own post-`fallback` vectors.
fn what_you_know_lines(world: &World, actor: &Character, since: &[&str], recent: &[&str],
                       strings: &PromptStrings) -> Vec<KnownLine>;          // M1
// M5 (step 9) inserts `present: &[ActorId]` before `strings` — the greeting register.
```
`render_prompt` (`prompt/mod.rs:538`) gains **nothing in M1** (no `has_known`: the block's
instruction paragraph renders inside the block, D13). M4 adds, beside `has_scrub_verb` (`:564`):
```rust
let has_raise_word = crate::actions::may_raise_word(world, actor_id);
```
and passes it into the `context!` block (`prompt/mod.rs:607-625`, after `has_scrub_verb,` at `:624`).

`sheet_markdown` (`prompt/mod.rs:1420`) renders the block between `marks_here` (closes `:1551`) and
`word_in_the_ward` (`:1553`) — the same order it is declared in. The measured sheets carried no
`word_in_the_ward`, so the block's position relative to it is a **choice**, stated here: a standing
truth about the world sits with the chalk and the ward's word, above the time axis. **Its own
renderer, not `bullet_section`**, because the note paragraph sits between the header and the
bullets:
```
**what_you_know** ({know_note}):
<blank>
{know_discipline}
<blank>
- {rendered line}
- {rendered line}
```

New `PromptStrings` fields (D43: **four** edit sites each), all in `strings.toml`'s flat top level,
**29 in M1**: the 24 of `../m0_evidence/strings_draft.toml` under its own names — `know_note`,
`know_discipline`, the 21 `know_hedge_{default,top,low}_{hops0_own,hops0,hops1,hops2,hops3,hops4,cold}`,
`unknown_person_role` — plus M1's five unmeasured fragments `day_today`, `day_yesterday`,
`day_days_past`, `day_long_ago`, `place_unknown`. M3 adds `known_from`. Exact text in `03_assets.md`.
Placeholder validation in `PromptEnv::new` beside `accept_with`'s (`prompt/mod.rs:182`) **counts
`%s`** (D44): one in every `know_hedge_*`, `day_days_past` and `known_from`; two in
`unknown_person_role`; none in the rest.

---

## The verb (`actions.rs`)

```rust
// dispatch, beside "raise_notice" (actions.rs:125):
"raise_word" => raise_word(world, actor_id, args),

/// Whether the sim has put an occasion in front of this actor — the whole of
/// "how does the model know when to use it?". O(1), because `render_prompt`
/// asks it for everyone at 20,000.
pub fn may_raise_word(world: &World, actor_id: &ActorId) -> bool;

/// Coin a proposition, here, now, in your own words — the only path by which a
/// model creates a fact, and everything it creates is a **claim**.
///
/// The guardrails are the specification: `source` is always
/// `FactSource::claimed(speaker)` and is not a parameter; `seeded` is the
/// speaker alone; `decays` is always true; the mask is the topic's default; the
/// subject resolves only against actors on the speaker's own sheet; an
/// unrecognised topic falls to `Talk`; one raise per actor per office; and a
/// `(topic, subject, place, day)` collision in this ward's air is a no-op.
fn raise_word(world: &mut World, actor_id: &ActorId, args: &Value) -> Result<String, ActionError>;
```
`error.rs`: `ActionErrorCode::{NoOccasion, WordAlreadySaid, WordAlreadyInTheAir}`, all three collapsing
onto `CommandErrorCode::InvalidAction` in the exhaustive match at `error.rs:378`, beside the chalk
codes at `:421-425`.

---

## The host seam

```rust
// engine.rs, EngineMessage — hot, dedupe-published like ChalkStanding (engine.rs:4156)
/// The player's own receipts (`J`), and whatever standing lines a live clock
/// supplies. Hot rather than snapshot, for `ChalkStanding`'s reason: it is a
/// fact about the player, not carriage state. Carries **no heat**, so a
/// cooling step cannot churn the channel.
Journal { entries: Vec<JournalEntry>, standing: Vec<String> },

#[derive(Debug, Clone, PartialEq)]
pub struct JournalEntry {
    /// The sentence the player actually heard, as they heard it.
    pub word: String,
    /// Who said it, with the unknown-people rule already applied. `None` when
    /// they witnessed it.
    pub from: Option<String>,
    pub place: Option<String>,
    pub when: String,
    pub hops: u8,
    pub tellings: u16,
    pub wards: u8,
}

// engine.rs, EngineCommand
DebugSeedFact { fact: String, ward: Option<String> },
DebugRaiseWord { who: String, topic: String, said: String },
Knell { years: u32, at: Vec3 },                                  // M5
```
Host: `src/smart_actors/journal_ui.rs` (`JournalUiState { open: bool }`, `KeyCode::KeyJ` — verified
unused repo-wide), on `inventory_ui.rs`'s pattern (`InventoryUiState`, `inventory_ui.rs:104`, `KeyI`
at `:436`). Message handled in `src/smart_actors/mod.rs`'s `process_engine_message`
(`mod.rs:1186`), taking `&mut ResMut<_>` — never `&mut T` (D48).
`local_engine::translate` (`local_engine.rs:733`) maps the new `BridgeCommand` variants
(`bridge.rs:69`); `drive.rs` gains `seed-fact` / `raise-word` / `knell` actions (`enum Action` at
`drive.rs:183`, `describe` at `:275`, `parse_statement` at `:340`).

---

## Headless flags (`crates/cathedral-backends/src/bin/cathedral_headless.rs`, clap `struct Args` at `:80`)

```rust
// M1
/// load an extra authored fact pack (repeatable): --facts quest/pack.json
#[arg(long, value_name = "FILE")] facts: Vec<PathBuf>,
/// print one line per (holder, fact) after the world loads
#[arg(long)] trace_knowledge: bool,
// M2
/// print a `[pollen]` census per topic each sample: carriers per ward per game
/// hour, wards reached, crossings, store bytes. The cadence band's instrument.
/// Caps the watch step at 0.4 s (the most walking one poll realises, D22).
#[arg(long)] trace_pollen: bool,
/// how many `--trace-pollen` samples a watched game day takes (default 24)
#[arg(long, default_value_t = 24.0)] pollen_per_day: f64,
/// plant one authored fact per topic at a named place, so nine bands can be
/// measured in one run; the measurement pack, not shipped content
#[arg(long, value_name = "PLACE")] pollen_seed: Option<String>,
/// after --pollen-seed, put the pack's nine rows in every ward's air at full
/// heat, so the 20,000 cost guard measures a saturated roll load and not two
/// authored facts (`02_numbers.md` §6)
#[arg(long)] pollen_saturate: bool,
/// the watch-clock step in real seconds while --trace-pollen is on (default
/// 0.4, the most walking one poll realises; the cost guard passes 3)
#[arg(long, default_value_t = 0.4)] pollen_step: f64,
/// run with every salience band and affinity at 1.0 — the identity run
#[arg(long)] pollen_flat: bool,
/// run with the salience term removed from the roll entirely — the baseline
/// `--pollen-flat` must reproduce (`02_numbers.md`, the flat-table identity)
#[arg(long)] pollen_no_salience: bool,
```
`CATHEDRAL_NO_KNOWLEDGE=1` reaches this binary too: M1 step 17b wires
`knowledge_enabled: std::env::var_os("CATHEDRAL_NO_KNOWLEDGE").is_none()` into the `EngineConfig`
literal (`cathedral_headless.rs:476`).
