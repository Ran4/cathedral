//! Smart-actor status, inventory, and offer-card HUD.

use std::time::Duration;

use bevy::prelude::*;

use crate::fonts::CathedralFonts;

use super::bridge;

pub(super) const PANEL: Color = Color::srgba(0.025, 0.03, 0.045, 0.88);
const CENTERED_TEXT_BACKDROP: Color = Color::srgba(0.10, 0.105, 0.11, 0.80);
pub(super) const TEXT: Color = Color::srgb(0.96, 0.94, 0.86);
pub(super) const MUTED: Color = Color::srgb(0.70, 0.72, 0.74);
pub(super) const ONLINE: Color = Color::srgb(0.58, 0.88, 0.62);
pub(super) const DEGRADED: Color = Color::srgb(0.97, 0.74, 0.31);
pub(super) const OFFLINE: Color = Color::srgb(0.96, 0.40, 0.36);
pub(super) const LOADER_TRACK: Color = Color::srgba(0.24, 0.25, 0.27, 0.92);
const PLAYER_TRANSCRIPT_LIFETIME: Duration = Duration::from_secs(8);
/// Longer than a toast's 4 s: a refused offer is a thing that happened *to*
/// the player, and the 4 s slot is overwrite-prone by design.
const OFFER_OUTCOME_LIFETIME: Duration = Duration::from_secs(8);
const CONNECTION_FONT_SIZE: f32 = 21.0;
const CONNECTION_DETAIL_FONT_SIZE: f32 = 15.75;
const VOICE_PANEL_FONT_SIZE: f32 = 15.6;
const VOICE_PANEL_CONTROLS_FONT_SIZE: f32 = 11.7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VoiceModelUiState {
    Checking,
    NotLoaded,
    Loading,
    Transcribing,
    Ready,
    Failed,
    Unavailable,
}

impl VoiceModelUiState {
    pub(super) fn label(self) -> &'static str {
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

