//! The `I` inventory screen: what you carry, what rides in your body pockets,
//! and the right-click menu that moves things between the two
//! (`features/extra_pockets.md`).
//!
//! The screen is a *projection*, exactly like the HUD: it never mutates
//! anything. Every menu entry writes a [`PlayerIntent`] and waits for the
//! engine's `CommandResult`, so a refused `pocket_item` ("your mouth is full")
//! reaches the player as the sim's own honest error rather than as a rule this
//! file guessed at. That is why the menu offers "Put in frontbutt" to everyone:
//! slot availability is authored per character in the sim's seed and never
//! crosses into the mirror, so the server is the only place that knows — and it
//! answers `wrong_slot`.
//!
//! Structure and styling follow `config_menu.rs`: a spawn-hidden root, colours
//! from `hud.rs`, `Name` on every clickable so `CATHEDRAL_DRIVE`'s `click` can
//! reach it, and `Changed<Interaction>` polling in `Update`.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::fonts::CathedralFonts;
use crate::map::MapState;

use super::chat::ChatInputState;
use super::config_menu::ConfigMenuState;
use super::hud::{self, SmartActorHudState};
use super::interaction::{InteractionState, PlayerIntent, PlayerSpatialState};
use super::model::{ActorId, ItemId, WorldMirror};
use super::targeting::ActorFocus;
use super::{ITEM_INTERACTION_RADIUS_M, PLAYER_ID, SmartActorRuntime};

use cathedral_sim::BodySlot;

const SCRIM: Color = Color::srgba(0.01, 0.015, 0.03, 0.55);
const PANEL_SOLID: Color = Color::srgba(0.025, 0.03, 0.045, 0.96);
const MENU_SOLID: Color = Color::srgba(0.055, 0.062, 0.085, 0.99);
const TILE_BG: Color = Color::srgba(0.10, 0.11, 0.15, 1.0);
const TILE_BG_HOVER: Color = Color::srgba(0.19, 0.21, 0.26, 1.0);
const ENTRY_BG_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);

/// Where the panel drops the context menu when no real cursor position is
/// available — a scripted `click` has no pointer, but the menu must still
/// appear somewhere sane for the screenshot.
const MENU_FALLBACK_POSITION: Vec2 = Vec2::new(700.0, 300.0);
/// Kept clear of the window edges so a menu opened near the corner still fits.
const MENU_WIDTH_PX: f32 = 230.0;
const MENU_HEIGHT_PX: f32 = 190.0;

// ---------------------------------------------------------------- the state

/// Which shelf a tile's unit sits on: in the open, or in one of the cavities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemSource {
    Carried,
    Pocketed(BodySlot),
}

/// One menu entry's effect. Every one of them is a sim verb; none of them is
/// applied locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InventoryAction {
    Pocket(BodySlot),
    Retrieve,
    Swallow,
    Spit,
    Gargle,
    Expel,
    Eat,
}

impl InventoryAction {
    /// The label the button shows — also the `Name` a drive script clicks.
    fn label(self, spit_target: Option<&str>) -> String {
        match self {
            Self::Pocket(BodySlot::Mouth) => "Put in mouth".into(),
            Self::Pocket(BodySlot::Butt) => "Put in butt".into(),
            Self::Pocket(BodySlot::Frontbutt) => "Put in frontbutt".into(),
            Self::Retrieve => "Take out".into(),
            Self::Swallow => "Swallow".into(),
            Self::Spit => match spit_target {
                Some(name) => format!("Spit at {name}"),
                None => "Spit".into(),
            },
            Self::Gargle => "Gargle".into(),
            Self::Expel => "Expel".into(),
            Self::Eat => "Eat".into(),
        }
    }
}

/// The open right-click menu: which unit it acts on, where it sits, and the
/// target `spit` was aimed at when it opened (resolved once, so the menu does
/// not change under the player's hand while he reads it).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ContextMenu {
    pub(super) item_id: ItemId,
    pub(super) source: ItemSource,
    pub(super) screen_pos: Vec2,
    pub(super) spit_target: Option<(ActorId, String)>,
}

#[derive(Resource, Debug, Default)]
pub struct InventoryUiState {
    pub open: bool,
    pub(super) context_menu: Option<ContextMenu>,
}

// ----------------------------------------------------------- the components

#[derive(Component)]
pub(super) struct InventoryUiRoot;

/// The container whose children are the sections; rebuilt wholesale whenever
/// the projection changes (a dozen nodes — respawning is cheaper than diffing).
#[derive(Component)]
pub(super) struct InventorySectionsRoot;

