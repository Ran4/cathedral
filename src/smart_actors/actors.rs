//! Snapshot-driven smart-actor presentation.
//!
//! The engine owns identities, inventory, and offers.  Entities in this
//! module are a disposable visual projection: actor roots (and their puppet
//! bodies, labels and anchors) are created, updated, or removed only to match
//! the current `WorldMirror`. Carried and offered item props live in
//! `super::hands` (npc_bodies M2).

use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use cathedral_sim::MOVEMENT_TICK_SECONDS;

use crate::{controller::PlayerCamera, fonts::CathedralFonts};

use super::model::{ActorControl, ActorId, MovementInbox, WorldMirror};
use super::targeting::ActorTarget;
use super::{HEARING_RADIUS_M, SmartActorRuntime};

const NAME_ANCHOR_Y: f32 = 0.9;
const MAX_NAME_LABEL_DISTANCE_M: f32 = 80.0;
const FULL_STRANGER_NAME_LABEL_DISTANCE_M: f32 = 8.0;
const MAX_STRANGER_NAME_LABEL_DISTANCE_M: f32 = 15.0;
const NAME_LABEL_BACKGROUND_ALPHA: f32 = 0.78;
const NAME_LABEL_SHADOW_ALPHA: f32 = 0.75;
const MAX_VISIBLE_NAME_LABELS: usize = 20;
const THINKING_INDICATOR_WIDTH_PX: f32 = 38.0;
const THINKING_INDICATOR_HEAD_OFFSET_PX: f32 = 68.0;
const THINKING_DOT_STEP_SECONDS: f32 = 0.32;
pub(super) const SPEECH_ANCHOR_Y: f32 = 1.35;

/// Marker for an NPC root projected from the authoritative snapshot.
#[derive(Component, Debug)]
pub struct ActorView;

/// Per-mover interpolation state for [`drive_npc_bodies`].
///
/// The engine steps movers on a fixed [`MOVEMENT_TICK_SECONDS`] slice and ships
/// each new pose on the hot channel; this carries the previous and current
/// samples so the body sweeps smoothly between them at render rate rather than
/// snapping 20 times a second. `t0` is the host `elapsed_secs_f64()` at which
/// `current` was last set, and `seq` mirrors the inbox sample's so a re-read of
/// the same tick does not restart the sweep. Only movers carry it; it is
/// inserted lazily on their first sample.
#[derive(Component, Debug)]
pub struct NpcMotion {
    previous: Vec3,
    current: Vec3,
    prev_yaw: f32,
    cur_yaw: f32,
    t0: f64,
    seq: u64,
}

/// The world-space attachment point for an actor's speech bubble.
///
/// The ID makes the anchor directly discoverable without walking its hierarchy.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct SpeechAnchor(pub ActorId);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameAnchor(ActorId);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorNameLabel(ActorId);

/// Small animated thought bubble projected above one actor's head.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThinkingIndicator(ActorId);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorOutfit(pub(super) ActorId);

