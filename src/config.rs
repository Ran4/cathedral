//! Loading and persisting the player-editable `config.ron`.
//!
//! `config.ron` is a local, git-ignored file. When it is missing the committed
//! `default_config.ron` seeds the settings instead. In-game settings changes
//! rewrite `config.ron` (comments in the file are not preserved; the defaults
//! file keeps the documented reference copy).

use std::fs;

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
    pub smart_actors: SmartActorsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "The Cathedral of Impossible Light".into(),
            fullscreen: true,
            width: 1600,
            height: 900,
            resizable: true,
            smart_actors: SmartActorsConfig::default(),
        }
    }
}

pub fn load_config() -> AppConfig {
    for path in [CONFIG_PATH, DEFAULT_CONFIG_PATH] {
        match fs::read_to_string(path) {
            Ok(source) => match ron::from_str(&source) {
                Ok(config) => return config,
                Err(error) => eprintln!("Could not parse {path}: {error}."),
            },
            Err(error) => eprintln!("Could not read {path}: {error}."),
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
        assert!(!config.smart_actors.pause_microphone_during_npc_voice);
        assert!(!config.smart_actors.stt_streaming);
        assert_eq!(config.smart_actors.stt_trailing_silence_ms, 700);

        let defaults = AppConfig::default().smart_actors;
        assert!(defaults.stt_streaming);
        assert_eq!(defaults.stt_trailing_silence_ms, 400);
        assert_eq!(defaults.stt_backend, "cloud");
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
    }
}
