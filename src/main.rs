mod city;
mod controller;
mod fonts;
mod materials;
mod scene;
mod screenshot;
mod smart_actors;
mod ui;

use std::fs;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, WindowMode, WindowResolution};
use city::CityPlugin;
use controller::ControllerPlugin;
use fonts::CathedralFontsPlugin;
use scene::CathedralPlugin;
use screenshot::CathedralScreenshotPlugin;
use serde::Deserialize;
use smart_actors::{SmartActorsConfig, SmartActorsPlugin};
use ui::HudPlugin;

const CONFIG_PATH: &str = "config.ron";

#[derive(Debug, Deserialize)]
#[serde(default)]
struct AppConfig {
    title: String,
    fullscreen: bool,
    width: u32,
    height: u32,
    resizable: bool,
    smart_actors: SmartActorsConfig,
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

fn main() {
    let config = load_config();
    let smart_actors = config.smart_actors.clone();
    App::new()
        // The procedural atmosphere normally fills the background. This warm,
        // hazy blue is also a useful fallback on GPUs without atmosphere
        // compute-shader support.
        .insert_resource(ClearColor(Color::srgb(0.52, 0.67, 0.76)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.title,
                resolution: WindowResolution::new(config.width, config.height),
                resizable: config.resizable,
                mode: window_mode(config.fullscreen),
                ..default()
            }),
            primary_cursor_options: Some(CursorOptions {
                visible: false,
                grab_mode: CursorGrabMode::Locked,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            CathedralFontsPlugin,
            ControllerPlugin,
            CathedralPlugin,
            CityPlugin,
            HudPlugin,
            CathedralScreenshotPlugin,
        ))
        .add_plugins(SmartActorsPlugin::new(smart_actors))
        .run();
}

fn load_config() -> AppConfig {
    match fs::read_to_string(CONFIG_PATH) {
        Ok(source) => match ron::from_str(&source) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("Could not parse {CONFIG_PATH}: {error}. Using fullscreen defaults.");
                AppConfig::default()
            }
        },
        Err(error) => {
            eprintln!("Could not read {CONFIG_PATH}: {error}. Using fullscreen defaults.");
            AppConfig::default()
        }
    }
}

fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    } else {
        WindowMode::Windowed
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
        assert_eq!(defaults.stt_trailing_silence_ms, 500);
    }

    #[test]
    fn default_window_mode_is_fullscreen() {
        assert!(matches!(
            window_mode(AppConfig::default().fullscreen),
            WindowMode::BorderlessFullscreen(_)
        ));
    }
}
