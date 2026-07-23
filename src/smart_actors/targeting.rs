//! Gaze targeting for stationary smart actors.
//!
//! Actor bodies deliberately do not participate in the character controller's
//! collision world.  This module ray-tests the small set of semantic actor
//! volumes and asks the collision world only for the nearest static obstruction.

use bevy::prelude::*;

use crate::controller::{CollisionWorld, PlayerCamera, PlayerController};

use super::model::ActorId;

/// Maximum body-centre distance at which an actor can be identified under the
/// crosshair. This visual focus has no effect on who hears microphone speech.
pub const ACTOR_FOCUS_RADIUS_M: f32 = 20.0;

/// Maximum body-centre distance at which items can be exchanged.
pub const ITEM_FOCUS_RADIUS_M: f32 = 4.0;

const RAY_EPSILON: f32 = 1.0e-6;

/// A local-space, axis-aligned interaction volume attached to an actor root.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ActorTarget {
    pub center: Vec3,
    pub half_extents: Vec3,
}

impl ActorTarget {
    pub const fn new(center: Vec3, half_extents: Vec3) -> Self {
        Self {
            center,
            half_extents,
        }
    }
}

impl Default for ActorTarget {
    fn default() -> Self {
        Self::new(Vec3::ZERO, Vec3::new(0.52, 0.93, 0.52))
    }
}

/// One unobstructed actor hit under the centre of the camera.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusedActor {
    pub actor_id: ActorId,
    pub entity: Entity,
    /// Distance along the centre-camera ray to the target volume.
    pub ray_distance_m: f32,
    /// Full three-dimensional distance between body centres.
    pub body_distance_m: f32,
}

/// Current gaze targets for visual context and item interactions.
///
/// The two values are intentionally independent. An actor can remain identified
/// between four and twenty metres while item interaction is disabled.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct ActorFocus {
    pub actor: Option<FocusedActor>,
    pub item: Option<FocusedActor>,
}

