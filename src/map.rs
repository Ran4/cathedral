//! The in-game city map: a persistent minimap (bottom-right, ~1/6 screen width)
//! and an `M`-toggled fullscreen map. Both draw the baked
//! `assets/textures/city_map.png` — rendered from the authoritative cadastral
//! SVG by `scripts/render_map_texture.py` — with a "you are here" marker that
//! tracks the player's position and facing.
//!
//! In "debug mode" (the `B` area overlay), clicking the fullscreen map teleports
//! the player. The target is snapped onto the baked walkable surface, so a click
//! can never drop you inside a building.
//!
//! ## World <-> map transform (single source of truth)
//!
//! The baked image is the SVG viewBox sub-rectangle `[VX0, VX1] x [VY0, VY1]`
//! under the plan's projection `screen(x, z) = (-z, -x)` (east-right, north-up).
//! These four constants **must** match `scripts/render_map_texture.py`. For a
//! normalized point `(u, v)` in `[0, 1]^2` over the image:
//!
//! ```text
//! u = (-z - VX0) / (VX1 - VX0)     v = (-x - VY0) / (VY1 - VY0)
//! z = -(VX0 + u * (VX1 - VX0))     x = -(VY0 + v * (VY1 - VY0))
//! ```

use std::f32::consts::FRAC_PI_2;

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use cathedral_sim::WALK_Y;

use crate::controller::{PlayerController, TeleportPlayer};
use crate::fonts::CathedralFonts;
use crate::nav_overlay::Navigation;
use crate::smart_actors::{AreaDebugState, ChatInputState, ConfigMenuState};

// --- The baked crop, mirrored from scripts/render_map_texture.py ------------ //
const VX0: f32 = -600.0;
const VX1: f32 = 725.0;
const VY0: f32 = -610.0;
const VY1: f32 = 830.0;

/// Image aspect (width / height), from the crop. The minimap and fullscreen
/// frames are sized in one viewport axis and derive the other from this, so the
/// map never stretches regardless of window shape.
const MAP_ASPECT: f32 = (VX1 - VX0) / (VY1 - VY0);

/// Minimap width as a fraction of the viewport width (~1/6).
const MINIMAP_WIDTH_VW: f32 = 16.6;
/// Fullscreen map height as a fraction of the viewport height.
const FULLSCREEN_HEIGHT_VH: f32 = 92.0;

/// Snap a click to a named place if it lands within this many metres of one.
const PLACE_SNAP_M: f64 = 20.0;
/// Off walkable ground: accept the nearest graph node only within this distance.
const NODE_SNAP_M: f64 = 8.0;

const MAP_FRAME: Color = Color::srgb(0.14, 0.12, 0.09);
const MAP_BACKDROP: Color = Color::srgba(0.0, 0.0, 0.0, 0.82);
const MARKER_FILL: Color = Color::srgb(0.86, 0.18, 0.16);
const MARKER_OUTLINE: Color = Color::srgb(0.99, 0.96, 0.88);
const CAPTION_MUTED: Color = Color::srgb(0.62, 0.64, 0.66);

/// World `(x, z)` -> normalized `(u, v)` over the baked map image.
fn world_to_uv(x: f32, z: f32) -> Vec2 {
    Vec2::new((-z - VX0) / (VX1 - VX0), (-x - VY0) / (VY1 - VY0))
}

/// Normalized `(u, v)` over the map image -> world `(x, z)`.
fn uv_to_world(uv: Vec2) -> (f32, f32) {
    let z = -(VX0 + uv.x * (VX1 - VX0));
    let x = -(VY0 + uv.y * (VY1 - VY0));
    (x, z)
}

/// Marker rotation (radians, clockwise for [`UiTransform`]) for a player yaw.
///
/// The marker's arrow points up (screen `-y`) at rest. On the map, north (+x
/// world) is up and east (-z world) is right, so the player's on-screen facing
/// direction is `(cos yaw, sin yaw)`; rotating the up-arrow onto it is a
/// `yaw + 90°` clockwise turn.
fn marker_rotation(yaw: f32) -> Rot2 {
    Rot2::radians(yaw + FRAC_PI_2)
}

/// Whether the player is `fullscreen_open` on the city map.
#[derive(Resource, Default)]
pub struct MapState {
    pub fullscreen_open: bool,
}

/// The fullscreen overlay root; its [`Visibility`] follows [`MapState`].
#[derive(Component)]
struct FullscreenMapRoot;

/// The corner minimap root; hidden while the fullscreen map is open.
#[derive(Component)]
struct MinimapRoot;

