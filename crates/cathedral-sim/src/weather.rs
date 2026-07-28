//! Deterministic, clock-free weather authority.
//!
//! [`WeatherTimeline`] is handed an absolute game instant in fractional days.
//! It never reads a clock, filesystem, environment variable, or random device;
//! every front, fog bank, wet street and lightning strike is a stable function
//! of `(seed, game instant)`.  Presentation code consumes [`WeatherSample`]
//! through the engine's hot channel, while the behaviour and prompt layers read
//! the same value from [`crate::World::current_weather`].

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::{Office, math::Vec3};

const HOURS_PER_DAY: f64 = 24.0;
const DAYS_PER_HOUR: f64 = 1.0 / HOURS_PER_DAY;
const MINUTES_PER_DAY: f64 = HOURS_PER_DAY * 60.0;
const FRONT_LEAD_HOURS: f64 = 1.0;
const FRONT_TAIL_HOURS: f64 = 1.5;
/// How far back a shower can still be felt underfoot.  Six days of drying is
/// worth at least ~18 hours of exponent even under permanent overcast, so the
/// oldest episode in the window is down to under 2e-8 by the time it falls out
/// of it — the same invisible step the old night-rate bound bought with sixteen.
const WETNESS_WINDOW_DAYS: i64 = 6;
/// The quadrature cell of the drying integral: one whole game hour, on a grid
/// anchored to the absolute day axis rather than to the sample instant.
const DRYING_STEP_HOURS: f64 = 1.0;

/// The named state actors and diagnostics use.  Renderers should prefer the
/// continuous fields on [`WeatherSample`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherKind {
    Clear,
    BrokenCloud,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    Downpour,
    Thunderstorm,
}

impl WeatherKind {
    pub const ALL: [Self; 8] = [
        Self::Clear,
        Self::BrokenCloud,
        Self::Overcast,
        Self::Fog,
        Self::Drizzle,
        Self::Rain,
        Self::Downpour,
        Self::Thunderstorm,
    ];

    /// Config, CLI and drive spelling.  Friendly aliases are accepted, but the
    /// returned value always has one canonical [`Self::as_str`] spelling.
    pub fn from_config_name(name: &str) -> Option<Self> {
        let key: String = name
            .trim()
            .to_ascii_lowercase()
            .replace(['_', '-', ' '], "");
        match key.as_str() {
            "clear" => Some(Self::Clear),
            "broken" | "brokencloud" | "cloud" => Some(Self::BrokenCloud),
            "overcast" => Some(Self::Overcast),
            "fog" | "mist" | "dawnfog" => Some(Self::Fog),
            "drizzle" => Some(Self::Drizzle),
            "rain" | "steadyrain" => Some(Self::Rain),
            "downpour" | "heavyrain" => Some(Self::Downpour),
            "storm" | "thunder" | "thunderstorm" => Some(Self::Thunderstorm),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::BrokenCloud => "broken_cloud",
            Self::Overcast => "overcast",
            Self::Fog => "fog",
            Self::Drizzle => "drizzle",
            Self::Rain => "rain",
            Self::Downpour => "downpour",
            Self::Thunderstorm => "thunderstorm",
        }
    }

    pub fn prompt_name(self) -> &'static str {
        match self {
            Self::Clear => "clear summer weather",
            Self::BrokenCloud => "broken cloud",
            Self::Overcast => "a low overcast sky",
            Self::Fog => "quiet street fog",
            Self::Drizzle => "fine drizzle",
            Self::Rain => "steady rain",
            Self::Downpour => "a hard downpour",
            Self::Thunderstorm => "a thunderstorm",
        }
    }

    pub fn is_wet(self) -> bool {
        matches!(
            self,
            Self::Drizzle | Self::Rain | Self::Downpour | Self::Thunderstorm
        )
    }
}

impl std::fmt::Display for WeatherKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecipitationKind {
    None,
    Rain,
}

/// One fully-sanitized sample crossing the sim/host boundary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeatherSample {
    pub kind: WeatherKind,
    pub cloud_cover: f64,
    pub precipitation_kind: PrecipitationKind,
    pub precipitation: f64,
    /// World X/Z velocity in metres per second.  This is where the air travels,
    /// not where it came from.
    pub wind_xz_mps: [f64; 2],
    pub gust: f64,
    pub fog: f64,
    pub visibility_m: f64,
    pub surface_wetness: f64,
    pub standing_water: f64,
    pub thunder: f64,
    /// Changes only with an actor-nameable condition, never interpolated cloud,
    /// wind, visibility or wetness.
    pub semantic_revision: u64,
}

impl WeatherSample {
    pub const CLEAR: Self = Self {
        kind: WeatherKind::Clear,
        cloud_cover: 0.08,
        precipitation_kind: PrecipitationKind::None,
        precipitation: 0.0,
        wind_xz_mps: [0.8, -0.2],
        gust: 0.08,
        fog: 0.0,
        visibility_m: 340.0,
        surface_wetness: 0.0,
        standing_water: 0.0,
        thunder: 0.0,
        semantic_revision: 0,
    };

    fn sanitized(mut self) -> Self {
        self.cloud_cover = unit_or(self.cloud_cover, 0.0);
        self.precipitation = unit_or(self.precipitation, 0.0);
        self.wind_xz_mps[0] = finite_or(self.wind_xz_mps[0], 0.0).clamp(-45.0, 45.0);
        self.wind_xz_mps[1] = finite_or(self.wind_xz_mps[1], 0.0).clamp(-45.0, 45.0);
        self.gust = unit_or(self.gust, 0.0);
        self.fog = unit_or(self.fog, 0.0);
        self.visibility_m = finite_or(self.visibility_m, 300.0).clamp(20.0, 500.0);
        self.surface_wetness = unit_or(self.surface_wetness, 0.0);
        self.standing_water = unit_or(self.standing_water, 0.0);
        self.thunder = unit_or(self.thunder, 0.0);
        if self.precipitation <= f64::EPSILON {
            self.precipitation_kind = PrecipitationKind::None;
            self.precipitation = 0.0;
        }
        self
    }

    /// The short, perspective-aware sentence that sits beside the clock in an
    /// actor sheet.  Numeric internals and forecasts never enter it.
    pub fn prompt_phrase(self, shelter_label: Option<&str>) -> String {
        let mut phrase = format!("weather: {}", self.kind.prompt_name());
        if self.kind.is_wet() && wind_speed(self.wind_xz_mps) >= 1.4 {
            phrase.push_str(" from the ");
            phrase.push_str(wind_from(self.wind_xz_mps));
        }
        if self.surface_wetness >= 0.78 {
            phrase.push_str("; the streets are soaked");
        } else if self.surface_wetness >= 0.28 {
            phrase.push_str("; the streets are wet");
        } else if self.surface_wetness >= 0.08 {
            phrase.push_str("; the stones are damp");
        }
        if let Some(label) = shelter_label {
            phrase.push_str("; you are under ");
            phrase.push_str(label);
        } else if self.kind.is_wet() {
            phrase.push_str("; you are exposed to it");
        }
        phrase
    }

    pub fn wetness_band(self) -> &'static str {
        match self.surface_wetness {
            wet if wet >= 0.78 => "soaked",
            wet if wet >= 0.28 => "wet",
            wet if wet >= 0.08 => "damp",
            _ => "dry",
        }
    }

    pub fn wind_speed_mps(self) -> f64 {
        wind_speed(self.wind_xz_mps)
    }

    pub fn wind_from_label(self) -> &'static str {
        wind_from(self.wind_xz_mps)
    }
}

impl Default for WeatherSample {
    fn default() -> Self {
        Self::CLEAR
    }
}

/// Climate knobs are deliberately collected in one value so a future season
/// can replace the warm-summer table without touching any consumer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherClimate {
    pub precipitation_chance_per_slot: f64,
    pub fog_chance_per_day: f64,
    pub drizzle_share: f64,
    pub rain_share: f64,
    pub downpour_share: f64,
    pub thunderstorm_share: f64,
    pub minimum_wet_hours: f64,
    pub maximum_wet_hours: f64,
}

