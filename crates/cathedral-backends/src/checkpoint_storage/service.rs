use super::*;
use cathedral_sim::checkpoint::{
    Admitted, CheckpointBudget, Cohort, Reservation, complete::CompleteCheckpointCandidate,
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError {
    Full,
    Closed,
    Unknown,
    WrongTurn,
    WrongBudget,
    TooLate,
    Exhausted,
}
type Payload = Admitted<CompleteCheckpointCandidate>;
enum Request {
    Save,
    Load(LoadSelection),
    Recover,
}
enum State {
    Waiting,
    Ready(Payload, SaveMetadata),
    Active,
    Done(Outcome),
}
struct Entry {
    id: OperationId,
    slot: SlotId,
    request: Request,
    state: State,
    cancelled: bool,
}
struct Queue {
    entries: VecDeque<Entry>,
    next: u64,
    closed: bool,
    abandoned: bool,
}
struct Core {
    queue: Mutex<Queue>,
    wake: Condvar,
    budget: Arc<CheckpointBudget>,
    identity: [u8; 16],
    // Last field: all retained control/results are destroyed before their charge.
    _overhead: Reservation,
}

/// Eight admitted intents INCLUDING unread terminal results. Only the oldest
/// outstanding save intent can receive a payload; all file work is serialized.
/// Host calls move owners or bounded metadata and never wait for disk or a join.
/// Caller-owned returned load bytes need M3b's later worker disposal policy.
pub struct CheckpointStorage {
    core: Arc<Core>,
    worker: Option<JoinHandle<()>>,
}
impl CheckpointStorage {
    /// Startup is an off-frame action (OS entropy and thread construction).
    /// The directory must already exist; opening/locking it runs on the worker.
    /// Admission precedes service/control allocations and the explicit 2 MiB stack.
    pub fn start(path: &Path, budget: Arc<CheckpointBudget>) -> Result<Self, StorageError> {
        Self::start_inner(
            path,
            budget,
            #[cfg(test)]
            None,
        )
    }
    fn start_inner(
        path: &Path,
        budget: Arc<CheckpointBudget>,
        #[cfg(test)] hook: Option<disk::Hook>,
    ) -> Result<Self, StorageError> {
        use std::io::Read;
        if path.as_os_str().len() > 4096 {
            return Err(StorageError::invalid("store root path exceeds bound"));
        }
        let overhead = budget
            .reserve_running_overhead(SERVICE_ALLOWANCE_BYTES)
            .map_err(|_| StorageError::admission("storage service admission refused"))?;
        let mut identity = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut identity))
            .map_err(|e| StorageError::io(Phase::Open, e))?;
        if identity == [0; 16] {
            return Err(StorageError::invalid("operation identity unavailable"));
        }
        let core = Arc::new(Core {
            queue: Mutex::new(Queue {
                entries: VecDeque::with_capacity(OPERATION_CAPACITY),
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
        let root = path.to_owned();
        let worker = std::thread::Builder::new()
            .name("checkpoint-storage".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                run(
                    worker_core,
                    root,
                    #[cfg(test)]
                    hook,
                )
            })
            .map_err(|e| StorageError::io(Phase::Open, e))?;
        Ok(Self {
            core,
            worker: Some(worker),
        })
    }
    #[cfg(test)]
    pub(super) fn start_hooked(
        path: &Path,
        budget: Arc<CheckpointBudget>,
        hook: disk::Hook,
    ) -> Result<Self, StorageError> {
        Self::start_inner(path, budget, Some(hook))
    }
    #[cfg(test)]
    pub(super) fn set_next_for_test(&self, value: u64) {
        self.core.queue.lock().unwrap().next = value;
    }
    pub fn request_save(&self, slot: SlotId) -> Result<OperationId, SubmitError> {
        self.submit(slot, Request::Save)
    }
    pub fn request_load(
        &self,
        slot: SlotId,
        selection: LoadSelection,
    ) -> Result<OperationId, SubmitError> {
        self.submit(slot, Request::Load(selection))
    }
    /// Explicitly recover the journal's acknowledged predecessor, or the valid
    /// previous reference when active is damaged. Never promotes an orphan.
    pub fn request_recovery(&self, slot: SlotId) -> Result<OperationId, SubmitError> {
        self.submit(slot, Request::Recover)
    }
    fn submit(&self, slot: SlotId, request: Request) -> Result<OperationId, SubmitError> {
        let mut q = self.core.queue.lock().expect("storage queue");
        if q.closed {
            return Err(SubmitError::Closed);
        }
        if q.entries.len() == OPERATION_CAPACITY {
            return Err(SubmitError::Full);
        }
        let next = q.next.checked_add(1).ok_or(SubmitError::Exhausted)?;
        let id = OperationId {
            service: self.core.identity,
            sequence: q.next,
        };
        q.next = next;
        q.entries.push_back(Entry {
            id,
            slot,
            request,
            state: State::Waiting,
            cancelled: false,
        });
        self.core.wake.notify_one();
        Ok(id)
    }
    /// Capture only when this returns an ID. Later saves retain just their small
    /// manual intent; callers must not pre-capture them with independent budgets.
    pub fn next_capture(&self) -> Option<OperationId> {
        let q = self.core.queue.lock().expect("storage queue");
        if q.closed {
            return None;
        }
        q.entries
            .iter()
            .find(|e| !matches!(e.state, State::Done(_)))
            .filter(|e| {
                matches!(e.request, Request::Save)
                    && matches!(e.state, State::Waiting)
                    && !e.cancelled
            })
            .map(|e| e.id)
    }
    /// Every refusal returns the actual owner and its capture metadata; no extraction, cloning or
    /// premature release of the caller's admitted payload occurs.
    /// Metadata must describe the actual capture, not the earlier intent time.
    pub fn attach(
        &self,
        id: OperationId,
        payload: Payload,
        metadata: SaveMetadata,
    ) -> Result<(), (SubmitError, Payload, SaveMetadata)> {
        if !self
            .core
            .budget
            .owns_admitted(&payload, Cohort::SavePayload)
        {
            return Err((SubmitError::WrongBudget, payload, metadata));
        }
        let mut q = self.core.queue.lock().expect("storage queue");
        if q.closed {
            return Err((SubmitError::Closed, payload, metadata));
        }
        let Some(entry) = q
            .entries
            .iter_mut()
            .find(|e| !matches!(e.state, State::Done(_)))
        else {
            return Err((SubmitError::Unknown, payload, metadata));
        };
        if entry.id != id {
            return Err((SubmitError::WrongTurn, payload, metadata));
        }
        if !matches!(entry.request, Request::Save)
            || !matches!(entry.state, State::Waiting)
            || entry.cancelled
        {
            return Err((SubmitError::TooLate, payload, metadata));
        }
        entry.state = State::Ready(payload, metadata);
        self.core.wake.notify_one();
        Ok(())
    }
    /// Cancellation before the worker starts is terminal Cancelled. Once the
    /// worker owns the operation, cancellation is too late and its honest disk
    /// success/failure still arrives. It never pretends a published save vanished.
    pub fn cancel(&self, id: OperationId) -> Result<(), SubmitError> {
        cancel(&self.core, id)
    }
    pub fn take_result(&self, id: OperationId) -> Option<Outcome> {
        take(&self.core, id)
    }
    /// Asynchronous disposal for an unread terminal load; its charge and slot
    /// remain retained until the worker actually drops the input.
    pub fn discard_result(&self, id: OperationId) -> Result<(), SubmitError> {
        let mut q = self.core.queue.lock().expect("storage queue");
        let e = q
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(SubmitError::Unknown)?;
        if !matches!(e.state, State::Done(_)) {
            return Err(SubmitError::TooLate);
        }
        e.cancelled = true;
        self.core.wake.notify_one();
        Ok(())
    }
    pub fn retained_operations(&self) -> usize {
        self.core.queue.lock().expect("storage queue").entries.len()
    }
    /// Nonblocking stop. Queued work cancels, in-flight work finishes, and all
    /// terminal results remain accessible through the returned shutdown handle.
    pub fn shutdown(mut self) -> Shutdown {
        close(&self.core, false);
        Shutdown {
            core: Arc::clone(&self.core),
            worker: self.worker.take(),
        }
    }
}
impl Drop for CheckpointStorage {
    fn drop(&mut self) {
        if self.worker.is_some() {
            close(&self.core, true);
        }
        // Dropping JoinHandle detaches; slow filesystem calls cannot be killed
        // safely. Its directory lock and all leases survive until actual exit.
    }
}

pub struct Shutdown {
    core: Arc<Core>,
    worker: Option<JoinHandle<()>>,
}
impl Shutdown {
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn take_result(&self, id: OperationId) -> Option<Outcome> {
        take(&self.core, id)
    }
    /// Explicit off-frame join; never called from a destructor. Drain every
    /// terminal first: unread results deliberately keep the worker alive so a
    /// dropped shutdown handle can dispose them on that worker, not the host.
    pub fn join(mut self) -> Self {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self
    }
}
impl Drop for Shutdown {
    fn drop(&mut self) {
        close(&self.core, true);
    }
}
fn close(core: &Core, abandoned: bool) {
    let mut q = core.queue.lock().expect("storage queue");
    q.closed = true;
    q.abandoned |= abandoned;
    core.wake.notify_one();
}
fn cancel(core: &Core, id: OperationId) -> Result<(), SubmitError> {
    let mut q = core.queue.lock().expect("storage queue");
    let entry = q
        .entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or(SubmitError::Unknown)?;
    if matches!(entry.state, State::Active | State::Done(_)) {
        return Err(SubmitError::TooLate);
    }
    entry.cancelled = true;
    core.wake.notify_one();
    Ok(())
}
fn take(core: &Core, id: OperationId) -> Option<Outcome> {
    let mut q = core.queue.lock().expect("storage queue");
    let index = q
        .entries
        .iter()
        .position(|e| e.id == id && matches!(e.state, State::Done(_)) && !e.cancelled)?;
    let e = q.entries.remove(index)?;
    core.wake.notify_one();
    if let State::Done(outcome) = e.state {
        Some(outcome)
    } else {
        unreachable!()
    }
}
fn run(core: Arc<Core>, root: PathBuf, #[cfg(test)] hook: Option<disk::Hook>) {
    let mut store = disk::Store::open(&root);
    #[cfg(test)]
    if let Ok(store) = &mut store {
        store.hook = hook;
    }
    loop {
        let mut q = core.queue.lock().expect("storage queue");
        // Dispose abandoned terminal owners on this worker, outside the mutex.
        if let Some(index) = q
            .entries
            .iter()
            .position(|e| matches!(e.state, State::Done(_)) && (q.abandoned || e.cancelled))
        {
            let entry = q.entries.remove(index).unwrap();
            drop(q);
            drop(entry);
            continue;
        }
        let Some(index) = q
            .entries
            .iter()
            .position(|e| !matches!(e.state, State::Done(_)))
        else {
            if q.closed && q.entries.is_empty() {
                return;
            }
            drop(core.wake.wait(q).expect("storage queue"));
            continue;
        };
        let cancelled = q.closed || q.entries[index].cancelled;
        if !cancelled
            && matches!(q.entries[index].request, Request::Save)
            && matches!(q.entries[index].state, State::Waiting)
            && store.is_ok()
        {
            drop(core.wake.wait(q).expect("storage queue"));
            continue;
        }
        let entry = &mut q.entries[index];
        let id = entry.id;
        let slot = entry.slot.clone();
        let state = std::mem::replace(&mut entry.state, State::Active);
        let request = match &entry.request {
            Request::Save => Request::Save,
            Request::Load(s) => Request::Load(*s),
            Request::Recover => Request::Recover,
        };
        drop(q);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if cancelled {
                Outcome::Cancelled
            } else {
                match &mut store {
                    Err(e) => Outcome::Failed(e.clone()),
                    Ok(store) => match request {
                        Request::Save => match &state {
                            State::Ready(payload, metadata) => {
                                match store.save(&slot, id, metadata.clone(), payload) {
                                    Ok(reference) => Outcome::Saved(reference),
                                    Err(e) => Outcome::Failed(e),
                                }
                            }
                            _ => unreachable!("worker only starts a ready save"),
                        },
                        Request::Load(selection) => {
                            match store.load(&slot, selection, &core.budget) {
                                Ok(loaded) => Outcome::Loaded(loaded),
                                Err(e) => Outcome::Failed(e),
                            }
                        }
                        Request::Recover => match store.recover(&slot, id) {
                            Ok(reference) => Outcome::Recovered(reference),
                            Err(e) => Outcome::Failed(e),
                        },
                    },
                }
            }
        }))
        .unwrap_or_else(|_| {
            Outcome::Failed(StorageError {
                phase: Phase::Worker,
                kind: std::io::ErrorKind::Other,
                message: "checkpoint worker operation panicked; inspect/recover the slot",
            })
        });
        drop(state); // Payload destruction remains on the worker, with its lease.
        let mut q = core.queue.lock().expect("storage queue");
        let entry = q
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .expect("reserved terminal slot");
        entry.cancelled = false;
        entry.state = State::Done(outcome);
        core.wake.notify_all();
    }
}
