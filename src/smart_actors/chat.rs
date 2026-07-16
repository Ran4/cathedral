//! The typed-chat input box: speaking through the keyboard.
//!
//! Enter opens a one-line editor above the inventory bar; Enter sends the line
//! as a real `say` (Esc cancels). `/fart` emits the catalog fart instead — the
//! keyboard twin of the F key. The editor has a blinking cursor and the usual
//! readline motions: ←/→ and Ctrl+B/F, Home/End and Ctrl+A/E, Ctrl+K kills to
//! the end of the line, Ctrl+W deletes the word before the cursor, Ctrl+D
//! deletes the character under it (Delete's twin).
//!
//! The caret is an overlay node placed from the line's glyph layout, not a
//! `|` character spliced into the text — an inline glyph would take layout
//! space and shift everything after the cursor as it moves.
//!
//! While the box is open it owns the whole keyboard. Text arrives through the
//! raw [`KeyboardInput`] message stream (which carries layout-resolved
//! characters and key repeat), and afterwards `ButtonInput<KeyCode>` is reset
//! so no other binding — movement, F, V, Z, X, T, B, F5, the Esc menu — can
//! fire from a keystroke that was meant as text.

use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
    text::TextLayoutInfo,
    ui::ComputedNode,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::controller::PlayerController;
use crate::fonts::CathedralFonts;

use super::{
    PLAYER_SPEECH_MAX_CHARS, SmartActorRuntime, SmartActorsConfig,
    config_menu::ConfigMenuState,
    hud::{self, SmartActorHudState},
    interaction::{self, InteractionState, PlayerIntent, PlayerSpatialState},
};

/// Public so [`crate::drive`] can order its injected input before the box
/// consumes the frame's keyboard.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatInputSet;

const CURSOR_BLINK_PERIOD_SECONDS: f32 = 1.1;
const FONT_SIZE: f32 = 23.0;
const HINT_FONT_SIZE: f32 = 15.0;
/// Bevy's default line height is 1.2× the font size; the caret box follows it.
const LINE_HEIGHT: f32 = FONT_SIZE * 1.2;
/// What the editor shows before the text. Rendered as the root text section,
/// so glyph `section_index` 0 is the prompt and 1/2 are before/after the caret.
const PROMPT: &str = "» ";
/// Approximate EB Garamond space advance, for caret placement after trailing
/// spaces — the glyph layout carries no ink (and so no measure) for them.
const SPACE_ADVANCE_EM: f32 = 0.26;

/// The one-line editor. `cursor` is a char index into `buffer` (0..=chars).
#[derive(Resource, Debug, Default)]
pub struct ChatInputState {
    pub open: bool,
    buffer: String,
    cursor: usize,
    /// Tracked from the raw key stream because the box resets `ButtonInput`
    /// every frame it is open; seeded from it on open.
    ctrl_down: bool,
    /// Seconds since the last edit, driving the cursor blink; reset on every
    /// edit so the caret is solid while typing.
    blink: f32,
}

impl ChatInputState {
    fn open_empty(&mut self, ctrl_down: bool) {
        self.open = true;
        self.buffer.clear();
        self.cursor = 0;
        self.ctrl_down = ctrl_down;
        self.blink = 0.0;
    }

    fn close(&mut self) {
        self.open = false;
        self.buffer.clear();
        self.cursor = 0;
    }

    fn byte_cursor(&self) -> usize {
        self.buffer
            .char_indices()
            .nth(self.cursor)
            .map_or(self.buffer.len(), |(byte, _)| byte)
    }

    /// The text before and after the caret, for rendering.
    fn split(&self) -> (&str, &str) {
        self.buffer.split_at(self.byte_cursor())
    }

    fn insert(&mut self, text: &str) {
        for character in text.chars() {
            if character.is_control() {
                continue;
            }
            if self.buffer.chars().count() >= PLAYER_SPEECH_MAX_CHARS {
                break;
            }
            let at = self.byte_cursor();
            self.buffer.insert(at, character);
            self.cursor += 1;
        }
        self.blink = 0.0;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let at = self.byte_cursor();
        self.buffer.remove(at);
        self.blink = 0.0;
    }

