//! Developer visualization of the simulation-owned named-area map.
//!
//! The layer never reads `areas.json`. It borrows the exact parsed [`AreaMap`]
//! that is waiting in, or has moved into, [`LocalEngine`]. The gizmos and UI
//! labels are therefore a view of authoritative simulation data rather than a
//! second set of coordinates.

use std::collections::BTreeSet;

use bevy::prelude::*;
use cathedral_sim::{Area, AreaBox, AreaMap, Vec3 as SimVec3};

use crate::{controller::PlayerCamera, fonts::CathedralFonts};

use super::{hud::SmartActorStatusPanel, local_engine::LocalEngine};

const BOX_LABEL_WIDTH_PX: f32 = 280.0;
const BOX_LABEL_Y_OFFSET_PX: f32 = 25.0;
const MAX_VISIBLE_AREAS: usize = 8;
const MAX_VISIBLE_DISTANCE_M: f64 = 350.0;

#[derive(Resource, Debug, Default)]
pub struct AreaDebugState {
    enabled: bool,
    visible_area_ids: BTreeSet<String>,
}

impl AreaDebugState {
    /// Whether the `B` developer layer is on. Read by the sibling
    /// [`super::actor_sheet`] overlay (which shares this one toggle) and by the
    /// map's click-to-teleport, which treats this layer as "debug mode".
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[cfg(test)]
    pub(super) fn visible_area_ids(&self) -> &BTreeSet<String> {
        &self.visible_area_ids
    }
}

#[derive(Component, Debug)]
pub(super) struct AreaBoxLabel {
    area_id: String,
    anchor_m: Vec3,
}

#[derive(Component, Debug)]
pub(super) struct PlayerAreaDescription;

type BoxLabelQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static AreaBoxLabel,
        &'static mut Node,
        &'static mut Visibility,
    ),
    Without<PlayerAreaDescription>,
>;
type PlayerLabelQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static mut Visibility),
    (With<PlayerAreaDescription>, Without<AreaBoxLabel>),
>;

/// Build the static UI labels from the already parsed map. Their anchors and
/// text are presentation caches only; every wire box and every location lookup
/// still reads the map itself each frame.
pub(super) fn spawn_area_debug_ui(
    mut commands: Commands,
    engine: NonSend<LocalEngine>,
    fonts: Option<Res<CathedralFonts>>,
) {
    let body_font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();
    let display_font = fonts
        .as_deref()
        .map(CathedralFonts::display)
        .unwrap_or_default();

    commands.spawn((
        Name::new("Area debug: player location"),
        PlayerAreaDescription,
        Text::new("AREA DEBUG"),
        TextFont {
            font: display_font,
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            top: px(108),
            left: px(18),
            max_width: px(660),
            padding: UiRect::axes(px(13), px(8)),
            border_radius: BorderRadius::all(px(7)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.030, 0.040, 0.88)),
        ZIndex(20),
        Visibility::Hidden,
    ));

    let Some(map) = engine.area_map() else {
        return;
    };
    for (area_index, area) in map.areas.iter().enumerate() {
        for (box_index, bounds) in area.boxes.iter().enumerate() {
            commands.spawn((
                Name::new(format!("Area debug: {} box {box_index}", area.id)),
                AreaBoxLabel {
                    area_id: area.id.clone(),
                    anchor_m: box_label_anchor(bounds),
                },
                Text::new(box_label_text(area, box_index)),
                TextFont {
                    font: body_font.clone(),
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(area_color(area_index)),
                TextLayout::justify(Justify::Center),
                TextShadow::default(),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(BOX_LABEL_WIDTH_PX),
                    padding: UiRect::axes(px(5), px(3)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.018, 0.025, 0.80)),
                ZIndex(19),
                Visibility::Hidden,
            ));
        }
    }
}