#[derive(Component)]
pub(super) struct InventoryContextMenuRoot;

#[derive(Component)]
pub(super) struct InventoryFeedbackText;

#[derive(Component)]
pub(super) struct InventoryCloseButton;

#[derive(Component, Debug, Clone)]
pub(super) struct InventoryTile {
    pub(super) item_id: ItemId,
    pub(super) source: ItemSource,
}

#[derive(Component, Debug, Clone)]
pub(super) struct InventoryActionButton {
    pub(super) action: InventoryAction,
    pub(super) item_id: ItemId,
}

// ------------------------------------------------------------- the spawning

pub(super) fn spawn_inventory_ui(mut commands: Commands, fonts: Option<Res<CathedralFonts>>) {
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
            Name::new("Inventory overlay"),
            InventoryUiRoot,
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
                    Name::new("Inventory panel"),
                    Node {
                        width: px(760),
                        max_width: percent(94),
                        padding: UiRect::axes(px(26), px(22)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(14),
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BackgroundColor(PANEL_SOLID),
                ))
                .with_children(|panel| {
                    spawn_header(panel, display_font);
                    panel.spawn((
                        Name::new("Inventory sections"),
                        InventorySectionsRoot,
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(12),
                            ..default()
                        },
                    ));
                    panel.spawn((
                        Name::new("Inventory feedback"),
                        InventoryFeedbackText,
                        Text::new(""),
                        TextFont {
                            font: body_font.clone(),
                            font_size: FontSize::Px(14.5),
                            ..default()
                        },
                        TextColor(hud::MUTED),
                    ));
                });
            // A sibling of the panel, so its absolute position is in screen
            // space rather than inside the panel's padding box.
            overlay.spawn((
                Name::new("Inventory context menu"),
                InventoryContextMenuRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: px(MENU_WIDTH_PX),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(5)),
                    row_gap: px(2),
                    border_radius: BorderRadius::all(px(8)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(MENU_SOLID),
                ZIndex(36),
            ));
        });
}

fn spawn_header(panel: &mut ChildSpawnerCommands, font: FontSource) {
    panel
        .spawn((
            Name::new("Inventory header"),
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new("PACK AND POCKETS"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(19.0),
                    ..default()
                },
                TextColor(hud::TEXT),
            ));
            row.spawn((
                Name::new("Inv close"),
                Button,
                InventoryCloseButton,
                Node {
                    padding: UiRect::axes(px(14), px(6)),
                    border_radius: BorderRadius::all(px(999)),
                    ..default()
                },
                BackgroundColor(TILE_BG),
            ))
            .with_child((
                Text::new("Close  [I]"),
                TextFont {
                    font,
                    font_size: FontSize::Px(13.5),
                    ..default()
                },
                TextColor(hud::MUTED),
            ));
        });
}

// -------------------------------------------------------- the projection

/// One clickable unit of the screen.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct TileSpec {
    pub(super) item_id: ItemId,
    pub(super) source: ItemSource,
    pub(super) label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SectionSpec {
    pub(super) title: String,
    pub(super) tiles: Vec<TileSpec>,
}

/// How many units of `item_id` the actor has pocketed, over all cavities.
fn pocketed_units(pockets: &[(BodySlot, ItemId)], item_id: &ItemId) -> u32 {
    pockets
        .iter()
        .filter(|(_, pocketed)| pocketed == item_id)
        .count() as u32
}

/// The period-euphemistic heading each cavity gets. The register matters: the
/// sheet reads like the century it happens in (`features/extra_pockets.md`).
fn slot_title(slot: BodySlot) -> &'static str {
    match slot {
        BodySlot::Mouth => "In your mouth",
        BodySlot::Butt => "Carried privily, behind",
        BodySlot::Frontbutt => "Carried privily, before",
    }
}

