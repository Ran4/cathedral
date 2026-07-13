//! Snapshot-driven smart-actor and offered-item presentation.
//!
//! The engine owns identities, inventory, and offers.  Entities in this
//! module are a disposable visual projection: actor roots and offered-item props
//! are created, updated, or removed only to match the current `WorldMirror`.

use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use crate::{controller::PlayerCamera, fonts::CathedralFonts};

use super::SmartActorRuntime;
use super::model::{ActorControl, ActorId, ItemId, WorldMirror};
use super::targeting::ActorTarget;

const NAME_ANCHOR_Y: f32 = 0.9;
const MAX_NAME_LABEL_DISTANCE_M: f32 = 80.0;
const MAX_VISIBLE_NAME_LABELS: usize = 20;
const THINKING_INDICATOR_WIDTH_PX: f32 = 38.0;
const THINKING_INDICATOR_HEAD_OFFSET_PX: f32 = 68.0;
const THINKING_DOT_STEP_SECONDS: f32 = 0.32;
const SPEECH_ANCHOR_Y: f32 = 1.05;
const OFFER_ANCHOR_Y: f32 = 2.02;
const OFFER_FAN_SPACING_M: f32 = 0.48;
const OFFER_BOB_AMPLITUDE_M: f32 = 0.075;
const OFFER_BOB_RADIANS_PER_SECOND: f32 = 2.1;
const OFFER_TURN_RADIANS_PER_SECOND: f32 = 1.15;

/// Marker for an NPC root projected from the authoritative snapshot.
#[derive(Component, Debug)]
pub struct ActorView;

/// The world-space attachment point for an actor's speech bubble.
///
/// The ID makes the anchor directly discoverable without walking its hierarchy.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct SpeechAnchor(pub ActorId);

/// The attachment point under which all of one giver's offer props are fanned.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OfferAnchor(pub ActorId);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameAnchor(ActorId);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorNameLabel(ActorId);

/// Small animated thought bubble projected above one actor's head.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThinkingIndicator(ActorId);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorOutfit(ActorId);

/// A renderer-only copy of one pending offered item.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct OfferedItemVisual {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub visual_key: String,
    pub created_seq: u64,
    base_translation: Vec3,
    phase: f32,
}

#[derive(Resource)]
pub(crate) struct ActorVisualAssets {
    body_mesh: Handle<Mesh>,
    head_mesh: Handle<Mesh>,
    fish_body_mesh: Handle<Mesh>,
    fish_tail_mesh: Handle<Mesh>,
    coin_mesh: Handle<Mesh>,
    generic_item_mesh: Handle<Mesh>,
    sven_outfit: Handle<StandardMaterial>,
    conny_outfit: Handle<StandardMaterial>,
    ilse_outfit: Handle<StandardMaterial>,
    fallback_outfit: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    fish: Handle<StandardMaterial>,
    fish_fin: Handle<StandardMaterial>,
    copper: Handle<StandardMaterial>,
    generic_item: Handle<StandardMaterial>,
}

impl ActorVisualAssets {
    fn outfit(&self, appearance_key: &str) -> Handle<StandardMaterial> {
        match appearance_key {
            "sven" => self.sven_outfit.clone(),
            "conny" => self.conny_outfit.clone(),
            "ilse" => self.ilse_outfit.clone(),
            _ => self.fallback_outfit.clone(),
        }
    }
}

