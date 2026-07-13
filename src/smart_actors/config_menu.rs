//! Esc-driven settings menu for the voice backends.
//!
//! Each settings row shows the setting name, a cloud/local pill toggle, and
//! the live status of the selected backend. Choices persist to `config.ron`;
//! NPC voices only persist once the engine confirms the switch.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::config::PersistedConfig;
use crate::fonts::CathedralFonts;

use super::bridge::{self, TranscriptionBackend, TtsBackend};
use super::hud::{self, ConnectionUiState, SmartActorHudState, VoiceModelUiState};
use super::interaction::MicrophoneInputState;
use super::{SmartActorRuntime, request_tts_backend};

const SCRIM: Color = Color::srgba(0.01, 0.015, 0.03, 0.55);
const PANEL_SOLID: Color = Color::srgba(0.025, 0.03, 0.045, 0.96);
const CONTINUE_BG: Color = Color::srgba(0.12, 0.13, 0.17, 1.0);
const CONTINUE_BG_HOVER: Color = Color::srgba(0.20, 0.22, 0.27, 1.0);
const PILL_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);
const PILL_SELECTED_LABEL: Color = Color::srgb(0.09, 0.10, 0.12);
const PILL_DISABLED_LABEL: Color = Color::srgba(0.70, 0.72, 0.74, 0.45);

