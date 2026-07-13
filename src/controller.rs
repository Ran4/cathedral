//! A small, dependency-free first-person character controller.
//!
//! The cathedral only needs coarse, static collision.  Treating those colliders
//! as boxes keeps the controller deterministic and makes the movement code easy
//! to test without starting Bevy's renderer.
//!
//! AGENT: please keep this keyboard map up to date whenever you change anything:
//!    ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
//!    │   │   │   │   │   │   │   │   │   │   │ 0 │   ` = screenshot, 1-9 = inventory
//!    └─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┘
//!      │ Q │   │ E │   │ T │   │ U │ I │ O │ P │     W=fwd  R=retract  Y=accept
//!      └─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┘
//!        │   │   │   │   │ G │ H │ J │ K │ L │       A/S/D=move  F=fart  '=fly (ä on sv-SE)
//!        └─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴───┴───┘
//!          │   │   │ C │   │ B │   │ M │             Z=STT  X=TTS  V=mic  N=decline
//!          └───┴───┴───┴───┴───┴───┴───┘


use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{
    audio::SpatialListener,
    camera::Exposure,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    light::AtmosphereEnvironmentMapLight,
    pbr::AtmosphereSettings,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::smart_actors::ConfigMenuState;
use crate::smart_actors::model::ActorId;

const FIXED_HZ: f64 = 120.0;
// Begin on the open west approach, facing the actor cluster. At 17–19 m the
// whole initial cast is inside the settled 20 m hearing radius immediately.
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.91, 95.0);
const PLAYER_START_YAW: f32 = PI;
const PLAYER_HALF_SIZE: Vec3 = Vec3::new(0.35, 0.9, 0.35);
const EYE_OFFSET: f32 = 0.65;

const WALK_SPEED: f32 = 8.0;
const RUN_SPEED: f32 = 12.0;
const MAX_HORIZONTAL_SPEED: f32 = RUN_SPEED;
const GROUND_ACCELERATION: f32 = 12.0;
const AIR_ACCELERATION: f32 = 2.0;
const GROUND_FRICTION: f32 = 8.0;
const GRAVITY: f32 = 22.0;
const JUMP_SPEED: f32 = 7.0;
const JUMP_BUFFER_SECONDS: f32 = 0.12;
const COYOTE_SECONDS: f32 = 0.10;

const FLY_SPEED: f32 = 8.0;
const FLY_ACCELERATION: f32 = 12.0;
const FLY_FRICTION: f32 = 6.0;
const MAX_FLY_SPEED: f32 = 11.0;

const MOUSE_SENSITIVITY: f32 = 0.0018;
const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;

// The player is swept against boxes expanded by its half-size.  Keeping a tiny
// gap between the player and geometry prevents floating-point noise from
// repeatedly pushing the player in and out of a surface.
const COLLISION_SKIN: f32 = 0.002;
const SWEEP_EPSILON: f32 = 1.0e-6;
const MAX_SLIDE_PLANES: usize = 5;
const MAX_DEPENETRATION_STEPS: usize = 8;

/// Adds first-person input, fixed-step movement, and collision to an app.
pub struct ControllerPlugin;

impl Plugin for ControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CollisionWorld>()
            .init_resource::<ControllerInput>()
            .insert_resource(Time::<Fixed>::from_hz(FIXED_HZ))
            .add_systems(Startup, (spawn_player, initially_capture_cursor))
            .add_systems(FixedUpdate, fixed_player_movement)
            .add_systems(
                RunFixedMainLoop,
                (
                    (collect_input, mouse_look)
                        .chain()
                        .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                    interpolate_player.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
                ),
            );
    }
}

/// Runtime state exposed for the HUD.
#[derive(Component, Debug)]
pub struct PlayerController {
    /// Whether gravity-free flight is active.
    pub flying: bool,
    velocity: Vec3,
    yaw: f32,
    pitch: f32,
    grounded: bool,
    coyote_remaining: f32,
    jump_buffer_remaining: f32,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            flying: false,
            velocity: Vec3::ZERO,
            yaw: PLAYER_START_YAW,
            pitch: 0.0,
            grounded: false,
            coyote_remaining: 0.0,
            jump_buffer_remaining: 0.0,
        }
    }
}

