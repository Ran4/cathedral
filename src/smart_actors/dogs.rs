//! Street dogs: the render mirror of the sim's dog pack (`features/implemented/dogs.md`).
//!
//! The sim owns the dogs — the authored pack, the wander, every position —
//! and republishes the whole set on `EngineMessage::Dogs` whenever any dog
//! changes pose (the `Lamps` shape, at the movement tick's 20 Hz). This
//! module stands a small lofted quadruped up per dog, interpolates its root
//! between ticks exactly as `actors::drive_npc_bodies` does for people, and
//! swings the legs in a diagonal-pair trot off the sim's own `gait_phase`.
//!
//! Deliberately none of the person plumbing: no `ActorView` (reconcile never
//! sees a dog), no `ActorTarget` (the crosshair passes through), no name
//! label, no thinking indicator, no collider (`features/rats.md` §2.1 — a
//! moving collider punches holes in the walkable bake).

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use cathedral_sim::{DogCoat, MOVEMENT_TICK_SECONDS};

use super::body::{Caps, Ring, loft, merge_meshes};

// ---------------------------------------------------------------------------
// Proportions. Ground-local metres for a middling street dog (`build` = 1.0),
// hung under the root by a frame at −WALK_Y so the constants read from the
// ground up, like a dog. Shoulder ≈ 0.55 m; the sim's `build` (0.72 ratter to
// 1.2 mastiff) scales the whole frame uniformly.
// ---------------------------------------------------------------------------

/// The sim walk plane the root rides on; the frame hangs the body this far
/// back down so the paws land on the ground, mirroring `body::GROUND_Y`.
const FRAME_Y: f32 = -0.91;

/// Barrel centre height — low and long: a street dog, not a deer.
const BODY_Y: f32 = 0.40;
/// Where the leg pivots bury into the barrel.
const LEG_ROOT_Y: f32 = 0.44;
/// Upper leg: pivot to elbow/stifle.
const UPPER_LEG_LEN: f32 = 0.20;
/// Lower leg: elbow to sole, paw included.
const LOWER_LEG_LEN: f32 = 0.24;
/// Front and rear pivot stations along the barrel (−Z is forward).
const FRONT_LEG_Z: f32 = -0.24;
const REAR_LEG_Z: f32 = 0.26;
const LEG_X: f32 = 0.10;

/// Trot swing amplitude at the leg root, radians.
const LEG_SWING_RAD: f32 = 0.55;
/// The carpus/hock fold while a leg swings through the back of its arc.
const LOWER_FOLD_RAD: f32 = 0.7;
/// Barrel bob per trot beat — two beats per stride, like the diagonal pairs.
const BOB_AMPLITUDE_M: f32 = 0.012;
/// Speeds bracketing the settle→trot blend, mirroring the human walk blend.
const SETTLED_SPEED_MPS: f32 = 0.15;
const TROT_FULL_SPEED_MPS: f32 = 0.9;
const TROT_BLEND_SECONDS: f32 = 0.25;

const BODY_SECTORS: usize = 16;
const LIMB_SECTORS: usize = 10;
/// Flat colour coats: any tile scale works, this keeps the UVs sane.
const COAT_TILE_M: f32 = 0.5;

/// Dogs read at street level, not across the city: fade the pack out well
/// before the human crowd's 120–150 m (`body::crowd_fade`).
fn dog_fade() -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 90.0..110.0,
        use_aabb: false,
    }
}

/// One dog's latest hot-channel sample, plus the static facts the first
/// message carried (`DogView` resends them; spawn reads them once).
pub struct DogSample {
    pub name: String,
    pub coat: DogCoat,
    pub build: f32,
    pub position: Vec3,
    pub facing_yaw: f32,
    pub speed: f32,
    pub gait_phase: f32,
    pub seq: u64,
}

/// The projected pack, written by the bridge drain (`EngineMessage::Dogs`)
/// and consumed by [`sync_dogs`] / [`drive_dog_bodies`]. Keyed by the sim's
/// dog id.
#[derive(Resource, Default)]
pub struct DogInbox(pub HashMap<String, DogSample>);

/// Marker on a dog root, carrying its inbox key.
#[derive(Component)]
pub struct StreetDog {
    id: String,
}

/// The rig's animated parts, stored on the root so the gait system never
/// walks the hierarchy — `BodyRig`'s idiom.
#[derive(Component)]
pub(crate) struct DogRig {
    body: Entity,
    head: Entity,
    tail: Entity,
    /// Upper legs: front-left, front-right, rear-left, rear-right.
    uppers: [Entity; 4],
    /// Lower legs, same order.
    lowers: [Entity; 4],
}

