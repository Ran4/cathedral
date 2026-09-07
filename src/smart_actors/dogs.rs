//! Street dogs: the render mirror of the sim's dog pack (`features/implemented/dogs.md`).
//!
//! The sim owns the dogs — the authored pack, the wander, every position —
//! and republishes the whole set on `EngineMessage::Dogs` whenever any dog
//! changes pose (the `Lamps` shape, at the movement tick's 20 Hz). This
//! module stands a small lofted quadruped up per dog, interpolates its root
//! between ticks exactly as `actors::drive_npc_bodies` does for people, and
//! plants the paws through a walk/trot off the sim's own distance clock.
//! Quiet observation adds complete, interruptible actions without sim events.
//!
//! Deliberately none of the person plumbing: no `ActorView` (reconcile never
//! sees a dog), no `ActorTarget` (the crosshair passes through), no name
//! label, no thinking indicator, no collider (`features/rats.md` §2.1 — a
//! moving collider punches holes in the walkable bake).

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use bevy::camera::visibility::DynamicSkinnedMeshBounds;
use bevy::camera::visibility::VisibilityRange;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use cathedral_sim::{DogCoat, MOVEMENT_TICK_SECONDS};

#[path = "dogs/motion.rs"]
mod animation;
#[path = "dogs/appearance.rs"]
mod appearance;
#[path = "dogs/coat_surface.rs"]
mod coat_surface;
#[path = "dogs/geometry.rs"]
mod geometry;

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
/// Time to finish an interrupted pose and settle the next gait.
const TROT_BLEND_SECONDS: f32 = 0.25;

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
    frame: Entity,
    body: Entity,
    neck: Entity,
    /// Empty pivot at the atlas joint, above the merged lower neck. The skull
    /// mesh is offset below this pivot so the neutral geometry stays exact.
    head: Entity,
    jaw: Entity,
    ears: [Entity; 2],
    eyes: Entity,
    tail: Entity,
    /// Upper legs: front-left, front-right, rear-left, rear-right.
    uppers: [Entity; 4],
    /// Lower legs, same order.
    lowers: [Entity; 4],
    paws: [Entity; 4],
    hocks: [Entity; 2],
    stations: [Vec3; 4],
    build: f32,
}

/// Animated transform, including the empty skull joint. Mesh filtering would
/// silently exclude that joint and prevent the head from following attention.
#[derive(Component)]
pub(crate) struct DogPart;

/// Atlas joint in the skull's authored coordinates. At build 1 it lies at
/// ground-local (0, 0.59, -0.37). The lower neck has its own joint and blended
/// skin weights across the continuous front torso rings.
const SKULL_PIVOT: Vec3 = Vec3::new(0.0, 0.12, -0.10);
const JAW_PIVOT: Vec3 = Vec3::new(0.0, 0.122, -0.145);

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
    animation: animation::State,
}

/// Shared handles, built lazily on the first non-empty inbox (the lamp
/// assets' idiom — no startup system, no cost while the pack is empty).
pub(crate) struct DogAssets {
    forms: Vec<(DogCoat, appearance::Form)>,
    fur: Handle<StandardMaterial>,
    brindle_fur: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    eyes: Handle<StandardMaterial>,
    paw_bind: [Handle<SkinnedMeshInverseBindposes>; 2],
    jaw_bind: Handle<SkinnedMeshInverseBindposes>,
}

impl DogAssets {
    fn form(&self, coat: DogCoat) -> &appearance::Form {
        &self
            .forms
            .iter()
            .find(|(kind, _)| *kind == coat)
            .unwrap_or(&self.forms[0])
            .1
    }
}

fn tail_rest_rotation() -> Quat {
    Quat::IDENTITY
}