impl PlayerController {
    /// Current compass yaw in radians (yaw 0 faces -Z). The smart-actor
    /// bridge reports it with spatial updates for the sound witness test.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
}

/// Static collision boxes used by the character controller.
///
/// Boxes are world-space and axis-aligned.  Scene geometry may be as detailed
/// as desired; register simple boxes for floors, walls, stairs, and other solid
/// navigation surfaces.
#[derive(Resource, Debug, Default)]
pub struct CollisionWorld {
    boxes: Vec<SolidBox>,
}

impl CollisionWorld {
    /// Registers a world-space box. Reversed corners are accepted and fixed up.
    /// Empty or non-finite boxes are ignored.
    pub fn add_box(&mut self, min: Vec3, max: Vec3) {
        if !min.is_finite() || !max.is_finite() {
            return;
        }

        let lower = min.min(max);
        let upper = min.max(max);
        if upper.x <= lower.x || upper.y <= lower.y || upper.z <= lower.z {
            return;
        }

        self.boxes.push(SolidBox {
            min: lower,
            max: upper,
        });
    }

    /// Returns the distance to the first static collision box hit by a ray.
    ///
    /// Smart-actor gaze targeting uses the same deliberately coarse geometry
    /// as player movement. This keeps walls authoritative for interaction
    /// without ray-testing the city's rendered meshes.
    pub fn nearest_ray_hit(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<f32> {
        if !origin.is_finite()
            || !direction.is_finite()
            || !max_distance.is_finite()
            || max_distance <= 0.0
        {
            return None;
        }

        let direction = direction.normalize_or_zero();
        if direction == Vec3::ZERO {
            return None;
        }

        let displacement = direction * max_distance;
        self.boxes
            .iter()
            .filter_map(|solid| {
                sweep_point_box(origin, displacement, solid.min, solid.max)
                    .map(|hit| hit.time * max_distance)
            })
            .min_by(f32::total_cmp)
    }

    /// Number of coarse static colliders registered by the scene.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Whether the scene has registered any collision geometry yet.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
struct SolidBox {
    min: Vec3,
    max: Vec3,
}

#[derive(Component, Debug)]
struct PhysicalPosition {
    previous: Vec3,
    current: Vec3,
}

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Resource, Debug, Default)]
struct ControllerInput {
    movement: Vec2,
    running: bool,
    fly_vertical: f32,
}

fn spawn_player(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Player"),
            ActorId("player".into()),
            PlayerController::default(),
            PhysicalPosition {
                previous: PLAYER_SPAWN,
                current: PLAYER_SPAWN,
            },
            Transform::from_translation(PLAYER_SPAWN)
                .with_rotation(Quat::from_rotation_y(PLAYER_START_YAW)),
            Visibility::default(),
        ))
        .with_children(|player| {
            player.spawn((
                Name::new("Player camera"),
                PlayerCamera,
                SpatialListener::new(0.18),
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    near: 0.05,
                    far: 2_500.0,
                    fov: 70.0_f32.to_radians(),
                    ..default()
                }),
                AtmosphereSettings {
                    // Concentrate the aerial-perspective lookup precision over
                    // the kilometre-wide city rather than the 32 km default.
                    aerial_view_lut_max_distance: 2_500.0,
                    ..default()
                },
                AtmosphereEnvironmentMapLight {
                    intensity: 0.65,
                    size: UVec2::splat(128),
                    ..default()
                },
                Exposure { ev100: 12.8 },
                Tonemapping::AcesFitted,
                Transform::from_xyz(0.0, EYE_OFFSET, 0.0),
            ));
        });
}

