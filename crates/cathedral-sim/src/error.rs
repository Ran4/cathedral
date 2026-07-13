//! The expected error types. Each carries a human message (shown to the LLM and
//! the player, so the wording is behavior) plus a machine `code`.
//!
//! The code strings are the exact Python ones — the game and the prompt layer
//! match on them.

use std::fmt;

use crate::ids::ActorId;

/// Action validation failures (`sim.py:29-35`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionErrorCode {
    InvalidAction,
    InvalidArguments,
    TextTooLong,
    UnknownActor,
    UnknownVerb,
    UnknownTarget,
    SelfTarget,
    OutOfRange,
    NotOwner,
    UnknownItem,
    NoOffer,
    OwnOffer,
    NotOfferTarget,
    StaleOffer,
    BroadcastCannotDecline,
    UnknownSound,
    SoundsDisabled,
}

impl ActionErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAction => "invalid_action",
            Self::InvalidArguments => "invalid_arguments",
            Self::TextTooLong => "text_too_long",
            Self::UnknownActor => "unknown_actor",
            Self::UnknownVerb => "unknown_verb",
            Self::UnknownTarget => "unknown_target",
            Self::SelfTarget => "self_target",
            Self::OutOfRange => "out_of_range",
            Self::NotOwner => "not_owner",
            Self::UnknownItem => "unknown_item",
            Self::NoOffer => "no_offer",
            Self::OwnOffer => "own_offer",
            Self::NotOfferTarget => "not_offer_target",
            Self::StaleOffer => "stale_offer",
            Self::BroadcastCannotDecline => "broadcast_cannot_decline",
            Self::UnknownSound => "unknown_sound",
            Self::SoundsDisabled => "sounds_disabled",
        }
    }
}

impl fmt::Display for ActionErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An expected, player/LLM-safe action validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionError {
    pub code: ActionErrorCode,
    pub message: String,
}

impl ActionError {
    pub fn new(code: ActionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ActionError {}

/// A prompt that cannot be built or rendered (`prompt.py:172-173`, plus the
/// template/strings loading the Python side did at import time).
///
/// There is no fallback: the prose lives in `assets/prompts/`, so a missing or
/// broken data file is a hard startup error rather than a silently embedded
/// English string (prompt.md §6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptError {
    pub message: String,
}

impl PromptError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PromptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PromptError {}

/// Bad or stale position updates (`sim.py:37-43`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialUpdateErrorCode {
    InvalidPosition,
    StaleSpatialSeq,
    UnknownActor,
}

impl SpatialUpdateErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidPosition => "invalid_position",
            Self::StaleSpatialSeq => "stale_spatial_seq",
            Self::UnknownActor => "unknown_actor",
        }
    }
}

impl fmt::Display for SpatialUpdateErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An invalid or stale position update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialUpdateError {
    pub code: SpatialUpdateErrorCode,
    pub message: String,
}

impl SpatialUpdateError {
    pub fn new(code: SpatialUpdateErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The default code (`invalid_position`), as in Python's default argument.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(SpatialUpdateErrorCode::InvalidPosition, message)
    }
}

impl fmt::Display for SpatialUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SpatialUpdateError {}

/// Why a player command failed (`server-core.md` §6.2).
///
/// The union of the action codes, the spatial codes and the engine's own — the
/// game's HUD and `interaction.rs` match on these exact strings, so a player
/// command that fails an *action* validation must report the very code the LLM
/// would have seen. The protocol-only codes (`not_ready`, `already_ready`,
/// `malformed_message`) die with the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandErrorCode {
    // ---- from `ActionErrorCode` (the sim rejected the action itself)
    InvalidAction,
    InvalidArguments,
    TextTooLong,
    UnknownActor,
    UnknownVerb,
    UnknownTarget,
    SelfTarget,
    OutOfRange,
    NotOwner,
    UnknownItem,
    NoOffer,
    OwnOffer,
    NotOfferTarget,
    StaleOffer,
    BroadcastCannotDecline,
    UnknownSound,
    SoundsDisabled,
    // ---- from `SpatialUpdateErrorCode` (the position that came with it)
    InvalidPosition,
    StaleSpatialSeq,
    // ---- engine level
    /// The command's own shape was wrong. Typed commands make this nearly
    /// unreachable; it survives for the boundary that decodes them.
    InvalidRequest,
    /// `debug_player_say` outside fake mode.
    Forbidden,
    /// A spatial update naming somebody other than the player.
    ForbiddenActor,
    InvalidTtsBackend,
    TtsUnavailable,
    InternalError,
    // ---- speech (P6 owns the paths that raise these)
    InvalidPath,
    MissingAudio,
    AudioInUse,
    InvalidTarget,
    InvalidSttBackend,
    SttUnavailable,
    Overloaded,
    TranscriptionFailed,
    EmptyTranscription,
    InvalidTranscription,
    UnknownAudio,
}