fn build_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    poses: &mut Assets<SkinnedMeshInverseBindposes>,
) -> DogAssets {
    let features = meshes.add(appearance::face_mesh());
    let eyes = meshes.add(appearance::eye_mesh());
    DogAssets {
        jaw_bind: poses.add(SkinnedMeshInverseBindposes::from(vec![
            Mat4::IDENTITY,
            Mat4::from_translation(-JAW_PIVOT),
        ])),
        forms: [
            DogCoat::Brindle,
            DogCoat::Black,
            DogCoat::Grey,
            DogCoat::Fawn,
            DogCoat::White,
            DogCoat::Pied,
        ]
        .into_iter()
        .map(|coat| {
            let mut form = appearance::build(coat, meshes, features.clone(), eyes.clone());
            form.neck_bind = poses.add(SkinnedMeshInverseBindposes::from(vec![
                Mat4::IDENTITY,
                Mat4::from_translation(-neck_pivot(form.length)),
            ]));
            form.upper_bind = [
                Vec3::new(0.088 * form.width, 0.445, -0.235 * form.length),
                Vec3::new(-0.088 * form.width, 0.445, -0.235 * form.length),
                Vec3::new(0.090 * form.width, 0.455, 0.255 * form.length),
                Vec3::new(-0.090 * form.width, 0.455, 0.255 * form.length),
            ]
            .map(|station| {
                let mut matrices = vec![
                    Mat4::from_translation(station - Vec3::Y * BODY_Y),
                    Mat4::IDENTITY,
                ];
                if station.z > 0.0 {
                    matrices.push(Mat4::from_translation(-animation::elbow(true)));
                }
                poses.add(SkinnedMeshInverseBindposes::from(matrices))
            });
            (coat, form)
        })
        .collect(),
        fur: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(images.add(coat_surface::image(false))),
            perceptual_roughness: 0.94,
            reflectance: 0.22,
            ..default()
        }),
        brindle_fur: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(images.add(coat_surface::image(true))),
            perceptual_roughness: 0.94,
            reflectance: 0.22,
            ..default()
        }),
        skin: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.54,
            reflectance: 0.36,
            ..default()
        }),
        eyes: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.34,
            reflectance: 0.50,
            ..default()
        }),
        paw_bind: [false, true].map(|rear| {
            let mut matrices = vec![Mat4::IDENTITY];
            if rear {
                matrices.push(Mat4::from_translation(-animation::hock()));
            }
            matrices.push(Mat4::from_translation(-animation::ankle(rear)));
            poses.add(SkinnedMeshInverseBindposes::from(matrices))
        }),
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
    mut images: ResMut<Assets<Image>>,
    mut poses: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    existing: Query<&StreetDog>,
    mut assets: Local<Option<DogAssets>>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Dogs);
    if !inbox.is_changed() || inbox.0.is_empty() {
        return;
    }
    if existing.iter().len() == inbox.0.len() {
        return;
    }
    let assets = assets
        .get_or_insert_with(|| build_assets(&mut meshes, &mut materials, &mut images, &mut poses));

    for (id, sample) in &inbox.0 {
        if existing.iter().any(|dog| dog.id == *id) {
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
    let form = assets.form(sample.coat);
    let fur = if sample.coat == DogCoat::Brindle {
        &assets.brindle_fur
    } else {
        &assets.fur
    };
    let mut translation = sample.position;
    if let Some(profile) = margin {
        translation.y += profile.ground_lift(translation.x, translation.z);
    }
    let mut rig = DogRig {
        frame: Entity::PLACEHOLDER,
        body: Entity::PLACEHOLDER,
        neck: Entity::PLACEHOLDER,
        head: Entity::PLACEHOLDER,
        jaw: Entity::PLACEHOLDER,
        ears: [Entity::PLACEHOLDER; 2],
        eyes: Entity::PLACEHOLDER,
        tail: Entity::PLACEHOLDER,
        uppers: [Entity::PLACEHOLDER; 4],
        lowers: [Entity::PLACEHOLDER; 4],
        paws: [Entity::PLACEHOLDER; 4],
        hocks: [Entity::PLACEHOLDER; 2],
        stations: [Vec3::ZERO; 4],
        build: sample.build,
    };
    let root = commands
        .spawn((
            Name::new(format!("Street dog: {}", sample.name)),
            StreetDog { id: id.to_string() },
            DogGait {
                blend: 0.0,
                animation: animation::State::new(id),
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
                MeshMaterial3d(fur.clone()),
                DogPart,
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
            DogPart,
            Transform::from_xyz(0.0, FRAME_Y, 0.0).with_scale(Vec3::splat(sample.build)),
            Visibility::default(),
        ))
        .id();
    commands.entity(frame).insert(ChildOf(root));
    rig.frame = frame;

    rig.body = part(commands, &form.body, Transform::from_xyz(0.0, BODY_Y, 0.0));
    commands.entity(rig.body).insert(ChildOf(frame));
    rig.neck = commands
        .spawn((
            DogPart,
            Name::new("Dog neck joint"),
            Transform::from_translation(neck_pivot(form.length)),
            Visibility::default(),
            ChildOf(rig.body),
        ))
        .id();
    commands.entity(rig.body).insert((
        SkinnedMesh {
            inverse_bindposes: form.neck_bind.clone(),
            joints: vec![rig.body, rig.neck],
        },
        DynamicSkinnedMeshBounds,
    ));
    let skull = part(
        commands,
        &form.head,
        Transform::from_translation(-SKULL_PIVOT),
    );
    rig.head = commands
        .spawn((
            DogPart,
            Name::new("Dog skull joint"),
            Transform::from_translation(
                Vec3::new(0.0, 0.07, -0.27 * form.length) + SKULL_PIVOT * form.head_scale
                    - neck_pivot(form.length),
            )
            .with_scale(form.head_scale),
            Visibility::default(),
            ChildOf(rig.neck),
        ))
        .id();
    commands.entity(skull).insert(ChildOf(rig.head));
    rig.jaw = commands
        .spawn((
            DogPart,
            Name::new("Dog jaw joint"),
            Transform::from_translation(JAW_PIVOT),
            Visibility::default(),
            ChildOf(skull),
        ))
        .id();
    commands.entity(skull).insert((
        SkinnedMesh {
            inverse_bindposes: assets.jaw_bind.clone(),
            joints: vec![skull, rig.jaw],
        },
        DynamicSkinnedMeshBounds,
    ));
    commands
        .entity(skull)
        .insert(MeshMaterial3d(assets.fur.clone()));
    rig.tail = part(
        commands,
        &form.tail,
        Transform::from_xyz(0.0, 0.06, 0.34 * form.length).with_rotation(tail_rest_rotation()),
    );
    commands.entity(rig.tail).insert(ChildOf(rig.body));

    for (mesh, material) in [(&form.features, &assets.skin), (&form.eyes, &assets.eyes)] {
        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                DogPart,
                Transform::IDENTITY,
                dog_fade(),
                ChildOf(skull),
            ))
            .id();
        if mesh == &form.eyes {
            rig.eyes = entity;
        }
    }
    for (index, sign) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let ear = part(
            commands,
            &form.ears[index],
            Transform::from_xyz(sign * 0.061, 0.226, -0.066)
                .with_rotation(Quat::from_rotation_z(sign * -0.12)),
        );
        commands.entity(ear).insert(ChildOf(skull));
        rig.ears[index] = ear;
        commands
            .entity(ear)
            .insert(MeshMaterial3d(assets.fur.clone()));
    }

    let stations = [
        (0.088 * form.width, -0.235 * form.length),
        (-0.088 * form.width, -0.235 * form.length),
        (0.090 * form.width, 0.255 * form.length),
        (-0.090 * form.width, 0.255 * form.length),
    ];
    for (index, (x, z)) in stations.into_iter().enumerate() {
        let rear = index >= 2;
        let upper = part(
            commands,
            if rear {
                &form.rear_upper[index % 2]
            } else {
                &form.front_upper[index % 2]
            },
            Transform::from_xyz(x, if rear { 0.455 } else { 0.445 }, z),
        );
        commands.entity(upper).insert(ChildOf(frame));
        commands.entity(upper).insert((
            SkinnedMesh {
                inverse_bindposes: form.upper_bind[index].clone(),
                joints: vec![rig.body, upper],
            },
            DynamicSkinnedMeshBounds,
        ));
        let lower = part(
            commands,
            if rear {
                &form.rear_lower
            } else {
                &form.front_lower
            },
            Transform::from_xyz(
                0.0,
                if rear { -0.17 } else { -0.19 },
                if rear { -0.07 } else { 0.02 },
            ),
        );
        commands.entity(lower).insert(ChildOf(upper));
        if rear {
            commands.entity(upper).insert(SkinnedMesh {
                inverse_bindposes: form.upper_bind[index].clone(),
                joints: vec![rig.body, upper, lower],
            });
        }
        let paw_parent = if rear {
            let hock = commands
                .spawn((
                    DogPart,
                    Name::new(if index == 2 {
                        "Dog hock RL"
                    } else {
                        "Dog hock RR"
                    }),
                    Transform::from_translation(animation::hock()),
                    Visibility::default(),
                    ChildOf(lower),
                ))
                .id();
            rig.hocks[index - 2] = hock;
            hock
        } else {
            lower
        };
        let paw = commands
            .spawn((
                DogPart,
                Name::new(["Dog paw FL", "Dog paw FR", "Dog paw RL", "Dog paw RR"][index]),
                Transform::from_translation(
                    animation::ankle(rear) - if rear { animation::hock() } else { Vec3::ZERO },
                ),
                Visibility::default(),
                ChildOf(paw_parent),
            ))
            .id();
        commands.entity(lower).insert((
            SkinnedMesh {
                inverse_bindposes: assets.paw_bind[usize::from(rear)].clone(),
                joints: if rear {
                    vec![lower, paw_parent, paw]
                } else {
                    vec![lower, paw]
                },
            },
            DynamicSkinnedMeshBounds,
        ));
        rig.uppers[index] = upper;
        rig.lowers[index] = lower;
        rig.paws[index] = paw;
        rig.stations[index] = Vec3::new(x, if rear { 0.455 } else { 0.445 }, z);
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
    let _span = crate::perf::span(crate::perf::Probe::Dogs);
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

pub use animation::animate_dog_gait;

fn neck_pivot(length: f32) -> Vec3 {
    Vec3::new(0.0, 0.03, -0.265 * length)
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
