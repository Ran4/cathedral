//! Private checkpoint components, not a complete save or an Engine loader.
//!
//! V1 owner DTOs keep authority separate from PublicSnapshot. The component cuts
//! cover time/compatibility/admission, command replay, fixture operations and the
//! private character/inventory/World reference backbone.
//! Other city owners and the complete envelope must be added before a host can
//! capture or adopt a save. No API here polls, seeds, performs IO or submits work.

pub(crate) mod aggregate;
mod budget;
mod manifest;
pub(crate) mod records;
pub(crate) mod serde_support;
#[cfg(test)]
mod tests;
mod time;
mod wire;

pub use aggregate::ComponentCost;
pub use budget::{Admitted, CheckpointBudget, Cohort, MAX_RESIDENT_BYTES, Reservation};
pub use manifest::{CompatibilityManifestV1, VersionedAlgorithmV1};
pub use time::{CalendarAnchorV1, HostTimeV1, LogicalAnchorV1};
pub(crate) use wire::{BoundedText, BoundedVec, encoded_len};

use serde::{Serialize, de::DeserializeOwned};

pub const SCHEMA_VERSION: u16 = 1;
pub const AUTHORED_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
pub const POPULATED_PAYLOAD_BYTES: usize = 128 * 1024 * 1024;
/// Supported save range, rather than accepting finite values whose calendar
/// arithmetic or substep precision is unusable. These limits are v1 behavior.
pub const MAX_LOGICAL_SECONDS: f64 = 1_000_000_000.0;
pub const MAX_CALENDAR_DAYS: f64 = 1_000_000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointError {
    pub owner: &'static str,
    pub reason: String,
}
impl CheckpointError {
    pub(crate) fn new(owner: &'static str, reason: impl Into<String>) -> Self {
        Self {
            owner,
            reason: reason.into(),
        }
    }
}
impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.owner, self.reason)
    }
}
impl std::error::Error for CheckpointError {}
pub type Result<T> = std::result::Result<T, CheckpointError>;

pub(crate) fn logical(owner: &'static str, value: f64) -> Result<()> {
    if value.is_finite() && (0.0..=MAX_LOGICAL_SECONDS).contains(&value) {
        Ok(())
    } else {
        Err(CheckpointError::new(owner, "logical time outside v1 range"))
    }
}
pub(crate) fn calendar(owner: &'static str, value: f64) -> Result<()> {
    if value.is_finite() && value.abs() <= MAX_CALENDAR_DAYS {
        Ok(())
    } else {
        Err(CheckpointError::new(
            owner,
            "calendar time outside v1 range",
        ))
    }
}

/// Owner DTOs only call this after reserving their full conservative extraction
/// or decode/index cost. The input buffer is part of that reservation, not free
/// memory. Borrowed bytes are never copied into a serde_json::Value first.
pub(crate) fn decode<T: DeserializeOwned>(
    bytes: &[u8],
    owner: &'static str,
    limit: usize,
    reservation: &Reservation,
    working_bytes: usize,
) -> Result<T> {
    if bytes.len() > limit {
        return Err(CheckpointError::new(
            owner,
            "encoded owner byte limit exceeded",
        ));
    }
    // serde_json expands escaped strings into a reusable scratch Vec before
    // the bounded string visitor sees them. Its geometric capacity is at most
    // twice this input length; include the original input too.
    reservation.require(
        Cohort::LoadCandidate,
        working_bytes.saturating_add(bytes.len().saturating_mul(3)),
    )?;
    // serde_json's recursion limit remains enabled. Fixed DTO shapes also
    // reject nested or unknown fields instead of accepting arbitrary JSON.
    serde_json::from_slice(bytes).map_err(|e| CheckpointError::new(owner, e.to_string()))
}

pub(crate) fn encode<T: Serialize>(
    dto: &T,
    owner: &'static str,
    limit: usize,
    reservation: &Reservation,
    working_bytes: usize,
) -> Result<Vec<u8>> {
    let bytes = encoded_len(dto, limit).map_err(|e| CheckpointError::new(owner, e))?;
    reservation.require(Cohort::SavePayload, working_bytes.saturating_add(bytes))?;
    let mut output = Vec::with_capacity(bytes);
    serde_json::to_writer(&mut output, dto)
        .map_err(|e| CheckpointError::new(owner, e.to_string()))?;
    Ok(output)
}
