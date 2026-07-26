//! Authoritative world state and action execution for smart actors.
//!
//! A direct port of the Python sidecar's `sim.py` / `sounds.py`: the game
//! mirrors the public portion of this state but never commits an action
//! itself, so all validation lives here and LLM actions and player commands
//! have exactly the same semantics.
//!
//! The crate is pure domain: no bevy, no channels, no clock reads, no IO.
//! Distances are f64 metres ([`math::Vec3`] is `glam::DVec3`) — the hearing and
//! interaction boundaries are tested at `20.0 ± 1e-6`, which f32 cannot
//! represent.

pub mod actions;
pub mod appearance;
pub mod areas;
pub mod attention;
pub mod character;
pub mod clock;
pub mod engine;
pub mod error;
pub mod event;
pub mod fake;
pub mod floor;
pub mod gesture;
mod homes;
pub mod ids;
pub mod inventory;
pub mod item;
pub mod lore;
pub mod math;
pub mod nav;
pub mod night;
pub mod notices;
pub mod offer;
pub mod perception;
pub mod places;
pub mod prompt;
/// CPython text semantics (`str.strip`, `float.__repr__`, `repr`) — internal,
/// but shared by the action validators, the reply parser and the scheduler.
mod pyfmt;
pub mod round;
pub mod scheduler;
pub mod seed;
pub mod snapshot;
pub mod sounds;
pub mod speech_router;
pub mod status;
pub mod traits;
pub mod weather;
pub mod world;

pub use actions::{apply_action, apply_action_at};
pub use appearance::{AppearanceSnapshot, Build, Headgear, OutfitClass, palette_seed_of};
pub use areas::{
    Area, AreaBox, AreaMap, AreaMapError, AxisDirection, CoordinateSystem, NearestArea,
};
pub use attention::{
    CURIOSITY_BASE, CURIOSITY_WITHOUT_LORE, CuriosityConfig, DEFAULT_STAGE_MAX_ACTORS,
    DEFAULT_STAGE_RADIUS_M, IdleCognitionMode, IdleGate, NOVELTY_MEMORY_SECONDS, Novelty,
    STAGE_PARTNER_MEMORY_SECONDS, StageConfig, WarmExchanges, context_hash, curiosity_of, on_stage,
};
pub use character::{
    BodySlot, Character, CharacterSheet, CharacterState, Control, EconomicClass, GutEntry,
    IntentTarget, Movement, Needs, Patrol, PocketedUnit, Presence, RoundEdit, StatusKind,
    TravelIntent, VendorListing,
};
pub use clock::{
    BELL_STROKE_INTERVAL_SECONDS, Office, Weekday, WorldClock, WorldTime, stroke_times,
};
pub use engine::{ActorMotion, Capabilities, Engine, EngineCommand, EngineConfig, EngineMessage};
pub use error::{
    ActionError, ActionErrorCode, CommandError, CommandErrorCode, EngineInitError, PromptError,
    SpatialUpdateError, SpatialUpdateErrorCode,
};
pub use event::{DomainEvent, EventType};
pub use fake::{FakeCognition, fake_night_reply, fake_reply};
pub use floor::{ConversationFloor, floor_audio_failsafe_seconds, speech_reading_seconds};
pub use gesture::{DANCE_MAX_SECONDS, GESTURES, GestureKind, GestureSpec, GestureTarget};
pub use ids::{ActorId, InvalidId, ItemId, PartyId, PlaceId, RequestId, SpeechEventId};
pub use inventory::{
    CompletedTransform, InventoryError, InventoryErrorCode, ItemMatcher, LegacyRestockShare,
    MarketRequestLine, ReservedInput, SaleReceipt, SaleReceiptLine, StockSpec, TransformJob,
    TransformReceipt, TransformReceiptLine,
};
pub use item::{
    CONDITION_METADATA_KEY, CONDITION_POOPSTAINED, CONDITION_WET, DISPLAY_METADATA_KEY, Edible,
    InvalidKind, Item, ItemCatalog, ItemCatalogError, ItemKind, ItemKindDef, ItemSize, POOP_KIND,
};
pub use lore::{
    CONTROLLED_CIRCUMSTANCES, LoreCast, LoreCharacterSheet, LoreError, LoreProfile,
    LoreSpawnLocation, NO_FIXED_TRADE_FOLDER, PlanningWard, Significance,
};
pub use math::{Vec3, vec3, vec3_from_json, vec3_to_json};
pub use nav::{
    Door, Edge, NavData, NavError, NavGrid, Place, Route, Site, WALK_Y, door_edges_from_json,
};
pub use night::{
    NightGate, NightOffice, NightOfficeConfig, WARD_EDITS_MAX, WARD_MOOD_MAX_CHARS, stage_occupied,
};
pub use notices::{LAW_OCCUPATIONS, NOTICE_LIFE_GAME_DAYS, Notices, WardNotice};
pub use offer::Offer;
pub use perception::{cap_first, emit_sound, identify, sees};
pub use places::{PlaceEntry, PlaceError, PlaceRegistry};
pub use prompt::{
    ParsedAction, PromptEnv, PromptStrings, parse_reply, parse_reply_value, py_round,
    render_night_prompt, render_prompt, render_prompt_and_drain, render_sheet_value,
    render_ward_prompt,
};
pub use round::{
    Arrival, CartLoadKind, Census, ClosedMarketVisit, CounterBindingKey, CounterSession,
    ErrandDebug, HouseholdSettlementReceipt, MarketErrand, MarketErrandPhase, MarketVisitEnd,
    PartyPhase, PartyState, Phase as RoundPhase, RoadCart, Round, WaterSource,
};
pub use scheduler::{
    NpcScheduler, SchedulerEvent, background_turn_order, llm_turn_order, stage_turn_order,
};
pub use seed::{ItemSeed, PlayerKnowledge, SeedError, WorldConfig, WorldSeed, build_world};
pub use snapshot::{ActorSnapshot, ItemSnapshot, OfferSnapshot, PublicSnapshot};
pub use sounds::{AmbientSound, Sound, SoundCatalog, SoundCatalogError};
pub use speech_router::{DegradeReason, SpeechContext, SpeechRouter, has_unsupported_characters};
pub use status::{StatusEvent, Subsystem};
pub use traits::{
    Cognition, CognitionBusy, CognitionError, Completion, NullSight, NullTranscription, NullTts,
    PovFrame, RealtimeResult, Sight, SpeechError, SttBackendKind, SttSubmitError, Transcription,
    TranscriptionJobId, TranscriptionOutcome, Tts, TtsBackendKind, TtsOutcome, TtsRequest,
    TtsSubmitError,
};
pub use weather::{
    LightningStrike, PrecipitationKind, Shelter, ShelterAccess, ShelterCover, ShelterError,
    ShelterMap, WeatherClimate, WeatherConfig, WeatherKind, WeatherMode, WeatherSample,
    WeatherTimeline,
};
pub use world::{SpatialActorUpdate, World};

