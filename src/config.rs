//! Loading and persisting the player-editable `config.ron`.
//!
//! `config.ron` is a local, git-ignored file. When it is missing the committed
//! `default_config.ron` seeds the settings instead. In-game settings changes
//! rewrite `config.ron` (comments in the file are not preserved; the defaults
//! file keeps the documented reference copy).

use std::{fs, path::Path};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::smart_actors::SmartActorsConfig;

pub const CONFIG_PATH: &str = "config.ron";
pub const DEFAULT_CONFIG_PATH: &str = "default_config.ron";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub title: String,
    pub fullscreen: bool,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    /// Weather is top-level because its host-side presentation controls sit
    /// beside the authoritative simulation settings.
    pub weather: WeatherSettings,
    /// The rat colonies: host-side presentation with one sim-facing switch,
    /// on the `weather` precedent.
    pub vermin: VerminSettings,
    pub smart_actors: SmartActorsConfig,
}

#[derive(Resource, Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WeatherSettings {
    pub enabled: bool,
    pub seed: u64,
    /// `timeline`, or any canonical/alias weather kind.
    pub mode: String,
    /// Episode frequency only; representative forced intensity is unchanged.
    pub frequency: f64,
    /// `low`, `medium`, or `high`. It changes presentation capacity only.
    pub quality: String,
    pub volumetric_fog: bool,
}

impl Default for WeatherSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            seed: 437,
            mode: "timeline".into(),
            frequency: 1.0,
            quality: "high".into(),
            volumetric_fog: true,
        }
    }
}

/// The authored rat colonies (`src/city/vermin.rs`). Everything here is
/// render-side presentation except `swarm_percepts`, which decides whether a
/// nightly boil may cross into the sim as an unattributed world sound.
#[derive(Resource, Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct VerminSettings {
    pub enabled: bool,
    pub seed: u64,
    /// Scales the authored per-colony rat counts and nothing else.
    pub density: f32,
    pub swarm_percepts: bool,
}

impl Default for VerminSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            seed: 40,
            density: 1.0,
            swarm_percepts: true,
        }
    }
}

/// The supported ceiling for `vermin.density`. Past ~4× the authored counts
/// the colonies read as a carpet, and the startup bake's cost scales with the
/// value — a typo'd `1e20` would saturate the spawn code's float-to-`usize`
/// cast and hang startup on an effectively unbounded number of path bakes.
pub const VERMIN_DENSITY_MAX: f32 = 4.0;