    fn delete(&mut self) {
        let at = self.byte_cursor();
        if at < self.buffer.len() {
            self.buffer.remove(at);
        }
        self.blink = 0.0;
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.blink = 0.0;
    }

    fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.buffer.chars().count());
        self.blink = 0.0;
    }

    fn move_home(&mut self) {
        self.cursor = 0;
        self.blink = 0.0;
    }

    fn move_end(&mut self) {
        self.cursor = self.buffer.chars().count();
        self.blink = 0.0;
    }

    fn kill_to_end(&mut self) {
        let at = self.byte_cursor();
        self.buffer.truncate(at);
        self.blink = 0.0;
    }

    /// Readline's unix-word-rubout: delete back over any whitespace, then
    /// back over the word before it.
    fn delete_word_back(&mut self) {
        let end = self.byte_cursor();
        let word_end = self.buffer[..end].trim_end().len();
        let start = self.buffer[..word_end]
            .char_indices()
            .rev()
            .find(|(_, character)| character.is_whitespace())
            .map_or(0, |(index, character)| index + character.len_utf8());
        self.cursor -= self.buffer[start..end].chars().count();
        self.buffer.replace_range(start..end, "");
        self.blink = 0.0;
    }
}

#[derive(Component)]
pub(super) struct ChatInputPanel;
#[derive(Component)]
pub(super) struct ChatInputLine;
#[derive(Component)]
pub(super) struct ChatBeforeCursorSpan;
#[derive(Component)]
pub(super) struct ChatAfterCursorSpan;
#[derive(Component)]
pub(super) struct ChatCaret;

