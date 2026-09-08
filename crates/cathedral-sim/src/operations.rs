//! Bounded operation/resource/duty owner. The v1 fixture is deliberately small:
//! one stationary NPC performs measured work at one declared resource. Its
//! completion increments that resource's durable counter; no quest is inferred.
use crate::receipts::{CommandId, OperationId, Outcome, ReceiptState};
use crate::timeline::{ExclusiveDeadline, LogicalTime};
use crate::{ActorId, Control, Vec3, World, WorldClock, round::Round};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_INSTANCES: usize = 256;
pub const MAX_FIXTURES: usize = 256;
pub const MAX_KERNEL_ALLOCATED_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RETRIES: u16 = 32;
pub const MAX_DURATION_SECONDS: f64 = 86_400.0;
pub const TIMED_FIXTURE: &str = "timed_fixture";
pub const ADAPTER_VERSION: u16 = 1;
const WORK_RADIUS: f64 = 1.0;

/// Instance identity includes the initiating action's step, so sibling actions
/// within one semantic provider turn can never alias one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub CommandId);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepId {
    pub instance: InstanceId,
    pub index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDeclaration {
    pub name: String,
    pub version: u16,
}
impl Default for AdapterDeclaration {
    fn default() -> Self {
        Self {
            name: TIMED_FIXTURE.into(),
            version: ADAPTER_VERSION,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureDeclaration {
    pub id: String,
    pub adapter: AdapterDeclaration,
    pub position: [f64; 3],
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OperationConfig {
    pub fixtures: Vec<FixtureDeclaration>,
}
impl OperationConfig {
    pub(crate) fn compact(&mut self) {
        self.fixtures.shrink_to_fit();
        for f in &mut self.fixtures {
            f.id.shrink_to_fit();
            f.adapter.name.shrink_to_fit();
        }
    }
}

/// Every control is itself a consequential command. Replan retries the same
/// logical step; neither its required work nor its recovery deadline is reset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Start {
        actor: ActorId,
        resource: String,
        adapter: AdapterDeclaration,
        work_seconds: f64,
        recovery_seconds: f64,
        retries: u16,
    },
    Replan {
        instance: InstanceId,
    },
    SetObstructed {
        instance: InstanceId,
        obstructed: bool,
    },
    Cancel {
        instance: InstanceId,
    },
}
impl Request {
    pub(crate) fn valid_numbers(&self) -> bool {
        match self {
            Self::Start {
                work_seconds,
                recovery_seconds,
                ..
            } => work_seconds.is_finite() && recovery_seconds.is_finite(),
            _ => true,
        }
    }
    pub(crate) fn raw_bytes(&self) -> usize {
        match self {
            Self::Start {
                actor,
                resource,
                adapter,
                ..
            } => actor
                .as_str()
                .len()
                .saturating_add(resource.len())
                .saturating_add(adapter.name.len()),
            _ => 0,
        }
    }
}

/// One priority policy for all movement writers. The established custody owner
/// remains separate: ending generic work never releases a prisoner or escort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DutyPriority {
    Routine,
    LlmTravel,
    OrdinaryNeed,
    Conversation,
    CommittedWork,
    CriticalNeed,
    Curfew,
    RoadReturn,
    UrgentDanger,
    Custody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveOperation {
    pub step: StepId,
    pub actor: ActorId,
    pub presence_epoch: u64,
    pub resource: String,
    pub adapter: AdapterDeclaration,
    pub accepted_at: LogicalTime,
    pub last_observed_at: LogicalTime,
    pub last_progress_at: LogicalTime,
    pub required_work: f64,
    pub completed_work: f64,
    pub recovery_deadline: ExclusiveDeadline<LogicalTime>,
    pub retry_limit: u16,
    pub retries_spent: u16,
    pub obstruction_revision: u32,
    pub plan_revision: u32,
    pub obstructed: bool,
    pub running: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureState {
    pub declaration: FixtureDeclaration,
    pub completed_units: u64,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OperationKernel {
    active: BTreeMap<InstanceId, ActiveOperation>,
    actor_claims: BTreeMap<ActorId, InstanceId>,
    resource_claims: BTreeMap<String, InstanceId>,
    fixtures: BTreeMap<String, FixtureState>,
}
fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control)
}
fn valid_budget(
    accepted: LogicalTime,
    work: f64,
    deadline: ExclusiveDeadline<LogicalTime>,
) -> bool {
    work.is_finite()
        && work > 0.0
        && work <= MAX_DURATION_SECONDS
        && deadline.0.seconds() - accepted.seconds() > work
        && deadline.0.seconds() - accepted.seconds() <= MAX_DURATION_SECONDS
}
fn validate_adapter(adapter: &AdapterDeclaration) -> Result<(), &'static str> {
    if adapter.name != TIMED_FIXTURE {
        return Err("unknown_adapter");
    }
    if adapter.version != ADAPTER_VERSION {
        return Err("adapter_version");
    }
    Ok(())
}
impl OperationKernel {
    pub fn from_config(config: &OperationConfig) -> Result<Self, String> {
        if config.fixtures.len() > MAX_FIXTURES {
            return Err("too many operation fixtures".into());
        }
        let mut kernel = Self::default();
        for declaration in &config.fixtures {
            validate_adapter(&declaration.adapter).map_err(str::to_owned)?;
            if !valid_name(&declaration.id) || !Vec3::from_array(declaration.position).is_finite() {
                return Err("invalid operation fixture".into());
            }
            if kernel.fixtures.contains_key(&declaration.id) {
                return Err("duplicate operation fixture".into());
            }
            // Copy exact identifiers into compact allocations, not caller spare capacity.
            let mut declaration = declaration.clone();
            declaration.id.shrink_to_fit();
            declaration.adapter.name.shrink_to_fit();
            kernel.fixtures.insert(
                declaration.id.clone(),
                FixtureState {
                    declaration,
                    completed_units: 0,
                },
            );
        }
        Ok(kernel)
    }
    /// Decode only into a candidate. M2 must run this gate plus ledger/root
    /// and world-incarnation validation before adopting any decoded kernel.
    pub fn validate_continuation(
        &self,
        config: &OperationConfig,
        world: &World,
        now: LogicalTime,
    ) -> Result<(), String> {
        let declared = Self::from_config(config)?;
        if self.active.len() > MAX_INSTANCES
            || self.actor_claims.len() != self.active.len()
            || self.resource_claims.len() != self.active.len()
            || self.fixtures.len() != declared.fixtures.len()
        {
            return Err("invalid operation bounds".into());
        }
        if self.allocated_upper_bound() > MAX_KERNEL_ALLOCATED_BYTES {
            return Err("operation continuation allocation bound exceeded".into());
        }
        for (id, fixture) in &self.fixtures {
            if declared
                .fixtures
                .get(id)
                .is_none_or(|expected| expected.declaration != fixture.declaration)
            {
                return Err("operation fixture manifest mismatch".into());
            }
        }
        for (id, op) in &self.active {
            validate_adapter(&op.adapter).map_err(str::to_owned)?;
            if op.step.instance != *id
                || op.step.index != 0
                || id.0.step > crate::receipts::MAX_STEPS
                || id.0.operation.sequence == 0
                || usize::from(id.0.operation.producer) >= crate::receipts::PRODUCER_CAPACITY
                || !valid_name(op.actor.as_str())
                || !valid_name(&op.resource)
                || self.actor_claims.get(&op.actor) != Some(id)
                || self.resource_claims.get(&op.resource) != Some(id)
                || self.fixtures.get(&op.resource).is_none_or(|f| {
                    f.declaration.adapter != op.adapter || f.completed_units == u64::MAX
                })
                || !world.is_present(&op.actor)
                || world.characters.get(&op.actor).is_none_or(|c| {
                    c.state.presence_epoch != op.presence_epoch || c.control() == Control::Player
                })
            {
                return Err("invalid operation identity, incarnation or claim".into());
            }
            if ![op.required_work, op.completed_work]
                .into_iter()
                .all(f64::is_finite)
                || !(op.accepted_at <= op.last_progress_at
                    && op.last_progress_at <= op.last_observed_at
                    && op.last_observed_at <= now)
                || !(0.0 < op.required_work && op.required_work <= MAX_DURATION_SECONDS)
                || !(0.0 <= op.completed_work && op.completed_work < op.required_work)
                || !(!op.recovery_deadline.is_due(now)
                    && valid_budget(op.accepted_at, op.required_work, op.recovery_deadline))
                || op.retry_limit > MAX_RETRIES
                || op.retries_spent > op.retry_limit
                || op.plan_revision != u32::from(op.retries_spent)
                || op.running != (op.completed_work > 0.0)
            {
                return Err("invalid operation continuation budget".into());
            }
            let receipt = world
                .command_ledger
                .get(id.0)
                .ok_or("operation receipt missing")?;
            if receipt.outcome.state
                != if op.running {
                    ReceiptState::InProgress
                } else {
                    ReceiptState::Accepted
                }
                || !(op.accepted_at.seconds() <= receipt.at
                    && receipt.at <= op.last_observed_at.seconds())
                || !world.command_ledger.is_protected(id.0.operation)
            {
                return Err("operation receipt/root is not live".into());
            }
        }
        Ok(())
    }