#[derive(Debug, Clone, Copy)]
struct TargetCandidate {
    center: Vec3,
    half_extents: Vec3,
    body_center: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TargetHit {
    index: usize,
    ray_distance: f32,
    body_distance: f32,
}

/// Updates both focus bands from the unique player camera.
///
/// Run this after transform propagation and before collecting interaction input.
pub fn update_actor_focus(
    mut focus: ResMut<ActorFocus>,
    cameras: Query<&GlobalTransform, With<PlayerCamera>>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    actors: Query<(Entity, &ActorId, &ActorTarget, &GlobalTransform)>,
    collision_world: Res<CollisionWorld>,
) {
    let (Ok(camera), Ok(player)) = (cameras.single(), players.single()) else {
        *focus = ActorFocus::default();
        return;
    };

    let origin = camera.translation();
    let body_origin = player.translation();
    let direction = *camera.forward();
    let wall_distance = collision_world.nearest_ray_hit(origin, direction, ACTOR_FOCUS_RADIUS_M);

    // Focus can only land within ACTOR_FOCUS_RADIUS_M of the player's body;
    // pre-cull the whole-cast query on a cheap squared distance (with a small
    // margin for actor extents) so the sort below handles a handful of
    // records, not all ~510.
    let cull_radius = ACTOR_FOCUS_RADIUS_M + 2.0;
    let mut actor_records: Vec<_> = actors
        .iter()
        .filter(|(_, _, _, transform)| {
            transform.translation().distance_squared(body_origin) <= cull_radius * cull_radius
        })
        .map(|(entity, actor_id, target, transform)| {
            let (center, half_extents) = world_aabb(*target, transform);
            (
                entity,
                actor_id,
                TargetCandidate {
                    center,
                    half_extents,
                    body_center: transform.translation(),
                },
            )
        })
        .collect();
    actor_records.sort_by(|left, right| left.1.0.cmp(&right.1.0));
    let candidates: Vec<_> = actor_records
        .iter()
        .map(|(_, _, candidate)| *candidate)
        .collect();

    focus.actor = nearest_visible_target(
        origin,
        direction,
        body_origin,
        ACTOR_FOCUS_RADIUS_M,
        wall_distance,
        &candidates,
    )
    .map(|hit| focused_actor(&actor_records, hit));

    // A wall farther than four metres cannot affect item focus. Keeping the
    // already-computed distance avoids a second traversal of the static boxes.
    focus.item = nearest_visible_target(
        origin,
        direction,
        body_origin,
        ITEM_FOCUS_RADIUS_M,
        wall_distance,
        &candidates,
    )
    .map(|hit| focused_actor(&actor_records, hit));
}

fn focused_actor(records: &[(Entity, &ActorId, TargetCandidate)], hit: TargetHit) -> FocusedActor {
    let (entity, actor_id, _) = records[hit.index];
    FocusedActor {
        actor_id: actor_id.clone(),
        entity,
        ray_distance_m: hit.ray_distance,
        body_distance_m: hit.body_distance,
    }
}

fn world_aabb(target: ActorTarget, transform: &GlobalTransform) -> (Vec3, Vec3) {
    let affine = transform.affine();
    let matrix = affine.matrix3;
    let local_half = target.half_extents.abs();
    let world_half = matrix.x_axis.abs() * local_half.x
        + matrix.y_axis.abs() * local_half.y
        + matrix.z_axis.abs() * local_half.z;

    (affine.transform_point3(target.center), world_half.into())
}

fn nearest_visible_target(
    origin: Vec3,
    direction: Vec3,
    body_origin: Vec3,
    max_body_distance: f32,
    wall_distance: Option<f32>,
    candidates: &[TargetCandidate],
) -> Option<TargetHit> {
    if !origin.is_finite()
        || !direction.is_finite()
        || !body_origin.is_finite()
        || !max_body_distance.is_finite()
        || max_body_distance < 0.0
    {
        return None;
    }

    let direction = direction.try_normalize()?;
    let max_distance_squared = max_body_distance * max_body_distance;

    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            if !candidate.center.is_finite()
                || !candidate.half_extents.is_finite()
                || !candidate.body_center.is_finite()
                || candidate.half_extents.cmplt(Vec3::ZERO).any()
            {
                return None;
            }

            let body_distance_squared = body_origin.distance_squared(candidate.body_center);
            if body_distance_squared > max_distance_squared {
                return None;
            }

            let ray_distance =
                ray_aabb_distance(origin, direction, candidate.center, candidate.half_extents)?;
            if wall_distance.is_some_and(|wall| wall <= ray_distance) {
                return None;
            }

            Some(TargetHit {
                index,
                ray_distance,
                body_distance: body_distance_squared.sqrt(),
            })
        })
        .min_by(|left, right| {
            left.ray_distance
                .total_cmp(&right.ray_distance)
                .then_with(|| left.index.cmp(&right.index))
        })
}

