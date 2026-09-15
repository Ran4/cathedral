//! One bounded worker for complete typed preparation and actual owner disposal.
//! Delivery retains its return slot through non-Send construction and staging.
use crate::checkpoint_services::{ForwardingServices, SendContinuationServices};
use crate::checkpoint_storage::{LoadedCheckpoint, SlotReference};
use cathedral_sim::{
    RuntimeGeneration,
    checkpoint::{
        self, Admitted, CheckpointBudget, Cohort, Reservation, RetirementLease, RetirementRelease,
        complete::{
            CandidateDisposal, DecodedHydration, HydratedEngine, HydrationAssets, HydrationStage,
            PreparedContinuation,
        },
        host::DefinitionsV1,
    },
};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "checkpoint_preparation_review_tests.rs"]
mod review_tests;
#[cfg(test)]
pub(crate) mod tests;

/// 2 MiB explicit stack plus 2 MiB bounded control, two maximum inline owner
/// wrappers, phase samples, panic/diagnostic and worker TLS scratch. World,
/// asset/factory captures and retirement payload storage have separate leases.
pub const SERVICE_ALLOWANCE_BYTES: usize = 4 * 1024 * 1024;
static NEXT_SERVICE: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparationId {
    service: u64,
    sequence: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError {
    Full,
    Closed,
    WrongBudget,
    Admission,
    Exhausted,
    Unknown,
    WrongStage,
}
#[derive(Debug, Clone, Copy)]
pub struct PreparationOptions {
    pub recipe_upper_bytes: usize,
    pub assets_upper_bytes: usize,
    pub host_definitions: DefinitionsV1,
    pub generation: RuntimeGeneration,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct PreparationTimings {
    /// Assets, definitions, typed owners, raw disposal, retention; seconds.
    pub worker_phases: [f64; 5],
    pub worker_total: f64,
}
type FactoryFn = Box<dyn FnOnce() -> checkpoint::Result<HydrationAssets> + Send>;
struct Factory {
    call: Option<FactoryFn>,
    _lease: Reservation,
}
impl Drop for Factory {
    fn drop(&mut self) {
        // Cancelled factory captures can have user destructors. Dispose their
        // panic payload while the recipe charge still owns those captures.
        let call = self.call.take();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(call)));
    }
}
struct Job {
    loaded: LoadedCheckpoint,
    factory: Factory,
    options: PreparationOptions,
}
struct WorkerReady {
    reference: SlotReference,
    value: Admitted<DecodedHydration>,
    timings: PreparationTimings,
}
enum WorkerOutcome {
    Ready(WorkerReady),
    Failed(checkpoint::CheckpointError),
    Cancelled,
}
enum PrepState {
    Reserved,
    Queued(Job),
    Active,
    Ready(WorkerOutcome),
    Delivered,
    Dispose(Box<dyn Send>),
}
struct PrepEntry {
    id: PreparationId,
    state: PrepState,
    cancelled: bool,
    disposal_seconds: Option<f64>,
}
enum RetireState {
    Reserved,
    Queued(RetiredPayload),
    Active,
    AwaitRelease,
    Released,
}
struct RetireEntry {
    id: PreparationId,
    state: RetireState,
    release: RetirementRelease,
    disposal_seconds: Option<f64>,
    panicked: bool,
}
struct Queue {
    preparation: Option<PrepEntry>,
    retirement: Option<RetireEntry>,
    next: u64,
    closed: bool,
    abandoned: bool,
}
struct Core {
    queue: Mutex<Queue>,
    wake: Condvar,
    budget: Arc<CheckpointBudget>,
    identity: u64,
    _overhead: Reservation,
}
impl Core {
    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn id(&self, q: &mut Queue) -> Result<PreparationId, SubmitError> {
        let sequence = q.next;
        q.next = q.next.checked_add(1).ok_or(SubmitError::Exhausted)?;
        Ok(PreparationId {
            service: self.identity,
            sequence,
        })
    }
    fn return_disposal(&self, id: PreparationId, value: Box<dyn Send>) {
        let mut q = self.lock();
        let entry = q
            .preparation
            .as_mut()
            .expect("delivery retains preparation slot");
        assert_eq!(entry.id, id);
        assert!(matches!(
            entry.state,
            PrepState::Delivered | PrepState::Reserved
        ));
        entry.cancelled = true;
        entry.state = PrepState::Dispose(value);
        self.wake.notify_all();
    }
}

