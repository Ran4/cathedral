mod city;
mod config;
mod controller;
mod drive;
mod fonts;
mod map;
mod materials;
mod mesh_batch;
mod nav_overlay;
mod perf;
mod scene;
mod screenshot;
mod session_log;
mod smart_actors;
mod soundscape;
mod ui;
mod weather;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, WindowMode, WindowResolution};
use city::CityPlugin;
use config::{PersistedConfig, load_config};
use controller::ControllerPlugin;
use fonts::CathedralFontsPlugin;
use map::MapPlugin;
use nav_overlay::NavDebugPlugin;
use scene::CathedralPlugin;
use screenshot::CathedralScreenshotPlugin;
use smart_actors::SmartActorsPlugin;
use soundscape::SoundscapePlugin;
use ui::HudPlugin;
use weather::WeatherPlugin;

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
    let mut config = load_config();
    // Perf/CI runs force the deterministic offline engine without editing the
    // player's config.ron.
    if std::env::var_os("CATHEDRAL_FAKE_BACKEND").is_some() {
        config.smart_actors.fake_backend = true;
    }
    // Ablation levers for perf attribution runs: kill a whole subsystem
    // without touching config.ron.
    if std::env::var_os("CATHEDRAL_NO_ACTORS").is_some() {
        config.smart_actors.enabled = false;
    }
    if std::env::var_os("CATHEDRAL_NO_WEATHER").is_some() {
        config.weather.enabled = false;
    }
    if std::env::var_os("CATHEDRAL_NO_DOGS").is_some() {
        config.smart_actors.dogs_enabled = false;
    }
    let smart_actors = config.smart_actors.clone();
    let weather = config.weather.clone();
    let persisted = PersistedConfig(config.clone());
    let drive = drive::DrivePlugin::from_env();
    // Drive scripts always run windowed and small: fast, WM-friendly, and
    // independent of whatever config.ron says.
    let (resolution, mode) = if drive.is_some() {
        // CATHEDRAL_DRIVE_RES=1920x1080 measures at play resolution; the
        // default stays small and WM-friendly.
        let resolution = std::env::var("CATHEDRAL_DRIVE_RES")
            .ok()
            .and_then(|value| {
                let (w, h) = value.split_once(['x', 'X'])?;
                Some(WindowResolution::new(w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or_else(|| WindowResolution::new(1280, 720));
        (resolution, WindowMode::Windowed)
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
                        // A drive window should keep the size it asked for:
                        // screenshots get compared frame to frame, and a
                        // tiling WM will happily stretch a resizable window to
                        // whatever cell it has free, changing aspect and FOV
                        // between runs. Asking for a fixed size is a hint, not
                        // a guarantee — some WMs still pick the size — but it
                        // does stop it varying run to run.
                        resizable: config.resizable && drive.is_none(),
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
            SoundscapePlugin,
            CityPlugin,
            HudPlugin,
            CathedralScreenshotPlugin,
            NavDebugPlugin,
            MapPlugin,
            WeatherPlugin::new(weather.clone()),
            perf::PerfPlugin,
        ))
        .add_plugins(SmartActorsPlugin::with_weather(smart_actors, weather));
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
