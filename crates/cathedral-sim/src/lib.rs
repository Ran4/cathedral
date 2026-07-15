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
pub mod ids;
pub mod item;
pub mod lore;
pub mod math;
pub mod offer;
pub mod perception;
pub mod prompt;
/// CPython text semantics (`str.strip`, `float.__repr__`, `repr`) — internal,
/// but shared by the action validators, the sheet renderer and the scheduler.
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
    STAGE_PARTNER_MEMORY_SECONDS, StageConfig, context_hash, curiosity_of, on_stage,
};
pub use character::{Character, CharacterSheet, CharacterState, Control};
pub use clock::{
    BELL_STROKE_INTERVAL_SECONDS, Office, Weekday, WorldClock, WorldTime, stroke_times,
};
pub use engine::{Capabilities, Engine, EngineCommand, EngineConfig, EngineMessage};
pub use error::{
    ActionError, ActionErrorCode, CommandError, CommandErrorCode, EngineInitError, PromptError,
    SpatialUpdateError, SpatialUpdateErrorCode,
};
pub use event::{DomainEvent, EventType};
pub use fake::{FakeCognition, fake_reply};
pub use floor::{ConversationFloor, floor_audio_failsafe_seconds, speech_reading_seconds};
pub use ids::{ActorId, InvalidId, ItemId, RequestId, SpeechEventId};
pub use item::Item;
pub use lore::{
    CONTROLLED_STATUSES, LoreCast, LoreCharacterSheet, LoreError, LoreProfile, LoreSpawnLocation,
    NO_FIXED_TRADE_FOLDER, PlanningWard, Significance,
};
pub use math::{Vec3, vec3, vec3_from_json, vec3_to_json};
pub use offer::Offer;
pub use perception::{cap_first, emit_sound, identify, sees};
pub use prompt::{
    ParsedAction, PromptEnv, PromptStrings, parse_reply, parse_reply_value, py_round,
    render_prompt, render_prompt_and_drain, to_py_json,
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