    /// Conservative retained heap bound: allow one maximally sized BTree node
    /// per live entry (real nodes share entries), plus every owned allocation.
    /// This includes all claim indexes, not just serialized records.
    pub fn allocated_upper_bound(&self) -> usize {
        fn tree<K, V>(n: usize) -> usize {
            n * (64
                + 12 * std::mem::size_of::<usize>()
                + 11 * (std::mem::size_of::<K>() + std::mem::size_of::<V>()))
        }
        tree::<InstanceId, ActiveOperation>(self.active.len())
            + tree::<ActorId, InstanceId>(self.actor_claims.len())
            + tree::<String, InstanceId>(self.resource_claims.len())
            + tree::<String, FixtureState>(self.fixtures.len())
            + self
                .active
                .values()
                .map(|op| {
                    op.actor.allocated_bytes() + op.resource.capacity() + op.adapter.name.capacity()
                })
                .sum::<usize>()
            + self
                .actor_claims
                .keys()
                .map(ActorId::allocated_bytes)
                .sum::<usize>()
            + self
                .resource_claims
                .keys()
                .map(String::capacity)
                .sum::<usize>()
            + self
                .fixtures
                .iter()
                .map(|(id, f)| {
                    id.capacity()
                        + f.declaration.id.capacity()
                        + f.declaration.adapter.name.capacity()
                })
                .sum::<usize>()
    }