fn initially_capture_cursor(mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

/// Samples edge-triggered input once per rendered frame, before fixed physics
/// may run zero or several times. In particular, the fly toggle must not be
/// sampled from `FixedUpdate`, where one key press could toggle repeatedly.
///
/// `KeyCode::Quote` is a *physical* key position (Bevy/winit use US-layout
/// positions); on the sv-SE layout that same physical key is `ä`. Chosen
/// deliberately when F became the fart key — do not "fix" it to a logical key.
fn collect_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    menu: Res<ConfigMenuState>,
    mut input: ResMut<ControllerInput>,
    mut controller: Single<&mut PlayerController>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keyboard.just_pressed(KeyCode::Quote) {
        controller.flying = !controller.flying;
        controller.velocity.y = 0.0;
        controller.grounded = false;
        controller.coyote_remaining = 0.0;
        controller.jump_buffer_remaining = 0.0;
    }

    // Escape belongs to the settings menu, which releases and recaptures the
    // cursor as it opens and closes. Clicking only recaptures outside it.
    if !menu.open && mouse_buttons.just_pressed(MouseButton::Left) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }

    let right = axis(
        keyboard.pressed(KeyCode::KeyD),
        keyboard.pressed(KeyCode::KeyA),
    );
    let forward = axis(
        keyboard.pressed(KeyCode::KeyW),
        keyboard.pressed(KeyCode::KeyS),
    );
    input.movement = Vec2::new(right, forward).clamp_length_max(1.0);
    input.running = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    input.fly_vertical = axis(
        keyboard.pressed(KeyCode::Space),
        keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
    );

    if !controller.flying && keyboard.just_pressed(KeyCode::Space) {
        controller.jump_buffer_remaining = JUMP_BUFFER_SECONDS;
    }
}

fn mouse_look(
    mouse_motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    player: Single<(&mut PlayerController, &mut Transform), Without<PlayerCamera>>,
    mut camera: Single<&mut Transform, (With<PlayerCamera>, Without<PlayerController>)>,
) {
    if cursor.grab_mode == CursorGrabMode::None || mouse_motion.delta == Vec2::ZERO {
        return;
    }

    let (mut controller, mut player_transform) = player.into_inner();
    controller.yaw -= mouse_motion.delta.x * MOUSE_SENSITIVITY;
    controller.pitch = (controller.pitch - mouse_motion.delta.y * MOUSE_SENSITIVITY)
        .clamp(-PITCH_LIMIT, PITCH_LIMIT);

    player_transform.rotation = Quat::from_rotation_y(controller.yaw);
    camera.rotation = Quat::from_rotation_x(controller.pitch);
}

fn fixed_player_movement(
    fixed_time: Res<Time<Fixed>>,
    input: Res<ControllerInput>,
    collision_world: Res<CollisionWorld>,
    player: Single<(&mut PlayerController, &mut PhysicalPosition)>,
) {
    let dt = fixed_time.delta_secs();
    let (mut controller, mut physical_position) = player.into_inner();
    physical_position.previous = physical_position.current;

    if controller.flying {
        controller.grounded = false;
        controller.coyote_remaining = 0.0;
        controller.jump_buffer_remaining = 0.0;

        apply_friction(&mut controller.velocity, FLY_FRICTION, dt);
        let wish = flight_wish(
            input.movement,
            input.fly_vertical,
            controller.yaw,
            controller.pitch,
        );
        let wish_amount = wish.length().min(1.0);
        accelerate(
            &mut controller.velocity,
            wish.normalize_or_zero(),
            FLY_SPEED * wish_amount,
            FLY_ACCELERATION,
            dt,
        );
        controller.velocity = controller.velocity.clamp_length_max(MAX_FLY_SPEED);
    } else {
        update_walking_velocity(&mut controller, &input, dt);
    }

    let movement = move_aabb(
        physical_position.current,
        PLAYER_HALF_SIZE,
        controller.velocity * dt,
        &collision_world.boxes,
    );
    physical_position.current = movement.position;

    if movement.contacts.blocked_x {
        controller.velocity.x = 0.0;
    }
    if movement.contacts.blocked_z {
        controller.velocity.z = 0.0;
    }
    if movement.contacts.blocked_y {
        controller.velocity.y = 0.0;
    }

    if !controller.flying {
        controller.grounded = movement.contacts.ground;
        if controller.grounded {
            controller.coyote_remaining = COYOTE_SECONDS;
        }
    }
}