impl VerminSettings {
    /// Normalize `density` to a finite value in `0.0..=VERMIN_DENSITY_MAX`,
    /// returning what to log when the configured value was unusable.
    ///
    /// `config.ron` is player-editable and loaded before the scene spawns, so
    /// a wild value (`1e20`, `-3`, `inf` — RON accepts the non-finite
    /// spellings) must degrade to the nearest supported one with a precise
    /// diagnostic; refusing to start would leave the game unbootable until
    /// the file is fixed outside it.
    pub fn sanitize(&mut self) -> Option<String> {
        let configured = self.density;
        self.density = if configured.is_finite() {
            configured.clamp(0.0, VERMIN_DENSITY_MAX)
        } else {
            Self::default().density
        };
        // Bit comparison, so a configured NaN counts as changed.
        (self.density.to_bits() != configured.to_bits()).then(|| {
            format!(
                "vermin.density {configured} is outside the supported 0..={VERMIN_DENSITY_MAX}; using {}",
                self.density
            )
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "The Cathedral of Impossible Light".into(),
            fullscreen: true,
            width: 1600,
            height: 900,
            resizable: true,
            weather: WeatherSettings::default(),
            vermin: VerminSettings::default(),
            smart_actors: SmartActorsConfig::default(),
        }
    }
}

pub fn load_config() -> AppConfig {
    load_config_from_paths(CONFIG_PATH, DEFAULT_CONFIG_PATH)
}

/// Load the player override first and the committed defaults second.
///
/// Keeping the path selection injectable lets host integration tests exercise
/// this production parser without allowing a developer's local `config.ron`
/// to change the result.
pub fn load_config_from_paths(
    config_path: impl AsRef<Path>,
    default_config_path: impl AsRef<Path>,
) -> AppConfig {
    for path in [config_path.as_ref(), default_config_path.as_ref()] {
        match fs::read_to_string(path) {
            Ok(source) => match ron::from_str::<AppConfig>(&source) {
                Ok(mut config) => {
                    if let Some(warning) = config.vermin.sanitize() {
                        eprintln!("{}: {warning}.", path.display());
                    }
                    return config;
                }
                Err(error) => eprintln!("Could not parse {}: {error}.", path.display()),
            },
            Err(error) => eprintln!("Could not read {}: {error}.", path.display()),
        }
    }
    eprintln!("Using built-in fullscreen defaults.");
    AppConfig::default()
}

/// The configuration as loaded at startup, kept current so in-game settings
/// changes can rewrite `config.ron` without losing unrelated fields.
#[derive(Resource, Debug, Clone)]
pub struct PersistedConfig(pub AppConfig);

impl PersistedConfig {
    pub fn save(&self) -> Result<(), String> {
        let body = ron::ser::to_string_pretty(&self.0, ron::ser::PrettyConfig::default())
            .map_err(|error| format!("Could not serialize settings: {error}"))?;
        fs::write(CONFIG_PATH, body)
            .map_err(|error| format!("Could not write {CONFIG_PATH}: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_config_overrides_window_defaults() {
        let config: AppConfig = ron::from_str(
            r#"(
                fullscreen: false,
                width: 1280,
                height: 720,
                smart_actors: (
                    pause_microphone_during_npc_voice: false,
                    stt_streaming: false,
                    stt_trailing_silence_ms: 700,
                ),
            )"#,
        )
        .expect("test configuration should parse");

        assert!(!config.fullscreen);
        assert_eq!(config.width, 1280);
        assert_eq!(config.height, 720);
        assert_eq!(config.title, AppConfig::default().title);
        assert_eq!(config.weather.seed, 437);
        assert!(!config.smart_actors.pause_microphone_during_npc_voice);
        assert!(!config.smart_actors.stt_streaming);
        assert_eq!(config.smart_actors.stt_trailing_silence_ms, 700);

        let defaults = AppConfig::default().smart_actors;
        assert!(defaults.stt_streaming);
        assert_eq!(defaults.stt_trailing_silence_ms, 400);
        assert_eq!(defaults.stt_backend, "cloud");
    }

    /// The other direction of the same worry: `smart_actors.dogs_enabled` was
    /// removed (2026-08-15, the pack is always on), and every `config.ron`
    /// already on disk still names it. Nothing here denies unknown fields, so
    /// the line is ignored rather than taking the player's whole file down with
    /// it — a parse error would silently revert their backends, resolution and
    /// clock to the shipped defaults.
    #[test]
    fn a_config_still_naming_dogs_enabled_loads() {
        let old: AppConfig = ron::from_str(
            "(width: 1280, smart_actors: (dogs_enabled: false, stt_backend: \"local\"))",
        )
        .expect("a config naming the removed switch parses");
        assert_eq!(old.width, 1280);
        assert_eq!(
            old.smart_actors.stt_backend, "local",
            "the settings either side of the dead line survive it"
        );
    }

    /// `features/rats.md` §2.5: the vermin block is `#[serde(default)]` all the
    /// way down, so a `config.ron` written before the feature existed still
    /// loads — and one that names a single field keeps the shipped values for
    /// every other. A player's local config is not regenerated on upgrade, so
    /// this is the only thing standing between a new block and a settings wipe.
    #[test]
    fn a_config_written_before_the_rats_still_loads() {
        let old: AppConfig =
            ron::from_str("(fullscreen: false)").expect("a pre-vermin config parses");
        let shipped = VerminSettings::default();
        assert_eq!(old.vermin.enabled, shipped.enabled);
        assert_eq!(old.vermin.seed, shipped.seed);
        assert_eq!(old.vermin.density, shipped.density);
        assert_eq!(old.vermin.swarm_percepts, shipped.swarm_percepts);

        let partial: AppConfig = ron::from_str("(vermin: (swarm_percepts: false))")
            .expect("naming one vermin field parses");
        assert!(
            !partial.vermin.swarm_percepts,
            "the sim-facing switch is the one thing that changed"
        );
        assert_eq!(partial.vermin.enabled, shipped.enabled);
        assert_eq!(partial.vermin.seed, shipped.seed);
        assert_eq!(partial.vermin.density, shipped.density);

        // …and the committed reference copy agrees with the Rust defaults.
        let defaults: AppConfig = ron::from_str(
            &fs::read_to_string(DEFAULT_CONFIG_PATH).expect("the defaults file is committed"),
        )
        .expect("default_config.ron parses");
        assert_eq!(defaults.vermin.enabled, shipped.enabled);
        assert_eq!(defaults.vermin.seed, shipped.seed);
        assert_eq!(defaults.vermin.density, shipped.density);
        assert_eq!(defaults.vermin.swarm_percepts, shipped.swarm_percepts);
    }

    /// The crowd knob is the newest block-less field, so it gets the same
    /// guarantee the vermin block has: a `config.ron` written before it existed
    /// still loads, and loads the shipped city (nobody generated) rather than
    /// a wall of strangers.
    #[test]
    fn a_config_written_before_the_crowd_knob_still_loads_an_empty_city() {
        let old: AppConfig =
            ron::from_str("(fullscreen: false)").expect("a pre-crowd config parses");
        assert_eq!(old.smart_actors.extra_ambient_npcs, 0);

        let asked: AppConfig = ron::from_str("(smart_actors: (extra_ambient_npcs: 2000))")
            .expect("naming only the crowd field parses");
        assert_eq!(asked.smart_actors.extra_ambient_npcs, 2000);
        // …and naming it leaves every neighbouring setting alone.
        assert_eq!(
            asked.smart_actors.stt_backend,
            AppConfig::default().smart_actors.stt_backend
        );

        let defaults: AppConfig = ron::from_str(
            &fs::read_to_string(DEFAULT_CONFIG_PATH).expect("the defaults file is committed"),
        )
        .expect("default_config.ron parses");
        assert_eq!(defaults.smart_actors.extra_ambient_npcs, 0);
    }

    /// LE-05: `config.ron` is player-editable and loaded before the scene
    /// spawns, so a wild `vermin.density` — a typo like `1e20`, a negative, or
    /// the non-finite spellings RON accepts — must normalize to the supported
    /// range with a warning rather than saturate the spawn code's
    /// float-to-`usize` cast and hang startup.
    #[test]
    fn a_wild_vermin_density_is_normalized_not_obeyed() {
        let sanitized = |source: &str| -> (f32, bool) {
            let mut config: AppConfig =
                ron::from_str(source).expect("the test vermin block parses");
            let warned = config.vermin.sanitize().is_some();
            (config.vermin.density, warned)
        };

        // In range — both boundaries included — passes through, silently.
        assert_eq!(sanitized("(vermin: (density: 0.0))"), (0.0, false));
        assert_eq!(
            sanitized("(vermin: (density: 4.0))"),
            (VERMIN_DENSITY_MAX, false)
        );

        // A typo'd huge-but-finite value clamps to the ceiling; a negative
        // clamps to zero. Both are worth a precise line on stderr.
        assert_eq!(
            sanitized("(vermin: (density: 1e20))"),
            (VERMIN_DENSITY_MAX, true)
        );
        assert_eq!(sanitized("(vermin: (density: -3.0))"), (0.0, true));

        // The non-finite spellings fall back to the shipped default: there is
        // no nearest supported value to clamp a NaN to.
        let shipped = VerminSettings::default().density;
        assert_eq!(sanitized("(vermin: (density: inf))"), (shipped, true));
        assert_eq!(sanitized("(vermin: (density: NaN))"), (shipped, true));

        // And the production load path applies it, so no caller sees the raw
        // configured value.
        let path = std::env::temp_dir().join(format!(
            "cathedral_wild_density_config_{}.ron",
            std::process::id()
        ));
        fs::write(&path, "(vermin: (density: 1e20))").expect("the test config writes");
        let loaded = load_config_from_paths(&path, "does_not_exist.ron");
        fs::remove_file(&path).ok();
        assert_eq!(loaded.vermin.density, VERMIN_DENSITY_MAX);
    }

    #[test]
    fn saved_settings_round_trip_through_ron() {
        let mut config = AppConfig::default();
        config.smart_actors.tts_backend = "cloud".into();
        config.smart_actors.stt_backend = "local".into();

        let body = ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default())
            .expect("configuration should serialize");
        let restored: AppConfig = ron::from_str(&body).expect("serialized settings should parse");

        assert_eq!(restored.title, config.title);
        assert_eq!(restored.width, config.width);
        assert_eq!(restored.smart_actors.tts_backend, "cloud");
        assert_eq!(restored.smart_actors.stt_backend, "local");
        assert_eq!(restored.weather.mode, "timeline");
    }
}