pub struct CheckpointPreparation {
    core: Arc<Core>,
    worker: Option<JoinHandle<()>>,
}
impl CheckpointPreparation {
    /// Off-frame startup. Admission precedes all worker/control allocation.
    pub fn start(budget: Arc<CheckpointBudget>) -> Result<Self, SubmitError> {
        let overhead = budget
            .reserve_running_overhead(SERVICE_ALLOWANCE_BYTES)
            .map_err(|_| SubmitError::Admission)?;
        let identity = NEXT_SERVICE
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map_err(|_| SubmitError::Exhausted)?;
        let core = Arc::new(Core {
            queue: Mutex::new(Queue {
                preparation: None,
                retirement: None,
                next: 1,
                closed: false,
                abandoned: false,
            }),
            wake: Condvar::new(),
            budget,
            identity,
            _overhead: overhead,
        });
        let worker_core = Arc::clone(&core);
        let worker = std::thread::Builder::new()
            .name("checkpoint-prepare".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || run(worker_core))
            .map_err(|_| SubmitError::Closed)?;
        Ok(Self {
            core,
            worker: Some(worker),
        })
    }
    /// Reserve the return slot and recipe storage while the original loaded
    /// input remains intact. Every refusal returns that same admitted input.
    pub fn reserve_load(
        &self,
        loaded: LoadedCheckpoint,
        options: PreparationOptions,
    ) -> Result<PreparationPermit, (SubmitError, LoadedCheckpoint)> {
        let mut q = self.core.lock();
        let checked = (|| {
            if q.closed {
                return Err(SubmitError::Closed);
            }
            if q.preparation.is_some() {
                return Err(SubmitError::Full);
            }
            if !self.core.budget.owns_complete_input(&loaded.input) {
                return Err(SubmitError::WrongBudget);
            }
            let recipe = self
                .core
                .budget
                .reserve_load_overhead(options.recipe_upper_bytes)
                .map_err(|_| SubmitError::Admission)?;
            let id = self.core.id(&mut q)?;
            Ok((id, recipe))
        })();
        match checked {
            Err(error) => Err((error, loaded)),
            Ok((id, recipe)) => {
                q.preparation = Some(PrepEntry {
                    id,
                    state: PrepState::Reserved,
                    cancelled: false,
                    disposal_seconds: None,
                });
                Ok(PreparationPermit {
                    core: Arc::clone(&self.core),
                    id,
                    loaded: Some(loaded),
                    recipe: Some(recipe),
                    options,
                })
            }
        }
    }
    pub fn cancel(&self, id: PreparationId) -> Result<(), SubmitError> {
        let mut q = self.core.lock();
        let p = q
            .preparation
            .as_mut()
            .filter(|p| p.id == id)
            .ok_or(SubmitError::Unknown)?;
        p.cancelled = true;
        self.core.wake.notify_all();
        Ok(())
    }
    /// Available after cancellation cleanup and before its terminal is taken.
    /// Measures worker destruction separately from host return/enqueue time.
    pub fn preparation_disposal_seconds(&self, id: PreparationId) -> Option<f64> {
        self.core
            .lock()
            .preparation
            .as_ref()
            .filter(|p| p.id == id)?
            .disposal_seconds
    }
    pub fn try_take(&self, id: PreparationId) -> Option<PreparationOutcome> {
        let mut q = self.core.lock();
        let p = q.preparation.as_mut().filter(|p| p.id == id)?;
        if p.cancelled && !matches!(p.state, PrepState::Ready(WorkerOutcome::Cancelled)) {
            return None;
        }
        if !matches!(p.state, PrepState::Ready(_)) {
            return None;
        }
        let PrepState::Ready(outcome) = std::mem::replace(&mut p.state, PrepState::Delivered)
        else {
            unreachable!()
        };
        match outcome {
            WorkerOutcome::Ready(ready) => Some(PreparationOutcome::Ready(PreparedDelivery {
                core: Arc::clone(&self.core),
                id,
                reference: ready.reference,
                stage: Some(DeliveryStage::Decoded(ready.value)),
                services: None,
                timings: ready.timings,
            })),
            WorkerOutcome::Failed(error) => {
                q.preparation = None;
                self.core.wake.notify_all();
                Some(PreparationOutcome::Failed(error))
            }
            WorkerOutcome::Cancelled => {
                q.preparation = None;
                self.core.wake.notify_all();
                Some(PreparationOutcome::Cancelled)
            }
        }
    }
    /// Acquire before fencing or dismantling any old host owner. Payload submit
    /// after this succeeds is infallible, including after service shutdown.
    pub fn reserve_retirement(
        &self,
        actual_upper_bytes: usize,
    ) -> Result<RetirementPermit, SubmitError> {
        let mut q = self.core.lock();
        if q.closed {
            return Err(SubmitError::Closed);
        }
        if q.retirement.is_some() {
            return Err(SubmitError::Full);
        }
        let lease = RetirementLease::new(
            self.core
                .budget
                .reserve(Cohort::RetiringGeneration, actual_upper_bytes)
                .map_err(|_| SubmitError::Admission)?,
        )
        .map_err(|_| SubmitError::Admission)?;
        let id = self.core.id(&mut q)?;
        q.retirement = Some(RetireEntry {
            id,
            state: RetireState::Reserved,
            release: lease.release_observer(),
            disposal_seconds: None,
            panicked: false,
        });
        Ok(RetirementPermit {
            core: Arc::clone(&self.core),
            id,
            lease: Some(lease),
        })
    }
    pub fn retirement_status(&self, id: PreparationId) -> Option<RetirementStatus> {
        let q = self.core.lock();
        let r = q.retirement.as_ref().filter(|r| r.id == id)?;
        Some(RetirementStatus {
            owners_disposed: r.disposal_seconds.is_some(),
            released: matches!(r.state, RetireState::Released),
            disposal_seconds: r.disposal_seconds,
            panicked: r.panicked,
        })
    }
    pub fn take_retirement_result(&self, id: PreparationId) -> Option<RetirementStatus> {
        let mut q = self.core.lock();
        let r = q
            .retirement
            .as_ref()
            .filter(|r| r.id == id && matches!(r.state, RetireState::Released))?;
        let status = RetirementStatus {
            owners_disposed: r.disposal_seconds.is_some(),
            released: true,
            disposal_seconds: r.disposal_seconds,
            panicked: r.panicked,
        };
        q.retirement = None;
        self.core.wake.notify_all();
        Some(status)
    }
    /// Stops new admission, retaining terminal access and outstanding return
    /// permits. This never joins or waits for factory, disk or destruction.
    pub fn close(&self) {
        let mut q = self.core.lock();
        q.closed = true;
        self.core.wake.notify_all();
    }
    /// Explicit off-frame join only after every delivered/reserved owner returns
    /// and all requested terminal outcomes have been consumed.
    pub fn join(mut self) -> Result<(), Self> {
        {
            let q = self.core.lock();
            if q.preparation.is_some() || q.retirement.is_some() {
                drop(q);
                return Err(self);
            }
        }
        self.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        Ok(())
    }
}
impl Drop for CheckpointPreparation {
    fn drop(&mut self) {
        let mut q = self.core.lock();
        q.closed = true;
        q.abandoned = true;
        self.core.wake.notify_all();
    }
}
pub struct PreparationPermit {
    core: Arc<Core>,
    id: PreparationId,
    loaded: Option<LoadedCheckpoint>,
    recipe: Option<Reservation>,
    options: PreparationOptions,
}
impl PreparationPermit {
    pub fn id(&self) -> PreparationId {
        self.id
    }
    /// The builder runs only after recipe admission. It must create its owned
    /// captures here (or borrow separately admitted immutable source inputs);
    /// passing an already uncharged heap through a closure is not admission.
    /// Builder panic still returns the original input for worker disposal.
    pub fn submit<F>(mut self, builder: impl FnOnce() -> F) -> PreparationId
    where
        F: FnOnce() -> checkpoint::Result<HydrationAssets> + Send + 'static,
    {
        let call: FactoryFn = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(builder))
        {
            Ok(factory) => Box::new(factory),
            Err(payload) => {
                drop(payload);
                Box::new(|| Err(error("checkpoint recipe builder panicked")))
            }
        };
        let factory = Factory {
            call: Some(call),
            _lease: self.recipe.take().unwrap(),
        };
        let job = Job {
            loaded: self.loaded.take().unwrap(),
            factory,
            options: self.options,
        };
        let mut q = self.core.lock();
        let entry = q.preparation.as_mut().expect("reserved preparation slot");
        assert_eq!(entry.id, self.id);
        assert!(matches!(entry.state, PrepState::Reserved));
        entry.state = PrepState::Queued(job);
        self.core.wake.notify_all();
        self.id
    }
}
impl Drop for PreparationPermit {
    fn drop(&mut self) {
        if let Some(loaded) = self.loaded.take() {
            // Field/tuple order releases actual bytes before the recipe lease.
            self.core
                .return_disposal(self.id, Box::new((loaded, self.recipe.take())));
        }
    }
}
pub enum PreparationOutcome {
    Ready(PreparedDelivery),
    Failed(checkpoint::CheckpointError),
    Cancelled,
}
enum DeliveryStage {
    Decoded(Admitted<DecodedHydration>),
    Hydrated(Admitted<HydratedEngine>),
    Prepared(Admitted<PreparedContinuation>),
}
#[allow(dead_code)] // Opaque owners are retained solely for worker destruction.
enum DisposalStage {
    Decoded(Admitted<DecodedHydration>),
    Constructed(Admitted<CandidateDisposal>),
}
struct DeliveryDisposal {
    _services: Option<SendContinuationServices>,
    _candidate: DisposalStage,
}
impl Drop for DeliveryDisposal {
    fn drop(&mut self) {
        let services = self._services.take();
        // Preserve the candidate's service lease through arbitrary service
        // destructor panic payload disposal, just as through ordinary Drop.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(services)));
    }
}
/// Host-thread owner. Its reserved return entitlement survives construction,
/// continuation, binding, failure and service shutdown. Drop returns all large
/// owners to the original preparation slot, never to the retiring slot.
pub struct PreparedDelivery {
    core: Arc<Core>,
    id: PreparationId,
    reference: SlotReference,
    stage: Option<DeliveryStage>,
    services: Option<ForwardingServices>,
    timings: PreparationTimings,
}
impl PreparedDelivery {
    /// Coordinator cancellation/shutdown never revokes the return entitlement,
    /// but it prevents any further construction or binding of this generation.
    pub fn is_current(&self) -> bool {
        let q = self.core.lock();
        !q.closed
            && q.preparation.as_ref().is_some_and(|p| {
                p.id == self.id && !p.cancelled && matches!(p.state, PrepState::Delivered)
            })
    }
    pub fn id(&self) -> PreparationId {
        self.id
    }
    pub fn reference(&self) -> &SlotReference {
        &self.reference
    }
    pub fn timings(&self) -> PreparationTimings {
        self.timings
    }
    pub fn decoded(&self) -> Option<&DecodedHydration> {
        match self.stage.as_ref()? {
            DeliveryStage::Decoded(v) => Some(v.value()),
            _ => None,
        }
    }
    pub fn hydrated(&self) -> Option<&HydratedEngine> {
        match self.stage.as_ref()? {
            DeliveryStage::Hydrated(v) => Some(v.value()),
            _ => None,
        }
    }
    pub fn prepared(&self) -> Option<&PreparedContinuation> {
        match self.stage.as_ref()? {
            DeliveryStage::Prepared(v) => Some(v.value()),
            _ => None,
        }
    }
    pub fn construct(&mut self) -> Result<(), SubmitError> {
        if !self.is_current() {
            return Err(SubmitError::Closed);
        }
        if !matches!(self.stage, Some(DeliveryStage::Decoded(_))) {
            return Err(SubmitError::WrongStage);
        }
        let Some(DeliveryStage::Decoded(value)) = self.stage.take() else {
            unreachable!()
        };
        self.stage = Some(DeliveryStage::Hydrated(value.construct()));
        Ok(())
    }
    pub fn prepare_continuation(&mut self) -> checkpoint::Result<()> {
        if !self.is_current() {
            return Err(error("candidate preparation is cancelled or closed"));
        }
        if !matches!(self.stage, Some(DeliveryStage::Hydrated(_))) {
            return Err(error("candidate is not hydrated"));
        }
        let Some(DeliveryStage::Hydrated(value)) = self.stage.take() else {
            unreachable!()
        };
        match value.prepare_continuation_retained() {
            Ok(prepared) => {
                self.stage = Some(DeliveryStage::Prepared(prepared));
                Ok(())
            }
            Err((error, value)) => {
                self.stage = Some(DeliveryStage::Hydrated(value));
                Err(error)
            }
        }
    }
    /// Service construction happens only after the pure continuation has admitted
    /// its disjoint storage. Production factories must be bounded host operations;
    /// off-thread preparation/startup owns expensive IO, probes and devices.
    pub fn bind_services(
        &mut self,
        upper_bytes: usize,
        factory: impl FnOnce(RuntimeGeneration) -> checkpoint::Result<SendContinuationServices>,
    ) -> checkpoint::Result<()> {
        if !self.is_current() {
            return Err(error("candidate preparation is cancelled or closed"));
        }
        if !matches!(self.stage, Some(DeliveryStage::Prepared(_))) {
            return Err(error("candidate is not prepared"));
        }
        let Some(DeliveryStage::Prepared(value)) = self.stage.take() else {
            unreachable!()
        };
        let result = value.bind_services_retained(upper_bytes, |generation| {
            let owners = ForwardingServices::new(
                factory(generation).map_err(checkpoint::CheckpointError::bounded)?,
            );
            let adapters = owners.adapters();
            self.services = Some(owners);
            Ok(adapters)
        });
        match result {
            Ok(value) => {
                self.stage = Some(DeliveryStage::Prepared(value));
                Ok(())
            }
            Err((error, value)) => {
                self.stage = Some(DeliveryStage::Prepared(value));
                Err(error)
            }
        }
    }
}
impl Drop for PreparedDelivery {
    fn drop(&mut self) {
        let Some(stage) = self.stage.take() else {
            return;
        };
        let services = self.services.take().map(ForwardingServices::into_send);
        let candidate = match stage {
            DeliveryStage::Decoded(v) => DisposalStage::Decoded(v),
            DeliveryStage::Hydrated(v) => DisposalStage::Constructed(v.into_disposal()),
            DeliveryStage::Prepared(v) => DisposalStage::Constructed(v.into_disposal()),
        };
        self.core.return_disposal(
            self.id,
            Box::new(DeliveryDisposal {
                _services: services,
                _candidate: candidate,
            }),
        );
    }
}
#[derive(Debug, Clone, Copy)]
pub struct RetirementStatus {
    pub owners_disposed: bool,
    pub released: bool,
    pub disposal_seconds: Option<f64>,
    pub panicked: bool,
}
pub struct RetirementPermit {
    core: Arc<Core>,
    id: PreparationId,
    lease: Option<RetirementLease>,
}
struct RetiredPayload {
    value: Box<dyn Send>,
    lease: RetirementLease,
}
impl RetirementPermit {
    pub fn id(&self) -> PreparationId {
        self.id
    }
    pub fn lease(&self) -> &RetirementLease {
        self.lease.as_ref().unwrap()
    }
    pub fn budget(&self) -> &CheckpointBudget {
        &self.core.budget
    }
    /// The trusted bound includes the complete payload's owned/spare/shared
    /// storage. Caller has already fenced producers and pinned surviving clones.
    pub fn submit<T: Send + 'static>(mut self, value: T) -> PreparationId {
        let payload = RetiredPayload {
            value: Box::new(value),
            lease: self.lease.take().unwrap(),
        };
        let mut q = self.core.lock();
        let entry = q
            .retirement
            .as_mut()
            .expect("retirement permit retains its slot");
        assert_eq!(entry.id, self.id);
        assert!(matches!(entry.state, RetireState::Reserved));
        entry.state = RetireState::Queued(payload);
        self.core.wake.notify_all();
        self.id
    }
}
impl Drop for RetirementPermit {
    fn drop(&mut self) {
        if self.lease.take().is_some() {
            let mut q = self.core.lock();
            let entry = q.retirement.as_mut().expect("reserved retirement slot");
            assert_eq!(entry.id, self.id);
            entry.state = RetireState::AwaitRelease;
            self.core.wake.notify_all();
        }
    }
}
fn error(reason: &str) -> checkpoint::CheckpointError {
    checkpoint::CheckpointError {
        owner: "preparation",
        reason: reason.into(),
    }
}
fn prepare(job: Job) -> WorkerOutcome {
    let started = Instant::now();
    let mut last = started;
    let mut timings = PreparationTimings::default();
    let Job {
        loaded,
        mut factory,
        options,
    } = job;
    let mut observe = |phase| {
        let index = match phase {
            HydrationStage::Assets => 0,
            HydrationStage::Definitions => 1,
            HydrationStage::TypedOwners => 2,
            HydrationStage::RawDisposal => 3,
            HydrationStage::Retention => 4,
            HydrationStage::Construction => unreachable!(),
        };
        let now = Instant::now();
        timings.worker_phases[index] = (now - last).as_secs_f64();
        last = now;
    };
    let decoded = loaded
        .input
        .prepare_hydration(options.assets_upper_bytes)
        .and_then(|p| {
            p.decode_observed(
                || {
                    // Catch inside decode: both the asset and recipe charges still own
                    // arbitrary factory diagnostics and panic payloads at this point.
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        factory.call.take().unwrap(),
                    )) {
                        Ok(result) => result.map_err(checkpoint::CheckpointError::bounded),
                        Err(payload) => {
                            drop(payload);
                            Err(error("checkpoint asset factory panicked"))
                        }
                    }
                },
                options.host_definitions,
                options.generation,
                &mut observe,
            )
        })
        .map_err(checkpoint::CheckpointError::bounded);
    drop(factory);
    timings.worker_total = started.elapsed().as_secs_f64();
    match decoded {
        Ok(value) => WorkerOutcome::Ready(WorkerReady {
            reference: loaded.reference,
            value,
            timings,
        }),
        Err(error) => WorkerOutcome::Failed(error),
    }
}
fn run(core: Arc<Core>) {
    enum Work {
        Prepare(PreparationId, Job),
        Dispose(PreparationId, Box<dyn Send>),
        Retire(PreparationId, RetiredPayload),
    }
    loop {
        let work = {
            let mut q = core.lock();
            loop {
                let abandoned = q.abandoned;
                if let Some(r) = &mut q.retirement {
                    if matches!(r.state, RetireState::AwaitRelease) && r.release.released() {
                        r.state = RetireState::Released;
                    }
                    if matches!(r.state, RetireState::Queued(_)) {
                        let RetireState::Queued(value) =
                            std::mem::replace(&mut r.state, RetireState::Active)
                        else {
                            unreachable!()
                        };
                        break Work::Retire(r.id, value);
                    }
                    if abandoned && matches!(r.state, RetireState::Released) {
                        q.retirement = None;
                    }
                }
                if let Some(p) = &mut q.preparation {
                    if (p.cancelled || abandoned)
                        && matches!(p.state, PrepState::Queued(_) | PrepState::Ready(_))
                    {
                        let old = std::mem::replace(&mut p.state, PrepState::Active);
                        break Work::Dispose(p.id, Box::new(old));
                    }
                    if matches!(p.state, PrepState::Queued(_) | PrepState::Dispose(_)) {
                        let old = std::mem::replace(&mut p.state, PrepState::Active);
                        break match old {
                            PrepState::Queued(job) => Work::Prepare(p.id, job),
                            PrepState::Dispose(value) => Work::Dispose(p.id, value),
                            _ => unreachable!(),
                        };
                    }
                }
                if q.closed && q.preparation.is_none() && q.retirement.is_none() {
                    return;
                }
                q = core
                    .wake
                    .wait_timeout(q, Duration::from_millis(10))
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        };
        match work {
            Work::Prepare(id, job) => {
                let outcome =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| prepare(job)))
                        .unwrap_or_else(|_| {
                            WorkerOutcome::Failed(error("checkpoint preparation panicked"))
                        });
                let mut q = core.lock();
                let entry = q.preparation.as_mut().expect("active preparation retained");
                assert_eq!(entry.id, id);
                entry.state = PrepState::Ready(outcome);
            }
            Work::Dispose(id, value) => {
                let started = Instant::now();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value)));
                let mut q = core.lock();
                if q.abandoned {
                    q.preparation = None;
                } else {
                    let entry = q.preparation.as_mut().expect("disposal retains slot");
                    assert_eq!(entry.id, id);
                    entry.cancelled = false;
                    entry.disposal_seconds = Some(started.elapsed().as_secs_f64());
                    entry.state = PrepState::Ready(WorkerOutcome::Cancelled);
                }
            }
            Work::Retire(id, value) => {
                let started = Instant::now();
                let RetiredPayload { value, lease } = value;
                let panicked =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(value))).is_err();
                drop(lease);
                let mut q = core.lock();
                let entry = q.retirement.as_mut().expect("retirement retains slot");
                assert_eq!(entry.id, id);
                entry.disposal_seconds = Some(started.elapsed().as_secs_f64());
                entry.panicked = panicked;
                entry.state = RetireState::AwaitRelease;
            }
        }
        core.wake.notify_all();
    }
}