/// Builds the whole screen from the mirror: the open carry first, then one
/// section per cavity. A stack with every unit pocketed leaves the carried
/// list entirely — it is in a cheek, and that is where it is shown.
///
/// `mouth` and `butt` always render (everybody has them, and an empty section
/// is how the player learns they exist); `frontbutt` renders only when it has
/// something in it, because whether the player has one at all is the sim's
/// authored secret.
pub(super) fn build_sections(mirror: Option<&WorldMirror>) -> Vec<SectionSpec> {
    let player_id = ActorId(PLAYER_ID.into());
    let player = mirror.and_then(|mirror| mirror.actor(&player_id));
    let mut carried = Vec::new();
    if let (Some(mirror), Some(player)) = (mirror, player) {
        for item_id in &player.holds {
            let Some(item) = mirror.item(item_id) else {
                continue;
            };
            let open_units = item.quantity.saturating_sub(pocketed_units(&player.pockets, item_id));
            if open_units == 0 {
                continue;
            }
            carried.push(TileSpec {
                item_id: item_id.clone(),
                source: ItemSource::Carried,
                label: if open_units > 1 {
                    format!("{open_units} {}", item.display_plural)
                } else {
                    item.name.clone()
                },
            });
        }
    }
    let mut sections = vec![SectionSpec {
        title: "Carried".into(),
        tiles: carried,
    }];

    for slot in BodySlot::ALL {
        let mut tiles: Vec<TileSpec> = Vec::new();
        if let (Some(mirror), Some(player)) = (mirror, player) {
            for (pocketed_slot, item_id) in &player.pockets {
                if *pocketed_slot != slot {
                    continue;
                }
                // Two units of one stack in one cavity are two entries in the
                // sim; the screen folds them into one tile with a count.
                if let Some(tile) = tiles.iter_mut().find(|tile| tile.item_id == *item_id) {
                    let count = tile.label.split_whitespace().next().and_then(|first| first.parse::<u32>().ok());
                    let base = mirror.item(item_id).map(|item| item.name.clone());
                    if let (Some(base), Some(count)) = (base, count.or(Some(1))) {
                        tile.label = format!("{} {base}", count + 1);
                    }
                    continue;
                }
                let label = mirror
                    .item(item_id)
                    .map_or_else(|| item_id.0.clone(), |item| item.name.clone());
                tiles.push(TileSpec {
                    item_id: item_id.clone(),
                    source: ItemSource::Pocketed(slot),
                    label,
                });
            }
        }
        if tiles.is_empty() && slot == BodySlot::Frontbutt {
            continue;
        }
        sections.push(SectionSpec {
            title: slot_title(slot).into(),
            tiles,
        });
    }
    sections
}

/// The entries a right-click on one tile deserves. Everything a cavity can do
/// is offered; the sim refuses what does not apply (a coin cannot be gargled)
/// and the refusal is the feedback line.
pub(super) fn menu_actions(source: ItemSource, has_spit_target: bool) -> Vec<InventoryAction> {
    match source {
        ItemSource::Carried => vec![
            InventoryAction::Pocket(BodySlot::Mouth),
            InventoryAction::Pocket(BodySlot::Butt),
            InventoryAction::Pocket(BodySlot::Frontbutt),
            InventoryAction::Eat,
        ],
        ItemSource::Pocketed(BodySlot::Mouth) => {
            let mut actions = vec![InventoryAction::Swallow];
            // No one in reach: the entry is absent rather than dead, so the
            // menu never lies about what a click will do.
            if has_spit_target {
                actions.push(InventoryAction::Spit);
            }
            actions.push(InventoryAction::Gargle);
            actions.push(InventoryAction::Retrieve);
            actions
        }
        ItemSource::Pocketed(_) => vec![InventoryAction::Retrieve, InventoryAction::Expel],
    }
}

// ---------------------------------------------------------------- the input

