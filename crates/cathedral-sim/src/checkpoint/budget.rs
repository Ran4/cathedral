use super::{CheckpointError, Result};
use std::sync::{Arc, Mutex};

pub const MAX_RESIDENT_BYTES: usize = 1024 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cohort {
    Running,
    SavePayload,
    LoadCandidate,
    RetiringGeneration,
}
impl Cohort {
    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Default)]
struct Usage {
    cohorts: [Option<Slot>; 4],
    peak: usize,
}
#[derive(Debug)]
struct Slot {
    bytes: usize,
    owners: usize,
}
impl Usage {
    fn total(&self) -> usize {
        self.cohorts.iter().flatten().map(|slot| slot.bytes).sum()
    }
    fn record_peak(&mut self) {
        self.peak = self.peak.max(self.total());
    }
}

/// Pure admission owner. The host reserves conservative peak
/// bytes before extraction/decoding/staging; cancellation releases only when the
/// actual retained owner is dropped. A reservation cannot be cloned or copied.
#[derive(Debug, Default)]
pub struct CheckpointBudget {
    usage: Arc<Mutex<Usage>>,
}
impl CheckpointBudget {
    pub fn reserve(&self, cohort: Cohort, bytes: usize) -> Result<Reservation> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if bytes == 0 || usage.cohorts[cohort.index()].is_some() {
            return Err(CheckpointError::new(
                "admission",
                "cohort already retained or zero byte charge",
            ));
        }
        if bytes > MAX_RESIDENT_BYTES.saturating_sub(usage.total()) {
            return Err(CheckpointError::new(
                "admission",
                "shared resident byte budget exceeded",
            ));
        }
        usage.cohorts[cohort.index()] = Some(Slot { bytes, owners: 1 });
        usage.record_peak();
        Ok(Reservation {
            usage: self.usage.clone(),
            cohort,
            bytes,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .total()
    }
    pub fn peak_retained_bytes(&self) -> usize {
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .peak
    }
}

#[derive(Debug)]
pub struct Reservation {
    usage: Arc<Mutex<Usage>>,
    cohort: Cohort,
    bytes: usize,
}
impl Reservation {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn cohort(&self) -> Cohort {
        self.cohort
    }
    /// Growth must precede new allocations; shrink only after disposing them.
    pub fn resize(&mut self, bytes: usize) -> Result<()> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if bytes == 0 || bytes > MAX_RESIDENT_BYTES - (usage.total() - self.bytes) {
            return Err(CheckpointError::new(
                "admission",
                "shared resident byte budget exceeded",
            ));
        }
        let slot = usage.cohorts[self.cohort.index()]
            .as_mut()
            .expect("retained checkpoint cohort missing");
        slot.bytes = slot.bytes - self.bytes + bytes;
        self.bytes = bytes;
        usage.record_peak();
        Ok(())
    }
    /// A private disjoint allocation lease in this same cohort. This is not a
    /// cloned charge: its bytes join the shared total before its owner allocates.
    /// Dropping the coordinator cannot release a surviving child's cohort slot.
    pub(crate) fn sublease(&self, bytes: usize) -> Result<Self> {
        let mut usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if bytes == 0 || bytes > MAX_RESIDENT_BYTES.saturating_sub(usage.total()) {
            return Err(CheckpointError::new(
                "admission",
                "shared resident byte budget exceeded",
            ));
        }
        let slot = usage.cohorts[self.cohort.index()]
            .as_mut()
            .expect("retained checkpoint cohort missing");
        slot.bytes += bytes;
        slot.owners += 1;
        usage.record_peak();
        Ok(Self {
            usage: self.usage.clone(),
            cohort: self.cohort,
            bytes,
        })
    }
    pub(crate) fn require_running(&self, bytes: usize) -> Result<()> {
        let usage = self
            .usage
            .lock()
            .expect("checkpoint admission lock poisoned");
        if usage.cohorts[Cohort::Running.index()]
            .as_ref()
            .is_none_or(|slot| slot.bytes < bytes)
        {
            return Err(CheckpointError::new(
                "admission",
                "complete checkpoint requires its shared Running authority allowance",
            ));
        }
        Ok(())
    }
    pub(crate) fn require(&self, cohort: Cohort, bytes: usize) -> Result<()> {
        if self.cohort != cohort || self.bytes < bytes {
            Err(CheckpointError::new(
                "admission",
                "owner working allocation was not reserved",
            ))
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
        let slot = usage.cohorts[self.cohort.index()]
            .as_mut()
            .expect("retained checkpoint cohort missing");
        slot.bytes -= self.bytes;
        slot.owners -= 1;
        if slot.owners == 0 {
            usage.cohorts[self.cohort.index()] = None;
        }
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
    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn reserved_bytes(&self) -> usize {
        self.reservation.bytes
    }
    pub(crate) fn new(value: T, reservation: Reservation) -> Self {
        Self { value, reservation }
    }
    pub(crate) fn try_map<U>(
        self,
        f: impl FnOnce(T, &Reservation) -> Result<U>,
    ) -> Result<Admitted<U>> {
        let value = f(self.value, &self.reservation)?;
        Ok(Admitted::new(value, self.reservation))
    }
}
