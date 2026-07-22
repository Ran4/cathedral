//! Host-side world clock (movement M0).
//!
//! The sim owns the clock — it is a pure projection of the `now` the host
//! already hands [`cathedral_sim::Engine::poll`], and it rings the offices and
//! publishes an [`cathedral_sim::EngineMessage::Clock`] every poll. This module
//! is only the game's side of that: it projects the message into a resource, the
//! [`WorldClockState`], prints the hour on the HUD, and cycles the debug time
//! scale on the `T` key. The weather environment compositor is the sole writer
//! of sun and ambient light. Nothing here decides the time — it only projects it.

use bevy::prelude::*;
use cathedral_sim::{Office, Weekday};

use super::bridge::{BridgeCommand, BridgeHandle};
use crate::fonts::CathedralFonts;

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
        (
            minutes.div_euclid(60).rem_euclid(24),
            minutes.rem_euclid(60),
        )
    }
}

/// Marker on the clock HUD's text node.
#[derive(Component)]
pub struct ClockHudText;

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
pub fn handle_time_scale_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    handle: Option<Res<BridgeHandle>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyT) {
        return;
    }
    if let Some(handle) = handle {
        let _ = handle.try_send(BridgeCommand::CycleTimeScale);
    }
}
