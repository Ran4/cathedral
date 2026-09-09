//! Exact accepted cognition arguments, separately bound to the historical V1
//! scheduler and Night owners. Neither legacy component alone supplies these
//! inputs. This candidate cannot install owners or resubmit provider work.
use super::{Engine, NightOffice, NpcScheduler, WorldClock};
use crate::{
    ActorId, RequestId,
    checkpoint::{Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate},
    engine::{
        night_checkpoint::EngineNightCandidate,
        scheduler_checkpoint::{EngineSchedulerCandidate, EngineSchedulerCheckpointContext},
    },
    night::checkpoint::{self as night_owner, NightCheckpointContext},
    receipts::{CommandLedgerDtoV1, OperationId},
    scheduler::checkpoint::{self as scheduler_owner, SchedulerCheckpointContext},
    timeline::LogicalTime,
    traits::AcceptedOutputBudget,
    world::checkpoint::BackboneCandidate,
};
use serde::{Deserialize, Serialize};
pub(crate) mod records;
pub use records::{
    CognitionRequestMethod, NightInput, NightInputSubject, SchedulerInput, SchedulerInputLane,
};
const OWNER: &str = "cognition_inputs";
/// Existing scheduler and Night indexes are built sequentially, never retained
/// together. The new owner has only two fixed rows and no validation index.
pub const VALIDATION_WORKING_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CognitionInputsCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for CognitionInputsCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, reason))
    }
}
#[derive(Clone, Copy)]
enum Owners<'a> {
    Live(&'a Engine),
    Saved {
        scheduler: &'a EngineSchedulerCandidate,
        night: &'a EngineNightCandidate,
        backbone: &'a BackboneCandidate,
        ledger: &'a CommandLedgerDtoV1,
        clock: &'a WorldClock,
    },
}
/// Borrowing-only construction. All validation (including existing owner
/// scratch) occurs after the caller's cohort has reserved the working charge.
#[derive(Clone, Copy)]
pub struct CognitionInputsCheckpointContext<'a> {
    owners: Owners<'a>,
    now: LogicalTime,
}
impl<'a> CognitionInputsCheckpointContext<'a> {
    pub fn from_components(
        scheduler: &'a EngineSchedulerCandidate,
        night: &'a EngineNightCandidate,
        backbone: &'a BackboneCandidate,
        now: LogicalTime,
        ledger: &'a CommandLedgerDtoV1,
        clock: &'a WorldClock,
    ) -> Self {
        Self {
            owners: Owners::Saved {
                scheduler,
                night,
                backbone,
                ledger,
                clock,
            },
            now,
        }
    }
    fn player(self) -> &'a ActorId {
        match self.owners {
            Owners::Live(e) => &e.config.player_id,
            Owners::Saved { scheduler, .. } => scheduler.player_id(),
        }
    }
    fn scheduler(self) -> &'a NpcScheduler {
        match self.owners {
            Owners::Live(e) => &e.scheduler,
            Owners::Saved { scheduler, .. } => scheduler.scheduler(),
        }
    }
    fn night(self) -> &'a NightOffice {
        match self.owners {
            Owners::Live(e) => &e.night,
            Owners::Saved { night, .. } => night.night(),
        }
    }
    fn validate(self) -> Result<()> {
        crate::checkpoint::logical(OWNER, self.now.seconds())?;
        scheduler_owner::id(self.player())?;
        match self.owners {
            Owners::Live(e) => {
                check(
                    e.world.characters.contains_key(self.player()),
                    "cognition input player disagreement",
                )?;
                scheduler_owner::validate(
                    &e.scheduler,
                    SchedulerCheckpointContext::from_world(&e.world, self.now),
                )?;
                // Prior scheduler scratch has been released at this boundary.
                night_owner::validate_cognition_inputs(
                    &e.night,
                    NightCheckpointContext::from_world(&e.world, self.now, &e.clock),
                )?;
            }
            Owners::Saved {
                scheduler,
                night,
                backbone,
                ledger,
                clock,
            } => {
                // Both explicit bit checks precede validation. Legacy Night's
                // numeric equality alone intentionally cannot certify -0/+0.
                check(
                    scheduler.cognition_inputs_boundary().seconds().to_bits()
                        == self.now.seconds().to_bits(),
                    "scheduler cognition input boundary disagreement",
                )?;
                check(
                    night.cognition_inputs_boundary().seconds().to_bits()
                        == self.now.seconds().to_bits(),
                    "Night cognition input boundary disagreement",
                )?;
                scheduler.validate_cognition_inputs(
                    EngineSchedulerCheckpointContext::from_backbone(
                        backbone,
                        self.now,
                        ledger,
                        self.player(),
                    ),
                )?;
                night.validate_cognition_inputs(NightCheckpointContext::from_backbone(
                    backbone, self.now, clock, ledger,
                ))?;
            }
        }
        Ok(())
    }
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    boundary: LogicalTime,
    player_id: &'a ActorId,
    scheduler: Option<records::SchedulerInputRef<'a>>,
    night: Option<records::NightInputRef<'a>>,
}
impl<'a> View<'a> {
    fn new(c: CognitionInputsCheckpointContext<'a>) -> Self {
        Self {
            version: 1,
            boundary: c.now,
            player_id: c.player(),
            scheduler: scheduler_owner::cognition_input(c.scheduler()),
            night: night_owner::cognition_input(c.night()),
        }
    }
}
fn prepare(v: &View<'_>, r: &mut Reservation) -> Result<CognitionInputsCost> {
    let c = CognitionInputsCost::from(aggregate::prepare_export(v, OWNER, r)?);
    if r.bytes() < c.peak_bytes {
        r.resize(c.peak_bytes)?;
    }
    Ok(c)
}
#[derive(Debug, Serialize)]
pub struct EngineCognitionInputsDtoV1 {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    scheduler: Option<SchedulerInput>,
    night: Option<NightInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    player_id: ActorId,
    #[serde(deserialize_with = "records::scheduler")]
    scheduler: Option<SchedulerInput>,
    #[serde(deserialize_with = "records::night")]
    night: Option<NightInput>,
}
#[derive(Debug)]
pub struct EngineCognitionInputsCandidate {
    data: EngineCognitionInputsDtoV1,
}
impl Engine {
    pub fn cognition_inputs_checkpoint_context(
        &self,
        now: LogicalTime,
    ) -> CognitionInputsCheckpointContext<'_> {
        CognitionInputsCheckpointContext {
            owners: Owners::Live(self),
            now,
        }
    }
    pub fn checkpoint_cognition_inputs_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<CognitionInputsCost>> {
        let c = self.cognition_inputs_checkpoint_context(now);
        let cost = prepare(&View::new(c), &mut r)?;
        c.validate()?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_cognition_inputs_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineCognitionInputsDtoV1>> {
        let c = self.cognition_inputs_checkpoint_context(now);
        let v = View::new(c);
        prepare(&v, &mut r)?;
        c.validate()?;
        let d = EngineCognitionInputsDtoV1 {
            version: 1,
            boundary: now,
            player_id: c.player().clone(),
            scheduler: v
                .scheduler
                .as_ref()
                .map(records::SchedulerInputRef::copy)
                .transpose()?,
            night: v
                .night
                .as_ref()
                .map(records::NightInputRef::copy)
                .transpose()?,
        };
        Ok(Admitted::new(d, r))
    }
}
impl EngineCognitionInputsDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: CognitionInputsCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            player_id: w.player_id,
            scheduler: w.scheduler,
            night: w.night,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: CognitionInputsCheckpointContext<'_>) -> Result<()> {
        check(self.version == 1, "unsupported cognition input version")?;
        check(
            self.boundary.seconds().to_bits() == c.now.seconds().to_bits(),
            "cognition input boundary disagreement",
        )?;
        check(
            &self.player_id == c.player(),
            "cognition input player disagreement",
        )?;
        c.validate()?;
        check(
            match (
                self.scheduler.as_ref(),
                scheduler_owner::cognition_input(c.scheduler()),
            ) {
                (None, None) => true,
                (Some(v), Some(f)) => f.agrees(v),
                _ => false,
            },
            "scheduler cognition input flight disagreement",
        )?;
        check(
            match (self.night.as_ref(), night_owner::cognition_input(c.night())) {
                (None, None) => true,
                (Some(v), Some(f)) => f.agrees(v),
                _ => false,
            },
            "Night cognition input flight disagreement",
        )
    }
    pub fn cost(&self) -> Result<CognitionInputsCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn player_id(&self) -> &ActorId {
        &self.player_id
    }
    pub fn scheduler(&self) -> Option<&SchedulerInput> {
        self.scheduler.as_ref()
    }
    pub fn night(&self) -> Option<&NightInput> {
        self.night.as_ref()
    }
    pub fn counts(&self) -> CognitionInputsCounts {
        CognitionInputsCounts {
            scheduler: self.scheduler.is_some(),
            night: self.night.is_some(),
            scheduler_prompt_bytes: self.scheduler.as_ref().map_or(0, |v| v.prompt().len()),
            night_prompt_bytes: self.night.as_ref().map_or(0, |v| v.prompt().len()),
            scheduler_provider_default: self
                .scheduler
                .as_ref()
                .is_some_and(|v| v.output_token_budget().is_none()),
            night_provider_default: self
                .night
                .as_ref()
                .is_some_and(|v| v.output_token_budget().is_none()),
        }
    }
}
impl Admitted<EngineCognitionInputsDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: CognitionInputsCheckpointContext<'_>,
    ) -> Result<Admitted<EngineCognitionInputsCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineCognitionInputsCandidate { data: d })
        })
    }
}
impl EngineCognitionInputsCandidate {
    pub fn player_id(&self) -> &ActorId {
        self.data.player_id()
    }
    pub fn scheduler(&self) -> Option<&SchedulerInput> {
        self.data.scheduler()
    }
    pub fn night(&self) -> Option<&NightInput> {
        self.data.night()
    }
    pub fn counts(&self) -> CognitionInputsCounts {
        self.data.counts()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CognitionInputsCounts {
    pub scheduler: bool,
    pub night: bool,
    pub scheduler_prompt_bytes: usize,
    pub night_prompt_bytes: usize,
    pub scheduler_provider_default: bool,
    pub night_provider_default: bool,
}
#[cfg(test)]
mod tests;
