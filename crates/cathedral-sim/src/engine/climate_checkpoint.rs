//! Clock/weather component only. All other Engine owners, complete-poll
//! capture, host debt and production adoption remain separate mandatory work.
use super::*;
use crate::{
    checkpoint::{
        self, Admitted, CheckpointError, Reservation, Result, aggregate, records,
        serde_support::required_option,
    },
    clock::{WorldClockDtoV1, WorldTime},
    timeline::LogicalTime,
    weather::checkpoint::{
        self as weather_wire, ClimateCost, ConfigV1, SampleV1, TimelineV1, VALIDATION_WORKING_BYTES,
    },
    world::checkpoint::{BackboneCandidate, BackboneRefs},
};
use serde::{Deserialize, Serialize};
const OWNER: &str = "engine_climate";
#[cfg(test)]
mod tests;
/// Supported component queue policy; not a proof of a CPU-safe clock rate.
pub const MAX_BELL_STROKES: usize = 65_536;
const MAX_CONTEXT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
pub struct ClimateCheckpointContext<'a> {
    now: LogicalTime,
    backbone: BackboneRefs<'a>,
    nav: Option<&'a NavData>,
    shelters: &'a ShelterMap,
    areas: &'a AreaMap,
    sounds: &'a SoundCatalog,
}
impl<'a> ClimateCheckpointContext<'a> {
    pub fn from_world(w: &'a World, now: LogicalTime) -> Self {
        Self {
            now,
            backbone: BackboneRefs::from_world(w),
            nav: w.nav.as_deref(),
            shelters: &w.shelters,
            areas: &w.area_map,
            sounds: &w.sound_catalog,
        }
    }
    pub fn from_backbone(
        candidate: &'a BackboneCandidate,
        now: LogicalTime,
        nav: Option<&'a NavData>,
        shelters: &'a ShelterMap,
        areas: &'a AreaMap,
        sounds: &'a SoundCatalog,
    ) -> Self {
        Self {
            now,
            backbone: candidate.references(),
            nav,
            shelters,
            areas,
            sounds,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextV1 {
    #[serde(deserialize_with = "required_option")]
    nav: Option<[u8; 32]>,
    shelters: [u8; 32],
    areas: [u8; 32],
    sounds: [u8; 32],
}
// Stream borrowed definitions, never build a parallel array of owned strings.
struct Sounds<'a>(&'a SoundCatalog);
impl Serialize for Sounds<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(self.0.sounds().iter().map(|x| {
            (
                &x.sound_id,
                &x.sound_class,
                x.audible_distance,
                &x.heard,
                &x.seen,
                &x.sfx_prompt,
                x.duration_seconds,
                x.actor_emittable,
            )
        }))
    }
}
struct Ambients<'a>(&'a SoundCatalog);
impl Serialize for Ambients<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(
            self.0
                .ambients()
                .iter()
                .map(|x| (&x.sound_id, &x.sfx_prompt, x.duration_seconds)),
        )
    }
}
fn digest<T: Serialize>(value: &T) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    struct Sink {
        hash: Sha256,
        bytes: usize,
    }
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_CONTEXT_BYTES)
                .ok_or_else(|| std::io::Error::other("climate context size limit exceeded"))?;
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut s = Sink {
        hash: Sha256::new(),
        bytes: 0,
    };
    s.hash.update(b"cathedral-climate-context-v1\0");
    serde_json::to_writer(&mut s, value).map_err(|e| error(&e.to_string()))?;
    Ok(s.hash.finalize().into())
}
impl ContextV1 {
    fn new(c: ClimateCheckpointContext<'_>) -> Result<Self> {
        checkpoint::logical(OWNER, c.now.seconds())?;
        check(
            c.sounds.sounds().len() <= 25_000 && c.sounds.ambients().len() <= 25_000,
            "sound context count limit",
        )?;
        check(
            c.sounds
                .sounds()
                .iter()
                .all(|s| s.audible_distance.is_finite() && s.duration_seconds.is_finite())
                && c.sounds
                    .ambients()
                    .iter()
                    .all(|s| s.duration_seconds.is_finite()),
            "nonfinite sound context",
        )?;
        // AreaMap is publicly mutable. Refuse nonfinite geometry before JSON
        // could collapse distinct invalid floats into the same null hash. Full
        // geography/reference validation remains with its later owner.
        check(c.areas.areas.len() <= 4096, "area context count limit")?;
        for a in &c.areas.areas {
            check(a.boxes.len() <= 1024, "area context box limit")?;
            for b in &a.boxes {
                check(
                    [
                        b.min_m.x, b.min_m.y, b.min_m.z, b.max_m.x, b.max_m.y, b.max_m.z,
                    ]
                    .iter()
                    .all(|v| v.is_finite()),
                    "nonfinite area context",
                )?;
            }
        }
        Ok(Self {
            nav: c.nav.map(NavData::checkpoint_fingerprint),
            shelters: digest(&c.shelters.shelters())?,
            areas: digest(c.areas)?,
            sounds: digest(&(Sounds(c.sounds), Ambients(c.sounds)))?,
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldClimateV1 {
    sounds_enabled: bool,
    #[serde(with = "records::world_time::option")]
    current_time: Option<WorldTime>,
    #[serde(with = "weather_wire::sample::option")]
    current_weather: Option<WeatherSample>,
}
impl WorldClimateV1 {
    fn from_world(w: &World) -> Self {
        Self {
            sounds_enabled: w.sounds_enabled,
            current_time: w.current_time,
            current_weather: w.current_weather,
        }
    }
    fn validate(&self, c: ClimateCheckpointContext<'_>) -> Result<()> {
        check(
            self.sounds_enabled == c.backbone.sounds_enabled,
            "sound switch disagrees with backbone",
        )?;
        check(
            same_time(self.current_time, c.backbone.current_time),
            "sampled time disagrees with backbone",
        )?;
        if let Some(t) = self.current_time {
            checkpoint::calendar(OWNER, t.game_days())?;
            check(
                t.fraction.is_finite()
                    && (0.0..1.0).contains(&t.fraction)
                    && t.office == WorldTime::from_game_days(t.fraction).office
                    && t.weekday == crate::clock::Weekday::of_day(t.day),
                "invalid sampled world time",
            )?;
        }
        if let Some(s) = self.current_weather {
            weather_wire::validate_sample(s)?;
        }
        Ok(())
    }
}
#[derive(Debug, Serialize)]
pub struct WorldClimateDtoV1 {
    version: u16,
    boundary: LogicalTime,
    context: ContextV1,
    world: WorldClimateV1,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    version: u16,
    boundary: LogicalTime,
    context: ContextV1,
    world: WorldClimateV1,
}
#[derive(Debug)]
pub struct WorldClimateCandidate {
    data: WorldClimateDtoV1,
}
impl World {
    pub fn export_climate_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<WorldClimateDtoV1>> {
        r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
        let c = ClimateCheckpointContext::from_world(self, now);
        let dto = WorldClimateDtoV1 {
            version: 1,
            boundary: now,
            context: ContextV1::new(c)?,
            world: WorldClimateV1::from_world(self),
        };
        weather_wire::prepare(&dto, OWNER, &mut r)?;
        dto.validate(c)?;
        Ok(Admitted::new(dto, r))
    }
}
impl WorldClimateDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: ClimateCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: WorldWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let dto = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            world: w.world,
        };
        dto.validate(c)?;
        Ok(Admitted::new(dto, r))
    }
    fn validate(&self, c: ClimateCheckpointContext<'_>) -> Result<()> {
        check(
            self.version == 1 && self.boundary == c.now && self.context == ContextV1::new(c)?,
            "world climate version/boundary/context mismatch",
        )?;
        self.world.validate(c)
    }
    pub fn cost(&self) -> Result<ClimateCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
}
impl Admitted<WorldClimateDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: ClimateCheckpointContext<'_>,
    ) -> Result<Admitted<WorldClimateCandidate>> {
        self.try_map(|data, _| {
            data.validate(c)?;
            Ok(WorldClimateCandidate { data })
        })
    }
}
impl WorldClimateCandidate {
    pub fn current_weather(&self) -> Option<WeatherSample> {
        self.data.world.current_weather
    }
}

