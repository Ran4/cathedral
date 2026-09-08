//! Exact private timeline component. No constructor sanitation or reseeding.
use super::*;
use crate::checkpoint::{
    self, Admitted, CheckpointError, ComponentCost, Reservation, Result, aggregate,
    serde_support::{remote_adapters, required_option},
};

const OWNER: &str = "weather";
#[cfg(test)]
mod tests;
/// Covers sampling's bounded semantic-boundary vector, stable-sort scratch,
/// episode/shower/integral buffers and validation error scratch. Separate from
/// the lexical charge even for a tiny disabled/virgin timeline.
pub const VALIDATION_WORKING_BYTES: usize = 64 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ClimateCost {
    pub encoded_bytes: usize,
    pub expanded_upper_bytes: usize,
    pub validation_working_bytes: usize,
    pub peak_bytes: usize,
}
impl From<ComponentCost> for ClimateCost {
    fn from(c: ComponentCost) -> Self {
        Self {
            encoded_bytes: c.encoded_bytes,
            expanded_upper_bytes: c.expanded_upper_bytes,
            validation_working_bytes: VALIDATION_WORKING_BYTES,
            peak_bytes: c.peak_bytes + VALIDATION_WORKING_BYTES,
        }
    }
}
pub(crate) fn prepare<T: Serialize>(
    value: &T,
    owner: &'static str,
    reservation: &mut Reservation,
) -> Result<ClimateCost> {
    let cost = ClimateCost::from(aggregate::prepare_export(value, owner, reservation)?);
    if reservation.bytes() < cost.peak_bytes {
        reservation.resize(cost.peak_bytes)?;
    }
    Ok(cost)
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherMode", rename_all = "snake_case")]
pub(crate) enum ModeV1 {
    Timeline,
    Forced(WeatherKind),
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherConfig", deny_unknown_fields)]
pub(crate) struct ConfigV1 {
    enabled: bool,
    seed: u64,
    #[serde(with = "ModeV1")]
    mode: WeatherMode,
    frequency: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherClimate", deny_unknown_fields)]
struct ClimateV1 {
    precipitation_chance_per_slot: f64,
    fog_chance_per_day: f64,
    drizzle_share: f64,
    rain_share: f64,
    downpour_share: f64,
    thunderstorm_share: f64,
    minimum_wet_hours: f64,
    maximum_wet_hours: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ForcedWeather", deny_unknown_fields)]
struct ForcedV1 {
    kind: WeatherKind,
    #[serde(deserialize_with = "required_option")]
    intensity: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    began_at_days: Option<f64>,
    initial_wetness: f64,
    initial_standing_water: f64,
    revision: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherResidue", deny_unknown_fields)]
struct ResidueV1 {
    cleared_at_days: f64,
    wetness: f64,
    standing_water: f64,
}
// Keep runtime private authority private. Macro adapters are crate-private only
// to match the common strict helper; their actual types cannot escape here.
#[allow(private_interfaces)]
mod optional {
    use super::*;
    remote_adapters!(forced, ForcedWeather, ForcedV1);
    remote_adapters!(residue, WeatherResidue, ResidueV1);
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherTimeline", deny_unknown_fields)]
pub(crate) struct TimelineV1 {
    #[serde(with = "ConfigV1")]
    config: WeatherConfig,
    #[serde(with = "ClimateV1")]
    climate: WeatherClimate,
    #[serde(with = "optional::forced::option")]
    forced: Option<ForcedWeather>,
    #[serde(with = "optional::residue::option")]
    residue: Option<WeatherResidue>,
    next_override_revision: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WeatherSample", deny_unknown_fields)]
pub(crate) struct SampleV1 {
    kind: WeatherKind,
    cloud_cover: f64,
    precipitation_kind: PrecipitationKind,
    precipitation: f64,
    wind_xz_mps: [f64; 2],
    gust: f64,
    fog: f64,
    visibility_m: f64,
    surface_wetness: f64,
    standing_water: f64,
    thunder: f64,
    semantic_revision: u64,
}
remote_adapters!(sample, WeatherSample, SampleV1);

#[derive(Debug, Serialize)]
pub struct WeatherTimelineDtoV1 {
    version: u16,
    #[serde(with = "TimelineV1")]
    timeline: WeatherTimeline,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u16,
    #[serde(with = "TimelineV1")]
    timeline: WeatherTimeline,
}
#[derive(Serialize)]
struct View<'a> {
    version: u16,
    #[serde(with = "TimelineV1")]
    timeline: &'a WeatherTimeline,
}
#[derive(Debug)]
pub struct WeatherCandidate {
    pub(crate) timeline: WeatherTimeline,
}
impl WeatherTimeline {
    pub fn export_checkpoint(&self, mut r: Reservation) -> Result<Admitted<WeatherTimelineDtoV1>> {
        prepare(
            &View {
                version: 1,
                timeline: self,
            },
            OWNER,
            &mut r,
        )?;
        validate_timeline(self)?;
        Ok(Admitted::new(
            WeatherTimelineDtoV1 {
                version: 1,
                timeline: self.clone(),
            },
            r,
        ))
    }
}
impl WeatherTimelineDtoV1 {
    pub fn decode(bytes: &[u8], mut r: Reservation) -> Result<Admitted<Self>> {
        let w: Wire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        check(w.version == 1, "unsupported timeline version")?;
        validate_timeline(&w.timeline)?;
        Ok(Admitted::new(
            Self {
                version: w.version,
                timeline: w.timeline,
            },
            r,
        ))
    }
    pub fn cost(&self) -> Result<ClimateCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
}
impl Admitted<WeatherTimelineDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|dto, r| aggregate::encode(&dto, OWNER, r))
    }
    pub fn into_candidate(self) -> Result<Admitted<WeatherCandidate>> {
        self.try_map(|dto, _| {
            validate_timeline(&dto.timeline)?;
            Ok(WeatherCandidate {
                timeline: dto.timeline,
            })
        })
    }
}
impl WeatherCandidate {
    /// Read-only deterministic inspection; never adopts a timeline into Engine.
    pub fn sample(&self, days: f64) -> Result<WeatherSample> {
        checkpoint::calendar(OWNER, days)?;
        Ok(self.timeline.sample(days))
    }
}
pub(crate) fn check(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CheckpointError::new(OWNER, reason))
    }
}
pub(crate) fn validate_config(c: WeatherConfig, initial: bool) -> Result<()> {
    // V1 supported numeric policy; not an effective clock-rate CPU gate.
    check(
        c.frequency.is_finite()
            && c.frequency.abs() <= 1_000_000.0
            && (initial || c.frequency >= 0.0),
        "unsupported weather frequency",
    )
}
pub(crate) fn validate_sample(s: WeatherSample) -> Result<()> {
    for x in [
        s.cloud_cover,
        s.precipitation,
        s.gust,
        s.fog,
        s.surface_wetness,
        s.standing_water,
        s.thunder,
    ] {
        check(
            x.is_finite() && (0.0..=1.0).contains(&x),
            "invalid weather sample unit scalar",
        )?;
    }
    check(
        s.wind_xz_mps
            .iter()
            .all(|x| x.is_finite() && x.abs() <= 45.0),
        "invalid sampled wind",
    )?;
    check(
        s.visibility_m.is_finite() && (20.0..=500.0).contains(&s.visibility_m),
        "invalid sampled visibility",
    )?;
    check(
        (s.precipitation == 0.0 && s.precipitation_kind == PrecipitationKind::None)
            || (s.precipitation > f64::EPSILON && s.precipitation_kind == PrecipitationKind::Rain),
        "precipitation kind disagrees",
    )
}
pub(crate) fn validate_timeline(t: &WeatherTimeline) -> Result<()> {
    validate_config(t.config, false)?;
    let c = t.climate;
    for x in [
        c.precipitation_chance_per_slot,
        c.fog_chance_per_day,
        c.drizzle_share,
        c.rain_share,
        c.downpour_share,
        c.thunderstorm_share,
        c.minimum_wet_hours,
        c.maximum_wet_hours,
    ] {
        check(
            x.is_finite() && (0.0..=1_000_000.0).contains(&x),
            "unsupported climate scalar",
        )?;
    }
    check(
        c.minimum_wet_hours <= c.maximum_wet_hours,
        "reversed wet duration range",
    )?;
    let floor = u64::MAX / 2;
    check(
        t.next_override_revision >= floor,
        "invalid override revision domain",
    )?;
    check(
        t.forced.is_none() || t.residue.is_none(),
        "forced sky and separate residue cannot coexist",
    )?;
    check(
        !matches!(
            (t.config.enabled, t.config.mode),
            (true, WeatherMode::Forced(_))
        ) || t.forced.is_some(),
        "enabled config-forced sky is missing",
    )?;
    if let Some(f) = t.forced {
        if let Some(at) = f.began_at_days {
            checkpoint::calendar(OWNER, at)?;
        }
        for x in [f.initial_wetness, f.initial_standing_water]
            .into_iter()
            .chain(f.intensity)
        {
            check(
                x.is_finite() && (0.0..=1.0).contains(&x),
                "invalid forced wetness/intensity",
            )?;
        }
        check(
            f.revision >= floor
                && f.revision.wrapping_add(1).max(floor) == t.next_override_revision,
            "forced/next revision disagree",
        )?;
        check(
            f.began_at_days.is_some()
                || (f.initial_wetness == 0.0 && f.initial_standing_water == 0.0),
            "unanchored override cannot inherit water",
        )?;
    }
    if let Some(r) = t.residue {
        checkpoint::calendar(OWNER, r.cleared_at_days)?;
        check(
            [r.wetness, r.standing_water]
                .iter()
                .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
                && (r.wetness > 0.0 || r.standing_water > 0.0),
            "invalid cleared residue",
        )?;
    }
    Ok(())
}
pub(crate) fn validate_position(t: &WeatherTimeline, days: f64) -> Result<()> {
    checkpoint::calendar(OWNER, days)?;
    // Engine commands stamp the current calendar position; standalone timeline
    // APIs may sample older positions but cannot create future Engine history.
    if let Some(at) = t.forced.and_then(|f| f.began_at_days) {
        check(at <= days, "forced sky begins after saved Engine sample")?;
    }
    if let Some(r) = t.residue {
        check(
            r.cleared_at_days <= days,
            "residue begins after saved Engine sample",
        )?;
    }
    Ok(())
}
pub(crate) fn state_flags(t: &WeatherTimeline) -> (bool, bool) {
    (t.forced.is_some(), t.residue.is_some())
}
