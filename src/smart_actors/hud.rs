//! Smart-actor status, inventory, subtitle, and offer-card HUD.

use std::time::Duration;

use bevy::prelude::*;

const PANEL: Color = Color::srgba(0.025, 0.03, 0.045, 0.88);
const CENTERED_TEXT_BACKDROP: Color = Color::srgba(0.10, 0.105, 0.11, 0.80);
const TEXT: Color = Color::srgb(0.96, 0.94, 0.86);
const MUTED: Color = Color::srgb(0.70, 0.72, 0.74);
const ONLINE: Color = Color::srgb(0.58, 0.88, 0.62);
const DEGRADED: Color = Color::srgb(0.97, 0.74, 0.31);
const OFFLINE: Color = Color::srgb(0.96, 0.40, 0.36);

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
    pub listening: bool,
    transient: Option<TimedMessage>,
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
            listening: false,
            transient: None,
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

    pub fn clear_transients_on_disconnect(&mut self, detail: impl Into<String>) {
        self.connection = ConnectionUiState::Offline;
        self.connection_detail = detail.into();
        self.offer_card.clear();
        self.focus_hint.clear();
        self.subtitle.clear();
        self.microphone_available = false;
        self.microphone_unavailable = false;
        self.listening = false;
        self.transient = None;
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
pub(super) struct SubtitleText;
#[derive(Component)]
pub(super) struct ListeningText;
#[derive(Component)]
pub(super) struct ToastText;

pub fn spawn_smart_actor_hud(mut commands: Commands) {
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
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(DEGRADED),
            ));
            panel.spawn((
                ConnectionDetailText,
                Text::new("Launching local service…"),
                TextFont {
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
    );
    spawn_centered_text(
        &mut commands,
        "Smart actor subtitle",
        SubtitleText,
        82.0,
        20.0,
        TEXT,
    );
    spawn_centered_text(
        &mut commands,
        "Microphone status",
        ListeningText,
        14.0,
        22.0,
        OFFLINE,
    );
    spawn_centered_text(
        &mut commands,
        "Smart actor toast",
        ToastText,
        73.0,
        14.0,
        DEGRADED,
    );
}

fn spawn_centered_text<M: Component>(
    commands: &mut Commands,
    name: &'static str,
    marker: M,
    top_percent: f32,
    font_size: f32,
    color: Color,
) {
    // Keep the centering layer transparent and let its child size to the text.
    // This gives subtitles and focus hints a compact backing instead of a wide
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
            Option<&SubtitleText>,
            Option<&ListeningText>,
            Option<&ToastText>,
        ),
        Or<(
            With<ConnectionText>,
            With<ConnectionDetailText>,
            With<InventoryText>,
            With<OfferCardText>,
            With<FocusHintText>,
            With<SubtitleText>,
            With<ListeningText>,
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

    let listening_text = microphone_label(
        state.microphone_available,
        state.microphone_unavailable,
        state.listening,
    );
    let toast_text = state
        .transient
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
        subtitle,
        listening,
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
        } else if subtitle.is_some() {
            set_optional_text(&state.subtitle, &mut text, node.as_deref_mut());
        } else if listening.is_some() {
            if let Some(color) = color.as_deref_mut() {
                color.0 = if state.microphone_unavailable {
                    DEGRADED
                } else if state.listening {
                    ONLINE
                } else {
                    MUTED
                };
            }
            set_optional_text(listening_text, &mut text, node.as_deref_mut());
        } else if toast.is_some() {
            set_optional_text(toast_text, &mut text, node.as_deref_mut());
        }
    }
}

fn microphone_label(available: bool, unavailable: bool, enabled: bool) -> &'static str {
    match (available, unavailable, enabled) {
        (_, true, _) => "MIC UNAVAILABLE",
        (true, false, true) => "●  MIC ON — OPEN SPEECH    [V] OFF",
        (true, false, false) => "MIC OFF    [V] ON",
        (false, false, _) => "",
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

        let mut subtitle_query = app
            .world_mut()
            .query_filtered::<&BackgroundColor, With<SubtitleText>>();
        let subtitle_background = subtitle_query
            .iter(app.world())
            .next()
            .expect("subtitle exists");
        assert_eq!(subtitle_background.0, CENTERED_TEXT_BACKDROP);
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
        state.toast("pending");
        state.clear_transients_on_disconnect("Python exited");

        assert_eq!(state.inventory, "[1] copper coin");
        assert!(state.offer_card.is_empty());
        assert!(state.subtitle.is_empty());
        assert!(!state.microphone_available);
        assert!(!state.microphone_unavailable);
        assert!(!state.listening);
        assert_eq!(state.connection, ConnectionUiState::Offline);
    }

    #[test]
    fn microphone_status_stays_visible_when_the_user_toggles_it_off() {
        assert_eq!(
            microphone_label(true, false, true),
            "●  MIC ON — OPEN SPEECH    [V] OFF"
        );
        assert_eq!(microphone_label(true, false, false), "MIC OFF    [V] ON");
        assert_eq!(microphone_label(false, true, false), "MIC UNAVAILABLE");
        assert_eq!(microphone_label(false, false, true), "");
    }
}
