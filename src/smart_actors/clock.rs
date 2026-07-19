//! Host-side world clock (movement M0).
//!
//! The sim owns the clock — it is a pure projection of the `now` the host
//! already hands [`cathedral_sim::Engine::poll`], and it rings the offices and
//! publishes an [`cathedral_sim::EngineMessage::Clock`] every poll. This module
//! is only the game's side of that: it projects the message into a resource, the
//! [`WorldClockState`]; drives the one directional sun and the ambient fill from
//! it; prints the hour on the HUD; and cycles the debug time scale on the `T`
//! key. Nothing here decides the time — it only reads and renders it.

use bevy::light::light_consts::lux;
use bevy::prelude::*;
use cathedral_sim::{Office, Weekday};

use crate::fonts::CathedralFonts;
use crate::scene::Sun;

use super::bridge::{BridgeCommand, BridgeHandle};

/// Brightness at or below this is full night — the sun is out, only the ambient
/// fill and the interior lamps remain. Matches the default
/// `clock.night_brightness` the sim floors at.
const NIGHT_FLOOR: f32 = 0.05;
/// Ambient sky fill at night and at full day (the day value matches scene.rs's
/// `GlobalAmbientLight.brightness`; both dropped from 300 when SSAO arrived —
/// a strong constant term fills back in exactly the crevices SSAO darkens).
const NIGHT_AMBIENT: f32 = 14.0;
const DAY_AMBIENT: f32 = 110.0;

/// The sun follows a real solar arc for a **temperate-summer** sky — a
/// central-European latitude at the June solstice. The sun rises ~04:00, so the
/// Dayspring (07:00) already opens on a high, bright morning (~30° elevation),
/// and noon peaks ~65°. `drive_sun` turns these into the sun's elevation and
/// azimuth each frame with the standard horizontal-coordinate transform.
const SUN_LATITUDE_DEG: f32 = 48.0;
const SUN_DECLINATION_DEG: f32 = 23.44;

/// The game's copy of the sim clock, refreshed from
/// [`cathedral_sim::EngineMessage::Clock`] every poll. `present` stays false
/// until the first message, so the sun keeps its spawned pose until the engine
/// has said what time it is.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WorldClockState {
    pub present: bool,
    pub day: i64,
    pub fraction: f64,
    pub office: Office,
    pub weekday: Weekday,
    pub brightness: f64,
    pub scale: f64,
    /// Real seconds per game day at 1× — with `scale`, what a wall-clock
    /// second is worth in sim time (the chimney smoke back-dates puff births
    /// through it).
    pub seconds_per_day: f64,
}

impl Default for WorldClockState {
    fn default() -> Self {
        Self {
            present: false,
            day: 0,
            fraction: 7.0 / 24.0,
            office: Office::Dayspring,
            weekday: Weekday::Bellday,
            brightness: 1.0,
            scale: 1.0,
            seconds_per_day: 3600.0,
        }
    }
}

impl WorldClockState {
    /// Clock hour and minute for the HUD readout.
    fn hour_minute(&self) -> (i64, i64) {
        let minutes = (self.fraction * 24.0 * 60.0).round() as i64;
        (minutes.div_euclid(60).rem_euclid(24), minutes.rem_euclid(60))
    }
}

/// Marker on the clock HUD's text node.
#[derive(Component)]
pub struct ClockHudText;

