//! Who is allowed to think, and when.
//!
//! The turn stream has always been a clock: the round robin handed out slots
//! whether or not anybody was there to notice. This module is the answer to the
//! rule a player would state — an NPC thinks because you are near them, because
//! you spoke to them, or because something happened to them, never merely
//! because their name came up in a rotation
//! (`features/gate_idle_cognition_on_proximity.md`).
//!
//! Proximity was only half the rule. Being near someone justifies keeping them
//! *simulated*; it does not justify asking them, every three seconds, whether
//! they have changed their mind about saying nothing. So an on-stage actor also
//! has to have **news** — [`Novelty`], the second gate
//! (`features/gate_idle_cognition_on_novelty.md`). A turn that ends in silence
//! changes nothing for anybody, which is exactly why it must not buy another
//! turn: that is the loop that pays to be told nothing happened.
//!
//! News was two thirds of the rule. It bought silence for free, but it still let
//! *everybody* remark on the stranger who walked into their street, which is
//! neither what a city sounds like nor what it should cost. So the third gate is
//! **character** — [`CuriosityConfig`]: who speaks first is a fact about the
//! person, not about the scheduler. It applies to unprompted initiative and to
//! nothing else. An aloof NPC never opens, but always answers.
//!
//! It is pure and cheap: [`on_stage`] is a distance query the engine recomputes
//! once per poll, [`Novelty`] is a hash comparison beside it, and [`IdleGate`]
//! is all the scheduler ever learns about either. Nothing here gates the lanes
//! that fire on a *real* event — an addressed `say`, a sound nudge, the player's
//! own speech. Those reach an ambient fishmonger in an empty field exactly as
//! they always did, and an aloof NPC with no news still answers you the instant
//! you speak to them.

use std::{
    collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use crate::{
    HEARING_RADIUS_M, ITEM_INTERACTION_RADIUS_M,
    character::{Character, Control},
    ids::ActorId,
    lore::LoreProfile,
    world::World,
};

/// Deliberately larger than `HEARING_RADIUS_M` (20 m).
///
/// The tell this feature risks is statues that come alive when looked at. A
/// generous radius is the cheap mitigation: by the time you are in earshot, the
/// NPC has been taking turns for the better part of a minute.
pub const DEFAULT_STAGE_RADIUS_M: f64 = 32.0;

/// How many neighbours may idle at once. Not a cost cap on concurrency — the
/// scheduler still allows exactly one request in flight — but on how thinly the
/// rotation is spread: six people around you each get a turn every ~19 s, where
/// 240 people would each get one every 13 minutes.
pub const DEFAULT_STAGE_MAX_ACTORS: usize = 6;

/// `characters_within` demands a finite radius, and a fat-fingered `config.ron`
/// must not panic the engine. The city is about 1.2 × 1.0 km, so this bound is
/// "the whole world" in practice — use `IdleCognitionMode::All` if that is what
/// you actually meant.
pub const MAX_STAGE_RADIUS_M: f64 = 2_000.0;

/// How long the player's conversation partner keeps their reserved seat after
/// the last line either of them addressed to the other.
///
/// Long enough to cover walking backwards out of the radius mid-sentence; short
/// enough that a conversation you simply walked away from stops costing
/// anything. Without a lapse the partner would idle forever, and "alone in a
/// field" would cost one prompt per rotation for the rest of the run.
pub const STAGE_PARTNER_MEMORY_SECONDS: f64 = 30.0;

/// The warm NPC↔NPC exchanges, keyed by unordered actor pair → when the last
/// targeted line (or item handoff) between them landed.
///
/// The pair-keyed generalization of the engine's single `last_player_exchange`
/// slot: while a pair is warm, the daily round holds both of them where they
/// stand — nobody keeps walking away from a conversation — and the same
/// [`STAGE_PARTNER_MEMORY_SECONDS`] silence lapses it, with no explicit
/// "conversation over" event. The player's own slot stays separate, because it
/// also feeds the stage's reserved seat and must keep its exact semantics.
///
/// Like [`Novelty`], this is derived bookkeeping, not world state: it lives on
/// the engine, never in [`World`], and a save file must not carry it. It is
/// bounded by the number of *concurrent* conversations — in practice a handful
/// — so a plain `BTreeMap` is the whole data structure.
#[derive(Debug, Clone, Default)]
pub struct WarmExchanges {
    pairs: BTreeMap<(ActorId, ActorId), f64>,
}

impl WarmExchanges {
    /// Record a targeted exchange between `a` and `b` at `now`. Order-blind:
    /// who spoke and who listened is the same conversation.
    pub fn note(&mut self, a: &ActorId, b: &ActorId, now: f64) {
        if a == b {
            return;
        }
        let key = if a <= b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };
        self.pairs.insert(key, now);
    }

    /// Everyone currently in a warm exchange — the set whose round errands are
    /// on hold. Prunes lapsed pairs as it goes, which is what keeps the map
    /// from accumulating every conversation of the run.
    pub fn warm_actors(&mut self, now: f64) -> BTreeSet<ActorId> {
        self.pairs
            .retain(|_, last_line_at| now - *last_line_at < STAGE_PARTNER_MEMORY_SECONDS);
        let mut actors = BTreeSet::new();
        for (a, b) in self.pairs.keys() {
            actors.insert(a.clone());
            actors.insert(b.clone());
        }
        actors
    }
}

/// Whether the idle lane is gated on the player's neighborhood at all.
///
/// `All` reproduces the pre-gate behavior exactly, which is what makes this
/// feature a one-line A/B in `config.ron` and what keeps the scheduler tests and
/// the headless runner exercising the full cast without faking proximity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdleCognitionMode {
    #[default]
    All,
    Stage,
}

impl IdleCognitionMode {
    /// `config.ron` carries the mode as a string, like `tts_backend` and
    /// `stt_backend`. Anything but an explicit `"all"` gates.
    pub fn from_config(value: &str) -> Self {
        if value.trim().eq_ignore_ascii_case("all") {
            Self::All
        } else {
            Self::Stage
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Stage => "stage",
        }
    }
}

/// The player's neighborhood, in metres and in bodies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageConfig {
    pub radius_m: f64,
    pub max_actors: usize,
}

impl Default for StageConfig {
    fn default() -> Self {
        Self {
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: DEFAULT_STAGE_MAX_ACTORS,
        }
    }
}