    pub fn active(&self, id: InstanceId) -> Option<&ActiveOperation> {
        self.active.get(&id)
    }
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
    pub fn fixture(&self, resource: &str) -> Option<&FixtureState> {
        self.fixtures.get(resource)
    }
    pub fn actor_owner(&self, actor: &ActorId) -> Option<InstanceId> {
        self.actor_claims.get(actor).copied()
    }
    pub fn resource_owner(&self, resource: &str) -> Option<InstanceId> {
        self.resource_claims.get(resource).copied()
    }
    pub fn owns_root(&self, root: OperationId) -> bool {
        self.active.keys().any(|id| id.0.operation == root)
    }
    pub fn permits(&self, actor: &ActorId, priority: DutyPriority) -> bool {
        !self.actor_claims.contains_key(actor) || priority > DutyPriority::CommittedWork
    }
    fn remove(&mut self, id: InstanceId) -> Option<ActiveOperation> {
        let op = self.active.remove(&id)?;
        self.actor_claims.remove(&op.actor);
        self.resource_claims.remove(&op.resource);
        Some(op)
    }
}

pub(crate) fn interrupt(
    world: &mut World,
    actor: &ActorId,
    now: LogicalTime,
    priority: DutyPriority,
    code: &str,
) {
    if priority <= DutyPriority::CommittedWork {
        return;
    }
    if let Some(id) = world.operations.actor_owner(actor) {
        finish(world, id, now, ReceiptState::Interrupted, code);
    }
}
fn finish(world: &mut World, id: InstanceId, now: LogicalTime, state: ReceiptState, code: &str) {
    if world
        .operations
        .active(id)
        .is_none_or(|op| now < op.last_observed_at)
    {
        return;
    }
    world.operations.remove(id);
    let _ = world.command_ledger.advance(
        id.0,
        now.seconds(),
        Outcome::new(
            state,
            code,
            match state {
                ReceiptState::Completed => "fixture work completed",
                _ => "fixture work ended; actor and resource claims released",
            },
        ),
    );
    crate::receipts::release_finished_root(world, id.0.operation);
}

pub(crate) fn command(
    world: &mut World,
    round: &mut Round,
    clock: &WorldClock,
    now: LogicalTime,
    id: CommandId,
    request: Request,
) -> Outcome {
    let controlled = match &request {
        Request::Start { .. } => None,
        Request::Cancel { instance }
        | Request::Replan { instance }
        | Request::SetObstructed { instance, .. } => Some(*instance),
    };
    if controlled
        .and_then(|id| world.operations.active(id))
        .is_some_and(|op| now < op.last_observed_at)
    {
        return Outcome::rejected(
            "operation_time",
            "operation controls cannot precede their last observed logical time",
        );
    }
    match request {
        Request::Start {
            actor,
            resource,
            adapter,
            work_seconds,
            recovery_seconds,
            retries,
        } => {
            if !world.command_ledger.command_pending(id)
                || world.operations.active.contains_key(&InstanceId(id))
            {
                return Outcome::rejected(
                    "operation_admission",
                    "start requires a fresh admitted command ticket",
                );
            }
            if let Err(code) = validate_adapter(&adapter) {
                return Outcome::rejected(code, "operation adapter is unavailable");
            }
            let Some(deadline) =
                LogicalTime::new(now.seconds() + recovery_seconds).map(ExclusiveDeadline)
            else {
                return Outcome::rejected(
                    "operation_arguments",
                    "invalid logical recovery deadline",
                );
            };
            if !valid_name(actor.as_str())
                || !valid_name(&resource)
                || !(0.0 < work_seconds && work_seconds <= MAX_DURATION_SECONDS)
                || !(work_seconds < recovery_seconds && recovery_seconds <= MAX_DURATION_SECONDS)
                || retries > MAX_RETRIES
                || !valid_budget(now, work_seconds, deadline)
            {
                return Outcome::rejected(
                    "operation_arguments",
                    "invalid operation identity or work/recovery budget",
                );
            }
            let Some(character) = world.characters.get(&actor) else {
                return Outcome::rejected("unknown_actor", "no such actor");
            };
            // v1 has no player-controller lock or assembly animation adapter.
            if !world.is_present(&actor) || character.control() == Control::Player {
                return Outcome::rejected("adapter_actor", "this adapter requires a present NPC");
            }
            let epoch = character.state.presence_epoch;
            let Some(fixture) = world.operations.fixtures.get(&resource) else {
                return Outcome::rejected("unknown_resource", "no declared operation resource");
            };
            if fixture.declaration.adapter != adapter {
                return Outcome::rejected("adapter_mismatch", "resource adapter does not match");
            }
            if character
                .position_m()
                .distance(Vec3::from_array(fixture.declaration.position))
                > WORK_RADIUS
            {
                return Outcome::rejected(
                    "out_of_range",
                    "actor must reach the fixture before starting work",
                );
            }
            if fixture.completed_units == u64::MAX {
                return Outcome::rejected("resource_exhausted", "fixture counter is exhausted");
            }
            if world.operations.active.len() >= MAX_INSTANCES {
                return Outcome::rejected(
                    "operation_capacity",
                    "active operation capacity is full",
                );
            }
            if world.operations.actor_claims.contains_key(&actor) {
                return Outcome::rejected("actor_claimed", "actor already has committed work");
            }
            if world.operations.resource_claims.contains_key(&resource) {
                return Outcome::rejected("resource_claimed", "fixture is exclusively claimed");
            }
            if round.operation_atomic_busy(&actor) {
                return Outcome::rejected(
                    "atomic_service",
                    "finish the committed well or food service first",
                );
            }
            if round
                .operation_pressure(world, &actor, clock.at(now.seconds()), now.seconds())
                .is_some()
            {
                return Outcome::rejected("urgent_duty", "a higher priority duty owns this actor");
            }
            // This is the final fallible step. No route, receipt binding, resource
            // counter or Round state is changed on any earlier refusal.
            if let Err(outcome) = world.command_ledger.protect(id.operation) {
                return outcome;
            }
            let actor = ActorId::from_raw(actor.as_str());
            let mut resource = resource;
            resource.shrink_to_fit();
            round.prepare_operation(world, &actor);
            crate::receipts::end_travel(
                world,
                &actor,
                now.seconds(),
                ReceiptState::Superseded,
                "travel_replaced_by_work",
                "committed fixture work superseded this travel intent",
            );
            let c = world.characters.get_mut(&actor).expect("validated actor");
            c.state.intent = None;
            c.state.movement = None;
            let instance = InstanceId(id);
            world
                .operations
                .actor_claims
                .insert(actor.clone(), instance);
            world
                .operations
                .resource_claims
                .insert(resource.clone(), instance);
            world.operations.active.insert(
                instance,
                ActiveOperation {
                    step: StepId { instance, index: 0 },
                    actor,
                    presence_epoch: epoch,
                    resource,
                    adapter: AdapterDeclaration::default(),
                    accepted_at: now,
                    last_observed_at: now,
                    last_progress_at: now,
                    required_work: work_seconds,
                    completed_work: 0.0,
                    recovery_deadline: deadline,
                    retry_limit: retries,
                    retries_spent: 0,
                    obstruction_revision: 0,
                    plan_revision: 0,
                    obstructed: false,
                    running: false,
                },
            );
            Outcome::new(
                ReceiptState::Accepted,
                "operation_accepted",
                "fixture work accepted; completion is pending",
            )
        }
        Request::Cancel { instance } => {
            if !world.operations.active.contains_key(&instance) {
                return Outcome::rejected("operation_inactive", "operation is not active");
            }
            finish(
                world,
                instance,
                now,
                ReceiptState::Interrupted,
                "operation_cancelled",
            );
            Outcome::completed("operation cancelled and claims released")
        }
        Request::Replan { instance } => {
            let Some(op) = world.operations.active.get_mut(&instance) else {
                return Outcome::rejected("operation_inactive", "operation is not active");
            };
            if op.retries_spent >= op.retry_limit || op.plan_revision == u32::MAX {
                finish(
                    world,
                    instance,
                    now,
                    ReceiptState::Interrupted,
                    "operation_retry_exhausted",
                );
                return Outcome::rejected(
                    "operation_retry_exhausted",
                    "operation-wide retry budget is exhausted",
                );
            }
            op.retries_spent += 1;
            op.plan_revision += 1;
            Outcome::completed("same logical step replanned within its original budget")
        }
        Request::SetObstructed {
            instance,
            obstructed,
        } => {
            let Some(op) = world.operations.active.get_mut(&instance) else {
                return Outcome::rejected("operation_inactive", "operation is not active");
            };
            if op.obstructed != obstructed {
                if op.obstruction_revision == u32::MAX {
                    return Outcome::rejected(
                        "obstruction_exhausted",
                        "obstruction revision exhausted",
                    );
                }
                op.obstructed = obstructed;
                op.obstruction_revision += 1;
            }
            Outcome::completed("fixture obstruction recorded")
        }
    }
}

/// Reconcile external ownership changes both before physical progress and after
/// ordinary Round/command writers. This never steals or releases custody care.
pub(crate) fn reconcile(world: &mut World, round: &Round, clock: &WorldClock, now: LogicalTime) {
    if world.operations.active.is_empty() {
        return;
    }
    let ends: Vec<_> = world
        .operations
        .active
        .iter()
        .filter_map(|(id, op)| {
            if now < op.last_observed_at {
                return None;
            }
            let code = if !world.is_present(&op.actor)
                || world
                    .characters
                    .get(&op.actor)
                    .is_none_or(|c| c.state.presence_epoch != op.presence_epoch)
            {
                Some("operation_incarnation_ended")
            } else {
                round
                    .operation_pressure(world, &op.actor, clock.at(now.seconds()), now.seconds())
                    .map(|(_, code)| code)
            };
            code.map(|code| (*id, code))
        })
        .collect();
    for (id, code) in ends {
        finish(world, id, now, ReceiptState::Interrupted, code);
    }
}

/// Called once at the ordinary accepted-time boundary, before same-time controls.
/// Blocked time advances the observation/recovery clock but contributes no work.
pub(crate) fn poll(world: &mut World, round: &Round, clock: &WorldClock, now: LogicalTime) {
    reconcile(world, round, clock, now);
    let mut transitions = Vec::new();
    for (id, op) in &mut world.operations.active {
        if now <= op.last_observed_at {
            continue;
        }
        // Deadline is exclusive, including when completion could coincide with it.
        if op.recovery_deadline.is_due(now) {
            transitions.push((
                *id,
                ReceiptState::Interrupted,
                "operation_recovery_exhausted",
            ));
            continue;
        }
        let dt = now.seconds() - op.last_observed_at.seconds();
        op.last_observed_at = now;
        let at_fixture = world.characters.get(&op.actor).is_some_and(|c| {
            c.position_m().distance(Vec3::from_array(
                world.operations.fixtures[&op.resource].declaration.position,
            )) <= WORK_RADIUS
        });
        if op.obstructed || !at_fixture {
            continue;
        }
        op.completed_work = (op.completed_work + dt).min(op.required_work);
        op.last_progress_at = now;
        if !op.running {
            op.running = true;
            let _ = world.command_ledger.advance(
                id.0,
                now.seconds(),
                Outcome::new(
                    ReceiptState::InProgress,
                    "operation_running",
                    "fixture work is progressing",
                ),
            );
        }
        if op.completed_work >= op.required_work {
            world
                .operations
                .fixtures
                .get_mut(&op.resource)
                .expect("claimed fixture")
                .completed_units += 1;
            transitions.push((*id, ReceiptState::Completed, "operation_completed"));
        }
    }
    for (id, state, code) in transitions {
        finish(world, id, now, state, code);
    }
}

#[cfg(test)]
mod tests;