/// `I` opens and closes the screen; while it is open the cursor is released so
/// the menus can be clicked. The Esc menu and the fullscreen map own the cursor
/// too, so the inventory yields to the first and refuses to open over either.
pub(super) fn toggle_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<ConfigMenuState>>,
    chat: Option<Res<ChatInputState>>,
    map: Option<Res<MapState>>,
    mut inventory: ResMut<InventoryUiState>,
    cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
) {
    let menu_open = menu.is_some_and(|menu| menu.open);
    let chat_open = chat.is_some_and(|chat| chat.open);
    let map_open = map.is_some_and(|map| map.fullscreen_open);

    // The settings menu takes over the cursor; yield to it without touching
    // the cursor ourselves (it has already released it).
    if menu_open && inventory.open {
        inventory.open = false;
        inventory.context_menu = None;
        return;
    }
    if !keyboard.just_pressed(KeyCode::KeyI) || menu_open || chat_open {
        return;
    }
    if !inventory.open && map_open {
        return;
    }
    let open = !inventory.open;
    inventory.open = open;
    inventory.context_menu = None;
    let Some(mut cursor) = cursor else {
        return;
    };
    if open {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    } else if !map_open {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

/// Opens the context menu for the tile under the pointer, and closes an open
/// one on any click that is not on a menu entry.
///
/// A *left* press on a tile opens the menu too. The spec asks for right-click,
/// and right-click is what a mouse user will reach for; the left press is a
/// deliberate addition so `CATHEDRAL_DRIVE`'s `click <Name>` — which can only
/// inject `Interaction::Pressed` — can reach the menus at all.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_inventory_tile_clicks(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mirror: Option<Res<WorldMirror>>,
    focus: Option<Res<ActorFocus>>,
    tiles: Query<(&Interaction, &InventoryTile)>,
    changed_tiles: Query<(), (Changed<Interaction>, With<InventoryTile>)>,
    entries: Query<&Interaction, With<InventoryActionButton>>,
    mut inventory: ResMut<InventoryUiState>,
) {
    if !inventory.open {
        return;
    }
    let right = mouse.just_pressed(MouseButton::Right);
    let left = mouse.just_pressed(MouseButton::Left);

    // `Changed<Interaction>` is how a drive-injected press announces itself;
    // for a real mouse the button edge is the signal and the hover is the
    // target.
    let injected = !changed_tiles.is_empty();
    let hit = tiles.iter().find(|(interaction, _)| match **interaction {
        Interaction::Pressed => true,
        Interaction::Hovered => right,
        Interaction::None => false,
    });

    if let Some((_, tile)) = hit
        && (right || left || injected)
    {
        // The menu opens at the pointer — but only when a real button edge put
        // it there. A drive-injected press moves no pointer, so following the
        // window's stale cursor would drop the menu in a corner.
        let screen_pos = window
            .as_deref()
            .filter(|_| right || left)
            .and_then(|window| {
                let cursor = window.cursor_position()?;
                let width = window.width();
                let height = window.height();
                Some(Vec2::new(
                    cursor.x.min((width - MENU_WIDTH_PX).max(0.0)),
                    cursor.y.min((height - MENU_HEIGHT_PX).max(0.0)),
                ))
            })
            .unwrap_or(MENU_FALLBACK_POSITION);
        inventory.context_menu = Some(ContextMenu {
            item_id: tile.item_id.clone(),
            source: tile.source,
            screen_pos,
            spit_target: resolve_spit_target(mirror.as_deref(), focus.as_deref()),
        });
        return;
    }

    // A click that landed on neither a tile nor a menu entry dismisses the
    // menu — the ordinary desktop rule.
    if (left || right)
        && inventory.context_menu.is_some()
        && !entries
            .iter()
            .any(|interaction| *interaction != Interaction::None)
    {
        inventory.context_menu = None;
    }
}

/// Whoever the player is looking at, if they are inside `spit`'s 4 m — the same
/// band and the same gate the right-click offer uses.
fn resolve_spit_target(
    mirror: Option<&WorldMirror>,
    focus: Option<&ActorFocus>,
) -> Option<(ActorId, String)> {
    let focused = focus?.item.as_ref()?;
    if focused.body_distance_m > ITEM_INTERACTION_RADIUS_M || focused.actor_id.0 == PLAYER_ID {
        return None;
    }
    let name = mirror
        .and_then(|mirror| mirror.actor(&focused.actor_id))
        .map_or_else(|| focused.actor_id.0.clone(), |actor| actor.name_for_player.clone());
    Some((focused.actor_id.clone(), name))
}

/// Turns a pressed menu entry into a [`PlayerIntent`]. Nothing is applied
/// locally: the tile disappears when the engine's next snapshot says so.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_inventory_actions(
    entries: Query<(&Interaction, &InventoryActionButton), Changed<Interaction>>,
    close: Query<&Interaction, (Changed<Interaction>, With<InventoryCloseButton>)>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    mut inventory: ResMut<InventoryUiState>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut state: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
    mut intents: MessageWriter<PlayerIntent>,
    mut cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
) {
    if !inventory.open {
        return;
    }
    if close.iter().any(|interaction| *interaction == Interaction::Pressed) {
        inventory.open = false;
        inventory.context_menu = None;
        if let Some(cursor) = cursor.as_deref_mut() {
            cursor.visible = false;
            cursor.grab_mode = CursorGrabMode::Locked;
        }
        return;
    }
    let Some((_, pressed)) = entries
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
    else {
        return;
    };
    let spit_target = inventory
        .context_menu
        .as_ref()
        .and_then(|menu| menu.spit_target.clone());
    inventory.context_menu = None;

    if !runtime.interactions_enabled() {
        hud.toast("The actor engine is offline");
        return;
    }
    let revision = runtime.mirror_revision.unwrap_or(0);
    let item_id = pressed.item_id.clone();
    let request_id = state.request_id();
    let pending = match pressed.action {
        InventoryAction::Expel => super::interaction::PendingKind::Expel,
        _ => super::interaction::PendingKind::BodySlot {
            item_id: item_id.clone(),
        },
    };
    let intent = match pressed.action {
        InventoryAction::Pocket(slot) => PlayerIntent::Pocket {
            request_id: request_id.clone(),
            item_id,
            slot,
        },
        InventoryAction::Retrieve => PlayerIntent::Retrieve {
            request_id: request_id.clone(),
            item_id,
        },
        InventoryAction::Swallow => PlayerIntent::Swallow {
            request_id: request_id.clone(),
            item_id,
        },
        InventoryAction::Gargle => PlayerIntent::Gargle {
            request_id: request_id.clone(),
            item_id,
        },
        InventoryAction::Eat => PlayerIntent::Eat {
            request_id: request_id.clone(),
            item_id,
        },
        InventoryAction::Expel => PlayerIntent::Expel {
            request_id: request_id.clone(),
        },
        InventoryAction::Spit => {
            let Some((target_id, _)) = spit_target else {
                hud.toast("Nobody within spitting distance");
                return;
            };
            let Ok(player) = players.single() else { return };
            let position = player.translation();
            let spatial_seq = spatial.position_for_action(position);
            PlayerIntent::Spit {
                request_id: request_id.clone(),
                item_id,
                target_id,
                spatial_seq,
                position,
            }
        }
    };
    state.insert_pending(request_id, pending, revision);
    intents.write(intent);
}