pub(super) fn spawn_chat_input(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
    let body_font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();
    let display_font = fonts
        .as_deref()
        .map(CathedralFonts::display)
        .unwrap_or_default();
    let line_font = TextFont {
        font: body_font.clone(),
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };

    commands
        .spawn((
            Name::new("Chat input centering layer"),
            Node {
                position_type: PositionType::Absolute,
                bottom: px(96),
                left: percent(20),
                width: percent(60),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ZIndex(30),
        ))
        .with_children(|layer| {
            layer
                .spawn((
                    Name::new("Chat input box"),
                    ChatInputPanel,
                    Node {
                        min_width: px(460),
                        max_width: percent(100),
                        padding: UiRect::axes(px(14), px(10)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        border_radius: BorderRadius::all(px(7)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(hud::PANEL),
                ))
                .with_children(|panel| {
                    // A padding-free wrapper, so the caret's absolute insets
                    // are the line's own glyph coordinates.
                    panel
                        .spawn((Name::new("Chat input line wrapper"), Node::default()))
                        .with_children(|wrapper| {
                            wrapper
                                .spawn((
                                    Name::new("Chat input line"),
                                    ChatInputLine,
                                    Text::new(PROMPT),
                                    line_font.clone(),
                                    TextColor(hud::MUTED),
                                ))
                                .with_children(|line| {
                                    line.spawn((
                                        ChatBeforeCursorSpan,
                                        TextSpan::new(""),
                                        line_font.clone(),
                                        TextColor(hud::TEXT),
                                    ));
                                    line.spawn((
                                        ChatAfterCursorSpan,
                                        TextSpan::new(""),
                                        line_font,
                                        TextColor(hud::TEXT),
                                    ));
                                });
                            wrapper.spawn((
                                Name::new("Chat caret"),
                                ChatCaret,
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(0),
                                    top: px(1),
                                    width: px(2),
                                    height: px(LINE_HEIGHT - 2.0),
                                    ..default()
                                },
                                BackgroundColor(hud::TEXT),
                            ));
                        });
                    panel.spawn((
                        Name::new("Chat input hint"),
                        Text::new("ENTER  SEND   ·   ESC  CANCEL   ·   /FART"),
                        TextFont {
                            font: display_font,
                            font_size: FontSize::Px(HINT_FONT_SIZE),
                            ..default()
                        },
                        TextColor(hud::MUTED),
                    ));
                });
        });
}

/// Everything the open box does with one frame of keys. Runs in `PreUpdate`
/// after Bevy's input collection (and after drive-mode injection), so the
/// final `ButtonInput` reset hides the keyboard from every later system.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_chat_input(
    mut chat: ResMut<ChatInputState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    menu: Res<ConfigMenuState>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    config: Res<SmartActorsConfig>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut interaction: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !chat.open {
        if !keyboard.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter])
            || menu.open
            || cursor
                .single()
                .map_or(true, |cursor| cursor.grab_mode == CursorGrabMode::None)
        {
            return;
        }
        if !runtime.interactions_enabled() {
            hud.toast("Chat is unavailable while the actor engine is offline");
            return;
        }
        chat.open_empty(keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]));
        // The Enter that opened the box must not also submit it: drop the raw
        // events already queued this frame, and eat the whole keyboard.
        keyboard_events.clear();
        keyboard.reset_all();
        return;
    }

    let mut submit = false;
    let mut cancel = false;
    let mut event_keys: Vec<KeyCode> = Vec::new();
    for event in keyboard_events.read() {
        if matches!(
            event.key_code,
            KeyCode::ControlLeft | KeyCode::ControlRight
        ) {
            chat.ctrl_down = event.state == ButtonState::Pressed;
            continue;
        }
        if event.state != ButtonState::Pressed {
            continue;
        }
        event_keys.push(event.key_code);
        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => submit = true,
            KeyCode::Escape => cancel = true,
            KeyCode::Backspace => chat.backspace(),
            KeyCode::Delete => chat.delete(),
            KeyCode::ArrowLeft => chat.move_left(),
            KeyCode::ArrowRight => chat.move_right(),
            KeyCode::Home => chat.move_home(),
            KeyCode::End => chat.move_end(),
            KeyCode::KeyA if chat.ctrl_down => chat.move_home(),
            KeyCode::KeyD if chat.ctrl_down => chat.delete(),
            KeyCode::KeyE if chat.ctrl_down => chat.move_end(),
            KeyCode::KeyB if chat.ctrl_down => chat.move_left(),
            KeyCode::KeyF if chat.ctrl_down => chat.move_right(),
            KeyCode::KeyK if chat.ctrl_down => chat.kill_to_end(),
            KeyCode::KeyW if chat.ctrl_down => chat.delete_word_back(),
            // Any other Ctrl chord is not text, whatever control character the
            // platform puts in `text`.
            _ if chat.ctrl_down => {}
            _ => {
                if let Some(text) = event.text.as_ref() {
                    chat.insert(text);
                }
            }
        }
    }
    // Drive scripts inject `ButtonInput` presses without raw keyboard events.
    // Only keys that produced no raw event count, so a real keypress (which
    // produces both) is never applied twice.
    let fallback =
        |key: KeyCode| keyboard.just_pressed(key) && !event_keys.contains(&key);
    submit = submit || fallback(KeyCode::Enter) || fallback(KeyCode::NumpadEnter);
    cancel = cancel || fallback(KeyCode::Escape);
    if fallback(KeyCode::Backspace) {
        chat.backspace();
    }
    if fallback(KeyCode::Delete) {
        chat.delete();
    }
    if fallback(KeyCode::ArrowLeft) {
        chat.move_left();
    }
    if fallback(KeyCode::ArrowRight) {
        chat.move_right();
    }
    if fallback(KeyCode::Home) {
        chat.move_home();
    }
    if fallback(KeyCode::End) {
        chat.move_end();
    }

    if cancel {
        chat.close();
    } else if submit {
        submit_chat_line(
            &mut chat,
            &config,
            &runtime,
            &players,
            &mut spatial,
            &mut interaction,
            &mut hud,
            &mut intents,
        );
    }
    keyboard.reset_all();
}

