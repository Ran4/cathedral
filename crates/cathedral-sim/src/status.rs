//! Subsystem health, as the HUD reads it (`server.py:2231-2247`, server-core
//! spec §11).
//!
//! The `subsystem`/`state` strings are wire format: the game's HUD matches on
//! them today and keeps doing so day one (D30), so a [`StatusEvent`] is a typed
//! carrier for the exact same words rather than a redesign. The scheduler emits
//! the `llm` rows; the speech backends emit the `stt`/`tts` ones.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::ActorId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subsystem {
    Llm,
    Stt,
    Tts,
}

impl Subsystem {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Stt => "stt",
            Self::Tts => "tts",
        }
    }
}

impl fmt::Display for Subsystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A turn was submitted; the provider is thinking.
pub const STATE_THINKING: &str = "thinking";
/// A turn was applied (or a backend went quiet); nothing is pending.
pub const STATE_IDLE: &str = "idle";
/// The subsystem still works, but worse — the message says how.
pub const STATE_DEGRADED: &str = "degraded";
/// The subsystem is not configured at all.
pub const STATE_UNAVAILABLE: &str = "unavailable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEvent {
    pub subsystem: Subsystem,
    /// One of the `STATE_*` constants — free-form because the speech backends
    /// add rows of their own (`synthesizing`, `loading`, `selected`, …).
    pub state: String,
    /// Present for actor-attributable rows only; a stale-result discard has no
    /// actor to blame (`scheduler.py:215-219`).
    pub actor_id: Option<ActorId>,
    pub message: Option<String>,
    /// The speech backend a row is about; scheduler rows never carry one
    /// (`scheduler.py:25-34`).
    pub backend: Option<String>,
}

impl StatusEvent {
    /// A scheduler row: subsystem `llm`, never a `backend`.
    pub fn llm(state: &str, actor_id: Option<ActorId>, message: Option<String>) -> Self {
        Self {
            subsystem: Subsystem::Llm,
            state: state.to_string(),
            actor_id,
            message,
            backend: None,
        }
    }
}
