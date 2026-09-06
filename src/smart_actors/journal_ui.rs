//! The `J` journal: the player's receipts and the standing words supplied by
//! the sim. Like the inventory, it owns the pointer while the panel is open.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::fonts::CathedralFonts;
use crate::map::MapState;

use super::chat::ChatInputState;
use super::config_menu::ConfigMenuState;
use super::hud::{self, SmartActorHudState};
use super::inventory_ui::InventoryUiState;

const SCRIM: Color = Color::srgba(0.01, 0.015, 0.03, 0.55);
const PANEL_SOLID: Color = Color::srgba(0.025, 0.03, 0.045, 0.96);
const BUTTON_BG: Color = Color::srgba(0.10, 0.11, 0.15, 1.0);
const BUTTON_HOVER: Color = Color::srgba(0.19, 0.21, 0.26, 1.0);

/// The player's receipts, projected by one `process_engine_message` arm and
/// assigned only when they differ. No objectives: future quest-supplied
/// `standing` text must be a question or a clock naming its own door, never an
/// instruction. This panel renders only what the player heard or caused.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub(super) struct PlayerJournal {
    pub entries: Vec<JournalRow>,
    pub standing: Vec<String>,
}

/// Owned, already-resolved prose; the UI never reaches back into the sim.
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct JournalRow {
    pub attribution: String,
    pub word: String,
}

#[derive(Resource, Debug, Default)]
pub struct JournalUiState {
    pub open: bool,
}

#[derive(Component)]
pub(super) struct JournalUiRoot;
#[derive(Component)]
pub(super) struct JournalEntriesRoot;
#[derive(Component)]
pub(super) struct JournalCloseButton;

/// The sim has already resolved every name through the player. The host only
/// joins those parts and expresses distance as a register the player can read.
pub(super) fn row_from_entry(entry: cathedral_sim::JournalEntry) -> JournalRow {
    let mut parts = Vec::new();
    if let Some(from) = entry.from {
        parts.push(from);
    }
    if let Some(place) = entry.place {
        let lower = place.to_lowercase();
        if [
            "in ", "inside ", "at ", "next to ", "near ", "on ", "beside ", "outside ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        {
            let mut letters = place.chars();
            parts.push(letters.next().map_or_else(String::new, |first| {
                first.to_lowercase().collect::<String>() + letters.as_str()
            }));
        } else {
            parts.push(format!("at {place}"));
        }
    }
    parts.push(entry.when);
    let register = match entry.hops {
        0 => "you saw it yourself",
        1 => "at one remove",
        2 => "at second hand",
        _ => "at third hand or worse",
    };
    let mut attribution = format!("{} — {register}", parts.join(", "));
    if entry.tellings > 1 {
        attribution.push_str(&format!(
            " — and {} {} since, in {} {}",
            entry.tellings - 1,
            if entry.tellings == 2 {
                "other"
            } else {
                "others"
            },
            entry.wards,
            if entry.wards == 1 { "ward" } else { "wards" },
        ));
    }
    attribution.push(':');
    JournalRow {
        attribution,
        word: format!("“{}”", entry.word),
    }
}

fn build_rows(journal: &PlayerJournal) -> Vec<JournalRow> {
    journal.entries.clone()
}

