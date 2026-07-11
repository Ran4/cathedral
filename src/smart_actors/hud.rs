//! Smart-actor status, inventory, and offer-card HUD.

use std::time::Duration;

use bevy::prelude::*;

use crate::fonts::CathedralFonts;

const PANEL: Color = Color::srgba(0.025, 0.03, 0.045, 0.88);
const CENTERED_TEXT_BACKDROP: Color = Color::srgba(0.10, 0.105, 0.11, 0.80);
const TEXT: Color = Color::srgb(0.96, 0.94, 0.86);
const MUTED: Color = Color::srgb(0.70, 0.72, 0.74);
const ONLINE: Color = Color::srgb(0.58, 0.88, 0.62);
const DEGRADED: Color = Color::srgb(0.97, 0.74, 0.31);
const OFFLINE: Color = Color::srgb(0.96, 0.40, 0.36);
const LOADER_TRACK: Color = Color::srgba(0.24, 0.25, 0.27, 0.92);
const PLAYER_TRANSCRIPT_LIFETIME: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoiceModelUiState {
    Checking,
    NotLoaded,
    Loading,
    Transcribing,
    Ready,
    Failed,
    Unavailable,
}

impl VoiceModelUiState {
    fn label(self) -> &'static str {
        match self {
            Self::Checking => "CHECKING",
            Self::NotLoaded => "STANDBY",
            Self::Loading => "LOADING",
            Self::Transcribing => "TRANSCRIBING",
            Self::Ready => "READY",
            Self::Failed => "FAILED",
            Self::Unavailable => "UNAVAILABLE",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Ready => ONLINE,
            Self::Checking | Self::NotLoaded | Self::Loading | Self::Transcribing => DEGRADED,
            Self::Failed | Self::Unavailable => OFFLINE,
        }
    }

    fn busy(self) -> bool {
        matches!(self, Self::Checking | Self::Loading | Self::Transcribing)
    }
}

#[derive(Debug, Clone)]
struct VoiceBackendUi {
    state: VoiceModelUiState,
    detail: String,
}