#[derive(Resource, Debug, Default)]
pub struct ConfigMenuState {
    pub open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingRow {
    NpcVoices,
    PlayerVoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BackendChoice {
    Cloud,
    Local,
}

#[derive(Component)]
pub(super) struct ConfigMenuRoot;

#[derive(Component)]
pub(super) struct ContinueButton;

#[derive(Component)]
pub(super) struct BackendPill {
    row: SettingRow,
    choice: BackendChoice,
}

#[derive(Component)]
pub(super) struct RowStatusText(SettingRow);

pub(super) fn spawn_config_menu(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
    let body_font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();
    let display_font = fonts
        .as_deref()
        .map(CathedralFonts::display)
        .unwrap_or_default();

    commands
        .spawn((
            Name::new("Config menu overlay"),
            ConfigMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(SCRIM),
            ZIndex(40),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Name::new("Config menu panel"),
                    Node {
                        width: px(940),
                        max_width: percent(94),
                        padding: UiRect::axes(px(26), px(22)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(16),
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BackgroundColor(PANEL_SOLID),
                ))
                .with_children(|panel| {
                    spawn_continue_row(panel, display_font);
                    spawn_setting_row(
                        panel,
                        SettingRow::NpcVoices,
                        "NPC voices' text to speech (TTS) model",
                        "Cloud OpenAI",
                        "Local Pocket TTS",
                        &body_font,
                    );
                    spawn_setting_row(
                        panel,
                        SettingRow::PlayerVoice,
                        "Your voice's speech to text (STT) model",
                        "Cloud OpenAI",
                        "Local Canary-Qwen",
                        &body_font,
                    );
                });
        });
}

fn spawn_continue_row(panel: &mut ChildSpawnerCommands, font: FontSource) {
    panel
        .spawn((
            Name::new("Continue"),
            Button,
            ContinueButton,
            Node {
                width: percent(100),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(14), px(10)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(CONTINUE_BG),
        ))
        .with_child((
            Text::new("Continue"),
            TextFont {
                font,
                font_size: FontSize::Px(19.0),
                ..default()
            },
            TextColor(hud::TEXT),
        ));
}

fn spawn_setting_row(
    panel: &mut ChildSpawnerCommands,
    row: SettingRow,
    label: &str,
    cloud_label: &str,
    local_label: &str,
    font: &FontSource,
) {
    panel
        .spawn((
            Name::new(format!("Setting row: {label}")),
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(16),
                ..default()
            },
        ))
        .with_children(|row_node| {
            row_node.spawn((
                Text::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(hud::TEXT),
                Node {
                    width: px(340),
                    ..default()
                },
            ));
            row_node
                .spawn((
                    Name::new("Backend pill toggle"),
                    Node {
                        flex_direction: FlexDirection::Row,
                        padding: UiRect::all(px(3)),
                        column_gap: px(3),
                        border_radius: BorderRadius::all(px(999)),
                        ..default()
                    },
                    BackgroundColor(hud::LOADER_TRACK),
                ))
                .with_children(|pill_group| {
                    spawn_pill(pill_group, row, BackendChoice::Cloud, cloud_label, font);
                    spawn_pill(pill_group, row, BackendChoice::Local, local_label, font);
                });
            row_node.spawn((
                RowStatusText(row),
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(14.5),
                    ..default()
                },
                TextColor(hud::MUTED),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
        });
}

fn spawn_pill(
    group: &mut ChildSpawnerCommands,
    row: SettingRow,
    choice: BackendChoice,
    label: &str,
    font: &FontSource,
) {
    group
        .spawn((
            Name::new(format!("Pill: {label}")),
            Button,
            BackendPill { row, choice },
            Node {
                padding: UiRect::axes(px(14), px(6)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(999)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font: font.clone(),
                font_size: FontSize::Px(13.5),
                ..default()
            },
            TextColor(hud::MUTED),
        ));
}

pub(super) fn toggle_config_menu(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<ConfigMenuState>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        let open = !menu.open;
        set_menu_open(&mut menu, &mut cursor, open);
    }
}

fn set_menu_open(menu: &mut ConfigMenuState, cursor: &mut CursorOptions, open: bool) {
    menu.open = open;
    cursor.visible = open;
    cursor.grab_mode = if open {
        CursorGrabMode::None
    } else {
        CursorGrabMode::Locked
    };
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_config_menu_buttons(
    mut menu: ResMut<ConfigMenuState>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    continue_clicks: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
    pill_clicks: Query<(&Interaction, &BackendPill), Changed<Interaction>>,
    handle: Option<Res<bridge::BridgeHandle>>,
    mut runtime: Option<ResMut<SmartActorRuntime>>,
    mut microphone: Option<ResMut<MicrophoneInputState>>,
    mut hud_state: ResMut<SmartActorHudState>,
) {
    if !menu.open {
        return;
    }
    if continue_clicks
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        set_menu_open(&mut menu, &mut cursor, false);
    }
    for (interaction, pill) in &pill_clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match pill.row {
            SettingRow::NpcVoices => {
                let (Some(handle), Some(runtime)) = (handle.as_deref(), runtime.as_deref_mut())
                else {
                    hud_state.toast("NPC voice selection is unavailable: actors are disabled");
                    continue;
                };
                let backend = match pill.choice {
                    BackendChoice::Cloud => TtsBackend::Cloud,
                    BackendChoice::Local => TtsBackend::Local,
                };
                request_tts_backend(runtime, handle, &mut hud_state, backend);
            }
            SettingRow::PlayerVoice => {
                let Some(microphone) = microphone.as_deref_mut() else {
                    hud_state.toast("Transcription selection is unavailable: actors are disabled");
                    continue;
                };
                let backend = match pill.choice {
                    BackendChoice::Cloud => TranscriptionBackend::Cloud,
                    BackendChoice::Local => TranscriptionBackend::Local,
                };
                if microphone.stt_backend != backend {
                    microphone.stt_backend = backend;
                    hud_state.toast(match backend {
                        TranscriptionBackend::Cloud => "Transcription: cloud model",
                        TranscriptionBackend::Local => "Transcription: local Canary-Qwen FP16",
                    });
                }
            }
        }
    }
}

/// Writes confirmed backend selections back to `config.ron`, whether they came
/// from the settings menu or the X/Z keyboard toggles.
pub(super) fn persist_backend_selections(
    mut runtime: Option<ResMut<SmartActorRuntime>>,
    microphone: Option<Res<MicrophoneInputState>>,
    persisted: Option<ResMut<PersistedConfig>>,
    mut hud_state: ResMut<SmartActorHudState>,
) {
    let Some(mut persisted) = persisted else {
        return;
    };
    let mut changed = false;
    if let Some(runtime) = runtime.as_deref_mut()
        && runtime.tts_selection_dirty
    {
        runtime.tts_selection_dirty = false;
        let backend = runtime.tts_selected.name();
        if persisted.0.smart_actors.tts_backend != backend {
            persisted.0.smart_actors.tts_backend = backend.into();
            changed = true;
        }
    }
    if let Some(microphone) = microphone.as_deref() {
        let backend = microphone.stt_backend.name();
        if persisted.0.smart_actors.stt_backend != backend {
            persisted.0.smart_actors.stt_backend = backend.into();
            changed = true;
        }
    }
    if changed && let Err(error) = persisted.save() {
        hud_state.toast(error);
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn update_config_menu(
    menu: Res<ConfigMenuState>,
    runtime: Option<Res<SmartActorRuntime>>,
    microphone: Option<Res<MicrophoneInputState>>,
    hud_state: Res<SmartActorHudState>,
    mut root: Query<&mut Node, With<ConfigMenuRoot>>,
    mut pills: Query<(&BackendPill, &Interaction, &mut BackgroundColor, &Children)>,
    mut continue_button: Query<
        (&Interaction, &mut BackgroundColor),
        (With<ContinueButton>, Without<BackendPill>),
    >,
    mut statuses: Query<(&RowStatusText, &mut Text, &mut TextColor)>,
    mut pill_labels: Query<&mut TextColor, Without<RowStatusText>>,
) {
    let Ok(mut root_node) = root.single_mut() else {
        return;
    };
    let desired = if menu.open {
        Display::Flex
    } else {
        Display::None
    };
    if root_node.display != desired {
        root_node.display = desired;
    }
    if !menu.open {
        return;
    }

    let runtime = runtime.as_deref();
    // Show the requested backend immediately while the switch is confirming.
    let tts_target = runtime.map(|runtime| {
        runtime
            .tts_selection_pending
            .as_ref()
            .map_or(runtime.tts_selected, |(_, backend)| *backend)
    });
    let stt_selected = microphone
        .as_deref()
        .map(|microphone| microphone.stt_backend);

    for (interaction, mut background) in &mut continue_button {
        background.0 = if *interaction == Interaction::None {
            CONTINUE_BG
        } else {
            CONTINUE_BG_HOVER
        };
    }

    for (pill, interaction, mut background, children) in &mut pills {
        let selected = match (pill.row, pill.choice) {
            (SettingRow::NpcVoices, BackendChoice::Cloud) => tts_target == Some(TtsBackend::Cloud),
            (SettingRow::NpcVoices, BackendChoice::Local) => tts_target == Some(TtsBackend::Local),
            (SettingRow::PlayerVoice, BackendChoice::Cloud) => {
                stt_selected == Some(TranscriptionBackend::Cloud)
            }
            (SettingRow::PlayerVoice, BackendChoice::Local) => {
                stt_selected == Some(TranscriptionBackend::Local)
            }
        };
        let available = runtime.is_some_and(|runtime| match (pill.row, pill.choice) {
            (SettingRow::NpcVoices, BackendChoice::Cloud) => runtime.tts_cloud_available,
            (SettingRow::NpcVoices, BackendChoice::Local) => runtime.tts_local_available,
            (SettingRow::PlayerVoice, BackendChoice::Cloud) => runtime.stt_cloud_available,
            (SettingRow::PlayerVoice, BackendChoice::Local) => runtime.stt_local_available,
        });
        let (pill_background, label_color) = if selected {
            (hud::TEXT, PILL_SELECTED_LABEL)
        } else if !available {
            (Color::NONE, PILL_DISABLED_LABEL)
        } else if *interaction == Interaction::None {
            (Color::NONE, hud::MUTED)
        } else {
            (PILL_HOVER, hud::TEXT)
        };
        background.0 = pill_background;
        for child in children {
            if let Ok(mut label) = pill_labels.get_mut(*child) {
                label.0 = label_color;
            }
        }
    }

    for (row, mut text, mut color) in &mut statuses {
        let (value, value_color) = match row.0 {
            SettingRow::NpcVoices => tts_status_line(hud_state.connection, runtime),
            SettingRow::PlayerVoice => {
                stt_status_line(hud_state.connection, &hud_state, stt_selected)
            }
        };
        if text.0 != value {
            text.0 = value;
        }
        color.0 = value_color;
    }
}

fn tts_status_line(
    connection: ConnectionUiState,
    runtime: Option<&SmartActorRuntime>,
) -> (String, Color) {
    match connection {
        ConnectionUiState::Disabled => ("Disabled in config.ron".into(), hud::OFFLINE),
        ConnectionUiState::Starting => ("Starting…".into(), hud::DEGRADED),
        ConnectionUiState::Offline => ("Actor service offline".into(), hud::OFFLINE),
        ConnectionUiState::Online => match runtime {
            Some(runtime) if runtime.tts_selection_pending.is_some() => {
                ("Switching…".into(), hud::DEGRADED)
            }
            Some(runtime) if runtime.tts_selected == TtsBackend::Off => {
                ("Off — press X to cycle".into(), hud::MUTED)
            }
            Some(runtime) if !runtime.tts_available => ("Unavailable".into(), hud::OFFLINE),
            Some(_) => ("Working".into(), hud::ONLINE),
            None => ("Unavailable".into(), hud::OFFLINE),
        },
    }
}

fn stt_status_line(
    connection: ConnectionUiState,
    hud_state: &SmartActorHudState,
    selected: Option<TranscriptionBackend>,
) -> (String, Color) {
    match connection {
        ConnectionUiState::Disabled => ("Disabled in config.ron".into(), hud::OFFLINE),
        ConnectionUiState::Starting => ("Starting…".into(), hud::DEGRADED),
        ConnectionUiState::Offline => ("Actor service offline".into(), hud::OFFLINE),
        ConnectionUiState::Online => {
            let Some(selected) = selected else {
                return ("Unavailable".into(), hud::OFFLINE);
            };
            let (state, detail) = hud_state.transcription_status(selected);
            let label = match state {
                VoiceModelUiState::Checking => "Checking…",
                VoiceModelUiState::NotLoaded => "Standby",
                VoiceModelUiState::Loading => "Loading…",
                VoiceModelUiState::Transcribing => "Transcribing…",
                VoiceModelUiState::Ready => "Working",
                VoiceModelUiState::Failed => "Failed",
                VoiceModelUiState::Unavailable => "Unavailable",
            };
            let detail: String = detail.chars().take(64).collect();
            let text = if detail.is_empty() {
                label.to_string()
            } else {
                format!("{label} — {detail}")
            };
            (text, state.color())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_spawns_hidden_with_all_controls() {
        let mut app = App::new();
        app.add_systems(Startup, spawn_config_menu);
        app.update();

        let mut root_query = app
            .world_mut()
            .query_filtered::<&Node, With<ConfigMenuRoot>>();
        let root = root_query.single(app.world()).expect("menu root exists");
        assert_eq!(root.display, Display::None);

        let mut pill_query = app.world_mut().query::<&BackendPill>();
        assert_eq!(pill_query.iter(app.world()).count(), 4);

        let mut continue_query = app.world_mut().query::<&ContinueButton>();
        assert_eq!(continue_query.iter(app.world()).count(), 1);

        let mut status_query = app.world_mut().query::<&RowStatusText>();
        assert_eq!(status_query.iter(app.world()).count(), 2);
    }

    #[test]
    fn tts_status_reports_lifecycle() {
        let mut runtime = SmartActorRuntime::starting(false);
        assert_eq!(
            tts_status_line(ConnectionUiState::Offline, Some(&runtime)).0,
            "Actor service offline"
        );

        runtime.connected = true;
        runtime.ready = true;
        runtime.tts_available = true;
        runtime.tts_selected = TtsBackend::Local;
        assert_eq!(
            tts_status_line(ConnectionUiState::Online, Some(&runtime)).0,
            "Working"
        );

        runtime.tts_selection_pending = Some(("tts-mode-1".into(), TtsBackend::Cloud));
        assert_eq!(
            tts_status_line(ConnectionUiState::Online, Some(&runtime)).0,
            "Switching…"
        );

        runtime.tts_selection_pending = None;
        runtime.tts_selected = TtsBackend::Off;
        assert_eq!(
            tts_status_line(ConnectionUiState::Online, Some(&runtime)).0,
            "Off — press X to cycle"
        );
    }

    #[test]
    fn stt_status_follows_selected_backend() {
        let mut hud_state = SmartActorHudState::default();
        hud_state.set_transcription_capabilities(true, true);

        let (cloud, _) = stt_status_line(
            ConnectionUiState::Online,
            &hud_state,
            Some(TranscriptionBackend::Cloud),
        );
        assert!(cloud.starts_with("Working"), "was: {cloud}");

        hud_state.apply_transcription_status("local", "loading", Some("Downloading model"));
        let (local, _) = stt_status_line(
            ConnectionUiState::Online,
            &hud_state,
            Some(TranscriptionBackend::Local),
        );
        assert_eq!(local, "Loading… — Downloading model");
    }
}