impl CommandErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAction => "invalid_action",
            Self::InvalidArguments => "invalid_arguments",
            Self::TextTooLong => "text_too_long",
            Self::UnknownActor => "unknown_actor",
            Self::UnknownVerb => "unknown_verb",
            Self::UnknownTarget => "unknown_target",
            Self::SelfTarget => "self_target",
            Self::OutOfRange => "out_of_range",
            Self::NotOwner => "not_owner",
            Self::UnknownItem => "unknown_item",
            Self::NoOffer => "no_offer",
            Self::OwnOffer => "own_offer",
            Self::NotOfferTarget => "not_offer_target",
            Self::StaleOffer => "stale_offer",
            Self::BroadcastCannotDecline => "broadcast_cannot_decline",
            Self::UnknownSound => "unknown_sound",
            Self::SoundsDisabled => "sounds_disabled",
            Self::InvalidPosition => "invalid_position",
            Self::StaleSpatialSeq => "stale_spatial_seq",
            Self::InvalidRequest => "invalid_request",
            Self::Forbidden => "forbidden",
            Self::ForbiddenActor => "forbidden_actor",
            Self::InvalidTtsBackend => "invalid_tts_backend",
            Self::TtsUnavailable => "tts_unavailable",
            Self::InternalError => "internal_error",
            Self::InvalidPath => "invalid_path",
            Self::MissingAudio => "missing_audio",
            Self::AudioInUse => "audio_in_use",
            Self::InvalidTarget => "invalid_target",
            Self::InvalidSttBackend => "invalid_stt_backend",
            Self::SttUnavailable => "stt_unavailable",
            Self::Overloaded => "overloaded",
            Self::TranscriptionFailed => "transcription_failed",
            Self::EmptyTranscription => "empty_transcription",
            Self::InvalidTranscription => "invalid_transcription",
            Self::UnknownAudio => "unknown_audio",
        }
    }
}

impl From<ActionErrorCode> for CommandErrorCode {
    fn from(code: ActionErrorCode) -> Self {
        match code {
            ActionErrorCode::InvalidAction => Self::InvalidAction,
            ActionErrorCode::InvalidArguments => Self::InvalidArguments,
            ActionErrorCode::TextTooLong => Self::TextTooLong,
            ActionErrorCode::UnknownActor => Self::UnknownActor,
            ActionErrorCode::UnknownVerb => Self::UnknownVerb,
            ActionErrorCode::UnknownTarget => Self::UnknownTarget,
            ActionErrorCode::SelfTarget => Self::SelfTarget,
            ActionErrorCode::OutOfRange => Self::OutOfRange,
            ActionErrorCode::NotOwner => Self::NotOwner,
            ActionErrorCode::UnknownItem => Self::UnknownItem,
            ActionErrorCode::NoOffer => Self::NoOffer,
            ActionErrorCode::OwnOffer => Self::OwnOffer,
            ActionErrorCode::NotOfferTarget => Self::NotOfferTarget,
            ActionErrorCode::StaleOffer => Self::StaleOffer,
            ActionErrorCode::BroadcastCannotDecline => Self::BroadcastCannotDecline,
            ActionErrorCode::UnknownSound => Self::UnknownSound,
            ActionErrorCode::SoundsDisabled => Self::SoundsDisabled,
        }
    }
}

impl From<SpatialUpdateErrorCode> for CommandErrorCode {
    fn from(code: SpatialUpdateErrorCode) -> Self {
        match code {
            SpatialUpdateErrorCode::InvalidPosition => Self::InvalidPosition,
            SpatialUpdateErrorCode::StaleSpatialSeq => Self::StaleSpatialSeq,
            SpatialUpdateErrorCode::UnknownActor => Self::UnknownActor,
        }
    }
}

impl fmt::Display for CommandErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failed player command, as the `command_result` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
}

impl CommandError {
    pub fn new(code: CommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

/// Python's `_safe_message` let only Action/Spatial/Protocol error texts reach
/// the player. In Rust every failure that gets this far is already one of ours,
/// so the conversion is total and the fallback string is unreachable.
impl From<ActionError> for CommandError {
    fn from(error: ActionError) -> Self {
        Self::new(error.code.into(), error.message)
    }
}

impl From<SpatialUpdateError> for CommandError {
    fn from(error: SpatialUpdateError) -> Self {
        Self::new(error.code.into(), error.message)
    }
}

/// A world the engine cannot run (`server.py:442-444`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineInitError {
    /// The seed has no character with the configured player id — nothing to
    /// project snapshots for, nobody to attribute player commands to.
    MissingPlayer(ActorId),
    /// The player's spawn position could not be applied.
    PlayerSpawn(SpatialUpdateError),
}

impl fmt::Display for EngineInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPlayer(actor_id) => {
                write!(f, "world has no stable player character '{actor_id}'")
            }
            Self::PlayerSpawn(error) => write!(f, "player spawn was rejected: {error}"),
        }
    }
}

impl std::error::Error for EngineInitError {}