/// Per-dog interpolation state — [`super::actors::NpcMotion`] with the gait
/// scalars riding along, because the same 20 Hz sample carries them and the
/// dog's whole animation wants one clock.
#[derive(Component)]
pub(crate) struct DogMotion {
    previous: Vec3,
    current: Vec3,
    prev_yaw: f32,
    cur_yaw: f32,
    prev_phase: f32,
    cur_phase: f32,
    speed: f32,
    t0: f64,
    seq: u64,
}

/// The settle→trot blend and this dog's own wag rhythm.
#[derive(Component)]
pub(crate) struct DogGait {
    blend: f32,
    wag_seed: f32,
}

/// Shared handles, built lazily on the first non-empty inbox (the lamp
/// assets' idiom — no startup system, no cost while the pack is empty).
pub(crate) struct DogAssets {
    barrel: Handle<Mesh>,
    head: Handle<Mesh>,
    upper_leg: Handle<Mesh>,
    lower_leg: Handle<Mesh>,
    tail: Handle<Mesh>,
    coats: Vec<(DogCoat, Handle<StandardMaterial>)>,
}

impl DogAssets {
    fn coat(&self, coat: DogCoat) -> Handle<StandardMaterial> {
        self.coats
            .iter()
            .find(|(kind, _)| *kind == coat)
            .map(|(_, handle)| handle.clone())
            .unwrap_or_else(|| self.coats[0].1.clone())
    }
}

// ---------------------------------------------------------------------------
// Meshes. The barrel and muzzle use the turnshoe trick — lofted along +Y,
// then laid down so +Y becomes −Z (forward) — and every part keeps the rig
// invariant: origin at the joint it rotates around.
// ---------------------------------------------------------------------------

/// Chest to rump in one loft, authored nose-first along +Y then laid down.
/// Origin at the barrel centre.
fn barrel_mesh() -> Mesh {
    loft(
        &[
            Ring::new(-0.36, 0.085, 0.100).at(0.0, -0.010),
            Ring::new(-0.20, 0.105, 0.130).boxy(2.4),
            Ring::new(0.00, 0.110, 0.140).boxy(2.4),
            Ring::new(0.16, 0.115, 0.150).at(0.0, 0.005).boxy(2.4),
            Ring::new(0.34, 0.090, 0.120).at(0.0, 0.010),
        ],
        BODY_SECTORS,
        Caps::BOTH,
        COAT_TILE_M,
    )
    .rotated_by(Quat::from_rotation_x(-FRAC_PI_2))
}

/// Neck, cranium, muzzle and both pricked ears, merged into one part. Origin
/// at the neck root on the barrel's front shoulder, so a head nod pivots
/// where a neck does.
fn head_mesh() -> Mesh {
    let neck = loft(
        &[
            Ring::new(-0.02, 0.055, 0.060).at(0.0, -0.020),
            Ring::new(0.06, 0.048, 0.052).at(0.0, -0.045),
            Ring::new(0.12, 0.045, 0.048).at(0.0, -0.070),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        COAT_TILE_M,
    );
    let cranium = loft(
        &[
            Ring::new(0.10, 0.050, 0.055).at(0.0, -0.090),
            Ring::new(0.16, 0.062, 0.065).at(0.0, -0.100),
            Ring::new(0.22, 0.050, 0.050).at(0.0, -0.100),
            Ring::new(0.245, 0.020, 0.020).at(0.0, -0.100),
        ],
        BODY_SECTORS,
        Caps::TOP,
        COAT_TILE_M,
    );
    let muzzle = loft(
        &[
            Ring::new(0.00, 0.036, 0.032).boxy(2.6),
            Ring::new(0.06, 0.030, 0.026).boxy(2.6),
            Ring::new(0.10, 0.024, 0.020).boxy(2.8),
        ],
        LIMB_SECTORS,
        Caps::TOP,
        COAT_TILE_M,
    )
    .rotated_by(Quat::from_rotation_x(-FRAC_PI_2))
    .translated_by(Vec3::new(0.0, 0.155, -0.10));
    let ear = |sign: f32| {
        loft(
            &[
                Ring::new(0.000, 0.020, 0.008),
                Ring::new(0.050, 0.012, 0.005),
                Ring::new(0.075, 0.004, 0.003),
            ],
            8,
            Caps::TOP,
            COAT_TILE_M,
        )
        .rotated_by(Quat::from_rotation_z(sign * 0.25) * Quat::from_rotation_x(-0.15))
        .translated_by(Vec3::new(sign * 0.035, 0.230, -0.080))
    };
    merge_meshes([neck, cranium, muzzle, ear(1.0), ear(-1.0)])
}

/// Shoulder/hip to elbow/stifle, hanging −Y — the human limb convention.
fn upper_leg_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.03, 0.050, 0.055),
            Ring::new(-0.09, 0.042, 0.050),
            Ring::new(-UPPER_LEG_LEN, 0.030, 0.034),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        COAT_TILE_M,
    )
}