pub(super) fn spawn_journal_ui(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
    let display_font = fonts
        .as_deref()
        .map(CathedralFonts::display)
        .unwrap_or_default();
    commands
        .spawn((
            Name::new("Journal overlay"),
            JournalUiRoot,
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
            ZIndex(35),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Name::new("Journal panel"),
                    Node {
                        width: px(760),
                        max_width: percent(94),
                        max_height: percent(88),
                        padding: UiRect::axes(px(26), px(22)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(18),
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BackgroundColor(PANEL_SOLID),
                ))
                .with_children(|panel| {
                    panel
                        .spawn(Node {
                            width: percent(100),
                            flex_shrink: 0.0,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|header| {
                            spawn_text(
                                header,
                                "WORDS YOU HAVE HEARD",
                                &display_font,
                                19.0,
                                hud::TEXT,
                            );
                            header
                                .spawn((
                                    Name::new("Journal close"),
                                    Button,
                                    JournalCloseButton,
                                    Node {
                                        padding: UiRect::axes(px(14), px(6)),
                                        border_radius: BorderRadius::all(px(999)),
                                        ..default()
                                    },
                                    BackgroundColor(BUTTON_BG),
                                ))
                                .with_children(|button| {
                                    spawn_text(
                                        button,
                                        "Close  [J]",
                                        &display_font,
                                        13.5,
                                        hud::MUTED,
                                    );
                                });
                        });
                    panel.spawn((
                        Name::new("Journal entries"),
                        JournalEntriesRoot,
                        Node {
                            width: percent(100),
                            min_height: px(0),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(18),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollPosition::default(),
                    ));
                });
        });
}

fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    font: &FontSource,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font: font.clone(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        Node {
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

/// Settings takes over the pointer; the other panels refuse to stack. The
/// close button follows the very same cursor path as the key, even offline.
#[allow(clippy::too_many_arguments)]
pub(super) fn toggle_journal(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<ConfigMenuState>>,
    chat: Option<Res<ChatInputState>>,
    map: Option<Res<MapState>>,
    inventory: Option<Res<InventoryUiState>>,
    close: Query<&Interaction, (Changed<Interaction>, With<JournalCloseButton>)>,
    mut journal: ResMut<JournalUiState>,
    cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
) {
    let menu_open = menu.is_some_and(|menu| menu.open);
    let chat_open = chat.is_some_and(|chat| chat.open);
    let map_open = map.is_some_and(|map| map.fullscreen_open);
    let inventory_open = inventory.is_some_and(|inventory| inventory.open);
    if menu_open && journal.open {
        journal.open = false;
        return;
    }
    let close_pressed = journal.open
        && close
            .iter()
            .any(|interaction| *interaction == Interaction::Pressed);
    if (!keyboard.just_pressed(KeyCode::KeyJ) && !close_pressed) || menu_open || chat_open {
        return;
    }
    if !journal.open && (map_open || inventory_open) {
        return;
    }
    journal.open = !journal.open;
    let Some(mut cursor) = cursor else { return };
    if journal.open {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    } else if !map_open && !inventory_open {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

pub(super) fn update_journal_ui(
    journal: Res<JournalUiState>,
    mut roots: Query<&mut Node, With<JournalUiRoot>>,
    mut buttons: Query<(&Interaction, &mut BackgroundColor), With<JournalCloseButton>>,
) {
    let desired = if journal.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut root in &mut roots {
        if root.display != desired {
            root.display = desired;
        }
    }
    for (interaction, mut background) in &mut buttons {
        let desired = if *interaction == Interaction::None {
            BUTTON_BG
        } else {
            BUTTON_HOVER
        };
        if background.0 != desired {
            background.0 = desired;
        }
    }
}

/// Rebuild only when the visible projection changes, retaining the scroll
/// position while the reader watches another telling arrive.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_journal_ui(
    mut commands: Commands,
    state: Res<JournalUiState>,
    journal: Res<PlayerJournal>,
    fonts: Option<Res<CathedralFonts>>,
    mut roots: Query<(Entity, Option<&Children>, &mut ScrollPosition), With<JournalEntriesRoot>>,
    mut cached: Local<Option<(Vec<JournalRow>, Vec<String>)>>,
) {
    let Ok((entity, children, mut scroll)) = roots.single_mut() else {
        return;
    };
    let next = state
        .open
        .then(|| (build_rows(&journal), journal.standing.clone()));
    if *cached == next {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    if let Some((rows, standing)) = &next {
        let font = fonts
            .as_deref()
            .map(CathedralFonts::body)
            .unwrap_or_default();
        commands.entity(entity).with_children(|root| {
            for line in standing {
                spawn_text(root, line, &font, 16.0, hud::OFFLINE);
            }
            if rows.is_empty() {
                spawn_text(root, "No words set down yet.", &font, 16.0, hud::MUTED);
            }
            for row in rows {
                root.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: px(6),
                    ..default()
                })
                .with_children(|entry| {
                    spawn_text(entry, &row.attribution, &font, 14.0, hud::MUTED);
                    spawn_text(entry, &row.word, &font, 17.0, hud::TEXT);
                });
            }
        });
    } else if scroll.0 != Vec2::ZERO {
        scroll.0 = Vec2::ZERO;
    }
    *cached = next;
}

pub(super) fn scroll_journal(
    journal: Res<JournalUiState>,
    wheel: Option<Res<AccumulatedMouseScroll>>,
    mut roots: Query<(&ComputedNode, &mut ScrollPosition), With<JournalEntriesRoot>>,
) {
    if !journal.open {
        return;
    }
    let Some(wheel) = wheel else { return };
    if wheel.delta.y == 0.0 {
        return;
    }
    let step = match wheel.unit {
        MouseScrollUnit::Line => 36.0,
        MouseScrollUnit::Pixel => 1.0,
    };
    for (node, mut position) in &mut roots {
        let max = ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0);
        let y = (position.y - wheel.delta.y * step).clamp(0.0, max);
        if position.y != y {
            position.y = y;
        }
    }
}

/// Exactly one writer, after the bridge and before the HUD presents it.
pub(super) fn journal_standing_hud(
    journal: Res<PlayerJournal>,
    mut hud: ResMut<SmartActorHudState>,
) {
    if journal.is_changed() {
        hud.set_journal_standing(journal.standing.join("\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singular_receipt_counts_and_locative_places_read_naturally() {
        for (place, expected) in [
            ("Inside the Lanthorn", "inside the Lanthorn"),
            ("In a street", "in a street"),
            ("Next to a well", "next to a well"),
            ("At the gate", "at the gate"),
            ("the board", "at the board"),
        ] {
            let row = row_from_entry(cathedral_sim::JournalEntry {
                word: "A bell rang.".into(),
                from: Some("Ilse".into()),
                place: Some(place.into()),
                when: "today".into(),
                hops: 1,
                tellings: 2,
                wards: 1,
            });
            assert_eq!(
                row.attribution,
                format!("Ilse, {expected}, today — at one remove — and 1 other since, in 1 ward:")
            );
        }
    }

    #[test]
    fn attribution_keeps_the_mouth_register_and_repeat_counts() {
        for (hops, register) in [
            (0, "you saw it yourself"),
            (1, "at one remove"),
            (2, "at second hand"),
            (3, "at third hand or worse"),
            (u8::MAX, "at third hand or worse"),
        ] {
            let row = row_from_entry(cathedral_sim::JournalEntry {
                word: "The salt weighed short.".into(),
                from: Some("a stranger".into()),
                place: Some("the porter stand".into()),
                when: "yesterday".into(),
                hops,
                tellings: 4,
                wards: 2,
            });
            assert_eq!(
                row.attribution,
                format!(
                    "a stranger, at the porter stand, yesterday — {register} — and 3 others since, in 2 wards:"
                )
            );
            assert_eq!(row.word, "“The salt weighed short.”");
        }
        let row = row_from_entry(cathedral_sim::JournalEntry {
            word: "The bell rang.".into(),
            from: None,
            place: None,
            when: "at some point".into(),
            hops: 0,
            tellings: 1,
            wards: 0,
        });
        assert_eq!(row.attribution, "at some point — you saw it yourself:");
    }

    #[test]
    fn a_full_journal_keeps_all_rows_and_scrolls_to_both_ends() {
        let mut app = App::new();
        app.insert_resource(JournalUiState { open: true })
            .insert_resource(PlayerJournal {
                entries: (0..24)
                    .map(|i| JournalRow {
                        attribution: format!("Mouth {i}, today — at one remove:"),
                        word: format!("“Sentence {i}.”"),
                    })
                    .collect(),
                standing: Vec::new(),
            })
            .init_resource::<AccumulatedMouseScroll>()
            .add_systems(Startup, spawn_journal_ui)
            .add_systems(Update, (refresh_journal_ui, scroll_journal).chain());
        app.update();
        let entries = {
            let world = app.world_mut();
            let mut roots =
                world.query_filtered::<(Entity, &Children, &Node), With<JournalEntriesRoot>>();
            let (entity, children, node) = roots.single(world).unwrap();
            assert_eq!(children.len(), 24);
            assert_eq!(node.overflow.y, OverflowAxis::Scroll);
            entity
        };
        // The layout's dimensions are physical; the wheel and ScrollPosition
        // are logical. At scale 2, the bottom is 900 logical pixels away.
        app.world_mut().entity_mut(entries).insert(ComputedNode {
            content_size: Vec2::new(700.0, 2400.0),
            size: Vec2::new(700.0, 600.0),
            inverse_scale_factor: 0.5,
            ..default()
        });
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta
            .y = -100.0;
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(entries).unwrap().y, 900.0);
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta = Vec2::ZERO;
        app.world_mut().resource_mut::<PlayerJournal>().entries[0]
            .attribution
            .push_str(" again");
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(entries).unwrap().y, 900.0);
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta
            .y = 100.0;
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(entries).unwrap().y, 0.0);
        app.world_mut().resource_mut::<JournalUiState>().open = false;
        app.world_mut()
            .resource_mut::<AccumulatedMouseScroll>()
            .delta
            .y = -100.0;
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(entries).unwrap().y, 0.0);
    }
}
