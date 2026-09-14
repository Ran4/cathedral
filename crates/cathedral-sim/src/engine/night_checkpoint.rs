//! Existing Night owner and independent World mood/config composition.
use super::*;
use crate::{
    checkpoint::{Admitted, Reservation, Result, aggregate},
    night::checkpoint::{
        self as owner, NightCheckpointContext, NightCost, NightCounts, NightOfficeDtoV1,
        WorldNightDtoV1,
    },
    timeline::LogicalTime,
};
use serde::{Deserialize, Serialize};
const OWNER: &str = "engine_night";
#[derive(Debug, Serialize)]
pub struct EngineNightDtoV1 {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldNightV1")]
    world: WorldNightDtoV1,
    #[serde(with = "owner::NightOfficeV1")]
    night: NightOfficeDtoV1,
    #[serde(with = "owner::records::ConfigV1")]
    config_night_office: NightOfficeConfig,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    boundary: LogicalTime,
    #[serde(with = "owner::WorldNightV1")]
    world: WorldNightDtoV1,
    #[serde(with = "owner::NightOfficeV1")]
    night: NightOfficeDtoV1,
    #[serde(with = "owner::records::ConfigV1")]
    config_night_office: NightOfficeConfig,
}
#[derive(Serialize)]
struct View<'a, W: Serialize, N: Serialize> {
    version: u16,
    boundary: LogicalTime,
    world: W,
    night: N,
    #[serde(with = "owner::records::ConfigV1")]
    config_night_office: &'a NightOfficeConfig,
}
#[derive(Debug)]
pub struct EngineNightCandidate {
    data: EngineNightDtoV1,
}
impl Engine {
    pub fn night_checkpoint_context(&self, now: LogicalTime) -> NightCheckpointContext<'_> {
        NightCheckpointContext::from_world(&self.world, now, &self.clock)
    }
    pub fn checkpoint_night_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<NightCost>> {
        let c = self.night_checkpoint_context(now);
        let binding = owner::binding(c, &mut r)?;
        let cost = owner::prepare(
            &View {
                version: 1,
                boundary: now,
                world: owner::WorldView::new(&self.world, now),
                night: owner::View::new(&self.night, now, &binding),
                config_night_office: &self.config.night_office,
            },
            &mut r,
        )?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_night_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineNightDtoV1>> {
        let c = self.night_checkpoint_context(now);
        let binding = owner::binding(c, &mut r)?;
        owner::prepare(
            &View {
                version: 1,
                boundary: now,
                world: owner::WorldView::new(&self.world, now),
                night: owner::View::new(&self.night, now, &binding),
                config_night_office: &self.config.night_office,
            },
            &mut r,
        )?;
        self.night.export_checkpoint(c, r)?.try_map(|night, r| {
            // Both subcomponents have already been measured in the full view.
            let world = WorldNightDtoV1::from_world_for_engine(&self.world, now);
            let d = EngineNightDtoV1 {
                version: 1,
                boundary: now,
                world,
                night,
                config_night_office: self.config.night_office,
            };
            d.validate(c)?;
            let _ = r;
            Ok(d)
        })
    }
}
impl EngineNightDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: NightCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, owner::VALIDATION_WORKING_BYTES)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            world: w.world,
            night: w.night,
            config_night_office: w.config_night_office,
        };
        d.validate(c)?;
        Ok(Admitted::new(d, r))
    }
    fn validate(&self, c: NightCheckpointContext<'_>) -> Result<()> {
        owner::check(self.version == 1, "unsupported Engine Night version")?;
        owner::check(self.boundary == c.now, "Engine Night boundary disagreement")?;
        self.world.validate(c)?;
        self.night.validate(c)
    }
    pub fn cost(&self) -> Result<NightCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: NightCheckpointContext<'_>) -> EngineNightCounts {
        EngineNightCounts {
            night: self.night.counts(c),
            ward_moods: self.world.ward_moods.len(),
            ward_mood_bytes: self.world.ward_moods.values().map(String::len).sum(),
        }
    }
}
impl Admitted<EngineNightDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: NightCheckpointContext<'_>,
    ) -> Result<Admitted<EngineNightCandidate>> {
        self.try_map(|d, _| {
            d.validate(c)?;
            Ok(EngineNightCandidate { data: d })
        })
    }
}
impl EngineNightCandidate {
    pub(crate) fn cognition_inputs_boundary(&self) -> LogicalTime {
        self.data.boundary
    }
    pub(crate) fn validate_cognition_inputs(&self, c: NightCheckpointContext<'_>) -> Result<()> {
        owner::check(
            self.data.boundary.seconds().to_bits() == c.now.seconds().to_bits()
                && self.data.night.boundary.seconds().to_bits() == c.now.seconds().to_bits()
                && self
                    .data
                    .world
                    .cognition_inputs_boundary()
                    .seconds()
                    .to_bits()
                    == c.now.seconds().to_bits(),
            "Night cognition input component boundary disagreement",
        )?;
        self.data.validate(c)
    }
    pub fn night(&self) -> &NightOffice {
        &self.data.night.night
    }
    pub fn ward_moods(&self) -> &std::collections::BTreeMap<crate::lore::PlanningWard, String> {
        &self.data.world.ward_moods
    }
    pub fn config_night_office(&self) -> NightOfficeConfig {
        self.data.config_night_office
    }
    pub fn counts(&self, c: NightCheckpointContext<'_>) -> EngineNightCounts {
        self.data.counts(c)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineNightCounts {
    #[serde(flatten)]
    pub night: NightCounts,
    pub ward_moods: usize,
    pub ward_mood_bytes: usize,
}
#[cfg(test)]
mod tests;

// Closed full-envelope decoding: the shared meter reserves before every
// typed allocation. This function never creates an independent component budget.
impl EngineNightDtoV1 {
    pub(crate) fn complete_decode(
        bytes: &[u8],
        meter: &crate::checkpoint::complete::meter::DecodeMeter<'_>,
        c: NightCheckpointContext<'_>,
    ) -> Result<EngineNightCandidate> {
        let w: Wire = meter.decode(bytes)?;
        let d = Self {
            version: w.version,
            boundary: w.boundary,
            world: w.world,
            night: w.night,
            config_night_office: w.config_night_office,
        };
        d.validate(c)?;
        Ok(EngineNightCandidate { data: d })
    }
}

impl Engine {
    pub(crate) fn complete_write_night<W: std::io::Write>(
        &self,
        now: LogicalTime,
        writer: &mut W,
        r: &mut Reservation,
    ) -> Result<()> {
        r.require(
            crate::checkpoint::Cohort::SavePayload,
            crate::checkpoint::complete::meter::VALIDATION_SCRATCH,
        )?;
        let c = self.night_checkpoint_context(now);
        let binding = owner::binding(c, r)?;
        let view = View {
            version: 1,
            boundary: now,
            world: owner::WorldView::new(&self.world, now),
            night: owner::View::new(&self.night, now, &binding),
            config_night_office: &self.config.night_office,
        };
        crate::checkpoint::complete::write_json(writer, &view)
    }
}