/// Creates the shared primitive meshes and palette used by all actor views.
pub(crate) fn setup_actor_visual_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let matte = |color: Color| StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.78,
        ..default()
    };

    commands.insert_resource(ActorVisualAssets {
        body_mesh: meshes.add(Capsule3d::new(0.40, 0.82)),
        head_mesh: meshes.add(Sphere::new(0.27).mesh().uv(20, 12)),
        fish_body_mesh: meshes.add(Sphere::new(1.0).mesh().uv(16, 10)),
        fish_tail_mesh: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4)),
        coin_mesh: meshes.add(Cylinder::new(0.20, 0.055)),
        generic_item_mesh: meshes.add(Cuboid::new(0.30, 0.30, 0.30)),
        sven_outfit: materials.add(matte(Color::srgb(0.19, 0.28, 0.36))),
        conny_outfit: materials.add(matte(Color::srgb(0.16, 0.42, 0.49))),
        ilse_outfit: materials.add(matte(Color::srgb(0.50, 0.24, 0.18))),
        fallback_outfit: materials.add(matte(Color::srgb(0.38, 0.34, 0.43))),
        skin: materials.add(matte(Color::srgb(0.73, 0.54, 0.39))),
        fish: materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.61, 0.64),
            metallic: 0.12,
            perceptual_roughness: 0.42,
            ..default()
        }),
        fish_fin: materials.add(matte(Color::srgb(0.20, 0.39, 0.45))),
        copper: materials.add(StandardMaterial {
            base_color: Color::srgb(0.76, 0.34, 0.12),
            metallic: 0.72,
            perceptual_roughness: 0.30,
            ..default()
        }),
        generic_item: materials.add(matte(Color::srgb(0.74, 0.66, 0.24))),
    });
}

/// Reconciles stationary NPC roots, primitive bodies, anchors, and name labels.
///
/// Player-controlled records are represented by the controller's existing root
/// and therefore never get a duplicate primitive body here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reconcile_actor_views(
    mut commands: Commands,
    mirror: Res<WorldMirror>,
    assets: Res<ActorVisualAssets>,
    fonts: Option<Res<CathedralFonts>>,
    mut roots: Query<(Entity, &ActorId, &mut Transform), With<ActorView>>,
    mut labels: Query<(Entity, &ActorNameLabel, &mut Text)>,
    indicators: Query<(Entity, &ThinkingIndicator)>,
    mut outfits: Query<(&ActorOutfit, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let actor_snapshots: Vec<_> = mirror
        .actors()
        .filter(|actor| actor.control == ActorControl::Llm)
        .collect();
    let desired_ids: HashSet<_> = actor_snapshots
        .iter()
        .map(|actor| actor.id.clone())
        .collect();
    let desired_by_id: HashMap<_, _> = actor_snapshots
        .iter()
        .map(|actor| (actor.id.clone(), *actor))
        .collect();

    let name_font = fonts
        .as_deref()
        .map(CathedralFonts::display)
        .unwrap_or_default();
    let mut existing_ids = HashSet::new();
    for (entity, actor_id, mut transform) in &mut roots {
        if let Some(actor) = desired_by_id.get(actor_id) {
            existing_ids.insert(actor_id.clone());
            transform.translation = actor.position_m.into();
            // The render is the only place the player can read the sound
            // witness rule from: if the sim thinks an NPC faces away and the
            // body faces the player, the rule is unlearnable.
            transform.rotation = Quat::from_rotation_y(actor.facing_yaw);
            transform.scale = Vec3::ONE;
        } else {
            // This path is reached only because the authoritative projection no
            // longer contains the actor.
            commands.entity(entity).despawn();
        }
    }

    for actor in actor_snapshots {
        if !existing_ids.contains(&actor.id) {
            spawn_actor(&mut commands, &assets, name_font.clone(), actor);
        }
    }

    for (entity, label, mut text) in &mut labels {
        if let Some(actor) = desired_by_id.get(&label.0) {
            if text.0 != actor.name_for_player {
                text.0.clone_from(&actor.name_for_player);
            }
        } else if !desired_ids.contains(&label.0) {
            commands.entity(entity).despawn();
        }
    }

    for (entity, indicator) in &indicators {
        if !desired_ids.contains(&indicator.0) {
            commands.entity(entity).despawn();
        }
    }

    for (outfit, mut material) in &mut outfits {
        if let Some(actor) = desired_by_id.get(&outfit.0) {
            let desired = assets.outfit(&actor.appearance_key);
            if material.0 != desired {
                material.0 = desired;
            }
        }
    }
}