// ------------------------------------------------------------- the refresh

/// Shows/hides the root, mirrors the newest toast into the panel's own
/// feedback line (the HUD's transient sits *behind* this overlay), and tints
/// the hovered tiles and entries.
#[allow(clippy::type_complexity)]
pub(super) fn update_inventory_ui(
    inventory: Res<InventoryUiState>,
    hud_state: Res<SmartActorHudState>,
    mut root: Query<&mut Node, (With<InventoryUiRoot>, Without<InventoryContextMenuRoot>)>,
    mut menu_root: Query<&mut Node, With<InventoryContextMenuRoot>>,
    mut feedback: Query<&mut Text, With<InventoryFeedbackText>>,
    mut tiles: Query<(&Interaction, &mut BackgroundColor), With<InventoryTile>>,
    mut entries: Query<
        (&Interaction, &mut BackgroundColor),
        (With<InventoryActionButton>, Without<InventoryTile>),
    >,
) {
    let Ok(mut root_node) = root.single_mut() else {
        return;
    };
    let desired = if inventory.open {
        Display::Flex
    } else {
        Display::None
    };
    if root_node.display != desired {
        root_node.display = desired;
    }
    if let Ok(mut menu_node) = menu_root.single_mut() {
        let menu_desired = match (&inventory.context_menu, inventory.open) {
            (Some(_), true) => Display::Flex,
            _ => Display::None,
        };
        if menu_node.display != menu_desired {
            menu_node.display = menu_desired;
        }
        if let Some(menu) = &inventory.context_menu {
            let left = px(menu.screen_pos.x);
            let top = px(menu.screen_pos.y);
            if menu_node.left != left || menu_node.top != top {
                menu_node.left = left;
                menu_node.top = top;
            }
        }
    }
    if !inventory.open {
        return;
    }
    if let Ok(mut text) = feedback.single_mut() {
        let line = hud_state.transient_text().unwrap_or_default().to_string();
        if text.0 != line {
            text.0 = line;
        }
    }
    for (interaction, mut background) in &mut tiles {
        background.0 = if *interaction == Interaction::None {
            TILE_BG
        } else {
            TILE_BG_HOVER
        };
    }
    for (interaction, mut background) in &mut entries {
        background.0 = if *interaction == Interaction::None {
            Color::NONE
        } else {
            ENTRY_BG_HOVER
        };
    }
}

