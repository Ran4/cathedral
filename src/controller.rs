//! A small, dependency-free first-person character controller.
//!
//! The cathedral only needs simple collision. Axis-aligned boxes cover floors
//! and authored scene pieces; convex vertical prisms follow the city's rotated
//! cadastral footprints; the handful of moving gate throats contribute dynamic
//! boxes. All keep the controller deterministic and easy to test without
//! starting Bevy's renderer.
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
    anti_alias::smaa::Smaa,
    audio::SpatialListener,
    camera::Exposure,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    light::AtmosphereEnvironmentMapLight,
    pbr::{AtmosphereSettings, ScreenSpaceAmbientOcclusion},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::Msaa,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::map::MapState;
use crate::smart_actors::{ConfigMenuState, InventoryUiState};
use crate::smart_actors::model::ActorId;

const FIXED_HZ: f64 = 120.0;
// Begin on the open west approach, facing the actor cluster. At 17–19 m the
// whole initial cast is inside the settled 20 m hearing radius immediately.
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.91, 95.0);
const PLAYER_START_YAW: f32 = PI;
const PLAYER_HALF_SIZE: Vec3 = Vec3::new(0.35, 0.9, 0.35);
const EYE_OFFSET: f32 = 0.65;

// A standing player is a full-height AABB centred at WALK_Y, so its body spans
// the vertical band [WALK_Y - half_height, WALK_Y + half_height]. The collision
// sweep expands every collider by that half-height, so a solid blocks the XZ an
// NPC stands on iff its [min_y, max_y] *overlaps* this band — not merely when it
// crosses WALK_Y. (The Minkowski-expanded collider contains the standing centre
// exactly when the interval overlap holds.) Derived from PLAYER_SPAWN.y (0.91) ∓
// PLAYER_HALF_SIZE.y (0.9); glam const field access is unavailable in a `const`,
// so the endpoints are written out. The navigation bake subtracts exactly the
// footprints in this band, so its walkable surface matches what stops the player
// — including solids that top out below 0.91 (troughs, low platforms, rims).
#[allow(dead_code)] // used by the collision-export and walkable-surface tests
pub const WALK_BAND_LO: f32 = 0.01; // PLAYER_SPAWN.y - PLAYER_HALF_SIZE.y
#[allow(dead_code)] // used by the collision-export and walkable-surface tests
pub const WALK_BAND_HI: f32 = 1.81; // PLAYER_SPAWN.y + PLAYER_HALF_SIZE.y

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
const PRISM_GEOMETRY_EPSILON: f32 = 1.0e-3;
/// How long to wait before hanging the sky probe on the camera — long enough
/// for the window surface and first rendered view to exist. See
/// [`attach_sky_probe_once_rendering`].
const SKY_PROBE_DELAY_SECONDS: f32 = 0.75;

const MAX_SLIDE_PLANES: usize = 5;
const MAX_DEPENETRATION_STEPS: usize = 8;

/// Adds first-person input, fixed-step movement, and collision to an app.
pub struct ControllerPlugin;

