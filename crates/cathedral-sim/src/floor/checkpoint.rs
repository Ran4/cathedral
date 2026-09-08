//! Exact floor authority, before the later load-specific audio/recording
//! interruption projection. Candidates cannot resume old audio or be adopted.
use super::*;
use crate::{
    checkpoint::{Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
pub(crate) mod records;
pub(crate) const OWNER: &str = "continuity";
/// SpeechEventId is an arbitrary string, not an ActorId. Empty/control/Unicode
/// strings remain exact. This is a supported format bound, not a runtime rule.
pub const MAX_EVENT_ID_BYTES: usize = 65_536;
/// Validation borrows strings and scans at most 32 rows pairwise. No heap
/// working storage beyond the aggregate parser/copy/error allowance is needed.
pub const VALIDATION_WORKING_BYTES: usize = 0;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FloorCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for FloorCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes,
        }
    }
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, reason))
    }
}
pub(crate) fn future(v: f64) -> Result<()> {
    check(
        v >= 0.0 && (v.is_finite() || v == f64::INFINITY),
        "invalid continuity pacing",
    )
}
pub(crate) fn binding(boundary: LogicalTime, now: LogicalTime) -> Result<()> {
    crate::checkpoint::logical(OWNER, now.seconds())?;
    check(
        boundary.seconds().to_bits() == now.seconds().to_bits(),
        "continuity boundary disagreement",
    )
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<FloorCost> {
    Ok(aggregate::prepare_export(v, OWNER, r)?.into())
}
pub(crate) fn validate(f: &ConversationFloor) -> Result<()> {
    check(
        f.awaiting.len() <= MAX_FLOOR_AWAITING,
        "floor awaited count limit",
    )?;
    for (i, row) in f.awaiting.iter().enumerate() {
        check(
            row.event_id.0.len() <= MAX_EVENT_ID_BYTES,
            "floor event id byte limit",
        )?;
        check(
            f.awaiting[..i]
                .iter()
                .all(|prior| prior.event_id != row.event_id),
            "duplicate floor event id",
        )?;
        future(row.deadline)?;
    }
    for t in [
        f.foreground_floor_until,
        f.background_floor_until,
        f.player_hold_until,
    ] {
        future(t)?;
    }
    Ok(())
}
pub(crate) fn copy(f: &ConversationFloor) -> ConversationFloor {
    // Closed construction makes new runtime fields require an owner decision.
    ConversationFloor {
        awaiting: f
            .awaiting
            .iter()
            .map(|r| AwaitedSpeech {
                event_id: r.event_id.clone(),
                deadline: r.deadline,
                blocks_player_reaction: r.blocks_player_reaction,
            })
            .collect(),
        foreground_floor_until: f.foreground_floor_until,
        background_floor_until: f.background_floor_until,
        player_hold_until: f.player_hold_until,
    }
}
#[derive(Debug, Serialize)]
pub struct FloorDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "records::FloorV1")]
    state: ConversationFloor,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "records::FloorV1")]
    state: ConversationFloor,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "records::FloorV1")]
    state: &'a ConversationFloor,
}
#[derive(Debug)]
pub struct FloorCandidate {
    data: FloorDtoV1,
}
impl ConversationFloor {
    pub fn checkpoint_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<FloorCost>> {
        let c = prepare(
            &View {
                version: 1,
                boundary: now,
                state: self,
            },
            &mut r,
        )?;
        Ok(Admitted::new(c, r))
    }
    pub fn export_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<FloorDtoV1>> {
        prepare(
            &View {
                version: 1,
                boundary: now,
                state: self,
            },
            &mut r,
        )?;
        binding(now, now)?;
        validate(self)?;
        Ok(Admitted::new(
            FloorDtoV1 {
                version: 1,
                boundary: now,
                state: copy(self),
            },
            r,
        ))
    }
}
impl FloorDtoV1 {
    pub fn decode(bytes: &[u8], mut r: Reservation, now: LogicalTime) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            state: w.state,
        };
        d.validate(now)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, now: LogicalTime) -> Result<()> {
        check(self.version == 1, "unsupported floor version")?;
        binding(self.boundary, now)?;
        validate(&self.state)
    }
    pub fn cost(&self) -> Result<FloorCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self) -> FloorCounts {
        counts(&self.state)
    }
}
impl Admitted<FloorDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(self, now: LogicalTime) -> Result<Admitted<FloorCandidate>> {
        self.try_map(|d, _| {
            d.validate(now)?;
            Ok(FloorCandidate { data: d })
        })
    }
}
impl FloorCandidate {
    pub fn floor(&self) -> &ConversationFloor {
        &self.data.state
    }
    pub fn counts(&self) -> FloorCounts {
        self.data.counts()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FloorCounts {
    pub awaiting: usize,
    pub foreground_awaiting: usize,
    pub background_awaiting: usize,
    pub event_id_bytes: usize,
    pub foreground_pacing: bool,
    pub background_pacing: bool,
    pub player_hold: bool,
}
pub(crate) fn counts(f: &ConversationFloor) -> FloorCounts {
    let foreground = f
        .awaiting
        .iter()
        .filter(|r| r.blocks_player_reaction)
        .count();
    FloorCounts {
        awaiting: f.awaiting.len(),
        foreground_awaiting: foreground,
        background_awaiting: f.awaiting.len() - foreground,
        event_id_bytes: f.awaiting.iter().map(|r| r.event_id.0.len()).sum(),
        foreground_pacing: f.foreground_floor_until != 0.0,
        background_pacing: f.background_floor_until != 0.0,
        player_hold: f.player_hold_until != 0.0,
    }
}
#[cfg(test)]
mod tests;