/// Rebuilds the tile list and the context menu when — and only when — what
/// they would show has actually changed. The projection is a dozen short
/// strings, so building it every frame and comparing is cheaper than diffing
/// the node tree, and respawning stays rare.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_inventory_ui(
    mut commands: Commands,
    inventory: Res<InventoryUiState>,
    mirror: Option<Res<WorldMirror>>,
    fonts: Option<Res<CathedralFonts>>,
    sections_root: Query<(Entity, Option<&Children>), With<InventorySectionsRoot>>,
    menu_root: Query<(Entity, Option<&Children>), With<InventoryContextMenuRoot>>,
    mut cached_sections: Local<Option<Vec<SectionSpec>>>,
    mut cached_menu: Local<Option<ContextMenu>>,
) {
    let Ok((sections_entity, sections_children)) = sections_root.single() else {
        return;
    };
    let font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();

    if inventory.open {
        let sections = build_sections(mirror.as_deref());
        if cached_sections.as_ref() != Some(&sections) {
            for child in sections_children.into_iter().flatten() {
                commands.entity(*child).despawn();
            }
            let font = font.clone();
            let spawn = sections.clone();
            commands.entity(sections_entity).with_children(move |root| {
                for section in &spawn {
                    spawn_section(root, section, &font);
                }
            });
            *cached_sections = Some(sections);
        }
    } else if cached_sections.is_some() {
        for child in sections_children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        *cached_sections = None;
    }

    let Ok((menu_entity, menu_children)) = menu_root.single() else {
        return;
    };
    let wanted = if inventory.open {
        inventory.context_menu.clone()
    } else {
        None
    };
    if *cached_menu != wanted {
        for child in menu_children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        if let Some(menu) = &wanted {
            let actions = menu_actions(menu.source, menu.spit_target.is_some());
            let target = menu.spit_target.as_ref().map(|(_, name)| name.clone());
            let item_id = menu.item_id.clone();
            commands.entity(menu_entity).with_children(move |parent| {
                for action in &actions {
                    spawn_menu_entry(parent, *action, &item_id, target.as_deref(), &font);
                }
            });
        }
        *cached_menu = wanted;
    }
}

fn spawn_section(root: &mut ChildSpawnerCommands, section: &SectionSpec, font: &FontSource) {
    root.spawn((
        Name::new(format!("Inv section: {}", section.title)),
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
    ))
    .with_children(|column| {
        column.spawn((
            Text::new(section.title.clone()),
            TextFont {
                font: font.clone(),
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(hud::MUTED),
        ));
        if section.tiles.is_empty() {
            column.spawn((
                Text::new("nothing"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(hud::MUTED),
            ));
            return;
        }
        column
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(8),
                    row_gap: px(8),
                    ..default()
                },
            ))
            .with_children(|grid| {
                for tile in &section.tiles {
                    spawn_tile(grid, tile, font);
                }
            });
    });
}

fn spawn_tile(grid: &mut ChildSpawnerCommands, tile: &TileSpec, font: &FontSource) {
    grid.spawn((
        Name::new(format!("Inv item: {} ({})", tile.label, tile.item_id.0)),
        Button,
        InventoryTile {
            item_id: tile.item_id.clone(),
            source: tile.source,
        },
        Node {
            min_width: px(140),
            padding: UiRect::axes(px(12), px(9)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(7)),
            ..default()
        },
        BackgroundColor(TILE_BG),
    ))
    .with_child((
        Text::new(tile.label.clone()),
        TextFont {
            font: font.clone(),
            font_size: FontSize::Px(14.5),
            ..default()
        },
        TextColor(hud::TEXT),
    ));
}