fn update_walking_velocity(controller: &mut PlayerController, input: &ControllerInput, dt: f32) {
    let started_grounded = controller.grounded;
    if started_grounded {
        controller.coyote_remaining = COYOTE_SECONDS;
    } else {
        controller.coyote_remaining = (controller.coyote_remaining - dt).max(0.0);
    }

    let mut horizontal = Vec3::new(controller.velocity.x, 0.0, controller.velocity.z);
    if started_grounded {
        apply_friction(&mut horizontal, GROUND_FRICTION, dt);
    }

    let wish = walking_wish(input.movement, controller.yaw);
    let wish_amount = wish.length().min(1.0);
    let target_speed = if input.running { RUN_SPEED } else { WALK_SPEED };
    accelerate(
        &mut horizontal,
        wish.normalize_or_zero(),
        target_speed * wish_amount,
        if started_grounded {
            GROUND_ACCELERATION
        } else {
            AIR_ACCELERATION
        },
        dt,
    );
    horizontal = horizontal.clamp_length_max(MAX_HORIZONTAL_SPEED);
    controller.velocity.x = horizontal.x;
    controller.velocity.z = horizontal.z;

    if controller.jump_buffer_remaining > 0.0 && controller.coyote_remaining > 0.0 {
        controller.velocity.y = JUMP_SPEED;
        controller.grounded = false;
        controller.coyote_remaining = 0.0;
        controller.jump_buffer_remaining = 0.0;
    } else {
        controller.jump_buffer_remaining = (controller.jump_buffer_remaining - dt).max(0.0);
    }

    controller.velocity.y -= GRAVITY * dt;
}

fn interpolate_player(
    fixed_time: Res<Time<Fixed>>,
    player: Single<(&mut Transform, &PhysicalPosition), Without<PlayerCamera>>,
) {
    let (mut transform, physical_position) = player.into_inner();
    transform.translation = physical_position
        .previous
        .lerp(physical_position.current, fixed_time.overstep_fraction());
}

fn axis(positive: bool, negative: bool) -> f32 {
    (positive as i8 - negative as i8) as f32
}

fn walking_wish(input: Vec2, yaw: f32) -> Vec3 {
    let yaw_rotation = Quat::from_rotation_y(yaw);
    let right = yaw_rotation * Vec3::X;
    let forward = yaw_rotation * Vec3::NEG_Z;
    right * input.x + forward * input.y
}

fn flight_wish(input: Vec2, vertical: f32, yaw: f32, pitch: f32) -> Vec3 {
    let yaw_rotation = Quat::from_rotation_y(yaw);
    let view_rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    let right = yaw_rotation * Vec3::X;
    let forward = view_rotation * Vec3::NEG_Z;
    (right * input.x + forward * input.y + Vec3::Y * vertical).clamp_length_max(1.0)
}

/// Quake-style acceleration: only adds speed along the desired direction, so
/// perpendicular momentum is retained for responsive strafing.
fn accelerate(
    velocity: &mut Vec3,
    wish_direction: Vec3,
    wish_speed: f32,
    acceleration: f32,
    dt: f32,
) {
    if wish_speed <= 0.0 || acceleration <= 0.0 || dt <= 0.0 {
        return;
    }

    let current_speed = velocity.dot(wish_direction);
    let speed_to_add = wish_speed - current_speed;
    if speed_to_add <= 0.0 {
        return;
    }

    let acceleration_speed = (acceleration * wish_speed * dt).min(speed_to_add);
    *velocity += wish_direction * acceleration_speed;
}

fn apply_friction(velocity: &mut Vec3, friction: f32, dt: f32) {
    let speed = velocity.length();
    if speed <= SWEEP_EPSILON {
        *velocity = Vec3::ZERO;
        return;
    }

    let new_speed = (speed - speed * friction.max(0.0) * dt.max(0.0)).max(0.0);
    *velocity *= new_speed / speed;
}

#[derive(Debug, Default, Clone, Copy)]
struct CollisionContacts {
    blocked_x: bool,
    blocked_y: bool,
    blocked_z: bool,
    ground: bool,
    ceiling: bool,
}

