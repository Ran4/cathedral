//! Bevy projection of the sim-owned weather authority.
//!
//! [`WorldWeatherState`] is the only host write seam: the smart-actor bridge
//! refreshes it from hot `EngineMessage::Weather` values. Everything here is a
//! read-only presentation consumer, and the clear default remains available in
//! no-engine and no-renderer apps.

mod cover;
mod materials;
mod render;

use bevy::prelude::*;
use cathedral_sim::{LightningStrike, WeatherSample};

pub(crate) use cover::{CoverMaterial, PrecipitationOcclusionMap};
pub(crate) use materials::{WeatherReactiveMaterials, WetResponse};

pub(crate) use crate::config::WeatherSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeatherQuality {
    Low,
    Medium,
    High,
}

impl WeatherQuality {
    pub(crate) fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            other => {
                warn_once!("unknown weather.quality `{other}`; using high");
                Self::High
            }
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct WorldWeatherState {
    pub current: WeatherSample,
    pub previous: WeatherSample,
    pub received_at_seconds: f64,
    pub present: bool,
}

impl Default for WorldWeatherState {
    fn default() -> Self {
        Self {
            current: WeatherSample::CLEAR,
            previous: WeatherSample::CLEAR,
            received_at_seconds: 0.0,
            present: false,
        }
    }
}

impl WorldWeatherState {
    pub(crate) fn receive(&mut self, sample: WeatherSample, now: f64) {
        self.previous = self.current;
        self.current = sample;
        self.received_at_seconds = now;
        self.present = true;
    }
}

/// Real-time-smoothed rendering values. Semantic state and NPC behavior keep
/// reading the exact sim sample; only visual/audio scalar edges use this.
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct SmoothedWeather {
    pub cloud_cover: f32,
    pub precipitation: f32,
    pub wind: Vec2,
    pub gust: f32,
    pub fog: f32,
    pub visibility_m: f32,
    pub surface_wetness: f32,
    pub standing_water: f32,
    pub thunder: f32,
    initialized: bool,
}

impl Default for SmoothedWeather {
    fn default() -> Self {
        Self::from_sample(WeatherSample::CLEAR, false)
    }
}

impl SmoothedWeather {
    fn from_sample(sample: WeatherSample, initialized: bool) -> Self {
        Self {
            cloud_cover: sample.cloud_cover as f32,
            precipitation: sample.precipitation as f32,
            wind: Vec2::new(sample.wind_xz_mps[0] as f32, sample.wind_xz_mps[1] as f32),
            gust: sample.gust as f32,
            fog: sample.fog as f32,
            visibility_m: sample.visibility_m as f32,
            surface_wetness: sample.surface_wetness as f32,
            standing_water: sample.standing_water as f32,
            thunder: sample.thunder as f32,
            initialized,
        }
    }
}

/// One transient event. Visual flash and soundscape have independent readers;
/// neither infers a strike from the ongoing storm scalar.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct WeatherLightning(pub LightningStrike);

/// Shared material seam for the west rose. The cathedral registers its one
/// existing handle; lightning temporarily raises that handle's emissive value
/// and restores the exact authored baseline after the pulse.
#[derive(Resource, Debug, Clone)]
pub(crate) struct WeatherRoseWindow {
    pub handle: Option<Handle<StandardMaterial>>,
    pub baseline_emissive: LinearRgba,
}

impl Default for WeatherRoseWindow {
    fn default() -> Self {
        Self {
            handle: None,
            baseline_emissive: LinearRgba::BLACK,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WeatherSet {
    Smooth,
    Environment,
    Effects,
}

pub(crate) struct WeatherPlugin {
    settings: WeatherSettings,
}

impl WeatherPlugin {
    pub(crate) fn new(settings: WeatherSettings) -> Self {
        Self { settings }
    }
}

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone())
            .init_resource::<WorldWeatherState>()
            .init_resource::<SmoothedWeather>()
            .init_resource::<PrecipitationOcclusionMap>()
            .init_resource::<WeatherReactiveMaterials>()
            .init_resource::<WeatherRoseWindow>()
            .add_message::<WeatherLightning>()
            .configure_sets(
                Update,
                (
                    WeatherSet::Smooth,
                    WeatherSet::Environment,
                    WeatherSet::Effects,
                )
                    .chain(),
            )
            .add_systems(Update, smooth_weather.in_set(WeatherSet::Smooth));

        // Minimal/no-renderer apps still get every authority/projection
        // resource above. Render systems are installed only when the standard
        // mesh/material asset stores exist.
        let has_render_assets = app.world().contains_resource::<Assets<Mesh>>()
            && app.world().contains_resource::<Assets<StandardMaterial>>();
        if has_render_assets {
            app.add_systems(Startup, render::setup_weather_rendering)
                .add_systems(
                    Update,
                    render::compose_environment.in_set(WeatherSet::Environment),
                )
                .add_systems(
                    Update,
                    (
                        render::animate_clouds,
                        render::animate_precipitation,
                        render::update_weather_materials,
                        materials::update_wet_materials,
                        render::present_lightning,
                        render::update_lightning_flashes,
                    )
                        .in_set(WeatherSet::Effects),
                );
        }
    }
}

fn smooth_weather(
    time: Res<Time>,
    authoritative: Res<WorldWeatherState>,
    mut visual: ResMut<SmoothedWeather>,
) {
    // The snap is the sim's *first* sample's, not the placeholder's: this runs
    // from frame 1, long before the bridge has heard an `EngineMessage::Weather`,
    // and spending it on the clear default would ease a session that opens
    // mid-downpour up out of a clear sky over the next few seconds. Until the
    // sim speaks the smoothed values already are the clear default, so nothing
    // is lost by standing still.
    if !authoritative.present {
        return;
    }
    let target = authoritative.current;
    if !visual.initialized {
        *visual = SmoothedWeather::from_sample(target, true);
        return;
    }
    let dt = time.delta_secs().clamp(0.0, 0.25);
    let blend = |tau: f32| 1.0 - (-dt / tau.max(0.001)).exp();
    visual.cloud_cover += (target.cloud_cover as f32 - visual.cloud_cover) * blend(2.4);
    visual.precipitation += (target.precipitation as f32 - visual.precipitation) * blend(0.55);
    let current_wind = visual.wind;
    visual.wind += (Vec2::new(target.wind_xz_mps[0] as f32, target.wind_xz_mps[1] as f32)
        - current_wind)
        * blend(1.8);
    visual.gust += (target.gust as f32 - visual.gust) * blend(0.8);
    visual.fog += (target.fog as f32 - visual.fog) * blend(1.4);
    visual.visibility_m += (target.visibility_m as f32 - visual.visibility_m) * blend(1.2);
    visual.surface_wetness += (target.surface_wetness as f32 - visual.surface_wetness) * blend(0.8);
    visual.standing_water += (target.standing_water as f32 - visual.standing_water) * blend(1.0);
    visual.thunder += (target.thunder as f32 - visual.thunder) * blend(0.45);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_renderer_app_still_constructs_weather_projection() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(WeatherPlugin::new(WeatherSettings::default()));
        app.update();
        assert!(app.world().contains_resource::<WorldWeatherState>());
        assert!(app.world().contains_resource::<PrecipitationOcclusionMap>());
        assert_eq!(
            app.world().resource::<WorldWeatherState>().current,
            WeatherSample::CLEAR
        );
    }