#[derive(Debug, Serialize)]
pub struct EngineClimateDtoV1 {
    version: u16,
    boundary: LogicalTime,
    context: ContextV1,
    world: WorldClimateV1,
    initial_clock: WorldClockDtoV1,
    #[serde(with = "ConfigV1")]
    initial_weather: WeatherConfig,
    ring_the_offices: bool,
    #[serde(with = "records::TextV1")]
    player_id: String,
    clock: WorldClockDtoV1,
    #[serde(with = "TimelineV1")]
    weather: WeatherTimeline,
    last_weather_days: f64,
    #[serde(with = "SampleV1")]
    last_weather_sample: WeatherSample,
    last_clock_days: f64,
    bell_strokes: VecDeque<f64>,
    bell_seq: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineWire {
    version: u16,
    boundary: LogicalTime,
    context: ContextV1,
    world: WorldClimateV1,
    initial_clock: WorldClockDtoV1,
    #[serde(with = "ConfigV1")]
    initial_weather: WeatherConfig,
    ring_the_offices: bool,
    #[serde(with = "records::TextV1")]
    player_id: String,
    clock: WorldClockDtoV1,
    #[serde(with = "TimelineV1")]
    weather: WeatherTimeline,
    last_weather_days: f64,
    #[serde(with = "SampleV1")]
    last_weather_sample: WeatherSample,
    last_clock_days: f64,
    bell_strokes: VecDeque<f64>,
    bell_seq: u64,
}
#[derive(Serialize)]
struct EngineView<'a> {
    version: u16,
    boundary: LogicalTime,
    context: ContextV1,
    world: WorldClimateV1,
    initial_clock: WorldClockDtoV1,
    #[serde(with = "ConfigV1")]
    initial_weather: WeatherConfig,
    ring_the_offices: bool,
    player_id: &'a str,
    clock: WorldClockDtoV1,
    #[serde(with = "TimelineV1")]
    weather: &'a WeatherTimeline,
    last_weather_days: f64,
    #[serde(with = "SampleV1")]
    last_weather_sample: WeatherSample,
    last_clock_days: f64,
    bell_strokes: &'a VecDeque<f64>,
    bell_seq: u64,
}
impl<'a> EngineView<'a> {
    fn new(e: &'a Engine, c: ClimateCheckpointContext<'_>) -> Result<Self> {
        Ok(Self {
            version: 1,
            boundary: c.now,
            context: ContextV1::new(c)?,
            world: WorldClimateV1::from_world(&e.world),
            initial_clock: e.config.clock.checkpoint_provenance_v1(c.now)?,
            initial_weather: e.config.weather,
            ring_the_offices: e.config.ring_the_offices,
            player_id: e.config.player_id.as_str(),
            clock: e.clock.checkpoint_v1(c.now)?,
            weather: &e.weather,
            last_weather_days: e.last_weather_days,
            last_weather_sample: e.last_weather_sample,
            last_clock_days: e.last_clock_days,
            bell_strokes: &e.bell_strokes,
            bell_seq: e.bell_seq,
        })
    }
}
#[derive(Debug)]
pub struct EngineClimateCandidate {
    data: EngineClimateDtoV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ClimateCounts {
    pub characters: usize,
    pub bell_strokes: usize,
    pub weather_forced: bool,
    pub weather_residue: bool,
}
impl Engine {
    pub fn checkpoint_climate_cost(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<ClimateCost>> {
        r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
        let view = EngineView::new(self, ClimateCheckpointContext::from_world(&self.world, now))?;
        let cost = weather_wire::prepare(&view, OWNER, &mut r)?;
        Ok(Admitted::new(cost, r))
    }
    pub fn export_climate_checkpoint(
        &self,
        now: LogicalTime,
        mut r: Reservation,
    ) -> Result<Admitted<EngineClimateDtoV1>> {
        r.require(checkpoint::Cohort::SavePayload, aggregate::INITIAL_BYTES)?;
        let c = ClimateCheckpointContext::from_world(&self.world, now);
        let v = EngineView::new(self, c)?;
        weather_wire::prepare(&v, OWNER, &mut r)?;
        let dto = EngineClimateDtoV1 {
            version: v.version,
            boundary: v.boundary,
            context: v.context,
            world: v.world,
            initial_clock: v.initial_clock,
            initial_weather: v.initial_weather,
            ring_the_offices: v.ring_the_offices,
            player_id: v.player_id.to_string(),
            clock: v.clock,
            weather: v.weather.clone(),
            last_weather_days: v.last_weather_days,
            last_weather_sample: v.last_weather_sample,
            last_clock_days: v.last_clock_days,
            bell_strokes: v.bell_strokes.clone(),
            bell_seq: v.bell_seq,
        };
        dto.validate(c)?;
        Ok(Admitted::new(dto, r))
    }
}
impl EngineClimateDtoV1 {
    pub fn decode(
        bytes: &[u8],
        mut r: Reservation,
        c: ClimateCheckpointContext<'_>,
    ) -> Result<Admitted<Self>> {
        let w: EngineWire =
            aggregate::decode_with_working(bytes, OWNER, &mut r, VALIDATION_WORKING_BYTES)?;
        let dto = Self {
            version: w.version,
            boundary: w.boundary,
            context: w.context,
            world: w.world,
            initial_clock: w.initial_clock,
            initial_weather: w.initial_weather,
            ring_the_offices: w.ring_the_offices,
            player_id: w.player_id,
            clock: w.clock,
            weather: w.weather,
            last_weather_days: w.last_weather_days,
            last_weather_sample: w.last_weather_sample,
            last_clock_days: w.last_clock_days,
            bell_strokes: w.bell_strokes,
            bell_seq: w.bell_seq,
        };
        dto.validate(c)?;
        Ok(Admitted::new(dto, r))
    }
    pub fn cost(&self) -> Result<ClimateCost> {
        Ok(aggregate::measure(self, OWNER)?.into())
    }
    pub fn counts(&self, c: ClimateCheckpointContext<'_>) -> ClimateCounts {
        ClimateCounts {
            characters: c.backbone.characters.len(),
            bell_strokes: self.bell_strokes.len(),
            weather_forced: weather_wire::state_flags(&self.weather).0,
            weather_residue: weather_wire::state_flags(&self.weather).1,
        }
    }
    fn validate(&self, c: ClimateCheckpointContext<'_>) -> Result<()> {
        check(
            self.version == 1 && self.boundary == c.now && self.context == ContextV1::new(c)?,
            "engine climate version/boundary/context mismatch",
        )?;
        self.world.validate(c)?;
        self.initial_clock.validate_provenance(c.now)?;
        self.clock
            .validate_successor_of(&self.initial_clock, c.now)?;
        self.clock.validate_position(c.now, self.last_clock_days)?;
        self.clock
            .validate_position(c.now, self.last_weather_days)?;
        check(
            crate::ids::is_valid_id(&self.player_id)
                && c.backbone.characters.contains_key(self.player_id.as_str()),
            "missing/invalid bell recipient",
        )?;
        weather_wire::validate_config(self.initial_weather, true)?;
        weather_wire::validate_timeline(&self.weather)?;
        weather_wire::validate_position(&self.weather, self.last_weather_days)?;
        let expected = WeatherConfig {
            frequency: self.initial_weather.frequency.max(0.0),
            ..self.initial_weather
        };
        check(
            self.weather.config() == expected,
            "live/initial weather configuration disagree",
        )?;
        weather_wire::validate_sample(self.last_weather_sample)?;
        let sampled = self.weather.sample(self.last_weather_days);
        check(
            same_sample(sampled, self.last_weather_sample)
                && self
                    .world
                    .current_weather
                    .is_some_and(|s| same_sample(s, sampled)),
            "sampled weather disagrees with saved timeline",
        )?;
        check(
            same_time(
                self.world.current_time,
                Some(self.clock.clock().at(c.now.seconds())),
            ),
            "sampled clock disagrees with saved timeline",
        )?;
        check(
            self.bell_strokes.len() <= MAX_BELL_STROKES,
            "bell queue count limit",
        )?;
        check(
            self.bell_seq <= u64::MAX - (MAX_BELL_STROKES + 2 * 7) as u64,
            "bell sequence lacks next-pass headroom",
        )?;
        let mut prior = c.now.seconds();
        for &due in &self.bell_strokes {
            checkpoint::logical(OWNER, due)?;
            check(
                due > c.now.seconds() && due >= prior,
                "bell queue must retain ordered future obligations",
            )?;
            prior = due;
        }
        // Existing queued strokes drain even when either flag is disabled.
        Ok(())
    }
}
impl Admitted<EngineClimateDtoV1> {
    pub fn encode(self) -> Result<Admitted<Vec<u8>>> {
        self.try_map(|d, r| aggregate::encode(&d, OWNER, r))
    }
    pub fn into_candidate(
        self,
        c: ClimateCheckpointContext<'_>,
    ) -> Result<Admitted<EngineClimateCandidate>> {
        self.try_map(|data, _| {
            data.validate(c)?;
            Ok(EngineClimateCandidate { data })
        })
    }
}
impl EngineClimateCandidate {
    pub fn counts(&self, c: ClimateCheckpointContext<'_>) -> ClimateCounts {
        self.data.counts(c)
    }
}
fn error(reason: &str) -> CheckpointError {
    CheckpointError::new(OWNER, reason)
}
fn check(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(error(reason)) }
}
fn same_time(a: Option<WorldTime>, b: Option<WorldTime>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.day == b.day
                && a.fraction.to_bits() == b.fraction.to_bits()
                && a.office == b.office
                && a.weekday == b.weekday
        }
        _ => false,
    }
}
fn same_sample(a: WeatherSample, b: WeatherSample) -> bool {
    a.kind == b.kind
        && a.precipitation_kind == b.precipitation_kind
        && a.semantic_revision == b.semantic_revision
        && [
            a.cloud_cover,
            a.precipitation,
            a.wind_xz_mps[0],
            a.wind_xz_mps[1],
            a.gust,
            a.fog,
            a.visibility_m,
            a.surface_wetness,
            a.standing_water,
            a.thunder,
        ]
        .into_iter()
        .zip([
            b.cloud_cover,
            b.precipitation,
            b.wind_xz_mps[0],
            b.wind_xz_mps[1],
            b.gust,
            b.fog,
            b.visibility_m,
            b.surface_wetness,
            b.standing_water,
            b.thunder,
        ])
        .all(|(a, b)| a.to_bits() == b.to_bits())
}