impl Plugin for ControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CollisionWorld>()
            .init_resource::<ControllerInput>()
            .add_message::<TeleportPlayer>()
            .insert_resource(Time::<Fixed>::from_hz(FIXED_HZ))
            .add_systems(Startup, (spawn_player, initially_capture_cursor))
            .add_systems(Update, attach_sky_probe_once_rendering)
            .add_systems(FixedUpdate, fixed_player_movement)
            .add_systems(
                RunFixedMainLoop,
                (
                    (collect_input, mouse_look, apply_teleports)
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

impl PlayerController {
    /// The live velocity, in m/s. Read by the custody grab reflex and the
    /// strain meter to tell *stepping aside* from *leaving*
    /// (`law_and_order.md` M4c/M4d) — pressing a key into a wall is not
    /// pulling, and the world-frame velocity is the only thing that knows that.
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Test-only: a controller already moving at `velocity`. Only the fixed-step
    /// solve writes that field, so the custody tests — which never run the
    /// solve — need a way to say *this player is running away* without one.
    #[cfg(test)]
    pub(crate) fn moving_at(velocity: Vec3) -> Self {
        Self {
            velocity,
            ..Self::default()
        }
    }
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

    /// Horizontal world speed in metres per second.
    ///
    /// Presentation systems use this rather than reconstructing velocity from
    /// interpolated render transforms (which would make cadence frame-rate
    /// dependent).
    pub(crate) fn horizontal_speed(&self) -> f32 {
        Vec2::new(self.velocity.x, self.velocity.z).length()
    }

    /// Whether the fixed-step controller is resting on walkable geometry.
    pub(crate) fn is_grounded(&self) -> bool {
        self.grounded
    }
}

/// An axis-aligned collider whose participation can change at runtime.
///
/// The cadastral collision world is intentionally immutable after Startup,
/// but gates are physical mechanisms: their throat must stop the player only
/// after the leaves have materially closed.  Keeping this tiny component next
/// to the controller preserves one collision authority without making the
/// static world's internals mutable or globally exposing `SolidBox`.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DynamicBarrier {
    pub half_size: Vec3,
    pub active: bool,
}

/// Static collision geometry used by the character controller.
///
/// Scene geometry may be as detailed as desired; register simple boxes for
/// floors, walls, and stairs, or convex vertical prisms for rotated footprints.
#[derive(Resource, Debug, Default)]
pub struct CollisionWorld {
    boxes: Vec<SolidBox>,
    convex_prisms: Vec<SolidConvexPrism>,
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

    /// Registers a solid vertical prism with a convex footprint in the XZ plane.
    ///
    /// Vertices may use either winding order. Invalid, degenerate, or concave
    /// footprints are ignored, just like invalid boxes; callers can decompose a
    /// concave footprint into convex pieces before registering it.
    pub fn add_convex_prism(&mut self, footprint_xz: &[[f32; 2]], min_y: f32, max_y: f32) {
        let lower_y = min_y.min(max_y);
        let upper_y = min_y.max(max_y);
        if footprint_xz.len() < 3
            || !lower_y.is_finite()
            || !upper_y.is_finite()
            || upper_y <= lower_y
            || footprint_xz
                .iter()
                .flatten()
                .any(|coordinate| !coordinate.is_finite())
        {
            return;
        }

        let signed_area = footprint_xz
            .iter()
            .zip(footprint_xz.iter().cycle().skip(1))
            .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
            .sum::<f32>()
            * 0.5;
        if signed_area.abs() <= PRISM_GEOMETRY_EPSILON {
            return;
        }

        let winding = signed_area.signum();
        let vertices = footprint_xz
            .iter()
            .copied()
            .map(Vec2::from_array)
            .collect::<Vec<_>>();
        let mut planes = Vec::with_capacity(vertices.len() + 4);

        // An AABB swept against a convex polygon uses the separating axes of
        // both shapes. Put the AABB's four axes first so they also serve as a
        // cheap broad-phase rejection before testing the footprint edges.
        for normal in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
            let offset = vertices
                .iter()
                .map(|vertex| normal.dot(*vertex))
                .max_by(f32::total_cmp)
                .expect("a validated prism has vertices");
            planes.push(PrismPlane { normal, offset });
        }

        for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
            let edge = *b - *a;
            if edge.length_squared() <= PRISM_GEOMETRY_EPSILON * PRISM_GEOMETRY_EPSILON {
                return;
            }
            let outward = (Vec2::new(edge.y, -edge.x) * winding).normalize();
            let edge_offset = outward.dot(*a);
            let offset = vertices
                .iter()
                .map(|vertex| outward.dot(*vertex))
                .max_by(f32::total_cmp)
                .expect("a validated prism has vertices");
            if offset > edge_offset + PRISM_GEOMETRY_EPSILON {
                return;
            }
            planes.push(PrismPlane {
                normal: outward,
                offset,
            });
        }

        self.convex_prisms.push(SolidConvexPrism {
            min_y: lower_y,
            max_y: upper_y,
            planes: planes.into_boxed_slice(),
            footprint: vertices.into_boxed_slice(),
        });
    }

    /// Returns the distance to the first static collider hit by a ray.
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
            .chain(self.convex_prisms.iter().filter_map(|solid| {
                sweep_point_prism(origin, displacement, Vec3::ZERO, solid)
                    .map(|hit| hit.time * max_distance)
            }))
            .min_by(f32::total_cmp)
    }

    /// Whether a world point lies inside any static solid. The same coarse
    /// geometry that stops the player, queried as a point test — used to prove
    /// the baked walkable surface never overlaps a collider.
    #[allow(dead_code)] // exercised by tests; movement will read it in M2
    pub fn contains_point(&self, point: Vec3) -> bool {
        let xz = Vec2::new(point.x, point.z);
        let in_box = self.boxes.iter().any(|solid| {
            point.x >= solid.min.x
                && point.x <= solid.max.x
                && point.y >= solid.min.y
                && point.y <= solid.max.y
                && point.z >= solid.min.z
                && point.z <= solid.max.z
        });
        if in_box {
            return true;
        }
        self.convex_prisms.iter().any(|solid| {
            point.y >= solid.min_y
                && point.y <= solid.max_y
                && solid
                    .planes
                    .iter()
                    .all(|plane| plane.normal.dot(xz) <= plane.offset)
        })
    }

    /// The XZ footprint polygon of every solid whose vertical extent overlaps the
    /// band `[lo, hi]` — a box as its four corners, a prism as the polygon it was
    /// built from. This is what the navigation bake subtracts, so the walkable
    /// surface is the exact complement of what stops the player across the whole
    /// band its standing AABB sweeps (`WALK_BAND_LO`..=`WALK_BAND_HI`), not only
    /// the colliders that happen to cross the single walk plane. A solid overlaps
    /// iff `max.y >= lo && min.y <= hi` — the standard interval-overlap test, so a
    /// trough (top 0.68) or a low platform (top 0.8) beneath the walk plane counts.
    #[allow(dead_code)] // used by the collision-export and walkable-surface tests
    pub fn solid_footprints_in_band(&self, lo: f32, hi: f32) -> Vec<Vec<Vec2>> {
        let mut out = Vec::new();
        for solid in &self.boxes {
            if solid.max.y >= lo && solid.min.y <= hi {
                out.push(vec![
                    Vec2::new(solid.min.x, solid.min.z),
                    Vec2::new(solid.max.x, solid.min.z),
                    Vec2::new(solid.max.x, solid.max.z),
                    Vec2::new(solid.min.x, solid.max.z),
                ]);
            }
        }
        for solid in &self.convex_prisms {
            if solid.max_y >= lo && solid.min_y <= hi {
                out.push(solid.footprint.to_vec());
            }
        }
        out
    }

    /// Whether the vertical band `[lo, hi]` at world XZ `(x, z)` lies inside any
    /// static solid — i.e. whether a standing player's AABB collides there. This
    /// mirrors [`Self::contains_point`]'s XZ tests (box corners, prism half-planes)
    /// but swaps its single-plane `y` check for band overlap, so it also catches
    /// solids topping out below the walk plane (troughs, cistern rims, the
    /// bellstand platform). The walkable-surface test uses it to prove no baked
    /// cell overlaps a collider the player's body sweeps.
    #[allow(dead_code)] // exercised by the walkable-surface test
    pub fn blocks_walk_band(&self, x: f32, z: f32, lo: f32, hi: f32) -> bool {
        let xz = Vec2::new(x, z);
        let in_box = self.boxes.iter().any(|solid| {
            solid.max.y >= lo
                && solid.min.y <= hi
                && x >= solid.min.x
                && x <= solid.max.x
                && z >= solid.min.z
                && z <= solid.max.z
        });
        if in_box {
            return true;
        }
        self.convex_prisms.iter().any(|solid| {
            solid.max_y >= lo
                && solid.min_y <= hi
                && solid
                    .planes
                    .iter()
                    .all(|plane| plane.normal.dot(xz) <= plane.offset)
        })
    }

    /// Number of static colliders registered by the scene.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.boxes.len() + self.convex_prisms.len()
    }

    /// Whether the scene has registered any collision geometry yet.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty() && self.convex_prisms.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
