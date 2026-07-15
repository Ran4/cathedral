mod city;
mod config;
mod controller;
mod drive;
mod fonts;
mod materials;
mod nav_overlay;
mod scene;
mod screenshot;
mod session_log;
mod smart_actors;
mod ui;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, WindowMode, WindowResolution};
use city::CityPlugin;
use config::{PersistedConfig, load_config};
use controller::ControllerPlugin;
use fonts::CathedralFontsPlugin;
use nav_overlay::NavDebugPlugin;
use scene::CathedralPlugin;
use screenshot::CathedralScreenshotPlugin;
use smart_actors::SmartActorsPlugin;
use ui::HudPlugin;

fn main() {
    // The session directory must exist before anything logs, screenshots, or
    // starts the actor engine; all three consume this process-wide state.
    session_log::init();
    // The speech workers are subprocesses and write to their own stderr. Route
    // it into `logs.jsonl` under the worker's own source name (`stt` / `tts`),
    // so a session log still accounts for every line the run produced.
    cathedral_backends::set_log_sink(std::sync::Arc::new(|source: &str, line: &str| {
        eprintln!("[smart actors/{source}] {line}");
        session_log::log_line(source, "INFO", line);
    }));
    let config = load_config();
    let smart_actors = config.smart_actors.clone();
    let persisted = PersistedConfig(config.clone());
    let drive = drive::DrivePlugin::from_env();
    // Drive scripts always run windowed and small: fast, WM-friendly, and
    // independent of whatever config.ron says.
    let (resolution, mode) = if drive.is_some() {
        (WindowResolution::new(1280, 720), WindowMode::Windowed)
    } else {
        (
            WindowResolution::new(config.width, config.height),
            window_mode(config.fullscreen),
        )
    };
    let mut app = App::new();
    app
        // The procedural atmosphere normally fills the background. This warm,
        // hazy blue is also a useful fallback on GPUs without atmosphere
        // compute-shader support.
        .insert_resource(ClearColor(Color::srgb(0.52, 0.67, 0.76)))
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    // Mirror the console log stream into the session's
                    // `logs.jsonl` (see `session_log`).
                    custom_layer: session_log::custom_layer,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: config.title,
                        resolution,
                        resizable: config.resizable,
                        mode,
                        ..default()
                    }),
                    primary_cursor_options: Some(CursorOptions {
                        visible: false,
                        grab_mode: CursorGrabMode::Locked,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(persisted)
        .add_plugins((
            CathedralFontsPlugin,
            ControllerPlugin,
            CathedralPlugin,
            CityPlugin,
            HudPlugin,
            CathedralScreenshotPlugin,
            NavDebugPlugin,
        ))
        .add_plugins(SmartActorsPlugin::new(smart_actors));
    if let Some(drive) = drive {
        app.add_plugins(drive);
    }
    app.run();
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
    use config::AppConfig;

    #[test]
    fn default_window_mode_is_fullscreen() {
        assert!(matches!(
            window_mode(AppConfig::default().fullscreen),
            WindowMode::BorderlessFullscreen(_)
        ));
    }
}
