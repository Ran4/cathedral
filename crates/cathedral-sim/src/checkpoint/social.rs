//! Shared admission for the closed social components. No production adoption.
use super::{ComponentCost, Reservation, Result, aggregate};
use serde::Serialize;
pub const MAX_WARM_PAIRS: usize = 25_000;
pub const MAX_NOVELTY_MEMORIES: usize = 25_000;
pub const MAX_WITNESSES: usize = 128;
/// Validators borrow all entries and allocate no secondary index. This covers
/// fixed serializer, recursion, error and one inline decoded-record state.
/// Variable-sized ID/error heaps are covered separately by aggregate E/J.
pub const VALIDATION_WORKING_BYTES: usize = 64 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SocialCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for SocialCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(super::CheckpointError::new("social", reason))
    }
}
pub(crate) fn id(a: &crate::ActorId) -> Result<()> {
    check(
        a.as_str().len() <= 4 * crate::MAX_ID_CHARS && crate::ids::is_valid_id(a.as_str()),
        "invalid social identity",
    )
}
pub(crate) fn anchor(at: f64) -> Result<()> {
    super::logical("social", at)
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<SocialCost> {
    let c = SocialCost::from(aggregate::prepare_export(v, "social", r)?);
    if r.bytes() < c.peak_bytes {
        r.resize(c.peak_bytes)?;
    }
    Ok(c)
}
