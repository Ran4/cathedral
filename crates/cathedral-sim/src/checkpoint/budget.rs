use super::{CheckpointError, Result};
use std::sync::{Arc, Mutex, Weak};

pub const MAX_RESIDENT_BYTES: usize = 1024 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cohort {
    Running,
    SavePayload,
    LoadCandidate,
    RetiringGeneration,
}

// Four ordinary roots, one persistent Running group, and the old Running
// group while it joins the existing retirement transport. No unbounded map.
const MAX_GROUPS: usize = 6;
#[derive(Debug, Default)]
struct Usage {
    groups: [Option<Group>; MAX_GROUPS],
    next_group: u64,
    promotion: Option<PromotionIds>,
    peak: usize,
}
#[derive(Debug)]
struct Group {
    id: u64,
    cohort: Cohort,
    persistent: bool,
    // A descendant must not wrap a previously used retirement group into a
    // fresh lease and merge a second old world into the same retiring cohort.
    retirement_used: bool,
    bytes: usize,
    owners: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PromotionIds {
    running: u64,
    candidate: u64,
    retirement: u64,
}
impl Usage {
    fn total(&self) -> usize {
        self.groups.iter().flatten().map(|group| group.bytes).sum()
    }
    fn record_peak(&mut self) {
        self.peak = self.peak.max(self.total());
    }
    fn group(&self, id: u64) -> &Group {
        self.groups
            .iter()
            .flatten()
            .find(|g| g.id == id)
            .expect("retained checkpoint allocation group missing")
    }
    fn group_mut(&mut self, id: u64) -> &mut Group {
        self.groups
            .iter_mut()
            .flatten()
            .find(|g| g.id == id)
            .expect("retained checkpoint allocation group missing")
    }
    fn admit(&mut self, bytes: usize) -> Result<()> {
        if bytes == 0 || bytes > MAX_RESIDENT_BYTES.saturating_sub(self.total()) {
            return Err(error(
                "shared resident byte budget exceeded or zero byte charge",
            ));
        }
        Ok(())
    }
    fn insert(&mut self, cohort: Cohort, persistent: bool, bytes: usize) -> Result<u64> {
        self.admit(bytes)?;
        let id = self
            .next_group
            .checked_add(1)
            .ok_or_else(|| error("allocation group identities exhausted"))?;
        let slot = self
            .groups
            .iter_mut()
            .find(|g| g.is_none())
            .ok_or_else(|| error("allocation group capacity exhausted"))?;
        *slot = Some(Group {
            id,
            cohort,
            persistent,
            retirement_used: false,
            bytes,
            owners: 1,
        });
        self.next_group = id;
        self.record_peak();
        Ok(id)
    }
    fn subordinate(&mut self, id: u64, bytes: usize) -> Result<()> {
        self.admit(bytes)?;
        let group = self.group_mut(id);
        group.bytes += bytes;
        group.owners += 1;
        self.record_peak();
        Ok(())
    }
}
fn error(reason: &str) -> CheckpointError {
    CheckpointError::new("admission", reason)
}

/// Pure admission owner. Stable allocation identities, rather than copied
/// cohort labels, follow children through atomic application replacement.
/// Admission always precedes allocation; actual owners drop before leases.
#[derive(Debug, Default)]
pub struct CheckpointBudget {
    usage: Arc<Mutex<Usage>>,
}
impl CheckpointBudget {
    pub fn owns_admitted<T>(&self, owner: &Admitted<T>, cohort: Cohort) -> bool {
        self.owns_reservation(&owner.reservation, cohort)
    }
    pub fn owns_reservation(&self, owner: &Reservation, cohort: Cohort) -> bool {
        Arc::ptr_eq(&self.usage, &owner.usage) && owner.cohort() == cohort
    }
    pub fn owns_complete_input(&self, input: &super::complete::CompleteCheckpointInput) -> bool {
        self.owns_reservation(&input.reservation, Cohort::LoadCandidate)
    }
    pub fn reserve_load_overhead(&self, bytes: usize) -> Result<Reservation> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        let id = usage
            .groups
            .iter()
            .flatten()
            .find(|g| g.cohort == Cohort::LoadCandidate)
            .map(|g| g.id)
            .ok_or_else(|| error("load overhead requires retained candidate"))?;
        usage.subordinate(id, bytes)?;
        Ok(Reservation {
            usage: self.usage.clone(),
            group: id,
            bytes,
        })
    }
    pub fn owns_retirement_lease(&self, owner: &RetirementLease) -> bool {
        self.owns_reservation(&owner.0.reservation, Cohort::RetiringGeneration)
    }
    /// Persistent service and installed-definition storage is disjoint from
    /// replaceable Running authority. This group never follows an old world.
    pub fn reserve_running_overhead(&self, bytes: usize) -> Result<Reservation> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if !usage
            .groups
            .iter()
            .flatten()
            .any(|g| g.cohort == Cohort::Running)
        {
            return Err(error(
                "persistent overhead requires retained Running authority",
            ));
        }
        let existing = usage
            .groups
            .iter()
            .flatten()
            .find(|g| g.persistent)
            .map(|g| g.id);
        let id = if let Some(id) = existing {
            usage.subordinate(id, bytes)?;
            id
        } else {
            usage.insert(Cohort::Running, true, bytes)?
        };
        Ok(Reservation {
            usage: self.usage.clone(),
            group: id,
            bytes,
        })
    }
    pub fn reserve(&self, cohort: Cohort, bytes: usize) -> Result<Reservation> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if usage.groups.iter().flatten().any(|g| g.cohort == cohort) {
            return Err(error("cohort already retained"));
        }
        let id = usage.insert(cohort, false, bytes)?;
        Ok(Reservation {
            usage: self.usage.clone(),
            group: id,
            bytes,
        })
    }
    /// Prepare the sole atomic role transition without changing any charge or
    /// fencing any producer. The borrowed live root stays in its live bundle
    /// even if the ticket is simply dropped. The candidate cannot disappear or
    /// resize its root until this ticket has committed or been dropped.
    ///
    /// The retiring lease must be the *newly admitted* transport, with no
    /// previously attached world. This cannot merge another retired generation.
    /// A caller must preflight the complete owned application before commit.
    pub fn prepare_promotion<'a>(
        &self,
        running: &'a mut Option<Reservation>,
        candidate: &'a Reservation,
        retirement: &RetirementLease,
    ) -> Result<PromotionPermit<'a>> {
        let root = running
            .as_ref()
            .ok_or_else(|| error("missing live Running root"))?;
        root.require_shared(candidate)?;
        root.require_shared(&retirement.0.reservation)?;
        if !Arc::ptr_eq(&self.usage, &root.usage) {
            return Err(error("promotion belongs to another shared budget"));
        }
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if usage.promotion.is_some() {
            return Err(error("another promotion already prepared"));
        }
        let old = usage.group(root.group);
        if old.cohort != Cohort::Running
            || old.persistent
            || usage.group(candidate.group).cohort != Cohort::LoadCandidate
            || usage.group(retirement.0.reservation.group).cohort != Cohort::RetiringGeneration
            || usage.group(retirement.0.reservation.group).retirement_used
            || usage.groups.iter().flatten().any(|g| {
                g.cohort == Cohort::RetiringGeneration && g.id != retirement.0.reservation.group
            })
            || retirement
                .0
                .running
                .lock()
                .expect("retirement ownership lock poisoned")
                .is_some()
        {
            return Err(error(
                "promotion requires intact Running, candidate and empty retirement transport",
            ));
        }
        let ids = PromotionIds {
            running: root.group,
            candidate: candidate.group,
            retirement: retirement.0.reservation.group,
        };
        usage.promotion = Some(ids);
        drop(usage);
        Ok(PromotionPermit {
            usage: self.usage.clone(),
            running,
            candidate,
            retirement: retirement.clone(),
            ids,
            committed: false,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .total()
    }
    pub fn cohort_bytes(&self, cohort: Cohort) -> usize {
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .groups
            .iter()
            .flatten()
            .filter(|g| g.cohort == cohort)
            .map(|g| g.bytes)
            .sum()
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .peak
    }
}