fn spawn_menu_entry(
    parent: &mut ChildSpawnerCommands,
    action: InventoryAction,
    item_id: &ItemId,
    spit_target: Option<&str>,
    font: &FontSource,
) {
    let label = action.label(spit_target);
    parent
        .spawn((
            Name::new(format!("Inv action: {label}")),
            Button,
            InventoryActionButton {
                action,
                item_id: item_id.clone(),
            },
            Node {
                width: percent(100),
                padding: UiRect::axes(px(10), px(6)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font: font.clone(),
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(hud::TEXT),
        ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_actors::model::{
        ActorControl, ActorSnapshot, ItemSnapshot, Position, WorldSnapshot,
    };

    fn item(id: &str, name: &str, quantity: u32) -> ItemSnapshot {
        ItemSnapshot {
            id: ItemId(id.into()),
            kind: "loaf".into(),
            name: name.into(),
            display_plural: format!("{name}s"),
            visual_key: "loaf".into(),
            quantity,
            metadata: Default::default(),
        }
    }

    fn mirror_with(holds: &[&str], pockets: &[(BodySlot, &str)], items: Vec<ItemSnapshot>) -> WorldMirror {
        let player = ActorSnapshot {
            id: ActorId(PLAYER_ID.into()),
            name_for_player: "You".into(),
            control: ActorControl::Player,
            position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: holds.iter().map(|id| ItemId((*id).into())).collect(),
            active_gesture: None,
            statuses: Vec::new(),
            pockets: pockets
                .iter()
                .map(|(slot, id)| (*slot, ItemId((*id).into())))
                .collect(),
        };
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId(PLAYER_ID.into()),
                actors: vec![player],
                items,
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
            })
            .unwrap();
        mirror
    }

    #[test]
    fn screen_spawns_hidden_with_its_containers() {
        let mut app = App::new();
        app.add_systems(Startup, spawn_inventory_ui);
        app.update();

        let mut root_query = app
            .world_mut()
            .query_filtered::<&Node, With<InventoryUiRoot>>();
        let root = root_query.single(app.world()).expect("inventory root exists");
        assert_eq!(root.display, Display::None);

        let mut sections = app
            .world_mut()
            .query_filtered::<(), With<InventorySectionsRoot>>();
        assert_eq!(sections.iter(app.world()).count(), 1);

        let mut menu = app
            .world_mut()
            .query_filtered::<&Node, With<InventoryContextMenuRoot>>();
        assert_eq!(
            menu.single(app.world()).expect("menu root exists").display,
            Display::None
        );

        let mut close = app.world_mut().query::<&InventoryCloseButton>();
        assert_eq!(close.iter(app.world()).count(), 1);
    }

    /// The sections an empty-handed player sees: an empty carry plus the two
    /// cavities everybody has. The frontbutt stays out of it — whether the
    /// player has one is the sim's business.
    #[test]
    fn an_empty_player_still_sees_the_two_common_slots() {
        let titles: Vec<String> = build_sections(None)
            .into_iter()
            .map(|section| section.title)
            .collect();
        assert_eq!(
            titles,
            vec!["Carried", "In your mouth", "Carried privily, behind"]
        );
        assert!(build_sections(None).iter().all(|s| s.tiles.is_empty()));
    }

    /// A stack with every unit pocketed leaves the carry list and appears in
    /// its cavity; a stack with units still in the open stays in both, counted.
    #[test]
    fn pocketed_units_move_out_of_the_carried_section() {
        let mirror = mirror_with(
            &["loaf", "sparks"],
            &[(BodySlot::Mouth, "loaf"), (BodySlot::Butt, "sparks")],
            vec![item("loaf", "loaf", 1), item("sparks", "spark", 3)],
        );
        let sections = build_sections(Some(&mirror));
        let carried = &sections[0];
        assert_eq!(carried.title, "Carried");
        assert_eq!(carried.tiles.len(), 1, "the pocketed loaf is gone: {carried:?}");
        assert_eq!(carried.tiles[0].label, "2 sparks");

        let mouth = sections
            .iter()
            .find(|section| section.title == "In your mouth")
            .expect("mouth section");
        assert_eq!(mouth.tiles.len(), 1);
        assert_eq!(mouth.tiles[0].source, ItemSource::Pocketed(BodySlot::Mouth));

        let behind = sections
            .iter()
            .find(|section| section.title == "Carried privily, behind")
            .expect("behind section");
        assert_eq!(behind.tiles.len(), 1);
    }

    /// A frontbutt section only exists once something is in it.
    #[test]
    fn the_frontbutt_section_appears_only_when_occupied() {
        let mirror = mirror_with(
            &["loaf"],
            &[(BodySlot::Frontbutt, "loaf")],
            vec![item("loaf", "loaf", 1)],
        );
        assert!(
            build_sections(Some(&mirror))
                .iter()
                .any(|section| section.title == "Carried privily, before")
        );
    }

    /// Two units of one stack in one cheek fold into a single counted tile.
    #[test]
    fn two_units_in_one_cavity_fold_into_one_tile() {
        let mirror = mirror_with(
            &["sparks"],
            &[(BodySlot::Mouth, "sparks"), (BodySlot::Mouth, "sparks")],
            vec![item("sparks", "spark", 2)],
        );
        let sections = build_sections(Some(&mirror));
        let mouth = sections
            .iter()
            .find(|section| section.title == "In your mouth")
            .expect("mouth section");
        assert_eq!(mouth.tiles.len(), 1);
        assert_eq!(mouth.tiles[0].label, "2 spark");
        assert!(sections[0].tiles.is_empty(), "both units are pocketed");
    }

    /// Which entries each shelf offers, and that `spit` is absent with nobody
    /// in reach.
    #[test]
    fn menu_entries_follow_the_shelf() {
        assert_eq!(
            menu_actions(ItemSource::Carried, true),
            vec![
                InventoryAction::Pocket(BodySlot::Mouth),
                InventoryAction::Pocket(BodySlot::Butt),
                InventoryAction::Pocket(BodySlot::Frontbutt),
                InventoryAction::Eat,
            ]
        );
        assert_eq!(
            menu_actions(ItemSource::Pocketed(BodySlot::Mouth), false),
            vec![
                InventoryAction::Swallow,
                InventoryAction::Gargle,
                InventoryAction::Retrieve,
            ]
        );
        assert!(
            menu_actions(ItemSource::Pocketed(BodySlot::Mouth), true)
                .contains(&InventoryAction::Spit)
        );
        assert_eq!(
            menu_actions(ItemSource::Pocketed(BodySlot::Butt), true),
            vec![InventoryAction::Retrieve, InventoryAction::Expel]
        );
    }

    /// `I` opens and closes the screen, refuses to open under the settings
    /// menu, and yields the moment that menu takes the cursor.
    #[test]
    fn the_screen_toggles_on_i_and_yields_to_the_settings_menu() {
        let mut app = App::new();
        app.init_resource::<InventoryUiState>()
            .init_resource::<ConfigMenuState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, toggle_inventory);

        // `reset`, not `clear`: `clear` only drops the just_pressed edge, and a
        // key that stays `pressed` never produces another one.
        let press = |app: &mut App| {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyI);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset(KeyCode::KeyI);
        };

        press(&mut app);
        assert!(app.world().resource::<InventoryUiState>().open);
        press(&mut app);
        assert!(!app.world().resource::<InventoryUiState>().open);

        // Under the settings menu the key does nothing…
        app.world_mut().resource_mut::<ConfigMenuState>().open = true;
        press(&mut app);
        assert!(!app.world().resource::<InventoryUiState>().open);

        // …and an already-open screen closes itself when that menu appears.
        app.world_mut().resource_mut::<ConfigMenuState>().open = false;
        press(&mut app);
        assert!(app.world().resource::<InventoryUiState>().open);
        app.world_mut().resource_mut::<ConfigMenuState>().open = true;
        app.update();
        assert!(!app.world().resource::<InventoryUiState>().open);
    }

    /// A press on a tile opens that tile's context menu; a press that lands on
    /// nothing dismisses it again.
    #[test]
    fn pressing_a_tile_opens_and_a_stray_click_dismisses_the_menu() {
        let mut app = App::new();
        app.init_resource::<InventoryUiState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, handle_inventory_tile_clicks);
        app.world_mut().resource_mut::<InventoryUiState>().open = true;
        let tile = app
            .world_mut()
            .spawn((
                Interaction::Pressed,
                InventoryTile {
                    item_id: ItemId("k3f9x".into()),
                    source: ItemSource::Pocketed(BodySlot::Mouth),
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let menu = app
            .world()
            .resource::<InventoryUiState>()
            .context_menu
            .clone()
            .expect("a press opens the menu");
        assert_eq!(menu.item_id, ItemId("k3f9x".into()));
        assert_eq!(menu.source, ItemSource::Pocketed(BodySlot::Mouth));
        assert!(menu.spit_target.is_none(), "no focus, no target");

        // The pointer moves off the tile and clicks the scrim.
        *app.world_mut().get_mut::<Interaction>(tile).unwrap() = Interaction::None;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(
            app.world()
                .resource::<InventoryUiState>()
                .context_menu
                .is_none(),
            "a click on nothing dismisses the menu"
        );
    }

    /// Every clickable carries the `Name` a drive script matches on.
    #[test]
    fn tiles_and_entries_are_named_for_drive_clicks() {
        let tile = TileSpec {
            item_id: ItemId("k3f9x".into()),
            source: ItemSource::Carried,
            label: "loaf".into(),
        };
        let mut app = App::new();
        app.add_systems(Startup, spawn_inventory_ui);
        app.update();
        let sections = app
            .world_mut()
            .query_filtered::<Entity, With<InventorySectionsRoot>>()
            .single(app.world())
            .expect("sections root");
        app.world_mut()
            .commands()
            .entity(sections)
            .with_children(|root| {
                spawn_section(
                    root,
                    &SectionSpec {
                        title: "Carried".into(),
                        tiles: vec![tile],
                    },
                    &FontSource::default(),
                );
            });
        app.update();

        let mut names = app.world_mut().query_filtered::<&Name, With<InventoryTile>>();
        let name = names.single(app.world()).expect("one tile");
        assert_eq!(name.as_str(), "Inv item: loaf (k3f9x)");

        assert_eq!(
            InventoryAction::Spit.label(Some("Ilse")),
            "Spit at Ilse",
            "the entry names its target so a drive click can find it"
        );
    }
}
