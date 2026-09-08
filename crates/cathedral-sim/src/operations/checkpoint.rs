//! Save authority once; rebuild the actor/resource indexes deterministically.
use super::*;
use crate::checkpoint::{
    self, Admitted, BoundedText, BoundedVec, CheckpointError, Cohort, Reservation, Result,
    encoded_len,
};
use crate::receipts::CommandIdV1;

const OWNER: &str = "operations";
const MAX_ENCODED_BYTES: usize = 1024 * 1024;
const WORKING_BYTES: usize = 8 * 1024 * 1024;
fn err(reason: impl Into<String>) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
fn text(s: &str) -> Result<BoundedText<64>> {
    BoundedText::new(s).map_err(err)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterV1 {
    name: BoundedText<64>,
    version: u16,
}
impl AdapterV1 {
    fn export(a: &AdapterDeclaration) -> Result<Self> {
        Ok(Self {
            name: text(&a.name)?,
            version: a.version,
        })
    }
    fn declaration(&self) -> AdapterDeclaration {
        AdapterDeclaration {
            name: self.name.0.clone(),
            version: self.version,
        }
    }
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureV1 {
    id: BoundedText<64>,
    adapter: AdapterV1,
    position: [f64; 3],
    completed_units: u64,
}
impl FixtureV1 {
    fn export(f: &FixtureState) -> Result<Self> {
        Ok(Self {
            id: text(&f.declaration.id)?,
            adapter: AdapterV1::export(&f.declaration.adapter)?,
            position: f.declaration.position,
            completed_units: f.completed_units,
        })
    }
    fn state(&self) -> FixtureState {
        FixtureState {
            declaration: FixtureDeclaration {
                id: self.id.0.clone(),
                adapter: self.adapter.declaration(),
                position: self.position,
            },
            completed_units: self.completed_units,
        }
    }
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveV1 {
    instance: CommandIdV1,
    step_index: u16,
    actor: BoundedText<64>,
    presence_epoch: u64,
    resource: BoundedText<64>,
    adapter: AdapterV1,
    accepted_at: LogicalTime,
    last_observed_at: LogicalTime,
    last_progress_at: LogicalTime,
    required_work: f64,
    completed_work: f64,
    recovery_deadline: ExclusiveDeadline<LogicalTime>,
    retry_limit: u16,
    retries_spent: u16,
    obstruction_revision: u32,
    plan_revision: u32,
    obstructed: bool,
    running: bool,
}
impl ActiveV1 {
    fn export(op: &ActiveOperation) -> Result<Self> {
        Ok(Self {
            instance: op.step.instance.0.into(),
            step_index: op.step.index,
            actor: text(op.actor.as_str())?,
            presence_epoch: op.presence_epoch,
            resource: text(&op.resource)?,
            adapter: AdapterV1::export(&op.adapter)?,
            accepted_at: op.accepted_at,
            last_observed_at: op.last_observed_at,
            last_progress_at: op.last_progress_at,
            required_work: op.required_work,
            completed_work: op.completed_work,
            recovery_deadline: op.recovery_deadline,
            retry_limit: op.retry_limit,
            retries_spent: op.retries_spent,
            obstruction_revision: op.obstruction_revision,
            plan_revision: op.plan_revision,
            obstructed: op.obstructed,
            running: op.running,
        })
    }
    fn operation(&self) -> ActiveOperation {
        ActiveOperation {
            step: StepId {
                instance: InstanceId(self.instance.into()),
                index: self.step_index,
            },
            actor: ActorId::from_raw(&self.actor.0),
            presence_epoch: self.presence_epoch,
            resource: self.resource.0.clone(),
            adapter: self.adapter.declaration(),
            accepted_at: self.accepted_at,
            last_observed_at: self.last_observed_at,
            last_progress_at: self.last_progress_at,
            required_work: self.required_work,
            completed_work: self.completed_work,
            recovery_deadline: self.recovery_deadline,
            retry_limit: self.retry_limit,
            retries_spent: self.retries_spent,
            obstruction_revision: self.obstruction_revision,
            plan_revision: self.plan_revision,
            obstructed: self.obstructed,
            running: self.running,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationKernelDtoV1 {
    version: u16,
    active: BoundedVec<ActiveV1, MAX_INSTANCES>,
    fixtures: BoundedVec<FixtureV1, MAX_FIXTURES>,
}
impl OperationKernel {
    pub fn checkpoint_v1(
        &self,
        config: &OperationConfig,
        world: &World,
        now: LogicalTime,
        reservation: Reservation,
    ) -> Result<Admitted<OperationKernelDtoV1>> {
        reservation.require(Cohort::SavePayload, WORKING_BYTES)?;
        checkpoint::logical(OWNER, now.seconds())?;
        // Validates the live indexes too, before flattening their authoritative
        // records. Export must not silently repair a broken owner.
        self.validate_continuation(config, world, now)
            .map_err(err)?;
        let dto = OperationKernelDtoV1 {
            version: 1,
            active: BoundedVec(
                self.active
                    .values()
                    .map(ActiveV1::export)
                    .collect::<Result<_>>()?,
            ),
            fixtures: BoundedVec(
                self.fixtures
                    .values()
                    .map(FixtureV1::export)
                    .collect::<Result<_>>()?,
            ),
        };
        dto.validate(config, world, now)?;
        Ok(Admitted::new(dto, reservation))
    }
}
impl OperationKernelDtoV1 {
    pub const MAX_ENCODED_BYTES: usize = MAX_ENCODED_BYTES;
    pub const WORKING_BYTES: usize = WORKING_BYTES;
    pub fn decode_json(
        bytes: &[u8],
        config: &OperationConfig,
        world: &World,
        now: LogicalTime,
        reservation: Reservation,
    ) -> Result<Admitted<Self>> {
        let dto: Self =
            checkpoint::decode(bytes, OWNER, MAX_ENCODED_BYTES, &reservation, WORKING_BYTES)?;
        dto.validate(config, world, now)?;
        Ok(Admitted::new(dto, reservation))
    }
    pub fn encode_json(
        dto: Admitted<Self>,
        config: &OperationConfig,
        world: &World,
        now: LogicalTime,
    ) -> Result<Admitted<Vec<u8>>> {
        dto.try_map(|value, reservation| {
            value.validate(config, world, now)?;
            checkpoint::encode(&value, OWNER, MAX_ENCODED_BYTES, reservation, WORKING_BYTES)
        })
    }
    /// Requires the same separately validated ledger/actor candidates and exact
    /// fixture declarations the complete envelope will use. Historical receipt
    /// principals do not replace these actual live owner references.
    pub fn validate(
        &self,
        config: &OperationConfig,
        world: &World,
        now: LogicalTime,
    ) -> Result<()> {
        self.candidate(config, world, now).map(|_| ())
    }
    pub(crate) fn candidate(
        &self,
        config: &OperationConfig,
        world: &World,
        now: LogicalTime,
    ) -> Result<OperationKernel> {
        checkpoint::logical(OWNER, now.seconds())?;
        if self.version != 1
            || self.active.0.len() > MAX_INSTANCES
            || self.fixtures.0.len() > MAX_FIXTURES
        {
            return Err(err("unsupported version or owner count"));
        }
        // Validate scalar domains before serializing: serde_json would otherwise
        // encode some nonfinite floating values as null.
        for f in &self.fixtures.0 {
            if !f.position.into_iter().all(f64::is_finite) {
                return Err(err("nonfinite fixture position"));
            }
        }
        for op in &self.active.0 {
            for t in [
                op.accepted_at,
                op.last_observed_at,
                op.last_progress_at,
                op.recovery_deadline.0,
            ] {
                checkpoint::logical(OWNER, t.seconds())?;
            }
            if !op.required_work.is_finite() || !op.completed_work.is_finite() {
                return Err(err("nonfinite operation work"));
            }
        }
        encoded_len(self, MAX_ENCODED_BYTES).map_err(err)?;
        let mut kernel = OperationKernel::default();
        for f in &self.fixtures.0 {
            if kernel.fixtures.insert(f.id.0.clone(), f.state()).is_some() {
                return Err(err("duplicate fixture identity"));
            }
        }
        for op in &self.active.0 {
            let operation = op.operation();
            let id = operation.step.instance;
            if kernel
                .actor_claims
                .insert(operation.actor.clone(), id)
                .is_some()
                || kernel
                    .resource_claims
                    .insert(operation.resource.clone(), id)
                    .is_some()
                || kernel.active.insert(id, operation).is_some()
            {
                return Err(err("duplicate operation identity, actor or resource claim"));
            }
        }
        kernel
            .validate_continuation(config, world, now)
            .map_err(err)?;
        Ok(kernel)
    }
}