impl CollisionContacts {
    fn add(&mut self, normal: Vec3) {
        if normal.x.abs() > 0.5 {
            self.blocked_x = true;
        }
        if normal.y.abs() > 0.5 {
            self.blocked_y = true;
            self.ground |= normal.y > 0.5;
            self.ceiling |= normal.y < -0.5;
        }
        if normal.z.abs() > 0.5 {
            self.blocked_z = true;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MovementResult {
    position: Vec3,
    contacts: CollisionContacts,
}

#[derive(Debug, Clone, Copy)]
struct SweepHit {
    time: f32,
    normal: Vec3,
}

/// Sweeps the player's AABB continuously, then removes only the velocity into
/// each contacted plane. This prevents tunnelling and permits natural sliding.
fn move_aabb(
    start: Vec3,
    half_size: Vec3,
    displacement: Vec3,
    boxes: &[SolidBox],
) -> MovementResult {
    let mut position = start;
    let mut contacts = CollisionContacts::default();
    depenetrate(&mut position, half_size, boxes, &mut contacts);

    let mut remaining = displacement;
    for _ in 0..MAX_SLIDE_PLANES {
        if remaining.length_squared() <= SWEEP_EPSILON * SWEEP_EPSILON {
            break;
        }

        let Some(hit) = nearest_hit(position, half_size, remaining, boxes) else {
            position += remaining;
            break;
        };

        let time = hit.time.clamp(0.0, 1.0);
        position += remaining * time;
        remaining *= 1.0 - time;

        let into_surface = remaining.dot(hit.normal);
        if into_surface < 0.0 {
            remaining -= hit.normal * into_surface;
        }
        contacts.add(hit.normal);
    }

    // If the maximum number of slide planes was reached, discarding the tiny
    // remainder is safer than allowing it to cross unchecked geometry.
    MovementResult { position, contacts }
}

fn nearest_hit(
    origin: Vec3,
    half_size: Vec3,
    displacement: Vec3,
    boxes: &[SolidBox],
) -> Option<SweepHit> {
    let expansion = half_size + Vec3::splat(COLLISION_SKIN);
    let mut nearest: Option<SweepHit> = None;

    for solid in boxes {
        let expanded_min = solid.min - expansion;
        let expanded_max = solid.max + expansion;
        let Some(hit) = sweep_point_box(origin, displacement, expanded_min, expanded_max) else {
            continue;
        };

        if nearest.is_none_or(|current| hit.time < current.time - SWEEP_EPSILON) {
            nearest = Some(hit);
        }
    }

    nearest
}

/// Ray-versus-AABB slab test over the normalized interval `[0, 1]`.
fn sweep_point_box(origin: Vec3, displacement: Vec3, min: Vec3, max: Vec3) -> Option<SweepHit> {
    let mut enter_time = f32::NEG_INFINITY;
    let mut exit_time = f32::INFINITY;
    let mut enter_normal = Vec3::ZERO;

    for axis_index in 0..3 {
        let origin_axis = origin[axis_index];
        let displacement_axis = displacement[axis_index];
        let min_axis = min[axis_index];
        let max_axis = max[axis_index];

        if displacement_axis.abs() <= SWEEP_EPSILON {
            // A point exactly on a face and travelling parallel to it is
            // outside that solid, which is what permits frictionless sliding.
            if origin_axis <= min_axis + SWEEP_EPSILON || origin_axis >= max_axis - SWEEP_EPSILON {
                return None;
            }
            continue;
        }

        let (axis_enter, axis_exit, axis_normal) = if displacement_axis > 0.0 {
            (
                (min_axis - origin_axis) / displacement_axis,
                (max_axis - origin_axis) / displacement_axis,
                axis_vector(axis_index, -1.0),
            )
        } else {
            (
                (max_axis - origin_axis) / displacement_axis,
                (min_axis - origin_axis) / displacement_axis,
                axis_vector(axis_index, 1.0),
            )
        };

        if axis_enter > enter_time {
            enter_time = axis_enter;
            enter_normal = axis_normal;
        }
        exit_time = exit_time.min(axis_exit);
        if enter_time > exit_time + SWEEP_EPSILON {
            return None;
        }
    }

    if !(-SWEEP_EPSILON..=1.0 + SWEEP_EPSILON).contains(&enter_time)
        || exit_time < -SWEEP_EPSILON
        || enter_normal == Vec3::ZERO
        || displacement.dot(enter_normal) >= 0.0
    {
        return None;
    }

    Some(SweepHit {
        time: enter_time.max(0.0),
        normal: enter_normal,
    })
}

/// Recovers from invalid spawn points or numerical penetration by repeatedly
/// taking the shortest route out of an expanded solid.
fn depenetrate(
    position: &mut Vec3,
    half_size: Vec3,
    boxes: &[SolidBox],
    contacts: &mut CollisionContacts,
) {
    let expansion = half_size + Vec3::splat(COLLISION_SKIN);

    for _ in 0..MAX_DEPENETRATION_STEPS {
        let mut shortest_push: Option<Vec3> = None;

        for solid in boxes {
            let min = solid.min - expansion;
            let max = solid.max + expansion;
            if position.x <= min.x
                || position.x >= max.x
                || position.y <= min.y
                || position.y >= max.y
                || position.z <= min.z
                || position.z >= max.z
            {
                continue;
            }

            let candidates = [
                Vec3::new(min.x - position.x, 0.0, 0.0),
                Vec3::new(max.x - position.x, 0.0, 0.0),
                Vec3::new(0.0, min.y - position.y, 0.0),
                Vec3::new(0.0, max.y - position.y, 0.0),
                Vec3::new(0.0, 0.0, min.z - position.z),
                Vec3::new(0.0, 0.0, max.z - position.z),
            ];
            let push = candidates
                .into_iter()
                .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
                .expect("penetration candidates are non-empty");

            if shortest_push.is_none_or(|current| push.length_squared() < current.length_squared())
            {
                shortest_push = Some(push);
            }
        }

        let Some(mut push) = shortest_push else {
            break;
        };
        let normal = push.normalize_or_zero();
        push += normal * SWEEP_EPSILON;
        *position += push;
        contacts.add(normal);
    }
}

fn axis_vector(axis_index: usize, value: f32) -> Vec3 {
    match axis_index {
        0 => Vec3::new(value, 0.0, 0.0),
        1 => Vec3::new(0.0, value, 0.0),
        2 => Vec3::new(0.0, 0.0, value),
        _ => unreachable!("a Vec3 only has three axes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_HALF_SIZE: Vec3 = Vec3::splat(0.5);

    fn solid(min: Vec3, max: Vec3) -> SolidBox {
        SolidBox { min, max }
    }

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1.0e-4,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn acceleration_stops_at_wish_speed_and_keeps_perpendicular_momentum() {
        let mut velocity = Vec3::new(0.0, 2.0, 0.0);
        accelerate(&mut velocity, Vec3::X, 5.0, 20.0, 1.0);

        close(velocity.x, 5.0);
        close(velocity.y, 2.0);
        close(velocity.z, 0.0);
    }

    #[test]
    fn diagonal_walking_input_is_not_faster() {
        let input = Vec2::ONE.clamp_length_max(1.0);
        let wish = walking_wish(input, 0.0);
        close(wish.length(), 1.0);
    }

    #[test]
    fn walk_and_run_speeds_match_the_control_contract() {
        assert_eq!(WALK_SPEED, 8.0);
        assert_eq!(RUN_SPEED, 12.0);
        assert_eq!(MAX_HORIZONTAL_SPEED, RUN_SPEED);
    }

    #[test]
    fn player_starts_on_the_west_approach_facing_the_actor_cluster() {
        assert_eq!(PLAYER_SPAWN, Vec3::new(0.0, 0.91, 95.0));
        let initial_forward = Quat::from_rotation_y(PLAYER_START_YAW) * Vec3::NEG_Z;
        assert!(initial_forward.dot(Vec3::Z) > 0.999);
        for actor_position in [
            Vec3::new(0.0, 0.91, 112.0),
            Vec3::new(-1.8, 0.91, 114.0),
            Vec3::new(1.8, 0.91, 114.0),
        ] {
            assert!(PLAYER_SPAWN.distance(actor_position) <= 20.0);
        }
    }

    #[test]
    fn friction_is_monotonic_and_safe_at_zero() {
        let mut velocity = Vec3::new(3.0, 0.0, 4.0);
        apply_friction(&mut velocity, 2.0, 0.1);
        close(velocity.length(), 4.0);

        apply_friction(&mut velocity, 100.0, 1.0);
        assert_eq!(velocity, Vec3::ZERO);
        apply_friction(&mut velocity, 8.0, 1.0 / 120.0);
        assert!(velocity.is_finite());
    }

    #[test]
    fn high_speed_sweep_cannot_tunnel_through_a_thin_wall() {
        let wall = solid(Vec3::new(0.0, -2.0, -2.0), Vec3::new(0.05, 2.0, 2.0));
        let result = move_aabb(
            Vec3::new(-5.0, 0.0, 0.0),
            TEST_HALF_SIZE,
            Vec3::new(20.0, 0.0, 0.0),
            &[wall],
        );

        close(result.position.x, -0.5 - COLLISION_SKIN);
        assert!(result.contacts.blocked_x);
    }

    #[test]
    fn diagonal_sweep_hits_a_box_instead_of_skipping_its_corner() {
        let obstacle = solid(Vec3::splat(-0.25), Vec3::splat(0.25));
        let result = move_aabb(
            Vec3::new(-3.0, 0.0, -3.0),
            TEST_HALF_SIZE,
            Vec3::new(6.0, 0.0, 6.0),
            &[obstacle],
        );

        assert!(result.position.x < -0.7 || result.position.z < -0.7);
        assert!(result.contacts.blocked_x || result.contacts.blocked_z);
    }

    #[test]
    fn wall_collision_preserves_tangential_slide() {
        let wall = solid(Vec3::new(0.0, -3.0, -10.0), Vec3::new(0.2, 3.0, 10.0));
        let result = move_aabb(
            Vec3::new(-2.0, 0.0, -2.0),
            TEST_HALF_SIZE,
            Vec3::new(4.0, 0.0, 3.0),
            &[wall],
        );

        close(result.position.x, -0.5 - COLLISION_SKIN);
        close(result.position.z, 1.0);
        assert!(result.contacts.blocked_x);
        assert!(!result.contacts.blocked_z);
    }

    #[test]
    fn falling_lands_on_floor_and_reports_ground() {
        let floor = solid(Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
        let result = move_aabb(
            Vec3::new(0.0, 5.0, 0.0),
            TEST_HALF_SIZE,
            Vec3::new(0.0, -10.0, 0.0),
            &[floor],
        );

        close(result.position.y, 0.5 + COLLISION_SKIN);
        assert!(result.contacts.ground);
        assert!(result.contacts.blocked_y);
    }

    #[test]
    fn upward_sweep_stops_at_ceiling() {
        let ceiling = solid(Vec3::new(-10.0, 3.0, -10.0), Vec3::new(10.0, 4.0, 10.0));
        let result = move_aabb(
            Vec3::new(0.0, 1.0, 0.0),
            TEST_HALF_SIZE,
            Vec3::new(0.0, 10.0, 0.0),
            &[ceiling],
        );

        close(result.position.y, 3.0 - 0.5 - COLLISION_SKIN);
        assert!(result.contacts.ceiling);
    }

    #[test]
    fn resting_on_floor_does_not_block_horizontal_motion() {
        let floor = solid(Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
        let result = move_aabb(
            Vec3::new(0.0, 0.5 + COLLISION_SKIN, 0.0),
            TEST_HALF_SIZE,
            Vec3::new(5.0, 0.0, 0.0),
            &[floor],
        );

        close(result.position.x, 5.0);
        close(result.position.y, 0.5 + COLLISION_SKIN);
    }

    #[test]
    fn static_world_ray_returns_nearest_wall_distance() {
        let mut world = CollisionWorld::default();
        world.add_box(Vec3::new(-1.0, -1.0, 3.0), Vec3::new(1.0, 1.0, 3.5));
        world.add_box(Vec3::new(-1.0, -1.0, 7.0), Vec3::new(1.0, 1.0, 8.0));

        let distance = world
            .nearest_ray_hit(Vec3::ZERO, Vec3::Z, 20.0)
            .expect("the ray should hit the near wall");
        close(distance, 3.0);
        assert!(world.nearest_ray_hit(Vec3::ZERO, Vec3::X, 20.0).is_none());
    }

    #[test]
    fn fly_acceleration_has_no_implicit_gravity() {
        let mut velocity = Vec3::ZERO;
        let wish = flight_wish(Vec2::ZERO, 0.0, 0.0, 0.0);
        accelerate(
            &mut velocity,
            wish.normalize_or_zero(),
            FLY_SPEED * wish.length(),
            FLY_ACCELERATION,
            1.0 / 120.0,
        );

        assert_eq!(velocity, Vec3::ZERO);
    }
}
