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
    cohorts: [Option<usize>; 4],
}
impl Usage {
    fn total(&self) -> usize {
        self.cohorts.iter().flatten().sum()
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
        usage.cohorts[cohort.index()] = Some(bytes);
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
        usage.cohorts[self.cohort.index()] = Some(bytes);
        self.bytes = bytes;
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
        self.usage
            .lock()
            .expect("checkpoint admission lock poisoned")
            .cohorts[self.cohort.index()] = None;
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