/// Rotate the one directional sun and set its strength and the ambient fill from
/// the clock: the sun arcs east→overhead→west across the day, is fully out at
/// night, and the physical [`bevy::light::Atmosphere`] colours dawn and dusk for
/// free.
pub fn drive_sun(
    clock: Res<WorldClockState>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    ambient: Option<ResMut<GlobalAmbientLight>>,
) {
    if !clock.present {
        return;
    }
    let Ok((mut transform, mut light)) = sun.single_mut() else {
        return;
    };

    // A real solar arc, not a fudge curve. Elevation and azimuth come from the
    // hour angle through the standard horizontal-coordinate transform, for a
    // temperate-summer sky (SUN_LATITUDE_DEG at SUN_DECLINATION_DEG). Because
    // the sun rises ~04:00 in that sky, by the Dayspring (07:00) it already
    // stands ~30° above the horizon — a bright, high morning — and it peaks
    // ~65° at noon. The physical `bevy::light::Atmosphere` still colours the
    // low sun at dawn and dusk for free.
    let fraction = clock.fraction as f32;
    let lat = SUN_LATITUDE_DEG.to_radians();
    let decl = SUN_DECLINATION_DEG.to_radians();
    let (sin_lat, cos_lat) = lat.sin_cos();
    let (sin_decl, cos_decl) = decl.sin_cos();
    // Hour angle: 0 at solar noon, −75° at 07:00. World axes: +x east, +y up,
    // +z south (a northern-hemisphere sun stays on the south side of the sky).
    let hour_angle = (fraction - 0.5) * std::f32::consts::TAU;
    let (sin_h, cos_h) = hour_angle.sin_cos();
    let up = sin_lat * sin_decl + cos_lat * cos_decl * cos_h; // = sin(elevation)
    let east = -cos_decl * sin_h;
    let south = (sin_lat * up - sin_decl) / cos_lat;
    // A shallow floor keeps the key from beaming up from underground during the
    // dusk ramp: the sun sets ~19:55 but the brightness floor lingers to the
    // Snuffing (21:00), so let it graze the horizon rather than light from below.
    let mut direction = Vec3::new(east, up, south);
    direction.y = direction.y.max(-0.15);
    let direction = direction.normalize();
    *transform =
        Transform::from_translation(direction * 500.0).looking_at(Vec3::new(0.0, 0.0, 40.0), Vec3::Y);

    // Night is genuinely dark: the sun fades fully out at the floor, full at noon.
    let lit = ((clock.brightness as f32 - NIGHT_FLOOR) / (1.0 - NIGHT_FLOOR)).clamp(0.0, 1.0);
    light.illuminance = lux::RAW_SUNLIGHT * lit;
    if let Some(mut ambient) = ambient {
        // sqrt: the shadowed side of the city is all the player sees at dawn
        // and dusk, so the fill recovers faster than the key — the mid-ramp
        // fill sits at ~84% of the day value instead of ~71%.
        ambient.brightness = NIGHT_AMBIENT + (DAY_AMBIENT - NIGHT_AMBIENT) * lit.sqrt();
    }
}

/// Spawn the clock readout — a small panel, top-centre.
pub fn spawn_clock_hud(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
    let body_font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();
    commands.spawn((
        Name::new("World clock readout"),
        ClockHudText,
        Text::new(""),
        TextFont {
            font: body_font,
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.92, 0.97)),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Percent(34.0),
            width: Val::Percent(32.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.66)),
        ZIndex(12),
    ));
}

/// Keep the readout current: office, hour, day, weekday, and the debug scale.
pub fn update_clock_hud(
    clock: Res<WorldClockState>,
    mut readouts: Query<&mut Text, With<ClockHudText>>,
) {
    let Ok(mut text) = readouts.single_mut() else {
        return;
    };
    if !clock.present {
        text.0.clear();
        return;
    }
    let (hour, minute) = clock.hour_minute();
    let scale = clock.scale.round() as i64;
    text.0 = format!(
        "{}   {hour:02}:{minute:02}   ·   Day {} {}   ·   {scale}×",
        clock.office.label(),
        clock.day,
        clock.weekday.label(),
    );
}

/// The `T` key cycles the debug time scale (1× / 10× / 60×). The sim owns the
/// cycle and the continuity; the host only asks for the next step, and learns
/// the result from the next clock message.
pub fn handle_time_scale_key(keyboard: Res<ButtonInput<KeyCode>>, handle: Option<Res<BridgeHandle>>) {
    if !keyboard.just_pressed(KeyCode::KeyT) {
        return;
    }
    if let Some(handle) = handle {
        let _ = handle.try_send(BridgeCommand::CycleTimeScale);
    }
}