    pub(super) fn color(self) -> Color {
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
    /// The offer card's counterpart: why an offer the player was part of ended
    /// with nothing changing hands. Its own slot, on its own clock, so a toast
    /// cannot swallow it.
    offer_outcome: Option<TimedMessage>,
    /// The law's standing line — no clock on it at all, because it is true
    /// until the sim says otherwise (`law_and_order.md` M4).
    law_standing: String,
    pub focus_hint: String,
    pub subtitle: String,
    pub microphone_available: bool,
    pub microphone_unavailable: bool,
    /// Persistent user preference controlled by V, distinct from the temporary
    /// worker pauses an offline engine causes.
    pub microphone_enabled: bool,
    pub listening: bool,
    pub transcription_backend: String,
    pub npc_voice_backend: String,
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
            offer_outcome: None,
            law_standing: String::new(),
            focus_hint: String::new(),
            subtitle: String::new(),
            microphone_available: false,
            microphone_unavailable: false,
            microphone_enabled: true,
            listening: false,
            transcription_backend: "CLOUD".into(),
            npc_voice_backend: "CHECKING".into(),
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

    /// An offer of the player's that ended with nothing changing hands, and
    /// why: `headline` names the outcome, `reason` says what caused it. Both
    /// are shown, on two lines — "OFFER DECLINED" alone leaves the player
    /// guessing whether they were refused or simply walked too far.
    pub fn show_offer_outcome(&mut self, headline: &str, reason: &str) {
        if headline.is_empty() {
            return;
        }
        self.offer_outcome = Some(TimedMessage {
            text: format!("{headline}\n{reason}"),
            remaining: OFFER_OUTCOME_LIFETIME,
        });
    }

    /// Where the player stands with the law, in the words the sim resolved.
    /// Empty clears the line — which is the overwhelmingly common case, and the
    /// one where the panel must not be on screen at all.
    pub fn set_law_standing(&mut self, text: String) {
        self.law_standing = text;
    }

    #[cfg(test)]
    pub(super) fn law_standing_text(&self) -> &str {
        &self.law_standing
    }

    #[cfg(test)]
    pub(super) fn offer_outcome_text(&self) -> Option<&str> {
        self.offer_outcome
            .as_ref()
            .map(|message| message.text.as_str())
    }

    /// The transient toast still on screen, if any. The inventory overlay sits
    /// *above* the HUD, so it mirrors this into its own feedback line rather
    /// than letting the engine's refusal ("your mouth is full") hide behind it.
    pub(super) fn transient_text(&self) -> Option<&str> {
        self.transient.as_ref().map(|message| message.text.as_str())
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
        self.offer_outcome = None;
        self.focus_hint.clear();
        self.subtitle.clear();
        self.player_transcript = None;
        self.microphone_available = false;
        self.microphone_unavailable = false;
        self.listening = false;
        self.cloud_voice = VoiceBackendUi::checking();
        self.local_voice = VoiceBackendUi::checking();
        self.npc_voice_backend = "OFF".into();
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

    pub fn set_npc_voice_backend(&mut self, backend: bridge::TtsBackend) {
        self.npc_voice_backend = match backend {
            bridge::TtsBackend::Cloud => "OPENAI".into(),
            bridge::TtsBackend::Local => "POCKET TTS".into(),
            bridge::TtsBackend::Off => "OFF".into(),
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

    /// Per-backend transcription readiness for the settings menu.
    pub(super) fn transcription_status(
        &self,
        backend: bridge::TranscriptionBackend,
    ) -> (VoiceModelUiState, &str) {
        let voice = match backend {
            bridge::TranscriptionBackend::Cloud => &self.cloud_voice,
            bridge::TranscriptionBackend::Local => &self.local_voice,
        };
        (voice.state, voice.detail.as_str())
    }
}

#[derive(Component)]
pub(super) struct SmartActorStatusPanel;
#[derive(Component)]
pub(super) struct ConnectionText;
#[derive(Component)]
pub(super) struct ConnectionDetailText;
#[derive(Component)]
pub(super) struct InventoryText;
#[derive(Component)]
pub(super) struct OfferCardText;
#[derive(Component)]
pub(super) struct OfferOutcomeText;
/// The player's standing with the law (`law_and_order.md` M4). A **standing**
/// line rather than a toast: it must always be on screen while it is true, and
/// it must always name what would clear it — a brand with a visible door is a
/// story, a brand with no door is a bug.
#[derive(Component)]
pub(crate) struct LawStandingText;
#[derive(Component)]
pub(super) struct FocusHintText;
#[derive(Component)]
pub(super) struct PlayerTranscriptLayer;
#[derive(Component)]
pub(super) struct PlayerTranscriptText;
#[derive(Component)]
pub(super) struct VoicePanelText;
#[derive(Component)]
pub(super) struct VoicePanelControlsText;
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
            SmartActorStatusPanel,
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
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                ConnectionText,
                Text::new("ACTORS STARTING"),
                TextFont {
                    font: display_font.clone(),
                    font_size: FontSize::Px(CONNECTION_FONT_SIZE),
                    ..default()
                },
                TextColor(DEGRADED),
            ));
            panel.spawn((
                ConnectionDetailText,
                Text::new("Launching local service…"),
                TextFont {
                    font: body_font.clone(),
                    font_size: FontSize::Px(CONNECTION_DETAIL_FONT_SIZE),
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

    // Directly under the offer card, whose place it takes once the offer is
    // gone: the same part of the screen the player was already reading, and
    // clear of the focus hint above and the toast below.
    spawn_centered_text(
        &mut commands,
        "Offer outcome notice",
        OfferOutcomeText,
        65.0,
        17.0,
        OFFLINE,
        body_font.clone(),
    );
    // Top-left of the lower band, above the focus hint: the standing line is
    // read at leisure, not glanced at, and it must never fight the offer card
    // for the same pixels.
    spawn_centered_text(
        &mut commands,
        "Law standing line",
        LawStandingText,
        20.0,
        16.0,
        OFFLINE,
        body_font.clone(),
    );
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
                Text::new(
                    "VOICE INPUT\nMODEL   CLOUD\nMIC     CHECKING\nNPC VOICES  CHECKING\nSTATUS  CHECKING",
                ),
                TextFont {
                    font: body_font,
                    font_size: FontSize::Px(VOICE_PANEL_FONT_SIZE),
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
                VoicePanelControlsText,
                Text::new("[V] MICROPHONE   [Z] TRANSCRIPTION   [X] NPC VOICES   [I] POCKETS"),
                TextFont {
                    font: title_font,
                    font_size: FontSize::Px(VOICE_PANEL_CONTROLS_FONT_SIZE),
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
            Option<&OfferOutcomeText>,
            Option<&LawStandingText>,
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
            With<OfferOutcomeText>,
            With<LawStandingText>,
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
    if let Some(message) = state.offer_outcome.as_mut() {
        message.remaining = message.remaining.saturating_sub(time.delta());
        if message.remaining.is_zero() {
            state.offer_outcome = None;
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
        "VOICE INPUT\nMODEL   {}\nMIC     {microphone}\nNPC VOICES  {}\nSTATUS  {} — {selected_detail}",
        state.transcription_backend,
        state.npc_voice_backend,
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
        let (left, width, next_color) = match selected_state {
            VoiceModelUiState::Checking
            | VoiceModelUiState::Loading
            | VoiceModelUiState::Transcribing => (percent(travel * 72.0), percent(28), DEGRADED),
            VoiceModelUiState::Ready => (percent(0), percent(100), ONLINE),
            VoiceModelUiState::NotLoaded => (percent(0), percent(18), MUTED),
            VoiceModelUiState::Failed | VoiceModelUiState::Unavailable => {
                (percent(0), percent(100), OFFLINE)
            }
        };
        // Steady states repeat the same values; writing them anyway would
        // dirty UI layout every frame.
        if node.left != left {
            node.left = left;
        }
        if node.width != width {
            node.width = width;
        }
        if color.0 != next_color {
            color.0 = next_color;
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
    let offer_outcome_text = state
        .offer_outcome
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
        offer_outcome,
        law_standing,
        hint,
        player_transcript,
        voice_panel,
        toast,
    ) in &mut views
    {
        if connection.is_some() {
            let label = state.connection.label();
            if text.0 != label {
                text.0 = label.into();
            }
            if let Some(color) = color.as_mut() {
                let next = state.connection.color();
                if color.0 != next {
                    color.0 = next;
                }
            }
        } else if detail.is_some() {
            if text.0 != state.connection_detail {
                text.0 = state.connection_detail.clone();
            }
        } else if inventory.is_some() {
            set_optional_text(&state.inventory, &mut text, node.as_mut());
        } else if offer.is_some() {
            set_optional_text(&state.offer_card, &mut text, node.as_mut());
        } else if offer_outcome.is_some() {
            set_optional_text(offer_outcome_text, &mut text, node.as_mut());
        } else if law_standing.is_some() {
            set_optional_text(&state.law_standing, &mut text, node.as_mut());
        } else if hint.is_some() {
            set_optional_text(&state.focus_hint, &mut text, node.as_mut());
        } else if player_transcript.is_some() {
            set_optional_text(player_transcript_text, &mut text, node.as_mut());
        } else if voice_panel.is_some() {
            if let Some(color) = color.as_mut() {
                let next = selected_state.color();
                if color.0 != next {
                    color.0 = next;
                }
            }
            set_optional_text(&voice_panel_text, &mut text, node.as_mut());
        } else if toast.is_some() {
            set_optional_text(toast_text, &mut text, node.as_mut());
        }
    }
}

/// Takes the `Mut` wrappers themselves: coercing to `&mut Text`/`&mut Node`
/// at the call site would already flag both components changed every frame,
/// making the compares below pointless.
fn set_optional_text(value: &str, text: &mut Mut<Text>, node: Option<&mut Mut<Node>>) {
    if text.0 != value {
        text.0 = value.into();
    }
    if let Some(node) = node {
        let display = if value.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_status_is_hidden_by_default_and_uses_larger_text() {
        let mut app = App::new();
        app.add_systems(Startup, spawn_smart_actor_hud);
        app.update();

        let world = app.world_mut();
        let visibility = world
            .query_filtered::<&Visibility, With<SmartActorStatusPanel>>()
            .single(world)
            .expect("actor status panel exists");
        assert_eq!(*visibility, Visibility::Hidden);

        let connection_font = world
            .query_filtered::<&TextFont, With<ConnectionText>>()
            .single(world)
            .expect("connection text exists");
        assert_eq!(
            connection_font.font_size,
            FontSize::Px(CONNECTION_FONT_SIZE)
        );

        let detail_font = world
            .query_filtered::<&TextFont, With<ConnectionDetailText>>()
            .single(world)
            .expect("connection detail text exists");
        assert_eq!(
            detail_font.font_size,
            FontSize::Px(CONNECTION_DETAIL_FONT_SIZE)
        );
    }

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
    fn a_rejected_offer_notice_survives_the_toast_that_would_have_hidden_it() {
        let mut state = SmartActorHudState::default();
        state.show_offer_outcome("OFFER LAPSED", "You and Ilse drifted more than 20 m apart");
        // The 4 s slot is overwritten constantly; the reason must not be.
        state.toast("Ilse eats the loaf of bread");

        assert_eq!(
            state.offer_outcome_text(),
            Some("OFFER LAPSED\nYou and Ilse drifted more than 20 m apart")
        );
        assert_eq!(state.transient_text(), Some("Ilse eats the loaf of bread"));
        assert!(OFFER_OUTCOME_LIFETIME > Duration::from_secs(4));
    }

    #[test]
    fn the_offer_outcome_notice_spawns_hidden_and_clear_of_its_neighbours() {
        let mut app = App::new();
        app.add_systems(Startup, spawn_smart_actor_hud);
        app.update();

        let world = app.world_mut();
        let (notice_font, notice_color, notice_node) = world
            .query_filtered::<(&TextFont, &TextColor, &Node), With<OfferOutcomeText>>()
            .single(world)
            .expect("offer outcome notice exists");
        assert_eq!(notice_font.font_size, FontSize::Px(17.0));
        assert_eq!(notice_color.0, OFFLINE);
        assert_eq!(notice_node.display, Display::None);

        // Between the offer card it replaces (58%) and the toast (73%), so a
        // rejection never lands on top of either.
        let layer_top = world
            .query::<(&Name, &Node)>()
            .iter(world)
            .find(|(name, _)| name.as_str() == "Offer outcome notice centering layer")
            .map(|(_, node)| node.top)
            .expect("the notice has a centering layer");
        assert_eq!(layer_top, percent(65));
        let card_top = world
            .query_filtered::<&Node, With<OfferCardText>>()
            .single(world)
            .expect("offer card exists")
            .top;
        assert_eq!(card_top, percent(58));
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
        state.clear_transients_on_disconnect("The actor engine exited");

        assert_eq!(state.inventory, "[1] copper coin");
        assert!(state.offer_card.is_empty());
        assert!(state.offer_outcome_text().is_none());
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

        let mut status_font_query = app
            .world_mut()
            .query_filtered::<&TextFont, With<VoicePanelText>>();
        let status_font = status_font_query
            .single(app.world())
            .expect("voice status text exists");
        assert_eq!(status_font.font_size, FontSize::Px(15.6));

        let mut controls_font_query = app
            .world_mut()
            .query_filtered::<&TextFont, With<VoicePanelControlsText>>();
        let controls_font = controls_font_query
            .single(app.world())
            .expect("voice controls text exists");
        assert_eq!(controls_font.font_size, FontSize::Px(11.7));

        let mut loader_query = app
            .world_mut()
            .query_filtered::<&Node, With<VoiceLoaderFill>>();
        let loader = loader_query.single(app.world()).expect("loader exists");
        assert_eq!(loader.width, percent(28));
    }
}