fn spawn_actor(
    commands: &mut Commands,
    assets: &ActorVisualAssets,
    name_font: FontSource,
    actor: &super::model::ActorSnapshot,
) {
    let actor_id = actor.id.clone();
    commands
        .spawn((
            Name::new(format!("Smart actor: {}", actor.name_for_player)),
            actor_id.clone(),
            ActorView,
            ActorTarget::default(),
            Transform::from_translation(actor.position_m.into())
                .with_rotation(Quat::from_rotation_y(actor.facing_yaw)),
            Visibility::default(),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Actor body"),
                ActorOutfit(actor_id.clone()),
                Mesh3d(assets.body_mesh.clone()),
                MeshMaterial3d(assets.outfit(&actor.appearance_key)),
                Transform::from_xyz(0.0, -0.10, 0.0),
            ));
            root.spawn((
                Name::new("Actor head"),
                Mesh3d(assets.head_mesh.clone()),
                MeshMaterial3d(assets.skin.clone()),
                Transform::from_xyz(0.0, 0.65, 0.0),
            ));
            // The capsule body is rotationally symmetric, so without a face
            // the seeded facing_yaw would be invisible — and the render is
            // how the player learns the sound witness cone.
            root.spawn((
                Name::new("Actor nose"),
                Mesh3d(assets.fish_tail_mesh.clone()),
                MeshMaterial3d(assets.skin.clone()),
                Transform::from_xyz(0.0, 0.65, -0.29)
                    .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                    .with_scale(Vec3::new(0.07, 0.12, 0.07)),
            ));
            root.spawn((
                Name::new("Actor name anchor"),
                NameAnchor(actor_id.clone()),
                Transform::from_xyz(0.0, NAME_ANCHOR_Y, 0.0),
            ));
            root.spawn((
                Name::new("Actor speech anchor"),
                SpeechAnchor(actor_id.clone()),
                Transform::from_xyz(0.0, SPEECH_ANCHOR_Y, 0.0),
            ));
            root.spawn((
                Name::new("Actor offer anchor"),
                OfferAnchor(actor_id.clone()),
                Transform::from_xyz(0.0, OFFER_ANCHOR_Y, 0.0),
                Visibility::default(),
            ));
        });

    // UI text is projected from the world anchor each frame. Unlike Text2d it
    // is rendered by the existing 3D + UI feature set and always faces the eye.
    commands.spawn((
        Name::new(format!("Actor name: {}", actor.name_for_player)),
        ActorNameLabel(actor_id.clone()),
        Text::new(actor.name_for_player.clone()),
        TextFont {
            font: name_font.clone(),
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::justify(Justify::Center),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(108.0),
            padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.025, 0.030, 0.040, 0.78)),
        ZIndex(4),
        Visibility::Hidden,
    ));

    commands.spawn((
        Name::new(format!(
            "Actor thinking indicator: {}",
            actor.name_for_player
        )),
        ThinkingIndicator(actor_id),
        Text::new("..."),
        TextFont {
            font: name_font,
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.10, 0.08, 0.045)),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(THINKING_INDICATOR_WIDTH_PX),
            min_height: Val::Px(26.0),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(13.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.96, 0.90, 0.72, 0.94)),
        ZIndex(5),
        Visibility::Hidden,
    ));
}