impl Default for WeatherClimate {
    fn default() -> Self {
        Self {
            // Two independent half-day opportunities.  With a mean 4.25 h
            // body this yields about 21% wet long-run time.
            precipitation_chance_per_slot: 0.60,
            fog_chance_per_day: 0.24,
            drizzle_share: 0.27,
            rain_share: 0.51,
            downpour_share: 0.17,
            thunderstorm_share: 0.05,
            minimum_wet_hours: 1.0,
            maximum_wet_hours: 6.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherMode {
    Timeline,
    Forced(WeatherKind),
}

impl WeatherMode {
    pub fn from_config_name(name: &str) -> Option<Self> {
        if name.trim().eq_ignore_ascii_case("timeline") {
            Some(Self::Timeline)
        } else {
            WeatherKind::from_config_name(name).map(Self::Forced)
        }
    }
}

/// The authoritative part of host weather configuration.  Visual quality and
/// volumetric toggles stay in the Bevy host and cannot alter this schedule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherConfig {
    pub enabled: bool,
    pub seed: u64,
    pub mode: WeatherMode,
    pub frequency: f64,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            seed: 437,
            mode: WeatherMode::Timeline,
            frequency: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ForcedWeather {
    kind: WeatherKind,
    intensity: Option<f64>,
    began_at_days: Option<f64>,
    initial_wetness: f64,
    initial_standing_water: f64,
    revision: u64,
}

/// A stable scheduled lightning event.  `game_instant_days` uses the same
/// absolute fractional-day axis passed to [`WeatherTimeline::sample`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LightningStrike {
    pub id: u64,
    pub game_instant_days: f64,
    pub origin_m: [f64; 3],
    pub strength: f64,
}

/// Pure weather schedule plus a developer override.  The only mutation is the
/// explicit override; timeline sampling itself is referentially transparent.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherTimeline {
    config: WeatherConfig,
    climate: WeatherClimate,
    forced: Option<ForcedWeather>,
    next_override_revision: u64,
}

impl WeatherTimeline {
    pub fn new(config: WeatherConfig) -> Self {
        let frequency = finite_or(config.frequency, 1.0).max(0.0);
        let config = WeatherConfig {
            frequency,
            ..config
        };
        let forced = match (config.enabled, config.mode) {
            (true, WeatherMode::Forced(kind)) => Some(ForcedWeather {
                kind,
                intensity: None,
                began_at_days: None,
                initial_wetness: 0.0,
                initial_standing_water: 0.0,
                revision: u64::MAX / 2,
            }),
            _ => None,
        };
        Self {
            config,
            climate: WeatherClimate::default(),
            forced,
            next_override_revision: u64::MAX / 2 + 1,
        }
    }

    pub fn with_climate(mut self, climate: WeatherClimate) -> Self {
        self.climate = climate;
        self
    }

    pub fn config(&self) -> WeatherConfig {
        self.config
    }

    pub fn set_override(
        &mut self,
        kind: WeatherKind,
        intensity: Option<f64>,
        game_instant_days: f64,
    ) {
        let intensity = intensity.map(|value| unit_or(value, representative_intensity(kind)));
        let inherited = if game_instant_days.is_finite() {
            self.sample(game_instant_days)
        } else {
            WeatherSample::CLEAR
        };
        let revision = self.next_override_revision;
        self.next_override_revision = self
            .next_override_revision
            .wrapping_add(1)
            .max(u64::MAX / 2);
        self.forced = Some(ForcedWeather {
            kind,
            intensity,
            began_at_days: game_instant_days.is_finite().then_some(game_instant_days),
            initial_wetness: inherited.surface_wetness,
            initial_standing_water: inherited.standing_water,
            revision,
        });
    }

    pub fn clear_override(&mut self) {
        self.forced = match (self.config.enabled, self.config.mode) {
            (true, WeatherMode::Forced(kind)) => Some(ForcedWeather {
                kind,
                intensity: None,
                began_at_days: None,
                initial_wetness: 0.0,
                initial_standing_water: 0.0,
                revision: self.next_override_revision,
            }),
            _ => None,
        };
        self.next_override_revision = self
            .next_override_revision
            .wrapping_add(1)
            .max(u64::MAX / 2);
    }

    pub fn sample(&self, game_instant_days: f64) -> WeatherSample {
        let time = finite_or(game_instant_days, 0.0);
        if let Some(forced) = self.forced {
            return forced_sample(forced, time);
        }
        if !self.config.enabled {
            return WeatherSample::CLEAR;
        }
        if self.config.frequency == 0.0 {
            return WeatherSample::CLEAR;
        }
        self.timeline_sample(time)
    }

    fn timeline_sample(&self, time: f64) -> WeatherSample {
        let mut sample = self.baseline_sample(time);
        let baseline_cloud = sample.cloud_cover;
        let mut semantic = sample.kind;
        let mut winning_severity = 0_u8;
        let mut front_envelope = 0.0_f64;

        // Neighbouring-day candidates are enough: the body is at most six
        // hours and its lead/tail another 2.5 h, so even a midnight crossing
        // cannot reach farther.
        let day = time.floor() as i64;
        for episode_day in (day - 1)..=(day + 1) {
            for slot in 0..2_u8 {
                let Some(episode) = self.precipitation_episode(episode_day, slot) else {
                    continue;
                };
                if let Some(phase) = episode.phase_at(time) {
                    let envelope = episode.cloud_envelope(time);
                    front_envelope = front_envelope.max(envelope);
                    // Return exactly to the continuous baseline at both front
                    // edges. A fixed 0.62 lower endpoint made the final clear
                    // sliver of a tail pop when the episode ceased to apply.
                    sample.cloud_cover =
                        sample.cloud_cover.max(lerp(baseline_cloud, 0.99, envelope));
                    let target_wind = episode.wind;
                    sample.wind_xz_mps[0] = lerp(sample.wind_xz_mps[0], target_wind[0], envelope);
                    sample.wind_xz_mps[1] = lerp(sample.wind_xz_mps[1], target_wind[1], envelope);
                    sample.gust = sample.gust.max(episode.gust * envelope);
                    let severity = phase.kind.severity();
                    if severity >= winning_severity {
                        winning_severity = severity;
                        semantic = phase.kind;
                    }
                    let precipitation = episode.precipitation_at(time);
                    if precipitation > sample.precipitation {
                        sample.precipitation = precipitation;
                        sample.precipitation_kind = if precipitation > 0.0 {
                            PrecipitationKind::Rain
                        } else {
                            PrecipitationKind::None
                        };
                    }
                    if episode.kind == WeatherKind::Thunderstorm {
                        sample.thunder = sample.thunder.max(envelope * episode.intensity);
                    }
                }
            }
        }

        // Dawn fog loses to precipitation but not to ordinary cloud.  Its wind
        // is deliberately low: a fog episode cannot coexist with a gale merely
        // because the baseline noise happened to choose one.
        if winning_severity < WeatherKind::Drizzle.severity()
            && let Some(fog) = self.fog_episode(day)
            && let Some(density) = fog.at(time)
        {
            // An approaching wet front erodes a dawn bank throughout its
            // overcast lead rather than deleting it at the first raindrop.
            // The semantic label remains fog until the wet body arrives, while
            // every presentation scalar reaches that boundary continuously.
            let density = density * (1.0 - front_envelope);
            sample.fog = density;
            let fog_cloud_target = sample.cloud_cover.max(0.66);
            sample.cloud_cover = lerp(sample.cloud_cover, fog_cloud_target, density);
            let original_wind = wind_speed(sample.wind_xz_mps);
            sample.wind_xz_mps[0] *= 1.0 - 0.72 * density;
            sample.wind_xz_mps[1] *= 1.0 - 0.72 * density;
            let fog_wind = wind_speed(sample.wind_xz_mps);
            let fog_wind_limit = lerp(original_wind, 2.0, smoothstep(density / 0.45));
            if fog_wind > fog_wind_limit {
                sample.wind_xz_mps[0] *= fog_wind_limit / fog_wind;
                sample.wind_xz_mps[1] *= fog_wind_limit / fog_wind;
            }
            sample.gust *= 1.0 - 0.8 * density;
            semantic = WeatherKind::Fog;
        }

        sample.kind = semantic;
        sample.semantic_revision = self.semantic_revision_at(time, semantic);
        let (wetness, standing) = self.accumulated_water(time);
        sample.surface_wetness = wetness;
        sample.standing_water = standing;
        sample.visibility_m = visibility_for(&sample);
        sample.sanitized()
    }

    fn baseline_sample(&self, time: f64) -> WeatherSample {
        // Six-hour value-noise knots.  Cloud and wind interpolate across
        // midnight because the knot index is absolute, never day-local.
        let knot = (time * 4.0).floor() as i64;
        let f = smoothstep(time * 4.0 - knot as f64);
        let cloud_a = baseline_cloud(self.hash(0x10, knot, 0));
        let cloud_b = baseline_cloud(self.hash(0x10, knot + 1, 0));
        let cloud = lerp(cloud_a, cloud_b, f);
        let wind_a = baseline_wind(self.hash(0x20, knot, 0));
        let wind_b = baseline_wind(self.hash(0x20, knot + 1, 0));
        let wind = [lerp(wind_a[0], wind_b[0], f), lerp(wind_a[1], wind_b[1], f)];
        let gust = lerp(
            unit(self.hash(0x30, knot, 0)) * 0.35,
            unit(self.hash(0x30, knot + 1, 0)) * 0.35,
            f,
        );
        WeatherSample {
            kind: semantic_cloud_kind(cloud_a),
            cloud_cover: cloud,
            precipitation_kind: PrecipitationKind::None,
            precipitation: 0.0,
            wind_xz_mps: wind,
            gust,
            fog: 0.0,
            visibility_m: 340.0,
            surface_wetness: 0.0,
            standing_water: 0.0,
            thunder: 0.0,
            semantic_revision: 0,
        }
    }

