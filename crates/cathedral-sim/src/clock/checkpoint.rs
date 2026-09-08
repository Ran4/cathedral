use super::WorldClock;
use crate::{
    checkpoint::{self, CheckpointError, Result},
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};

/// Preserve the actual rate segment; constructing a clock at a rounded office
/// or recomputing an epoch at process zero would move saved calendar edges.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldClockDtoV1 {
    version: u16,
    seconds_per_day: f64,
    epoch_days: f64,
    elapsed_origin: LogicalTime,
    scale: f64,
    night_brightness: f64,
}
impl WorldClock {
    pub(crate) fn checkpoint_provenance_v1(&self, now: LogicalTime) -> Result<WorldClockDtoV1> {
        checkpoint::logical("clock", self.elapsed_origin)?;
        let origin = LogicalTime::new(self.elapsed_origin).expect("validated clock origin");
        let dto = self.checkpoint_v1(origin)?;
        dto.validate_provenance(now)?;
        Ok(dto)
    }
    pub fn checkpoint_v1(&self, now: LogicalTime) -> Result<WorldClockDtoV1> {
        checkpoint::logical("clock", self.elapsed_origin)?;
        let dto = WorldClockDtoV1 {
            version: 1,
            seconds_per_day: self.seconds_per_day,
            epoch_days: self.epoch_days,
            elapsed_origin: LogicalTime::new(self.elapsed_origin).expect("validated time"),
            scale: self.scale,
            night_brightness: self.night_brightness,
        };
        dto.validate(now)?;
        Ok(dto)
    }
}
impl WorldClockDtoV1 {
    pub(crate) fn validate_provenance(&self, now: LogicalTime) -> Result<()> {
        checkpoint::logical("clock", now.seconds())?;
        self.validate(self.elapsed_origin)?;
        if self.elapsed_origin > now {
            return Err(CheckpointError::new(
                "clock",
                "initial clock origin follows saved boundary",
            ));
        }
        Ok(())
    }
    pub fn validate(&self, now: LogicalTime) -> Result<()> {
        checkpoint::logical("clock", now.seconds())?;
        checkpoint::logical("clock", self.elapsed_origin.seconds())?;
        checkpoint::calendar("clock", self.epoch_days)?;
        if self.version != 1
            || self.elapsed_origin > now
            || !(super::MIN_SECONDS_PER_DAY..=checkpoint::MAX_LOGICAL_SECONDS)
                .contains(&self.seconds_per_day)
            || !(0.000_001..=1_000_000.0).contains(&self.scale)
            || !(0.0..=1.0).contains(&self.night_brightness)
        {
            return Err(CheckpointError::new(
                "clock",
                "invalid clock version, rate, origin or brightness",
            ));
        }
        checkpoint::calendar("clock", self.clock().game_days(now.seconds()))
    }
    pub fn game_days_at(&self, now: LogicalTime) -> Result<f64> {
        self.validate(now)?;
        Ok(self.clock().game_days(now.seconds()))
    }
    /// Engine rate changes retain these immutable parameters and advance the
    /// segment origin. Historical intermediate slopes are not reconstructed.
    pub(crate) fn validate_successor_of(&self, initial: &Self, now: LogicalTime) -> Result<()> {
        initial.validate_provenance(now)?;
        self.validate(now)?;
        if self.seconds_per_day.to_bits() != initial.seconds_per_day.to_bits()
            || self.night_brightness.to_bits() != initial.night_brightness.to_bits()
            || self.elapsed_origin < initial.elapsed_origin
            || self.epoch_days < initial.epoch_days
        {
            return Err(CheckpointError::new(
                "clock",
                "live clock contradicts initial clock authority",
            ));
        }
        Ok(())
    }
    /// Check the explicit envelope position against this exact clock segment.
    pub fn validate_position(&self, now: LogicalTime, days: f64) -> Result<()> {
        checkpoint::calendar("clock", days)?;
        if self.game_days_at(now)?.to_bits() != days.to_bits() {
            return Err(CheckpointError::new(
                "clock",
                "calendar position does not match saved rate segment",
            ));
        }
        Ok(())
    }
    pub(crate) fn clock(&self) -> WorldClock {
        WorldClock {
            seconds_per_day: self.seconds_per_day,
            epoch_days: self.epoch_days,
            elapsed_origin: self.elapsed_origin.seconds(),
            scale: self.scale,
            night_brightness: self.night_brightness,
        }
    }
}