/// say / offer / accept / decline / eat notification radius, and the radius
/// within which a retract notifies its displaced target.
pub const HEARING_RADIUS_M: f64 = 20.0;
/// Proximity required for a targeted offer, an accept, and a decline.
pub const ITEM_INTERACTION_RADIUS_M: f64 = 4.0;
/// How far a targeted offer's two parties may drift before the promise lapses
/// on its own ([`actions::lapse_distant_offers`]). The same 20 m that bounds
/// every percept, and for the same reason: past it neither can answer the offer
/// — an accept and a decline both need 4 m — and neither hears what the other
/// does, so the held-out hand is a promise nobody can see or refuse. Inclusive:
/// only a strictly greater distance lapses it, matching the sheet's
/// `offered_to_you` boundary.
pub const OFFER_LAPSE_RADIUS_M: f64 = HEARING_RADIUS_M;
/// Maximum `say` text length — applies to every actor despite the name.
pub const PLAYER_SPEECH_MAX_CHARS: usize = 500;
/// Maximum `set_goal` text length.
pub const GOAL_MAX_CHARS: usize = 1_000;
/// Maximum `remember` / `forget` text length.
pub const MEMORY_MAX_CHARS: usize = 2_000;
/// Rolling per-character percept window.
pub const RECENT_HISTORY_MAX_ENTRIES: usize = 32;
/// Upper bound on a character's unread `inbox` (and its `pending_history`). A
/// scheduled actor drains its inbox every turn, so this only ever bites an actor
/// the stage gate never prompts — whose oldest unread percepts are stale anyway.
/// Without it, an ambient NPC near steady activity accumulates prose forever
/// (`character.rs`; `features/implemented/movement/05_the_llm_seam.md` §5.3). Generous
/// against `RECENT_HISTORY_MAX_ENTRIES` so a normal turn never loses a percept.
pub const INBOX_MAX_ENTRIES: usize = 64;
/// The fixed movement slice: NPC positions advance in whole 20 Hz steps, so a
/// stutter in the host's frame time can never change how far anyone walks — the
/// sim is the authoritative mover and the host only interpolates between the
/// samples it publishes (`features/implemented/movement/06_engineering.md`).
pub const MOVEMENT_TICK_SECONDS: f64 = 0.05;
/// A brisk medieval walking pace (`features/implemented/movement/01_the_clock.md` §6).
pub const WALK_SPEED_MPS: f64 = 1.8;
/// Below this an actor counts as "settled" for the novelty gate: a man crossing
/// the square is not news at every step, but the moment he stops (speed → 0) his
/// arrival is (`features/implemented/movement/05_the_llm_seam.md` §5.1).
pub const SETTLED_SPEED_MPS: f64 = 0.15;