/// Reconciles stationary NPC roots, primitive bodies, anchors, and name labels.
///
/// Player-controlled records are represented by the controller's existing root
/// and therefore never get a duplicate primitive body here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reconcile_actor_views(
    mut commands: Commands,
    mirror: Res<WorldMirror>,
    body_assets: Res<super::body::BodyAssets>,
    fonts: Option<Res<CathedralFonts>>,
    mut roots: Query<(Entity, &ActorId, &mut Transform), With<ActorView>>,
    mut labels: Query<(Entity, &ActorNameLabel, &mut Text)>,
    indicators: Query<(Entity, &ThinkingIndicator)>,
    mut outfits: Query<
        (&ActorOutfit, &mut MeshMaterial3d<StandardMaterial>),
        Without<super::body::ActorFace>,
    >,
    mut faces: Query<
        (&super::body::ActorFace, &mut MeshMaterial3d<StandardMaterial>),
        Without<ActorOutfit>,
    >,
) {
    // Snapshots replace the mirror at most ~10×/s (revision bumps); between
    // them nothing below can produce a different result, and running anyway
    // costs two N-entry maps plus a whole-cast Transform pass per frame.
    if !mirror.is_changed() {
        return;
    }
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
            let translation: Vec3 = actor.position_m.into();
            // The render is the only place the player can read the sound
            // witness rule from: if the sim thinks an NPC faces away and the
            // body faces the player, the rule is unlearnable.
            let rotation = Quat::from_rotation_y(actor.facing_yaw);
            // Compare before writing: a `Mut` deref flags the root changed and
            // re-propagates the whole puppet subtree even for identical values.
            if transform.translation != translation
                || transform.rotation != rotation
                || transform.scale != Vec3::ONE
            {
                transform.translation = translation;
                transform.rotation = rotation;
                transform.scale = Vec3::ONE;
            }
        } else {
            // This path is reached only because the authoritative projection no
            // longer contains the actor.
            commands.entity(entity).despawn();
        }
    }

    for actor in actor_snapshots {
        if !existing_ids.contains(&actor.id) {
            spawn_actor(&mut commands, &body_assets, name_font.clone(), actor);
        }
    }

    for (entity, label, mut text) in &mut labels {
        if let Some(actor) = desired_by_id.get(&label.0) {
            let nameplate_text = actor_nameplate_text(actor);
            if text.0 != nameplate_text {
                text.0 = nameplate_text.to_owned();
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

    // Appearance hot-swap: every cloth part of a body shares the actor's
    // outfit material; the head carries its face. Meshes (headgear, build)
    // are spawn-time — appearances never restructure after creation today.
    for (outfit, mut material) in &mut outfits {
        if let Some(actor) = desired_by_id.get(&outfit.0) {
            let desired = body_assets.outfit_material(&actor.appearance);
            if material.0 != desired {
                material.0 = desired;
            }
        }
    }
    for (face, mut material) in &mut faces {
        if let Some(actor) = desired_by_id.get(&face.0) {
            let desired = body_assets.face_material(&actor.appearance);
            if material.0 != desired {
                material.0 = desired;
            }
        }
    }
}

/// Interpolates the walking NPCs between the engine's 20 Hz movement samples.
///
/// The sim is the authoritative mover: it ships one [`MotionSample`](super::model::MotionSample)
/// per [`MOVEMENT_TICK_SECONDS`] tick on the hot channel ([`MovementInbox`]).
/// Reusing the player controller's global 120 Hz `Time<Fixed>` would fight it,
/// so this does its own two-sample interpolation over the known tick window: as
/// each fresh sample arrives it becomes `current` and the old one `previous`,
/// and the body lerps between them by `t = (now - t0) / tick`, clamped to
/// `[0, 1]`, so `t` sweeps 0→1 across each 50 ms window.
///
/// Runs after `reconcile_actor_views` (same `ReconcileMirror` set), which writes
/// a mover's *stale* snapshot position on every revision bump; this corrects it
/// the same frame. Non-movers never appear in `MovementInbox`, so they are left
/// entirely to reconcile. Children (body parts, labels) ride the root
/// automatically. This only moves and turns the root; `speed`/`gait_phase` are
/// deliberately unread *here* — the walk cycle they drive lives on the part
/// transforms, in `body::animate_body_pose` (npc_bodies M1), which keeps its
/// own two-sample history because this component's fields are private.
pub(crate) fn drive_npc_bodies(
    mut commands: Commands,
    time: Res<Time>,
    inbox: Res<MovementInbox>,
    mut movers: Query<(Entity, &ActorId, &mut Transform, Option<&mut NpcMotion>), With<ActorView>>,
) {
    if inbox.0.is_empty() {
        return;
    }
    let now = time.elapsed_secs_f64();
    for (entity, actor_id, mut transform, motion) in &mut movers {
        let Some(sample) = inbox.0.get(actor_id) else {
            continue;
        };
        match motion {
            // First sample: sweep from where reconcile last placed the body to
            // the sample, so the opening tick reads as a step, not a teleport.
            // Deferred insertion means there is nothing to interpolate this frame;
            // reconcile already owns the transform, so leave it be.
            None => {
                commands.entity(entity).insert(NpcMotion {
                    previous: transform.translation,
                    current: sample.position,
                    prev_yaw: sample.facing_yaw,
                    cur_yaw: sample.facing_yaw,
                    t0: now,
                    seq: sample.seq,
                });
            }
            Some(mut motion) => {
                if sample.seq != motion.seq {
                    motion.previous = motion.current;
                    motion.prev_yaw = motion.cur_yaw;
                    motion.current = sample.position;
                    motion.cur_yaw = sample.facing_yaw;
                    motion.t0 = now;
                    motion.seq = sample.seq;
                }
                let t = ((now - motion.t0) / MOVEMENT_TICK_SECONDS).clamp(0.0, 1.0) as f32;
                let translation = motion.previous.lerp(motion.current, t);
                let rotation = Quat::from_rotation_y(lerp_angle(motion.prev_yaw, motion.cur_yaw, t));
                // An arrived walker keeps its stale sample forever (the sim
                // sends no "stopped" tick); once t clamps to 1.0 the values
                // repeat every frame, so writing them again would keep the
                // whole subtree permanently dirty.
                if transform.translation != translation || transform.rotation != rotation {
                    transform.translation = translation;
                    transform.rotation = rotation;
                }
            }
        }
    }
}

/// Shortest-arc interpolation between two yaw angles (radians), so a mover
/// turning past ±π sweeps the short way round instead of unwinding the long way.
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let mut delta = (to - from) % TAU;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    from + delta * t
}

fn spawn_actor(
    commands: &mut Commands,
    body_assets: &super::body::BodyAssets,
    name_font: FontSource,
    actor: &super::model::ActorSnapshot,
) {
    let actor_id = actor.id.clone();
    let nameplate_text = actor_nameplate_text(actor);
    // M7: dither-fade the crowd out between 120 and 150 m
    // (`features/performance_improvements.md` item 8; see `body::crowd_fade`).
    // The fog already owns depth at that distance, and the name labels
    // self-cull far earlier.
    let fade = super::body::crowd_fade();
    let root = commands
        .spawn((
            Name::new(format!("Smart actor: {}", actor.name_for_player)),
            actor_id.clone(),
            ActorView,
            ActorTarget::default(),
            Transform::from_translation(actor.position_m.into())
                .with_rotation(Quat::from_rotation_y(actor.facing_yaw)),
            Visibility::default(),
        ))
        .id();
    // The articulated puppet (npc_bodies M0). Its asymmetric silhouette and
    // headgear make the seeded facing_yaw readable — the render is how the
    // player learns the sound witness cone.
    let rig = super::body::spawn_body(
        commands,
        root,
        body_assets,
        &actor_id,
        &actor.appearance,
        &fade,
    );
    commands.entity(root).insert(rig).with_children(|root| {
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
    });

    // UI text is projected from the world anchor each frame. Unlike Text2d it
    // is rendered by the existing 3D + UI feature set and always faces the eye.
    commands.spawn((
        Name::new(format!("Actor name: {nameplate_text}")),
        ActorNameLabel(actor_id.clone()),
        Text::new(nameplate_text),
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
        BackgroundColor(Color::srgba(
            0.025,
            0.030,
            0.040,
            NAME_LABEL_BACKGROUND_ALPHA,
        )),
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
    mirror: Res<WorldMirror>,
    cameras: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    anchors: Query<(&NameAnchor, &GlobalTransform)>,
    mut labels: Query<(
        &ActorNameLabel,
        &mut Node,
        &mut Visibility,
        &mut TextColor,
        &mut BackgroundColor,
        &mut TextShadow,
    )>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        for (_, _, mut visibility, _, _, _) in &mut labels {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
        return;
    };
    // One pass, no per-actor allocation: only anchors inside the widest label
    // radius (a couple dozen in a crowd) are collected and sorted. Same
    // policy as `nearest_name_anchor_ids`, which the tests pin.
    let camera_position = camera_transform.translation();
    let mut nearest: Vec<(&ActorId, Vec3, bool, f32)> = Vec::new();
    for (anchor, transform) in &anchors {
        let stranger = mirror
            .actor(&anchor.0)
            .is_some_and(actor_is_stranger_to_player);
        let maximum_distance = if stranger {
            MAX_STRANGER_NAME_LABEL_DISTANCE_M
        } else {
            MAX_NAME_LABEL_DISTANCE_M
        };
        let position = transform.translation();
        let distance_squared = camera_position.distance_squared(position);
        if distance_squared <= maximum_distance * maximum_distance {
            nearest.push((&anchor.0, position, stranger, distance_squared));
        }
    }
    nearest.sort_unstable_by(|left, right| {
        left.3.total_cmp(&right.3).then_with(|| left.0.cmp(right.0))
    });
    nearest.truncate(MAX_VISIBLE_NAME_LABELS);
    let visible: HashMap<&ActorId, (Vec3, bool)> = nearest
        .iter()
        .map(|(actor_id, position, stranger, _)| (*actor_id, (*position, *stranger)))
        .collect();

    for (
        label,
        mut node,
        mut visibility,
        mut text_color,
        mut background_color,
        mut text_shadow,
    ) in &mut labels
    {
        // Every write below is compare-guarded: ~500 resident labels are
        // hidden on any given frame, and re-flagging their UI components
        // would keep taffy busy for nothing.
        let Some((world_position, stranger)) = visible.get(&label.0).copied() else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        let Ok(viewport_position) = camera.world_to_viewport(camera_transform, world_position)
        else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        let left = Val::Px(viewport_position.x - 54.0);
        let top = Val::Px(viewport_position.y - 34.0);
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        let opacity = if stranger {
            stranger_name_label_opacity(camera_position.distance(world_position))
        } else {
            1.0
        };
        let color = Color::linear_rgba(1.0, 1.0, 1.0, opacity);
        if text_color.0 != color {
            text_color.0 = color;
        }
        let background = Color::srgba(
            0.025,
            0.030,
            0.040,
            NAME_LABEL_BACKGROUND_ALPHA * opacity,
        );
        if background_color.0 != background {
            background_color.0 = background;
        }
        let shadow = Color::linear_rgba(0.0, 0.0, 0.0, NAME_LABEL_SHADOW_ALPHA * opacity);
        if text_shadow.color != shadow {
            text_shadow.color = shadow;
        }
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
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
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
        return;
    };
    let active_actor = runtime.thinking_actor();
    // At most one actor thinks at a time; only its anchor is worth resolving
    // (an N-entry map here cloned every actor id every frame).
    let active_position = active_actor.and_then(|actor_id| {
        anchors
            .iter()
            .find(|(anchor, _)| &anchor.0 == actor_id)
            .map(|(_, transform)| transform.translation())
    });
    let dots = thinking_dots(time.elapsed_secs());

    for (indicator, mut text, mut node, mut visibility) in &mut indicators {
        let shown = active_actor == Some(&indicator.0)
            && active_position.is_some_and(|world_position| {
                thinking_indicator_in_range(camera_transform.translation(), world_position)
            });
        if !shown {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        let Some(world_position) = active_position else {
            continue;
        };
        let Ok(viewport_position) = camera.world_to_viewport(camera_transform, world_position)
        else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };

        if text.0 != dots {
            text.0 = dots.into();
        }
        let left = Val::Px(viewport_position.x - THINKING_INDICATOR_WIDTH_PX * 0.5);
        let top = Val::Px(viewport_position.y - THINKING_INDICATOR_HEAD_OFFSET_PX);
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}

fn thinking_indicator_in_range(camera_position: Vec3, actor_position: Vec3) -> bool {
    camera_position.distance_squared(actor_position) <= HEARING_RADIUS_M * HEARING_RADIUS_M
}

fn thinking_dots(elapsed_seconds: f32) -> &'static str {
    match ((elapsed_seconds / THINKING_DOT_STEP_SECONDS) as u32) % 3 {
        0 => ".",
        1 => "..",
        _ => "...",
    }
}

fn stranger_name_label_opacity(distance_m: f32) -> f32 {
    ((MAX_STRANGER_NAME_LABEL_DISTANCE_M - distance_m)
        / (MAX_STRANGER_NAME_LABEL_DISTANCE_M - FULL_STRANGER_NAME_LABEL_DISTANCE_M))
        .clamp(0.0, 1.0)
}

/// Test-only twin of the selection pass inlined in
/// `position_actor_name_labels` (which avoids the per-actor id clones this
/// map-based form needs): the tests pin the policy — per-actor radius, sort
/// by (distance, id), cap at `MAX_VISIBLE_NAME_LABELS`.
#[cfg(test)]
fn nearest_name_anchor_ids(
    camera_position: Vec3,
    anchor_positions: &HashMap<ActorId, Vec3>,
    is_stranger: impl Fn(&ActorId) -> bool,
) -> HashSet<ActorId> {
    let mut nearest: Vec<_> = anchor_positions
        .iter()
        .filter_map(|(actor_id, position)| {
            let maximum_distance = if is_stranger(actor_id) {
                MAX_STRANGER_NAME_LABEL_DISTANCE_M
            } else {
                MAX_NAME_LABEL_DISTANCE_M
            };
            let distance_squared = camera_position.distance_squared(*position);
            (distance_squared <= maximum_distance * maximum_distance)
                .then_some((actor_id, distance_squared))
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

fn actor_is_stranger_to_player(actor: &super::model::ActorSnapshot) -> bool {
    actor
        .name_for_player
        .strip_prefix("a stranger (id ")
        .and_then(|name| name.strip_suffix(')'))
        .is_some_and(|id| id == actor.id.0)
}

fn actor_nameplate_text(actor: &super::model::ActorSnapshot) -> &str {
    if actor_is_stranger_to_player(actor) {
        "A stranger"
    } else {
        &actor.name_for_player
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::camera::visibility::VisibilityRange;

    use super::*;
    use crate::smart_actors::model::{ActorControl, ActorSnapshot, Position, WorldSnapshot};

    #[test]
    fn lerp_angle_takes_the_short_way_round() {
        // A quarter turn is unambiguous.
        assert!((lerp_angle(0.0, FRAC_PI_2, 0.5) - FRAC_PI_2 / 2.0).abs() < 1e-6);
        // Endpoints are exact.
        assert!((lerp_angle(1.0, 2.0, 0.0) - 1.0).abs() < 1e-6);
        assert!((lerp_angle(1.0, 2.0, 1.0) - 2.0).abs() < 1e-6);
        // The seam: -0.9π to +0.9π is a 0.2π step across ±π, not a 1.8π unwind.
        // The short arc at t=0.5 lands just past ±π (equivalently ∓π), never 0.
        let mid = lerp_angle(-0.9 * PI, 0.9 * PI, 0.5);
        assert!(mid.abs() > 0.99 * PI, "took the long way: {mid}");
    }

    #[test]
    fn name_labels_stop_at_eighty_metres() {
        let anchors = HashMap::from([
            (ActorId("near".into()), Vec3::new(0.0, 0.0, 79.0)),
            (ActorId("boundary".into()), Vec3::new(0.0, 0.0, 80.0)),
            (ActorId("far".into()), Vec3::new(0.0, 0.0, 80.01)),
        ]);

        let visible = nearest_name_anchor_ids(Vec3::ZERO, &anchors, |_| false);

        assert!(visible.contains(&ActorId("near".into())));
        assert!(visible.contains(&ActorId("boundary".into())));
        assert!(!visible.contains(&ActorId("far".into())));
    }

    #[test]
    fn stranger_name_labels_stop_after_fifteen_metres() {
        let anchors = HashMap::from([
            (ActorId("known".into()), Vec3::new(0.0, 0.0, 79.0)),
            (ActorId("near".into()), Vec3::new(0.0, 0.0, 14.99)),
            (ActorId("boundary".into()), Vec3::new(0.0, 0.0, 15.0)),
            (ActorId("far".into()), Vec3::new(0.0, 0.0, 15.01)),
        ]);
        let strangers = HashSet::from([
            ActorId("near".into()),
            ActorId("boundary".into()),
            ActorId("far".into()),
        ]);

        let visible = nearest_name_anchor_ids(Vec3::ZERO, &anchors, |id| strangers.contains(id));

        assert!(visible.contains(&ActorId("known".into())));
        assert!(visible.contains(&ActorId("near".into())));
        assert!(visible.contains(&ActorId("boundary".into())));
        assert!(!visible.contains(&ActorId("far".into())));
    }

    #[test]
    fn stranger_name_labels_fade_linearly_from_fifteen_to_eight_metres() {
        assert_eq!(stranger_name_label_opacity(7.0), 1.0);
        assert_eq!(stranger_name_label_opacity(8.0), 1.0);
        assert_eq!(stranger_name_label_opacity(11.5), 0.5);
        assert_eq!(stranger_name_label_opacity(15.0), 0.0);
        assert_eq!(stranger_name_label_opacity(16.0), 0.0);
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

        let visible = nearest_name_anchor_ids(Vec3::ZERO, &anchors, |_| false);

        assert_eq!(visible.len(), 20);
        for distance in 1..=20 {
            assert!(visible.contains(&ActorId(format!("actor-{distance:02}"))));
        }
        for distance in 21..=25 {
            assert!(!visible.contains(&ActorId(format!("actor-{distance:02}"))));
        }
    }

    #[test]
    fn stranger_nameplates_hide_the_actor_id() {
        let actor = ActorSnapshot {
            id: ActorId("pv3k4b".into()),
            name_for_player: "a stranger (id pv3k4b)".into(),
            control: ActorControl::Llm,
            position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: vec![],
            active_gesture: None,
            statuses: Vec::new(),
        };

        assert!(actor_is_stranger_to_player(&actor));
        assert_eq!(actor_nameplate_text(&actor), "A stranger");
    }

    #[test]
    fn thinking_ellipsis_animates_through_three_steps() {
        assert_eq!(thinking_dots(0.0), ".");
        assert_eq!(thinking_dots(THINKING_DOT_STEP_SECONDS), "..");
        assert_eq!(thinking_dots(THINKING_DOT_STEP_SECONDS * 2.0), "...");
        assert_eq!(thinking_dots(THINKING_DOT_STEP_SECONDS * 3.0), ".");
    }

    #[test]
    fn thinking_indicator_uses_the_conversation_radius() {
        assert!(thinking_indicator_in_range(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, HEARING_RADIUS_M)
        ));
        assert!(!thinking_indicator_in_range(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, HEARING_RADIUS_M + 0.01)
        ));
    }

    /// The M0 contract: every LLM actor grows one articulated puppet whose
    /// parts are all fade-carrying shared-handle meshes, `ActorView` stays
    /// root-only, and the whole crowd draws from a bounded material set.
    #[test]
    fn puppet_rig_spawns_bounded_shared_parts() {
        use crate::smart_actors::body::{BodyRig, BodySide, HandAnchor};
        use cathedral_sim::{AppearanceSnapshot, Headgear, OutfitClass};

        let mut mirror = WorldMirror::default();
        let actor = |id: &str, appearance: AppearanceSnapshot| ActorSnapshot {
            id: ActorId(id.into()),
            name_for_player: id.into(),
            control: ActorControl::Llm,
            position_m: Position::new(0.0, 0.91, 112.0).unwrap(),
            facing_yaw: 0.0,
            appearance,
            holds: vec![],
            active_gesture: None,
            statuses: Vec::new(),
        };
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    // Bare-headed default: 11 mesh parts.
                    actor("plain", AppearanceSnapshot::default()),
                    // Helmed watch: 12 parts, iron headgear.
                    actor(
                        "watch",
                        AppearanceSnapshot {
                            outfit: OutfitClass::Watch,
                            headgear: Headgear::KettleHelm,
                            palette_seed: 0xDEAD_BEEF,
                            ..Default::default()
                        },
                    ),
                    // Hooded cleric: 12 parts, cloth headgear.
                    actor(
                        "cleric",
                        AppearanceSnapshot {
                            outfit: OutfitClass::Cleric,
                            headgear: Headgear::Hood,
                            palette_seed: 7,
                            ..Default::default()
                        },
                    ),
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.91, 95.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                    },
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
            })
            .unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(mirror)
            .add_systems(Startup, super::super::body::setup_body_assets)
            .add_systems(Update, reconcile_actor_views);
        app.update();

        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<ActorView>>()
                .iter(world)
                .count(),
            3,
            "ActorView stays root-only — the parts must not carry it"
        );
        let rigs: Vec<_> = world
            .query::<(&ActorId, &BodyRig)>()
            .iter(world)
            .map(|(id, rig)| (id.0.clone(), rig.headgear.is_some()))
            .collect();
        assert_eq!(rigs.len(), 3);
        for (id, has_headgear) in &rigs {
            assert_eq!(*has_headgear, id != "plain", "headgear mismatch for {id}");
        }

        let parts: Vec<_> = world
            .query::<(
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
                Option<&VisibilityRange>,
            )>()
            .iter(world)
            .map(|(mesh, material, fade)| {
                (mesh.0.clone(), material.0.clone(), fade.is_some())
            })
            .collect();
        assert_eq!(parts.len(), 11 + 12 + 12, "13-part budget: 11 + headgear");
        assert!(
            parts.iter().all(|(_, _, fade)| *fade),
            "every mesh part carries the VisibilityRange fade"
        );
        let distinct_meshes: HashSet<_> = parts.iter().map(|(mesh, _, _)| mesh.clone()).collect();
        let distinct_materials: HashSet<_> =
            parts.iter().map(|(_, material, _)| material.clone()).collect();
        assert!(
            distinct_meshes.len() <= 11,
            "meshes are shared handles: {}",
            distinct_meshes.len()
        );
        assert!(
            (4..=9).contains(&distinct_materials.len()),
            "bounded but varied material set: {}",
            distinct_materials.len()
        );

        let hands: Vec<_> = world
            .query::<&HandAnchor>()
            .iter(world)
            .map(|anchor| anchor.side)
            .collect();
        assert_eq!(hands.len(), 6);
        assert_eq!(
            hands.iter().filter(|side| **side == BodySide::Left).count(),
            3
        );
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
            appearance: Default::default(),
            holds: vec![],
            active_gesture: None,
            statuses: Vec::new(),
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
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                    },
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
            })
            .unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(mirror)
            .add_systems(Startup, super::super::body::setup_body_assets)
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

        // The projection is change-gated (2026-07 perf work): between
        // snapshots nothing re-runs it — that is what stops the whole cast
        // being re-flagged every frame — so authority reasserts on the next
        // mirror write, the same trigger a live snapshot arrival produces.
        app.world_mut()
            .resource_mut::<super::super::model::WorldMirror>()
            .set_changed();
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
