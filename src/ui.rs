use bevy::prelude::*;

use crate::{controller::PlayerController, fonts::CathedralFonts};

const PANEL_BACKGROUND: Color = Color::srgba(0.025, 0.03, 0.045, 0.78);
const TEXT_PRIMARY: Color = Color::srgb(0.94, 0.92, 0.84);
const TEXT_MUTED: Color = Color::srgb(0.66, 0.68, 0.70);
const WALK_COLOR: Color = Color::srgb(0.72, 0.82, 0.67);
const FLY_COLOR: Color = Color::srgb(0.55, 0.78, 0.96);

/// Adds the unobtrusive in-game controls, crosshair, and movement-mode display.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, update_flight_status);
    }
}

/// Marks the text that reports whether the player is walking or flying.
#[derive(Component)]
pub struct FlightStatusText;

fn spawn_hud(mut commands: Commands, fonts: Res<CathedralFonts>) {
    spawn_crosshair(&mut commands);
    spawn_controls(&mut commands, &fonts);
    spawn_flight_status(&mut commands, &fonts);
}

fn spawn_crosshair(commands: &mut Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: px(18),
                    height: px(18),
                    ..default()
                })
                .with_children(|crosshair| {
                    let crosshair_color = BackgroundColor(Color::srgba(0.96, 0.95, 0.90, 0.88));

                    crosshair.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(1),
                            top: px(8),
                            width: px(16),
                            height: px(2),
                            ..default()
                        },
                        crosshair_color,
                    ));

                    crosshair.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(8),
                            top: px(1),
                            width: px(2),
                            height: px(16),
                            ..default()
                        },
                        crosshair_color,
                    ));
                });
        });
}

fn spawn_controls(commands: &mut Commands, fonts: &CathedralFonts) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(18),
                left: px(18),
                padding: UiRect::axes(px(14), px(11)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(PANEL_BACKGROUND),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("THE CATHEDRAL OF IMPOSSIBLE LIGHT"),
                TextFont {
                    font: fonts.display(),
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(TEXT_PRIMARY),
            ));

            panel.spawn((
                Text::new("WASD  move   ·   SHIFT  run   ·   SPACE  jump"),
                TextFont {
                    font: fonts.body(),
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(TEXT_MUTED),
            ));

            panel.spawn((
                Text::new("ENTER  chat   ·   F  fart   ·   '  flight   ·   M  map   ·   I  pockets   ·   C  chalk   ·   B  areas   ·   ESC  settings"),
                TextFont {
                    font: fonts.body(),
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(TEXT_MUTED),
            ));
        });
}

fn spawn_flight_status(commands: &mut Commands, fonts: &CathedralFonts) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(18),
                bottom: px(18),
                min_width: px(94),
                padding: UiRect::axes(px(14), px(9)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(1),
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(PANEL_BACKGROUND),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("MOVEMENT"),
                TextFont {
                    font: fonts.display(),
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(TEXT_MUTED),
            ));

            panel.spawn((
                FlightStatusText,
                Text::new("WALK"),
                TextFont {
                    font: fonts.display(),
                    font_size: FontSize::Px(19.0),
                    ..default()
                },
                TextColor(WALK_COLOR),
            ));
        });
}

fn update_flight_status(
    controllers: Query<&PlayerController>,
    mut labels: Query<(&mut Text, &mut TextColor), With<FlightStatusText>>,
) {
    let Ok(controller) = controllers.single() else {
        return;
    };
    let Ok((mut label, mut color)) = labels.single_mut() else {
        return;
    };

    let (next_label, next_color) = if controller.flying {
        ("FLY", FLY_COLOR)
    } else {
        ("WALK", WALK_COLOR)
    };

    if label.0 != next_label {
        label.0 = next_label.into();
        color.0 = next_color;
    }
}
