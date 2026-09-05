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

use bevy::app::{TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, WindowMode, WindowResolution};
use bevy::winit::{UpdateMode, WinitSettings};
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
    if std::env::var_os("CATHEDRAL_NO_MARKS").is_some() {
        config.smart_actors.marks.enabled = false;
    }
    if std::env::var_os("CATHEDRAL_NO_KNOWLEDGE").is_some() {
        config.smart_actors.knowledge.enabled = false;
    }
    if std::env::var_os("CATHEDRAL_NO_VERMIN").is_some() {
        config.vermin.enabled = false;
    }
    // The crowd knob, for one run. A value that will not parse is worth a word
    // rather than a silent fallback to config.ron: the whole point of typing it
    // on the command line is that you are watching what it does.
    if let Some(value) = std::env::var_os("CATHEDRAL_EXTRA_NPCS") {
        match value.to_string_lossy().trim().parse::<u32>() {
            Ok(count) => config.smart_actors.extra_ambient_npcs = count,
            Err(error) => eprintln!(
                "CATHEDRAL_EXTRA_NPCS={} is not a count ({error}); using config.ron",
                value.to_string_lossy()
            ),
        }
    }
    let smart_actors = config.smart_actors.clone();
    let weather = config.weather.clone();
    let vermin = config.vermin.clone();
    let persisted = PersistedConfig(config.clone());
    let drive = drive::DrivePlugin::from_env();
    // A headless run renders, drives and screenshots exactly as usual — the
    // window is simply never mapped, so nothing appears on screen and nothing
    // takes the keyboard focus away from whatever the player is doing.
    let headless = std::env::var_os("CATHEDRAL_HEADLESS").is_some();
    // Drive scripts always run windowed and small: fast, WM-friendly, and
    // independent of whatever config.ron says.
    let (resolution, mode) = if drive.is_some() || headless {
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
    let mut plugins = DefaultPlugins
        .set(compute_thread_pool())
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
                // screenshots get compared frame to frame, and a tiling WM will
                // happily stretch a resizable window to whatever cell it has
                // free, changing aspect and FOV between runs. Asking for a
                // fixed size is a hint, not a guarantee — some WMs still pick
                // the size — but it does stop it varying run to run.
                resizable: config.resizable && drive.is_none() && !headless,
                // An unmapped window still renders and still fills a swapchain,
                // so `shot` captures the real frame.
                visible: !headless,
                mode,
                ..default()
            }),
            primary_cursor_options: Some(CursorOptions {
                visible: false,
                // Left `Locked` even headless, deliberately: chat, the
                // interaction prompt and mouse-look all read this as "gameplay
                // owns the input", so releasing it would make a headless run
                // answer `Enter` and `E` differently from the windowed one it
                // stands in for. X refuses to confine the pointer to an
                // unmapped window anyway — hence the one harmless "not
                // viewable" line at startup, and hence `mouse_look`'s own guard.
                grab_mode: CursorGrabMode::Locked,
                ..default()
            }),
            ..default()
        });
    // A run you cannot see is one you almost certainly do not want to hear
    // either: the city keeps its own ambience going for as long as it lives.
    // The codebase already supports running without the audio output (that is
    // how the headless tests run), so drop the plugin outright rather than
    // chase every fade that re-sets a sink's volume. `CATHEDRAL_HEADLESS_AUDIO=1`
    // keeps the sound for the runs that are *about* the soundscape.
    let muted = headless && std::env::var_os("CATHEDRAL_HEADLESS_AUDIO").is_none();
    if muted {
        plugins = plugins.disable::<bevy::audio::AudioPlugin>();
    }
    app
        // The procedural atmosphere normally fills the background. This warm,
        // hazy blue is also a useful fallback on GPUs without atmosphere
        // compute-shader support.
        .insert_resource(ClearColor(Color::srgb(0.52, 0.67, 0.76)))
        .add_plugins(plugins)
        .insert_resource(persisted)
        .insert_resource(vermin);
    if muted {
        // `AudioPlugin` is what registers this asset type, and the sound-effect
        // path asks the asset server for one whether or not anything can play
        // it — an unregistered handle allocation is a panic, not a no-op. The
        // headless test harness registers it by hand for the same reason
        // (`smart_actors::tests`); a muted run is that same app with a window.
        app.init_asset::<bevy::audio::AudioSource>();
    }
    if headless {
        // An unmapped window is never focused, and the default unfocused mode
        // throttles the app to a reactive 60 Hz. A headless run stands in for a
        // played one, so let it tick like the window it cannot show.
        app.insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        });
    }
    app.add_plugins((
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

/// How many threads the ECS scheduler gets.
///
/// Bevy's default takes the whole machine — here 12 compute threads plus 4
/// async-compute and 4 IO, on 20 cores. That is the right answer on an idle
/// machine and the wrong one on a shared desktop: a frame is not finished until
/// its *slowest* system is, so every extra worker is another chance that one of
/// them is sitting on a run queue behind somebody's compiler when the frame
/// wants to end. Fewer, busier threads finish a frame later on average and much
/// more predictably, and predictability is what stutter is about.
///
/// `CATHEDRAL_THREADS=<n>` overrides it; 0 restores Bevy's own default. The
/// measured curve is in `features/implemented/performance_improvements/`.
fn compute_thread_pool() -> TaskPoolPlugin {
    let Some(threads) = std::env::var("CATHEDRAL_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|threads| *threads > 0)
    else {
        return TaskPoolPlugin::default();
    };
    // `TaskPoolThreadAssignmentPolicy` has no `Default` in Bevy 0.19 (the two
    // callbacks are `Option<Arc<dyn Fn…>>`), so every field is named. Only the
    // compute pool is overridden: IO and async-compute are already capped at 4
    // each and are not where a frame's systems run.
    let mut options = TaskPoolOptions::default();
    options.compute = TaskPoolThreadAssignmentPolicy {
        min_threads: threads,
        max_threads: threads,
        percent: 1.0,
        on_thread_spawn: None,
        on_thread_destroy: None,
    };
    TaskPoolPlugin {
        task_pool_options: options,
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
    use config::AppConfig;

    #[test]
    fn default_window_mode_is_fullscreen() {
        assert!(matches!(
            window_mode(AppConfig::default().fullscreen),
            WindowMode::BorderlessFullscreen(_)
        ));
    }
}
