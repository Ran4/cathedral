//! Exact existing scheduler authority. No constructor, provider call, prompt
//! rendering, pending-work transformation or production installation on decode.
use super::*;
use crate::{
    checkpoint::{self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
mod context;
pub(crate) mod records;
pub use context::SchedulerCheckpointContext;
pub(crate) const OWNER: &str = "scheduler";
pub(crate) fn cognition_input(
    n: &NpcScheduler,
) -> Option<crate::engine::cognition_inputs_checkpoint::records::SchedulerInputRef<'_>> {
    use crate::engine::cognition_inputs_checkpoint::{
        CognitionRequestMethod, SchedulerInputLane, records::SchedulerInputRef,
    };
    n.in_flight.as_ref().map(|f| SchedulerInputRef {
        method: CognitionRequestMethod::RequestWithBudget,
        actor_id: &f.actor_id,
        presence_epoch: f.presence_epoch,
        request_id: f.request_id,
        semantic: f.semantic,
        lane: match f.lane {
            TurnLane::PlayerReaction => SchedulerInputLane::PlayerReaction,
            TurnLane::Handoff => SchedulerInputLane::Handoff,
            TurnLane::Idle => SchedulerInputLane::Idle,
        },
        prompt: &f.prompt,
        output_token_budget: f.output_token_budget,
    })
}
pub const MAX_ORDER_SLOTS: usize = 100_000;
pub const MAX_LANE_ACTORS: usize = 25_000;
pub const MAX_RETRY_WORK: usize = 256;
pub const MAX_INPUT_ROWS: usize = 64;
pub const MAX_PROMPT_BYTES: usize = 65_536;
pub const MAX_HELD_REPLY_BYTES: usize = 400_000;
/// The next increment must remain a positive i32 before `as i32 - 1`.
/// This supplies local headroom, not an all-future consumer horizon.
pub const MAX_PROVIDER_FAILURES: u32 = i32::MAX as u32 - 1;
/// Ledger validation finishes before the <=50,000 borrowed queue pointers and
/// <=257 semantic roots are sorted. See the closed layout proof in M2a11.
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SchedulerCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for SchedulerCost {
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
        Err(CheckpointError::new(OWNER, reason))
    }
}
pub(crate) fn id(a: &ActorId) -> Result<()> {
    check(
        a.as_str().len() <= 4 * crate::MAX_ID_CHARS && crate::ids::is_valid_id(a.as_str()),
        "invalid scheduler identity",
    )
}
pub(crate) fn prepare<T: Serialize>(v: &T, r: &mut Reservation) -> Result<SchedulerCost> {
    let c = SchedulerCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < c.peak_bytes {
        r.resize(c.peak_bytes)?;
    }
    Ok(c)
}
pub(crate) fn validate(n: &NpcScheduler, c: SchedulerCheckpointContext<'_>) -> Result<()> {
    // Saved-ledger scratch is dropped before constructing the lane/root index.
    c.validate()?;
    check(
        n.order.len() <= MAX_ORDER_SLOTS
            && n.priority_handoffs.len() <= MAX_LANE_ACTORS
            && n.player_reactions.len() <= MAX_LANE_ACTORS
            && n.retry_work.len() <= MAX_RETRY_WORK,
        "scheduler count limit",
    )?;
    check(
        if n.order.is_empty() {
            n.round_robin_index == 0
        } else {
            n.round_robin_index < n.order.len()
        },
        "scheduler cursor outside weighted order",
    )?;
    for a in n
        .order
        .iter()
        .chain(&n.priority_handoffs)
        .chain(&n.player_reactions)
        .chain(n.submitted.iter())
    {
        id(a)?;
    }
    for x in [
        n.minimum_delay_seconds,
        n.maximum_backoff_seconds,
        n.next_turn_at,
    ] {
        check(
            x >= 0.0 && (x.is_finite() || x == f64::INFINITY),
            "invalid scheduler pacing",
        )?;
    }
    check(
        n.maximum_backoff_seconds >= 1.0 && n.maximum_backoff_seconds >= n.minimum_delay_seconds,
        "scheduler normalized backoff disagreement",
    )?;
    check(
        n.provider_failures <= MAX_PROVIDER_FAILURES,
        "scheduler failure counter headroom",
    )?;
    {
        let mut lane: Vec<_> = n
            .player_reactions
            .iter()
            .chain(&n.priority_handoffs)
            .collect();
        lane.sort_unstable();
        check(
            lane.windows(2).all(|w| w[0] != w[1]),
            "duplicate scheduler lane actor",
        )?;
    }
    let mut roots = Vec::with_capacity(n.retry_work.len() + usize::from(n.in_flight.is_some()));
    for (actor, work) in &n.retry_work {
        id(actor)?;
        check(c.root(work.semantic), "scheduler retry root disagreement")?;
        roots.push(work.semantic);
    }
    if let Some(f) = &n.in_flight {
        id(&f.actor_id)?;
        check(c.root(f.semantic), "scheduler flight root disagreement")?;
        roots.push(f.semantic);
        check(
            f.drained_events.len() <= MAX_INPUT_ROWS && f.presented.len() <= MAX_INPUT_ROWS,
            "scheduler input row limit",
        )?;
        check(
            f.drained_events
                .iter()
                .chain(&f.presented)
                .all(|s| s.len() <= checkpoint::records::MAX_TEXT_BYTES)
                && f.prompt.len() <= MAX_PROMPT_BYTES,
            "scheduler input text limit",
        )?;
    }
    roots.sort_unstable();
    check(
        roots.windows(2).all(|w| w[0] != w[1]),
        "duplicate scheduler semantic root",
    )?;
    if let Some(done) = &n.held_result {
        check(
            n.in_flight
                .as_ref()
                .is_some_and(|f| f.request_id == done.request_id),
            "held scheduler request disagreement",
        )?;
        checkpoint::logical(OWNER, done.duration_seconds)?;
        match &done.result {
            // Floor deferral precedes the scalar success predicate. Preserve an
            // oversized answer so ordinary continuation takes its failure path.
            Ok(t) => check(
                t.len() <= MAX_HELD_REPLY_BYTES,
                "held scheduler reply byte limit",
            )?,
            Err(e) => check(
                e.kind().len() <= checkpoint::records::MAX_TEXT_BYTES
                    && e.detail().len() <= checkpoint::records::MAX_TEXT_BYTES,
                "held scheduler error limit",
            )?,
        }
    }
    Ok(())
}
pub(crate) fn copy(n: &NpcScheduler) -> NpcScheduler {
    NpcScheduler {
        order: n.order.clone(),
        minimum_delay_seconds: n.minimum_delay_seconds,
        maximum_backoff_seconds: n.maximum_backoff_seconds,
        round_robin_index: n.round_robin_index,
        priority_handoffs: n.priority_handoffs.clone(),
        player_reactions: n.player_reactions.clone(),
        in_flight: n.in_flight.clone().map(|mut f| {
            f.output_token_budget = crate::traits::AcceptedOutputBudget::MissingLegacy;
            f
        }),
        retry_work: n.retry_work.clone(),
        held_result: n.held_result.clone(),
        next_turn_at: n.next_turn_at,
        provider_failures: n.provider_failures,
        running: n.running,
        submitted: n.submitted.clone(),
    }
}
#[derive(Serialize)]
pub struct NpcSchedulerDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "records::SchedulerV1")]
    pub(crate) scheduler: NpcScheduler,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "records::SchedulerV1")]
    scheduler: NpcScheduler,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "records::SchedulerV1")]
    scheduler: &'a NpcScheduler,
}
pub struct NpcSchedulerCandidate {
    pub(crate) data: NpcSchedulerDtoV1,
}
impl NpcScheduler {
    pub fn export_checkpoint(
        &self,
        c: SchedulerCheckpointContext<'_>,
        mut r: Reservation,
    ) -> Result<Admitted<NpcSchedulerDtoV1>> {
        prepare(
            &View {
                version: 1,
                boundary: c.now,
                scheduler: self,
            },
            &mut r,
        )?;
        validate(self, c)?;
        Ok(Admitted::new(
            NpcSchedulerDtoV1 {
                version: 1,
                boundary: c.now,
                scheduler: copy(self),
            },
            r,
        ))
    }
}
impl NpcSchedulerDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: SchedulerCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            scheduler: w.scheduler,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: SchedulerCheckpointContext<'_>) -> Result<()> {
        check(self.version == 1, "unsupported scheduler version")?;
        check(
            self.boundary.seconds().to_bits() == c.now.seconds().to_bits(),
            "scheduler boundary disagreement",
        )?;
        validate(&self.scheduler, c)
    }
    pub fn cost(&self) -> Result<SchedulerCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: SchedulerCheckpointContext<'_>) -> SchedulerCounts {
        counts(&self.scheduler, c)
    }
}
impl Admitted<NpcSchedulerDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: SchedulerCheckpointContext<'_>,
    ) -> Result<Admitted<NpcSchedulerCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(NpcSchedulerCandidate { data: d })
        })
    }
}
impl NpcSchedulerCandidate {
    pub fn scheduler(&self) -> &NpcScheduler {
        &self.data.scheduler
    }
    pub fn counts(&self, c: SchedulerCheckpointContext<'_>) -> SchedulerCounts {
        self.data.counts(c)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SchedulerCounts {
    pub characters: usize,
    pub order_slots: usize,
    pub round_robin_index: usize,
    pub priority_handoffs: usize,
    pub player_reactions: usize,
    pub retry_work: usize,
    pub in_flight: bool,
    pub flight_player_reaction: bool,
    pub held_success: bool,
    pub held_error: bool,
    pub prompt_bytes: usize,
    pub drained_rows: usize,
    pub drained_bytes: usize,
    pub presented_rows: usize,
    pub presented_bytes: usize,
    pub held_bytes: usize,
    pub provider_failures: u32,
    pub running: bool,
    pub submitted: bool,
}
pub(crate) fn counts(n: &NpcScheduler, c: SchedulerCheckpointContext<'_>) -> SchedulerCounts {
    SchedulerCounts {
        characters: c.backbone.characters.len(),
        order_slots: n.order.len(),
        round_robin_index: n.round_robin_index,
        priority_handoffs: n.priority_handoffs.len(),
        player_reactions: n.player_reactions.len(),
        retry_work: n.retry_work.len(),
        in_flight: n.in_flight.is_some(),
        flight_player_reaction: n.in_flight_is_player_reaction(),
        held_success: n.held_result.as_ref().is_some_and(|x| x.result.is_ok()),
        held_error: n.held_result.as_ref().is_some_and(|x| x.result.is_err()),
        prompt_bytes: n.in_flight.as_ref().map_or(0, |f| f.prompt.len()),
        drained_rows: n.in_flight.as_ref().map_or(0, |f| f.drained_events.len()),
        drained_bytes: n
            .in_flight
            .as_ref()
            .map_or(0, |f| f.drained_events.iter().map(String::len).sum()),
        presented_rows: n.in_flight.as_ref().map_or(0, |f| f.presented.len()),
        presented_bytes: n
            .in_flight
            .as_ref()
            .map_or(0, |f| f.presented.iter().map(String::len).sum()),
        held_bytes: n.held_result.as_ref().map_or(0, |x| match &x.result {
            Ok(t) => t.len(),
            Err(e) => e.kind().len() + e.detail().len(),
        }),
        provider_failures: n.provider_failures,
        running: n.running,
        submitted: n.submitted.is_some(),
    }
}
#[cfg(test)]
mod tests;

impl std::fmt::Debug for NpcSchedulerDtoV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcSchedulerDtoV1").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for NpcSchedulerCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcSchedulerCandidate")
            .finish_non_exhaustive()
    }
}

impl NpcScheduler {
    pub(crate) fn complete_roots(&self) -> impl Iterator<Item = crate::receipts::OperationId> + '_ {
        self.retry_work
            .values()
            .map(|w| w.semantic)
            .chain(self.in_flight.iter().map(|f| f.semantic))
    }
}

impl NpcScheduler {
    pub(crate) fn complete_submission_consumed(&self) -> bool {
        self.submitted.is_none()
    }
}