/// Which actors the scheduler's *idle* lanes may select this poll.
///
/// The protected player-reaction lane is never gated by this — the player spoke,
/// so someone answers — and neither variant can start a turn that is not in
/// `order` to begin with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleGate<'a> {
    /// Every actor in the rotation may idle. Today's behavior.
    All,
    /// Only these may: the set [`on_stage`] computed for this poll.
    Stage(&'a BTreeSet<ActorId>),
    /// Nobody may — not even the ordinary priority slot. The player is
    /// mid-utterance, and the one in-flight slot is about to be his.
    ///
    /// The scheduler cannot preempt a provider call once it is out, so the only
    /// way to keep the player's words from queueing behind two seconds of some
    /// irrelevant NPC's thinking is to not start that turn at all.
    Suppressed,
}

impl IdleGate<'_> {
    pub fn allows(&self, actor_id: &ActorId) -> bool {
        match self {
            Self::All => true,
            Self::Stage(stage) => stage.contains(actor_id),
            Self::Suppressed => false,
        }
    }

    /// Whether even an event-driven handoff should wait.
    pub fn is_suppressed(&self) -> bool {
        matches!(self, Self::Suppressed)
    }
}

/// The actors eligible for an idle turn: the player's neighborhood, nearest
/// first, plus whoever the player is currently in an exchange with.
///
/// `partner` keeps a reserved seat — it is exactly the case where distance has
/// stopped being the right question. The engine supplies it from the last
/// targeted line the player and an NPC exchanged, and lets it lapse after
/// [`STAGE_PARTNER_MEMORY_SECONDS`].
///
/// `World::characters_within` already orders by distance then id, so the cap is
/// free and the result is deterministic.
pub fn on_stage(
    world: &World,
    player_id: &ActorId,
    partner: Option<&ActorId>,
    config: &StageConfig,
) -> BTreeSet<ActorId> {
    let mut stage = BTreeSet::new();
    if let Some(partner) = partner.filter(|actor_id| is_llm(world, actor_id)) {
        stage.insert(partner.clone());
    }
    let Some(player) = world.characters.get(player_id) else {
        return stage;
    };
    // NOT `clamp`, which clippy will offer and which would be a bug: `clamp`
    // returns NaN for a NaN input, and `characters_within` asserts the radius is
    // finite. `f64::max` returns the other operand for NaN, so this chain
    // sanitizes a garbage `config.ron` radius to zero instead of panicking the
    // engine (`a_garbage_radius_empties_the_stage_instead_of_panicking`).
    #[allow(clippy::manual_clamp)]
    let radius_m = config.radius_m.max(0.0).min(MAX_STAGE_RADIUS_M);
    for actor_id in world.characters_within(player.position_m(), radius_m, Some(player_id)) {
        if stage.len() >= config.max_actors {
            break;
        }
        // The player is already excluded; this drops any other non-LLM body, and
        // a nearby partner simply re-inserts as a no-op.
        if is_llm(world, &actor_id) {
            stage.insert(actor_id);
        }
    }
    stage
}

fn is_llm(world: &World, actor_id: &ActorId) -> bool {
    world
        .characters
        .get(actor_id)
        .is_some_and(|character| character.control() == Control::Llm)
}

/// How long an actor remembers having thought, once they are off the stage.
///
/// Without a lapse the record is immortal, and an NPC you walked away from and
/// came back to an hour later would find nothing had changed since he last
/// thought — because, for him, nothing had. He would never look up again.
///
/// With one, the arrival of a player who has actually been *gone* is news
/// again. It must be comfortably longer than the churn it exists to absorb (see
/// [`Novelty::observe`]), and it is the knob to turn if returning to a street
/// re-greets you too eagerly.
pub const NOVELTY_MEMORY_SECONDS: f64 = 60.0;

/// What each actor had already been told when their last prompt went out.
///
/// This is derived bookkeeping, not world state, and it deliberately does not
/// live in [`World`]: nothing in the simulation may branch on it, and a save
/// file must not carry it. The engine owns one and recomputes it beside
/// [`on_stage`], once per poll (D20).
#[derive(Debug, Clone, Default)]
pub struct Novelty {
    last_told: BTreeMap<ActorId, Memory>,
}

#[derive(Debug, Clone, Copy)]
struct Memory {
    /// [`context_hash`] as it stood the moment their prompt was submitted — the
    /// world exactly as that prompt showed it to them. `None` while they have
    /// been *watched* but never *told*: on stage, owed their first thought, and
    /// in the map only so that [`Memory::visit`] has somewhere to live.
    context: Option<u64>,
    /// Which meeting this is: `now.to_bits()` from the poll we first noticed
    /// them, held unchanged for as long as the memory lives.
    ///
    /// It exists for the curiosity roll, which is otherwise a function of who
    /// they are and what they are looking at — both permanent. Without a per-
    /// meeting term the verdict would be a permanent fact about the pair of you:
    /// the man who did not look up the first time you walked down his street
    /// would never look up, in any hour of any run, because the street he is
    /// looking at hashes the same every time. With it, each meeting is its own
    /// roll, and a meeting ends where the memory does — on absence.
    visit: u64,
    /// When we last cared about this actor: the last poll they were on stage, or
    /// the last turn they took. Entries lapse [`NOVELTY_MEMORY_SECONDS`] after
    /// that, which is also what keeps the map bounded by the cast the player has
    /// actually met.
    touched_at: f64,
}

impl Novelty {
    /// Refresh the stage's memories and drop whoever has been gone too long.
    ///
    /// Touching every on-stage actor each poll — rather than only those who take
    /// a turn — is what makes the lapse mean *absence* instead of *silence*.
    /// Somebody standing quietly beside you for ten minutes is not forgotten and
    /// re-greeted; he is simply someone with nothing to say.
    ///
    /// Called once per poll, before [`Self::has_news`].
    pub fn observe(&mut self, now: f64, stage: &BTreeSet<ActorId>) {
        for actor_id in stage {
            match self.last_told.get_mut(actor_id) {
                Some(memory) => memory.touched_at = now,
                // Somebody new in front of the player. They are owed their first
                // thought either way (`context: None` reads as news below); the
                // entry exists so the meeting itself gets an identity, which is
                // what stops the curiosity roll from being the same coin flip
                // forever.
                None => {
                    self.last_told.insert(actor_id.clone(), Memory::new(now));
                }
            }
        }
        // A NaN `now` would make every comparison false and retain everything,
        // which is the safe direction: a stale memory costs nothing but a missed
        // greeting, where a wrongly-dropped one costs a prompt.
        self.last_told
            .retain(|_, memory| now - memory.touched_at <= NOVELTY_MEMORY_SECONDS);
    }