struct SolidBox {
    min: Vec3,
    max: Vec3,
}

#[derive(Debug)]
struct SolidConvexPrism {
    min_y: f32,
    max_y: f32,
    planes: Box<[PrismPlane]>,
    /// The XZ footprint the prism was built from — kept so the navigation bake
    /// can subtract the exact solid, not its axis-aligned bound.
    #[allow(dead_code)] // read via solid_footprints_in_band in tests
    footprint: Box<[Vec2]>,
}

#[derive(Debug, Clone, Copy)]
struct PrismPlane {
    normal: Vec2,
    offset: f32,
}

/// The fixed-step authoritative player position, interpolated for rendering.
/// `pub` so the custody tether can read where the player actually is without
/// going through the interpolated `Transform` (`law_and_order.md` M4c).
#[derive(Component, Debug)]
pub struct PhysicalPosition {
    pub previous: Vec3,
    pub current: Vec3,
}

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Resource, Debug, Default)]
pub struct ControllerInput {
    pub movement: Vec2,
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
                // NOTE: the sky-probe light is *not* spawned here — see
                // [`attach_sky_probe_once_rendering`].
                Exposure { ev100: 12.8 },
                Tonemapping::AcesFitted,
                // Screen-space AO is the only occlusion signal a fully dynamic
                // city has: it darkens reveals, eaves, and alley mouths that the
                // flat ambient otherwise fills in. It requires MSAA off, so SMAA
                // takes over edge smoothing.
                Msaa::Off,
                Smaa::default(),
                ScreenSpaceAmbientOcclusion::default(),
                // Subtle energy bleed for the sky, sunlit plaster, and the rose
                // window; NATURAL keeps it below music-video threshold.
                Bloom::NATURAL,
                Transform::from_xyz(0.0, EYE_OFFSET, 0.0),
            ));
        });
}

/// Give the camera its sky-derived ambient probe, but only once the app has
/// actually drawn a few frames.
///
/// `AtmosphereEnvironmentMapLight` makes bevy build an atmosphere probe whose
/// bind group reads the *view's* atmosphere transform uniform. Those two are
/// prepared by different systems keyed off different queries, so during the
/// first frames — while the window surface is still coming up and the camera
/// isn't extracted as a rendered view — the probe can exist with no transform
/// written, and `prepare_atmosphere_probe_bind_groups` unwraps a `None`
/// (bevy_pbr 0.19 `atmosphere/environment.rs:116`). That is the intermittent
/// startup panic that has hit this project since the atmosphere landed.
/// Attaching the component after the surface is live sidesteps the race
/// entirely; the probe warms up within a frame of being added, so the visual
/// result is the spawn-time one.
fn attach_sky_probe_once_rendering(
    mut commands: Commands,
    time: Res<Time>,
    cameras: Query<Entity, (With<PlayerCamera>, Without<AtmosphereEnvironmentMapLight>)>,
) {
    if time.elapsed_secs() < SKY_PROBE_DELAY_SECONDS {
        return;
    }
    for camera in &cameras {
        commands.entity(camera).insert(AtmosphereEnvironmentMapLight {
            intensity: 0.65,
            size: UVec2::splat(128),
            ..default()
        });
    }
}