/// Toggle the layer, update the live player description, and project each box
/// label's world anchor into UI space.
pub(super) fn update_area_debug_ui(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<AreaDebugState>,
    engine: NonSend<LocalEngine>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    cameras: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    mut box_labels: BoxLabelQuery,
    mut player_label: PlayerLabelQuery,
) {
    if keyboard.just_pressed(KeyCode::KeyB) {
        state.enabled = !state.enabled;
    }

    let Ok((mut player_text, mut player_visibility)) = player_label.single_mut() else {
        return;
    };
    // Every write below goes through a compare. The layer is off almost all of
    // the time, and re-flagging `Visibility` on the 90-odd box labels each
    // frame re-ran their visibility propagation and re-queued them for UI
    // extraction for nothing — the same defect the name labels had before the
    // 2026-07 pass.
    if !state.enabled {
        state.visible_area_ids.clear();
        player_visibility.set_if_neq(Visibility::Hidden);
        hide_box_labels(&mut box_labels);
        return;
    }
    player_visibility.set_if_neq(Visibility::Inherited);

    let Some(map) = engine.area_map() else {
        state.visible_area_ids.clear();
        set_text(&mut player_text, "AREA DEBUG  ·  area map unavailable");
        hide_box_labels(&mut box_labels);
        return;
    };
    let Ok(player) = players.single() else {
        state.visible_area_ids.clear();
        set_text(&mut player_text, "AREA DEBUG  ·  player transform unavailable");
        hide_box_labels(&mut box_labels);
        return;
    };
    let position = player.translation();
    let sim_position = SimVec3::new(
        f64::from(position.x),
        f64::from(position.y),
        f64::from(position.z),
    );
    state.visible_area_ids = visible_area_ids(map, sim_position);
    let description = map
        .location_description(sim_position)
        .unwrap_or_else(|| "No areas are defined".to_string());
    set_text(
        &mut player_text,
        &format!("AREA DEBUG  ·  Player: {description}"),
    );

    let Ok((camera, camera_transform)) = cameras.single() else {
        hide_box_labels(&mut box_labels);
        return;
    };
    let Some(viewport_size) = camera.logical_viewport_size() else {
        hide_box_labels(&mut box_labels);
        return;
    };
    for (label, mut node, mut visibility) in &mut box_labels {
        if !state.visible_area_ids.contains(&label.area_id) {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        let Ok(viewport) = camera.world_to_viewport(camera_transform, label.anchor_m) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if viewport.x < 0.0
            || viewport.y < 0.0
            || viewport.x > viewport_size.x
            || viewport.y > viewport_size.y
        {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        let left = px(viewport.x - BOX_LABEL_WIDTH_PX * 0.5);
        let top = px(viewport.y - BOX_LABEL_Y_OFFSET_PX);
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        visibility.set_if_neq(Visibility::Inherited);
    }
}

/// Writes a label only when the text actually differs: an unconditional write
/// re-runs the text measure and re-shapes the glyphs every frame.
fn set_text(text: &mut Mut<Text>, value: &str) {
    if text.0 != value {
        text.0 = value.to_string();
    }
}

/// Keep developer-only actor connection diagnostics with the area debug layer.
pub(super) fn update_actor_status_visibility(
    state: Res<AreaDebugState>,
    mut status_panel: Query<&mut Visibility, With<SmartActorStatusPanel>>,
) {
    let wanted = if state.enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut status_panel {
        visibility.set_if_neq(wanted);
    }
}

/// Draw all twelve edges of each selected area box. Multiple boxes in one
/// logical area share a color; their projected labels carry the distinguishing
/// box index.
pub(super) fn draw_area_boxes(
    state: Res<AreaDebugState>,
    engine: NonSend<LocalEngine>,
    mut gizmos: Gizmos,
) {
    if !state.enabled {
        return;
    }
    let Some(map) = engine.area_map() else {
        return;
    };
    draw_map_boxes(map, &state.visible_area_ids, &mut gizmos);
}

fn draw_map_boxes(map: &AreaMap, visible_area_ids: &BTreeSet<String>, gizmos: &mut Gizmos) {
    for (area_index, area) in map.areas.iter().enumerate() {
        if !visible_area_ids.contains(&area.id) {
            continue;
        }
        let color = area_color(area_index);
        for bounds in &area.boxes {
            gizmos.cube(box_transform(bounds), color);
        }
    }
}

fn visible_area_ids(map: &AreaMap, position: SimVec3) -> BTreeSet<String> {
    map.nearest_areas(position, MAX_VISIBLE_DISTANCE_M, MAX_VISIBLE_AREAS)
        .into_iter()
        .map(|nearest| nearest.area.id.clone())
        .collect()
}

fn hide_box_labels(labels: &mut BoxLabelQuery) {
    for (_, _, mut visibility) in labels {
        visibility.set_if_neq(Visibility::Hidden);
    }
}

fn box_transform(bounds: &AreaBox) -> Transform {
    let min = sim_to_render(bounds.min_m);
    let max = sim_to_render(bounds.max_m);
    Transform::from_translation((min + max) * 0.5).with_scale(max - min)
}

fn box_label_anchor(bounds: &AreaBox) -> Vec3 {
    let min = sim_to_render(bounds.min_m);
    let max = sim_to_render(bounds.max_m);
    // Keep labels at human-readable height even for cathedral-height boxes.
    Vec3::new((min.x + max.x) * 0.5, min.y + 1.5, (min.z + max.z) * 0.5)
}

fn sim_to_render(position: SimVec3) -> Vec3 {
    Vec3::new(position.x as f32, position.y as f32, position.z as f32)
}

fn box_label_text(area: &Area, box_index: usize) -> String {
    format!("{}\n{} [box {box_index}]", area.label, area.id)
}

fn area_color(area_index: usize) -> Color {
    match area_index % 6 {
        0 => Color::srgb(0.20, 0.95, 0.98),
        1 => Color::srgb(1.00, 0.78, 0.20),
        2 => Color::srgb(0.98, 0.34, 0.30),
        3 => Color::srgb(0.52, 0.98, 0.40),
        4 => Color::srgb(0.78, 0.46, 1.00),
        _ => Color::srgb(1.00, 0.48, 0.78),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_transform_uses_the_authoritative_min_and_max() {
        let bounds = AreaBox {
            min_m: SimVec3::new(-4.0, -1.0, 10.0),
            max_m: SimVec3::new(8.0, 5.0, 30.0),
        };
        let transform = box_transform(&bounds);
        assert_eq!(transform.translation, Vec3::new(2.0, 2.0, 20.0));
        assert_eq!(transform.scale, Vec3::new(12.0, 6.0, 20.0));
        assert_eq!(box_label_anchor(&bounds), Vec3::new(2.0, 0.5, 20.0));
    }

    #[test]
    fn labels_show_the_exact_label_id_and_box_index() {
        let area = Area {
            id: "lanthorn_interior".to_string(),
            label: "Inside the Lanthorn".to_string(),
            boxes: Vec::new(),
        };
        assert_eq!(
            box_label_text(&area, 2),
            "Inside the Lanthorn\nlanthorn_interior [box 2]"
        );
    }
}
