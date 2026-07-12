mod city;
mod config;
mod controller;
mod fonts;
mod materials;
mod scene;
mod screenshot;
mod smart_actors;
mod ui;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, WindowMode, WindowResolution};
use city::CityPlugin;
use config::{PersistedConfig, load_config};
use controller::ControllerPlugin;
use fonts::CathedralFontsPlugin;
use scene::CathedralPlugin;
use screenshot::CathedralScreenshotPlugin;
use smart_actors::SmartActorsPlugin;
use ui::HudPlugin;

fn main() {
    let config = load_config();
    let smart_actors = config.smart_actors.clone();
    let persisted = PersistedConfig(config.clone());
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
        .insert_resource(persisted)
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