/// Relocate the player and aim the view. The drive `tp` action sets `fly: true`
/// so an elevated vantage holds for a screenshot; the map's click-to-travel sets
/// `fly: false` so you land walking on the ground.
#[derive(Message, Debug, Clone, Copy)]
pub struct TeleportPlayer {
    pub position: Vec3,
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub fly: bool,
}

fn apply_teleports(
    mut teleports: MessageReader<TeleportPlayer>,
    player: Single<
        (&mut PlayerController, &mut PhysicalPosition, &mut Transform),
        Without<PlayerCamera>,
    >,
    mut camera: Single<&mut Transform, (With<PlayerCamera>, Without<PlayerController>)>,
) {
    let Some(teleport) = teleports.read().last().copied() else {
        return;
    };
    let (mut controller, mut physical, mut transform) = player.into_inner();
    controller.flying = teleport.fly;
    controller.velocity = Vec3::ZERO;
    controller.yaw = teleport.yaw_degrees.to_radians();
    controller.pitch = teleport
        .pitch_degrees
        .to_radians()
        .clamp(-PITCH_LIMIT, PITCH_LIMIT);
    physical.previous = teleport.position;
    physical.current = teleport.position;
    transform.translation = teleport.position;
    transform.rotation = Quat::from_rotation_y(controller.yaw);
    camera.rotation = Quat::from_rotation_x(controller.pitch);
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
#[allow(clippy::too_many_arguments)]
fn collect_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    menu: Res<ConfigMenuState>,
    map: Res<MapState>,
    inventory: Option<Res<InventoryUiState>>,
    mut input: ResMut<ControllerInput>,
    mut controller: Single<&mut PlayerController>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    // While the fullscreen map or the `I` inventory screen is open the cursor is
    // released for clicking; that overlay owns the cursor, and the player holds
    // still (movement, jump, the fly toggle, and click-to-recapture all pause).
    // Mouse-look already stops on its own once the cursor is unlocked.
    if map.fullscreen_open || inventory.is_some_and(|inventory| inventory.open) {
        input.movement = Vec2::ZERO;
        input.running = false;
        input.fly_vertical = 0.0;
        return;
    }

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
    // Mouse right turns the view right. Increasing yaw turns *left* (the
    // controller's own right vector is `from_rotation_y(yaw) * X`, and yaw +90°
    // swings forward from -Z onto -X, i.e. away from that right vector), so the
    // horizontal look must be negated here. Do not "fix" this sign to make the
    // minimap marker agree — see `map::marker_rotation`.
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
    custody: Res<crate::smart_actors::custody::PlayerCustodyState>,
    dynamic_barriers: Query<(&DynamicBarrier, &Transform)>,
    player: Single<(&mut PlayerController, &mut PhysicalPosition)>,
    mut dynamic_boxes: Local<Vec<SolidBox>>,
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

    dynamic_boxes.clear();
    dynamic_boxes.extend(dynamic_barriers.iter().filter_map(|(barrier, transform)| {
        (barrier.active
            && barrier.half_size.is_finite()
            && barrier.half_size.cmpgt(Vec3::ZERO).all()
            && transform.translation.is_finite())
        .then_some(SolidBox {
            min: transform.translation - barrier.half_size,
            max: transform.translation + barrier.half_size,
        })
    }));
    // The custody tether (`law_and_order.md` M4c) goes on the **wish**, before
    // the sweep below — see `tethered_delta` for why that order is the whole
    // mechanic.
    let delta = tethered_delta(
        physical_position.current,
        controller.velocity * dt,
        custody.tether(controller.flying),
    );
    let movement = move_aabb(
        physical_position.current,
        PLAYER_HALF_SIZE,
        delta,
        &collision_world.boxes,
        &dynamic_boxes,
        &collision_world.convex_prisms,
    );
    physical_position.current = movement.position;

    movement.contacts.clip_velocity(&mut controller.velocity);

    if !controller.flying {
        controller.grounded = movement.contacts.ground;
        if controller.grounded {
            controller.coyote_remaining = COYOTE_SECONDS;
        }
    }
}

