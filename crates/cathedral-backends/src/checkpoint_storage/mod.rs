//! Bounded, world-independent durable slot IO for Linux ext-family filesystems
//! (validated on ext4). Other filesystem types are explicitly refused.
//!
//! Construct over an existing trusted private directory off the host frame.
//! One pinned directory FD and an exclusive advisory flock serialize cooperating
//! processes. Network filesystems and concurrent external edits are unsupported.
//! Files are opened relative to that FD without following symlinks. No engine,
//! provider runtime, device or ECS owner crosses this service boundary.
//!
//! A load returns admitted bytes with checked framing and integrity, NOT a
//! validated world: complete M2 validation and M3b adoption remain mandatory.
mod disk;
mod service;
pub use service::{CheckpointStorage, Shutdown, SubmitError};

use cathedral_sim::checkpoint::complete::CompleteCheckpointInput;
use serde::{Deserialize, Serialize};

pub const OPERATION_CAPACITY: usize = 8;
pub const SERVICE_ALLOWANCE_BYTES: usize = 3 * 1024 * 1024;

/// Safe filename component. Slot identity is never a path supplied by a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct SlotId(String);
impl SlotId {
    pub fn new(value: &str) -> Result<Self, StorageError> {
        if value.is_empty()
            || value.len() > 48
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(StorageError::invalid("invalid slot identity"));
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SlotId {
    type Error = StorageError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

/// Caller-supplied player-known preview only. Never derive this from NPC plans.
/// The simulation boundary and lineage are taken from the actual saved owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MetadataInput")]
pub struct SaveMetadata {
    title: String,
    captured_unix_seconds: i64,
    known_location: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataInput {
    title: String,
    captured_unix_seconds: i64,
    known_location: String,
}
impl TryFrom<MetadataInput> for SaveMetadata {
    type Error = StorageError;
    fn try_from(value: MetadataInput) -> Result<Self, Self::Error> {
        Self::check(
            &value.title,
            value.captured_unix_seconds,
            &value.known_location,
        )?;
        Ok(Self {
            title: value.title,
            captured_unix_seconds: value.captured_unix_seconds,
            known_location: value.known_location,
        })
    }
}
impl SaveMetadata {
    pub fn new(
        title: &str,
        captured_unix_seconds: i64,
        known_location: &str,
    ) -> Result<Self, StorageError> {
        Self::check(title, captured_unix_seconds, known_location)?;
        let value = Self {
            title: title.into(),
            captured_unix_seconds,
            known_location: known_location.into(),
        };
        Ok(value)
    }
    fn validate(&self) -> Result<(), StorageError> {
        Self::check(
            &self.title,
            self.captured_unix_seconds,
            &self.known_location,
        )
    }
    fn check(
        title: &str,
        captured_unix_seconds: i64,
        known_location: &str,
    ) -> Result<(), StorageError> {
        let safe = |s: &str, max| s.len() <= max && !s.chars().any(char::is_control);
        if title.is_empty()
            || !safe(title, 96)
            || !safe(known_location, 160)
            || !(-62_135_596_800..=253_402_300_799).contains(&captured_unix_seconds)
        {
            return Err(StorageError::invalid(
                "invalid bounded player-known metadata",
            ));
        }
        Ok(())
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn captured_unix_seconds(&self) -> i64 {
        self.captured_unix_seconds
    }
    pub fn known_location(&self) -> &str {
        &self.known_location
    }
}

/// Random service identity plus a checked monotonically increasing sequence.
/// Unrelated world loads do not change this identity or authorize reassignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationId {
    service: [u8; 16],
    sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotReference {
    pub(crate) format_version: u16,
    pub(crate) slot: SlotId,
    pub(crate) generation: OperationId,
    pub(crate) payload_bytes: u64,
    pub(crate) payload_sha256: [u8; 32],
    pub(crate) world_identity: [u8; 16],
    pub(crate) boundary: f64,
    pub(crate) metadata: SaveMetadata,
}
impl SlotReference {
    pub fn format_version(&self) -> u16 {
        self.format_version
    }
    pub fn generation(&self) -> OperationId {
        self.generation
    }
    pub fn metadata(&self) -> &SaveMetadata {
        &self.metadata
    }
    pub fn boundary_seconds(&self) -> f64 {
        self.boundary
    }
    pub fn world_identity(&self) -> [u8; 16] {
        self.world_identity
    }
    pub fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    pub fn payload_sha256(&self) -> [u8; 32] {
        self.payload_sha256
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSelection {
    /// Refuses interrupted publication. A freshly reopened reference proves
    /// validated publication, not that a prior UI received its acknowledgement.
    Active,
    /// The acknowledged predecessor, including the journal's original active
    /// reference when interruption happened before recovery-reference rotation.
    Previous,
}

#[derive(Debug)]
pub struct LoadedCheckpoint {
    pub reference: SlotReference,
    pub input: CompleteCheckpointInput,
}
#[derive(Debug)]
pub enum Outcome {
    Saved(SlotReference),
    Loaded(LoadedCheckpoint),
    /// Explicitly selected acknowledged predecessor is restored durably.
    Recovered(Option<SlotReference>),
    Cancelled,
    Failed(StorageError),
}

/// Stable phase attribution; diagnostics never copy payload text or paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Open,
    Worker,
    Inspect,
    Admission,
    Read,
    Envelope,
    JournalWrite,
    JournalFlush,
    JournalReplace,
    JournalSync,
    PayloadWrite,
    PayloadValidate,
    PayloadFlush,
    PayloadPublish,
    PayloadSync,
    RecoveryWrite,
    RecoveryFlush,
    RecoveryReplace,
    RecoverySync,
    ActiveWrite,
    ActiveFlush,
    ActiveReplace,
    ActiveSync,
    PendingRemove,
    PendingSync,
    Cleanup,
    CleanupSync,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    pub phase: Phase,
    pub kind: std::io::ErrorKind,
    pub message: &'static str,
}
impl StorageError {
    fn admission(message: &'static str) -> Self {
        Self {
            phase: Phase::Admission,
            kind: std::io::ErrorKind::WouldBlock,
            message,
        }
    }
    fn invalid(message: &'static str) -> Self {
        Self {
            phase: Phase::Inspect,
            kind: std::io::ErrorKind::InvalidData,
            message,
        }
    }
    fn io(phase: Phase, error: std::io::Error) -> Self {
        Self {
            phase,
            kind: error.kind(),
            message: "checkpoint filesystem operation failed",
        }
    }
    fn at(mut self, phase: Phase) -> Self {
        self.phase = phase;
        self
    }
}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {} ({:?})", self.phase, self.message, self.kind)
    }
}
impl std::error::Error for StorageError {}

#[cfg(test)]
mod review_tests;
#[cfg(test)]
mod tests;