/// Elbow to the ground, the paw leaning forward off the pastern.
fn lower_leg_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.02, 0.026, 0.030),
            Ring::new(-0.10, 0.020, 0.024),
            Ring::new(-0.20, 0.024, 0.030).at(0.0, -0.008).boxy(3.0),
            Ring::new(-LOWER_LEG_LEN, 0.026, 0.038)
                .at(0.0, -0.016)
                .boxy(3.4),
        ],
        LIMB_SECTORS,
        Caps::BOTTOM,
        COAT_TILE_M,
    )
}

/// Authored hanging −Y like a limb; the rig's rest rotation carries it up and
/// back, and the wag plays about the root's own axis.
fn tail_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.00, 0.022, 0.022),
            Ring::new(-0.14, 0.016, 0.016),
            Ring::new(-0.26, 0.009, 0.009),
        ],
        8,
        Caps::BOTTOM,
        COAT_TILE_M,
    )
}

/// The tail's rest pose: carried back off the rump with a slight rise — a
/// street dog's easy line, not a hound at point.
fn tail_rest_rotation() -> Quat {
    Quat::from_rotation_x(-2.1)
}

fn coat_color(coat: DogCoat) -> Color {
    match coat {
        DogCoat::Brindle => Color::srgb(0.42, 0.32, 0.22),
        DogCoat::Black => Color::srgb(0.09, 0.08, 0.08),
        DogCoat::Grey => Color::srgb(0.45, 0.45, 0.47),
        DogCoat::Fawn => Color::srgb(0.62, 0.48, 0.30),
        DogCoat::White => Color::srgb(0.82, 0.80, 0.75),
        DogCoat::Pied => Color::srgb(0.35, 0.33, 0.30),
    }
}