/// The custody tether's whole arithmetic (`law_and_order.md` M4c): clamp the
/// **desired** position — where this tick's delta wants to put the player —
/// and return the delta that reaches it, for the swept solve to resolve.
///
/// Never the position the sweep already returned. Clamping *that* would be a raw
/// write into a solved result: it could push the player through a wall, and it
/// would drag them back through the very stall that ought to have saved them.
///
/// In this order, collision wins over the tether for free, and putting a market
/// stall between yourself and the officer breaks the grip by itself. If the
/// geometry beats the anchor entirely — a corner, a doorway the officer walks
/// past — the sweep simply stops the player, the separation grows, and the sim
/// ends the hold once it passes the leash. No teleport, no rubber band, and
/// never a player pinned inside a wall to preserve a state flag.
///
/// A player already outside the radius is pulled back to its surface, which is
/// what being *dragged* by a walking officer means. `None` — nobody has hold, or
/// the player is flying, which is never custody — leaves the delta exactly as
/// the controller solved it.
fn tethered_delta(current: Vec3, delta: Vec3, tether: Option<(Vec3, f32)>) -> Vec3 {
    let Some((anchor, radius)) = tether else {
        return delta;
    };
    let offset = current + delta - anchor;
    if offset.length() <= radius {
        return delta;
    }
    anchor + offset.normalize_or_zero() * radius - current
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

const MAX_CONTACT_NORMALS: usize = MAX_SLIDE_PLANES + MAX_DEPENETRATION_STEPS;

#[derive(Debug, Clone, Copy)]
struct CollisionContacts {
    blocked_x: bool,
    blocked_y: bool,
    blocked_z: bool,
    ground: bool,
    ceiling: bool,
    normals: [Vec3; MAX_CONTACT_NORMALS],
    normal_count: usize,
}

impl Default for CollisionContacts {
    fn default() -> Self {
        Self {
            blocked_x: false,
            blocked_y: false,
            blocked_z: false,
            ground: false,
            ceiling: false,
            normals: [Vec3::ZERO; MAX_CONTACT_NORMALS],
            normal_count: 0,
        }
    }
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

        if self.normal_count < self.normals.len()
            && !self.normals[..self.normal_count]
                .iter()
                .any(|registered| registered.dot(normal) > 1.0 - SWEEP_EPSILON)
        {
            self.normals[self.normal_count] = normal;
            self.normal_count += 1;
        }
    }

    fn clip_velocity(&self, velocity: &mut Vec3) {
        // A later plane can push a velocity slightly back into an earlier one
        // at a convex corner, so make a second inexpensive pass.
        for _ in 0..2 {
            for normal in &self.normals[..self.normal_count] {
                let into_surface = velocity.dot(*normal);
                if into_surface < 0.0 {
                    *velocity -= *normal * into_surface;
                }
            }
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
    dynamic_boxes: &[SolidBox],
    prisms: &[SolidConvexPrism],
) -> MovementResult {
    let mut position = start;
    let mut contacts = CollisionContacts::default();
    depenetrate(
        &mut position,
        half_size,
        boxes,
        dynamic_boxes,
        prisms,
        &mut contacts,
    );

    let mut remaining = displacement;
    for _ in 0..MAX_SLIDE_PLANES {
        if remaining.length_squared() <= SWEEP_EPSILON * SWEEP_EPSILON {
            break;
        }

        let box_hit = nearest_hit(position, half_size, remaining, boxes, dynamic_boxes);
        let prism_hit = nearest_prism_hit(position, half_size, remaining, prisms);
        let hit = match (box_hit, prism_hit) {
            (Some(box_hit), Some(prism_hit)) if prism_hit.time < box_hit.time - SWEEP_EPSILON => {
                prism_hit
            }
            (Some(box_hit), _) => box_hit,
            (None, Some(prism_hit)) => prism_hit,
            (None, None) => {
                position += remaining;
                break;
            }
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
    dynamic_boxes: &[SolidBox],
) -> Option<SweepHit> {
    let expansion = half_size + Vec3::splat(COLLISION_SKIN);
    let mut nearest: Option<SweepHit> = None;

    for solid in boxes.iter().chain(dynamic_boxes) {
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

fn nearest_prism_hit(
    origin: Vec3,
    half_size: Vec3,
    displacement: Vec3,
    prisms: &[SolidConvexPrism],
) -> Option<SweepHit> {
    let expansion = half_size + Vec3::splat(COLLISION_SKIN);
    let mut nearest: Option<SweepHit> = None;

    for solid in prisms {
        let Some(hit) = sweep_point_prism(origin, displacement, expansion, solid) else {
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

/// Ray-versus-expanded-convex-prism clipping over `[0, 1]`.
fn sweep_point_prism(
    origin: Vec3,
    displacement: Vec3,
    expansion: Vec3,
    solid: &SolidConvexPrism,
) -> Option<SweepHit> {
    let mut enter_time = f32::NEG_INFINITY;
    let mut exit_time = f32::INFINITY;
    let mut enter_normal = Vec3::ZERO;

    for plane in &solid.planes {
        let normal = Vec3::new(plane.normal.x, 0.0, plane.normal.y);
        let offset =
            plane.offset + plane.normal.x.abs() * expansion.x + plane.normal.y.abs() * expansion.z;
        if !clip_sweep_half_space(
            origin,
            displacement,
            normal,
            offset,
            &mut enter_time,
            &mut exit_time,
            &mut enter_normal,
        ) {
            return None;
        }
    }

    for (normal, offset) in [
        (Vec3::Y, solid.max_y + expansion.y),
        (Vec3::NEG_Y, -solid.min_y + expansion.y),
    ] {
        if !clip_sweep_half_space(
            origin,
            displacement,
            normal,
            offset,
            &mut enter_time,
            &mut exit_time,
            &mut enter_normal,
        ) {
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

#[allow(clippy::too_many_arguments)]
fn clip_sweep_half_space(
    origin: Vec3,
    displacement: Vec3,
    outward_normal: Vec3,
    offset: f32,
    enter_time: &mut f32,
    exit_time: &mut f32,
    enter_normal: &mut Vec3,
) -> bool {
    let distance_inside = offset - outward_normal.dot(origin);
    let speed_outward = outward_normal.dot(displacement);
    if speed_outward.abs() <= SWEEP_EPSILON {
        return distance_inside >= -SWEEP_EPSILON;
    }

    let plane_time = distance_inside / speed_outward;
    if speed_outward < 0.0 {
        if plane_time > *enter_time {
            *enter_time = plane_time;
            *enter_normal = outward_normal;
        }
    } else {
        *exit_time = exit_time.min(plane_time);
    }
    *enter_time <= *exit_time + SWEEP_EPSILON
}

/// Recovers from invalid spawn points or numerical penetration by repeatedly
/// taking the shortest route out of an expanded solid.
fn depenetrate(
    position: &mut Vec3,
    half_size: Vec3,
    boxes: &[SolidBox],
    dynamic_boxes: &[SolidBox],
    prisms: &[SolidConvexPrism],
    contacts: &mut CollisionContacts,
) {
    let expansion = half_size + Vec3::splat(COLLISION_SKIN);

    for _ in 0..MAX_DEPENETRATION_STEPS {
        let mut shortest_push: Option<Vec3> = None;

        for solid in boxes.iter().chain(dynamic_boxes) {
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

        for solid in prisms {
            let Some(push) = prism_penetration_push(*position, expansion, solid) else {
                continue;
            };
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

fn prism_penetration_push(
    position: Vec3,
    expansion: Vec3,
    solid: &SolidConvexPrism,
) -> Option<Vec3> {
    let min_y = solid.min_y - expansion.y;
    let max_y = solid.max_y + expansion.y;
    if position.y <= min_y || position.y >= max_y {
        return None;
    }

    let mut shortest_push = if position.y - min_y < max_y - position.y {
        Vec3::NEG_Y * (position.y - min_y)
    } else {
        Vec3::Y * (max_y - position.y)
    };
    let position_xz = Vec2::new(position.x, position.z);
    for plane in &solid.planes {
        let offset =
            plane.offset + plane.normal.x.abs() * expansion.x + plane.normal.y.abs() * expansion.z;
        let distance_inside = offset - plane.normal.dot(position_xz);
        if distance_inside <= 0.0 {
            return None;
        }
        let push = Vec3::new(plane.normal.x, 0.0, plane.normal.y) * distance_inside;
        if push.length_squared() < shortest_push.length_squared() {
            shortest_push = push;
        }
    }
    Some(shortest_push)
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
    use std::time::Duration;

    use cathedral_sim::custody::{CUSTODY_LEASH_M, CUSTODY_TETHER_M};

    use crate::smart_actors::custody::PlayerCustodyState;

    use super::*;

    const TEST_HALF_SIZE: Vec3 = Vec3::splat(0.5);
    /// The officer's own pace. A held player is dragged at whatever speed the
    /// grip point moves, and the grip point is a person walking.
    const OFFICER_SPEED_MPS: f32 = 1.8;

    fn solid(min: Vec3, max: Vec3) -> SolidBox {
        SolidBox { min, max }
    }

    fn move_boxes(
        start: Vec3,
        half_size: Vec3,
        displacement: Vec3,
        boxes: &[SolidBox],
    ) -> MovementResult {
        move_aabb(start, half_size, displacement, boxes, &[], &[])
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
        let result = move_boxes(
            Vec3::new(-5.0, 0.0, 0.0),
            TEST_HALF_SIZE,
            Vec3::new(20.0, 0.0, 0.0),
            &[wall],
        );

        close(result.position.x, -0.5 - COLLISION_SKIN);
        assert!(result.contacts.blocked_x);
    }

    #[test]
    fn dynamic_barrier_boxes_share_the_continuous_collision_path() {
        let gate = solid(Vec3::new(0.0, -2.0, -2.0), Vec3::new(0.4, 2.0, 2.0));
        let result = move_aabb(
            Vec3::new(-5.0, 0.0, 0.0),
            TEST_HALF_SIZE,
            Vec3::new(20.0, 0.0, 0.0),
            &[],
            &[gate],
            &[],
        );

        close(result.position.x, -0.5 - COLLISION_SKIN);
        assert!(result.contacts.blocked_x);
    }

    #[test]
    fn diagonal_sweep_hits_a_box_instead_of_skipping_its_corner() {
        let obstacle = solid(Vec3::splat(-0.25), Vec3::splat(0.25));
        let result = move_boxes(
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
        let result = move_boxes(
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
        let result = move_boxes(
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
        let result = move_boxes(
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
        let result = move_boxes(
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
    fn rotated_prism_ray_hits_its_actual_facade() {
        let footprint = [[0.0, -2.0], [2.0, 0.0], [0.0, 2.0], [-2.0, 0.0]];
        let outward = Vec3::new(1.0, 0.0, -1.0).normalize();
        let facade_midpoint = Vec3::new(1.0, 1.0, -1.0);
        let mut world = CollisionWorld::default();
        world.add_convex_prism(&footprint, 0.0, 3.0);

        let distance = world
            .nearest_ray_hit(facade_midpoint + outward * 3.0, -outward, 6.0)
            .expect("the ray should hit the rotated facade");
        close(distance, 3.0);
    }

    #[test]
    fn swept_aabb_cannot_enter_a_rotated_prism() {
        let footprint = [[0.0, -2.0], [2.0, 0.0], [0.0, 2.0], [-2.0, 0.0]];
        let outward = Vec3::new(1.0, 0.0, -1.0).normalize();
        let facade_midpoint = Vec3::new(1.0, 1.0, -1.0);
        let mut world = CollisionWorld::default();
        world.add_convex_prism(&footprint, 0.0, 3.0);

        let start = facade_midpoint + outward * 3.0;
        let result = move_aabb(
            start,
            TEST_HALF_SIZE,
            -outward * 6.0,
            &world.boxes,
            &[],
            &world.convex_prisms,
        );
        let expected_clearance = TEST_HALF_SIZE.x * outward.x.abs()
            + TEST_HALF_SIZE.z * outward.z.abs()
            + COLLISION_SKIN * (outward.x.abs() + outward.z.abs());
        close(
            (result.position - facade_midpoint).dot(outward),
            expected_clearance,
        );
    }

    #[test]
    fn diagonal_contact_keeps_tangential_velocity() {
        let normal = Vec3::new(1.0, 0.0, -1.0).normalize();
        let tangent = Vec3::new(1.0, 0.0, 1.0).normalize();
        let mut contacts = CollisionContacts::default();
        contacts.add(normal);
        let mut velocity = tangent * 4.0 - normal * 2.0;

        contacts.clip_velocity(&mut velocity);

        close(velocity.dot(normal), 0.0);
        close(velocity.dot(tangent), 4.0);
    }

    /// `law_and_order.md` M4c, "the tether": the clamp is applied to the
    /// **desired** position and the swept solve resolves it, so collision is
    /// always the last word. This case is built so the two orders disagree — the
    /// officer stands on the far side of a wall, and the only point on the
    /// tether sphere in that direction is *inside* the wall — and the test
    /// asserts both halves: clamping afterwards really would put the player in
    /// the solid, and clamping first really does not.
    #[test]
    fn the_tether_clamps_the_desired_position_so_a_wall_still_stops_a_held_player() {
        let wall = solid(Vec3::new(0.0, -3.0, -10.0), Vec3::new(0.4, 3.0, 10.0));
        let anchor = Vec3::new(2.0, 0.0, 0.0);
        let radius = CUSTODY_TETHER_M as f32;
        let start = Vec3::new(-3.0, 0.0, 0.0);
        // The player is running away from the officer; the tether overrides that
        // wish entirely and pulls them back toward the grip point.
        let delta = tethered_delta(start, Vec3::new(-1.0, 0.0, 0.0), Some((anchor, radius)));
        assert!(delta.x > 0.0, "an outside player is dragged back inward");

        let result = move_boxes(start, TEST_HALF_SIZE, delta, &[wall]);

        // Outside the wall, on the player's own side of it.
        assert!(result.contacts.blocked_x);
        close(result.position.x, wall.min.x - TEST_HALF_SIZE.x - COLLISION_SKIN);

        // …and the discriminator: had the clamp been applied to this resolved
        // position instead, the player would now be standing in masonry.
        let clamped_after_the_sweep =
            anchor + (result.position - anchor).normalize_or_zero() * radius;
        assert!(
            clamped_after_the_sweep.x > wall.min.x - TEST_HALF_SIZE.x
                && clamped_after_the_sweep.x < wall.max.x + TEST_HALF_SIZE.x,
            "the case has to be one the two orders disagree about, and {} is inside the wall",
            clamped_after_the_sweep.x
        );
    }

    /// `law_and_order.md` M4c, "if the geometry beats the anchor entirely": a
    /// solid between the player and a walking officer breaks the grip rather than
    /// dragging them through it. The *ending* of the hold is the sim's (past
    /// `CUSTODY_LEASH_M`); the host's half is that the player stays put and the
    /// separation is allowed to grow.
    #[test]
    fn a_solid_between_the_player_and_a_walking_anchor_breaks_the_grip_rather_than_dragging_them_through_it()
     {
        let wall = solid(Vec3::new(0.0, -3.0, -10.0), Vec3::new(0.4, 3.0, 10.0));
        let radius = CUSTODY_TETHER_M as f32;
        let dt = 1.0 / 120.0;
        let mut position = Vec3::new(-1.0, 0.0, 0.0);
        // The officer is already past the wall — through a doorway, around a
        // corner — and keeps walking, so the grip point leaves without the
        // player.
        let mut anchor = Vec3::new(1.0, 0.0, 0.0);
        let mut separation = f32::INFINITY;

        for tick in 0..720 {
            anchor.x += OFFICER_SPEED_MPS * dt;
            // The player themselves is standing still: every metre of travel
            // here is the tether dragging them.
            let delta = tethered_delta(position, Vec3::ZERO, Some((anchor, radius)));
            position = move_boxes(position, TEST_HALF_SIZE, delta, &[wall]).position;

            assert!(
                position.x <= wall.min.x - TEST_HALF_SIZE.x,
                "the drag must never pull the player into the wall (x = {})",
                position.x
            );
            let now = position.distance(anchor);
            // The first tick's drag closes the gap — the tether does its job
            // until the wall is in the way. From the moment the sweep stops the
            // player, every step the officer takes only opens it further.
            assert!(
                tick == 0 || now >= separation - 1.0e-4,
                "a walking officer only ever increases the separation once the player is stuck"
            );
            separation = now;
        }

        // Dragged as far as the wall let them, and no further.
        close(position.x, wall.min.x - TEST_HALF_SIZE.x - COLLISION_SKIN);
        assert!(
            separation > CUSTODY_LEASH_M as f32,
            "the separation has to pass the leash for the sim to end the hold, but it was {separation} m"
        );
    }

    /// `law_and_order.md` M4c: "fly mode ignores custody (developer flying is not
    /// a jailbreak)". The tether returns `None`, so the clamp is not merely
    /// generous — it is absent.
    #[test]
    fn flying_is_not_custody_so_the_clamp_never_touches_the_delta() {
        let anchor = Vec3::new(2.0, 0.0, 0.0);
        let state = PlayerCustodyState::held_at(anchor, 1, cathedral_sim::custody::STRAIN_BASE_SECONDS as f32);
        let start = Vec3::new(-3.0, 0.0, 0.0);
        let wish = Vec3::new(-1.0, 0.0, 0.0);

        assert_eq!(state.tether(true), None);
        assert_eq!(tethered_delta(start, wish, state.tether(true)), wish);
        assert_ne!(tethered_delta(start, wish, state.tether(false)), wish);
    }

    /// The universal case: nobody has hold of you, and the tether is not in the
    /// movement path at all.
    #[test]
    fn a_free_players_delta_is_handed_to_the_sweep_untouched() {
        let wish = Vec3::new(0.4, -0.1, 0.7);
        assert_eq!(
            tethered_delta(Vec3::new(12.0, 1.0, -30.0), wish, None),
            wish
        );
        // Inside the radius the clamp is a no-op too: turn, face them, face
        // away, circle — a held player keeps their feet inside the sphere.
        let anchor = Vec3::ZERO;
        let step = Vec3::new(0.1, 0.0, 0.0);
        assert_eq!(
            tethered_delta(
                Vec3::new(0.5, 0.0, 0.0),
                step,
                Some((anchor, CUSTODY_TETHER_M as f32))
            ),
            step
        );
    }

    /// The same property as the pure test above, but through the real
    /// fixed-step system, so the *order* inside `fixed_player_movement` is what
    /// is under test and not just the arithmetic it calls. (The system is a
    /// plain function, so the test drives it from `Update` rather than standing
    /// up Bevy's fixed-timestep plumbing.)
    #[test]
    fn the_fixed_step_solve_clamps_before_it_sweeps_so_a_held_player_ends_up_outside_the_wall() {
        let start = Vec3::new(-3.0, PLAYER_HALF_SIZE.y + COLLISION_SKIN, 0.0);
        let anchor = Vec3::new(2.0, start.y, 0.0);

        let mut app = App::new();
        let mut fixed = Time::<Fixed>::from_hz(FIXED_HZ);
        fixed.advance_by(Duration::from_secs_f64(1.0 / FIXED_HZ));
        app.insert_resource(fixed);
        app.init_resource::<ControllerInput>();

        let mut collision_world = CollisionWorld::default();
        collision_world.add_box(Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
        collision_world.add_box(Vec3::new(0.0, 0.0, -10.0), Vec3::new(0.4, 3.0, 10.0));
        app.insert_resource(collision_world);
        app.insert_resource(PlayerCustodyState::held_at(anchor, 1, cathedral_sim::custody::STRAIN_BASE_SECONDS as f32));
        app.world_mut().spawn((
            PlayerController::default(),
            PhysicalPosition {
                previous: start,
                current: start,
            },
        ));
        app.add_systems(Update, fixed_player_movement);

        for _ in 0..60 {
            app.update();
        }

        let world = app.world_mut();
        let position = world
            .query::<&PhysicalPosition>()
            .single(world)
            .expect("the player exists")
            .current;
        // Stopped at the wall's near face — the tether was dragging them at it
        // the whole time, and the sweep won every tick.
        close(position.x, -PLAYER_HALF_SIZE.x - COLLISION_SKIN);
        assert!(
            position.distance(anchor) > CUSTODY_TETHER_M as f32,
            "geometry beating the anchor is correct; being pinned inside it is not"
        );
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