/// Returns the entry distance for a ray and axis-aligned box.
fn ray_aabb_distance(
    origin: Vec3,
    direction: Vec3,
    center: Vec3,
    half_extents: Vec3,
) -> Option<f32> {
    if !origin.is_finite()
        || !direction.is_finite()
        || !center.is_finite()
        || !half_extents.is_finite()
        || half_extents.cmplt(Vec3::ZERO).any()
    {
        return None;
    }

    let min = center - half_extents;
    let max = center + half_extents;
    let mut entry: f32 = 0.0;
    let mut exit = f32::INFINITY;

    for axis in 0..3 {
        let origin_axis = origin[axis];
        let direction_axis = direction[axis];
        if direction_axis.abs() <= RAY_EPSILON {
            if origin_axis < min[axis] || origin_axis > max[axis] {
                return None;
            }
            continue;
        }

        let inverse = direction_axis.recip();
        let mut near = (min[axis] - origin_axis) * inverse;
        let mut far = (max[axis] - origin_axis) * inverse;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        entry = entry.max(near);
        exit = exit.min(far);
        if entry > exit {
            return None;
        }
    }

    (exit >= 0.0).then_some(entry.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(z: f32) -> TargetCandidate {
        TargetCandidate {
            center: Vec3::new(0.0, 0.0, z),
            half_extents: Vec3::splat(0.5),
            body_center: Vec3::new(0.0, 0.0, z),
        }
    }

    #[test]
    fn ray_aabb_returns_entry_distance_and_rejects_misses() {
        assert_eq!(
            ray_aabb_distance(Vec3::ZERO, Vec3::Z, Vec3::Z * 5.0, Vec3::splat(0.5)),
            Some(4.5)
        );
        assert_eq!(
            ray_aabb_distance(Vec3::X * 2.0, Vec3::Z, Vec3::Z * 5.0, Vec3::splat(0.5)),
            None
        );
    }

    #[test]
    fn nearest_actor_volume_wins_independent_of_input_order() {
        let targets = [target(8.0), target(3.0), target(12.0)];
        let hit = nearest_visible_target(Vec3::ZERO, Vec3::Z, Vec3::ZERO, 20.0, None, &targets)
            .expect("an actor should be hit");

        assert_eq!(hit.index, 1);
        assert_eq!(hit.ray_distance, 2.5);
    }

    #[test]
    fn static_wall_blocks_actor_but_not_actor_in_front_of_it() {
        let targets = [target(8.0), target(3.0)];
        let hit =
            nearest_visible_target(Vec3::ZERO, Vec3::Z, Vec3::ZERO, 20.0, Some(5.0), &targets)
                .expect("near actor should remain visible");
        assert_eq!(hit.index, 1);

        assert_eq!(
            nearest_visible_target(Vec3::ZERO, Vec3::Z, Vec3::ZERO, 20.0, Some(2.0), &targets,),
            None
        );
    }

    #[test]
    fn body_distance_cutoffs_are_inclusive() {
        let actor_edge = target(ACTOR_FOCUS_RADIUS_M);
        let item_edge = target(ITEM_FOCUS_RADIUS_M);

        assert!(
            nearest_visible_target(
                Vec3::ZERO,
                Vec3::Z,
                Vec3::ZERO,
                ACTOR_FOCUS_RADIUS_M,
                None,
                &[actor_edge],
            )
            .is_some()
        );
        assert!(
            nearest_visible_target(
                Vec3::ZERO,
                Vec3::Z,
                Vec3::ZERO,
                ITEM_FOCUS_RADIUS_M,
                None,
                &[item_edge],
            )
            .is_some()
        );
        assert!(
            nearest_visible_target(
                Vec3::ZERO,
                Vec3::Z,
                Vec3::ZERO,
                ITEM_FOCUS_RADIUS_M,
                None,
                &[target(ITEM_FOCUS_RADIUS_M + 0.001)],
            )
            .is_none()
        );
    }

    #[test]
    fn origin_inside_target_hits_at_zero() {
        let hit =
            nearest_visible_target(Vec3::ZERO, Vec3::Z, Vec3::ZERO, 4.0, None, &[target(0.0)])
                .expect("origin starts inside target");
        assert_eq!(hit.ray_distance, 0.0);
    }

    #[test]
    fn camera_ray_and_body_proximity_use_distinct_origins() {
        let candidate = TargetCandidate {
            center: Vec3::new(0.0, 1.5, 5.0),
            half_extents: Vec3::new(0.5, 1.0, 0.5),
            body_center: Vec3::new(0.0, 0.0, 5.0),
        };
        let camera_origin = Vec3::new(0.0, 1.5, 0.0);

        assert!(
            nearest_visible_target(camera_origin, Vec3::Z, Vec3::ZERO, 5.0, None, &[candidate],)
                .is_some()
        );
        assert!(
            nearest_visible_target(
                camera_origin,
                Vec3::Z,
                Vec3::new(0.0, 0.0, -0.01),
                5.0,
                None,
                &[candidate],
            )
            .is_none()
        );
    }
}