/// The fullscreen map image — carries [`RelativeCursorPosition`] for clicks.
#[derive(Component)]
struct MapClickArea;

/// A "you are here" marker container (one on each map).
#[derive(Component)]
struct PlayerMapMarker;

/// Adds the minimap and the `M`-toggled fullscreen map with click-to-teleport.
pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapState>()
            .add_systems(Startup, setup_map)
            .add_systems(
                Update,
                (
                    (toggle_fullscreen_map, handle_map_teleport_click, sync_map_state).chain(),
                    update_map_markers,
                ),
            );
    }
}

fn setup_map(mut commands: Commands, asset_server: Res<AssetServer>, fonts: Res<CathedralFonts>) {
    let image = asset_server.load("textures/city_map.png");
    spawn_minimap(&mut commands, image.clone());
    spawn_fullscreen_map(&mut commands, image, &fonts);
}

/// The always-on minimap, framed in the bottom-right corner.
fn spawn_minimap(commands: &mut Commands, image: Handle<Image>) {
    commands
        .spawn((
            Name::new("Minimap"),
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                right: px(16),
                bottom: px(16),
                width: vw(MINIMAP_WIDTH_VW),
                height: vw(MINIMAP_WIDTH_VW / MAP_ASPECT),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(4)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(MAP_FRAME),
        ))
        .with_children(|frame| {
            spawn_map_image(frame, image, false);
        });
}

/// The fullscreen map: a dimmed backdrop with a centered, framed map and a
/// short caption. Hidden until `M` is pressed.
fn spawn_fullscreen_map(commands: &mut Commands, image: Handle<Image>, fonts: &CathedralFonts) {
    commands
        .spawn((
            Name::new("Fullscreen map"),
            FullscreenMapRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                left: px(0),
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(MAP_BACKDROP),
            GlobalZIndex(50),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Map frame"),
                Node {
                    width: vh(FULLSCREEN_HEIGHT_VH * MAP_ASPECT),
                    height: vh(FULLSCREEN_HEIGHT_VH),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::all(px(6)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(MAP_FRAME),
            ))
            .with_children(|frame| {
                spawn_map_image(frame, image, true);
            });

            root.spawn((
                Text::new("M  close map    ·    B debug mode: click a place to travel"),
                TextFont {
                    font: fonts.body(),
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(CAPTION_MUTED),
            ));
        });
}

/// The map image filling its frame, with the player marker on top. When
/// `clickable`, it also carries the click-area components used by teleport.
fn spawn_map_image(parent: &mut ChildSpawnerCommands, image: Handle<Image>, clickable: bool) {
    let mut image_node = parent.spawn((
        Name::new("City map"),
        ImageNode::new(image),
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
    ));
    if clickable {
        image_node.insert((MapClickArea, RelativeCursorPosition::default()));
    }
    image_node.with_children(spawn_marker);
}

/// The "you are here" marker: a round dot with an arrow "prow" pointing in the
/// player's facing direction. Positioned and rotated each frame.
fn spawn_marker(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Name::new("Player map marker"),
            PlayerMapMarker,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(22),
                height: px(22),
                ..default()
            },
            // Centered on its (left, top) point and rotated to face; both fields
            // are overwritten every frame by `update_map_markers`.
            UiTransform {
                translation: Val2::percent(-50.0, -50.0),
                scale: Vec2::ONE,
                rotation: Rot2::IDENTITY,
            },
        ))
        .with_children(|marker| {
            // The prow: a small arrow tip at the top (screen -y), tapered so the
            // heading reads clearly once the marker rotates.
            marker.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(8),
                    top: px(0),
                    width: px(6),
                    height: px(10),
                    border_radius: BorderRadius {
                        top_left: px(3),
                        top_right: px(3),
                        bottom_left: px(0),
                        bottom_right: px(0),
                    },
                    ..default()
                },
                BackgroundColor(MARKER_FILL),
            ));
            // The body dot.
            marker.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(5),
                    top: px(5),
                    width: px(12),
                    height: px(12),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(MARKER_FILL),
                BorderColor::all(MARKER_OUTLINE),
            ));
        });
}

/// `M` toggles the fullscreen map (ignored while typing chat or in the settings
/// menu); the map also closes itself if the settings menu opens.
fn toggle_fullscreen_map(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu: Option<Res<ConfigMenuState>>,
    chat: Option<Res<ChatInputState>>,
    mut map_state: ResMut<MapState>,
) {
    let menu_open = menu.map(|m| m.open).unwrap_or(false);
    let chat_open = chat.map(|c| c.open).unwrap_or(false);

    if menu_open && map_state.fullscreen_open {
        // The settings menu takes over the cursor; yield the map to it.
        map_state.fullscreen_open = false;
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyM) && !menu_open && !chat_open {
        map_state.fullscreen_open = !map_state.fullscreen_open;
    }
}