/// Before commit the live application still owns its root. Ordinary Drop is a
/// cancellation with no ownership loss. Commit contains only exhaustive moves
/// and two role changes under the admission mutex; it cannot refuse.
pub struct PromotionPermit<'a> {
    usage: Arc<Mutex<Usage>>,
    running: &'a mut Option<Reservation>,
    candidate: &'a Reservation,
    retirement: RetirementLease,
    ids: PromotionIds,
    committed: bool,
}
impl PromotionPermit<'_> {
    pub fn commit(mut self) {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        assert_eq!(usage.promotion, Some(self.ids));
        // Both borrowed roots and the cloned retirement Arc keep these stable
        // groups alive. Children may resize/drop concurrently under this lock.
        usage.group_mut(self.ids.running).cohort = Cohort::RetiringGeneration;
        usage.group_mut(self.ids.running).retirement_used = true;
        usage.group_mut(self.ids.retirement).retirement_used = true;
        usage.group_mut(self.candidate.group).cohort = Cohort::Running;
        *self
            .retirement
            .0
            .running
            .lock()
            .expect("retirement ownership lock poisoned") = self.running.take();
        self.retirement
            .0
            .groups
            .lock()
            .expect("retirement group lock poisoned")[1] = self.ids.running;
        usage.promotion = None;
        self.committed = true;
    }
}
impl Drop for PromotionPermit<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let mut usage = self
                .usage
                .lock()
                .expect("checkpoint admission lock poisoned");
            assert_eq!(usage.promotion, Some(self.ids));
            usage.promotion = None;
        }
    }
}

