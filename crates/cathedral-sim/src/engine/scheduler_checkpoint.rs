//! Scheduler composition preserves original Engine configuration independently
//! of actual normalized pacing. Complete capture must additionally reconcile
//! the consumed submission notification with Novelty; no partial adoption here.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate},
    receipts::CommandLedgerDtoV1,
    scheduler::checkpoint::{self as owner, SchedulerCheckpointContext, records::raw_float},
    timeline::LogicalTime,
    world::checkpoint::BackboneCandidate,
};
pub use owner::{SchedulerCost, SchedulerCounts};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy)]
pub struct EngineSchedulerCheckpointContext<'a> {
    scheduler: SchedulerCheckpointContext<'a>,
    player: &'a ActorId,
}
impl<'a> EngineSchedulerCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime, player: &'a ActorId) -> Self {
        Self {
            scheduler: SchedulerCheckpointContext::from_world(w, now),
            player,
        }
    }
    pub fn from_backbone(
        b: &'a BackboneCandidate,
        now: LogicalTime,
        ledger: &'a CommandLedgerDtoV1,
        player: &'a ActorId,
    ) -> Self {
        Self {
            scheduler: SchedulerCheckpointContext::from_backbone(b, now, ledger),
            player,
        }
    }
}
#[derive(Serialize)]
pub struct EngineSchedulerDtoV1 {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(with = "owner::records::SchedulerV1")]
    scheduler: NpcScheduler,
    #[serde(with = "raw_float")]
    turn_delay_seconds: f64,
    #[serde(with = "raw_float")]
    maximum_backoff_seconds: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(with = "owner::records::SchedulerV1")]
    scheduler: NpcScheduler,
    #[serde(with = "raw_float")]
    turn_delay_seconds: f64,
    #[serde(with = "raw_float")]
    maximum_backoff_seconds: f64,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    player_id: &'a ActorId,
    #[serde(with = "owner::records::SchedulerV1")]
    scheduler: &'a NpcScheduler,
    #[serde(with = "raw_float")]
    turn_delay_seconds: f64,
    #[serde(with = "raw_float")]
    maximum_backoff_seconds: f64,
}
impl<'a> View<'a> {
    fn new(e: &'a Engine, now: LogicalTime) -> Self {
        Self {
            version: 1,
            boundary: now,
            player_id: &e.config.player_id,
            scheduler: &e.scheduler,
            turn_delay_seconds: e.config.turn_delay_seconds,
            maximum_backoff_seconds: e.config.maximum_backoff_seconds,
        }
    }
}
pub struct EngineSchedulerCandidate {
    data: EngineSchedulerDtoV1,
}
impl Engine {
    pub fn scheduler_checkpoint_context(
        &self,
        now: LogicalTime,
    ) -> EngineSchedulerCheckpointContext<'_> {
        EngineSchedulerCheckpointContext::from_world(&self.world, now, &self.config.player_id)
    }
    pub fn checkpoint_scheduler_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<SchedulerCost>> {
        let cost = owner::prepare(&View::new(self, now), &mut r)?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_scheduler_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineSchedulerDtoV1>> {
        owner::prepare(&View::new(self, now), &mut r)?;
        let c = self.scheduler_checkpoint_context(now);
        validate_binding(now, &self.config.player_id, c)?;
        owner::validate(&self.scheduler, c.scheduler)?;
        Ok(Admitted::new(
            EngineSchedulerDtoV1 {
                version: 1,
                boundary: now,
                player_id: self.config.player_id.clone(),
                scheduler: owner::copy(&self.scheduler),
                turn_delay_seconds: self.config.turn_delay_seconds,
                maximum_backoff_seconds: self.config.maximum_backoff_seconds,
            },
            r,
        ))
    }
}
fn validate_binding(
    boundary: LogicalTime,
    player: &ActorId,
    c: EngineSchedulerCheckpointContext<'_>,
) -> Result<()> {
    owner::check(
        boundary.seconds().to_bits() == c.scheduler.now.seconds().to_bits(),
        "Engine scheduler boundary disagreement",
    )?;
    owner::id(player)?;
    owner::check(
        player == c.player && c.scheduler.backbone.characters.contains_key(player),
        "Engine scheduler player binding disagreement",
    )
}
impl EngineSchedulerDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: EngineSchedulerCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire = aggregate::decode_with_working(
            bytes,
            owner::OWNER,
            &mut r,
            owner::VALIDATION_WORKING_BYTES,
        )?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            player_id: w.player_id,
            scheduler: w.scheduler,
            turn_delay_seconds: w.turn_delay_seconds,
            maximum_backoff_seconds: w.maximum_backoff_seconds,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: EngineSchedulerCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine scheduler version")?;
        validate_binding(self.boundary, &self.player_id, c)?;
        owner::validate(&self.scheduler, c.scheduler)
    }
    pub fn cost(&self) -> Result<SchedulerCost> {
        Ok(aggregate::measure(self, owner::OWNER)?.into())
    }
    pub fn counts(&self, c: EngineSchedulerCheckpointContext<'_>) -> SchedulerCounts {
        owner::counts(&self.scheduler, c.scheduler)
    }
}
impl Admitted<EngineSchedulerDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, owner::OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: EngineSchedulerCheckpointContext<'_>,
    ) -> Result<Admitted<EngineSchedulerCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineSchedulerCandidate { data: d })
        })
    }
}
impl EngineSchedulerCandidate {
    pub fn scheduler(&self) -> &NpcScheduler {
        &self.data.scheduler
    }
    pub fn player_id(&self) -> &ActorId {
        &self.data.player_id
    }
    pub fn turn_delay_seconds(&self) -> f64 {
        self.data.turn_delay_seconds
    }
    pub fn maximum_backoff_seconds(&self) -> f64 {
        self.data.maximum_backoff_seconds
    }
    pub fn counts(&self, c: EngineSchedulerCheckpointContext<'_>) -> SchedulerCounts {
        self.data.counts(c)
    }
}
#[cfg(test)]
mod tests;

impl std::fmt::Debug for EngineSchedulerDtoV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineSchedulerDtoV1")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for EngineSchedulerCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineSchedulerCandidate")
            .finish_non_exhaustive()
    }
}