/// Sets every matched node's [`Visibility`], skipping no-op writes so change
/// detection stays quiet.
fn set_visibility<F: bevy::ecs::query::QueryFilter>(
    query: &mut Query<&mut Visibility, F>,
    wanted: Visibility,
) {
    for mut visibility in query.iter_mut() {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Applies `fullscreen_open`: shows/hides the overlay and, on the open/close
/// edge, releases or recaptures the mouse cursor (like the settings menu). The
/// cursor is only recaptured when the settings menu is not the one holding it.
fn sync_map_state(
    map_state: Res<MapState>,
    menu: Option<Res<ConfigMenuState>>,
    mut previous_open: Local<Option<bool>>,
    mut fullscreen: Query<&mut Visibility, (With<FullscreenMapRoot>, Without<MinimapRoot>)>,
    mut minimap: Query<&mut Visibility, (With<MinimapRoot>, Without<FullscreenMapRoot>)>,
    cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
) {
    let open = map_state.fullscreen_open;
    // The fullscreen map shows only when open; the corner minimap hides then, so
    // the two never stack.
    set_visibility(&mut fullscreen, if open { Visibility::Visible } else { Visibility::Hidden });
    set_visibility(&mut minimap, if open { Visibility::Hidden } else { Visibility::Visible });

    if *previous_open == Some(open) {
        return;
    }
    *previous_open = Some(open);

    let Some(mut cursor) = cursor else {
        return;
    };
    if open {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    } else if !menu.map(|m| m.open).unwrap_or(false) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

/// Positions and rotates every map marker from the player's pose.
fn update_map_markers(
    player: Option<Single<(&PlayerController, &Transform)>>,
    mut markers: Query<(&mut Node, &mut UiTransform), With<PlayerMapMarker>>,
) {
    let Some(player) = player else {
        return;
    };
    let (controller, transform) = player.into_inner();
    let uv = world_to_uv(transform.translation.x, transform.translation.z);
    let rotation = marker_rotation(controller.yaw());

    for (mut node, mut ui_transform) in &mut markers {
        node.left = percent(uv.x * 100.0);
        node.top = percent(uv.y * 100.0);
        ui_transform.rotation = rotation;
        ui_transform.translation = Val2::percent(-50.0, -50.0);
    }
}

/// In debug mode (the `B` overlay), a left-click on the fullscreen map teleports
/// the player to a walkable point near the click, snapping to a named place when
/// one is close. Never lands inside a building — the target is always on the
/// baked walkable surface.
fn handle_map_teleport_click(
    mouse: Res<ButtonInput<MouseButton>>,
    area_debug: Option<Res<AreaDebugState>>,
    navigation: Option<Res<Navigation>>,
    click_area: Query<&RelativeCursorPosition, With<MapClickArea>>,
    player: Option<Single<&PlayerController>>,
    mut map_state: ResMut<MapState>,
    mut teleports: MessageWriter<TeleportPlayer>,
) {
    if !map_state.fullscreen_open || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if !area_debug.map(|d| d.is_enabled()).unwrap_or(false) {
        return; // Not in debug mode: the fullscreen map is view-only.
    }
    let (Some(navigation), Some(player)) = (navigation, player) else {
        return;
    };
    let Ok(cursor) = click_area.single() else {
        return;
    };
    if !cursor.cursor_over {
        return;
    }
    let Some(normalized) = cursor.normalized else {
        return;
    };
    // `RelativeCursorPosition.normalized` is centred: (-0.5, -0.5) is the
    // top-left corner, (0.5, 0.5) the bottom-right. Shift to [0, 1] for the map.
    let uv = normalized + Vec2::splat(0.5);
    let (x, z) = uv_to_world(uv);
    let Some(target) = resolve_teleport_target(&navigation.0, x as f64, z as f64) else {
        return; // Clicked off any reachable ground.
    };

    teleports.write(TeleportPlayer {
        position: Vec3::new(target.x as f32, target.y as f32, target.z as f32),
        yaw_degrees: player.yaw().to_degrees(),
        pitch_degrees: 0.0,
    });
    map_state.fullscreen_open = false;
}

/// Resolve a clicked world point to a safe, walkable teleport target:
/// 1. the nearest named place, if within [`PLACE_SNAP_M`];
/// 2. otherwise the exact point, if it is walkable;
/// 3. otherwise the nearest graph node, if within [`NODE_SNAP_M`];
/// 4. otherwise `None` (the click has no reachable ground nearby).
fn resolve_teleport_target(nav: &cathedral_sim::NavData, x: f64, z: f64) -> Option<cathedral_sim::Vec3> {
    let mut nearest_place: Option<(f64, usize)> = None;
    for place in nav.places() {
        let [px, pz] = nav.node_xz(place.node);
        let d2 = (px - x) * (px - x) + (pz - z) * (pz - z);
        if nearest_place.is_none_or(|(best, _)| d2 < best) {
            nearest_place = Some((d2, place.node));
        }
    }
    if let Some((d2, node)) = nearest_place {
        if d2 <= PLACE_SNAP_M * PLACE_SNAP_M {
            return Some(nav.node_point(node));
        }
    }

    if nav.is_walkable(x, z) {
        return Some(cathedral_sim::Vec3::new(x, WALK_Y, z));
    }

    let node = nav.nearest_node(x, z)?;
    let [nx, nz] = nav.node_xz(node);
    let d2 = (nx - x) * (nx - x) + (nz - z) * (nz - z);
    if d2 <= NODE_SNAP_M * NODE_SNAP_M {
        Some(nav.node_point(node))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAV_JSON: &str = include_str!("../assets/world/navigation.json");
    const NAV_BIN: &[u8] = include_bytes!("../assets/world/navigation.bin");

    fn nav() -> cathedral_sim::NavData {
        cathedral_sim::NavData::from_parts(NAV_JSON, NAV_BIN).expect("baked nav graph loads")
    }

    fn round_trip(x: f32, z: f32) {
        let uv = world_to_uv(x, z);
        let (rx, rz) = uv_to_world(uv);
        assert!((rx - x).abs() < 1e-3, "x {x} -> {rx}");
        assert!((rz - z).abs() < 1e-3, "z {z} -> {rz}");
    }

    #[test]
    fn transform_round_trips() {
        // Player spawn (controller.rs PLAYER_SPAWN), the cathedral origin, and a
        // point out toward the walls all recover exactly.
        round_trip(0.0, 95.0);
        round_trip(0.0, 0.0);
        round_trip(-300.0, 400.0);
    }

    #[test]
    fn spawn_lands_inside_the_image() {
        // The player spawn must fall within the baked crop, or the marker would
        // sit off the edge of the minimap.
        let uv = world_to_uv(0.0, 95.0);
        assert!((0.0..=1.0).contains(&uv.x), "u = {}", uv.x);
        assert!((0.0..=1.0).contains(&uv.y), "v = {}", uv.y);
    }

    #[test]
    fn constants_match_the_bake_viewbox() {
        // These four define the crop in scripts/render_map_texture.py; if they
        // drift, the marker and click math silently desync from the image.
        assert_eq!((VX0, VX1, VY0, VY1), (-600.0, 725.0, -610.0, 830.0));
    }

    #[test]
    fn click_on_a_place_snaps_to_it() {
        let nav = nav();
        let place = &nav.places()[0];
        let [x, z] = nav.node_xz(place.node);
        let target = resolve_teleport_target(&nav, x, z).expect("a place resolves");
        let expected = nav.node_point(place.node);
        assert!((target.x - expected.x).abs() < 1e-6);
        assert!((target.z - expected.z).abs() < 1e-6);
    }

    #[test]
    fn resolved_targets_are_always_walkable() {
        // The core safety guarantee: whatever a click resolves to, it is on the
        // baked walkable surface — never inside a building, never off the graph.
        let nav = nav();
        // World bounds implied by the crop (see world_to_uv): z in [-VX1, -VX0],
        // x in [-VY1, -VY0]. Sweep a grid across the whole city.
        let mut resolved = 0;
        for i in 0..48 {
            for j in 0..48 {
                let x = -830.0 + 1440.0 * (i as f64 / 47.0);
                let z = -725.0 + 1325.0 * (j as f64 / 47.0);
                if let Some(target) = resolve_teleport_target(&nav, x, z) {
                    resolved += 1;
                    assert!(
                        nav.is_walkable(target.x, target.z),
                        "target ({}, {}) from click ({x}, {z}) is not walkable",
                        target.x,
                        target.z
                    );
                    assert!((target.y - WALK_Y).abs() < 1e-6);
                }
            }
        }
        assert!(resolved > 100, "most in-city clicks should resolve, got {resolved}");
    }

    #[test]
    fn click_far_outside_the_city_resolves_to_nothing() {
        let nav = nav();
        assert!(resolve_teleport_target(&nav, 100_000.0, 100_000.0).is_none());
    }
}