    /// The semantic kind without continuous presentation fields. This small
    /// evaluator is used only around deterministic schedule boundaries so a
    /// revision identifies the last *actual named transition*, not whichever
    /// overlapping front happened to win an implementation tie.
    fn semantic_kind_at(&self, time: f64) -> WeatherKind {
        let knot = (time * 4.0).floor() as i64;
        let mut kind = semantic_cloud_kind(baseline_cloud(self.hash(0x10, knot, 0)));
        let mut winning_severity = 0_u8;
        let day = time.floor() as i64;
        for episode_day in (day - 1)..=(day + 1) {
            for slot in 0..2_u8 {
                let Some(episode) = self.precipitation_episode(episode_day, slot) else {
                    continue;
                };
                let Some(phase) = episode.phase_at(time) else {
                    continue;
                };
                let severity = phase.kind.severity();
                if severity >= winning_severity {
                    winning_severity = severity;
                    kind = phase.kind;
                }
            }
        }
        if winning_severity < WeatherKind::Drizzle.severity()
            && self
                .fog_episode(day)
                .is_some_and(|fog| fog.at(time).is_some())
        {
            WeatherKind::Fog
        } else {
            kind
        }
    }

    fn semantic_revision_at(&self, time: f64, current_kind: WeatherKind) -> u64 {
        // A 32-day guard matches the old baseline-run search. With two wet
        // opportunities and one fog opportunity per day, reaching it without
        // any named boundary is astronomically unlikely, while the cap keeps a
        // deliberately degenerate custom climate cheap and bounded.
        let latest_knot = (time * 4.0).floor() as i64;
        let earliest_knot = latest_knot - 128;
        let search_start = earliest_knot as f64 / 4.0;
        let mut boundaries = Vec::with_capacity(512);
        for knot in earliest_knot..=latest_knot {
            boundaries.push(knot as f64 / 4.0);
        }
        let first_day = search_start.floor() as i64 - 1;
        let last_day = time.floor() as i64 + 1;
        for day in first_day..=last_day {
            for slot in 0..2_u8 {
                if let Some(episode) = self.precipitation_episode(day, slot) {
                    boundaries.extend(episode.semantic_boundaries());
                }
            }
            if let Some(fog) = self.fog_episode(day) {
                boundaries.extend([fog.start, fog.end]);
            }
        }
        boundaries.retain(|boundary| {
            boundary.is_finite() && *boundary >= search_start && *boundary <= time
        });
        boundaries.sort_by(|left, right| left.total_cmp(right));
        boundaries.dedup_by(|left, right| left.to_bits() == right.to_bits());

        for boundary in boundaries.into_iter().rev() {
            if self.semantic_kind_at(previous_f64(boundary)) != current_kind {
                return revision_from_boundary(boundary, current_kind);
            }
        }
        revision_from_boundary(search_start, current_kind)
    }

    fn precipitation_episode(&self, day: i64, slot: u8) -> Option<Episode> {
        let chance =
            (self.climate.precipitation_chance_per_slot * self.config.frequency).clamp(0.0, 1.0);
        let roll = self.hash(0x40, day, slot);
        if unit(roll) >= chance {
            return None;
        }
        let slot_start = if slot == 0 { 0.5 } else { 12.0 };
        let slot_span = if slot == 0 { 11.0 } else { 10.75 };
        let start_hour = slot_start + unit(self.hash(0x41, day, slot)) * slot_span;
        let duration_hours = lerp(
            self.climate.minimum_wet_hours,
            self.climate.maximum_wet_hours,
            unit(self.hash(0x42, day, slot)),
        )
        .clamp(0.25, 8.0);
        let kind_roll = unit(self.hash(0x43, day, slot));
        let total_share = (self.climate.drizzle_share
            + self.climate.rain_share
            + self.climate.downpour_share
            + self.climate.thunderstorm_share)
            .max(f64::EPSILON);
        let drizzle_end = self.climate.drizzle_share / total_share;
        let rain_end = drizzle_end + self.climate.rain_share / total_share;
        let downpour_end = rain_end + self.climate.downpour_share / total_share;
        let mut kind = if kind_roll < drizzle_end {
            WeatherKind::Drizzle
        } else if kind_roll < rain_end {
            WeatherKind::Rain
        } else if kind_roll < downpour_end {
            WeatherKind::Downpour
        } else {
            WeatherKind::Thunderstorm
        };
        // Thunder cells prefer the Waning and Lamplight.  A rare roll in the
        // morning degrades to a downpour rather than violating that constraint.
        if kind == WeatherKind::Thunderstorm && !(14.0..22.5).contains(&start_hour) {
            kind = WeatherKind::Downpour;
        }
        let range = match kind {
            WeatherKind::Drizzle => (0.10, 0.30),
            WeatherKind::Rain => (0.30, 0.70),
            WeatherKind::Downpour => (0.70, 1.00),
            WeatherKind::Thunderstorm => (0.55, 1.00),
            _ => unreachable!("only wet kinds are generated"),
        };
        let intensity = lerp(range.0, range.1, unit(self.hash(0x44, day, slot)));
        let angle = unit(self.hash(0x45, day, slot)) * TAU;
        let speed = lerp(
            if kind == WeatherKind::Thunderstorm {
                4.5
            } else {
                0.8
            },
            if kind == WeatherKind::Thunderstorm {
                13.0
            } else {
                7.0
            },
            unit(self.hash(0x46, day, slot)),
        );
        let (sin, cos) = angle.sin_cos();
        Some(Episode {
            id: self.hash(0x47, day, slot),
            kind,
            rain_start: day as f64 + start_hour * DAYS_PER_HOUR,
            rain_end: day as f64 + (start_hour + duration_hours) * DAYS_PER_HOUR,
            intensity,
            wind: [cos * speed, sin * speed],
            gust: if kind == WeatherKind::Thunderstorm {
                lerp(0.68, 1.0, unit(self.hash(0x48, day, slot)))
            } else {
                lerp(0.18, 0.62, unit(self.hash(0x48, day, slot)))
            },
        })
    }

    fn fog_episode(&self, day: i64) -> Option<FogEpisode> {
        let chance = (self.climate.fog_chance_per_day * self.config.frequency).clamp(0.0, 0.8);
        if unit(self.hash(0x50, day, 0)) >= chance {
            return None;
        }
        let start_hour = lerp(3.2, 5.6, unit(self.hash(0x51, day, 0)));
        let duration = lerp(1.5, 3.2, unit(self.hash(0x52, day, 0)));
        let start = day as f64 + start_hour * DAYS_PER_HOUR;
        let end = day as f64 + (start_hour + duration).min(10.0) * DAYS_PER_HOUR;
        // A wet front's lead/tail owns the sky when the intervals overlap.
        // Suppressing this candidate is both more meteorologically legible and
        // avoids calling a wind-driven overcast a low-wind fog until the exact
        // first raindrop. The independent hash roll remains deterministic.
        for episode_day in (day - 1)..=day {
            for slot in 0..2_u8 {
                if self
                    .precipitation_episode(episode_day, slot)
                    .is_some_and(|episode| episode.lead_start() < end && episode.tail_end() > start)
                {
                    return None;
                }
            }
        }
        // Fog is selected only from genuinely calm baseline air. Probe every
        // half game-hour across the short bank (including its endpoints); the
        // interpolated wind is convex between knots, so this also bounds the
        // intervals between probes. Presentation attenuation then quiets it
        // further as density rises, without an onset discontinuity.
        let probe_count = (((end - start) * HOURS_PER_DAY * 2.0).ceil() as usize).max(1);
        for probe in 0..=probe_count {
            let at = lerp(start, end, probe as f64 / probe_count as f64);
            if wind_speed(self.baseline_sample(at).wind_xz_mps) > 2.35 {
                return None;
            }
        }
        Some(FogEpisode {
            start,
            end,
            density: lerp(0.45, 1.0, unit(self.hash(0x54, day, 0))),
        })
    }