#[derive(Debug)]
pub struct Reservation {
    usage: Arc<Mutex<Usage>>,
    group: u64,
    bytes: usize,
}
impl Reservation {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn cohort(&self) -> Cohort {
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .group(self.group)
            .cohort
    }
    /// Growth precedes allocation; shrink follows destruction. Resolve the
    /// group and debit it under the same lock as promotion, avoiding an ABA
    /// debit of whichever new world now happens to occupy Running.
    pub fn resize(&mut self, bytes: usize) -> Result<()> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if bytes == 0 || bytes > MAX_RESIDENT_BYTES - (usage.total() - self.bytes) {
            return Err(error("shared resident byte budget exceeded"));
        }
        let group = usage.group_mut(self.group);
        group.bytes = group.bytes - self.bytes + bytes;
        self.bytes = bytes;
        usage.record_peak();
        Ok(())
    }
    pub(crate) fn sublease(&self, bytes: usize) -> Result<Self> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        usage.subordinate(self.group, bytes)?;
        Ok(Self {
            usage: self.usage.clone(),
            group: self.group,
            bytes,
        })
    }
    pub(crate) fn require_shared(&self, other: &Self) -> Result<()> {
        if !Arc::ptr_eq(&self.usage, &other.usage) {
            return Err(error("complete owners require the same shared budget"));
        }
        Ok(())
    }
    pub(crate) fn require_running(&self, bytes: usize) -> Result<()> {
        let usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if usage
            .groups
            .iter()
            .flatten()
            .filter(|g| g.cohort == Cohort::Running)
            .map(|g| g.bytes)
            .sum::<usize>()
            < bytes
        {
            return Err(error(
                "complete checkpoint requires its shared Running authority allowance",
            ));
        }
        Ok(())
    }
    pub(crate) fn require(&self, cohort: Cohort, bytes: usize) -> Result<()> {
        if self.cohort() != cohort || self.bytes < bytes {
            Err(error("owner working allocation was not reserved"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        let slot = usage
            .groups
            .iter_mut()
            .find(|g| g.as_ref().is_some_and(|g| g.id == self.group))
            .expect("retained checkpoint allocation group missing");
        let group = slot.as_mut().unwrap();
        group.bytes -= self.bytes;
        group.owners -= 1;
        if group.owners == 0 {
            *slot = None;
        }
    }
}

/// A single retirement transport charge shared by its actual lifetime owners.
/// Atomic adoption attaches the old live root to this same pin. Last endpoint,
/// job, queued publication or worker owner releases it, never an earlier handle.
#[derive(Debug)]
struct RetirementOwner {
    running: Mutex<Option<Reservation>>,
    groups: Arc<Mutex<[u64; 2]>>,
    reservation: Reservation,
}
#[derive(Debug, Clone)]
pub struct RetirementLease(Arc<RetirementOwner>);
#[derive(Debug, Clone)]
pub struct RetirementRelease {
    owner: Weak<RetirementOwner>,
    usage: Weak<Mutex<Usage>>,
    groups: Arc<Mutex<[u64; 2]>>,
}
impl RetirementLease {
    pub fn new(reservation: Reservation) -> Result<Self> {
        reservation.require(Cohort::RetiringGeneration, 1)?;
        let groups = Arc::new(Mutex::new([reservation.group, 0]));
        Ok(Self(Arc::new(RetirementOwner {
            running: Mutex::new(None),
            groups,
            reservation,
        })))
    }
    pub fn bytes(&self) -> usize {
        self.0.reservation.bytes()
            + self
                .0
                .running
                .lock()
                .expect("retirement ownership lock poisoned")
                .as_ref()
                .map_or(0, Reservation::bytes)
    }
    pub fn release_observer(&self) -> RetirementRelease {
        RetirementRelease {
            owner: Arc::downgrade(&self.0),
            usage: Arc::downgrade(&self.0.reservation.usage),
            groups: self.0.groups.clone(),
        }
    }
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl RetirementRelease {
    pub fn released(&self) -> bool {
        if self.owner.strong_count() != 0 {
            return false;
        }
        let Some(shared) = self.usage.upgrade() else {
            return true;
        };
        let usage = shared.lock().expect("checkpoint admission lock poisoned");
        let groups = self.groups.lock().expect("retirement group lock poisoned");
        !usage
            .groups
            .iter()
            .flatten()
            .any(|g| groups.contains(&g.id))
    }
}

/// A retained component owns its charge. There is no Clone or extraction API:
/// a caller cannot release the cohort while retaining the admitted allocation.
/// Send values can travel to a worker with the same charge. M3 still owns the
/// actual coordinator and must include all other retained owners in its budget.
#[derive(Debug)]
pub struct Admitted<T> {
    // Field order drops the allocation before releasing its admission.
    value: T,
    reservation: Reservation,
}
impl<T> Admitted<T> {
    /// Keep the candidate opaque while preparing the admission half of an
    /// application swap. This borrows its existing lease rather than exposing
    /// or extracting its value; dropping the permit leaves every role intact.
    pub fn prepare_promotion<'a>(
        &'a self,
        budget: &CheckpointBudget,
        running: &'a mut Option<Reservation>,
        retirement: &RetirementLease,
    ) -> Result<PromotionPermit<'a>> {
        budget.prepare_promotion(running, &self.reservation, retirement)
    }

    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn reserved_bytes(&self) -> usize {
        self.reservation.bytes
    }
    pub(crate) fn new(value: T, reservation: Reservation) -> Self {
        Self { value, reservation }
    }
    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> Admitted<U> {
        Admitted::new(f(self.value), self.reservation)
    }
    pub(crate) fn try_map_retained<U>(
        mut self,
        f: impl FnOnce(T, &mut Reservation) -> std::result::Result<U, (CheckpointError, T)>,
    ) -> std::result::Result<Admitted<U>, (CheckpointError, Admitted<T>)> {
        match f(self.value, &mut self.reservation) {
            Ok(value) => Ok(Admitted::new(value, self.reservation)),
            Err((error, value)) => Err((error, Admitted::new(value, self.reservation))),
        }
    }
    pub(crate) fn try_map_mut<U>(
        mut self,
        f: impl FnOnce(T, &mut Reservation) -> Result<U>,
    ) -> Result<Admitted<U>> {
        // The moved value and all closure-local allocations are disposed before
        // the still-owned reservation, on success, error and unwind.
        let value = f(self.value, &mut self.reservation)?;
        Ok(Admitted::new(value, self.reservation))
    }
    pub(crate) fn try_map<U>(
        self,
        f: impl FnOnce(T, &Reservation) -> Result<U>,
    ) -> Result<Admitted<U>> {
        let value = f(self.value, &self.reservation)?;
        Ok(Admitted::new(value, self.reservation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ObservedAllocation {
        budget: Arc<CheckpointBudget>,
        cohort: Cohort,
        minimum: usize,
        disposed: Arc<AtomicUsize>,
        // The real backing storage dies while its external admission is live.
        _bytes: Vec<u8>,
    }
    impl Drop for ObservedAllocation {
        fn drop(&mut self) {
            assert!(self.budget.cohort_bytes(self.cohort) >= self.minimum);
            self.disposed.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn opaque_admitted_owner_promotes_and_keeps_actual_payload_charged_through_drop() {
        let budget = Arc::new(CheckpointBudget::default());
        let mut running = Some(budget.reserve(Cohort::Running, 4096).unwrap());
        let persistent = budget.reserve_running_overhead(2048).unwrap();
        let disposed = Arc::new(AtomicUsize::new(0));
        let reservation = budget.reserve(Cohort::LoadCandidate, 8192).unwrap();
        let candidate = Admitted::new(
            ObservedAllocation {
                budget: budget.clone(),
                cohort: Cohort::Running,
                minimum: 8192,
                disposed: disposed.clone(),
                _bytes: vec![0; 8192],
            },
            reservation,
        );
        let retirement =
            RetirementLease::new(budget.reserve(Cohort::RetiringGeneration, 1024).unwrap())
                .unwrap();
        candidate
            .prepare_promotion(&budget, &mut running, &retirement)
            .unwrap()
            .commit();
        assert!(running.is_none());
        assert_eq!(retirement.bytes(), 5120);
        assert!(budget.owns_admitted(&candidate, Cohort::Running));
        assert_eq!(budget.cohort_bytes(Cohort::Running), 10240);
        assert_eq!(budget.cohort_bytes(Cohort::LoadCandidate), 0);
        drop(candidate);
        assert_eq!(disposed.load(Ordering::SeqCst), 1);
        assert_eq!(budget.cohort_bytes(Cohort::Running), persistent.bytes());
        drop(retirement);
        drop(persistent);
        assert_eq!(budget.retained_bytes(), 0);
    }

    #[test]
    fn promotion_retirement_observation_tracks_children_without_pinning_them() {
        let budget = CheckpointBudget::default();
        let mut running = Some(budget.reserve(Cohort::Running, 4096).unwrap());
        let old_child = running.as_ref().unwrap().sublease(2048).unwrap();
        let candidate = budget.reserve(Cohort::LoadCandidate, 8192).unwrap();
        let retirement =
            RetirementLease::new(budget.reserve(Cohort::RetiringGeneration, 1024).unwrap())
                .unwrap();
        let observer = retirement.release_observer();
        budget
            .prepare_promotion(&mut running, &candidate, &retirement)
            .unwrap()
            .commit();
        drop(retirement);
        assert!(!observer.released());
        assert_eq!(old_child.cohort(), Cohort::RetiringGeneration);
        assert_eq!(budget.cohort_bytes(Cohort::RetiringGeneration), 2048);
        drop(old_child);
        assert!(observer.released());
        assert_eq!(budget.cohort_bytes(Cohort::RetiringGeneration), 0);
        drop(candidate);
        assert_eq!(budget.retained_bytes(), 0);
        // Weak observation of the old unique group cannot confuse a new root.
        let new_retiring = budget.reserve(Cohort::RetiringGeneration, 4096).unwrap();
        assert!(observer.released());
        drop(new_retiring);
    }

    #[test]
    fn previously_used_retirement_group_cannot_be_rewrapped_for_another_world() {
        for retain_control_child in [false, true] {
            let budget = CheckpointBudget::default();
            let mut running = Some(budget.reserve(Cohort::Running, 4096).unwrap());
            let retired_root = budget.reserve(Cohort::RetiringGeneration, 1024).unwrap();
            let survivor = if retain_control_child {
                retired_root.sublease(256).unwrap()
            } else {
                running.as_ref().unwrap().sublease(256).unwrap()
            };
            let retirement = RetirementLease::new(retired_root).unwrap();
            let candidate = budget.reserve(Cohort::LoadCandidate, 8192).unwrap();
            budget
                .prepare_promotion(&mut running, &candidate, &retirement)
                .unwrap()
                .commit();
            drop(retirement);
            let rewrapped = RetirementLease::new(survivor).unwrap();
            let next = budget.reserve(Cohort::LoadCandidate, 8192).unwrap();
            let mut running = Some(candidate);
            let before = budget.retained_bytes();
            assert!(
                budget
                    .prepare_promotion(&mut running, &next, &rewrapped)
                    .is_err()
            );
            assert_eq!(budget.retained_bytes(), before);
            assert_eq!(running.as_ref().unwrap().cohort(), Cohort::Running);
            drop((running, next, rewrapped));
            assert_eq!(budget.retained_bytes(), 0);
        }
    }
}