fn build_assets(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> DogAssets {
    let coats = [
        DogCoat::Brindle,
        DogCoat::Black,
        DogCoat::Grey,
        DogCoat::Fawn,
        DogCoat::White,
        DogCoat::Pied,
    ]
    .into_iter()
    .map(|coat| {
        (
            coat,
            materials.add(StandardMaterial {
                base_color: coat_color(coat),
                perceptual_roughness: 0.9,
                ..default()
            }),
        )
    })
    .collect();
    DogAssets {
        barrel: meshes.add(barrel_mesh()),
        head: meshes.add(head_mesh()),
        upper_leg: meshes.add(upper_leg_mesh()),
        lower_leg: meshes.add(lower_leg_mesh()),
        tail: meshes.add(tail_mesh()),
        coats,
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Stand a body up for every dog the inbox knows and no entity mirrors yet.
/// The pack is fixed for the session (the sim seeds it once), so there is no
/// despawn arm — a dog that stops moving simply rests.
pub fn sync_dogs(
    mut commands: Commands,
    inbox: Res<DogInbox>,
    margin: Option<Res<crate::city::CutMarginProfile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<&StreetDog>,
    mut assets: Local<Option<DogAssets>>,
) {
    if !inbox.is_changed() || inbox.0.is_empty() {
        return;
    }
    let spawned: std::collections::HashSet<&str> =
        existing.iter().map(|dog| dog.id.as_str()).collect();
    if spawned.len() == inbox.0.len() {
        return;
    }
    let assets = assets.get_or_insert_with(|| build_assets(&mut meshes, &mut materials));

    for (id, sample) in &inbox.0 {
        if spawned.contains(id.as_str()) {
            continue;
        }
        spawn_dog(&mut commands, assets, margin.as_deref(), id, sample);
    }
}

fn spawn_dog(
    commands: &mut Commands,
    assets: &DogAssets,
    margin: Option<&crate::city::CutMarginProfile>,
    id: &str,
    sample: &DogSample,
) {
    let coat = assets.coat(sample.coat);
    let mut translation = sample.position;
    if let Some(profile) = margin {
        translation.y += profile.ground_lift(translation.x, translation.z);
    }
    let mut rig = DogRig {
        body: Entity::PLACEHOLDER,
        head: Entity::PLACEHOLDER,
        tail: Entity::PLACEHOLDER,
        uppers: [Entity::PLACEHOLDER; 4],
        lowers: [Entity::PLACEHOLDER; 4],
    };
    let root = commands
        .spawn((
            Name::new(format!("Street dog: {}", sample.name)),
            StreetDog { id: id.to_string() },
            DogGait {
                blend: 0.0,
                // A per-dog phase so ten tails never beat as one metronome.
                wag_seed: id
                    .bytes()
                    .fold(0.0_f32, |seed, byte| (seed + f32::from(byte) * 0.37) % TAU),
            },
            Transform::from_translation(translation)
                .with_rotation(Quat::from_rotation_y(sample.facing_yaw)),
            Visibility::default(),
        ))
        .id();
    let part = |commands: &mut Commands, mesh: &Handle<Mesh>, transform: Transform| {
        commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(coat.clone()),
                transform,
                dog_fade(),
            ))
            .id()
    };
    // The frame drops the ground to the paws and carries the build scale, so
    // every constant above stays a ground-up metre and the root's transform
    // stays the sim's.
    let frame = commands
        .spawn((
            Transform::from_xyz(0.0, FRAME_Y, 0.0).with_scale(Vec3::splat(sample.build)),
            Visibility::default(),
        ))
        .id();
    commands.entity(frame).insert(ChildOf(root));

    rig.body = part(
        commands,
        &assets.barrel,
        Transform::from_xyz(0.0, BODY_Y, 0.0),
    );
    commands.entity(rig.body).insert(ChildOf(frame));
    rig.head = part(
        commands,
        &assets.head,
        Transform::from_xyz(0.0, 0.09, -0.32),
    );
    commands.entity(rig.head).insert(ChildOf(rig.body));
    rig.tail = part(
        commands,
        &assets.tail,
        Transform::from_xyz(0.0, 0.06, 0.34).with_rotation(tail_rest_rotation()),
    );
    commands.entity(rig.tail).insert(ChildOf(rig.body));

    let stations = [
        (LEG_X, FRONT_LEG_Z),
        (-LEG_X, FRONT_LEG_Z),
        (LEG_X + 0.005, REAR_LEG_Z),
        (-LEG_X - 0.005, REAR_LEG_Z),
    ];
    for (index, (x, z)) in stations.into_iter().enumerate() {
        let upper = part(
            commands,
            &assets.upper_leg,
            Transform::from_xyz(x, LEG_ROOT_Y, z),
        );
        commands.entity(upper).insert(ChildOf(frame));
        let lower = part(
            commands,
            &assets.lower_leg,
            Transform::from_xyz(0.0, -UPPER_LEG_LEN, 0.0),
        );
        commands.entity(lower).insert(ChildOf(upper));
        rig.uppers[index] = upper;
        rig.lowers[index] = lower;
    }
    commands.entity(root).insert(rig);
}

/// Sweep each dog root between its 20 Hz samples — `drive_npc_bodies` for the
/// pack, including the Cut margin lift at the interpolated XZ.
pub fn drive_dog_bodies(
    mut commands: Commands,
    time: Res<Time>,
    inbox: Res<DogInbox>,
    margin: Option<Res<crate::city::CutMarginProfile>>,
    mut dogs: Query<(Entity, &StreetDog, &mut Transform, Option<&mut DogMotion>)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, dog, mut transform, motion) in &mut dogs {
        let Some(sample) = inbox.0.get(&dog.id) else {
            continue;
        };
        match motion {
            None => {
                commands.entity(entity).insert(DogMotion {
                    previous: Vec3::new(
                        transform.translation.x,
                        sample.position.y,
                        transform.translation.z,
                    ),
                    current: sample.position,
                    prev_yaw: sample.facing_yaw,
                    cur_yaw: sample.facing_yaw,
                    prev_phase: sample.gait_phase,
                    cur_phase: sample.gait_phase,
                    speed: sample.speed,
                    t0: now,
                    seq: sample.seq,
                });
            }
            Some(mut motion) => {
                if sample.seq != motion.seq {
                    motion.previous = motion.current;
                    motion.prev_yaw = motion.cur_yaw;
                    motion.prev_phase = motion.cur_phase;
                    motion.current = sample.position;
                    motion.cur_yaw = sample.facing_yaw;
                    motion.cur_phase = sample.gait_phase;
                    motion.speed = sample.speed;
                    motion.t0 = now;
                    motion.seq = sample.seq;
                }
                let t = ((now - motion.t0) / MOVEMENT_TICK_SECONDS).clamp(0.0, 1.0) as f32;
                let mut translation = motion.previous.lerp(motion.current, t);
                if let Some(profile) = margin.as_ref() {
                    translation.y += profile.ground_lift(translation.x, translation.z);
                }
                let rotation =
                    Quat::from_rotation_y(lerp_angle(motion.prev_yaw, motion.cur_yaw, t));
                if transform.translation != translation || transform.rotation != rotation {
                    transform.translation = translation;
                    transform.rotation = rotation;
                }
            }
        }
    }
}