#[allow(clippy::too_many_arguments)]
fn submit_chat_line(
    chat: &mut ChatInputState,
    config: &SmartActorsConfig,
    runtime: &SmartActorRuntime,
    players: &Query<&GlobalTransform, With<PlayerController>>,
    spatial: &mut PlayerSpatialState,
    interaction: &mut InteractionState,
    hud: &mut SmartActorHudState,
    intents: &mut MessageWriter<PlayerIntent>,
) {
    let text = chat.buffer.trim().to_string();
    if text.is_empty() {
        chat.close();
        return;
    }
    if let Some(command) = text.strip_prefix('/') {
        match command.trim() {
            "fart" => {
                if config.sounds.enabled {
                    intents.write(PlayerIntent::Sound {
                        sound_id: "fart".into(),
                    });
                } else {
                    hud.toast("Sounds are disabled in config.ron");
                }
                chat.close();
            }
            // The box stays open with the text intact so a typo can be fixed.
            other => hud.toast(format!("Unknown command /{other} — try /fart")),
        }
        return;
    }
    let Ok(player) = players.single() else {
        chat.close();
        return;
    };
    match interaction::prepare_player_say(
        &text,
        player.translation(),
        runtime,
        spatial,
        interaction,
    ) {
        Some(intent) => {
            intents.write(intent);
            chat.close();
        }
        None => {
            hud.toast("You cannot speak right now: the actor engine is offline");
            chat.close();
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn update_chat_input_ui(
    time: Res<Time>,
    mut chat: ResMut<ChatInputState>,
    mut panels: Query<&mut Node, (With<ChatInputPanel>, Without<ChatCaret>)>,
    mut spans: Query<
        (&mut TextSpan, Option<&ChatBeforeCursorSpan>),
        Or<(With<ChatBeforeCursorSpan>, With<ChatAfterCursorSpan>)>,
    >,
    lines: Query<(&TextLayoutInfo, &ComputedNode), With<ChatInputLine>>,
    mut carets: Query<(&mut Node, &mut BackgroundColor), With<ChatCaret>>,
) {
    let Ok(mut panel) = panels.single_mut() else {
        return;
    };
    let desired = if chat.open {
        Display::Flex
    } else {
        Display::None
    };
    if panel.display != desired {
        panel.display = desired;
    }
    if !chat.open {
        return;
    }
    chat.blink = (chat.blink + time.delta_secs()) % CURSOR_BLINK_PERIOD_SECONDS;
    let caret_visible = chat.blink < CURSOR_BLINK_PERIOD_SECONDS * 0.6;
    let (before, after) = chat.split();
    let (before, after) = (before.to_string(), after.to_string());
    for (mut span, is_before) in &mut spans {
        let value = if is_before.is_some() { &before } else { &after };
        if span.0 != *value {
            span.0 = value.clone();
        }
    }
    let Ok((mut caret, mut color)) = carets.single_mut() else {
        return;
    };
    color.0 = if caret_visible { hud::TEXT } else { Color::NONE };
    if let Ok((layout, computed)) = lines.single()
        && let Some((x, line_index)) = caret_offset(layout, computed, &before)
    {
        caret.left = px(x - 1.0);
        caret.top = px(line_index as f32 * LINE_HEIGHT + 1.0);
    }
}

/// Where the caret sits in the line's logical-pixel space: the left ink edge
/// of the first glyph after the cursor, or past the right ink edge of the last
/// glyph before it. Glyph layout lags this frame's span writes by one frame —
/// invisible at frame rate. Trailing spaces carry no ink, so their advance is
/// approximated; the moment a character follows, the real layout takes over.
fn caret_offset(
    layout: &TextLayoutInfo,
    computed: &ComputedNode,
    before: &str,
) -> Option<(f32, usize)> {
    let logical = computed.inverse_scale_factor();
    if let Some(glyph) = layout
        .glyphs
        .iter()
        .find(|glyph| glyph.section_index == 2)
    {
        let x = glyph.position.x - glyph.atlas_info.rect.size().x / 2.0;
        return Some((x * logical, glyph.line_index));
    }
    // Anchor on the last glyph with ink: whitespace has a zero-size rect (or
    // no glyph at all), so its advance is added from the text instead.
    let glyph = layout
        .glyphs
        .iter()
        .rfind(|glyph| glyph.section_index <= 1 && glyph.atlas_info.rect.size().x > 0.5)?;
    let x = (glyph.position.x + glyph.atlas_info.rect.size().x / 2.0) * logical;
    let trailing_spaces = PROMPT
        .chars()
        .chain(before.chars())
        .rev()
        .take_while(|character| character.is_whitespace())
        .count();
    Some((
        x + trailing_spaces as f32 * FONT_SIZE * SPACE_ADVANCE_EM,
        glyph.line_index,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(buffer: &str, cursor: usize) -> ChatInputState {
        let mut state = ChatInputState::default();
        state.open_empty(false);
        state.insert(buffer);
        state.cursor = cursor;
        state
    }

    #[test]
    fn insert_types_at_the_cursor_and_filters_control_characters() {
        let mut state = state_with("hllo", 1);
        state.insert("e");
        assert_eq!(state.buffer, "hello");
        assert_eq!(state.cursor, 2);

        state.insert("\r\n\t");
        assert_eq!(state.buffer, "hello");
    }

    #[test]
    fn the_buffer_caps_at_the_engine_say_limit() {
        let mut state = state_with("", 0);
        state.insert(&"x".repeat(PLAYER_SPEECH_MAX_CHARS + 40));
        assert_eq!(state.buffer.chars().count(), PLAYER_SPEECH_MAX_CHARS);
    }

    #[test]
    fn backspace_and_delete_work_at_the_boundaries() {
        let mut state = state_with("ab", 0);
        state.backspace();
        assert_eq!(state.buffer, "ab");

        state.delete();
        assert_eq!(state.buffer, "b");

        state.move_end();
        state.delete();
        assert_eq!(state.buffer, "b");
        state.backspace();
        assert_eq!(state.buffer, "");
    }

    #[test]
    fn readline_motions_move_by_characters_not_bytes() {
        // Multi-byte characters exercise the char/byte cursor split.
        let mut state = state_with("héllo", 5);
        state.move_left();
        state.move_left();
        assert_eq!(state.split(), ("hél", "lo"));

        state.move_home();
        assert_eq!(state.split(), ("", "héllo"));
        state.move_right();
        state.move_right();
        state.kill_to_end();
        assert_eq!(state.buffer, "hé");

        state.move_end();
        assert_eq!(state.split(), ("hé", ""));
    }

    #[test]
    fn ctrl_w_deletes_the_word_before_the_cursor() {
        // Trailing whitespace is eaten together with the word before it.
        let mut state = state_with("say hello there  ", 17);
        state.delete_word_back();
        assert_eq!(state.buffer, "say hello ");
        state.delete_word_back();
        assert_eq!(state.buffer, "say ");

        // Mid-word ("say hél|lo there"): only the part left of the cursor goes.
        let mut state = state_with("say héllo there", 7);
        state.delete_word_back();
        assert_eq!(state.buffer, "say lo there");
        assert_eq!(state.split(), ("say ", "lo there"));

        // At the start of the only word, and on an empty line: no-ops.
        let mut state = state_with("word", 4);
        state.delete_word_back();
        assert_eq!(state.buffer, "");
        state.delete_word_back();
        assert_eq!(state.buffer, "");
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn closing_clears_the_line() {
        let mut state = state_with("draft", 5);
        state.close();
        assert!(!state.open);
        assert_eq!(state.buffer, "");
        assert_eq!(state.cursor, 0);
    }
}