    /// Whether anything has happened to `actor_id` since they last thought.
    ///
    /// An actor with no memory at all has news by definition — they have never
    /// been told anything, so the world in front of them is entirely new. That
    /// is the one turn an arrival is worth.
    pub fn has_news(&self, world: &World, actor_id: &ActorId) -> bool {
        let Some(actor) = world.characters.get(actor_id) else {
            return false;
        };
        // Somebody spoke near them, a sound reached them, a system line landed.
        // The inbox needs no memory of its own: a turn is what drains it, so a
        // non-empty one is *by construction* something they have not yet been
        // shown — including anything that arrived while their own prompt was in
        // flight.
        !actor.inbox().is_empty() || self.context_is_new(world, actor_id)
    }

    /// Whether `actor_id` may take an idle turn: they have news, and — if the
    /// only news is that the world in front of them changed shape — they are the
    /// sort of person who says something about it.
    ///
    /// The split is the whole of the curiosity design. An **inbox** is somebody
    /// else's initiative arriving: a word, a bell, a system line. It is answered
    /// by everyone, aloof or not, and it is what makes the reticence below safe
    /// to ship. A **changed context** is nobody's initiative — the world merely
    /// rearranged itself in front of them — and remarking on that unbidden is
    /// exactly the thing that is a fact about the character.
    ///
    /// With curiosity disabled this is [`Self::has_news`] exactly, which is what
    /// lets the engine call it unconditionally.
    pub fn admits_idle(
        &self,
        world: &World,
        actor_id: &ActorId,
        curiosity: &CuriosityConfig,
    ) -> bool {
        let Some(actor) = world.characters.get(actor_id) else {
            return false;
        };
        if !actor.inbox().is_empty() {
            return true;
        }
        let context = context_hash(world, actor_id);
        let memory = self.last_told.get(actor_id);
        if memory.is_some_and(|memory| memory.context == Some(context)) {
            return false;
        }
        // `visit` defaults to zero for an actor with no memory at all, which the
        // engine cannot produce (`observe` runs first) but a direct caller can.
        opens_first(
            actor_id,
            context,
            memory.map_or(0, |memory| memory.visit),
            curiosity.chance(world, actor_id),
        )
    }

    /// Record that `actor_id`'s prompt has gone out, showing them the world as it
    /// stands. Everything that happens after this instant is news.
    ///
    /// Submission — not completion — is the honest moment: the render has just
    /// drained their inbox, and a line that lands during the two seconds their
    /// call is in flight must survive to be shown on their *next* turn. Stamping
    /// on completion would swallow it.
    pub fn told(&mut self, now: f64, world: &World, actor_id: &ActorId) {
        let context = Some(context_hash(world, actor_id));
        match self.last_told.get_mut(actor_id) {
            Some(memory) => {
                memory.context = context;
                memory.touched_at = now;
            }
            // A turn taken off stage: a sound nudge, or an answer called back
            // across a square the player has already left. The meeting starts
            // here, because as far as this actor is concerned it just did.
            None => {
                self.last_told.insert(
                    actor_id.clone(),
                    Memory {
                        context,
                        ..Memory::new(now)
                    },
                );
            }
        }
    }

    fn context_is_new(&self, world: &World, actor_id: &ActorId) -> bool {
        match self.last_told.get(actor_id) {
            None => true,
            Some(memory) => memory.context != Some(context_hash(world, actor_id)),
        }
    }
}

impl Memory {
    fn new(now: f64) -> Self {
        Self {
            context: None,
            visit: now.to_bits(),
            touched_at: now,
        }
    }
}