/// The trot: diagonal pairs — front-left with rear-right — swinging off the
/// sim's `gait_phase`, the lower leg folding through the back of its arc, a
/// two-beat barrel bob, a wag that livens with speed, and a slow idle head
/// sway while resting. Pure cosmetics on part transforms; the root belongs to
/// [`drive_dog_bodies`].
pub fn animate_dog_gait(
    time: Res<Time>,
    mut dogs: Query<(&DogRig, &mut DogGait, Option<&DogMotion>)>,
    mut parts: Query<&mut Transform, With<Mesh3d>>,
) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    for (rig, mut gait, motion) in &mut dogs {
        let (phase, speed) = match motion {
            Some(motion) => {
                let t =
                    ((f64::from(now) - motion.t0) / MOVEMENT_TICK_SECONDS).clamp(0.0, 1.0) as f32;
                (
                    motion.prev_phase + (motion.cur_phase - motion.prev_phase) * t,
                    motion.speed,
                )
            }
            None => (0.0, 0.0),
        };
        let target = smoothstep(SETTLED_SPEED_MPS, TROT_FULL_SPEED_MPS, speed);
        gait.blend = move_toward(gait.blend, target, dt / TROT_BLEND_SECONDS);
        let blend = gait.blend;

        let cycle = phase * TAU;
        let swing = cycle.sin();
        // Diagonal pairing: FL and RR lead, FR and RL counter.
        let pair_sign = [1.0, -1.0, -1.0, 1.0];
        for (index, sign) in pair_sign.into_iter().enumerate() {
            let leg_swing = sign * swing * LEG_SWING_RAD * blend;
            if let Ok(mut upper) = parts.get_mut(rig.uppers[index]) {
                upper.rotation = Quat::from_rotation_x(leg_swing);
            }
            // Fold through the back of the arc, like the human shin gate.
            let fold = (-sign * swing).max(0.0) * LOWER_FOLD_RAD * blend;
            if let Ok(mut lower) = parts.get_mut(rig.lowers[index]) {
                lower.rotation = Quat::from_rotation_x(-fold);
            }
        }

        if let Ok(mut body) = parts.get_mut(rig.body) {
            let bob = (cycle * 2.0).sin().abs() * BOB_AMPLITUDE_M * blend;
            body.translation.y = BODY_Y + bob;
        }
        if let Ok(mut head) = parts.get_mut(rig.head) {
            // A trot nod when moving; a slow scenting sway when not.
            let nod = (cycle * 2.0).sin() * 0.06 * blend;
            let sway = ((now * 0.4 + gait.wag_seed) * TAU * 0.1).sin() * 0.12 * (1.0 - blend);
            head.rotation =
                Quat::from_rotation_x(nod + sway.abs() * 0.5) * Quat::from_rotation_y(sway);
        }
        if let Ok(mut tail) = parts.get_mut(rig.tail) {
            let wag_hz = 1.2 + 1.6 * blend;
            let wag_amp = 0.22 + 0.33 * blend;
            let wag = (now * wag_hz * TAU + gait.wag_seed).sin() * wag_amp;
            tail.rotation = tail_rest_rotation() * Quat::from_rotation_z(wag);
        }
    }
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn move_toward(current: f32, target: f32, max_step: f32) -> f32 {
    current + (target - current).clamp(-max_step, max_step)
}

/// Shortest-arc yaw interpolation — `actors::lerp_angle`'s twin.
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let mut delta = (to - from) % TAU;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    from + delta * t
}
