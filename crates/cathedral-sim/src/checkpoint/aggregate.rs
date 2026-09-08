//! Allocation admission for nested city records. This lexer stores no strings,
//! keys, containers or decoded values. Typed decoding only starts after its
//! conservative aggregate charge fits the attached cohort reservation.
use super::{CheckpointError, Cohort, POPULATED_PAYLOAD_BYTES, Reservation, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;

pub const INITIAL_BYTES: usize = 4096;
pub const MAX_EXPANDED_BYTES: usize = 128 * 1024 * 1024;
const CONTAINER_BYTES: usize = 512;
const SCALAR_BYTES: usize = 64;
const MAX_DEPTH: usize = 64;

/// A deliberately conservative bound, not allocator/RSS measurement. Each JSON
/// object/array contributes 512 bytes, every scalar/string/key 64 bytes and
/// every encoded string byte two more. The supported record-layout tests bound
/// the concrete v1 record/Option/enum layouts, not arbitrary Deserialize types. Container/field
/// charges cover Vec growth and BTree nodes as well as short/empty values.
/// Four expanded copies cover DTO, candidate/indexes, and validation scratch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ComponentCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub peak_bytes: usize,
}
#[derive(Default)]
struct Meter {
    encoded: usize,
    expanded: usize,
    depth: usize,
    quoted: bool,
    escape: bool,
    bare: bool,
}
impl Meter {
    fn add(&mut self, bytes: usize) -> std::io::Result<()> {
        self.expanded = self
            .expanded
            .checked_add(bytes)
            .filter(|n| *n <= MAX_EXPANDED_BYTES)
            .ok_or_else(|| std::io::Error::other("aggregate expanded allocation limit exceeded"))?;
        Ok(())
    }
    fn cost(&self) -> ComponentCost {
        ComponentCost {
            encoded_bytes: self.encoded,
            expanded_upper_bytes: self.expanded,
            peak_bytes: INITIAL_BYTES + self.expanded * 4 + self.encoded * 3,
        }
    }
}
impl Write for Meter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.encoded = self
            .encoded
            .checked_add(bytes.len())
            .filter(|n| *n <= POPULATED_PAYLOAD_BYTES)
            .ok_or_else(|| std::io::Error::other("component encoded byte limit exceeded"))?;
        for b in bytes {
            if self.quoted {
                self.add(2)?;
                if self.escape {
                    self.escape = false;
                } else if *b == b'\\' {
                    self.escape = true;
                } else if *b == b'"' {
                    self.quoted = false;
                }
                continue;
            }
            match *b {
                b'"' => {
                    self.add(SCALAR_BYTES)?;
                    self.quoted = true;
                    self.bare = false;
                }
                b'[' | b'{' => {
                    self.add(CONTAINER_BYTES)?;
                    self.depth += 1;
                    self.bare = false;
                    if self.depth > MAX_DEPTH {
                        return Err(std::io::Error::other("component nesting limit exceeded"));
                    }
                }
                b']' | b'}' => {
                    self.depth = self.depth.saturating_sub(1);
                    self.bare = false;
                }
                b',' | b':' | b' ' | b'\t' | b'\r' | b'\n' => self.bare = false,
                _ => {
                    if !self.bare {
                        self.add(SCALAR_BYTES)?;
                        self.bare = true;
                    }
                }
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn measure<T: Serialize>(value: &T, owner: &'static str) -> Result<ComponentCost> {
    let mut meter = Meter::default();
    serde_json::to_writer(&mut meter, value)
        .map_err(|e| CheckpointError::new(owner, e.to_string()))?;
    Ok(meter.cost())
}
pub(crate) fn prepare_export<T: Serialize>(
    value: &T,
    owner: &'static str,
    reservation: &mut Reservation,
) -> Result<ComponentCost> {
    reservation.require(Cohort::SavePayload, INITIAL_BYTES)?;
    let cost = measure(value, owner)?;
    if reservation.bytes() < cost.peak_bytes {
        reservation.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}
pub(crate) fn decode<T: DeserializeOwned>(
    bytes: &[u8],
    owner: &'static str,
    reservation: &mut Reservation,
) -> Result<T> {
    decode_with_working(bytes, owner, reservation, 0)
}
/// Additional closed-owner scratch joins the raw-input peak before typed
/// parsing. Canonical reserialization must never shrink an untrusted charge.
pub(crate) fn decode_with_working<T: DeserializeOwned>(
    bytes: &[u8],
    owner: &'static str,
    reservation: &mut Reservation,
    working_bytes: usize,
) -> Result<T> {
    // The host must have charged its bounded input buffer before handing it in.
    // The lexer itself allocates only bounded error text, never escape scratch.
    reservation.require(
        Cohort::LoadCandidate,
        bytes.len().saturating_add(INITIAL_BYTES),
    )?;
    let mut meter = Meter::default();
    meter
        .write_all(bytes)
        .map_err(|e| CheckpointError::new(owner, e.to_string()))?;
    let mut cost = meter.cost();
    cost.peak_bytes = cost
        .peak_bytes
        .checked_add(working_bytes)
        .ok_or_else(|| CheckpointError::new(owner, "working charge overflow"))?;
    if reservation.bytes() < cost.peak_bytes {
        reservation.resize(cost.peak_bytes)?;
    }
    // The 3x encoded allowance covers retained input plus serde_json's escaped
    // string scratch (geometric capacity <=2x input). Recursion remains enabled.
    serde_json::from_slice(bytes).map_err(|e| CheckpointError::new(owner, e.to_string()))
}
pub(crate) fn encode<T: Serialize>(
    value: &T,
    owner: &'static str,
    reservation: &Reservation,
) -> Result<Vec<u8>> {
    let cost = measure(value, owner)?;
    reservation.require(Cohort::SavePayload, cost.peak_bytes)?;
    let mut bytes = Vec::with_capacity(cost.encoded_bytes);
    serde_json::to_writer(&mut bytes, value)
        .map_err(|e| CheckpointError::new(owner, e.to_string()))?;
    Ok(bytes)
}

pub(crate) fn inspect(bytes: &[u8]) -> Result<ComponentCost> {
    let mut meter = Meter::default();
    meter
        .write_all(bytes)
        .map_err(|e| CheckpointError::new("aggregate", e.to_string()))?;
    Ok(meter.cost())
}