/// Projects actor name anchors to screen space, producing billboarded labels.
pub(crate) fn position_actor_name_labels(
    cameras: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    anchors: Query<(&NameAnchor, &GlobalTransform)>,
    mut labels: Query<(&ActorNameLabel, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        for (_, _, mut visibility) in &mut labels {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let anchor_positions: HashMap<_, _> = anchors
        .iter()
        .map(|(anchor, transform)| (anchor.0.clone(), transform.translation()))
        .collect();
    let visible_ids = nearest_name_anchor_ids(camera_transform.translation(), &anchor_positions);

    for (label, mut node, mut visibility) in &mut labels {
        if !visible_ids.contains(&label.0) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Some(world_position) = anchor_positions.get(&label.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(viewport_position) = camera.world_to_viewport(camera_transform, *world_position)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };

        node.left = Val::Px(viewport_position.x - 54.0);
        node.top = Val::Px(viewport_position.y - 34.0);
        *visibility = Visibility::Inherited;
    }
}

/// Shows an animated ellipsis above the actor whose LLM request is in flight.
pub(crate) fn update_thinking_indicators(
    time: Res<Time>,
    runtime: Res<SmartActorRuntime>,
    cameras: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    anchors: Query<(&NameAnchor, &GlobalTransform)>,
    mut indicators: Query<(&ThinkingIndicator, &mut Text, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        for (_, _, _, mut visibility) in &mut indicators {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let anchor_positions: HashMap<_, _> = anchors
        .iter()
        .map(|(anchor, transform)| (anchor.0.clone(), transform.translation()))
        .collect();
    let active_actor = runtime.thinking_actor();
    let maximum_distance_squared = MAX_NAME_LABEL_DISTANCE_M * MAX_NAME_LABEL_DISTANCE_M;
    let dots = thinking_dots(time.elapsed_secs());

    for (indicator, mut text, mut node, mut visibility) in &mut indicators {
        if active_actor != Some(&indicator.0) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Some(world_position) = anchor_positions.get(&indicator.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if camera_transform
            .translation()
            .distance_squared(*world_position)
            > maximum_distance_squared
        {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Ok(viewport_position) = camera.world_to_viewport(camera_transform, *world_position)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if text.0 != dots {
            text.0 = dots.into();
        }
        node.left = Val::Px(viewport_position.x - THINKING_INDICATOR_WIDTH_PX * 0.5);
        node.top = Val::Px(viewport_position.y - THINKING_INDICATOR_HEAD_OFFSET_PX);
        *visibility = Visibility::Inherited;
    }
}

fn thinking_dots(elapsed_seconds: f32) -> &'static str {
    match ((elapsed_seconds / THINKING_DOT_STEP_SECONDS) as u32) % 3 {
        0 => ".",
        1 => "..",
        _ => "...",
    }
}

fn nearest_name_anchor_ids(
    camera_position: Vec3,
    anchor_positions: &HashMap<ActorId, Vec3>,
) -> HashSet<ActorId> {
    let maximum_distance_squared = MAX_NAME_LABEL_DISTANCE_M * MAX_NAME_LABEL_DISTANCE_M;
    let mut nearest: Vec<_> = anchor_positions
        .iter()
        .filter_map(|(actor_id, position)| {
            let distance_squared = camera_position.distance_squared(*position);
            (distance_squared <= maximum_distance_squared).then_some((actor_id, distance_squared))
        })
        .collect();
    nearest.sort_unstable_by(|left, right| {
        left.1.total_cmp(&right.1).then_with(|| left.0.cmp(right.0))
    });
    nearest.truncate(MAX_VISIBLE_NAME_LABELS);
    nearest
        .into_iter()
        .map(|(actor_id, _)| actor_id.clone())
        .collect()
}

#[derive(Debug, Clone)]
struct DesiredOfferVisual {
    item_id: ItemId,
    giver_id: ActorId,
    visual_key: String,
    created_seq: u64,
    base_translation: Vec3,
    phase: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisualDisposition {
    Create,
    Keep,
    Replace,
}

fn visual_disposition(
    current: Option<&OfferedItemVisual>,
    desired: &DesiredOfferVisual,
) -> VisualDisposition {
    match current {
        None => VisualDisposition::Create,
        Some(current)
            if current.giver_id != desired.giver_id || current.visual_key != desired.visual_key =>
        {
            VisualDisposition::Replace
        }
        Some(_) => VisualDisposition::Keep,
    }
}

/// Reconciles one visual copy for every offer in the latest snapshot.
///
/// No command intent calls this system directly, so accepting, declining, or
/// retracting an offer cannot make a prop disappear before the engine confirms it.
pub(crate) fn reconcile_offered_item_views(
    mut commands: Commands,
    mirror: Res<WorldMirror>,
    assets: Res<ActorVisualAssets>,
    anchors: Query<(Entity, &OfferAnchor)>,
    mut visuals: Query<(Entity, &mut OfferedItemVisual)>,
) {
    let desired = desired_offer_visuals(&mirror);
    let desired_ids: HashSet<_> = desired.iter().map(|offer| offer.item_id.clone()).collect();
    let anchor_by_actor: HashMap<_, _> = anchors
        .iter()
        .map(|(entity, anchor)| (anchor.0.clone(), entity))
        .collect();
    let existing_by_item: HashMap<_, _> = visuals
        .iter()
        .map(|(entity, visual)| (visual.item_id.clone(), entity))
        .collect();

    for (entity, visual) in &mut visuals {
        if !desired_ids.contains(&visual.item_id) {
            commands.entity(entity).despawn();
        }
    }

    for offer in desired {
        let current_entity = existing_by_item.get(&offer.item_id).copied();
        let disposition = current_entity
            .and_then(|entity| visuals.get(entity).ok().map(|(_, visual)| visual))
            .map_or(VisualDisposition::Create, |visual| {
                visual_disposition(Some(visual), &offer)
            });

        match disposition {
            VisualDisposition::Keep => {
                let Some(entity) = current_entity else {
                    continue;
                };
                let Ok((_, mut visual)) = visuals.get_mut(entity) else {
                    continue;
                };
                visual.created_seq = offer.created_seq;
                visual.base_translation = offer.base_translation;
                visual.phase = offer.phase;
            }
            VisualDisposition::Replace => {
                if let Some(entity) = current_entity {
                    commands.entity(entity).despawn();
                }
                if let Some(anchor) = anchor_by_actor.get(&offer.giver_id).copied() {
                    spawn_offer_visual(&mut commands, &assets, anchor, offer);
                }
            }
            VisualDisposition::Create => {
                if let Some(anchor) = anchor_by_actor.get(&offer.giver_id).copied() {
                    spawn_offer_visual(&mut commands, &assets, anchor, offer);
                }
            }
        }
    }
}

fn desired_offer_visuals(mirror: &WorldMirror) -> Vec<DesiredOfferVisual> {
    let mut offers: Vec<_> = mirror
        .offers()
        .filter_map(|offer| {
            mirror.item(&offer.item_id).map(|item| {
                (
                    offer.giver_id.clone(),
                    offer.item_id.clone(),
                    item.visual_key.clone(),
                    offer.created_seq,
                )
            })
        })
        .collect();
    offers.sort_by(|left, right| {
        left.0
            .0
            .cmp(&right.0.0)
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.1.0.cmp(&right.1.0))
    });

    let mut desired = Vec::with_capacity(offers.len());
    let mut group_start = 0;
    while group_start < offers.len() {
        let giver = &offers[group_start].0;
        let group_end = offers[group_start..]
            .iter()
            .position(|offer| offer.0 != *giver)
            .map_or(offers.len(), |offset| group_start + offset);
        let count = group_end - group_start;

        for (index, (giver_id, item_id, visual_key, created_seq)) in
            offers[group_start..group_end].iter().cloned().enumerate()
        {
            desired.push(DesiredOfferVisual {
                phase: offer_phase(&item_id),
                item_id,
                giver_id,
                visual_key,
                created_seq,
                base_translation: fan_translation(index, count),
            });
        }
        group_start = group_end;
    }

    desired
}

fn fan_translation(index: usize, count: usize) -> Vec3 {
    let centred_index = index as f32 - (count.saturating_sub(1) as f32 * 0.5);
    let x = centred_index * OFFER_FAN_SPACING_M;
    // A small backwards arc prevents neighbouring silhouettes from perfectly
    // overlapping when the player views the fan obliquely.
    let z = -0.035 * centred_index.abs();
    Vec3::new(x, 0.0, z)
}

fn offer_phase(item_id: &ItemId) -> f32 {
    let hash = item_id.0.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    });
    (hash as f32 / u32::MAX as f32) * 2.0 * PI
}

fn spawn_offer_visual(
    commands: &mut Commands,
    assets: &ActorVisualAssets,
    anchor: Entity,
    desired: DesiredOfferVisual,
) {
    let item_name = format!("Offered item: {}", desired.item_id.0);
    let visual_key = desired.visual_key.clone();
    let base_translation = desired.base_translation;
    let phase = desired.phase;
    let root = commands
        .spawn((
            Name::new(item_name),
            OfferedItemVisual {
                item_id: desired.item_id,
                giver_id: desired.giver_id,
                visual_key: desired.visual_key,
                created_seq: desired.created_seq,
                base_translation,
                phase,
            },
            Transform::from_translation(base_translation),
            Visibility::default(),
            ChildOf(anchor),
        ))
        .id();

    commands
        .entity(root)
        .with_children(|prop| match visual_key.as_str() {
            "fish" => {
                prop.spawn((
                    Name::new("Fish body"),
                    Mesh3d(assets.fish_body_mesh.clone()),
                    MeshMaterial3d(assets.fish.clone()),
                    Transform::from_scale(Vec3::new(0.30, 0.13, 0.12)),
                ));
                prop.spawn((
                    Name::new("Fish tail"),
                    Mesh3d(assets.fish_tail_mesh.clone()),
                    MeshMaterial3d(assets.fish_fin.clone()),
                    Transform::from_xyz(-0.31, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_z(-FRAC_PI_2))
                        .with_scale(Vec3::new(0.13, 0.19, 0.13)),
                ));
            }
            "copper_coin" | "coin" => {
                prop.spawn((
                    Name::new("Coin"),
                    Mesh3d(assets.coin_mesh.clone()),
                    MeshMaterial3d(assets.copper.clone()),
                    Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                ));
            }
            _ => {
                prop.spawn((
                    Name::new("Generic offered item"),
                    Mesh3d(assets.generic_item_mesh.clone()),
                    MeshMaterial3d(assets.generic_item.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(0.28)),
                ));
            }
        });
}

/// Applies gentle deterministic bobbing and turning without changing semantics.
pub(crate) fn animate_offered_items(
    time: Res<Time>,
    mut visuals: Query<(&OfferedItemVisual, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    for (visual, mut transform) in &mut visuals {
        let angle = elapsed * OFFER_BOB_RADIANS_PER_SECOND + visual.phase;
        transform.translation =
            visual.base_translation + Vec3::Y * (OFFER_BOB_AMPLITUDE_M * angle.sin());
        transform.rotation =
            Quat::from_rotation_y(elapsed * OFFER_TURN_RADIANS_PER_SECOND + visual.phase);
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;
    use crate::smart_actors::model::{
        ActorControl, ActorSnapshot, ItemSnapshot, OfferSnapshot, Position, WorldSnapshot,
    };

    fn desired(item: &str, giver: &str, visual_key: &str) -> DesiredOfferVisual {
        DesiredOfferVisual {
            item_id: ItemId(item.into()),
            giver_id: ActorId(giver.into()),
            visual_key: visual_key.into(),
            created_seq: 1,
            base_translation: Vec3::ZERO,
            phase: 0.0,
        }
    }

    fn current(item: &str, giver: &str, visual_key: &str) -> OfferedItemVisual {
        OfferedItemVisual {
            item_id: ItemId(item.into()),
            giver_id: ActorId(giver.into()),
            visual_key: visual_key.into(),
            created_seq: 1,
            base_translation: Vec3::ZERO,
            phase: 0.0,
        }
    }

    #[test]
    fn fan_is_centred_for_one_even_and_odd_counts() {
        assert_eq!(fan_translation(0, 1).x, 0.0);

        let two = [fan_translation(0, 2).x, fan_translation(1, 2).x];
        assert_eq!(two[0], -two[1]);

        let three = [
            fan_translation(0, 3).x,
            fan_translation(1, 3).x,
            fan_translation(2, 3).x,
        ];
        assert_eq!(three[1], 0.0);
        assert_eq!(three[0], -three[2]);
        assert!(three.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn snapshot_create_keep_and_replace_are_distinct() {
        let desired = desired("coin", "ilse", "copper_coin");
        assert_eq!(
            visual_disposition(None, &desired),
            VisualDisposition::Create
        );
        assert_eq!(
            visual_disposition(Some(&current("coin", "ilse", "copper_coin")), &desired),
            VisualDisposition::Keep
        );
        assert_eq!(
            visual_disposition(Some(&current("coin", "sven", "copper_coin")), &desired),
            VisualDisposition::Replace
        );
        assert_eq!(
            visual_disposition(Some(&current("coin", "ilse", "generic")), &desired),
            VisualDisposition::Replace
        );
    }

    #[test]
    fn target_change_does_not_replace_renderer_copy() {
        // Target IDs intentionally do not occur in renderer records. Re-offering
        // to a new target retains the same giver-owned visual copy.
        let desired = desired("fish", "sven", "fish");
        assert_eq!(
            visual_disposition(Some(&current("fish", "sven", "fish")), &desired),
            VisualDisposition::Keep
        );
    }

    #[test]
    fn snapshot_omission_is_the_only_removal_signal() {
        let existing = [ItemId("fish".into()), ItemId("coin".into())];
        let desired: HashSet<_> = [ItemId("coin".into())].into_iter().collect();
        let removed: Vec<_> = existing
            .iter()
            .filter(|item| !desired.contains(*item))
            .cloned()
            .collect();

        assert_eq!(removed, vec![ItemId("fish".into())]);
    }

    #[test]
    fn name_labels_stop_at_eighty_metres() {
        let anchors = HashMap::from([
            (ActorId("near".into()), Vec3::new(0.0, 0.0, 79.0)),
            (ActorId("boundary".into()), Vec3::new(0.0, 0.0, 80.0)),
            (ActorId("far".into()), Vec3::new(0.0, 0.0, 80.01)),
        ]);

        let visible = nearest_name_anchor_ids(Vec3::ZERO, &anchors);

        assert!(visible.contains(&ActorId("near".into())));
        assert!(visible.contains(&ActorId("boundary".into())));
        assert!(!visible.contains(&ActorId("far".into())));
    }

    #[test]
    fn name_labels_are_limited_to_the_nearest_twenty_people() {
        let anchors: HashMap<_, _> = (1..=25)
            .map(|distance| {
                (
                    ActorId(format!("actor-{distance:02}")),
                    Vec3::new(distance as f32, 0.0, 0.0),
                )
            })
            .collect();

        let visible = nearest_name_anchor_ids(Vec3::ZERO, &anchors);

        assert_eq!(visible.len(), 20);
        for distance in 1..=20 {
            assert!(visible.contains(&ActorId(format!("actor-{distance:02}"))));
        }
        for distance in 21..=25 {
            assert!(!visible.contains(&ActorId(format!("actor-{distance:02}"))));
        }
    }

    #[test]
    fn thinking_ellipsis_animates_through_three_steps() {
        assert_eq!(thinking_dots(0.0), ".");
        assert_eq!(thinking_dots(THINKING_DOT_STEP_SECONDS), "..");
        assert_eq!(thinking_dots(THINKING_DOT_STEP_SECONDS * 2.0), "...");
        assert_eq!(thinking_dots(THINKING_DOT_STEP_SECONDS * 3.0), ".");
    }

    #[test]
    fn every_offer_from_one_giver_gets_a_fanned_visual() {
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "player".into(),
                        holds: vec![],
                    },
                    ActorSnapshot {
                        id: ActorId("ilse".into()),
                        name_for_player: "Ilse".into(),
                        control: ActorControl::Llm,
                        position_m: Position::new(1.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "ilse".into(),
                        holds: vec![ItemId("coin".into()), ItemId("fish".into())],
                    },
                ],
                items: vec![
                    ItemSnapshot {
                        id: ItemId("coin".into()),
                        name: "coin".into(),
                        visual_key: "copper_coin".into(),
                    },
                    ItemSnapshot {
                        id: ItemId("fish".into()),
                        name: "fish".into(),
                        visual_key: "fish".into(),
                    },
                ],
                offers: vec![
                    OfferSnapshot {
                        item_id: ItemId("coin".into()),
                        giver_id: ActorId("ilse".into()),
                        target_id: Some(ActorId("player".into())),
                        created_seq: 2,
                    },
                    OfferSnapshot {
                        item_id: ItemId("fish".into()),
                        giver_id: ActorId("ilse".into()),
                        target_id: None,
                        created_seq: 3,
                    },
                ],
            })
            .unwrap();

        let visuals = desired_offer_visuals(&mirror);
        assert_eq!(visuals.len(), 2);
        assert_eq!(
            visuals[0].base_translation.x,
            -visuals[1].base_translation.x
        );
        assert_ne!(visuals[0].item_id, visuals[1].item_id);
    }

    #[test]
    fn stationary_actor_projection_reasserts_authoritative_spawn_positions() {
        let mut mirror = WorldMirror::default();
        let actor = |id: &str, name: &str, position_m: Position| ActorSnapshot {
            id: ActorId(id.into()),
            name_for_player: name.into(),
            control: ActorControl::Llm,
            position_m,
            facing_yaw: 0.0,
            appearance_key: name.to_ascii_lowercase(),
            holds: vec![],
        };
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    actor("cb947", "Conny", Position::new(0.0, 0.91, 112.0).unwrap()),
                    actor("sv3n1", "Sven", Position::new(-1.8, 0.91, 114.0).unwrap()),
                    actor("k0fb1", "Ilse", Position::new(1.8, 0.91, 114.0).unwrap()),
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.91, 95.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "player".into(),
                        holds: vec![],
                    },
                ],
                items: vec![],
                offers: vec![],
            })
            .unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(mirror)
            .add_systems(Startup, setup_actor_visual_assets)
            .add_systems(Update, reconcile_actor_views);
        app.update();

        {
            let world = app.world_mut();
            let indicators: Vec<_> = world
                .query::<(&ThinkingIndicator, &Visibility)>()
                .iter(world)
                .map(|(indicator, visibility)| (indicator.0.clone(), *visibility))
                .collect();
            assert_eq!(indicators.len(), 3);
            assert!(
                indicators
                    .iter()
                    .all(|(_, visibility)| *visibility == Visibility::Hidden)
            );
        }

        {
            let world = app.world_mut();
            let mut actors = world.query_filtered::<(&ActorId, &mut Transform), With<ActorView>>();
            let mut count = 0;
            for (id, mut transform) in actors.iter_mut(world) {
                count += 1;
                if id.0 == "sv3n1" {
                    assert_eq!(transform.translation, Vec3::new(-1.8, 0.91, 114.0));
                    transform.translation = Vec3::splat(999.0);
                }
            }
            assert_eq!(count, 3, "the player must not get a duplicate actor body");
        }

        app.update();
        let world = app.world_mut();
        let mut actors = world.query_filtered::<(&ActorId, &Transform), With<ActorView>>();
        let sven = actors
            .iter(world)
            .find(|(id, _)| id.0 == "sv3n1")
            .expect("Sven actor view should remain present");
        assert_eq!(sven.1.translation, Vec3::new(-1.8, 0.91, 114.0));
    }
}