    /// The opening frames must not spend the one-time snap on the clear
    /// placeholder: a session that starts mid-downpour has to present as a
    /// downpour, not ramp into one.
    #[test]
    fn the_one_time_snap_lands_on_the_sims_first_sample() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(WeatherPlugin::new(WeatherSettings::default()));
        app.update();
        app.update();

        let mut downpour = WeatherSample::CLEAR;
        downpour.cloud_cover = 0.97;
        downpour.precipitation = 0.85;
        downpour.visibility_m = 60.0;
        downpour.surface_wetness = 0.9;
        app.world_mut()
            .resource_mut::<WorldWeatherState>()
            .receive(downpour, 3.0);
        app.update();

        let visual = *app.world().resource::<SmoothedWeather>();
        assert_eq!(visual.cloud_cover, downpour.cloud_cover as f32);
        assert_eq!(visual.precipitation, downpour.precipitation as f32);
        assert_eq!(visual.visibility_m, downpour.visibility_m as f32);
        assert_eq!(visual.surface_wetness, downpour.surface_wetness as f32);
    }

    /// Before the sim speaks, the projection sits on the clear default rather
    /// than easing anywhere — and the snap is still unspent.
    #[test]
    fn a_silent_sim_leaves_the_projection_on_the_clear_default() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(WeatherPlugin::new(WeatherSettings::default()));
        app.update();

        let visual = *app.world().resource::<SmoothedWeather>();
        assert!(!visual.initialized);
        assert_eq!(visual.cloud_cover, WeatherSample::CLEAR.cloud_cover as f32);
        assert_eq!(visual.precipitation, 0.0);
    }

    #[test]
    fn bridge_receive_does_not_need_a_world_mirror() {
        let mut state = WorldWeatherState::default();
        let mut rain = WeatherSample::CLEAR;
        rain.precipitation = 0.5;
        state.receive(rain, 12.0);
        assert_eq!(state.current.precipitation, 0.5);
        assert_eq!(state.previous, WeatherSample::CLEAR);
        assert_eq!(state.received_at_seconds, 12.0);
    }
}
