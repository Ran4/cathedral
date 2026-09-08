use super::{CheckpointError, Result, calendar, logical};
use crate::timeline::{AcceptedTime, LogicalTime};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LogicalAnchorV1 {
    Never,
    At(LogicalTime),
}
impl LogicalAnchorV1 {
    pub fn from_legacy(value: f64) -> Result<Self> {
        if value == f64::NEG_INFINITY {
            return Ok(Self::Never);
        }
        logical("time", value)?;
        Ok(Self::At(
            LogicalTime::new(value).expect("validated logical time"),
        ))
    }
    pub fn validate(self) -> Result<()> {
        match self {
            Self::Never => Ok(()),
            Self::At(t) => logical("time", t.seconds()),
        }
    }
    pub fn legacy(self) -> f64 {
        match self {
            Self::Never => f64::NEG_INFINITY,
            Self::At(t) => t.seconds(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CalendarAnchorV1 {
    Never,
    At(f64),
}
impl CalendarAnchorV1 {
    pub fn from_legacy(value: f64) -> Result<Self> {
        if value == f64::NEG_INFINITY {
            return Ok(Self::Never);
        }
        calendar("time", value)?;
        Ok(Self::At(value))
    }
    pub fn validate(self) -> Result<()> {
        match self {
            Self::Never => Ok(()),
            Self::At(t) => calendar("time", t),
        }
    }
    pub fn legacy(self) -> f64 {
        match self {
            Self::Never => f64::NEG_INFINITY,
            Self::At(t) => t,
        }
    }
}

/// Exact nanosecond duration; floats are never used to encode host residuals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurationV1 {
    seconds: u64,
    nanos: u32,
}
impl DurationV1 {
    fn from_duration(t: Duration) -> Self {
        Self {
            seconds: t.as_secs(),
            nanos: t.subsec_nanos(),
        }
    }
    fn duration(self) -> Result<Duration> {
        if self.nanos >= 1_000_000_000 || self.seconds > super::MAX_LOGICAL_SECONDS as u64 {
            return Err(CheckpointError::new(
                "host_time",
                "duration outside v1 range",
            ));
        }
        let d = Duration::new(self.seconds, self.nanos);
        if d > Duration::from_secs(super::MAX_LOGICAL_SECONDS as u64) {
            return Err(CheckpointError::new(
                "host_time",
                "duration outside v1 range",
            ));
        }
        Ok(d)
    }
}

/// M1 accepted-time contract only. The complete host payload must additionally
/// supply the physical/controller/custody boundary, mechanisms, vermin and
/// readable presentation owners; this value is not that complete payload.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostTimeV1 {
    elapsed: DurationV1,
    ordinary_debt: DurationV1,
    observed_wall: DurationV1,
    fixed_step: DurationV1,
    fixed_residual: DurationV1,
    movement_residual: f64,
}
impl HostTimeV1 {
    pub fn from_accepted(
        time: AcceptedTime,
        fixed_step: Duration,
        fixed_residual: Duration,
        movement_residual: f64,
    ) -> Result<Self> {
        let dto = Self {
            elapsed: DurationV1::from_duration(time.elapsed),
            ordinary_debt: DurationV1::from_duration(time.debt),
            observed_wall: DurationV1::from_duration(time.wall),
            fixed_step: DurationV1::from_duration(fixed_step),
            fixed_residual: DurationV1::from_duration(fixed_residual),
            movement_residual,
        };
        dto.validate()?;
        Ok(dto)
    }
    pub fn validate(&self) -> Result<()> {
        let elapsed = self.elapsed.duration()?;
        let debt = self.ordinary_debt.duration()?;
        let wall = self.observed_wall.duration()?;
        let step = self.fixed_step.duration()?;
        let residual = self.fixed_residual.duration()?;
        if elapsed.checked_add(debt) != Some(wall)
            || step.is_zero()
            || step > crate::timeline::MAX_ACCEPTED_FRAME
            || residual >= step
            || residual > elapsed
            || !self.movement_residual.is_finite()
            || !(0.0..crate::MOVEMENT_TICK_SECONDS).contains(&self.movement_residual)
            || self.movement_residual > elapsed.as_secs_f64()
        {
            return Err(CheckpointError::new(
                "host_time",
                "accepted wall/debt or residual invariant",
            ));
        }
        Ok(())
    }
    pub fn accepted(&self) -> Result<AcceptedTime> {
        self.validate()?;
        Ok(AcceptedTime {
            elapsed: self.elapsed.duration()?,
            debt: self.ordinary_debt.duration()?,
            wall: self.observed_wall.duration()?,
        })
    }
    pub fn fixed_step(&self) -> Result<Duration> {
        self.validate()?;
        self.fixed_step.duration()
    }
    pub fn fixed_residual(&self) -> Result<Duration> {
        self.validate()?;
        self.fixed_residual.duration()
    }
    pub fn movement_residual(&self) -> Result<f64> {
        self.validate()?;
        Ok(self.movement_residual)
    }
}