    /// Surface wetness and standing water underfoot at `time`.
    ///
    /// Drying is an **integral** over the hours since a shower ended, never one
    /// instant's rate stretched backwards across them.  Sampling the rate once
    /// and multiplying it by the whole elapsed window made a dry evening's
    /// wetness *rise*: the moment dusk crossed the old night/day branch, hours
    /// already spent in full sun were re-billed at the night rate, and a street
    /// went from damp to soaked between two game minutes with no rain in it.
    ///
    /// The quadrature holds the rate constant across each whole game hour, on a
    /// grid anchored to the absolute day axis.  The anchoring is the whole
    /// trick: two samples of the same span always read the same cells, so
    /// advancing `time` can only ever add to the exponent.  Wetness is therefore
    /// monotonically non-increasing whenever nothing is falling, and continuous
    /// through dusk and dawn.  Sampling stays independent of poll cadence.
    fn accumulated_water(&self, time: f64) -> (f64, f64) {
        let day = time.floor() as i64;
        // Every episode whose sky the sweep below has to read, materialised
        // once: re-rolling the hashes per game hour would cost more than the
        // integral itself.  Tomorrow's slot is in because a front's overcast
        // lead reaches back across midnight.
        let mut episodes: Vec<Episode> = Vec::with_capacity(2 * (WETNESS_WINDOW_DAYS as usize + 2));
        for episode_day in (day - WETNESS_WINDOW_DAYS)..=(day + 1) {
            for slot in 0..2_u8 {
                if let Some(episode) = self.precipitation_episode(episode_day, slot) {
                    episodes.push(episode);
                }
            }
        }
        let mut showers: Vec<Shower> = episodes
            .iter()
            .filter(|episode| time > episode.rain_start)
            .map(|episode| {
                let wetting_rate = match episode.kind {
                    WeatherKind::Drizzle => 0.8,
                    WeatherKind::Rain => 3.0,
                    WeatherKind::Downpour | WeatherKind::Thunderstorm => 8.5,
                    _ => 0.0,
                };
                let saturated = 1.0 - (-episode.rainfall_hours_until(time) * wetting_rate).exp();
                Shower {
                    // A shower still falling has spent no drying at all.
                    ended_hours: episode.rain_end.min(time) * HOURS_PER_DAY,
                    saturated,
                    ponded: ((saturated - 0.55) / 0.45).clamp(0.0, 1.0),
                }
            })
            .collect();
        if showers.is_empty() {
            return (0.0, 0.0);
        }
        // One sweep serves them all: walk the cursor through the endings in
        // order, banking the running budgets as each one passes, and the
        // integral from any ending to now is a difference of two totals.
        showers.sort_by(|left, right| left.ended_hours.total_cmp(&right.ended_hours));
        let mut cursor = showers[0].ended_hours;
        let mut dried = 0.0_f64;
        let mut drained = 0.0_f64;
        let mut banked: Vec<(f64, f64)> = Vec::with_capacity(showers.len());
        for shower in &showers {
            let (since_dried, since_drained) =
                self.integrate_drying(&episodes, cursor, shower.ended_hours);
            dried += since_dried;
            drained += since_drained;
            cursor = cursor.max(shower.ended_hours);
            banked.push((dried, drained));
        }
        let (since_dried, since_drained) =
            self.integrate_drying(&episodes, cursor, time * HOURS_PER_DAY);
        dried += since_dried;
        drained += since_drained;

        let mut dry_product = 1.0_f64;
        let mut standing = 0.0_f64;
        for (shower, (dried_at_end, drained_at_end)) in showers.iter().zip(banked) {
            let contribution = shower.saturated * (-(dried - dried_at_end)).exp();
            dry_product *= 1.0 - contribution.clamp(0.0, 0.999_999);
            standing = standing.max(shower.ponded * (-(drained - drained_at_end)).exp());
        }
        (
            (1.0 - dry_product).clamp(0.0, 1.0),
            standing.clamp(0.0, 1.0),
        )
    }

    /// The drying and draining budgets spent between two absolute game-hour
    /// instants.  The rate is held constant across each whole hour of the fixed
    /// grid, so the two halves of a split hour add up to exactly the whole one
    /// and a caller may chain segments without the grid shifting under it.
    ///
    /// The loop is bounded by the episode window: the oldest ending it is ever
    /// handed lies inside [`WETNESS_WINDOW_DAYS`] + 1 days of `to_hours`.
    fn integrate_drying(&self, episodes: &[Episode], from_hours: f64, to_hours: f64) -> (f64, f64) {
        if !from_hours.is_finite() || !to_hours.is_finite() || to_hours <= from_hours {
            return (0.0, 0.0);
        }
        let mut dried = 0.0_f64;
        let mut drained = 0.0_f64;
        let first = from_hours.div_euclid(DRYING_STEP_HOURS) as i64;
        let last = to_hours.div_euclid(DRYING_STEP_HOURS) as i64;
        for cell in first..=last {
            let cell_start = cell as f64 * DRYING_STEP_HOURS;
            let span = to_hours.min(cell_start + DRYING_STEP_HOURS) - from_hours.max(cell_start);
            if span <= 0.0 {
                continue;
            }
            let at = (cell_start + 0.5 * DRYING_STEP_HOURS) * DAYS_PER_HOUR;
            let (cloud, wind) = self.drying_sky(at, episodes);
            dried += drying_per_hour(summer_daylight(at.rem_euclid(1.0)), cloud, wind) * span;
            // 1–3 game hours to drain most standing water; dense overcast keeps
            // the slow end of that range.
            drained += lerp(0.9, 0.38, cloud) * span;
        }
        (dried, drained)
    }

    /// Cloud cover and wind speed as the drying integral sees them: the
    /// continuous baseline, lifted by whatever front stood overhead at that
    /// hour.  It rebuilds those two scalars the way [`Self::timeline_sample`]
    /// does rather than calling it, since a sample is the thing being built.
    /// Dawn fog is deliberately left out — a short bank whose own cloud lift is
    /// small, and never worth a second episode search per hour.
    fn drying_sky(&self, time: f64, episodes: &[Episode]) -> (f64, f64) {
        let baseline = self.baseline_sample(time);
        let mut cloud = baseline.cloud_cover;
        let mut wind = baseline.wind_xz_mps;
        for episode in episodes {
            let envelope = episode.cloud_envelope(time);
            if envelope <= 0.0 {
                continue;
            }
            cloud = cloud.max(lerp(baseline.cloud_cover, 0.99, envelope));
            wind[0] = lerp(wind[0], episode.wind[0], envelope);
            wind[1] = lerp(wind[1], episode.wind[1], envelope);
        }
        (cloud, wind_speed(wind))
    }