// --- The water round (M3): the dynamic thirst need and the behaviour ladder
// (`features/implemented/movement/03_the_ladder.md`). Every gauge runs `0..=THIRST_MAX`,
// high = satisfied.
/// Full satisfaction, and the seed cap for a need gauge.
pub const THIRST_MAX: f64 = 255.0;
/// Rung 2 — "parched": drop everything and go to the nearest ward well now.
pub const THIRST_PARCHED: f64 = 38.0;
/// Rung 6 — "thirsty": go to the well, but only if its queue is short.
pub const THIRST_THIRSTY: f64 = 178.0;
/// Thirst lost per game-second. ~255 over four game hours — the fastest gauge,
/// so a servant's day is punctuated by trips to the curb. Read against game
/// time (the clock), never wall-clock, so the debug time-scale speeds it up too.
pub const THIRST_DECAY_PER_GAME_SECOND: f64 = THIRST_MAX / (4.0 * 3600.0);
/// How long one draw takes at the curb, in real seconds — long enough to read as
/// an act, short enough that a queue keeps moving.
pub const WATER_DRAW_SECONDS: f64 = 3.0;
/// While a source is busy, its keeper works the gear this often (real seconds),
/// so the windlass is heard as a steady rhythm rather than one clank per drawer.
pub const WELL_KEEPER_SOUND_INTERVAL_SECONDS: f64 = 4.0;
/// The behaviour ladder re-evaluates an idle actor on a jittered cadence in this
/// range (real seconds), staggered across the cast by a per-actor hash so no
/// scheduler is needed (`features/implemented/movement/03_the_ladder.md` §5).
pub const LADDER_DECISION_MIN_SECONDS: f64 = 1.0;
pub const LADDER_DECISION_MAX_SECONDS: f64 = 6.0;
/// A mover is "at the well" once within this of the source's draw point.
pub const WELL_ARRIVE_RADIUS_M: f64 = 3.0;
/// A queue longer than this turns the non-urgent rung 6 away (rung 2 still goes).
pub const WELL_QUEUE_SHORT: usize = 4;