impl VoiceBackendUi {
    fn checking() -> Self {
        Self {
            state: VoiceModelUiState::Checking,
            detail: "Waiting for the actor service".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionUiState {
    Disabled,
    Starting,
    Online,
    Offline,
}

impl ConnectionUiState {
    fn label(self) -> &'static str {
        match self {
            Self::Disabled => "ACTORS DISABLED",
            Self::Starting => "ACTORS STARTING",
            Self::Online => "ACTORS ONLINE",
            Self::Offline => "ACTORS OFFLINE",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Online => ONLINE,
            Self::Starting => DEGRADED,
            Self::Disabled | Self::Offline => OFFLINE,
        }
    }
}

#[derive(Debug, Clone)]
struct TimedMessage {
    text: String,
    remaining: Duration,
}

/// Presentation-only state. Authoritative inventory and offers never live here.
#[derive(Resource, Debug)]
pub struct SmartActorHudState {
    pub connection: ConnectionUiState,
    pub connection_detail: String,
    pub inventory: String,
    pub offer_card: String,
    pub focus_hint: String,
    pub subtitle: String,
    pub microphone_available: bool,
    pub microphone_unavailable: bool,
    /// Persistent user preference controlled by V, distinct from temporary
    /// worker pauses during protocol resynchronization.
    pub microphone_enabled: bool,
    pub listening: bool,
    pub transcription_backend: String,
    cloud_voice: VoiceBackendUi,
    local_voice: VoiceBackendUi,
    voice_loader_phase: f32,
    transient: Option<TimedMessage>,
    player_transcript: Option<TimedMessage>,
}

impl Default for SmartActorHudState {
    fn default() -> Self {
        Self {
            connection: ConnectionUiState::Starting,
            connection_detail: "Launching the local character service…".into(),
            inventory: String::new(),
            offer_card: String::new(),
            focus_hint: String::new(),
            subtitle: String::new(),
            microphone_available: false,
            microphone_unavailable: false,
            microphone_enabled: true,
            listening: false,
            transcription_backend: "CLOUD".into(),
            cloud_voice: VoiceBackendUi::checking(),
            local_voice: VoiceBackendUi::checking(),
            voice_loader_phase: 0.0,
            transient: None,
            player_transcript: None,
        }
    }
}

impl SmartActorHudState {
    pub fn toast(&mut self, message: impl Into<String>) {
        let text = message.into();
        if text.is_empty() {
            return;
        }
        self.transient = Some(TimedMessage {
            text,
            remaining: Duration::from_secs(4),
        });
    }

    pub fn show_player_transcript(&mut self, transcript: &str) {
        let transcript = transcript.trim();
        if transcript.is_empty() {
            return;
        }
        self.set_player_transcript(format!("You: {transcript}"));
    }

    pub fn show_player_transcript_delivery(&mut self, transcript: &str, recipients: usize) {
        let transcript = transcript.trim();
        if transcript.is_empty() {
            return;
        }
        let delivery = match recipients {
            0 => "nobody nearby".to_string(),
            1 => "heard by 1 nearby person".to_string(),
            count => format!("heard by {count} nearby people"),
        };
        self.set_player_transcript(format!("You: {transcript}  ·  {delivery}"));
    }

    fn set_player_transcript(&mut self, text: String) {
        self.player_transcript = Some(TimedMessage {
            text,
            remaining: PLAYER_TRANSCRIPT_LIFETIME,
        });
    }

    #[cfg(test)]
    pub(super) fn player_transcript_text(&self) -> Option<&str> {
        self.player_transcript
            .as_ref()
            .map(|message| message.text.as_str())
    }

    pub fn clear_transients_on_disconnect(&mut self, detail: impl Into<String>) {
        self.connection = ConnectionUiState::Offline;
        self.connection_detail = detail.into();
        self.offer_card.clear();
        self.focus_hint.clear();
        self.subtitle.clear();
        self.player_transcript = None;
        self.microphone_available = false;
        self.microphone_unavailable = false;
        self.listening = false;
        self.cloud_voice = VoiceBackendUi::checking();
        self.local_voice = VoiceBackendUi::checking();
        self.transient = None;
    }

    pub fn set_transcription_capabilities(&mut self, cloud: bool, local: bool) {
        self.cloud_voice = if cloud {
            VoiceBackendUi {
                state: VoiceModelUiState::Ready,
                detail: "Cloud transcription is ready".into(),
            }
        } else {
            VoiceBackendUi {
                state: VoiceModelUiState::Unavailable,
                detail: "Cloud transcription is not configured".into(),
            }
        };
        self.local_voice = if local {
            VoiceBackendUi {
                state: VoiceModelUiState::NotLoaded,
                detail: "Model loads after the first local utterance".into(),
            }
        } else {
            VoiceBackendUi {
                state: VoiceModelUiState::Unavailable,
                detail: "Local transcription worker is unavailable".into(),
            }
        };
    }

    pub fn apply_transcription_status(
        &mut self,
        backend: &str,
        state: &str,
        message: Option<&str>,
    ) {
        let target = match backend {
            "cloud" => &mut self.cloud_voice,
            "local" => &mut self.local_voice,
            _ => return,
        };
        target.state = match state {
            "loading" => VoiceModelUiState::Loading,
            "transcribing" => VoiceModelUiState::Transcribing,
            "ready" | "idle" => VoiceModelUiState::Ready,
            "degraded" => VoiceModelUiState::Failed,
            "unavailable" => VoiceModelUiState::Unavailable,
            _ => target.state,
        };
        target.detail = message.map_or_else(
            || match target.state {
                VoiceModelUiState::Ready => "Ready for speech".into(),
                VoiceModelUiState::Loading => "Loading transcription model".into(),
                VoiceModelUiState::Transcribing => "Transcribing captured speech".into(),
                VoiceModelUiState::Failed => "Transcription failed".into(),
                VoiceModelUiState::Unavailable => "Transcription is unavailable".into(),
                VoiceModelUiState::Checking | VoiceModelUiState::NotLoaded => String::new(),
            },
            |message| message.chars().take(96).collect(),
        );
    }

    fn selected_voice(&self) -> &VoiceBackendUi {
        if self.transcription_backend == "CANARY-QWEN FP16" {
            &self.local_voice
        } else {
            &self.cloud_voice
        }
    }
}

#[derive(Component)]
pub(super) struct ConnectionText;
#[derive(Component)]
pub(super) struct ConnectionDetailText;
#[derive(Component)]
pub(super) struct InventoryText;
#[derive(Component)]
pub(super) struct OfferCardText;
#[derive(Component)]
pub(super) struct FocusHintText;
#[derive(Component)]
pub(super) struct PlayerTranscriptLayer;
#[derive(Component)]
pub(super) struct PlayerTranscriptText;
#[derive(Component)]
pub(super) struct VoicePanelText;
#[derive(Component)]
pub(super) struct VoiceLoaderFill;
#[derive(Component)]
pub(super) struct VoiceStatusPanel;
#[derive(Component)]
pub(super) struct ToastText;

pub fn spawn_smart_actor_hud(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
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
            Name::new("Smart actor status"),
            Node {
                position_type: PositionType::Absolute,
                top: px(18),
                right: px(18),
                min_width: px(190),
                padding: UiRect::axes(px(13), px(9)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(2),
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(PANEL),
            ZIndex(12),
        ))
        .with_children(|panel| {
            panel.spawn((
                ConnectionText,
                Text::new("ACTORS STARTING"),
                TextFont {
                    font: display_font.clone(),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(DEGRADED),
            ));
            panel.spawn((
                ConnectionDetailText,
                Text::new("Launching local service…"),
                TextFont {
                    font: body_font.clone(),
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(MUTED),
            ));
        });

    commands.spawn((
        Name::new("Smart actor inventory quickbar"),
        InventoryText,
        Text::new(""),
        TextFont {
            font: body_font.clone(),
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(TEXT),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(22),
            left: percent(20),
            width: percent(60),
            padding: UiRect::axes(px(14), px(9)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(7)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(PANEL),
        ZIndex(12),
    ));

    commands.spawn((
        Name::new("Incoming offer card"),
        OfferCardText,
        Text::new(""),
        TextFont {
            font: body_font.clone(),
            font_size: FontSize::Px(19.0),
            ..default()
        },
        TextColor(TEXT),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: percent(58),
            left: percent(28),
            width: percent(44),
            padding: UiRect::all(px(15)),
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(9)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(PANEL),
        ZIndex(12),
    ));

    spawn_centered_text(
        &mut commands,
        "Actor focus hint",
        FocusHintText,
        52.0,
        14.0,
        MUTED,
        body_font.clone(),
    );
    spawn_player_transcript(&mut commands, body_font.clone());
    spawn_voice_status_panel(&mut commands, body_font.clone(), display_font.clone());
    spawn_centered_text(
        &mut commands,
        "Smart actor toast",
        ToastText,
        73.0,
        14.0,
        DEGRADED,
        body_font,
    );
}

fn spawn_voice_status_panel(
    commands: &mut Commands,
    body_font: FontSource,
    title_font: FontSource,
) {
    commands
        .spawn((
            Name::new("Voice input status panel"),
            VoiceStatusPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(18),
                left: px(18),
                width: px(400),
                min_height: px(100),
                padding: UiRect::axes(px(12), px(9)),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                border_radius: BorderRadius::all(px(7)),
                ..default()
            },
            BackgroundColor(PANEL),
            ZIndex(12),
        ))
        .with_children(|panel| {
            panel.spawn((
                VoicePanelText,
                Text::new("VOICE INPUT\nMODEL   CLOUD\nMIC     CHECKING\nSTATUS  CHECKING"),
                TextFont {
                    font: body_font,
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(DEGRADED),
                TextLayout::justify(Justify::Left),
            ));
            panel
                .spawn((
                    Name::new("Voice loader track"),
                    Node {
                        width: percent(100),
                        height: px(8),
                        overflow: Overflow::clip(),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(LOADER_TRACK),
                ))
                .with_child((
                    Name::new("Voice loader fill"),
                    VoiceLoaderFill,
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(0),
                        width: percent(28),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(DEGRADED),
                ));
            panel.spawn((
                Text::new("[V] MICROPHONE     [Z] TRANSCRIPTION MODEL"),
                TextFont {
                    font: title_font,
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(MUTED),
            ));
        });
}

fn spawn_player_transcript(commands: &mut Commands, font: FontSource) {
    commands
        .spawn((
            Name::new("Player transcript centering layer"),
            PlayerTranscriptLayer,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(66),
                left: percent(20),
                width: percent(60),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ZIndex(12),
        ))
        .with_children(|layer| {
            layer.spawn((
                Name::new("Player transcript"),
                PlayerTranscriptText,
                Text::new(""),
                TextFont {
                    font,
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(TEXT),
                TextLayout::justify(Justify::Center),
                TextShadow::default(),
                Node {
                    max_width: percent(100),
                    padding: UiRect::axes(px(7), px(3)),
                    border_radius: BorderRadius::all(px(4)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(CENTERED_TEXT_BACKDROP),
            ));
        });
}

fn spawn_centered_text<M: Component>(
    commands: &mut Commands,
    name: &'static str,
    marker: M,
    top_percent: f32,
    font_size: f32,
    color: Color,
    font: FontSource,
) {
    // Keep the centering layer transparent and let its child size to the text.
    // This gives centered status text a compact backing instead of a wide
    // screen-spanning strip, while retaining the existing 60% wrap boundary.
    commands
        .spawn((
            Name::new(format!("{name} centering layer")),
            Node {
                position_type: PositionType::Absolute,
                top: percent(top_percent),
                left: percent(20),
                width: percent(60),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ZIndex(10),
        ))
        .with_children(|layer| {
            layer.spawn((
                Name::new(name),
                marker,
                Text::new(""),
                TextFont {
                    font,
                    font_size: FontSize::Px(font_size),
                    ..default()
                },
                TextColor(color),
                TextLayout::justify(Justify::Center),
                TextShadow::default(),
                Node {
                    max_width: percent(100),
                    padding: UiRect::axes(px(10), px(5)),
                    border_radius: BorderRadius::all(px(5)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(CENTERED_TEXT_BACKDROP),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn update_smart_actor_hud(
    time: Res<Time>,
    mut state: ResMut<SmartActorHudState>,
    mut loader: Query<(&mut Node, &mut BackgroundColor), (With<VoiceLoaderFill>, Without<Text>)>,
    mut views: Query<
        (
            &mut Text,
            Option<&mut TextColor>,
            Option<&mut Node>,
            Option<&ConnectionText>,
            Option<&ConnectionDetailText>,
            Option<&InventoryText>,
            Option<&OfferCardText>,
            Option<&FocusHintText>,
            Option<&PlayerTranscriptText>,
            Option<&VoicePanelText>,
            Option<&ToastText>,
        ),
        Or<(
            With<ConnectionText>,
            With<ConnectionDetailText>,
            With<InventoryText>,
            With<OfferCardText>,
            With<FocusHintText>,
            With<PlayerTranscriptText>,
            With<VoicePanelText>,
            With<ToastText>,
        )>,
    >,
) {
    if let Some(message) = state.transient.as_mut() {
        message.remaining = message.remaining.saturating_sub(time.delta());
        if message.remaining.is_zero() {
            state.transient = None;
        }
    }
    if let Some(message) = state.player_transcript.as_mut() {
        message.remaining = message.remaining.saturating_sub(time.delta());
        if message.remaining.is_zero() {
            state.player_transcript = None;
        }
    }

    let selected_state = state.selected_voice().state;
    let selected_detail: String = state.selected_voice().detail.chars().take(58).collect();
    let microphone = match (
        state.microphone_available,
        state.microphone_unavailable,
        state.microphone_enabled,
        state.listening,
    ) {
        (_, true, _, _) => "UNAVAILABLE",
        (true, false, true, true) => "VOICE DETECTION READY",
        (true, false, true, false) => "PAUSED",
        (true, false, false, _) => "OFF",
        (false, false, _, _) => "CHECKING",
    };
    let voice_panel_text = format!(
        "VOICE INPUT\nMODEL   {}\nMIC     {microphone}\nSTATUS  {} — {selected_detail}",
        state.transcription_backend,
        selected_state.label(),
    );
    if selected_state.busy() {
        state.voice_loader_phase = (state.voice_loader_phase + time.delta_secs() * 0.65) % 2.0;
    }
    let travel = if state.voice_loader_phase <= 1.0 {
        state.voice_loader_phase
    } else {
        2.0 - state.voice_loader_phase
    };
    for (mut node, mut color) in &mut loader {
        match selected_state {
            VoiceModelUiState::Checking
            | VoiceModelUiState::Loading
            | VoiceModelUiState::Transcribing => {
                node.left = percent(travel * 72.0);
                node.width = percent(28);
                color.0 = DEGRADED;
            }
            VoiceModelUiState::Ready => {
                node.left = percent(0);
                node.width = percent(100);
                color.0 = ONLINE;
            }
            VoiceModelUiState::NotLoaded => {
                node.left = percent(0);
                node.width = percent(18);
                color.0 = MUTED;
            }
            VoiceModelUiState::Failed | VoiceModelUiState::Unavailable => {
                node.left = percent(0);
                node.width = percent(100);
                color.0 = OFFLINE;
            }
        }
    }
    let toast_text = state
        .transient
        .as_ref()
        .map_or("", |message| message.text.as_str());
    let player_transcript_text = state
        .player_transcript
        .as_ref()
        .map_or("", |message| message.text.as_str());

    for (
        mut text,
        mut color,
        mut node,
        connection,
        detail,
        inventory,
        offer,
        hint,
        player_transcript,
        voice_panel,
        toast,
    ) in &mut views
    {
        if connection.is_some() {
            text.0 = state.connection.label().into();
            if let Some(color) = color.as_deref_mut() {
                color.0 = state.connection.color();
            }
        } else if detail.is_some() {
            text.0 = state.connection_detail.clone();
        } else if inventory.is_some() {
            set_optional_text(&state.inventory, &mut text, node.as_deref_mut());
        } else if offer.is_some() {
            set_optional_text(&state.offer_card, &mut text, node.as_deref_mut());
        } else if hint.is_some() {
            set_optional_text(&state.focus_hint, &mut text, node.as_deref_mut());
        } else if player_transcript.is_some() {
            set_optional_text(player_transcript_text, &mut text, node.as_deref_mut());
        } else if voice_panel.is_some() {
            if let Some(color) = color.as_deref_mut() {
                color.0 = selected_state.color();
            }
            set_optional_text(&voice_panel_text, &mut text, node.as_deref_mut());
        } else if toast.is_some() {
            set_optional_text(toast_text, &mut text, node.as_deref_mut());
        }
    }
}

fn set_optional_text(value: &str, text: &mut Text, node: Option<&mut Node>) {
    if text.0 != value {
        text.0 = value.into();
    }
    if let Some(node) = node {
        node.display = if value.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_readability_text_uses_compact_neutral_backdrops() {
        let mut app = App::new();
        app.add_systems(Startup, spawn_smart_actor_hud);
        app.update();

        let mut focus_query = app
            .world_mut()
            .query_filtered::<(&BackgroundColor, &Node), With<FocusHintText>>();
        let (focus_background, focus_node) = focus_query
            .iter(app.world())
            .next()
            .expect("focus hint exists");
        assert_eq!(focus_background.0, CENTERED_TEXT_BACKDROP);
        assert_eq!(focus_node.display, Display::None);
        assert_eq!(focus_node.max_width, percent(100));

        let mut caption_query = app
            .world_mut()
            .query_filtered::<(&TextFont, &BackgroundColor, &Node), With<PlayerTranscriptText>>();
        let (caption_font, caption_background, caption_node) = caption_query
            .iter(app.world())
            .next()
            .expect("player transcript exists");
        assert_eq!(caption_font.font_size, FontSize::Px(14.0));
        assert_eq!(caption_background.0, CENTERED_TEXT_BACKDROP);
        assert_eq!(caption_node.display, Display::None);

        let mut caption_layer_query = app
            .world_mut()
            .query_filtered::<&Node, With<PlayerTranscriptLayer>>();
        let caption_layer = caption_layer_query
            .iter(app.world())
            .next()
            .expect("player transcript layer exists");
        assert_eq!(caption_layer.bottom, px(66));
    }

    #[test]
    fn disconnect_clears_only_transient_actionable_ui() {
        let mut state = SmartActorHudState {
            inventory: "[1] copper coin".into(),
            offer_card: "Ilse offers a coin".into(),
            subtitle: "Ilse: Hello".into(),
            microphone_available: true,
            microphone_unavailable: false,
            listening: true,
            ..default()
        };
        state.show_player_transcript("Can anyone hear me?");
        state.toast("pending");
        state.clear_transients_on_disconnect("Python exited");

        assert_eq!(state.inventory, "[1] copper coin");
        assert!(state.offer_card.is_empty());
        assert!(state.subtitle.is_empty());
        assert!(state.player_transcript.is_none());
        assert!(!state.microphone_available);
        assert!(!state.microphone_unavailable);
        assert!(!state.listening);
        assert_eq!(state.connection, ConnectionUiState::Offline);
    }

    #[test]
    fn player_transcript_is_independent_from_overwrite_prone_toasts() {
        let mut state = SmartActorHudState::default();
        state.show_player_transcript("  Hello everyone  ");
        state.toast("Another event happened");

        assert_eq!(
            state
                .player_transcript
                .as_ref()
                .map(|message| message.text.as_str()),
            Some("You: Hello everyone")
        );
        assert_eq!(
            state
                .transient
                .as_ref()
                .map(|message| message.text.as_str()),
            Some("Another event happened")
        );

        state.show_player_transcript_delivery("Hello?", 0);
        assert_eq!(
            state.player_transcript_text(),
            Some("You: Hello?  ·  nobody nearby")
        );
    }

    #[test]
    fn voice_panel_tracks_each_backend_readiness_independently() {
        let mut state = SmartActorHudState::default();
        state.set_transcription_capabilities(true, true);
        assert_eq!(state.cloud_voice.state, VoiceModelUiState::Ready);
        assert_eq!(state.local_voice.state, VoiceModelUiState::NotLoaded);

        state.apply_transcription_status("local", "loading", Some("Downloading torch (502 MiB)"));
        assert_eq!(state.local_voice.state, VoiceModelUiState::Loading);
        assert_eq!(state.local_voice.detail, "Downloading torch (502 MiB)");
        assert_eq!(state.cloud_voice.state, VoiceModelUiState::Ready);

        state.apply_transcription_status("local", "idle", None);
        assert_eq!(state.local_voice.state, VoiceModelUiState::Ready);
    }

    #[test]
    fn voice_status_panel_has_requested_bottom_left_loader_shape() {
        let mut app = App::new();
        app.add_systems(Startup, spawn_smart_actor_hud);
        app.update();

        let mut panel_query = app
            .world_mut()
            .query_filtered::<&Node, With<VoiceStatusPanel>>();
        let panel = panel_query.single(app.world()).expect("voice panel exists");
        assert_eq!(panel.left, px(18));
        assert_eq!(panel.bottom, px(18));
        assert_eq!(panel.width, px(400));
        assert_eq!(panel.min_height, px(100));

        let mut loader_query = app
            .world_mut()
            .query_filtered::<&Node, With<VoiceLoaderFill>>();
        let loader = loader_query.single(app.world()).expect("loader exists");
        assert_eq!(loader.width, percent(28));
    }
}