/// Everything an actor could react to that is not already in their inbox: who is
/// around them, and what is being offered.
///
/// The two rules that make this a gate rather than theatre:
///
/// - **Ids, never positions.** A neighbour's every step would otherwise be news,
///   and the player breathing on the spot would re-fire the whole stage.
///   `characters_within` orders by *distance*, so the ids are re-collected into
///   a [`BTreeSet`] first — otherwise two people merely swapping places would
///   reorder the hash and read as an arrival.
/// - **Their perspective, not the world's.** This mirrors the `you_see` and
///   `you_offer` / `offered_to_you` sections of the prompt, because those are
///   the only things a turn would show them. What it omits is as deliberate: the
///   clock, their own goal, their own inventory, their own `wait`. An actor is
///   not news to themselves.
pub fn context_hash(world: &World, actor_id: &ActorId) -> u64 {
    let mut hasher = DefaultHasher::new();
    let Some(actor) = world.characters.get(actor_id) else {
        return hasher.finish();
    };

    let seen: BTreeSet<ActorId> = world
        .characters_within(actor.position_m(), HEARING_RADIUS_M, Some(actor_id))
        .into_iter()
        .collect();
    for other_id in &seen {
        // Only a *settled* neighbour counts (`05_the_llm_seam.md` §5.1). A man
        // crossing the square at 1.8 m/s must not make his every step news; the
        // moment he stops (speed → 0) his arrival does. The player carries no
        // `Movement`, so he is always settled and always counted — unchanged.
        if world
            .characters
            .get(other_id)
            .is_some_and(|other| other.is_settled())
        {
            other_id.as_str().hash(&mut hasher);
        }
    }

    // `world.offers` is keyed by item id and iterates in that order, so this is
    // stable without a sort. An offer of an item that has left the world shows
    // in neither prompt section, so it must not show here either.
    for offer in world.offers.values() {
        if !world.items.contains_key(&offer.item_id) {
            continue;
        }
        if offer.giver_id == *actor_id {
            // `you_offer`: what they have put on the table, and to whom.
            offer.item_id.as_str().hash(&mut hasher);
            offer
                .target_id
                .as_ref()
                .map(ActorId::as_str)
                .hash(&mut hasher);
        } else if offer.target_id.is_none() || offer.target_id.as_ref() == Some(actor_id) {
            // `offered_to_you`, which the prompt drops beyond arm's reach — so
            // an offer walking into range is news, exactly as it reads.
            let Some(giver) = world.characters.get(&offer.giver_id) else {
                continue;
            };
            if actor.position_m().distance_squared(giver.position_m())
                <= ITEM_INTERACTION_RADIUS_M * ITEM_INTERACTION_RADIUS_M
            {
                offer.item_id.as_str().hash(&mut hasher);
                offer.giver_id.as_str().hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

// --------------------------------------------------------------- curiosity
//
// Who opens their mouth first (`features/gate_idle_cognition_on_novelty.md` §2).
// The novelty gate made silence free; it did not stop every one of the ~500
// people you walk past from thinking about you the moment you appear. That is
// both the bill and the silliness: a city where the magistrate, the gaoler and
// the anchoress each remark on a passing stranger is not a city.

/// The chance a person with no lore sheet remarks on you unbidden: all of it.
///
/// The demo trio, the test worlds, the headless fixtures — nobody there has a
/// character to be aloof *about*, and a world with no cast metadata must behave
/// exactly as it did before this feature. Reticence is a property of an authored
/// person, not a default.
pub const CURIOSITY_WITHOUT_LORE: f64 = 1.0;

/// Where a derived curiosity starts, before trade, age and standing move it.
///
/// It is a calibration constant and not a judgement — raise it and more of the
/// city speaks first — and the thing it is calibrated *against* is not itself.
/// Each person you walk past gets two pieces of news (you enter their street, and
/// then you enter their earshot) and each is rolled separately, so what a player
/// experiences is not the mean curiosity but roughly `1 − (1 − p)²` of it. This
/// value puts the measured per-passer-by rate at **19.3%** against a ~20% target,
/// walking the whole shipped cast (`cathedral-backends/tests/curiosity_walk.rs`).
///
/// Do not reason the number out from the mean. Measure it: the walk is the only
/// place the `max_actors` cap, the turn rate and the real spatial clustering of
/// the cast are all in the room at once, and between them they cost about half a
/// roll per person that the arithmetic above does not know about.
pub const CURIOSITY_BASE: f64 = 0.082;

/// Nobody is *entirely* deaf to a stranger, and nobody accosts every single one.
///
/// The floor is deliberately a number a player can actually meet. At 0.01 the
/// reserved half of the city was not aloof, it was scenery: a watchman you walk
/// past thirty times has spoken to you once, which reads as a broken NPC rather
/// than a taciturn one. Rarely is a character; never is a bug.
const CURIOSITY_FLOOR: f64 = 0.03;
const CURIOSITY_CEILING: f64 = 0.60;

/// So that the roll is not a re-reading of the context hash's own bits.
const CURIOSITY_SALT: u64 = 0x_c0ff_ee15_600d_1dea;

/// Trades whose whole day is spent hailing strangers in the street. The list is
/// the vocabulary of `lore/characters/`'s own directories, bucketed by one
/// question: does this person make a living by speaking to someone who did not
/// speak first?
const STREET_TRADES: &[&str] = &[
    "boatworker",
    "draper",
    "entertainer",
    "fish_trader",
    "food_provisioner",
    "grocer_and_spicer",
    "guide",
    "healer",
    "lamplighter",
    "market_seller",
    "messenger",
    "pilgrim",
    "salt_trader",
    "scavenger",
    "sex_worker",
    "tavern_worker",
];

/// Office, cloister and ledger: people for whom addressing a passer-by is at
/// best not their business and at worst beneath them. A merchant is here rather
/// than among the traders above because he sells to a factor, not to a street.
const RESERVED_TRADES: &[&str] = &[
    "anchoress",
    "bailiff_and_gaoler",
    "bell_ringer",
    "candor_cleric",
    "church_attendant",
    "civic_officer",
    "court_officer",
    "custody_clerk",
    "executioner",
    "freight_broker",
    "funerary_worker",
    "merchant",
    "militia_and_soldier",
    "money_dealer",
    "revenue_worker",
    "scholar",
    "scribe_and_clerk",
    "watchman_and_keeper",
];

/// Whether unprompted initiative is a fact about the character at all, and how
/// far the whole city leans.
///
/// Off reproduces the pre-curiosity behavior exactly — everyone with news speaks
/// — which is what keeps the sim's own tests and the headless runner honest, and
/// what makes this a rebuild-free A/B in `config.ron`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuriosityConfig {
    pub enabled: bool,
    /// Multiplies every derived *and* authored curiosity before the roll. The
    /// one knob for "the city is too chatty" / "the city is dead" — the
    /// character-to-character texture is preserved, only the mean moves.
    pub scale: f64,
}

impl Default for CuriosityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scale: 1.0,
        }
    }
}

impl CuriosityConfig {
    /// How likely `actor_id` is to remark on something nobody said to them.
    ///
    /// A garbage `config.ron` scale must not make the city silent by accident or
    /// panic the roll: `f64::max` returns the other operand for NaN, so nonsense
    /// sanitizes to zero rather than propagating.
    pub fn chance(&self, world: &World, actor_id: &ActorId) -> f64 {
        if !self.enabled {
            return 1.0;
        }
        (curiosity_of(world, actor_id) * self.scale.max(0.0)).clamp(0.0, 1.0)
    }
}

/// The character's own willingness to speak first, before [`CuriosityConfig`]'s
/// city-wide scale.
pub fn curiosity_of(world: &World, actor_id: &ActorId) -> f64 {
    world
        .characters
        .get(actor_id)
        .and_then(Character::lore)
        .map_or(CURIOSITY_WITHOUT_LORE, curiosity_from_lore)
}

fn curiosity_from_lore(profile: &LoreProfile) -> f64 {
    // The authored number is the last word, and it is precisely what licenses the
    // caricature below. A derivation from age and trade is a fine *default* and a
    // terrible *verdict*: it will say that every guard is aloof and every child
    // is a chatterbox, and for some guard and some child it will be wrong. That
    // character gets a `curiosity` in their own JSON, and none of the other 499
    // files are touched.
    if let Some(curiosity) = profile.curiosity {
        return curiosity.clamp(0.0, 1.0);
    }
    derived_curiosity(profile)
}