// --- Hunger (food & items M2): the second dynamic need, thirst's slower twin
// (`features/food_and_items/03_hunger.md`). Same `0..=HUNGER_MAX` gauge, high =
// satisfied; unlike thirst, it decays for *every* enrolled townsperson — everyone
// eats (README §8.1). Surfaced to the LLM as a computed `hungry`/`famished`
// condition on the sheet, never as a raw number.
/// Full satisfaction, and the seed cap for the hunger gauge.
pub const HUNGER_MAX: f64 = 255.0;
/// Rung 3 — "famished": drop everything, eat what you hold or go to the hearth
/// now. The ladder spec's reserved number (`features/implemented/movement/03_the_ladder.md`).
pub const HUNGER_FAMISHED: f64 = 15.0;
/// Rung 7 — "hungry": seek food when convenient (a short queue at an open,
/// affordable stall — M3). The ladder spec's other reserved number.
pub const HUNGER_HUNGRY: f64 = 70.0;
/// Hunger lost per game-second. ~255 over ten game hours — deliberately the
/// slower, heavier need (thirst crosses its whole gauge in four). A full meal at
/// noon carries someone to supper: two real meals a day, which is the lore's day.
/// Read against game time, so the debug time-scale speeds it up identically.
pub const HUNGER_DECAY_PER_GAME_SECOND: f64 = HUNGER_MAX / (10.0 * 3600.0);
/// Hunger regained per game-second while sitting at the hearth (home or tavern)
/// during a meal office — full in ~20 game minutes, no items, no coins (the
/// honest cheat of `03_hunger.md` §4, symmetric with the magic restock).
pub const HEARTH_REFILL_PER_GAME_SECOND: f64 = HUNGER_MAX / (20.0 * 60.0);
/// Seeded hunger floors here so nobody spawns mid-crisis; the spread above it is
/// `HUNGER_MAX * hash01("hunger_seed", id, 0)` (`03_hunger.md` §1).
pub const HUNGER_SEED_FLOOR: f64 = 40.0;
/// The seed for an actor whose lore sheet already declares the `hungry`
/// condition — Ilse, hungry now and famished within the hour, her story made
/// mechanical (`03_hunger.md` §1/§6).
pub const HUNGER_SEED_DECLARED_HUNGRY: f64 = 25.0;
/// A food stall's queue is "short" below this — the [`WELL_QUEUE_SHORT`] twin,
/// the predicate rung 7 joins on (M3 binds the stalls this reads against).
pub const FOOD_QUEUE_SHORT: usize = 4;
// --- Body pockets and the poop clock (`features/extra_pockets.md`).
/// How many stack-units one cavity carries. Two, per the resolved open
/// question: bread inserted beside a stool becomes `[poop, poopstained bread]`,
/// and a third thing simply does not fit (`slot_full`).
pub const POCKET_SLOT_CAPACITY: usize = 2;
/// The gut's floor, in game-days: nothing lands sooner than three game hours
/// after the meal (M3 — "the poop clock").
pub const GUT_MIN_GAME_DAYS: f64 = 3.0 / 24.0;
/// The deterministic per-meal spread above the floor: up to three further game
/// hours, from `hash01("gut_clock", actor, meals)` — no RNG, so headless runs
/// replay identically.
pub const GUT_SPREAD_GAME_DAYS: f64 = 3.0 / 24.0;
/// How long after formation the `urgency` carriage status takes to ramp from 0
/// to 1, in game-days (two game hours). Expel to clear.
pub const URGENCY_RAMP_GAME_DAYS: f64 = 2.0 / 24.0;
/// Arm's length: how close a witness must stand to see *what* somebody pocketed
/// or retrieved, and *where* they put it. Beyond it the act is only a motion —
/// concealment survives across a square, never at two paces, which is what
/// keeps "swallow the evidence" a matter of patience rather than nerve.
pub const POCKET_PLAIN_SIGHT_RADIUS_M: f64 = ITEM_INTERACTION_RADIUS_M;

/// How long one sale takes at the pitch, in real seconds — a coin counted, a
/// loaf wrapped ([`04_the_bread_round.md`] §5). The [`WATER_DRAW_SECONDS`] twin,
/// a touch longer: money changes hands.
pub const PURCHASE_SECONDS: f64 = 4.0;
/// How long a ladder-driven meal at the pitch takes, in real seconds — the
/// buyer stands and eats where they bought, so the act reads visually (an eater
/// at a bench). Satiety applies at the end, on the actual eat.
pub const EAT_SECONDS: f64 = 5.0;
/// A buyer is "at the pitch" — arrived, ready to queue — once within this of it.
/// The [`WELL_ARRIVE_RADIUS_M`] twin.
pub const STALL_ARRIVE_RADIUS_M: f64 = 3.0;
/// A bound vendor counts as *keeping* their pitch — the stall is staffed — while
/// within this of it. Generous (a wide market floor, the vendor's own wander
/// leash), so the seller the round delivers to the square is "at the stall"
/// without any pin: no new movement code, the stall borrows the delivered actor.
pub const STALL_PITCH_REACH_M: f64 = 12.0;
/// A famished (rung 3) or hungry (rung 7) actor will detour to an open stall no
/// farther than this — the visible economy concentrates in the squares, and a
/// famished mason across the city keeps working (or goes to a hearth) rather
/// than marching a kilometre for a loaf (`03_hunger.md` §3/§4).
pub const STALL_SEEK_RADIUS_M: f64 = 60.0;
/// How often a staffed, open stall cries its wares — a player-only `market_cry`
/// world sound, so a square *sounds* like a market before it looks like one
/// (`04` §5, the optional flourish). Sparse enough not to nag, like the well
/// keeper's rhythm but slower.
pub const MARKET_CRY_INTERVAL_SECONDS: f64 = 75.0;