    /// Lightning events crossed in `(previous, now]`.  A long catch-up returns
    /// only the newest useful flash; ordinary short spans retain every strike.
    pub fn lightning_crossed(&self, previous_days: f64, now_days: f64) -> Vec<LightningStrike> {
        if !previous_days.is_finite() || !now_days.is_finite() || now_days <= previous_days {
            return Vec::new();
        }
        if let Some(forced) = self.forced {
            return if forced.kind == WeatherKind::Thunderstorm {
                self.forced_lightning_crossed(forced, previous_days, now_days)
            } else {
                Vec::new()
            };
        }
        if !self.config.enabled {
            return Vec::new();
        }
        if self.config.frequency == 0.0 {
            return Vec::new();
        }
        let search_from = previous_days.max(now_days - 3.0);
        let first_day = search_from.floor() as i64 - 1;
        let last_day = now_days.floor() as i64;
        let mut strikes = Vec::new();
        for day in first_day..=last_day {
            for slot in 0..2_u8 {
                let Some(episode) = self.precipitation_episode(day, slot) else {
                    continue;
                };
                if episode.kind != WeatherKind::Thunderstorm {
                    continue;
                }
                strikes.extend(self.episode_strikes(episode).into_iter().filter(|strike| {
                    strike.game_instant_days > previous_days && strike.game_instant_days <= now_days
                }));
            }
        }
        strikes.sort_by(|left, right| {
            left.game_instant_days
                .partial_cmp(&right.game_instant_days)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        let crossed_minutes = (now_days - previous_days) * MINUTES_PER_DAY;
        if crossed_minutes > 30.0 || strikes.len() > 3 {
            strikes.pop().into_iter().collect()
        } else {
            strikes
        }
    }

    fn forced_lightning_crossed(
        &self,
        forced: ForcedWeather,
        previous_days: f64,
        now_days: f64,
    ) -> Vec<LightningStrike> {
        let anchor = forced.began_at_days.unwrap_or(0.0);
        let first = anchor
            + lerp(
                7.0,
                13.0,
                unit(mix64(forced.revision ^ self.config.seed ^ 0x71)),
            ) / MINUTES_PER_DAY;
        let interval = lerp(
            22.0,
            36.0,
            unit(mix64(forced.revision ^ self.config.seed ^ 0x72)),
        ) / MINUTES_PER_DAY;
        let crossed_from = previous_days.max(anchor);
        let first_sequence = (((crossed_from - first) / interval).floor() as i64 + 1).max(0);
        let last_sequence = ((now_days - first) / interval).floor() as i64;
        if last_sequence < first_sequence {
            return Vec::new();
        }
        let mut strikes = Vec::new();
        for sequence in first_sequence..=last_sequence {
            let instant = first + sequence as f64 * interval;
            if instant <= previous_days || instant > now_days {
                continue;
            }
            let hash = mix64(
                forced.revision
                    ^ self.config.seed
                    ^ zigzag(sequence).wrapping_mul(0x9e37_79b9)
                    ^ 0x73,
            );
            strikes.push(LightningStrike {
                id: hash,
                game_instant_days: instant,
                origin_m: [
                    lerp(-364.0, 364.0, unit(mix64(hash ^ 0x74))),
                    lerp(360.0, 680.0, unit(mix64(hash ^ 0x75))),
                    lerp(-392.0, 392.0, unit(mix64(hash ^ 0x76))),
                ],
                strength: lerp(0.62, 1.0, unit(mix64(hash ^ 0x77))),
            });
        }
        if (now_days - previous_days) * MINUTES_PER_DAY > 30.0 || strikes.len() > 3 {
            strikes.pop().into_iter().collect()
        } else {
            strikes
        }
    }

    fn episode_strikes(&self, episode: Episode) -> Vec<LightningStrike> {
        let mut strikes = Vec::new();
        let mut instant =
            episode.rain_start + lerp(8.0, 22.0, unit(mix64(episode.id ^ 0x61))) / MINUTES_PER_DAY;
        for sequence in 0..24_u64 {
            if instant >= episode.rain_end {
                break;
            }
            let hash = mix64(episode.id ^ sequence.wrapping_mul(0x9e37_79b9) ^ 0x62);
            let x = lerp(-364.0, 364.0, unit(mix64(hash ^ 0x63)));
            let z = lerp(-392.0, 392.0, unit(mix64(hash ^ 0x64)));
            strikes.push(LightningStrike {
                id: hash,
                game_instant_days: instant,
                origin_m: [x, lerp(360.0, 680.0, unit(mix64(hash ^ 0x65))), z],
                strength: lerp(0.58, 1.0, unit(mix64(hash ^ 0x66))),
            });
            let minutes = lerp(18.0, 43.0, unit(mix64(hash ^ 0x67)));
            instant += minutes / MINUTES_PER_DAY;
        }
        strikes
    }

    fn hash(&self, salt: u64, day_or_knot: i64, slot: u8) -> u64 {
        mix64(
            self.config.seed
                ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ zigzag(day_or_knot).rotate_left(17)
                ^ u64::from(slot).wrapping_mul(0xd6e8_feb8_6659_fd93),
        )
    }
}

impl Default for WeatherTimeline {
    fn default() -> Self {
        Self::new(WeatherConfig::default())
    }
}

#[derive(Debug, Clone, Copy)]
struct Episode {
    id: u64,
    kind: WeatherKind,
    rain_start: f64,
    rain_end: f64,
    intensity: f64,
    wind: [f64; 2],
    gust: f64,
}

#[derive(Debug, Clone, Copy)]
struct EpisodePhase {
    kind: WeatherKind,
}

/// One past shower reduced to what the ground still remembers of it: when it
/// stopped falling, how wet it got the stones, and how much of that stood in
/// the hollows.  Everything else about the episode is spent by then.
#[derive(Debug, Clone, Copy)]
struct Shower {
    ended_hours: f64,
    saturated: f64,
    ponded: f64,
}

impl Episode {
    fn lead_start(self) -> f64 {
        self.rain_start - FRONT_LEAD_HOURS * DAYS_PER_HOUR
    }

    fn tail_end(self) -> f64 {
        self.rain_end + FRONT_TAIL_HOURS * DAYS_PER_HOUR
    }

    fn semantic_boundaries(self) -> [f64; 6] {
        [
            self.lead_start(),
            lerp(self.lead_start(), self.rain_start, 0.46),
            self.rain_start,
            self.rain_end,
            lerp(self.rain_end, self.tail_end(), 0.68),
            self.tail_end(),
        ]
    }

    fn phase_at(self, time: f64) -> Option<EpisodePhase> {
        if !(self.lead_start()..self.tail_end()).contains(&time) {
            return None;
        }
        let kind = if time < self.rain_start {
            let split = lerp(self.lead_start(), self.rain_start, 0.46);
            if time < split {
                WeatherKind::BrokenCloud
            } else {
                WeatherKind::Overcast
            }
        } else if time < self.rain_end {
            self.kind
        } else {
            let split = lerp(self.rain_end, self.tail_end(), 0.68);
            if time < split {
                WeatherKind::Overcast
            } else {
                WeatherKind::BrokenCloud
            }
        };
        Some(EpisodePhase { kind })
    }

    fn cloud_envelope(self, time: f64) -> f64 {
        if time <= self.lead_start() || time >= self.tail_end() {
            0.0
        } else if time < self.rain_start {
            smoothstep((time - self.lead_start()) / (self.rain_start - self.lead_start()))
        } else if time <= self.rain_end {
            1.0
        } else {
            1.0 - smoothstep((time - self.rain_end) / (self.tail_end() - self.rain_end))
        }
    }

    fn precipitation_at(self, time: f64) -> f64 {
        if time <= self.rain_start || time >= self.rain_end {
            return 0.0;
        }
        let duration_hours = (self.rain_end - self.rain_start) * HOURS_PER_DAY;
        let rise_hours = 0.25_f64.min(duration_hours / 3.0);
        let fall_hours = (1.0 / 3.0_f64).min(duration_hours / 3.0);
        let elapsed = (time - self.rain_start) * HOURS_PER_DAY;
        let remaining = (self.rain_end - time) * HOURS_PER_DAY;
        let ramp = (elapsed / rise_hours)
            .min(remaining / fall_hours)
            .clamp(0.0, 1.0);
        self.intensity * smoothstep(ramp)
    }

    fn rainfall_hours_until(self, time: f64) -> f64 {
        if time <= self.rain_start {
            return 0.0;
        }
        let end = time.min(self.rain_end);
        let total = (self.rain_end - self.rain_start) * HOURS_PER_DAY;
        let elapsed = (end - self.rain_start) * HOURS_PER_DAY;
        let rise = 0.25_f64.min(total / 3.0);
        let fall = (1.0 / 3.0_f64).min(total / 3.0);
        let plateau_end = total - fall;
        let area = if elapsed <= rise {
            // The visual uses smoothstep; its integral is 0.5 at t=1.
            let u = elapsed / rise;
            rise * (u.powi(3) - 0.5 * u.powi(4))
        } else if elapsed <= plateau_end {
            0.5 * rise + (elapsed - rise)
        } else {
            let into_fall = elapsed - plateau_end;
            let u = (into_fall / fall).clamp(0.0, 1.0);
            0.5 * rise + (plateau_end - rise) + fall * (u - u.powi(3) + 0.5 * u.powi(4))
        };
        area.max(0.0) * self.intensity
    }
}

#[derive(Debug, Clone, Copy)]
struct FogEpisode {
    start: f64,
    end: f64,
    density: f64,
}

impl FogEpisode {
    fn at(self, time: f64) -> Option<f64> {
        if !(self.start..self.end).contains(&time) {
            return None;
        }
        let ramp = (20.0 / MINUTES_PER_DAY).min((self.end - self.start) / 3.0);
        let envelope = if time < self.start + ramp {
            smoothstep((time - self.start) / ramp)
        } else if time > self.end - ramp {
            1.0 - smoothstep((time - (self.end - ramp)) / ramp)
        } else {
            1.0
        };
        Some(self.density * envelope)
    }
}

trait WeatherSeverity {
    fn severity(self) -> u8;
}

impl WeatherSeverity for WeatherKind {
    fn severity(self) -> u8 {
        match self {
            Self::Clear => 0,
            Self::BrokenCloud => 1,
            Self::Overcast => 2,
            Self::Fog => 3,
            Self::Drizzle => 4,
            Self::Rain => 5,
            Self::Downpour => 6,
            Self::Thunderstorm => 7,
        }
    }
}

fn forced_sample(forced: ForcedWeather, time: f64) -> WeatherSample {
    let intensity = forced
        .intensity
        .unwrap_or_else(|| representative_intensity(forced.kind));
    let elapsed_hours = forced.began_at_days.map_or(1.0, |began| {
        ((time - began).max(0.0) * HOURS_PER_DAY).min(8.0)
    });
    let (cloud, precipitation, wind, gust, fog, visibility, thunder) = match forced.kind {
        WeatherKind::Clear => (0.06, 0.0, [0.9, -0.25], 0.08, 0.0, 350.0, 0.0),
        WeatherKind::BrokenCloud => (0.48, 0.0, [1.8, -0.7], 0.24, 0.0, 300.0, 0.0),
        WeatherKind::Overcast => (0.92, 0.0, [1.2, -0.4], 0.15, 0.0, 260.0, 0.0),
        WeatherKind::Fog => (
            0.62,
            0.0,
            [0.25, -0.08],
            0.02,
            intensity.max(0.55),
            72.0,
            0.0,
        ),
        WeatherKind::Drizzle => (0.88, intensity, [1.9, 0.4], 0.18, 0.08, 225.0, 0.0),
        WeatherKind::Rain => (0.96, intensity, [3.4, 0.7], 0.35, 0.10, 170.0, 0.0),
        WeatherKind::Downpour => (1.0, intensity, [6.2, 1.8], 0.68, 0.16, 105.0, 0.0),
        WeatherKind::Thunderstorm => (1.0, intensity, [8.5, 2.3], 0.94, 0.18, 95.0, intensity),
    };
    let wetting_rate = match forced.kind {
        WeatherKind::Drizzle => 0.8,
        WeatherKind::Rain => 3.0,
        WeatherKind::Downpour | WeatherKind::Thunderstorm => 8.5,
        _ => 0.0,
    };
    let new_wetness = 1.0 - (-elapsed_hours * precipitation * wetting_rate).exp();
    let daylight = summer_daylight(time.rem_euclid(1.0));
    let drying = drying_per_hour(daylight, cloud, wind_speed(wind));
    let inherited_wetness = forced.initial_wetness * (-drying * elapsed_hours).exp();
    let wetness = 1.0 - (1.0 - inherited_wetness) * (1.0 - new_wetness);
    let new_standing = ((new_wetness - 0.55) / 0.45).clamp(0.0, 1.0);
    let inherited_standing = forced.initial_standing_water * (-0.62 * elapsed_hours).exp();
    let standing_water = new_standing.max(inherited_standing);
    WeatherSample {
        kind: forced.kind,
        cloud_cover: cloud,
        precipitation_kind: if precipitation > 0.0 {
            PrecipitationKind::Rain
        } else {
            PrecipitationKind::None
        },
        precipitation,
        wind_xz_mps: wind,
        gust,
        fog,
        visibility_m: visibility,
        surface_wetness: wetness,
        standing_water,
        thunder,
        semantic_revision: forced.revision,
    }
    .sanitized()
}

fn representative_intensity(kind: WeatherKind) -> f64 {
    match kind {
        WeatherKind::Clear | WeatherKind::BrokenCloud | WeatherKind::Overcast => 0.0,
        WeatherKind::Fog => 0.82,
        WeatherKind::Drizzle => 0.20,
        WeatherKind::Rain => 0.52,
        WeatherKind::Downpour => 0.86,
        WeatherKind::Thunderstorm => 0.90,
    }
}

fn visibility_for(sample: &WeatherSample) -> f64 {
    // Visibility follows the continuous causes rather than the semantic enum:
    // changing from a tail's `BrokenCloud` label to baseline `Overcast` must
    // not snap the far plane while cloud and rain themselves remain smooth.
    let dry_visibility = lerp(355.0, 260.0, sample.cloud_cover);
    let precipitation_limit = lerp(dry_visibility, 82.0, sample.precipitation.powf(1.2));
    // A mature fog bank reaches its 45–120 m range by density 0.45, while the
    // twenty-minute formation ramp still begins continuously at dry visibility.
    let fog_factor = smoothstep(sample.fog / 0.55);
    lerp(precipitation_limit, 48.0, fog_factor).clamp(40.0, 360.0)
}

fn semantic_cloud_kind(cloud: f64) -> WeatherKind {
    if cloud < 0.34 {
        WeatherKind::Clear
    } else if cloud < 0.74 {
        WeatherKind::BrokenCloud
    } else {
        WeatherKind::Overcast
    }
}

fn baseline_cloud(hash: u64) -> f64 {
    let value = unit(hash);
    // A clear/broken bias: roughly 60% of knots lie below overcast, while
    // interpolation makes long clear/broken spans a little more common still.
    if value < 0.42 {
        lerp(0.04, 0.32, value / 0.42)
    } else if value < 0.78 {
        lerp(0.36, 0.70, (value - 0.42) / 0.36)
    } else {
        lerp(0.76, 0.94, (value - 0.78) / 0.22)
    }
}

fn baseline_wind(hash: u64) -> [f64; 2] {
    let angle = unit(hash) * TAU;
    let speed = lerp(0.35, 4.8, unit(mix64(hash ^ 0x7788)));
    let (sin, cos) = angle.sin_cos();
    [cos * speed, sin * speed]
}

fn summer_daylight(fraction: f64) -> f64 {
    const DAWN: f64 = 5.0 / 24.0;
    const FULL: f64 = 6.5 / 24.0;
    const DUSK: f64 = 17.0 / 24.0;
    const NIGHT: f64 = 21.0 / 24.0;
    if !(DAWN..NIGHT).contains(&fraction) {
        0.0
    } else if fraction < FULL {
        smoothstep((fraction - DAWN) / (FULL - DAWN))
    } else if fraction < DUSK {
        1.0
    } else {
        1.0 - smoothstep((fraction - DUSK) / (NIGHT - DUSK))
    }
}

/// How much of a wet street one game hour takes off, as a continuous function
/// of the sky over it.  Night is the floor; the sun, a thin sky and moving air
/// each add to it.
///
/// The day terms fade in across the same narrow band of first light they used
/// to switch on at, instead of stepping there.  A step in this rate is a step in
/// the exponent every wet street in the city is drawn from, and the old
/// `daylight < 0.08` branch put one at roughly 20:11 and 05:15 every day.
fn drying_per_hour(daylight: f64, cloud_cover: f64, wind_mps: f64) -> f64 {
    const NIGHT: f64 = 0.045;
    let day = 0.10 + 0.42 * daylight * (1.0 - cloud_cover * 0.78) + 0.025 * wind_mps.min(10.0);
    lerp(NIGHT, day, smoothstep(daylight / 0.08))
}

fn wind_speed(wind: [f64; 2]) -> f64 {
    wind[0].hypot(wind[1])
}

fn wind_from(wind: [f64; 2]) -> &'static str {
    // Travel vector -> source direction. Ombreval's baked plan uses +X for
    // north and -Z for east (the same convention documented by the city
    // soundscape), rather than treating +X as east like a conventional map.
    let from_x = -wind[0];
    let from_z = -wind[1];
    let angle = (-from_z).atan2(from_x);
    let octant = ((angle / (TAU / 8.0)).round() as i32).rem_euclid(8);
    match octant {
        0 => "north",
        1 => "north-east",
        2 => "east",
        3 => "south-east",
        4 => "south",
        5 => "south-west",
        6 => "west",
        _ => "north-west",
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

fn unit_or(value: f64, fallback: f64) -> f64 {
    finite_or(value, fallback).clamp(0.0, 1.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn previous_f64(value: f64) -> f64 {
    if value == 0.0 {
        -f64::from_bits(1)
    } else if value > 0.0 {
        f64::from_bits(value.to_bits() - 1)
    } else {
        f64::from_bits(value.to_bits() + 1)
    }
}

fn revision_from_boundary(boundary: f64, kind: WeatherKind) -> u64 {
    mix64(boundary.to_bits() ^ (kind as u64).wrapping_mul(0xa24b_aed4_963e_e407))
}

fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit(hash: u64) -> f64 {
    // 53 high bits map exactly into f64's integer mantissa.
    ((hash >> 11) as f64) * (1.0 / ((1_u64 << 53) as f64))
}

// -------------------------------------------------------------------------
// Social shelters: data-owned places a body can actually stand.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelterAccess {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelterCover {
    Slate,
    Tile,
    Thatch,
    Stone,
    Timber,
    Canvas,
    Glass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shelter {
    pub id: String,
    pub label: String,
    pub polygon_xz: Vec<[f64; 2]>,
    pub route_node: usize,
    #[serde(default = "public_access")]
    pub access: ShelterAccess,
    #[serde(default = "default_capacity")]
    pub capacity: usize,
    #[serde(default = "default_spread")]
    pub spread_radius_m: f64,
    pub cover: ShelterCover,
    #[serde(default)]
    pub offices: Vec<Office>,
}

fn public_access() -> ShelterAccess {
    ShelterAccess::Public
}

fn default_capacity() -> usize {
    12
}

fn default_spread() -> f64 {
    2.0
}

impl Shelter {
    pub fn is_open(&self, office: Office) -> bool {
        self.offices.is_empty() || self.offices.contains(&office)
    }

    pub fn contains(&self, position: Vec3) -> bool {
        point_in_polygon([position.x, position.z], &self.polygon_xz)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShelterMap {
    shelters: Vec<Shelter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelterError {
    pub message: String,
}

impl std::fmt::Display for ShelterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ShelterError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShelterDocument {
    schema_version: u32,
    shelters: Vec<Shelter>,
}

impl ShelterMap {
    pub fn from_json_str(source: &str) -> Result<Self, ShelterError> {
        let document: ShelterDocument =
            serde_json::from_str(source).map_err(|error| ShelterError {
                message: format!("invalid shelter JSON: {error}"),
            })?;
        if document.schema_version != 1 {
            return Err(ShelterError {
                message: format!(
                    "unsupported shelter schema {}; expected 1",
                    document.schema_version
                ),
            });
        }
        let mut ids = std::collections::BTreeSet::new();
        for shelter in &document.shelters {
            if shelter.id.trim().is_empty()
                || shelter.label.trim().is_empty()
                || shelter.polygon_xz.len() < 3
                || shelter
                    .polygon_xz
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || !shelter.spread_radius_m.is_finite()
                || shelter.spread_radius_m < 0.0
                || shelter.capacity == 0
            {
                return Err(ShelterError {
                    message: format!("shelter `{}` has invalid geometry or capacity", shelter.id),
                });
            }
            if !ids.insert(shelter.id.as_str()) {
                return Err(ShelterError {
                    message: format!("duplicate shelter id `{}`", shelter.id),
                });
            }
        }
        Ok(Self {
            shelters: document.shelters,
        })
    }

    pub fn shelters(&self) -> &[Shelter] {
        &self.shelters
    }

    pub fn at(&self, position: Vec3) -> Option<&Shelter> {
        self.shelters
            .iter()
            .find(|shelter| shelter.contains(position))
    }

    pub fn label_at(&self, position: Vec3) -> Option<&str> {
        self.at(position).map(|shelter| shelter.label.as_str())
    }

    pub fn is_sheltered(&self, position: Vec3) -> bool {
        self.at(position).is_some()
    }
}

fn point_in_polygon(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let [xi, zi] = polygon[i];
        let [xj, zj] = polygon[j];
        let crosses = (zi > point[1]) != (zj > point[1]);
        if crosses {
            let at_x = (xj - xi) * (point[1] - zi) / (zj - zi) + xi;
            if point[0] < at_x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid(sample: WeatherSample) {
        for value in [
            sample.cloud_cover,
            sample.precipitation,
            sample.wind_xz_mps[0],
            sample.wind_xz_mps[1],
            sample.gust,
            sample.fog,
            sample.visibility_m,
            sample.surface_wetness,
            sample.standing_water,
            sample.thunder,
        ] {
            assert!(value.is_finite(), "non-finite weather value in {sample:?}");
        }
        for value in [
            sample.cloud_cover,
            sample.precipitation,
            sample.gust,
            sample.fog,
            sample.surface_wetness,
            sample.standing_water,
            sample.thunder,
        ] {
            assert!(
                (0.0..=1.0).contains(&value),
                "out-of-range value in {sample:?}"
            );
        }
        assert!((20.0..=500.0).contains(&sample.visibility_m));
    }

    #[test]
    fn same_seed_and_instant_are_byte_for_byte_stable() {
        let left = WeatherTimeline::new(WeatherConfig::default());
        let right = WeatherTimeline::new(WeatherConfig::default());
        for minute in [0, 1, 719, 1_440, 17_281, 525_599] {
            let instant = minute as f64 / MINUTES_PER_DAY;
            assert_eq!(left.sample(instant), right.sample(instant));
        }
    }

    #[test]
    fn multi_year_sweep_is_finite_and_bounded() {
        for seed in [0, 1, 437, u32::MAX as u64] {
            let timeline = WeatherTimeline::new(WeatherConfig {
                seed,
                ..WeatherConfig::default()
            });
            for hour in (0..(3 * 365 * 24)).step_by(3) {
                assert_valid(timeline.sample(hour as f64 / 24.0));
            }
        }
    }

    #[test]
    fn fronts_and_wind_are_continuous_across_midnight() {
        let timeline = WeatherTimeline::default();
        for day in 0..120 {
            let before = timeline.sample(day as f64 + 1.0 - 1.0 / 86_400.0);
            let after = timeline.sample(day as f64 + 1.0 + 1.0 / 86_400.0);
            assert!((before.cloud_cover - after.cloud_cover).abs() < 0.02);
            assert!((before.precipitation - after.precipitation).abs() < 0.02);
            assert!((before.wind_xz_mps[0] - after.wind_xz_mps[0]).abs() < 0.1);
            assert!((before.wind_xz_mps[1] - after.wind_xz_mps[1]).abs() < 0.1);
        }
    }

    #[test]
    fn warm_summer_distribution_is_varied_and_thunder_is_rare() {
        let timeline = WeatherTimeline::default();
        let mut clear_or_broken = 0;
        let mut wet = 0;
        let mut thunder = 0;
        let mut kinds = std::collections::BTreeSet::new();
        let samples = 365 * 24 * 4;
        for quarter_hour in 0..samples {
            let sample = timeline.sample(quarter_hour as f64 / (24.0 * 4.0));
            kinds.insert(sample.kind);
            clear_or_broken += usize::from(matches!(
                sample.kind,
                WeatherKind::Clear | WeatherKind::BrokenCloud
            ));
            wet += usize::from(sample.precipitation > 0.001);
            thunder += usize::from(sample.kind == WeatherKind::Thunderstorm);
        }
        let ratio = |count| count as f64 / samples as f64;
        assert!(
            (0.45..=0.72).contains(&ratio(clear_or_broken)),
            "{}",
            ratio(clear_or_broken)
        );
        assert!((0.14..=0.31).contains(&ratio(wet)), "{}", ratio(wet));
        assert!(ratio(thunder) < 0.05, "{}", ratio(thunder));
        assert_eq!(
            kinds,
            WeatherKind::ALL.into_iter().collect(),
            "every first-release kind occurs on the natural timeline"
        );
    }

    #[test]
    fn semantic_revision_changes_only_at_actual_named_boundaries() {
        let timeline = WeatherTimeline::default();
        let mut boundaries = Vec::new();
        for knot in 0..=(120 * 4) {
            boundaries.push(knot as f64 / 4.0);
        }
        for day in -1..=120 {
            for slot in 0..2 {
                if let Some(episode) = timeline.precipitation_episode(day, slot) {
                    boundaries.extend(episode.semantic_boundaries());
                }
            }
            if let Some(fog) = timeline.fog_episode(day) {
                boundaries.extend([fog.start, fog.end]);
            }
        }
        boundaries.retain(|boundary| (0.0..=120.0).contains(boundary));
        boundaries.sort_by(|left, right| left.total_cmp(right));
        boundaries.dedup_by(|left, right| left.to_bits() == right.to_bits());

        for boundary in boundaries {
            let before = timeline.sample(previous_f64(boundary));
            let after = timeline.sample(boundary);
            if before.kind == after.kind {
                assert_eq!(
                    before.semantic_revision, after.semantic_revision,
                    "revision changed without a named transition at {boundary}: before={before:?}, after={after:?}"
                );
            } else {
                assert_ne!(
                    before.semantic_revision, after.semantic_revision,
                    "named transition reused a revision at {boundary}: before={before:?}, after={after:?}"
                );
            }
            assert!(
                (before.cloud_cover - after.cloud_cover).abs() < 1.0e-6,
                "cloud cover popped at {boundary}: before={before:?}, after={after:?}"
            );
            for (name, left, right) in [
                ("precipitation", before.precipitation, after.precipitation),
                ("wind x", before.wind_xz_mps[0], after.wind_xz_mps[0]),
                ("wind z", before.wind_xz_mps[1], after.wind_xz_mps[1]),
                ("gust", before.gust, after.gust),
                ("fog", before.fog, after.fog),
                ("visibility", before.visibility_m, after.visibility_m),
                ("wetness", before.surface_wetness, after.surface_wetness),
                (
                    "standing water",
                    before.standing_water,
                    after.standing_water,
                ),
                ("thunder", before.thunder, after.thunder),
            ] {
                assert!(
                    (left - right).abs() < 1.0e-5,
                    "{name} popped at {boundary}: before={before:?}, after={after:?}"
                );
            }
        }
    }

    #[test]
    fn fog_is_morning_low_wind_and_burns_off() {
        let timeline = WeatherTimeline::default();
        let mut found = 0;
        for hour in 0..(365 * 24) {
            let sample = timeline.sample(hour as f64 / 24.0);
            if sample.kind == WeatherKind::Fog {
                found += 1;
                let local_hour = hour % 24;
                assert!(local_hour < 10, "fog at {local_hour}:00");
                assert!(wind_speed(sample.wind_xz_mps) < 2.5);
            }
        }
        assert!(found > 0);
    }

    #[test]
    fn wetness_is_a_function_of_time_not_poll_step() {
        let timeline = WeatherTimeline::default();
        // Merely querying every second cannot mutate the answer at the end.
        let end = 12.75;
        let expected = timeline.sample(end);
        for second in 0..(6 * 3600) {
            let _ = timeline.sample(end - 0.25 + second as f64 / 86_400.0);
        }
        assert_eq!(timeline.sample(end), expected);
    }

    #[test]
    fn rain_can_end_while_wetness_persists() {
        let timeline = WeatherTimeline::default();
        let mut witnessed = false;
        for minute in 1..(90 * 24 * 60) {
            let previous = timeline.sample((minute - 1) as f64 / MINUTES_PER_DAY);
            let now = timeline.sample(minute as f64 / MINUTES_PER_DAY);
            if previous.kind.is_wet() && !now.kind.is_wet() {
                assert!(now.surface_wetness > 0.05);
                witnessed = true;
                break;
            }
        }
        assert!(witnessed, "the test window should contain a rain ending");
    }

    /// The fine sweep `semantic_revision_changes_only_at_actual_named_boundaries`
    /// cannot do: it only ever looks at schedule boundaries, and the drying rate
    /// changes with the light, which has no schedule.  Wetness is an integral of
    /// that rate, so with nothing falling it may only ever go down, and it may
    /// never travel far in one game minute even when something is.
    #[test]
    fn wetness_only_falls_in_dry_weather_and_never_jumps_in_a_minute() {
        // A downpour's own wetting is the fastest legitimate move on either
        // curve; the measured worst case across these seeds is about 0.066 for
        // wetness, and standing water amplifies that by its 1/0.45 ponding
        // slope.
        const MAX_WETNESS_DELTA: f64 = 0.12;
        const MAX_STANDING_DELTA: f64 = 0.28;
        for seed in [437, 0, 99] {
            let timeline = WeatherTimeline::new(WeatherConfig {
                seed,
                ..WeatherConfig::default()
            });
            let mut previous = timeline.sample(0.0);
            for minute in 1..=(60 * 24 * 60) {
                let time = minute as f64 / MINUTES_PER_DAY;
                let now = timeline.sample(time);
                let day = time.floor() as i64;
                let hour = ((time - day as f64) * HOURS_PER_DAY).floor() as i64;
                let where_ = format!("seed {seed}, day {day} {hour:02}h, minute {minute}");
                for (name, before, after, bound) in [
                    (
                        "wetness",
                        previous.surface_wetness,
                        now.surface_wetness,
                        MAX_WETNESS_DELTA,
                    ),
                    (
                        "standing water",
                        previous.standing_water,
                        now.standing_water,
                        MAX_STANDING_DELTA,
                    ),
                ] {
                    assert!(
                        (after - before).abs() <= bound,
                        "{name} jumped {before:.4} -> {after:.4} in one minute at {where_}"
                    );
                    if previous.precipitation == 0.0 && now.precipitation == 0.0 {
                        assert!(
                            after <= before + 1.0e-9,
                            "{name} rose {before:.4} -> {after:.4} with nothing falling at {where_}"
                        );
                    }
                }
                previous = now;
            }
        }
    }

    /// Dusk and dawn are the two instants the old hard `daylight < 0.08` branch
    /// stepped the drying rate at, and it stepped it retroactively across every
    /// hour already elapsed.  Wetness must cross them the way the light does.
    #[test]
    fn wetness_is_continuous_across_dusk_and_dawn() {
        let timeline = WeatherTimeline::default();
        for day in 0..14 {
            // The band the sun crosses 8% in: about 05:15 and about 20:19.
            for start_hour in [5.0_f64, 20.0] {
                let start = day as f64 + start_hour * DAYS_PER_HOUR;
                let mut previous = timeline.sample(start);
                for second in 1..=(45 * 60) {
                    let time = start + second as f64 / 86_400.0;
                    let now = timeline.sample(time);
                    if previous.precipitation == 0.0 && now.precipitation == 0.0 {
                        assert!(
                            (now.surface_wetness - previous.surface_wetness).abs() < 1.0e-3,
                            "wetness stepped {:.4} -> {:.4} at day {day} {time}",
                            previous.surface_wetness,
                            now.surface_wetness,
                        );
                    }
                    previous = now;
                }
            }
        }
    }

    #[test]
    fn lightning_crossings_are_not_duplicated_or_lost() {
        let timeline = WeatherTimeline::default();
        let mut strike = None;
        'days: for day in 0..2_000 {
            for slot in 0..2 {
                if let Some(episode) = timeline.precipitation_episode(day, slot)
                    && episode.kind == WeatherKind::Thunderstorm
                    && let Some(candidate) = timeline.episode_strikes(episode).first().copied()
                {
                    strike = Some(candidate);
                    break 'days;
                }
            }
        }
        let strike = strike.expect("the climate produces a thunder cell");
        let epsilon = 1.0 / 86_400.0;
        assert!(
            timeline
                .lightning_crossed(strike.game_instant_days, strike.game_instant_days)
                .is_empty()
        );
        let crossed = timeline.lightning_crossed(
            strike.game_instant_days - epsilon,
            strike.game_instant_days + epsilon,
        );
        assert_eq!(crossed.len(), 1);
        assert_eq!(crossed[0].id, strike.id);
        assert!(
            timeline
                .lightning_crossed(strike.game_instant_days + epsilon, strike.game_instant_days)
                .is_empty()
        );
        // A six-hour hitch still preserves the newest useful event.
        let catchup = timeline.lightning_crossed(
            strike.game_instant_days - 0.25,
            strike.game_instant_days + epsilon,
        );
        assert!(!catchup.is_empty());
    }

    #[test]
    fn every_override_is_deterministic_and_clearable() {
        let mut timeline = WeatherTimeline::default();
        let scheduled = timeline.sample(20.25);
        for kind in WeatherKind::ALL {
            timeline.set_override(kind, Some(0.5), 20.0);
            let first = timeline.sample(20.25);
            let second = timeline.sample(20.25);
            assert_eq!(first, second);
            assert_eq!(first.kind, kind);
            assert_valid(first);
        }
        timeline.clear_override();
        assert_eq!(timeline.sample(20.25), scheduled);
    }

    #[test]
    fn forced_storm_keeps_crossed_lightning_and_ignores_timeline_frequency() {
        let mut timeline = WeatherTimeline::new(WeatherConfig {
            frequency: 0.0,
            ..WeatherConfig::default()
        });
        assert_eq!(timeline.sample(4.0), WeatherSample::CLEAR);
        timeline.set_override(WeatherKind::Thunderstorm, Some(0.9), 4.0);
        assert_eq!(timeline.sample(4.01).kind, WeatherKind::Thunderstorm);
        let strikes = timeline.lightning_crossed(4.0, 4.03);
        assert_eq!(strikes.len(), 1, "forced drive storms get a prompt flash");
        assert!(
            timeline
                .lightning_crossed(strikes[0].game_instant_days, strikes[0].game_instant_days)
                .is_empty()
        );
    }

    #[test]
    fn developer_override_temporarily_supersedes_disabled_weather() {
        let mut timeline = WeatherTimeline::new(WeatherConfig {
            enabled: false,
            ..WeatherConfig::default()
        });
        assert_eq!(timeline.sample(3.0), WeatherSample::CLEAR);
        timeline.set_override(WeatherKind::Rain, None, 3.0);
        assert_eq!(timeline.sample(3.01).kind, WeatherKind::Rain);
        timeline.clear_override();
        assert_eq!(timeline.sample(3.01), WeatherSample::CLEAR);
    }

    #[test]
    fn forced_clear_preserves_a_wet_aftermath_and_then_drains_it() {
        let mut timeline = WeatherTimeline::default();
        timeline.set_override(WeatherKind::Downpour, Some(0.9), 8.0);
        let soaked = timeline.sample(8.08);
        assert!(soaked.surface_wetness > 0.8);
        assert!(soaked.standing_water > 0.4);

        timeline.set_override(WeatherKind::Clear, None, 8.08);
        let just_ended = timeline.sample(8.08);
        assert_eq!(just_ended.kind, WeatherKind::Clear);
        assert_eq!(just_ended.precipitation, 0.0);
        assert_eq!(just_ended.surface_wetness, soaked.surface_wetness);
        let drying = timeline.sample(8.20);
        assert!(drying.surface_wetness < just_ended.surface_wetness);
        assert!(drying.surface_wetness > 0.05);
        assert!(drying.standing_water < just_ended.standing_water);
    }

    #[test]
    fn shelter_json_validates_and_classifies_points() {
        let map = ShelterMap::from_json_str(
            r#"{
                "schema_version": 1,
                "shelters": [{
                    "id": "arch", "label": "the arch roof",
                    "polygon_xz": [[0,0],[4,0],[4,3],[0,3]],
                    "route_node": 2, "access": "public", "capacity": 4,
                    "spread_radius_m": 1.0, "cover": "stone"
                }]
            }"#,
        )
        .expect("valid shelters");
        assert_eq!(
            map.label_at(Vec3::new(2.0, 0.0, 1.0)),
            Some("the arch roof")
        );
        assert!(!map.is_sheltered(Vec3::new(8.0, 0.0, 1.0)));
    }

    #[test]
    fn prompt_weather_never_leaks_numeric_internals() {
        let sample = forced_sample(
            ForcedWeather {
                kind: WeatherKind::Rain,
                intensity: Some(0.52),
                began_at_days: None,
                initial_wetness: 0.0,
                initial_standing_water: 0.0,
                revision: 7,
            },
            0.0,
        );
        let phrase = sample.prompt_phrase(Some("the Bellfoot Passage roof"));
        assert!(phrase.starts_with("weather: steady rain"));
        assert!(phrase.contains("you are under the Bellfoot Passage roof"));
        assert!(!phrase.contains("0.52"));
    }

    #[test]
    fn prose_wind_uses_the_city_plans_compass() {
        // Air travelling south came from north; air travelling east came from
        // west. In the city plan those travel vectors are -X and -Z.
        assert_eq!(wind_from([-4.0, 0.0]), "north");
        assert_eq!(wind_from([0.0, -4.0]), "west");
        assert_eq!(wind_from([4.0, 0.0]), "south");
        assert_eq!(wind_from([0.0, 4.0]), "east");
    }
}
