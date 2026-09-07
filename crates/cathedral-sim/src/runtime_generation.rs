//! Ephemeral execution identity. It is independent of durable operations and
//! actor presence epochs, and is never restored from a saved world.

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct RuntimeGeneration(pub u64);

impl RuntimeGeneration {
    /// Isolated fixtures and the single-world headless runner use generation 0.
    pub const INITIAL: Self = Self(0);

    pub const fn successor(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub fn bind<T>(self, value: T) -> RuntimeEnvelope<T> {
        RuntimeEnvelope {
            generation: self,
            value,
        }
    }
}

/// Created at production/submission, never retagged when an outcome arrives.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEnvelope<T> {
    pub generation: RuntimeGeneration,
    pub value: T,
}

impl<T> RuntimeEnvelope<T> {
    pub fn into_current(self, current: RuntimeGeneration) -> Option<T> {
        (self.generation == current).then_some(self.value)
    }
}