// --- Wallets (food & items M1/M2): starting-purse seeding
// (`features/food_and_items/02_the_spark_standard.md` §4). M1 pins these
// constants; M2's round seeding consumes them (this milestone decides the
// numbers only — no seeding is wired here). Every enrolled townsperson gets
// `WALLET_SEED_MIN + floor(WALLET_SEED_SPREAD * hash01(WALLET_SEED_SALT, id,
// 0))` sparks (2..=7), the deterministic-hash idiom the water round already uses
// for thirst spread (`round.rs`). Majors with authored holds keep them — Ilse
// keeps exactly 1, her reluctance to spend it being her character. Wallets are
// ordinary `spark` stacks in `World.items`; M5 household settlement later
// protects each resident's day-zero spendable amount as their working reserve.
/// Floor of a seeded starting wallet, in sparks.
pub const WALLET_SEED_MIN: u32 = 2;
/// Width of the seeded-wallet spread above the floor: the hash draws an integer
/// `0..WALLET_SEED_SPREAD`, so a wallet is
/// `WALLET_SEED_MIN ..= WALLET_SEED_MIN + WALLET_SEED_SPREAD - 1` (2..=7).
pub const WALLET_SEED_SPREAD: u32 = 6;
/// The deterministic-hash salt for wallet seeding, paired with the actor id
/// (mirrors `"water_thirst_seed"`).
pub const WALLET_SEED_SALT: &str = "wallet";

// --- `go_to` (M5): the LLM-issued travel intent
// (`features/implemented/movement/05_the_llm_seam.md` §2). A suggestion layered on an
// already-autonomous body: it expires, needs preempt it, and lapsing is a
// percept.
/// An intent lives for this multiple of its route's expected travel time in
/// **real** seconds — never a flat span of game time: the clock is compressed
/// while walking is real-time, so "ten game minutes" would strand every
/// cross-city errand in its first street.
pub const GO_TO_BUDGET_FACTOR: f64 = 2.5;
/// The floor under the budget, so a doorstep trip is not strangled by a
/// seconds-long route.
pub const GO_TO_MIN_BUDGET_SECONDS: f64 = 30.0;
/// A place target counts as reached within this of its nav node — "standing
/// inside the area", by the same yardstick as the round's posts.
pub const PLACE_ARRIVE_RADIUS_M: f64 = 6.0;
/// A person target is reached at conversation distance — comfortably inside
/// the 4 m exchange radius, so a `go_to` then an `offer_item` next turn just
/// works.
pub const PERSON_ARRIVE_RADIUS_M: f64 = 2.0;
/// Naming a walk by its endpoint: the nearest registered place within this
/// counts as "where the walk is going". Shared by the sheet's `you_are` walk
/// line and the developer overlay, so the actor and the debugger read the
/// same destination.
pub const WALK_DESTINATION_SNAP_M: f64 = 15.0;

// --- Crowds (M7): lane offsets, on-stage avoidance, the Needle's claim
// (`features/implemented/movement/02_navigation.md` §5, `07_milestones.md` M7).
/// The keep-right centre of the lane band: a walker's stable lateral offset is
/// this fraction of the usable corridor half-width, to their right.
pub const LANE_KEEP_RIGHT_FRACTION: f64 = 0.4;
/// The per-actor jitter around the keep-right centre (±), hashed off the actor
/// id — never an RNG — so the band spreads without ever crossing the crown.
pub const LANE_JITTER_FRACTION: f64 = 0.3;
/// Separation steering considers a neighbour once inside this bubble.
pub const AVOID_PERSONAL_RADIUS_M: f64 = 1.2;
/// The strongest sideways push separation may add, in m/s — under walking
/// speed, so avoidance bends a path and never launches anyone.
pub const AVOID_PUSH_MPS: f64 = 0.9;
/// At most this many neighbours contribute to one mover's separation push
/// (`features/implemented/movement/02_navigation.md` §5: "the ≤ 6 neighbours").
pub const AVOID_MAX_NEIGHBOURS: usize = 6;
/// The one-person choke around the Needle's node: a claim circle, entered only
/// with the claim (or behind its holder). Covers the crossing and the mouths of
/// its four narrow edges.
pub const NEEDLE_CLAIM_RADIUS_M: f64 = 14.0;
/// Standing at an occupied choke's mouth this long sends the mover the long way
/// round instead ("you wait, or you take Cinder Row").
pub const NEEDLE_REROUTE_SECONDS: f64 = 18.0;

