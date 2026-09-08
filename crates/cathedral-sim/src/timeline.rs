//! Time admission and transaction boundaries. No clocks are read here.
//!
//! A host origin is disposable; `AcceptedTime` is a logical continuation value.
//! Only admitted elapsed duration reaches physics and the calendar. Ordinary
//! wall debt survives a checkpoint separately from either fixed-step residual.
use serde::{Deserialize, Deserializer, Serialize};
use std::time::Duration;

/// Roughly twelve 120 Hz controller steps (plus the existing fixed residual)
/// and two 20 Hz NPC steps per recovery frame.
pub const MAX_ACCEPTED_FRAME: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptedTime {
    pub elapsed: Duration,
    pub debt: Duration,
    /// Wall duration observed in this timeline, excluding offline/load time.
    pub wall: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedFrame {
    pub wall_delta: Duration,
    pub accepted_delta: Duration,
    pub elapsed: Duration,
    pub debt: Duration,
}

impl AcceptedTime {
    /// No automatic suspension/focus exception: every supplied duration is
    /// ordinary live time. Loading rebinds the host origin without calling this.
    pub fn admit(&mut self, wall_delta: Duration) -> AcceptedFrame {
        self.wall = self
            .wall
            .checked_add(wall_delta)
            .expect("wall duration overflow");
        self.debt = self
            .debt
            .checked_add(wall_delta)
            .expect("time debt overflow");
        let accepted_delta = self.debt.min(MAX_ACCEPTED_FRAME);
        self.debt -= accepted_delta;
        self.elapsed = self
            .elapsed
            .checked_add(accepted_delta)
            .expect("logical duration overflow");
        AcceptedFrame {
            wall_delta,
            accepted_delta,
            elapsed: self.elapsed,
            debt: self.debt,
        }
    }
}

/// Accepted elapsed seconds. Legacy service floats are adapted at this seam;
/// new deadlines use the explicit type, never a host process timestamp.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LogicalTime(f64);

impl<'de> Deserialize<'de> for LogicalTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let seconds = f64::deserialize(deserializer)?;
        Self::new(seconds)
            .ok_or_else(|| serde::de::Error::custom("logical time must be finite and non-negative"))
    }
}

impl LogicalTime {
    pub fn new(seconds: f64) -> Option<Self> {
        (seconds.is_finite() && seconds >= 0.0).then_some(Self(seconds))
    }
    pub fn seconds(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CalendarTime(f64);

impl CalendarTime {
    pub fn new(days: f64) -> Option<Self> {
        days.is_finite().then_some(Self(days))
    }
    pub fn days(self) -> f64 {
        self.0
    }
}

/// Exclusive expiry: a right ending at `until` is already unavailable at the
/// same-instant command boundary. Extending it then cannot revive the old right.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExclusiveDeadline<T>(pub T);

impl<T: Copy + PartialOrd> ExclusiveDeadline<T> {
    pub fn is_due(self, at: T) -> bool {
        at >= self.0
    }
}

/// The poll's due-expiry phase precedes FIFO consequential effects, including
/// completions. Domain adapters use this same instant for commit validation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DueBoundary {
    pub elapsed: LogicalTime,
    pub calendar: CalendarTime,
}

impl DueBoundary {
    pub fn new(elapsed: f64, calendar: f64) -> Option<Self> {
        Some(Self {
            elapsed: LogicalTime::new(elapsed)?,
            calendar: CalendarTime::new(calendar)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_time_decode_validates_and_preserves_exclusive_deadline() {
        use serde::de::value::{Error, F64Deserializer};
        for invalid in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(LogicalTime::deserialize(F64Deserializer::<Error>::new(invalid)).is_err());
        }
        let cutoff: ExclusiveDeadline<LogicalTime> = serde_json::from_str("12.5").unwrap();
        assert_eq!(serde_json::to_string(&cutoff).unwrap(), "12.5");
        assert!(!cutoff.is_due(LogicalTime::new(12.4).unwrap()));
        assert!(cutoff.is_due(LogicalTime::new(12.5).unwrap()));
    }

    #[test]
    fn ordinary_stall_is_retained_and_serviced_without_changing_origin() {
        let mut time = AcceptedTime::default();
        let frame = time.admit(Duration::from_millis(500));
        assert_eq!(frame.accepted_delta, MAX_ACCEPTED_FRAME);
        assert_eq!(frame.debt, Duration::from_millis(400));
        // This value is also the M2 continuation contract: no host origin in it.
        let mut restored = time;
        for _ in 0..5 {
            restored.admit(Duration::from_millis(20));
        }
        assert_eq!(restored.elapsed, Duration::from_millis(600));
        assert_eq!(restored.debt, Duration::ZERO);
        assert_eq!(restored.wall, restored.elapsed);
    }

    #[test]
    fn finite_grant_expires_before_same_instant_extension() {
        struct Grant {
            until: ExclusiveDeadline<CalendarTime>,
            live: bool,
        }
        let boundary = DueBoundary::new(5.0, 2.0).unwrap();
        let mut grant = Grant {
            until: ExclusiveDeadline(boundary.calendar),
            live: true,
        };
        // Domain adapters expire first, then process the queued extension.
        if grant.until.is_due(boundary.calendar) {
            grant.live = false;
        }
        let extended = if grant.live {
            grant.until = ExclusiveDeadline(CalendarTime::new(3.0).unwrap());
            true
        } else {
            false
        };
        assert!(!extended);
        assert!(!grant.live);
        assert!(DueBoundary::new(f64::NAN, 1.0).is_none());
        assert!(DueBoundary::new(1.0, f64::INFINITY).is_none());
    }
}
