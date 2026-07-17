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
pub mod areas;
pub mod attention;
pub mod character;
pub mod clock;
pub mod engine;
pub mod error;
pub mod event;
pub mod fake;
pub mod floor;
mod homes;
pub mod ids;
pub mod item;
pub mod lore;
pub mod math;
pub mod nav;
pub mod offer;
pub mod perception;
pub mod places;
pub mod prompt;
pub mod round;
/// CPython text semantics (`str.strip`, `float.__repr__`, `repr`) — internal,
/// but shared by the action validators, the reply parser and the scheduler.
mod pyfmt;
pub mod scheduler;
pub mod seed;
pub mod snapshot;
pub mod sounds;
pub mod speech_router;
pub mod status;
pub mod traits;
pub mod world;

pub use actions::{apply_action, apply_action_at};
pub use areas::{
    Area, AreaBox, AreaMap, AreaMapError, AxisDirection, CoordinateSystem, NearestArea,
};
pub use attention::{
    CURIOSITY_BASE, CURIOSITY_WITHOUT_LORE, CuriosityConfig, DEFAULT_STAGE_MAX_ACTORS,
    DEFAULT_STAGE_RADIUS_M, IdleCognitionMode, IdleGate, NOVELTY_MEMORY_SECONDS, Novelty,
    STAGE_PARTNER_MEMORY_SECONDS, StageConfig, WarmExchanges, context_hash, curiosity_of, on_stage,
};
pub use character::{
    Character, CharacterSheet, CharacterState, Control, IntentTarget, Movement, Needs, Patrol,
    TravelIntent,
};
pub use clock::{
    BELL_STROKE_INTERVAL_SECONDS, Office, Weekday, WorldClock, WorldTime, stroke_times,
};
pub use engine::{
    ActorMotion, Capabilities, Engine, EngineCommand, EngineConfig, EngineMessage,
};
pub use error::{
    ActionError, ActionErrorCode, CommandError, CommandErrorCode, EngineInitError, PromptError,
    SpatialUpdateError, SpatialUpdateErrorCode,
};
pub use event::{DomainEvent, EventType};
pub use fake::{FakeCognition, fake_reply};
pub use floor::{ConversationFloor, floor_audio_failsafe_seconds, speech_reading_seconds};
pub use ids::{ActorId, InvalidId, ItemId, PlaceId, RequestId, SpeechEventId};
pub use item::Item;
pub use lore::{
    CONTROLLED_CIRCUMSTANCES, LoreCast, LoreCharacterSheet, LoreError, LoreProfile,
    LoreSpawnLocation, NO_FIXED_TRADE_FOLDER, PlanningWard, Significance,
};
pub use math::{Vec3, vec3, vec3_from_json, vec3_to_json};
pub use nav::{
    Door, Edge, NavData, NavError, NavGrid, Place, Route, Site, WALK_Y, door_edges_from_json,
};
pub use offer::Offer;
pub use perception::{cap_first, emit_sound, identify, sees};
pub use places::{PlaceEntry, PlaceError, PlaceRegistry};
pub use round::{Arrival, Census, ErrandDebug, Phase as RoundPhase, Round, WaterSource};
pub use prompt::{
    ParsedAction, PromptEnv, PromptStrings, parse_reply, parse_reply_value, py_round,
    render_prompt, render_prompt_and_drain, render_sheet_value,
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
pub use world::{SpatialActorUpdate, World};

/// say / offer / accept / decline / eat notification radius, and the radius
/// within which a retract notifies its displaced target.
pub const HEARING_RADIUS_M: f64 = 20.0;
/// Proximity required for a targeted offer, an accept, and a decline.
pub const ITEM_INTERACTION_RADIUS_M: f64 = 4.0;
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
/// (`character.rs`; `features/movement/05_the_llm_seam.md` §5.3). Generous
/// against `RECENT_HISTORY_MAX_ENTRIES` so a normal turn never loses a percept.
pub const INBOX_MAX_ENTRIES: usize = 64;
/// The fixed movement slice: NPC positions advance in whole 20 Hz steps, so a
/// stutter in the host's frame time can never change how far anyone walks — the
/// sim is the authoritative mover and the host only interpolates between the
/// samples it publishes (`features/movement/06_engineering.md`).
pub const MOVEMENT_TICK_SECONDS: f64 = 0.05;
/// A brisk medieval walking pace (`features/movement/01_the_clock.md` §6).
pub const WALK_SPEED_MPS: f64 = 1.8;
/// Below this an actor counts as "settled" for the novelty gate: a man crossing
/// the square is not news at every step, but the moment he stops (speed → 0) his
/// arrival is (`features/movement/05_the_llm_seam.md` §5.1).
pub const SETTLED_SPEED_MPS: f64 = 0.15;

// --- The water round (M3): the dynamic thirst need and the behaviour ladder
// (`features/movement/03_the_ladder.md`). Every gauge runs `0..=THIRST_MAX`,
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
/// scheduler is needed (`features/movement/03_the_ladder.md` §5).
pub const LADDER_DECISION_MIN_SECONDS: f64 = 1.0;
pub const LADDER_DECISION_MAX_SECONDS: f64 = 6.0;
/// A mover is "at the well" once within this of the source's draw point.
pub const WELL_ARRIVE_RADIUS_M: f64 = 3.0;
/// A queue longer than this turns the non-urgent rung 6 away (rung 2 still goes).
pub const WELL_QUEUE_SHORT: usize = 4;
// --- `go_to` (M5): the LLM-issued travel intent
// (`features/movement/05_the_llm_seam.md` §2). A suggestion layered on an
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

// --- Crowds (M7): lane offsets, on-stage avoidance, the Needle's claim
// (`features/movement/02_navigation.md` §5, `07_milestones.md` M7).
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
/// (`features/movement/02_navigation.md` §5: "the ≤ 6 neighbours").
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
/// long pause — must not spin through thousands of slices: past this many, the
/// movement clock snaps forward and the backlog is dropped rather than walked.
pub const MAX_MOVEMENT_CATCHUP_SLICES: usize = 64;
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