/// Rung 11 — the social pull reaches a known, settled neighbour within this.
pub const SOCIAL_PULL_RADIUS_M: f64 = 8.0;
/// Rung 12 — an idle actor wanders no further than this from its home/post.
pub const WANDER_LEASH_M: f64 = 6.0;
/// Bounds the per-poll movement catch-up. A huge `now` jump — a resume from a
/// long pause, or a frame hitch — must not spin through hundreds of slices in
/// one poll and amplify the very stall it is recovering from: past this many,
/// the movement clock snaps forward to `now` and the owed backlog is dropped
/// rather than walked. Eight slices is 0.4 s of catch-up — far more than a
/// 60 Hz frame ever accrues, so it bites only a real stall, where a mover
/// skipping a step is invisible.
pub const MAX_MOVEMENT_CATCHUP_SLICES: usize = 8;
/// Bounds the per-poll bell catch-up, the same situation as
/// [`MAX_MOVEMENT_CATCHUP_SLICES`]: a huge `now` jump must not ring every
/// office it skipped as one wall of bells — past this many crossings, only the
/// most recent ones ring and the older backlog is dropped rather than rung.
pub const MAX_BELL_CATCHUP_OFFICES: usize = 2;
/// Total horizontal FOV for the sound witness test; overridable per run.
pub const DEFAULT_VIEW_CONE_DEGREES: f64 = 135.0;
/// An override narrower than this sees nothing at all (`server.py:449-451`).
pub const MIN_VIEW_CONE_DEGREES: f64 = 1.0;
/// A full circle: the widest cone that still means anything.
pub const MAX_VIEW_CONE_DEGREES: f64 = 360.0;
/// Longest LLM reply the scheduler will accept (Unicode scalar values).
pub const MAX_LLM_REPLY_CHARS: usize = 100_000;
/// Longest accepted actor/item id (Unicode scalar values).
pub const MAX_ID_CHARS: usize = 128;
/// The sentinel a character's `goal` carries when they have none.
pub const GOAL_NONE: &str = "None";

// Conversation-floor constants (server.py:79-84); the floor itself is in
// [`floor`].
pub const FLOOR_POST_UTTERANCE_BEAT_SECONDS: f64 = 0.4;
pub const FLOOR_AUDIO_FAILSAFE_MAX_SECONDS: f64 = 45.0;
pub const MAX_FLOOR_AWAITING: usize = 32;
pub const FLOOR_PLAYER_CHUNK_HOLD_SECONDS: f64 = 1.7;
pub const FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS: f64 = 3.0;
pub const FLOOR_PLAYER_TRANSCRIBING_HOLD_SECONDS: f64 = 8.0;

// Microphone-stream constants (server.py:74-78); the router is in
// [`speech_router`].
/// Concurrent streamed utterances; the oldest is evicted past this.
pub const MAX_ACTIVE_STREAMS: usize = 8;
/// Per-utterance chunk cap, enforced in the mic worker *and* here (parity).
pub const STT_STREAM_MAX_CHUNKS: u32 = 256;
/// Python capped a chunk at 32 000 base64 characters, which decode to at most
/// 24 000 bytes. The wire now carries typed `i16` samples, so the same bound
/// reads as 12 000 of them.
pub const STT_STREAM_MAX_CHUNK_SAMPLES: usize = 12_000;
/// How long a completed streamed transcript waits for its `player_recording`
/// before it is dropped — a held transcript must never become a late say.
pub const STT_STREAM_HELD_TRANSCRIPT_SECONDS: f64 = 5.0;
/// Latency-probe LRU cap.
pub const MAX_UTTERANCE_TIMINGS: usize = 64;
/// The only sample rate the realtime session speaks; anything else degrades the
/// stream to the batch fallback.
pub const STREAM_SAMPLE_RATE: u32 = 24_000;