/// Curiosity from the metadata the cast already carries. Kept legible and modest
/// on purpose — every term here is a stereotype, and the size of each is the
/// size of the apology owed for it.
///
/// Deliberately *not* a function of [`Significance`](crate::Significance), which
/// already sets the completion caps. The two are orthogonal and must stay so: an
/// ambient child is cheap and highly curious, a major canon is expensive and
/// aloof, and one field cannot mean both.
fn derived_curiosity(profile: &LoreProfile) -> f64 {
    let mut curiosity = CURIOSITY_BASE;

    // No fixed trade at all — the paupers, the dependents, the people the
    // validator makes carry a support status. The street is not where they work;
    // it is where they live, and a stranger in it is the day's event.
    curiosity += match profile.occupation_id.as_deref() {
        None => 0.10,
        Some(occupation) if STREET_TRADES.contains(&occupation) => 0.09,
        Some(occupation) if RESERVED_TRADES.contains(&occupation) => -0.02,
        Some(_) => 0.0,
    };

    // Children are curious; the very old have seen strangers before.
    curiosity += match profile.age {
        ..=14 => 0.09,
        15..=24 => 0.02,
        70.. => -0.02,
        _ => 0.0,
    };

    // Standing, and the two ways the cast carries it. `title` is *not* one of
    // them: the loader requires a title of everyone with a trade, so "Blacksmith"
    // is a job and not a station, and reading it as rank would make the whole
    // city haughty.
    if matches!(
        profile.rank.as_deref(),
        Some("master" | "mistress" | "warden")
    ) {
        curiosity -= 0.02;
    }
    // A sworn conspirator keeps his head down: the last thing a paid moth of the
    // Custody wants is to be the man who struck up a conversation.
    if profile.faction_role.is_some() {
        curiosity -= 0.03;
    }

    // Professionally curious in the most literal sense: a beggar has to speak
    // first, because it is the entire job. Counted **once** — `begs_regularly`
    // and `alms_dependent` are the same fact told twice, and most of the cast
    // that carries one carries the other, so adding both would buy the same
    // person the bonus twice over on nothing but bookkeeping.
    if profile
        .circumstances
        .iter()
        .any(|circumstance| matches!(circumstance.as_str(), "begs_regularly" | "alms_dependent"))
    {
        curiosity += 0.10;
    }
    for circumstance in &profile.circumstances {
        curiosity += match circumstance.as_str() {
            "unhoused" => 0.02,
            "enclosed_religious" => -0.05,
            "prisoner" => -0.03,
            "retired" => -0.02,
            _ => 0.0,
        };
    }

    curiosity.clamp(CURIOSITY_FLOOR, CURIOSITY_CEILING)
}

/// The verdict on one piece of news — and the one thing about it that matters is
/// that it is **stable**.
///
/// The engine polls at 60 Hz and the news outlives the poll: an actor who has
/// noticed you keeps on having noticed you until they take a turn. A fresh draw
/// each poll would therefore turn any chance at all into a certainty within a
/// frame or two — a 20% NPC would open his mouth on his fifth poll, which is to
/// say always, and the gate would be theatre. So there is no draw. The die *is*
/// the news: who they are, what they are looking at, and which meeting this is.
/// The same news yields the same verdict forever, and it changes only when the
/// news does (`the_same_news_never_changes_its_mind`).
fn opens_first(actor_id: &ActorId, context: u64, visit: u64, chance: f64) -> bool {
    // NaN is spelled out rather than left to `<=`, which would answer `false` and
    // let a garbage chance fall through to the roll. A chance nobody can read is
    // silence, not a coin flip.
    if chance.is_nan() || chance <= 0.0 {
        return false;
    }
    if chance >= 1.0 {
        return true;
    }
    let mut hasher = DefaultHasher::new();
    CURIOSITY_SALT.hash(&mut hasher);
    actor_id.as_str().hash(&mut hasher);
    context.hash(&mut hasher);
    visit.hash(&mut hasher);
    uniform(hasher.finish()) < chance
}

