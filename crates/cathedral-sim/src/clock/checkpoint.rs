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
    fn clock(&self) -> WorldClock {
        WorldClock {
            seconds_per_day: self.seconds_per_day,
            epoch_days: self.epoch_days,
            elapsed_origin: self.elapsed_origin.seconds(),
            scale: self.scale,
            night_brightness: self.night_brightness,
        }
    }
}