/// A hash's top 53 bits as a uniform `[0, 1)` — the same construction a float RNG
/// uses, and the reason `DefaultHasher`'s weaker low bits never reach the
/// comparison.
fn uniform(hash: u64) -> f64 {
    (hash >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CharacterSheet, Vec3, WALK_SPEED_MPS,
        character::{Character, Control, Movement, Patrol},
    };

    /// A live walk, fast enough to count as unsettled.
    fn walking() -> Movement {
        Movement {
            path: vec![Vec3::new(100.0, 0.0, 0.0)],
            speed: WALK_SPEED_MPS,
            gait_phase: 0.0,
            patrol: Some(Patrol {
                a: "a".into(),
                b: "b".into(),
                heading_to_b: true,
            }),
        }
    }

    fn character(id: &str, x: f64, control: Control) -> Character {
        Character::from_sheet(CharacterSheet {
            id: ActorId::from_raw(id),
            name: id.to_uppercase(),
            control,
            back_story: "test".into(),
            location_description: "the square".into(),
            appearance_key: id.into(),
            voice_key: None,
            position_m: Vec3::new(x, 0.0, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: crate::GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: None,
        })
    }

    /// The player at the origin, with company at 5, 15, 25 and 100 metres.
    fn world() -> World {
        let mut world = World::new();
        world.add_character(character("player", 0.0, Control::Player));
        world.add_character(character("near", 5.0, Control::Llm));
        world.add_character(character("mid", 15.0, Control::Llm));
        world.add_character(character("far", 25.0, Control::Llm));
        world.add_character(character("away", 100.0, Control::Llm));
        world
    }

    fn ids(stage: &BTreeSet<ActorId>) -> Vec<&str> {
        stage.iter().map(ActorId::as_str).collect()
    }

    fn player() -> ActorId {
        ActorId::from_raw("player")
    }

    #[test]
    fn the_stage_is_the_neighborhood_and_never_the_player() {
        let stage = on_stage(&world(), &player(), None, &StageConfig::default());
        // 32 m by default: `far` is in, `away` is not, and the player himself is
        // never scheduled.
        assert_eq!(ids(&stage), ["far", "mid", "near"]);
    }

    #[test]
    fn an_empty_field_costs_nothing() {
        let mut world = world();
        // Walk the player 500 m out. This is the headline number: no idle lane,
        // no prompt, no bill.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(500.0, 0.0, 0.0);
        assert!(on_stage(&world, &player(), None, &StageConfig::default()).is_empty());
    }

    #[test]
    fn the_cap_keeps_the_nearest_and_drops_the_rest() {
        let config = StageConfig {
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: 2,
        };
        // Nearest-first truncation, even though the set itself sorts by id: the
        // 25 m `far` is the one dropped, not the alphabetically-last `near`.
        assert_eq!(
            ids(&on_stage(&world(), &player(), None, &config)),
            ["mid", "near"]
        );
    }

    #[test]
    fn the_conversation_partner_keeps_a_seat_outside_the_radius() {
        let away = ActorId::from_raw("away");
        let config = StageConfig {
            radius_m: DEFAULT_STAGE_RADIUS_M,
            max_actors: 2,
        };
        let stage = on_stage(&world(), &player(), Some(&away), &config);
        // The partner is seated first and the cap spends what is left on the
        // nearest neighbour — you keep talking to the person you walked away
        // from, and the person beside you still lives.
        assert_eq!(ids(&stage), ["away", "near"]);
    }

    #[test]
    fn a_non_llm_partner_is_not_smuggled_onto_the_stage() {
        let mut world = world();
        world.add_character(character("statue", 200.0, Control::Player));
        let statue = ActorId::from_raw("statue");
        let stage = on_stage(&world, &player(), Some(&statue), &StageConfig::default());
        assert_eq!(ids(&stage), ["far", "mid", "near"]);
    }

    #[test]
    fn a_garbage_radius_empties_the_stage_instead_of_panicking() {
        for radius_m in [f64::NAN, -1.0] {
            let config = StageConfig {
                radius_m,
                max_actors: DEFAULT_STAGE_MAX_ACTORS,
            };
            assert!(on_stage(&world(), &player(), None, &config).is_empty());
        }
        // Infinity clamps to the whole world rather than tripping the assertion.
        let config = StageConfig {
            radius_m: f64::INFINITY,
            max_actors: DEFAULT_STAGE_MAX_ACTORS,
        };
        assert_eq!(
            ids(&on_stage(&world(), &player(), None, &config)),
            ["away", "far", "mid", "near"]
        );
    }

    #[test]
    fn the_gate_admits_by_variant() {
        let stage: BTreeSet<ActorId> = [ActorId::from_raw("near")].into_iter().collect();
        let near = ActorId::from_raw("near");
        let away = ActorId::from_raw("away");

        assert!(IdleGate::All.allows(&away));
        assert!(IdleGate::Stage(&stage).allows(&near));
        assert!(!IdleGate::Stage(&stage).allows(&away));
        assert!(!IdleGate::Suppressed.allows(&near));
        assert!(IdleGate::Suppressed.is_suppressed());
        assert!(!IdleGate::All.is_suppressed());
    }

    /// The load-bearing rule. If novelty keys on coordinates, the player
    /// breathing on the spot re-fires the whole stage and the feature is theatre.
    #[test]
    fn shuffling_on_the_spot_is_not_news() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let before = context_hash(&world, &near);

        // Everyone jiggles, nobody arrives or leaves. `characters_within` orders
        // by distance, so `mid` and `far` also *swap places* here — the case that
        // would reorder a naively-hashed list and read as an arrival.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(1.0, 0.0, 0.0);
        world
            .characters
            .get_mut(&ActorId::from_raw("mid"))
            .unwrap()
            .state
            .position_m = Vec3::new(18.0, 0.0, 0.0);
        world
            .characters
            .get_mut(&ActorId::from_raw("far"))
            .unwrap()
            .state
            .position_m = Vec3::new(17.0, 0.0, 0.0);

        assert_eq!(context_hash(&world, &near), before);
    }

    /// …and the other half of the same rule: a *body* arriving or leaving is.
    #[test]
    fn somebody_arriving_or_leaving_is_news() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let with_player = context_hash(&world, &near);

        // The player walks out of `near`'s 20 m hearing radius.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(500.0, 0.0, 0.0);
        let without_player = context_hash(&world, &near);
        assert_ne!(without_player, with_player);

        // And back. The world looks exactly as it did, so the hash says so —
        // this is what `Novelty`'s lapse exists to override.
        world
            .characters
            .get_mut(&player())
            .unwrap()
            .state
            .position_m = Vec3::new(0.0, 0.0, 0.0);
        assert_eq!(context_hash(&world, &near), with_player);
    }

    /// 05_the_llm_seam.md §5.1: a neighbour in motion drops out of the hash so
    /// his every step is not news; the moment he stops he is counted again, so a
    /// genuine arrival still fires.
    #[test]
    fn a_moving_neighbour_is_not_news_but_a_stopped_one_is() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let mid = ActorId::from_raw("mid");

        // Baseline: everyone within earshot is settled and counted.
        let all_settled = context_hash(&world, &near);

        // `mid` starts walking — excluded from `near`'s hash, so it changes.
        world.characters.get_mut(&mid).unwrap().state.movement = Some(walking());
        let mid_walking = context_hash(&world, &near);
        assert_ne!(
            mid_walking, all_settled,
            "a mover drops out of the settled-neighbour hash"
        );

        // `mid` stops (speed 0): settled again, counted again, back to baseline.
        world
            .characters
            .get_mut(&mid)
            .unwrap()
            .state
            .movement
            .as_mut()
            .unwrap()
            .speed = 0.0;
        assert_eq!(
            context_hash(&world, &near),
            all_settled,
            "a man who stops is an arrival again"
        );
    }

    #[test]
    fn an_inbox_is_news_all_by_itself() {
        let world = world();
        let near = ActorId::from_raw("near");
        let mut novelty = Novelty::default();

        // Never thought: the world in front of them is entirely new.
        assert!(novelty.has_news(&world, &near));

        novelty.told(0.0, &world, &near);
        assert!(!novelty.has_news(&world, &near));

        // Somebody speaks. Nobody moved, so the context hash is untouched — the
        // inbox is the whole signal, and it needs no memory of its own because a
        // turn is what drains it.
        let mut world = world;
        world
            .characters
            .get_mut(&near)
            .unwrap()
            .notify_percept("Someone said: \"Hoy!\"".to_string());
        assert!(novelty.has_news(&world, &near));
    }

    /// A turn that ends in silence must not buy another turn. This is the loop
    /// the whole feature exists to break.
    #[test]
    fn silence_stays_silent() {
        let world = world();
        let near = ActorId::from_raw("near");
        let mut novelty = Novelty::default();

        novelty.told(0.0, &world, &near);
        // A `wait` emits no domain event and no percept, so nothing about the
        // world has changed — and nothing keeps changing, poll after poll.
        for tick in 0..100 {
            novelty.observe(tick as f64 * 0.1, &[near.clone()].into_iter().collect());
            assert!(!novelty.has_news(&world, &near), "re-fired at tick {tick}");
        }
    }

    /// Standing quietly beside the player for ten minutes is not absence. If the
    /// lapse counted silence rather than absence, the gate would leak a turn per
    /// actor per minute — exactly the bill it is meant to remove.
    #[test]
    fn a_quiet_neighbour_is_never_forgotten() {
        let world = world();
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();
        let mut novelty = Novelty::default();
        novelty.told(0.0, &world, &near);

        let mut now = 0.0;
        while now < 600.0 {
            now += 1.0;
            novelty.observe(now, &stage);
        }
        assert!(!novelty.has_news(&world, &near));
    }

    /// But a real absence is. Walk away, come back, and he looks up — even though
    /// the street he last thought about is exactly the street he sees now.
    ///
    /// This is also what absorbs the churn of the `max_actors` cap: a neighbour
    /// bumped off the stage for a second by somebody closer keeps his memory, and
    /// does not re-greet you for the privilege of coming back.
    #[test]
    fn an_actor_the_player_left_forgets_they_ever_met() {
        let world = world();
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();
        let empty = BTreeSet::new();
        let mut novelty = Novelty::default();
        novelty.told(0.0, &world, &near);

        // Off stage, but only briefly — the cap churned, or the player stepped
        // over the radius and back.
        novelty.observe(NOVELTY_MEMORY_SECONDS - 1.0, &empty);
        novelty.observe(NOVELTY_MEMORY_SECONDS - 1.0, &stage);
        assert!(!novelty.has_news(&world, &near));

        // Genuinely gone. The memory lapses, and the next arrival is worth a turn.
        novelty.observe(NOVELTY_MEMORY_SECONDS * 3.0, &empty);
        assert!(novelty.has_news(&world, &near));
    }

    #[test]
    fn an_offer_is_news_and_an_unknown_actor_never_is() {
        let mut world = world();
        let near = ActorId::from_raw("near");
        let before = context_hash(&world, &near);

        // `mid` is 10 m away — well outside the 4 m the prompt will show an
        // offer at, so it is not yet `near`'s business.
        let coin = crate::ids::ItemId::from_raw("c0prs");
        world
            .items
            .insert(coin.clone(), crate::item::Item::new(coin.clone(), "a coin"));
        world.offers.insert(
            coin.clone(),
            crate::offer::Offer {
                item_id: coin,
                giver_id: ActorId::from_raw("mid"),
                target_id: Some(near.clone()),
                created_seq: 1,
            },
        );
        assert_eq!(context_hash(&world, &near), before);

        // The giver walks into arm's reach: now it appears in `offered_to_you`,
        // and an offer appearing in front of you is news.
        world
            .characters
            .get_mut(&ActorId::from_raw("mid"))
            .unwrap()
            .state
            .position_m = Vec3::new(6.0, 0.0, 0.0);
        assert_ne!(context_hash(&world, &near), before);

        // A hash for somebody who is not in the world is a question with no
        // answer, and must not panic.
        let novelty = Novelty::default();
        assert!(!novelty.has_news(&world, &ActorId::from_raw("nobody")));
    }

    #[test]
    fn only_an_explicit_all_turns_the_gate_off() {
        assert_eq!(
            IdleCognitionMode::from_config("all"),
            IdleCognitionMode::All
        );
        assert_eq!(
            IdleCognitionMode::from_config(" ALL "),
            IdleCognitionMode::All
        );
        assert_eq!(
            IdleCognitionMode::from_config("stage"),
            IdleCognitionMode::Stage
        );
        assert_eq!(
            IdleCognitionMode::from_config("nonsense"),
            IdleCognitionMode::Stage
        );
    }

    // ------------------------------------------------------------- curiosity

    use crate::{PlanningWard, Significance, lore::LoreProfile};

    /// The shape of a shipped lore sheet, with only the fields curiosity reads
    /// left interesting.
    fn profile() -> LoreProfile {
        LoreProfile {
            significance: Significance::Ambient,
            planning_ward: PlanningWard::Fabric,
            age: 40,
            gender: "f".into(),
            occupation_id: Some("mason".into()),
            occupation_display: Some("Mason".into()),
            title: Some("Mason".into()),
            rank: None,
            faction_role: None,
            illegal_activity: None,
            district: "The Gradine".into(),
            father: None,
            mother: None,
            children: Vec::new(),
            circumstances: Vec::new(),
            conditions: Vec::new(),
            core_character_description: "You lay stone.".into(),
            extended_character_description: String::new(),
            curiosity: None,
        }
    }

    /// A world in which `near` is somebody in particular.
    fn world_with(profile: LoreProfile) -> World {
        let mut world = world();
        world
            .characters
            .get_mut(&ActorId::from_raw("near"))
            .unwrap()
            .sheet
            .lore = Some(profile);
        world
    }

    fn on(scale: f64) -> CuriosityConfig {
        CuriosityConfig {
            enabled: true,
            scale,
        }
    }

    /// **The correctness trap.** The engine polls at 60 Hz and the news outlives
    /// the poll — an actor who has noticed you keeps on having noticed you until
    /// he takes a turn. A fresh draw per poll would therefore make a 20% chance
    /// a certainty inside of a frame or two: he would refuse four times and open
    /// his mouth on the fifth, which is to say always, and the gate would buy
    /// nothing at all.
    ///
    /// So the verdict is a function of the news, not of the clock. It never
    /// flips while the news stands.
    #[test]
    fn the_same_news_never_changes_its_mind() {
        let world = world_with(LoreProfile {
            curiosity: Some(0.2),
            ..profile()
        });
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();

        let mut novelty = Novelty::default();
        novelty.observe(0.0, &stage);
        // The whole point of the gate: this actor *has news* (he has never
        // thought at all), and the only question left is whether he is the sort
        // of person who says anything about it.
        assert!(novelty.has_news(&world, &near));
        let verdict = novelty.admits_idle(&world, &near, &on(1.0));

        let mut now = 0.0;
        for poll in 0..600 {
            now += 1.0 / 60.0;
            novelty.observe(now, &stage);
            assert_eq!(
                novelty.admits_idle(&world, &near, &on(1.0)),
                verdict,
                "the roll was re-drawn on poll {poll}: a chance became a certainty"
            );
        }
    }

    /// *"An aloof NPC never opens, but always answers."* The inbox is somebody
    /// else's initiative arriving — a word, a bell, a system line — and curiosity
    /// has no opinion about it. Only the changed-context branch is rolled.
    ///
    /// This is what makes the reticence safe to ship: speak to the haughtiest
    /// magistrate in the city and he is idle-eligible on the very next poll.
    #[test]
    fn an_aloof_npc_never_opens_but_always_answers() {
        let mut world = world_with(LoreProfile {
            curiosity: Some(0.0),
            ..profile()
        });
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();
        let mut novelty = Novelty::default();
        novelty.observe(0.0, &stage);

        // A stranger has walked into his street. He has news, and he does not
        // care to remark on it — not now, not on any later poll.
        assert!(novelty.has_news(&world, &near));
        assert!(!novelty.admits_idle(&world, &near, &on(1.0)));

        // The stranger speaks. He answers, exactly as he always did.
        world
            .characters
            .get_mut(&near)
            .unwrap()
            .notify_percept("PLAYER said to you: \"Good evening.\"".to_string());
        assert!(novelty.admits_idle(&world, &near, &on(1.0)));
    }

    /// Curiosity is a fact about the *character*, and the character is read off
    /// the sheet the cast already carries. Legible, modest, and — where it is
    /// wrong — overridable.
    #[test]
    fn curiosity_is_derived_from_the_person() {
        let tradesman = derived_curiosity(&profile());

        let beggar = derived_curiosity(&LoreProfile {
            occupation_id: None,
            occupation_display: None,
            title: None,
            circumstances: vec!["pauper".into(), "begs_regularly".into()],
            ..profile()
        });
        let hawker = derived_curiosity(&LoreProfile {
            occupation_id: Some("market_seller".into()),
            ..profile()
        });
        let child = derived_curiosity(&LoreProfile {
            age: 9,
            ..profile()
        });
        let watchman = derived_curiosity(&LoreProfile {
            occupation_id: Some("watchman_and_keeper".into()),
            ..profile()
        });
        let canon = derived_curiosity(&LoreProfile {
            occupation_id: Some("candor_cleric".into()),
            rank: Some("master".into()),
            age: 71,
            ..profile()
        });

        assert!(beggar > hawker, "{beggar} !> {hawker}");
        assert!(hawker > tradesman, "{hawker} !> {tradesman}");
        assert!(child > tradesman, "{child} !> {tradesman}");
        assert!(tradesman > watchman, "{tradesman} !> {watchman}");
        assert!(watchman > canon, "{watchman} !> {canon}");
        // Nobody is *entirely* deaf to a stranger, and nobody accosts every one.
        assert!(canon >= CURIOSITY_FLOOR && beggar <= CURIOSITY_CEILING);

        // Significance is the completion budget and nothing else. An ambient
        // child is cheap and forward; a major canon is expensive and aloof.
        let major = derived_curiosity(&LoreProfile {
            significance: Significance::Major,
            ..profile()
        });
        assert_eq!(major, tradesman);
    }

    /// The escape hatch from the caricature above. The derivation will say that
    /// every guard is aloof, and for some guard it will be wrong — so his own
    /// file gets the last word, and the other 499 are not touched.
    #[test]
    fn an_authored_curiosity_beats_the_derivation() {
        let chatty_watchman = LoreProfile {
            occupation_id: Some("watchman_and_keeper".into()),
            curiosity: Some(0.9),
            ..profile()
        };
        assert_eq!(curiosity_from_lore(&chatty_watchman), 0.9);

        let world = world_with(chatty_watchman);
        assert_eq!(curiosity_of(&world, &ActorId::from_raw("near")), 0.9);
    }

    /// A world with no lore is the world before this feature: the demo trio, the
    /// fixtures, the headless seed. Reticence is a property of an authored
    /// person, and there is nobody here to be reticent.
    #[test]
    fn nobody_without_a_lore_sheet_is_aloof() {
        let world = world();
        let near = ActorId::from_raw("near");
        assert_eq!(curiosity_of(&world, &near), CURIOSITY_WITHOUT_LORE);
        assert!(on(1.0).chance(&world, &near) >= 1.0);

        let mut novelty = Novelty::default();
        novelty.observe(0.0, &[near.clone()].into_iter().collect());
        assert!(novelty.admits_idle(&world, &near, &on(1.0)));
        // …and disabled, everyone with news speaks, lore or no lore. That is what
        // makes `config.ron: curiosity: false` an honest A/B.
        assert!(novelty.admits_idle(&world, &near, &CuriosityConfig::default()));
    }

    /// The one tuning knob. It moves the city's mean without touching the
    /// character-to-character texture — and a fat-fingered `config.ron` must
    /// silence the streets or leave them be, never panic the roll.
    #[test]
    fn the_scale_moves_the_whole_city_and_survives_garbage() {
        let world = world_with(LoreProfile {
            curiosity: Some(0.2),
            ..profile()
        });
        let near = ActorId::from_raw("near");

        assert!((on(1.0).chance(&world, &near) - 0.2).abs() < 1e-9);
        assert!((on(0.5).chance(&world, &near) - 0.1).abs() < 1e-9);
        // Clamped, not wrapped: 10 × 0.2 is "always", not 2.0.
        assert_eq!(on(10.0).chance(&world, &near), 1.0);

        for scale in [0.0, -1.0, f64::NAN] {
            assert_eq!(on(scale).chance(&world, &near), 0.0, "scale {scale}");
        }
        let mut novelty = Novelty::default();
        novelty.observe(0.0, &[near.clone()].into_iter().collect());
        assert!(!novelty.admits_idle(&world, &near, &on(f64::NAN)));
    }

    /// The roll has to actually be a *roll*: a chance of `p` has to admit about
    /// `p` of the cast, or the calibration in the feature doc is a fiction.
    #[test]
    fn the_roll_is_uniform_across_the_cast() {
        for chance in [0.05, 0.2, 0.5] {
            let admitted = (0..2_000)
                .filter(|index| {
                    let actor_id = ActorId::from_raw(format!("a{index:04}"));
                    opens_first(&actor_id, 0x_dead_beef, 0x_1234, chance)
                })
                .count();
            let rate = admitted as f64 / 2_000.0;
            assert!(
                (rate - chance).abs() < 0.03,
                "a {chance} chance admitted {rate} of the cast"
            );
        }
    }

    /// The same street, the same face, and yet he might look up this time.
    ///
    /// Without a per-meeting term the roll would be a function of who you are and
    /// what you are looking at — both permanent — so the man who ignored you on
    /// Tuesday would ignore you on every Tuesday there has ever been. The
    /// meeting is what makes it a *chance* rather than a caste.
    #[test]
    fn a_new_meeting_is_a_new_roll() {
        let world = world_with(LoreProfile {
            curiosity: Some(0.3),
            ..profile()
        });
        let near = ActorId::from_raw("near");
        let stage: BTreeSet<ActorId> = [near.clone()].into_iter().collect();
        let empty = BTreeSet::new();
        let mut novelty = Novelty::default();

        // One walk down the street per lap. The crowd, the faces and the context
        // hash are identical every time — only the meeting is new, and only
        // because the player was genuinely gone in between.
        let mut now = 0.0;
        let verdicts: Vec<bool> = (0..40)
            .map(|_| {
                now += NOVELTY_MEMORY_SECONDS * 2.0;
                novelty.observe(now, &empty);
                now += 1.0;
                novelty.observe(now, &stage);
                novelty.admits_idle(&world, &near, &on(1.0))
            })
            .collect();

        assert!(
            verdicts.contains(&true) && verdicts.contains(&false),
            "every meeting drew the same verdict: the roll is a caste, not a chance"
        );
    }
}
