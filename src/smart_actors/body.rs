//! The articulated puppet (`features/npc_bodies.md` §2 M0, §4–§5 + §9 M1).
//!
//! Replaces the capsule+head+nose stand-in with a 11–12-part primitive rig:
//! `root → pelvis → torso → head (→ headgear)`, `pelvis → thighs → shins`,
//! `torso → upper arms → forearms (→ hand anchors)`. Joints are plain entity
//! parenting; every shared mesh is authored with its pivot baked at the joint
//! (a thigh capsule's origin is the hip, not its centre) so pose systems
//! rotate part `Transform`s directly. The root stays the sim's — reconcile and
//! `drive_npc_bodies` own its translation/yaw — and all pose lives on parts.
//!
//! Everything visual is a **shared handle**: 7 outfit-class textures × a small
//! quantized tint band, 3 bespoke-major tints, 24 clamped face materials, and
//! 4 headgear looks. 514 actors draw from that fixed set so batching holds.
//!
//! The pose pipeline (M1) lives at the bottom of this module:
//! [`animate_body_pose`] evaluates layered joint deltas over the authored rest
//! pose — L0 locomotion from the sim's `speed`/`gait_phase`, L1 idle life when
//! settled, and in Tier A the upper layers: L2 activity (carry/offer, M2),
//! L4 speech & gaze (M3: talk gesticulation and head tracking, fed by
//! [`ReflexState`] from the presentation messages), and L3 one-shot gestures
//! (nod/head-shake) played over the gaze so a communicative beat stays
//! readable. M4 extends the one-shot catalog.

use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Instant;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::VisibilityRange;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, Mesh, MeshBuilder, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use cathedral_sim::{
    AppearanceSnapshot, Build, Headgear, MOVEMENT_TICK_SECONDS, OutfitClass, SETTLED_SPEED_MPS,
};

use crate::controller::PlayerCamera;
use crate::materials::load_repeating_texture;

use super::actors::{ActorOutfit, ActorView};
use super::model::{ActorId, MovementInbox, WorldMirror};

// ---------------------------------------------------------------------------
// Proportions. All root-local metres; the root sits on the sim walk plane
// (`WALK_Y` = 0.91 above ground), so ground is y = −0.91 and the head top ends
// at ≈ +0.92 (≈ 1.83 m tall), matching the retired capsule's silhouette.
// ---------------------------------------------------------------------------

const PELVIS_SIZE: Vec3 = Vec3::new(0.30, 0.19, 0.20);
const TORSO_JOINT_Y: f32 = 0.05;
const TORSO_HEIGHT: f32 = 0.52;
/// Torso cross-sections are ellipses; depth = width × this squash.
const TORSO_DEPTH_RATIO: f32 = 0.62;
/// Neck pivot (torso-local): the head nods around this, not its centre.
const NECK_JOINT_Y: f32 = 0.47;
/// Head centre above the neck pivot (baked into the head mesh).
const HEAD_CENTER_ABOVE_NECK: f32 = 0.16;
const HEAD_RADIUS: f32 = 0.24;
/// The head mesh and its headgear are authored at natural scale, then the head
/// joint renders them at [`HEAD_SCALE`]. The retired capsule sized the head at
/// its full [`HEAD_RADIUS`], which reads as an oversized ball; halving it about
/// the neck pivot shrinks the mesh, the face and any headgear together and
/// seats the smaller head down onto the shoulders (M6 npc_bodies follow-up).
const HEAD_SCALE: f32 = 0.5;
/// Raise the whole head joint (mesh, face, headgear and its nod pivot together)
/// this far above the neck so the halved head sits a touch higher on the neck.
const HEAD_LIFT: f32 = 0.06;
const SHOULDER_X: f32 = 0.265;
const SHOULDER_Y: f32 = 0.44;
const UPPER_ARM_RADIUS: f32 = 0.062;
const UPPER_ARM_LENGTH: f32 = 0.26;
const ELBOW_Y: f32 = -0.31;
const FOREARM_RADIUS: f32 = 0.052;
const FOREARM_LENGTH: f32 = 0.235;
const HAND_ANCHOR_Y: f32 = -0.27;
const HIP_X: f32 = 0.095;
const HIP_Y: f32 = -0.09;
const THIGH_RADIUS: f32 = 0.075;
const THIGH_LENGTH: f32 = 0.34;
const KNEE_Y: f32 = -0.42;
const SHIN_RADIUS: f32 = 0.058;
const SHIN_LENGTH: f32 = 0.34;

/// Neutral standing pose: arms hang with a slight outward tilt and elbow bend,
/// thighs splay a touch so the feet stand a little apart.
const ARM_HANG_TILT_RAD: f32 = 0.10;
const ELBOW_BEND_RAD: f32 = 0.28;
const THIGH_SPLAY_RAD: f32 = 0.03;

/// Physical metres one outfit-cloth texture tile covers. Part UVs are scaled
/// by their physical size over this so the weave reads consistently at body
/// scale across torso, limbs and pelvis.
const CLOTH_TILE_M: f32 = 0.85;

/// The face image spans the front hemisphere: its frame edge lands this many
/// radians away from the front pole (−Z) of the head sphere.
const FACE_EDGE_ANGLE_RAD: f32 = FRAC_PI_2;
/// UV radius cap for the rear of the head. Kept > √2 so triangle chords near
/// the back pole can never cut back into the [0,1]² face frame, while the
/// clamp-to-edge sampler paints everything out there in the image's uniform
/// skin-tone edge pixels.
const FACE_UV_RHO_MAX: f32 = 1.8;

const OUTFIT_TINT_BAND: usize = 4;
const FACE_COUNT: usize = 24;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodySide {
    Left,
    Right,
}

/// Empty attachment point in a forearm — the successor of the over-head
/// `OfferAnchor`. `super::hands` parents carried/offered props here (M2).
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct HandAnchor {
    pub actor: ActorId,
    pub side: BodySide,
}

/// The crowd's shared dither-fade (M7 of the movement feature,
/// `features/performance_improvements.md` item 8): every actor mesh part and
/// every hand prop carries a clone — Bevy's range check only considers
/// entities that carry it — and it culls the light views too, so a faded NPC
/// leaves the sun's shadow pass along with the render.
pub(super) fn crowd_fade() -> VisibilityRange {
    VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 120.0..150.0,
        use_aabb: false,
    }
}

/// Marks the head part carrying an actor's face material, so reconcile can
/// hot-swap it alongside the outfit if the appearance ever changes.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorFace(pub(crate) ActorId);

/// All part entities of one puppet, stored on the actor root so pose systems
/// (M1+) never walk the hierarchy by name.
#[derive(Component, Debug)]
#[allow(dead_code, /* the pose pipeline (M1) and hands (M2) read the parts */)]
pub struct BodyRig {
    pub pelvis: Entity,
    pub torso: Entity,
    pub head: Entity,
    pub headgear: Option<Entity>,
    pub left_upper_arm: Entity,
    pub right_upper_arm: Entity,
    pub left_forearm: Entity,
    pub right_forearm: Entity,
    pub left_thigh: Entity,
    pub right_thigh: Entity,
    pub left_shin: Entity,
    pub right_shin: Entity,
    pub left_hand: Entity,
    pub right_hand: Entity,
}

// ---------------------------------------------------------------------------
// Shared assets
// ---------------------------------------------------------------------------

/// The bounded shared-handle set every puppet draws from.
#[derive(Resource)]
pub(crate) struct BodyAssets {
    pelvis_mesh: Handle<Mesh>,
    torso_mesh: Handle<Mesh>,
    head_mesh: Handle<Mesh>,
    upper_arm_mesh: Handle<Mesh>,
    forearm_mesh: Handle<Mesh>,
    thigh_mesh: Handle<Mesh>,
    shin_mesh: Handle<Mesh>,
    hood_mesh: Handle<Mesh>,
    coif_mesh: Handle<Mesh>,
    brim_mesh: Handle<Mesh>,
    kettle_helm_mesh: Handle<Mesh>,
    /// `[class][tint]` — 7 textured cloth bands × the quantized tint band.
    outfits: [[Handle<StandardMaterial>; OUTFIT_TINT_BAND]; 7],
    /// The named majors' legacy colors as tints over their class texture.
    sven_outfit: Handle<StandardMaterial>,
    conny_outfit: Handle<StandardMaterial>,
    ilse_outfit: Handle<StandardMaterial>,
    faces: Vec<Handle<StandardMaterial>>,
    hood_material: Handle<StandardMaterial>,
    coif_material: Handle<StandardMaterial>,
    felt_material: Handle<StandardMaterial>,
    iron_material: Handle<StandardMaterial>,
}

impl BodyAssets {
    /// The outfit material every cloth part of this appearance wears.
    pub(crate) fn outfit_material(
        &self,
        appearance: &AppearanceSnapshot,
    ) -> Handle<StandardMaterial> {
        match appearance.bespoke.as_deref() {
            Some("sven") => return self.sven_outfit.clone(),
            Some("conny") => return self.conny_outfit.clone(),
            Some("ilse") => return self.ilse_outfit.clone(),
            _ => {}
        }
        self.outfits[class_index(appearance.outfit)][tint_index(appearance.palette_seed)].clone()
    }

    /// The face material for this appearance (deterministic per palette seed).
    pub(crate) fn face_material(
        &self,
        appearance: &AppearanceSnapshot,
    ) -> Handle<StandardMaterial> {
        self.faces[face_index(appearance.palette_seed)].clone()
    }

    /// Mesh + material of the appearance's headgear, if any. The hood wears
    /// its own neutral cloth (not the outfit band): the shell is open-bottomed
    /// and needs a double-sided material.
    fn headgear_visual(
        &self,
        appearance: &AppearanceSnapshot,
    ) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
        match appearance.headgear {
            Headgear::None => None,
            Headgear::Hood => Some((self.hood_mesh.clone(), self.hood_material.clone())),
            Headgear::Coif => Some((self.coif_mesh.clone(), self.coif_material.clone())),
            Headgear::Brim => Some((self.brim_mesh.clone(), self.felt_material.clone())),
            Headgear::KettleHelm => {
                Some((self.kettle_helm_mesh.clone(), self.iron_material.clone()))
            }
        }
    }
}

fn class_index(class: OutfitClass) -> usize {
    match class {
        OutfitClass::Cleric => 0,
        OutfitClass::Merchant => 1,
        OutfitClass::Craftsman => 2,
        OutfitClass::Laborer => 3,
        OutfitClass::Watch => 4,
        OutfitClass::Notable => 5,
        OutfitClass::Poor => 6,
    }
}

/// Quantized tint pick. Bits 16–17: away from bit 7 (headgear variance in the
/// sim) and the low bits, so tint, headgear and face stay uncorrelated.
fn tint_index(palette_seed: u32) -> usize {
    ((palette_seed >> 16) & (OUTFIT_TINT_BAND as u32 - 1)) as usize
}

/// Deterministic face pick — the seed rehashed so the face does not correlate
/// with the tint drawn from the raw seed's bits.
fn face_index(palette_seed: u32) -> usize {
    (palette_seed.wrapping_mul(0x9E37_79B1) >> 8) as usize % FACE_COUNT
}

/// Builds the bounded mesh/material set shared by all actor puppets.
pub(crate) fn setup_body_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cloth = |texture: Handle<Image>, tint: Color| StandardMaterial {
        base_color: tint,
        base_color_texture: Some(texture),
        perceptual_roughness: 0.88,
        reflectance: 0.28,
        ..default()
    };
    // Subtle multipliers over the already-muted cloth artwork: as-authored,
    // faded warm, cold-washed, sun-bleached.
    let tint_band = [
        Color::srgb(1.0, 1.0, 1.0),
        Color::srgb(0.80, 0.77, 0.73),
        Color::srgb(0.70, 0.74, 0.80),
        Color::srgb(0.88, 0.82, 0.68),
    ];
    const OUTFIT_TEXTURES: [&str; 7] = [
        "textures/npc/outfit_cleric.png",
        "textures/npc/outfit_merchant.png",
        "textures/npc/outfit_craftsman.png",
        "textures/npc/outfit_laborer.png",
        "textures/npc/outfit_watch.png",
        "textures/npc/outfit_notable.png",
        "textures/npc/outfit_poor.png",
    ];
    let textures: Vec<Handle<Image>> = OUTFIT_TEXTURES
        .iter()
        .map(|path| load_repeating_texture(&asset_server, path))
        .collect();
    let outfits = std::array::from_fn(|class| {
        std::array::from_fn(|tint| materials.add(cloth(textures[class].clone(), tint_band[tint])))
    });

    // The majors keep their legacy colors as tints over their composed class
    // texture (sven: smith→craftsman, conny: fish trader→merchant, ilse:
    // pilgrim→cleric). The flat legacy values were whole-body albedos; as
    // multipliers over textured cloth they are lifted so the brightest channel
    // lands at 0.85 — same hue, readable over the weave.
    let lift = |color: Color| {
        let linear = color.to_srgba();
        let max = linear.red.max(linear.green).max(linear.blue).max(1e-3);
        let scale = 0.85 / max;
        Color::srgb(
            (linear.red * scale).min(1.0),
            (linear.green * scale).min(1.0),
            (linear.blue * scale).min(1.0),
        )
    };
    let sven_outfit = materials.add(cloth(
        textures[class_index(OutfitClass::Craftsman)].clone(),
        lift(Color::srgb(0.19, 0.28, 0.36)),
    ));
    let conny_outfit = materials.add(cloth(
        textures[class_index(OutfitClass::Merchant)].clone(),
        lift(Color::srgb(0.16, 0.42, 0.49)),
    ));
    let ilse_outfit = materials.add(cloth(
        textures[class_index(OutfitClass::Cleric)].clone(),
        lift(Color::srgb(0.50, 0.24, 0.18)),
    ));

    // Painted faces, clamp-to-edge: the head mesh sends everything outside the
    // front hemisphere past [0,1], where the sampler holds the image's uniform
    // skin-tone border.
    let faces = (0..FACE_COUNT)
        .map(|index| {
            let texture = load_clamped_texture(
                &asset_server,
                format!("textures/npc/face_{index:02}.png"),
            );
            materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                perceptual_roughness: 0.72,
                reflectance: 0.24,
                ..default()
            })
        })
        .collect();

    // Open shells (caps, brims) need both faces drawn.
    let shell = |color: Color, roughness: f32, metallic: f32| StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        metallic,
        double_sided: true,
        cull_mode: None,
        ..default()
    };

    commands.insert_resource(BodyAssets {
        pelvis_mesh: meshes.add(pelvis_mesh()),
        torso_mesh: meshes.add(torso_mesh()),
        head_mesh: meshes.add(head_mesh()),
        upper_arm_mesh: meshes.add(limb_mesh(UPPER_ARM_RADIUS, UPPER_ARM_LENGTH)),
        forearm_mesh: meshes.add(limb_mesh(FOREARM_RADIUS, FOREARM_LENGTH)),
        thigh_mesh: meshes.add(limb_mesh(THIGH_RADIUS, THIGH_LENGTH)),
        shin_mesh: meshes.add(limb_mesh(SHIN_RADIUS, SHIN_LENGTH)),
        hood_mesh: meshes.add(hood_mesh()),
        coif_mesh: meshes.add(coif_mesh()),
        brim_mesh: meshes.add(brim_mesh()),
        kettle_helm_mesh: meshes.add(kettle_helm_mesh()),
        outfits,
        sven_outfit,
        conny_outfit,
        ilse_outfit,
        faces,
        hood_material: materials.add(shell(Color::srgb(0.30, 0.27, 0.24), 0.92, 0.0)),
        coif_material: materials.add(shell(Color::srgb(0.78, 0.74, 0.66), 0.90, 0.0)),
        felt_material: materials.add(shell(Color::srgb(0.16, 0.15, 0.14), 0.85, 0.0)),
        iron_material: materials.add(shell(Color::srgb(0.42, 0.44, 0.47), 0.45, 0.55)),
    });
}

fn load_clamped_texture(asset_server: &AssetServer, path: String) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            let mut sampler = ImageSamplerDescriptor::linear();
            sampler.set_address_mode(ImageAddressMode::ClampToEdge);
            settings.sampler = ImageSampler::Descriptor(sampler);
        })
        .load(path)
}

// ---------------------------------------------------------------------------
// Spawning
// ---------------------------------------------------------------------------

/// Per-build silhouette scaling (§2: Female/Male ±5% height/shoulder/hip).
///
/// Height rides the torso's local scale (not the pelvis'): the legs keep
/// their authored length so every foot stays planted on the walk plane, and
/// the stature difference reads in the torso/head, where the eye looks.
struct BuildScales {
    height: f32,
    shoulder: f32,
    hip: f32,
}

fn build_scales(build: Build) -> BuildScales {
    match build {
        Build::Male => BuildScales {
            height: 1.03,
            shoulder: 1.05,
            hip: 0.96,
        },
        Build::Female => BuildScales {
            height: 0.97,
            shoulder: 0.94,
            hip: 1.05,
        },
    }
}

/// The authored joint-local transforms every pose is expressed against.
///
/// Captured once at spawn (they fold in the per-build scaling) so the pose
/// writer can compose `rest ∘ delta` absolutely every frame — no incremental
/// drift, and a joint no layer owns snaps back to exactly its authored pose.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RestPose {
    pelvis: Transform,
    torso: Transform,
    head: Transform,
    left_upper_arm: Transform,
    right_upper_arm: Transform,
    left_forearm: Transform,
    right_forearm: Transform,
    left_thigh: Transform,
    right_thigh: Transform,
    left_shin: Transform,
    right_shin: Transform,
}

fn rest_pose(scales: &BuildScales) -> RestPose {
    let arm = |sign: f32| {
        Transform::from_xyz(sign * SHOULDER_X, SHOULDER_Y, 0.0)
            .with_rotation(Quat::from_rotation_z(sign * ARM_HANG_TILT_RAD))
    };
    let forearm = Transform::from_xyz(0.0, ELBOW_Y, 0.0)
        .with_rotation(Quat::from_rotation_x(ELBOW_BEND_RAD));
    let thigh = |sign: f32| {
        Transform::from_xyz(sign * HIP_X, HIP_Y, 0.0)
            .with_rotation(Quat::from_rotation_z(sign * THIGH_SPLAY_RAD))
    };
    RestPose {
        pelvis: Transform::IDENTITY.with_scale(Vec3::new(scales.hip, 1.0, scales.hip)),
        torso: Transform::from_xyz(0.0, TORSO_JOINT_Y, 0.0).with_scale(Vec3::new(
            scales.shoulder / scales.hip,
            scales.height,
            scales.shoulder / scales.hip,
        )),
        head: Transform::from_xyz(0.0, NECK_JOINT_Y + HEAD_LIFT, 0.0)
            .with_scale(Vec3::splat(HEAD_SCALE)),
        left_upper_arm: arm(-1.0),
        right_upper_arm: arm(1.0),
        left_forearm: forearm,
        right_forearm: forearm,
        left_thigh: thigh(-1.0),
        right_thigh: thigh(1.0),
        left_shin: Transform::from_xyz(0.0, KNEE_Y, 0.0),
        right_shin: Transform::from_xyz(0.0, KNEE_Y, 0.0),
    }
}

/// Spawns the full part hierarchy under `root` and returns the rig map.
///
/// The caller (actors.rs) owns the root and its anchors; this owns everything
/// below. Every mesh part carries the cloned `VisibilityRange` fade and cloth
/// parts carry [`ActorOutfit`] so the reconcile hot-swap reaches them all.
/// The root also grows a [`BodyPoseState`] so [`animate_body_pose`] can drive
/// the parts.
pub(super) fn spawn_body(
    commands: &mut Commands,
    root: Entity,
    assets: &BodyAssets,
    actor_id: &ActorId,
    appearance: &AppearanceSnapshot,
    fade: &VisibilityRange,
) -> BodyRig {
    let scales = build_scales(appearance.build);
    let rest = rest_pose(&scales);
    let outfit = assets.outfit_material(appearance);
    let face = assets.face_material(appearance);

    let cloth_part = |name: &'static str, mesh: &Handle<Mesh>, transform: Transform| {
        (
            Name::new(name),
            ActorOutfit(actor_id.clone()),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(outfit.clone()),
            transform,
            fade.clone(),
        )
    };

    let pelvis = commands
        .spawn((
            cloth_part("Body pelvis", &assets.pelvis_mesh, rest.pelvis),
            ChildOf(root),
        ))
        .id();
    let torso = commands
        .spawn((
            cloth_part("Body torso", &assets.torso_mesh, rest.torso),
            ChildOf(pelvis),
        ))
        .id();
    let head = commands
        .spawn((
            Name::new("Body head"),
            ActorFace(actor_id.clone()),
            Mesh3d(assets.head_mesh.clone()),
            MeshMaterial3d(face),
            rest.head,
            fade.clone(),
            ChildOf(torso),
        ))
        .id();
    let headgear = assets
        .headgear_visual(appearance)
        .map(|(mesh, material)| {
            commands
                .spawn((
                    Name::new("Body headgear"),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, HEAD_CENTER_ABOVE_NECK, 0.0),
                    fade.clone(),
                    ChildOf(head),
                ))
                .id()
        });

    let mut arm = |side: BodySide| {
        let (upper_rest, fore_rest, upper_name, fore_name) = match side {
            BodySide::Left => (
                rest.left_upper_arm,
                rest.left_forearm,
                "Left upper arm",
                "Left forearm",
            ),
            BodySide::Right => (
                rest.right_upper_arm,
                rest.right_forearm,
                "Right upper arm",
                "Right forearm",
            ),
        };
        let upper = commands
            .spawn((
                cloth_part(upper_name, &assets.upper_arm_mesh, upper_rest),
                ChildOf(torso),
            ))
            .id();
        let forearm = commands
            .spawn((
                cloth_part(fore_name, &assets.forearm_mesh, fore_rest),
                ChildOf(upper),
            ))
            .id();
        let hand = commands
            .spawn((
                Name::new(match side {
                    BodySide::Left => "Left hand anchor",
                    BodySide::Right => "Right hand anchor",
                }),
                HandAnchor {
                    actor: actor_id.clone(),
                    side,
                },
                Transform::from_xyz(0.0, HAND_ANCHOR_Y, 0.0),
                Visibility::default(),
                ChildOf(forearm),
            ))
            .id();
        (upper, forearm, hand)
    };
    let (left_upper_arm, left_forearm, left_hand) = arm(BodySide::Left);
    let (right_upper_arm, right_forearm, right_hand) = arm(BodySide::Right);

    let mut leg = |side: BodySide| {
        let (thigh_rest, shin_rest, thigh_name, shin_name) = match side {
            BodySide::Left => (rest.left_thigh, rest.left_shin, "Left thigh", "Left shin"),
            BodySide::Right => (
                rest.right_thigh,
                rest.right_shin,
                "Right thigh",
                "Right shin",
            ),
        };
        let thigh = commands
            .spawn((
                cloth_part(thigh_name, &assets.thigh_mesh, thigh_rest),
                ChildOf(pelvis),
            ))
            .id();
        let shin = commands
            .spawn((
                cloth_part(shin_name, &assets.shin_mesh, shin_rest),
                ChildOf(thigh),
            ))
            .id();
        (thigh, shin)
    };
    let (left_thigh, left_shin) = leg(BodySide::Left);
    let (right_thigh, right_shin) = leg(BodySide::Right);

    commands
        .entity(root)
        .insert(BodyPoseState::new(actor_id, rest));

    BodyRig {
        pelvis,
        torso,
        head,
        headgear,
        left_upper_arm,
        right_upper_arm,
        left_forearm,
        right_forearm,
        left_thigh,
        right_thigh,
        left_shin,
        right_shin,
        left_hand,
        right_hand,
    }
}

// ---------------------------------------------------------------------------
// The pose pipeline (M1): layered joint deltas over the rest pose.
//
// L0 locomotion + L1 idle life ship here; L2 activity (M2), L4 speech & gaze
// (M3) and the L3 one-shots (M2, extended by M4's catalog) append further
// `apply_*` calls to the same accumulator, gated to Tier A.
// ---------------------------------------------------------------------------

/// Idle↔walk crossfade time (§5: starts/stops must not pop).
const WALK_BLEND_SECONDS: f32 = 0.25;
/// Above this speed the walk cycle runs at full amplitude (§5's smoothstep
/// upper edge); the lower edge is the sim's `SETTLED_SPEED_MPS`.
const WALK_FULL_SPEED_MPS: f32 = 0.6;
/// A mover's samples arrive every 20 Hz tick while it walks; a sample older
/// than this means the mover stopped (the sim publishes no "I stopped" tick —
/// an arrived walker simply leaves the hot channel), so treat speed as zero.
const SAMPLE_STALE_SECONDS: f64 = 0.18;

/// Thigh swing amplitude. Together with the authored leg length (hip ≈ 0.82 m
/// over ground) this makes the visual stride ≈ `2·0.82·sin(A)` ≈ 0.75 m per
/// step — which is what `GAIT_CADENCE` in the sim is tuned against so feet
/// don't visibly skate.
const THIGH_SWING_RAD: f32 = 0.47;
/// Shin counter-bend amplitude: the knee folds during the swing phase only.
const SHIN_BEND_RAD: f32 = 0.62;
/// Phase advance of the knee-fold gate, so peak flexion lands early-swing.
const SHIN_GATE_ADVANCE_RAD: f32 = 0.6;
/// Arms counter-swing at about half the leg amplitude (§5).
const ARM_SWING_RAD: f32 = 0.5 * THIGH_SWING_RAD;
/// Torso bob, peak-to-peak, at double stride frequency via `|sin|`.
const BOB_AMPLITUDE_M: f32 = 0.03;
/// Lateral torso roll over the planted foot, once per full stride cycle.
const WALK_ROLL_RAD: f32 = 0.035;
/// Torso roll per rad/s of yaw rate — leaning into the turn — and its clamp.
const TURN_LEAN_PER_YAW_RATE: f32 = 0.055;
const TURN_LEAN_MAX_RAD: f32 = 0.11;
/// Yaw-rate low-pass rate (1/s): the 20 Hz derivative is noisy.
const YAW_RATE_SMOOTHING_PER_S: f32 = 8.0;

/// L1 breathing: a 2–4 s torso cycle, per-actor period and phase.
const BREATH_PERIOD_BASE_S: f64 = 2.6;
const BREATH_PERIOD_SPAN_S: f64 = 1.2;
const BREATH_SCALE: f32 = 0.013;
const BREATH_PITCH_RAD: f32 = 0.022;
/// L1 occasional weight shift: hips drift a couple of centimetres to
/// alternating sides for part of each (per-actor) 9–15 s cycle.
const SHIFT_PERIOD_BASE_S: f64 = 9.0;
const SHIFT_PERIOD_SPAN_S: f64 = 6.0;
const SHIFT_PELVIS_X_M: f32 = 0.022;
const SHIFT_TORSO_ROLL_RAD: f32 = 0.05;
/// L1 rare head glance: an alternating-side yaw held a beat, every 12–20 s.
const GLANCE_PERIOD_BASE_S: f64 = 12.0;
const GLANCE_PERIOD_SPAN_S: f64 = 8.0;
const GLANCE_YAW_RAD: f32 = 0.55;
const GLANCE_PITCH_RAD: f32 = 0.06;

// -- §8 carriage: drunkenness and weariness modulate L0/L1 (they are *not* a
// new layer, §4). Everything scales by the status value, so at 0 the pose is
// byte-identical to a sober, rested actor's; because it only reshapes the walk
// the sim already computed, the actor's position stays exactly the sim's.
/// Drunkenness perturbs the visual gait phase: a fast small jitter (phase
/// noise) plus a slow larger drift (cadence irregularity), both in stride-cycle
/// units so the legs speed up, lag and stagger. Foot placement is visual only —
/// the root is the sim's — so this is sloppy footwork, not a moved actor.
const DRUNK_PHASE_NOISE_CYCLES: f32 = 0.10;
const DRUNK_PHASE_NOISE_HZ: f64 = 1.1;
const DRUNK_CADENCE_CYCLES: f32 = 0.18;
const DRUNK_CADENCE_HZ: f64 = 0.23;
/// Drunkenness sways the torso side to side and holds a slowly wandering lean —
/// two separate roll contributions, clamped together so even full drunkenness
/// never tips the figure past a stagger.
const DRUNK_SWAY_RAD: f32 = 0.16;
const DRUNK_SWAY_HZ: f64 = 0.9;
const DRUNK_LEAN_RAD: f32 = 0.14;
const DRUNK_ROLL_MAX_RAD: f32 = 0.35;
/// Weariness drops the arm swing toward 0.3× (1 − 0.7) and folds the torso
/// forward into a stoop (a negative X-rotation, like the bow).
const WEARY_ARM_DROP: f32 = 0.7;
const WEARY_STOOP_RAD: f32 = 0.30;

/// L2 activity (M2): the idle↔carry/offer arm crossfade — §4's "a layer ramps
/// its weight in/out", so a retracted offer reads as motion, not a pop.
const ARM_BLEND_SECONDS: f32 = 0.22;
/// Carry pose (§6): the left arm relaxed at the side with a basket-carry
/// pitch — a light forward hang plus a deeper elbow bend that brings the hand
/// (and the prop riding its anchor) clear of the thigh.
const CARRY_UPPER_PITCH_RAD: f32 = 0.22;
const CARRY_FOREARM_BEND_RAD: f32 = 0.95;
/// Offer pose (§6): the right arm lifts from hanging to just under horizontal
/// and the elbow straightens, holding the prop out toward the recipient.
const OFFER_LIFT_RAD: f32 = 1.30;
const OFFER_FOREARM_STRAIGHTEN_RAD: f32 = -0.16;
/// How far the offered arm aims off the body's facing before clamping — the
/// sim turns the whole body for larger errors, the arm only fine-aims.
const OFFER_YAW_CLAMP_RAD: f32 = 1.1;
const OFFER_PITCH_CLAMP_RAD: f32 = 0.5;
/// The stall hand-over pulse (M2, `stall_sale`): no standing offer exists for
/// a silent purchase, so the vendor's arm extends on the event and retracts on
/// its own after this long.
const STALL_PULSE_SECONDS: f64 = 0.9;
/// L3 one-shot envelope ramps (§4: ~0.15 s in, a touch longer out).
const GESTURE_RAMP_IN_S: f32 = 0.15;
const GESTURE_RAMP_OUT_S: f32 = 0.20;
const NOD_PITCH_RAD: f32 = 0.30;
const SHAKE_YAW_RAD: f32 = 0.45;
/// How far a one-shot turns the head toward its optional target.
const GESTURE_FACE_YAW_CLAMP_RAD: f32 = 0.6;

// -- L3 deliberate one-shots (M4). Upper-body only, so a walking wave works;
// the aiming kinds (wave, beckon, point) swing the shoulder by `face_yaw`.
/// Wave: the right arm lifts high and out, elbow bent, hand oscillating.
const WAVE_UPPER_LIFT_RAD: f32 = 2.15;
const WAVE_UPPER_OUT_RAD: f32 = 0.34;
const WAVE_FOREARM_BEND_RAD: f32 = 0.75;
const WAVE_FOREARM_SWING_RAD: f32 = 0.55;
const WAVE_CYCLES: f32 = 3.0;
/// Beckon: the arm held forward-up, the forearm curling in and out.
const BECKON_UPPER_LIFT_RAD: f32 = 1.55;
const BECKON_FOREARM_BASE_RAD: f32 = 1.15;
const BECKON_FOREARM_CURL_RAD: f32 = 0.75;
const BECKON_CYCLES: f32 = 2.5;
/// Shrug: both shoulders lift and abduct, forearms turning out (palms up).
const SHRUG_UPPER_LIFT_RAD: f32 = 0.30;
const SHRUG_UPPER_OUT_RAD: f32 = 0.42;
const SHRUG_FOREARM_BEND_RAD: f32 = 0.85;
const SHRUG_FOREARM_OUT_RAD: f32 = 0.45;
const SHRUG_HEAD_TILT_RAD: f32 = 0.14;
/// Point: the right arm extends straight toward the target, forearm level.
const POINT_UPPER_LIFT_RAD: f32 = 1.42;
const POINT_FOREARM_STRAIGHTEN_RAD: f32 = -0.20;
/// Bow: the torso folds forward at the waist, the head dropping with it.
const BOW_TORSO_PITCH_RAD: f32 = 0.52;
const BOW_HEAD_PITCH_RAD: f32 = 0.30;

// -- L3 looping dance (M4). A per-actor phase offset keeps a crowd unsynced.
const DANCE_BLEND_SECONDS: f32 = 0.30;
const DANCE_HZ: f64 = 1.7;
const DANCE_TORSO_ROLL_RAD: f32 = 0.20;
const DANCE_TORSO_TWIST_RAD: f32 = 0.30;
const DANCE_PELVIS_BOB_M: f32 = 0.055;
const DANCE_PELVIS_SWAY_M: f32 = 0.03;
const DANCE_UPPER_LIFT_RAD: f32 = 1.5;
const DANCE_UPPER_SWING_RAD: f32 = 0.55;
const DANCE_FOREARM_BEND_RAD: f32 = 1.15;
const DANCE_HEAD_BOB_RAD: f32 = 0.13;

// -- L4 speech & gaze (M3). Host-only reflex (§3 Tier 1): everything below
// reads presentation messages and snapshot-derived L2 targets, and none of it
// ever produces sim state or a percept.

/// The head turns at most this far off the torso's facing (§4: "yaw clamp
/// ±70°"); pitch stays in a narrower band (looking up at a close speaker).
const GAZE_YAW_CLAMP_RAD: f32 = 70.0 * PI / 180.0;
const GAZE_PITCH_CLAMP_RAD: f32 = 0.5;
/// A target further off-axis than this is essentially behind the actor: the
/// head gives up instead of pinning at the clamp with a craned neck.
const GAZE_GIVE_UP_RAD: f32 = 2.4;
/// The gaze layer's weight ramp and the head's tracking rate (the "slerp").
const GAZE_BLEND_SECONDS: f32 = 0.25;
const GAZE_TRACK_PER_S: f32 = 7.0;
/// Aim height of eyes over the actor root — the head pivot sits ≈ 0.52 above
/// the root and the halved head centres ≈ 0.08 up (see [`HEAD_SCALE`]); both the
/// gazer's eye and an NPC gaze target use this one offset.
const GAZE_EYE_OFFSET_Y: f32 = 0.60;
/// Listeners this close to a live speaker glance at them — the social radius
/// speech itself uses.
const LISTEN_RADIUS_M: f32 = super::HEARING_RADIUS_M;
/// The talk layer's weight ramp (in/out as the deadline opens/expires).
const TALK_BLEND_SECONDS: f32 = 0.3;
/// Talk head bob: a small nodding cadence riding on top of the gaze.
const TALK_HEAD_BOB_RAD: f32 = 0.05;
const TALK_BOB_BASE_HZ: f64 = 2.3;
const TALK_BOB_SPAN_HZ: f64 = 1.0;
/// Talk forearm gesticulation: both forearms lift off the rest hang and wave,
/// sides out of phase, everything scaled by the per-id energy noise. The
/// upper arms pitch forward a touch and the elbows drift off the ribs so the
/// gesture still reads in silhouette at 8 m (§1's talking target).
const TALK_FOREARM_LIFT_RAD: f32 = 0.85;
const TALK_FOREARM_SWING_RAD: f32 = 0.38;
const TALK_UPPER_LIFT_RAD: f32 = 0.26;
const TALK_UPPER_TILT_RAD: f32 = 0.12;
const TALK_WAVE_BASE_HZ: f64 = 1.5;
const TALK_WAVE_SPAN_HZ: f64 = 0.9;
/// A sound glance holds for 2.2–3.6 s (per actor-and-sound hash), then decays.
const SOUND_GLANCE_BASE_S: f64 = 2.2;
const SOUND_GLANCE_SPAN_S: f64 = 1.4;
/// A sound closer than this is the actor's own doing (their coin clink, their
/// bucket) — nobody glances at their own feet.
const SOUND_GLANCE_MIN_DISTANCE_M: f32 = 1.0;
/// Recent-sound ring: enough for a noisy square, bounded forever.
const MAX_RECENT_SOUNDS: usize = 8;

/// Pose LOD (§9): Tier A (all layers) inside this radius, capped to the
/// nearest [`TIER_A_CAP`]; Tier B (L0 only, every other frame) to the fade
/// start; Tier C (no pose writes) beyond it.
const TIER_A_DISTANCE_M: f32 = 40.0;
const TIER_A_CAP: usize = 64;
const TIER_B_DISTANCE_M: f32 = 120.0;

/// How often the pose-cost diagnostic prints (§9 frame budget: ≤ 0.5 ms at
/// the Tier A cap).
const POSE_TIMING_LOG_SECONDS: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PoseTier {
    A,
    B,
    C,
}

/// One joint's offset from its rest transform.
#[derive(Debug, Clone, Copy, PartialEq)]
struct JointDelta {
    rotation: Quat,
    translation: Vec3,
    scale: Vec3,
}

impl Default for JointDelta {
    fn default() -> Self {
        Self {
            rotation: Quat::IDENTITY,
            translation: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }
}

impl JointDelta {
    fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }

    /// Blends `target` over the accumulated delta at `weight` — the layer
    /// override rule from §4: a layer at weight 1 owns the joint outright, a
    /// ramping layer drags the joint toward its pose.
    fn blend_over(&mut self, target: Self, weight: f32) {
        if weight >= 1.0 {
            *self = target;
            return;
        }
        self.rotation = self.rotation.slerp(target.rotation, weight);
        self.translation = self.translation.lerp(target.translation, weight);
        self.scale = self.scale.lerp(target.scale, weight);
    }
}

/// Deltas for every joint the pipeline animates, all relative to [`RestPose`].
/// Later layers blend over earlier ones per joint ([`JointDelta::blend_over`]).
#[derive(Debug, Default, Clone, Copy)]
struct PoseDeltas {
    pelvis: JointDelta,
    torso: JointDelta,
    head: JointDelta,
    left_upper_arm: JointDelta,
    right_upper_arm: JointDelta,
    left_forearm: JointDelta,
    right_forearm: JointDelta,
    left_thigh: JointDelta,
    right_thigh: JointDelta,
    left_shin: JointDelta,
    right_shin: JointDelta,
}

/// The two 20 Hz samples the gait interpolates between — the same
/// prev/current scheme `drive_npc_bodies` uses for the root, kept separately
/// here because `NpcMotion`'s fields are private to actors.rs and the pose
/// needs `gait_phase` and a yaw *rate*, which the root interpolation discards.
#[derive(Debug, Clone, Copy)]
struct GaitHistory {
    prev_phase: f32,
    cur_phase: f32,
    prev_yaw: f32,
    cur_yaw: f32,
    speed: f32,
    t0: f64,
    seq: u64,
}

/// The one-shot (non-looping) gesture kinds a body plays. `Nod`/`ShakeHead`
/// are the habit tier's autonomic beats (accept → nod, decline → head-shake,
/// §6/§7); the rest are M4's deliberate `gesture` verb (§7). The looping
/// `dance` is not here — it rides [`BodyPoseState::dance`], driven by the
/// snapshot rather than a fixed-length envelope. Durations match the sim
/// catalog (`crates/cathedral-sim/src/gesture.rs`) so the pose lasts exactly as
/// long as the sim says the act does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OneShotGesture {
    Nod,
    ShakeHead,
    Wave,
    Beckon,
    Shrug,
    Point,
    Bow,
}

impl OneShotGesture {
    fn duration_seconds(self) -> f32 {
        match self {
            OneShotGesture::Nod => 0.8,
            OneShotGesture::ShakeHead => 0.9,
            OneShotGesture::Wave => 1.5,
            OneShotGesture::Beckon => 1.5,
            OneShotGesture::Shrug => 1.0,
            OneShotGesture::Point => 1.2,
            OneShotGesture::Bow => 1.8,
        }
    }

    /// Map a sim gesture kind to the pose that plays it, or `None` for the
    /// looping `dance` (which the snapshot drives, not a one-shot).
    pub(super) fn from_kind(kind: cathedral_sim::GestureKind) -> Option<Self> {
        use cathedral_sim::GestureKind as K;
        Some(match kind {
            K::Nod => OneShotGesture::Nod,
            K::ShakeHead => OneShotGesture::ShakeHead,
            K::Wave => OneShotGesture::Wave,
            K::Beckon => OneShotGesture::Beckon,
            K::Shrug => OneShotGesture::Shrug,
            K::Point => OneShotGesture::Point,
            K::Bow => OneShotGesture::Bow,
            K::Dance => return None,
        })
    }
}

/// One running one-shot: kind, start time, and the optional world point the
/// head turns toward while it plays.
#[derive(Debug, Clone, Copy)]
struct ActiveGesture {
    kind: OneShotGesture,
    started_at: f64,
    face: Option<Vec3>,
}

/// Per-actor pose bookkeeping on the puppet root: the captured rest pose, the
/// layer blend state and the LOD tier. This is the §4 "active-layer
/// bookkeeping" — deliberately not a state machine.
#[derive(Component, Debug)]
pub(crate) struct BodyPoseState {
    rest: RestPose,
    /// FNV-1a of the actor id: idle phases, periods and the Tier B frame
    /// parity all derive from it so crowds never sync.
    seed: u32,
    walk_blend: f32,
    yaw_rate: f32,
    history: Option<GaitHistory>,
    tier: PoseTier,
    last_eval: f64,
    /// The parts currently hold exactly the rest pose, so an idle far-away
    /// body costs zero transform writes.
    at_rest: bool,
    // -- L2 activity (M2), targets written by `hands::reconcile_hand_props`
    // (state-driven, ReconcileMirror) and blended here (cosmetic, Present).
    carry_target: bool,
    /// The world point the offered right arm aims at, while an offer stands.
    offer_target: Option<Vec3>,
    /// A transient stall hand-over pulse: aim point and expiry.
    offer_pulse: Option<(Vec3, f64)>,
    carry_blend: f32,
    offer_blend: f32,
    /// The last resolved (yaw, pitch) aim, kept so a retracting arm swings
    /// back from where it pointed instead of snapping to forward.
    offer_aim: (f32, f32),
    // -- L3 one-shot gesture (M2 habit tier; M4 adds the deliberate verb).
    gesture: Option<ActiveGesture>,
    // -- L3 looping dance (M4): driven by the snapshot's `active_gesture`, not
    // a fixed envelope, so a player who arrives mid-loop still sees it. The
    // target is a bool ramped through `dance_blend`.
    dance: bool,
    dance_blend: f32,
    // -- L4 speech & gaze (M3): the talk layer's weight and the smoothed
    // head aim, kept across frames so the head slerps instead of snapping.
    talk_blend: f32,
    gaze_blend: f32,
    gaze_yaw: f32,
    gaze_pitch: f32,
    // -- §8 carriage: the two publicly-visible statuses, mirrored from the
    // snapshot each frame by `drive_gesture_pose` (ReconcileMirror) and read by
    // L0/L1 (Present). Applied directly, no blend — the debug hooks step them,
    // and a future ale would ramp its own float sim-side.
    carriage: Carriage,
}

impl BodyPoseState {
    fn new(actor_id: &ActorId, rest: RestPose) -> Self {
        Self {
            rest,
            seed: fnv1a(actor_id.0.as_bytes()),
            walk_blend: 0.0,
            yaw_rate: 0.0,
            history: None,
            // Spawned parts already stand in the rest pose; distant actors
            // then never get touched until they come into range.
            tier: PoseTier::C,
            last_eval: 0.0,
            at_rest: true,
            carry_target: false,
            offer_target: None,
            offer_pulse: None,
            carry_blend: 0.0,
            offer_blend: 0.0,
            offer_aim: (0.0, 0.0),
            gesture: None,
            dance: false,
            dance_blend: 0.0,
            talk_blend: 0.0,
            gaze_blend: 0.0,
            gaze_yaw: 0.0,
            gaze_pitch: 0.0,
            carriage: Carriage::default(),
        }
    }

    /// The habit tier's L2 targets (§6): `carry` fills the basket-carry left
    /// arm; `offer_at` extends the right arm toward the world point.
    pub(super) fn set_hand_activity(&mut self, carry: bool, offer_at: Option<Vec3>) {
        self.carry_target = carry;
        self.offer_target = offer_at;
    }

    /// A short vendor hand-over pulse toward `at` — the `stall_sale`
    /// choreography, where no standing offer exists to key the arm on.
    pub(super) fn pulse_offer(&mut self, at: Vec3, now: f64) {
        self.offer_pulse = Some((at, now + STALL_PULSE_SECONDS));
    }

    /// Starts a one-shot gesture, optionally turning the head (and, for the
    /// aiming kinds, the arm) toward a world point while it plays. The clean
    /// entry point M2's habit tier and M4's `gesture` verb both use.
    pub(super) fn start_gesture(&mut self, kind: OneShotGesture, face: Option<Vec3>, now: f64) {
        self.gesture = Some(ActiveGesture {
            kind,
            started_at: now,
            face,
        });
    }

    /// The snapshot's looping-gesture state (M4): the dance runs while set and
    /// stops when it clears, its blend ramping either way.
    pub(super) fn set_dance(&mut self, dancing: bool) {
        self.dance = dancing;
    }

    /// The snapshot's carriage statuses (§8), mirrored each frame. Values are
    /// already clamped to `0..=1` at the boundary; unknown kinds are ignored
    /// (a body can only render what it has a pose for).
    pub(super) fn set_carriage(&mut self, statuses: &[(cathedral_sim::StatusKind, f32)]) {
        self.carriage = Carriage::from_statuses(statuses);
    }
}

/// The transient host-side twin of `cathedral_sim::EngineMessage::Gesture`
/// (`features/npc_bodies.md` §7): the trigger [`drive_gesture_pose`] plays a
/// one-shot pose from. The looping `dance` is not carried here — it rides the
/// snapshot's `active_gesture`, so a player who arrives mid-loop still sees it.
#[derive(Message, Debug, Clone)]
pub struct PresentGesture {
    pub actor_id: ActorId,
    pub kind: cathedral_sim::GestureKind,
    /// The person the gesture aims at, if any — the arm and head turn toward
    /// their mirror position. `None` for an untargeted or place-pointed
    /// gesture, which then plays straight ahead.
    pub target_id: Option<ActorId>,
}

/// L3 gesture driver (M4), in the `ReconcileMirror` set: it starts one-shot
/// poses from [`PresentGesture`] triggers and keeps every body's looping-dance
/// flag in step with the snapshot's `active_gesture`. State only — the cosmetic
/// pose itself is written by [`animate_body_pose`] in the `Present` set.
pub(crate) fn drive_gesture_pose(
    time: Res<Time>,
    mirror: Res<WorldMirror>,
    mut triggers: MessageReader<PresentGesture>,
    mut poses: Query<(&ActorId, &mut BodyPoseState)>,
) {
    let now = time.elapsed_secs_f64();
    for trigger in triggers.read() {
        // The looping dance is driven from the snapshot below; a one-shot maps
        // to a pose here and aims at its target's live mirror position.
        let Some(kind) = OneShotGesture::from_kind(trigger.kind) else {
            continue;
        };
        let face = trigger
            .target_id
            .as_ref()
            .and_then(|id| mirror.actor(id))
            .map(|actor| Vec3::from(actor.position_m) + Vec3::Y * GAZE_EYE_OFFSET_Y);
        for (actor_id, mut pose) in poses.iter_mut() {
            if actor_id == &trigger.actor_id {
                pose.start_gesture(kind, face, now);
                break;
            }
        }
    }

    // The snapshot-driven pose inputs, synced every frame: the looping-dance
    // flag (authoritative so a late-arriving player sees the loop and it stops
    // the frame `active_gesture` clears) and the §8 carriage statuses (so a
    // walker with drunkenness/weariness sways and stoops). One mirror lookup
    // per body serves both.
    for (actor_id, mut pose) in poses.iter_mut() {
        let actor = mirror.actor(actor_id);
        let dancing =
            actor.is_some_and(|actor| actor.active_gesture == Some(cathedral_sim::GestureKind::Dance));
        pose.set_dance(dancing);
        pose.set_carriage(actor.map(|actor| actor.statuses.as_slice()).unwrap_or(&[]));
    }
}

// ---------------------------------------------------------------------------
// L4 reflex bookkeeping (M3): who is talking until when, and what recently
// made a sound. Fed exclusively from the presentation messages the bubbles
// and speakers already consume (`PresentSpeech`, `PlaySoundEffect`) — §3
// Tier 1: host-only, cosmetic, invisible to minds.
// ---------------------------------------------------------------------------

/// One live utterance: how long the speaker keeps talking (the bubble
/// formula) and whom the line explicitly addressed.
#[derive(Debug, Clone)]
struct TalkEntry {
    until: f64,
    partner: Option<ActorId>,
}

/// One recent audible event; glances at it decay per actor a beat or two
/// after `at`.
#[derive(Debug, Clone, Copy)]
struct SoundGlance {
    position: Vec3,
    audible_distance: f32,
    at: f64,
}

/// Everything the gaze selector needs about one actor this frame.
struct GazeQuery {
    /// The actor's root position (world).
    position: Vec3,
    /// Where the actor's own standing offer (or stall pulse) aims, if any.
    offer_at: Option<Vec3>,
    /// Settled actors glance at sounds; committed walkers don't.
    idle: bool,
    /// The actor-id seed (per-actor glance-hold variation).
    seed: u32,
}

/// The talk/sound signal store the pose system reads. Bounded: one entry per
/// concurrently-live speaker, at most [`MAX_RECENT_SOUNDS`] sounds.
#[derive(Resource, Debug, Default)]
pub(crate) struct ReflexState {
    talkers: HashMap<ActorId, TalkEntry>,
    sounds: Vec<SoundGlance>,
}

impl ReflexState {
    /// Records one presented line: the speaker talks until the bubble
    /// formula's deadline (a follow-up line extends it), at `partner`.
    fn note_speech(&mut self, speaker: ActorId, partner: Option<ActorId>, text: &str, now: f64) {
        let until = now + f64::from(super::speech::speech_text_seconds(text));
        self.talkers.insert(speaker, TalkEntry { until, partner });
    }

    /// Records one played world sound as glance material.
    fn note_sound(&mut self, position: Vec3, audible_distance: f32, now: f64) {
        if self.sounds.len() >= MAX_RECENT_SOUNDS {
            self.sounds.remove(0);
        }
        self.sounds.push(SoundGlance {
            position,
            audible_distance,
            at: now,
        });
    }

    /// Drops expired talk deadlines and sounds nobody can still glance at.
    fn prune(&mut self, now: f64) {
        self.talkers.retain(|_, entry| entry.until > now);
        self.sounds
            .retain(|glance| now < glance.at + SOUND_GLANCE_BASE_S + SOUND_GLANCE_SPAN_S);
    }

    /// Is this actor's talk deadline still open?
    fn is_talking(&self, id: &ActorId, now: f64) -> bool {
        self.talkers.get(id).is_some_and(|entry| entry.until > now)
    }

    /// Every actor id a gaze this frame may need a live position for: the
    /// speakers themselves and their addressees.
    fn referenced_ids(&self, now: f64) -> HashSet<ActorId> {
        self.talkers
            .iter()
            .filter(|(_, entry)| entry.until > now)
            .flat_map(|(speaker, entry)| {
                [Some(speaker.clone()), entry.partner.clone()].into_iter().flatten()
            })
            .collect()
    }

    /// The world point this actor's head should track, by the M3 priority:
    /// active conversation (their partner while talking, else the nearest
    /// live speaker in listening range) > their own offer's recipient > a
    /// recent sound in earshot > none. `resolve` maps an actor id to a live
    /// eye-height point (the player resolves to the camera).
    fn gaze_point(
        &self,
        id: &ActorId,
        query: &GazeQuery,
        now: f64,
        resolve: &impl Fn(&ActorId) -> Option<Vec3>,
    ) -> Option<Vec3> {
        // Talking: face the addressee.
        if let Some(entry) = self.talkers.get(id).filter(|entry| entry.until > now)
            && let Some(point) = entry.partner.as_ref().and_then(resolve)
        {
            return Some(point);
        }
        // Listening: glance at the nearest live speaker in social range.
        let nearest_speaker = self
            .talkers
            .iter()
            .filter(|(speaker, entry)| *speaker != id && entry.until > now)
            .filter_map(|(speaker, _)| resolve(speaker))
            .map(|point| (query.position.distance_squared(point), point))
            .filter(|(distance_sq, _)| *distance_sq <= LISTEN_RADIUS_M * LISTEN_RADIUS_M)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, point)) = nearest_speaker {
            return Some(point);
        }
        // Their own standing offer: gaze follows the recipient (§6, deferred
        // from M2 to here).
        if let Some(at) = query.offer_at {
            return Some(at + Vec3::Y * GAZE_EYE_OFFSET_Y * 0.5);
        }
        // A recent sound draws a brief glance from idle actors in earshot.
        if !query.idle {
            return None;
        }
        self.sounds.iter().rev().find_map(|glance| {
            let hold = SOUND_GLANCE_BASE_S
                + SOUND_GLANCE_SPAN_S
                    * f64::from(hash01(query.seed ^ (glance.at.to_bits() as u32), 31));
            let distance_sq = query.position.distance_squared(glance.position);
            (now < glance.at + hold
                && distance_sq <= glance.audible_distance * glance.audible_distance
                && distance_sq > SOUND_GLANCE_MIN_DISTANCE_M * SOUND_GLANCE_MIN_DISTANCE_M)
                .then_some(glance.position)
        })
    }
}

/// Feeds [`ReflexState`] from the presentation stream. Runs in the `Present`
/// set right before [`animate_body_pose`], so a line drained this frame
/// starts its speaker's talk layer the same frame the bubble appears.
pub(crate) fn track_reflex_signals(
    time: Res<Time>,
    mut state: ResMut<ReflexState>,
    mut speech: MessageReader<super::speech::PresentSpeech>,
    mut sounds: MessageReader<super::sound::PlaySoundEffect>,
) {
    let now = time.elapsed_secs_f64();
    for line in speech.read() {
        state.note_speech(
            line.speaker_id.clone(),
            line.target_id.clone(),
            &line.text,
            now,
        );
    }
    for sound in sounds.read() {
        state.note_sound(sound.position, sound.audible_distance, now);
    }
    state.prune(now);
}

/// Local-frame head aim at a world point: signed yaw off the torso facing and
/// elevation pitch, clamped to the neck's range. `None` when the target is
/// essentially behind the actor — the head returns to neutral rather than
/// pinning at the clamp.
fn gaze_aim(root: &Transform, at: Vec3) -> Option<(f32, f32)> {
    let eye = root.translation + Vec3::Y * GAZE_EYE_OFFSET_Y;
    let local = root.rotation.inverse() * (at - eye);
    let yaw = (-local.x).atan2(-local.z);
    if yaw.abs() > GAZE_GIVE_UP_RAD {
        return None;
    }
    let horizontal = (local.x * local.x + local.z * local.z).sqrt().max(1e-4);
    let pitch = local.y.atan2(horizontal);
    Some((
        yaw.clamp(-GAZE_YAW_CLAMP_RAD, GAZE_YAW_CLAMP_RAD),
        pitch.clamp(-GAZE_PITCH_CLAMP_RAD, GAZE_PITCH_CLAMP_RAD),
    ))
}

/// The id-seeded amplitude noise (§4 L4: "amplitude ramped by an id-seeded
/// noise so different speakers move differently"): two slow incommensurate
/// sines with per-actor rates and phases, wandering in [0.1, 1].
fn talk_energy(now: f64, seed: u32) -> f32 {
    use std::f64::consts::TAU as TAU64;
    let a = ((now * (0.35 + 0.30 * f64::from(hash01(seed, 23))) + f64::from(hash01(seed, 24)))
        * TAU64)
        .sin();
    let b = ((now * (0.11 + 0.10 * f64::from(hash01(seed, 25))) + f64::from(hash01(seed, 26)))
        * TAU64)
        .sin();
    (0.55 + 0.225 * (a + b) as f32).clamp(0.1, 1.0)
}

/// L4 talk gesticulation: a small head bob composed *over* the gaze (the head
/// keeps facing the partner) and both forearms lifting off the rest hang to
/// wave, sides out of phase. Per-arm weights let L2 keep an arm it owns — a
/// vendor mid-offer gesticulates with the free arm only.
fn apply_talk(
    pose: &mut PoseDeltas,
    weight: f32,
    left_arm_weight: f32,
    right_arm_weight: f32,
    now: f64,
    seed: u32,
) {
    use std::f64::consts::TAU as TAU64;
    let energy = talk_energy(now, seed);

    let bob_hz = TALK_BOB_BASE_HZ + TALK_BOB_SPAN_HZ * f64::from(hash01(seed, 27));
    let bob_phase = ((now * bob_hz + f64::from(hash01(seed, 28))) * TAU64).rem_euclid(TAU64) as f32;
    let bob = TALK_HEAD_BOB_RAD * energy * bob_phase.sin();
    let bobbed = pose.head.rotation * Quat::from_rotation_x(bob);
    pose.head.rotation = pose.head.rotation.slerp(bobbed, weight);

    let wave_hz = TALK_WAVE_BASE_HZ + TALK_WAVE_SPAN_HZ * f64::from(hash01(seed, 29));
    let wave = ((now * wave_hz + f64::from(hash01(seed, 30))) * TAU64).rem_euclid(TAU64) as f32;
    let lift = TALK_FOREARM_LIFT_RAD * (0.5 + 0.5 * energy);
    let upper = TALK_UPPER_LIFT_RAD * (0.4 + 0.6 * energy);
    let arm = |upper_joint: &mut JointDelta,
               forearm_joint: &mut JointDelta,
               phase: f32,
               side_sign: f32,
               arm_weight: f32| {
        let bend = lift + TALK_FOREARM_SWING_RAD * energy * phase.sin();
        forearm_joint.blend_over(
            JointDelta::from_rotation(Quat::from_rotation_x(bend)),
            weight * arm_weight,
        );
        // Elbows drift outward off the ribs while the upper arm pitches
        // forward — the silhouette half of the gesture.
        upper_joint.blend_over(
            JointDelta::from_rotation(
                Quat::from_rotation_z(side_sign * TALK_UPPER_TILT_RAD)
                    * Quat::from_rotation_x(upper),
            ),
            weight * arm_weight * 0.6,
        );
    };
    arm(
        &mut pose.left_upper_arm,
        &mut pose.left_forearm,
        wave,
        -1.0,
        left_arm_weight,
    );
    arm(
        &mut pose.right_upper_arm,
        &mut pose.right_forearm,
        wave + 0.6 * PI,
        1.0,
        right_arm_weight,
    );
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(16_777_619)
    })
}

/// A deterministic per-actor unit float, decorrelated per `salt`.
fn hash01(seed: u32, salt: u32) -> f32 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    (x >> 8) as f32 / (1 << 24) as f32
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn move_toward(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}

/// Shortest signed arc from one yaw to another, radians.
fn angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = (to - from) % TAU;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    delta
}

/// §5's walk factor: 0 when settled, 1 at a committed walk.
fn walk_factor(speed: f32) -> f32 {
    smoothstep(SETTLED_SPEED_MPS as f32, WALK_FULL_SPEED_MPS, speed)
}

/// The knee-fold gate: > 0 during (roughly) the leg's swing phase, peaking
/// just before mid-swing. `cycle` is the thigh's own phase in radians.
fn shin_gate(cycle: f32) -> f32 {
    (cycle + SHIN_GATE_ADVANCE_RAD).cos().max(0.0)
}

/// §8 carriage: the two publicly-visible statuses that reshape the walk L0/L1
/// already computed. Fed from [`crate::smart_actors::model::ActorSnapshot`]
/// via [`BodyPoseState::set_carriage`]. `default` is sober and rested, and
/// every field below is scaled by a status, so a defaulted `Carriage` leaves
/// the pose byte-identical.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Carriage {
    /// `0..=1`. Staggering phase noise, lateral sway, a wandering lean.
    drunkenness: f32,
    /// `0..=1`. Drops the arm swing and adds a forward stoop.
    weariness: f32,
}

impl Carriage {
    /// Build from a snapshot's `statuses` slice; unknown kinds are ignored (a
    /// body renders only what it has a pose for). Values are already clamped to
    /// `0..=1` at the boundary.
    fn from_statuses(statuses: &[(cathedral_sim::StatusKind, f32)]) -> Self {
        use cathedral_sim::StatusKind as K;
        let mut carriage = Carriage::default();
        for &(kind, value) in statuses {
            match kind {
                K::Drunkenness => carriage.drunkenness = value,
                K::Weariness => carriage.weariness = value,
            }
        }
        carriage
    }
}

/// Arm-swing amplitude after weariness drops it toward 0.3× at `w = 1`
/// (`1 − WEARY_ARM_DROP·w`). Identity at `w = 0`.
fn weary_arm_swing(base: f32, weariness: f32) -> f32 {
    base * (1.0 - WEARY_ARM_DROP * weariness)
}

/// The drunk stagger's phase offset (stride-cycle units), added to `gait_phase`
/// before the cycle is taken: a fast small jitter plus a slow cadence drift.
/// Scaled by drunkenness by the caller, so this raw wobble is bounded by
/// `DRUNK_PHASE_NOISE_CYCLES + DRUNK_CADENCE_CYCLES`.
fn drunk_phase_wobble(now: f64, seed: u32) -> f32 {
    use std::f64::consts::TAU as TAU64;
    let noise = ((now * DRUNK_PHASE_NOISE_HZ + f64::from(hash01(seed, 40))) * TAU64).sin() as f32;
    let cadence = ((now * DRUNK_CADENCE_HZ + f64::from(hash01(seed, 41))) * TAU64).sin() as f32;
    DRUNK_PHASE_NOISE_CYCLES * noise + DRUNK_CADENCE_CYCLES * cadence
}

/// A seeded low-frequency wander in [−1, 1] — two incommensurate sines centred
/// on zero (the `talk_energy` shape, but not biased positive). The drunk's lean
/// holds to one side, drifts, and slowly crosses over.
fn slow_wander(now: f64, seed: u32, salt: u32) -> f32 {
    use std::f64::consts::TAU as TAU64;
    let a = ((now * (0.13 + 0.08 * f64::from(hash01(seed, salt)))
        + f64::from(hash01(seed, salt.wrapping_add(1))))
        * TAU64)
        .sin();
    let b = ((now * (0.07 + 0.05 * f64::from(hash01(seed, salt.wrapping_add(2))))
        + f64::from(hash01(seed, salt.wrapping_add(3))))
        * TAU64)
        .sin();
    (0.5 * (a + b) as f32).clamp(-1.0, 1.0)
}

/// The carriage torso offset read by both L0 and L1: an extra roll (the drunk
/// sway plus a wandering lean, clamped) and an extra pitch (the weary forward
/// stoop, negative like the bow). Both vanish at status 0, so a defaulted
/// `Carriage` returns `(0.0, 0.0)` and the walk/idle torso is untouched.
fn carriage_torso(carriage: Carriage, now: f64, seed: u32) -> (f32, f32) {
    use std::f64::consts::TAU as TAU64;
    let d = carriage.drunkenness;
    let sway =
        d * DRUNK_SWAY_RAD * ((now * DRUNK_SWAY_HZ + f64::from(hash01(seed, 42))) * TAU64).sin() as f32;
    let lean = d * DRUNK_LEAN_RAD * slow_wander(now, seed, 43);
    let roll = (sway + lean).clamp(-DRUNK_ROLL_MAX_RAD, DRUNK_ROLL_MAX_RAD);
    let stoop = -WEARY_STOOP_RAD * carriage.weariness;
    (roll, stoop)
}

/// L0 locomotion (§5): legs in opposite phase, lagged knee folds, arm
/// counter-swing, torso bob at double frequency plus lateral roll and turn
/// lean. Positive local-X rotation swings a hanging limb toward −Z (forward).
/// `carriage` (§8) is read here, not as a separate layer: drunkenness staggers
/// the visual phase and sways/leans the torso, weariness drops the arm swing
/// and stoops the torso — all zero at a default `Carriage`, so a sober walk is
/// byte-identical to before M5.
fn apply_locomotion(
    pose: &mut PoseDeltas,
    weight: f32,
    gait_phase: f32,
    yaw_rate: f32,
    carriage: Carriage,
    now: f64,
    seed: u32,
) {
    // Drunkenness staggers the visual gait phase; sober, the offset is 0.
    let cycle = (gait_phase + carriage.drunkenness * drunk_phase_wobble(now, seed)) * TAU;
    let swing = cycle.sin();

    pose.left_thigh
        .blend_over(JointDelta::from_rotation(Quat::from_rotation_x(THIGH_SWING_RAD * swing)), weight);
    pose.right_thigh
        .blend_over(JointDelta::from_rotation(Quat::from_rotation_x(-THIGH_SWING_RAD * swing)), weight);
    pose.left_shin.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(-SHIN_BEND_RAD * shin_gate(cycle))),
        weight,
    );
    pose.right_shin.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(-SHIN_BEND_RAD * shin_gate(cycle + PI))),
        weight,
    );
    // Weariness drops the swing toward 0.3×; rested, the amplitude is unchanged.
    let arm_swing = weary_arm_swing(ARM_SWING_RAD, carriage.weariness);
    pose.left_upper_arm.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(-arm_swing * swing)),
        weight,
    );
    pose.right_upper_arm.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(arm_swing * swing)),
        weight,
    );

    // Highest as the legs pass, dipping when they spread; the roll shifts the
    // torso over the planted foot; the lean tips it into a turn. Carriage adds
    // the drunk sway/lean (roll) and the weary stoop (pitch) — both 0 sober.
    let bob = BOB_AMPLITUDE_M * (0.5 - swing.abs());
    let lean = (yaw_rate * TURN_LEAN_PER_YAW_RATE).clamp(-TURN_LEAN_MAX_RAD, TURN_LEAN_MAX_RAD);
    let (carriage_roll, carriage_pitch) = carriage_torso(carriage, now, seed);
    let roll = -WALK_ROLL_RAD * cycle.cos() + lean + carriage_roll;
    pose.torso.blend_over(
        JointDelta {
            rotation: Quat::from_rotation_z(roll) * Quat::from_rotation_x(carriage_pitch),
            translation: Vec3::new(0.0, bob, 0.0),
            scale: Vec3::ONE,
        },
        weight,
    );
}

/// A once-per-cycle bump: 0 outside the window, ramping in and out inside it.
/// Returns the envelope and the cycle index (for alternating sides).
fn idle_pulse(t: f64, period: f64, phase: f64, window: (f32, f32, f32, f32)) -> (f32, u64) {
    let cycles = t / period + phase;
    let u = cycles.fract() as f32;
    let (rise0, rise1, fall0, fall1) = window;
    let envelope = smoothstep(rise0, rise1, u) * (1.0 - smoothstep(fall0, fall1, u));
    (envelope, cycles as u64)
}

/// L1 idle life (§4): breathing, occasional weight shift, rare head glance —
/// every period and phase drawn from the actor-id seed so crowds don't sync.
/// `carriage` (§8) is read here too so a *standing* drunk still sways and a
/// weary body still stoops; zero at a default `Carriage`.
fn apply_idle(pose: &mut PoseDeltas, weight: f32, now: f64, seed: u32, carriage: Carriage) {
    // Breathing: a gentle chest expansion with a hint of pitch.
    let breath_period = BREATH_PERIOD_BASE_S + BREATH_PERIOD_SPAN_S * hash01(seed, 1) as f64;
    let breath = ((now / breath_period + hash01(seed, 2) as f64) * TAU as f64).sin() as f32;
    pose.torso.blend_over(
        JointDelta {
            rotation: Quat::from_rotation_x(BREATH_PITCH_RAD * breath),
            translation: Vec3::ZERO,
            scale: Vec3::new(1.0 + BREATH_SCALE * breath, 1.0, 1.0 + BREATH_SCALE * breath),
        },
        weight,
    );

    // Weight shift: hips drift to alternating sides and hold a while.
    let shift_period = SHIFT_PERIOD_BASE_S + SHIFT_PERIOD_SPAN_S * hash01(seed, 3) as f64;
    let (shift, shift_cycle) = idle_pulse(
        now,
        shift_period,
        hash01(seed, 4) as f64,
        (0.08, 0.25, 0.60, 0.80),
    );
    let shift_side = if shift_cycle % 2 == 0 { 1.0 } else { -1.0 };
    pose.pelvis.blend_over(
        JointDelta {
            rotation: Quat::IDENTITY,
            translation: Vec3::new(shift_side * SHIFT_PELVIS_X_M * shift, 0.0, 0.0),
            scale: Vec3::ONE,
        },
        weight,
    );
    // The torso leans slightly over the loaded hip; composed with the breath
    // delta already blended above rather than replacing it.
    let torso_shift = Quat::from_rotation_z(-shift_side * SHIFT_TORSO_ROLL_RAD * shift);
    pose.torso.rotation = pose
        .torso
        .rotation
        .slerp(torso_shift * pose.torso.rotation, weight);

    // Rare glance: the head turns to an alternating side and holds a beat.
    let glance_period = GLANCE_PERIOD_BASE_S + GLANCE_PERIOD_SPAN_S * hash01(seed, 5) as f64;
    let (glance, glance_cycle) = idle_pulse(
        now,
        glance_period,
        hash01(seed, 6) as f64,
        (0.04, 0.10, 0.18, 0.26),
    );
    let glance_side = if glance_cycle % 2 == 0 { 1.0 } else { -1.0 };
    let glance_yaw = glance_side
        * GLANCE_YAW_RAD
        * (0.6 + 0.4 * hash01(seed ^ (glance_cycle as u32), 7))
        * glance;
    pose.head.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_y(glance_yaw) * Quat::from_rotation_x(GLANCE_PITCH_RAD * glance),
        ),
        weight,
    );

    // Carriage (§8): a standing drunk sways and leans, a weary body stoops.
    // Composed *over* the idle torso rather than replacing it (like the weight
    // shift above), so breathing survives; identity at a default `Carriage`.
    if carriage != Carriage::default() {
        let (carriage_roll, carriage_pitch) = carriage_torso(carriage, now, seed);
        let carriage_rot =
            Quat::from_rotation_z(carriage_roll) * Quat::from_rotation_x(carriage_pitch);
        pose.torso.rotation = pose
            .torso
            .rotation
            .slerp(carriage_rot * pose.torso.rotation, weight);
    }
}

/// Local-frame aim of a world point from the root transform: signed yaw off
/// the facing (−Z; matches the sim's `(-x).atan2(-z)` convention) and the
/// elevation pitch, both clamped to the offer arm's reach.
fn offer_aim(root: &Transform, at: Vec3) -> (f32, f32) {
    let local = root.rotation.inverse() * (at - root.translation);
    let yaw = (-local.x).atan2(-local.z);
    let horizontal = (local.x * local.x + local.z * local.z).sqrt().max(1e-4);
    let pitch = local.y.atan2(horizontal);
    (
        yaw.clamp(-OFFER_YAW_CLAMP_RAD, OFFER_YAW_CLAMP_RAD),
        pitch.clamp(-OFFER_PITCH_CLAMP_RAD, OFFER_PITCH_CLAMP_RAD),
    )
}

/// L2 carry (§6): the left arm owns the basket-carry — at full weight it
/// replaces the L0 arm swing outright (`blend_over`'s override rule), which is
/// exactly the "suppressed on any arm owned by L2" behavior §5 asks for.
fn apply_carry(pose: &mut PoseDeltas, weight: f32) {
    pose.left_upper_arm.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(CARRY_UPPER_PITCH_RAD)),
        weight,
    );
    pose.left_forearm.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(CARRY_FOREARM_BEND_RAD)),
        weight,
    );
}

/// L2 offer (§6): the right arm extends toward the recipient — lifted almost
/// horizontal, yawed by the (pre-clamped) local aim, elbow straightened so the
/// prop in the hand anchor is visibly held out.
fn apply_offer(pose: &mut PoseDeltas, weight: f32, yaw: f32, pitch: f32) {
    pose.right_upper_arm.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(OFFER_LIFT_RAD + pitch),
        ),
        weight,
    );
    pose.right_forearm.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(OFFER_FOREARM_STRAIGHTEN_RAD)),
        weight,
    );
}

/// One-shot weight envelope: ramps in over the first ~0.15 s, out over the
/// final ~0.2 s, zero at both ends.
fn one_shot_weight(t: f32, duration: f32) -> f32 {
    smoothstep(0.0, GESTURE_RAMP_IN_S, t)
        * (1.0 - smoothstep(duration - GESTURE_RAMP_OUT_S, duration, t))
}

/// L3 one-shot pose (M2 nod/head-shake, M4's deliberate catalog). `u` is
/// normalized time 0..1; `face_yaw` the clamped local yaw toward the optional
/// target (the aiming kinds swing the shoulder by it). Upper-body only: legs
/// are left to L0/L1 so a walking wave still walks.
fn apply_one_shot(
    pose: &mut PoseDeltas,
    kind: OneShotGesture,
    u: f32,
    weight: f32,
    face_yaw: f32,
) {
    match kind {
        // Two small forward dips — |sin| keeps every beat downward.
        OneShotGesture::Nod => pose.head.blend_over(
            JointDelta::from_rotation(
                Quat::from_rotation_y(face_yaw)
                    * Quat::from_rotation_x(-NOD_PITCH_RAD * (u * TAU).sin().abs()),
            ),
            weight,
        ),
        // One-and-a-half side-to-side sweeps around the target direction.
        OneShotGesture::ShakeHead => pose.head.blend_over(
            JointDelta::from_rotation(Quat::from_rotation_y(
                face_yaw + SHAKE_YAW_RAD * (u * TAU * 1.5).sin(),
            )),
            weight,
        ),
        // Right arm up and out, the hand waving side to side, aimed at target.
        OneShotGesture::Wave => {
            pose.right_upper_arm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_y(face_yaw)
                        * Quat::from_rotation_z(WAVE_UPPER_OUT_RAD)
                        * Quat::from_rotation_x(WAVE_UPPER_LIFT_RAD),
                ),
                weight,
            );
            pose.right_forearm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_x(-WAVE_FOREARM_BEND_RAD)
                        * Quat::from_rotation_z(WAVE_FOREARM_SWING_RAD * (u * TAU * WAVE_CYCLES).sin()),
                ),
                weight,
            );
        }
        // Arm held forward-up, forearm curling in and out (come here).
        OneShotGesture::Beckon => {
            pose.right_upper_arm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_y(face_yaw) * Quat::from_rotation_x(BECKON_UPPER_LIFT_RAD),
                ),
                weight,
            );
            let curl = BECKON_FOREARM_BASE_RAD
                + BECKON_FOREARM_CURL_RAD * 0.5 * (1.0 - (u * TAU * BECKON_CYCLES).cos());
            pose.right_forearm
                .blend_over(JointDelta::from_rotation(Quat::from_rotation_x(-curl)), weight);
        }
        // Both shoulders lift and abduct, forearms turning out — a held shrug.
        OneShotGesture::Shrug => {
            pose.left_upper_arm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_z(-SHRUG_UPPER_OUT_RAD)
                        * Quat::from_rotation_x(SHRUG_UPPER_LIFT_RAD),
                ),
                weight,
            );
            pose.right_upper_arm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_z(SHRUG_UPPER_OUT_RAD)
                        * Quat::from_rotation_x(SHRUG_UPPER_LIFT_RAD),
                ),
                weight,
            );
            pose.left_forearm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_x(-SHRUG_FOREARM_BEND_RAD)
                        * Quat::from_rotation_z(-SHRUG_FOREARM_OUT_RAD),
                ),
                weight,
            );
            pose.right_forearm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_x(-SHRUG_FOREARM_BEND_RAD)
                        * Quat::from_rotation_z(SHRUG_FOREARM_OUT_RAD),
                ),
                weight,
            );
            pose.head.blend_over(
                JointDelta::from_rotation(Quat::from_rotation_z(SHRUG_HEAD_TILT_RAD)),
                weight,
            );
        }
        // Right arm extended straight toward the target, held.
        OneShotGesture::Point => {
            pose.right_upper_arm.blend_over(
                JointDelta::from_rotation(
                    Quat::from_rotation_y(face_yaw) * Quat::from_rotation_x(POINT_UPPER_LIFT_RAD),
                ),
                weight,
            );
            pose.right_forearm.blend_over(
                JointDelta::from_rotation(Quat::from_rotation_x(POINT_FOREARM_STRAIGHTEN_RAD)),
                weight,
            );
            pose.head.blend_over(
                JointDelta::from_rotation(Quat::from_rotation_y(face_yaw)),
                weight,
            );
        }
        // The torso folds forward at the waist, the head dropping with it.
        // Model-forward is −Z, and the upright torso pivots the opposite way to
        // a down-hanging arm, so folding forward is a negative X-rotation.
        OneShotGesture::Bow => {
            pose.torso.blend_over(
                JointDelta::from_rotation(Quat::from_rotation_x(-BOW_TORSO_PITCH_RAD)),
                weight,
            );
            pose.head.blend_over(
                JointDelta::from_rotation(Quat::from_rotation_x(-BOW_HEAD_PITCH_RAD)),
                weight,
            );
        }
    }
}

/// L3 looping dance (M4): an upper-body sway — torso roll and twist, a pelvis
/// bob and side-sway, both arms up and swinging in opposition, and a head bob —
/// driven while the snapshot's `active_gesture` is `dance`. Legs are left to
/// L0/L1 (foot IK is a non-goal), so a dancer standing in place still stands.
fn apply_dance(pose: &mut PoseDeltas, weight: f32, now: f64, seed: u32) {
    let phase = now * DANCE_HZ * std::f64::consts::TAU + f64::from(seed % 1000) * 0.0063;
    let swing = phase.sin() as f32;
    let bob = (phase * 2.0).sin().abs() as f32;
    let twist = (phase * 0.5).sin() as f32;
    let head = (phase * 2.0).cos() as f32;

    pose.torso.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_z(DANCE_TORSO_ROLL_RAD * swing)
                * Quat::from_rotation_y(DANCE_TORSO_TWIST_RAD * twist),
        ),
        weight,
    );
    pose.pelvis.blend_over(
        JointDelta {
            translation: Vec3::new(DANCE_PELVIS_SWAY_M * swing, DANCE_PELVIS_BOB_M * bob, 0.0),
            ..Default::default()
        },
        weight,
    );
    pose.left_upper_arm.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_z(-DANCE_UPPER_SWING_RAD * swing)
                * Quat::from_rotation_x(DANCE_UPPER_LIFT_RAD),
        ),
        weight,
    );
    pose.right_upper_arm.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_z(DANCE_UPPER_SWING_RAD * swing)
                * Quat::from_rotation_x(DANCE_UPPER_LIFT_RAD),
        ),
        weight,
    );
    let forearm = JointDelta::from_rotation(Quat::from_rotation_x(-DANCE_FOREARM_BEND_RAD));
    pose.left_forearm.blend_over(forearm, weight);
    pose.right_forearm.blend_over(forearm, weight);
    pose.head.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(DANCE_HEAD_BOB_RAD * head)),
        weight,
    );
}

/// The nearest-[`TIER_A_CAP`] cut: given `(distance², key)` pairs already
/// inside the Tier A radius, keep the closest cap's worth.
fn tier_a_cut<T: Copy + Eq + std::hash::Hash>(candidates: &mut Vec<(f32, T)>) -> HashSet<T> {
    if candidates.len() > TIER_A_CAP {
        candidates.select_nth_unstable_by(TIER_A_CAP - 1, |a, b| a.0.total_cmp(&b.0));
        candidates.truncate(TIER_A_CAP);
    }
    candidates.iter().map(|(_, key)| *key).collect()
}

/// Writes `rest ∘ delta` to every animated part of one rig.
fn write_pose(
    parts: &mut Query<&mut Transform, Without<ActorView>>,
    rig: &BodyRig,
    rest: &RestPose,
    pose: &PoseDeltas,
) {
    let mut write = |entity: Entity, rest: &Transform, delta: &JointDelta| {
        if let Ok(mut transform) = parts.get_mut(entity) {
            transform.translation = rest.translation + delta.translation;
            transform.rotation = rest.rotation * delta.rotation;
            transform.scale = rest.scale * delta.scale;
        }
    };
    write(rig.pelvis, &rest.pelvis, &pose.pelvis);
    write(rig.torso, &rest.torso, &pose.torso);
    write(rig.head, &rest.head, &pose.head);
    write(rig.left_upper_arm, &rest.left_upper_arm, &pose.left_upper_arm);
    write(rig.right_upper_arm, &rest.right_upper_arm, &pose.right_upper_arm);
    write(rig.left_forearm, &rest.left_forearm, &pose.left_forearm);
    write(rig.right_forearm, &rest.right_forearm, &pose.right_forearm);
    write(rig.left_thigh, &rest.left_thigh, &pose.left_thigh);
    write(rig.right_thigh, &rest.right_thigh, &pose.right_thigh);
    write(rig.left_shin, &rest.left_shin, &pose.left_shin);
    write(rig.right_shin, &rest.right_shin, &pose.right_shin);
}

/// Rolling cost window for the §9 frame-budget diagnostic.
#[derive(Default)]
pub(crate) struct PoseTimer {
    window_start: f64,
    accum_us: f64,
    max_us: f64,
    frames: u32,
    tier_a: usize,
    tier_b: usize,
}

/// The pose system (M1): evaluates the layer stack per actor and writes part
/// transforms, LOD-tiered by camera distance.
///
/// Runs in the `Present` set — pure cosmetics: it never touches the ROOT
/// (owned by `reconcile_actor_views` + `drive_npc_bodies`), never reads or
/// writes the mirror, and skipping it entirely would change nothing the sim
/// or another mind could see.
#[allow(clippy::too_many_arguments)]
pub(crate) fn animate_body_pose(
    time: Res<Time>,
    inbox: Res<MovementInbox>,
    reflex: Res<ReflexState>,
    cameras: Query<&GlobalTransform, With<PlayerCamera>>,
    mut roots: Query<(Entity, &ActorId, &Transform, &BodyRig, &mut BodyPoseState), With<ActorView>>,
    mut parts: Query<&mut Transform, Without<ActorView>>,
    mut frame: Local<u64>,
    mut timer: Local<PoseTimer>,
) {
    let started = Instant::now();
    *frame = frame.wrapping_add(1);
    let Ok(camera) = cameras.single() else {
        return;
    };
    let camera_position = camera.translation();
    let now = time.elapsed_secs_f64();

    // Tier A membership first: the cap needs the whole field ranked. The same
    // pass collects live eye points for everyone a gaze may track (speakers
    // and their addressees) — usually zero or a handful of ids.
    let gaze_ids = reflex.referenced_ids(now);
    let mut gaze_points: HashMap<ActorId, Vec3> = HashMap::new();
    let mut tier_a_candidates: Vec<(f32, Entity)> = Vec::new();
    for (entity, actor_id, transform, _, _) in roots.iter() {
        let distance_sq = camera_position.distance_squared(transform.translation);
        if distance_sq < TIER_A_DISTANCE_M * TIER_A_DISTANCE_M {
            tier_a_candidates.push((distance_sq, entity));
        }
        if gaze_ids.contains(actor_id) {
            gaze_points.insert(
                actor_id.clone(),
                transform.translation + Vec3::Y * GAZE_EYE_OFFSET_Y,
            );
        }
    }
    let tier_a = tier_a_cut(&mut tier_a_candidates);
    // An actor id becomes a live world point; the bodiless player is the
    // camera. Anyone else unresolved (not spawned) draws no gaze.
    let resolve = |id: &ActorId| -> Option<Vec3> {
        if id.0 == super::PLAYER_ID {
            return Some(camera_position);
        }
        gaze_points.get(id).copied()
    };

    let mut tier_a_count = 0usize;
    let mut tier_b_count = 0usize;
    for (entity, actor_id, transform, rig, mut state) in &mut roots {
        let distance_sq = camera_position.distance_squared(transform.translation);
        let tier = if tier_a.contains(&entity) {
            PoseTier::A
        } else if distance_sq < TIER_B_DISTANCE_M * TIER_B_DISTANCE_M {
            PoseTier::B
        } else {
            PoseTier::C
        };
        let previous_tier = state.tier;
        state.tier = tier;
        // The transient upper-body layers (L2 carry/offer, L3 dance, L4
        // talk/gaze) only ramp inside the Tier A block below. If we let their
        // weights freeze while a body sits off-stage, an activity that ends
        // there (speech finishes, a dance verb clears) would leave the weight
        // stuck at 1.0 while its target has flipped to 0 — so the body would
        // replay a spurious partial beat on the first Tier A frame back. Collapse
        // them the moment a body leaves Tier A so it re-enters each layer from a
        // weight consistent with its current target (a still-active layer simply
        // ramps back in, per the "gestures/gaze ramp in and OUT" intent).
        if tier != PoseTier::A {
            state.carry_blend = 0.0;
            state.offer_blend = 0.0;
            state.talk_blend = 0.0;
            state.gaze_blend = 0.0;
            state.dance_blend = 0.0;
        }
        match tier {
            PoseTier::C => {
                // Leaving the animated tiers: settle to rest once, then stop
                // writing entirely (the fade owns everything past 120 m).
                if previous_tier != PoseTier::C && !state.at_rest {
                    write_pose(&mut parts, rig, &state.rest, &PoseDeltas::default());
                    state.at_rest = true;
                }
                continue;
            }
            PoseTier::B => {
                tier_b_count += 1;
                // L0 only, every other frame; the parity is seeded so the
                // skipped half of the crowd differs frame to frame.
                if (frame.wrapping_add(u64::from(state.seed))) & 1 == 1 {
                    continue;
                }
            }
            PoseTier::A => tier_a_count += 1,
        }

        let state = &mut *state;
        let dt = ((now - state.last_eval).clamp(0.0, 0.25)) as f32;
        state.last_eval = now;

        // Interpolate the 20 Hz gait samples at frame rate, exactly like the
        // root position sweep, and derive a smoothed yaw rate for turn lean.
        let mut gait_phase = 0.0_f32;
        let mut speed = 0.0_f32;
        let mut target_yaw_rate = 0.0_f32;
        if let Some(sample) = inbox.0.get(actor_id) {
            let history = state.history.get_or_insert(GaitHistory {
                prev_phase: sample.gait_phase,
                cur_phase: sample.gait_phase,
                prev_yaw: sample.facing_yaw,
                cur_yaw: sample.facing_yaw,
                speed: sample.speed,
                t0: now,
                seq: sample.seq,
            });
            if sample.seq != history.seq {
                // After a long gap (the mover stood a while, or we skipped
                // them in Tier C) sweeping from the stale sample would thrash
                // the legs; snap instead.
                let stale = now - history.t0 > SAMPLE_STALE_SECONDS;
                history.prev_phase = if stale { sample.gait_phase } else { history.cur_phase };
                history.prev_yaw = if stale { sample.facing_yaw } else { history.cur_yaw };
                history.cur_phase = sample.gait_phase;
                history.cur_yaw = sample.facing_yaw;
                history.speed = sample.speed;
                history.t0 = now;
                history.seq = sample.seq;
            }
            let fresh = now - history.t0 <= SAMPLE_STALE_SECONDS;
            let t = ((now - history.t0) / MOVEMENT_TICK_SECONDS).clamp(0.0, 1.0) as f32;
            gait_phase = history.prev_phase + (history.cur_phase - history.prev_phase) * t;
            if fresh {
                speed = history.speed;
                target_yaw_rate = angle_delta(history.prev_yaw, history.cur_yaw)
                    / MOVEMENT_TICK_SECONDS as f32;
            }
        }
        state.yaw_rate +=
            (target_yaw_rate - state.yaw_rate) * (dt * YAW_RATE_SMOOTHING_PER_S).min(1.0);
        state.walk_blend = move_toward(state.walk_blend, walk_factor(speed), dt / WALK_BLEND_SECONDS);

        // The layer stack (§4). Idle life is Tier A only; at those weights a
        // Tier B body simply holds rest between strides.
        let locomotion_weight = state.walk_blend;
        let idle_weight = if tier == PoseTier::A {
            1.0 - state.walk_blend
        } else {
            0.0
        };
        if locomotion_weight <= 1e-3 && idle_weight <= 1e-3 {
            if !state.at_rest {
                write_pose(&mut parts, rig, &state.rest, &PoseDeltas::default());
                state.at_rest = true;
            }
            continue;
        }

        let mut pose = PoseDeltas::default();
        if locomotion_weight > 1e-3 {
            apply_locomotion(
                &mut pose,
                locomotion_weight,
                gait_phase,
                state.yaw_rate,
                state.carriage,
                now,
                state.seed,
            );
        }
        if idle_weight > 1e-3 {
            apply_idle(&mut pose, idle_weight, now, state.seed, state.carriage);
        }
        // L2 activity (M2) + L4 speech & gaze (M3) + L3 one-shot gestures,
        // Tier A only per §9 — at Tier B distances they fail the §1
        // readability test anyway. M4 extends the one-shot catalog.
        if tier == PoseTier::A {
            let offer_at = state.offer_target.or(match state.offer_pulse {
                Some((at, until)) if now < until => Some(at),
                _ => None,
            });
            state.carry_blend = move_toward(
                state.carry_blend,
                if state.carry_target { 1.0 } else { 0.0 },
                dt / ARM_BLEND_SECONDS,
            );
            state.offer_blend = move_toward(
                state.offer_blend,
                if offer_at.is_some() { 1.0 } else { 0.0 },
                dt / ARM_BLEND_SECONDS,
            );
            if let Some(at) = offer_at {
                state.offer_aim = offer_aim(transform, at);
            }
            if state.carry_blend > 1e-3 {
                apply_carry(&mut pose, state.carry_blend);
            }
            if state.offer_blend > 1e-3 {
                let (yaw, pitch) = state.offer_aim;
                apply_offer(&mut pose, state.offer_blend, yaw, pitch);
            }
            // L4 speech & gaze (M3), Tier A only. Evaluated before the L3
            // one-shot on purpose — a nod or head-shake is a *communicative
            // beat* that must stay readable over ambient head tracking, and
            // its envelope is closed at both ends so it hands the head back
            // to the gaze smoothly (the one §4 ordering deviation).
            let talking = reflex.is_talking(actor_id, now);
            state.talk_blend = move_toward(
                state.talk_blend,
                if talking { 1.0 } else { 0.0 },
                dt / TALK_BLEND_SECONDS,
            );
            let gaze_query = GazeQuery {
                position: transform.translation,
                offer_at,
                idle: state.walk_blend < 0.5,
                seed: state.seed,
            };
            let aim = reflex
                .gaze_point(actor_id, &gaze_query, now, &resolve)
                .and_then(|at| gaze_aim(transform, at));
            state.gaze_blend = move_toward(
                state.gaze_blend,
                if aim.is_some() { 1.0 } else { 0.0 },
                dt / GAZE_BLEND_SECONDS,
            );
            if let Some((yaw, pitch)) = aim {
                let track = (dt * GAZE_TRACK_PER_S).min(1.0);
                state.gaze_yaw += (yaw - state.gaze_yaw) * track;
                state.gaze_pitch += (pitch - state.gaze_pitch) * track;
            }
            if state.gaze_blend > 1e-3 {
                pose.head.blend_over(
                    JointDelta::from_rotation(
                        Quat::from_rotation_y(state.gaze_yaw)
                            * Quat::from_rotation_x(state.gaze_pitch),
                    ),
                    state.gaze_blend,
                );
            }
            if state.talk_blend > 1e-3 {
                apply_talk(
                    &mut pose,
                    state.talk_blend,
                    1.0 - state.carry_blend,
                    1.0 - state.offer_blend,
                    now,
                    state.seed,
                );
            }
            // L3 looping dance (M4), driven by the snapshot's `active_gesture`
            // rather than a fixed envelope. Applied before the one-shot so a
            // deliberate wave — which ends the loop sim-side — reads over it
            // during the brief blend-out.
            state.dance_blend = move_toward(
                state.dance_blend,
                if state.dance { 1.0 } else { 0.0 },
                dt / DANCE_BLEND_SECONDS,
            );
            if state.dance_blend > 1e-3 {
                apply_dance(&mut pose, state.dance_blend, now, state.seed);
            }
            if let Some(gesture) = state.gesture {
                let t = (now - gesture.started_at) as f32;
                let duration = gesture.kind.duration_seconds();
                if t >= duration {
                    state.gesture = None;
                } else {
                    let face_yaw = gesture
                        .face
                        .map(|at| {
                            offer_aim(transform, at).0.clamp(
                                -GESTURE_FACE_YAW_CLAMP_RAD,
                                GESTURE_FACE_YAW_CLAMP_RAD,
                            )
                        })
                        .unwrap_or(0.0);
                    apply_one_shot(
                        &mut pose,
                        gesture.kind,
                        t / duration,
                        one_shot_weight(t, duration),
                        face_yaw,
                    );
                }
            }
        }
        write_pose(&mut parts, rig, &state.rest, &pose);
        state.at_rest = false;
    }

    // §9 budget diagnostic: a cheap rolling average, printed every few
    // seconds so a release run on the forecourt reports the real cost.
    let elapsed_us = started.elapsed().as_secs_f64() * 1e6;
    timer.accum_us += elapsed_us;
    timer.max_us = timer.max_us.max(elapsed_us);
    timer.frames += 1;
    timer.tier_a = tier_a_count;
    timer.tier_b = tier_b_count;
    if now - timer.window_start >= POSE_TIMING_LOG_SECONDS {
        if timer.window_start > 0.0 {
            info!(
                "[body pose] avg {:.0} us, max {:.0} us over {} frames (tier A {}, tier B {})",
                timer.accum_us / f64::from(timer.frames.max(1)),
                timer.max_us,
                timer.frames,
                timer.tier_a,
                timer.tier_b,
            );
        }
        timer.window_start = now;
        timer.accum_us = 0.0;
        timer.max_us = 0.0;
        timer.frames = 0;
    }
}

// ---------------------------------------------------------------------------
// Mesh builders
// ---------------------------------------------------------------------------

/// Scales every UV so the cloth weave tiles at a consistent physical size.
fn scale_uvs(mesh: &mut Mesh, scale: Vec2) {
    if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        for uv in uvs.iter_mut() {
            uv[0] *= scale.x;
            uv[1] *= scale.y;
        }
    }
}

/// A thin capsule with its origin baked at the top joint (the ball the part
/// pivots around), hanging down −Y.
fn limb_mesh(radius: f32, length: f32) -> Mesh {
    let mut mesh = Capsule3d::new(radius, length)
        .mesh()
        .build()
        .translated_by(Vec3::new(0.0, -length * 0.5, 0.0));
    scale_uvs(
        &mut mesh,
        Vec2::new(
            TAU * radius / CLOTH_TILE_M,
            (length + 2.0 * radius) / CLOTH_TILE_M,
        ),
    );
    mesh
}

fn pelvis_mesh() -> Mesh {
    let mut mesh = Cuboid::new(PELVIS_SIZE.x, PELVIS_SIZE.y, PELVIS_SIZE.z)
        .mesh()
        .build();
    let span = PELVIS_SIZE.x.max(PELVIS_SIZE.y) / CLOTH_TILE_M;
    scale_uvs(&mut mesh, Vec2::splat(span));
    mesh
}

fn mesh_from_parts(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

/// The tapered torso: an elliptical frustum, origin at the waist (its base),
/// wider at the shoulders, capped top and bottom.
fn torso_mesh() -> Mesh {
    // (fraction of height, half-width). Shoulders round off at the top.
    const PROFILE: [(f32, f32); 7] = [
        (0.00, 0.150),
        (0.15, 0.158),
        (0.35, 0.175),
        (0.55, 0.196),
        (0.75, 0.220),
        (0.90, 0.235),
        (1.00, 0.226),
    ];
    const SECTORS: usize = 24;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let ring_stride = (SECTORS + 1) as u32;
    let mean_circumference = TAU * 0.19;

    for (ring, (t, rx)) in PROFILE.iter().enumerate() {
        let y = t * TORSO_HEIGHT;
        let rz = rx * TORSO_DEPTH_RATIO;
        // Profile slope for the normal's vertical component.
        let (prev, next) = (
            PROFILE[ring.saturating_sub(1)],
            PROFILE[(ring + 1).min(PROFILE.len() - 1)],
        );
        let dy = ((next.0 - prev.0) * TORSO_HEIGHT).max(1e-4);
        let slope = -(next.1 - prev.1) / dy;
        for sector in 0..=SECTORS {
            let a = sector as f32 / SECTORS as f32 * TAU;
            let (sin, cos) = a.sin_cos();
            positions.push([rx * cos, y, rz * sin]);
            // Ellipse outward direction, tilted by the taper slope.
            let n = Vec3::new(cos * rz, slope * rx.min(rz) * 0.9, sin * rx).normalize();
            normals.push([n.x, n.y, n.z]);
            uvs.push([
                a / TAU * mean_circumference / CLOTH_TILE_M,
                (TORSO_HEIGHT - y) / CLOTH_TILE_M,
            ]);
        }
    }
    for ring in 0..PROFILE.len() - 1 {
        let base = ring as u32 * ring_stride;
        for sector in 0..SECTORS as u32 {
            let a = base + sector;
            let b = a + 1;
            let c = a + ring_stride;
            let d = b + ring_stride;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    // Caps: a centre fan at each end (the top shoulder plane is visible from
    // bridges and stairs).
    for (t, rx, up) in [(0.0_f32, PROFILE[0].1, false), (1.0, PROFILE[6].1, true)] {
        let y = t * TORSO_HEIGHT;
        let rz = rx * TORSO_DEPTH_RATIO;
        let normal = [0.0, if up { 1.0 } else { -1.0 }, 0.0];
        let center = positions.len() as u32;
        positions.push([0.0, y, 0.0]);
        normals.push(normal);
        uvs.push([0.5, 0.5]);
        let first_rim = positions.len() as u32;
        for sector in 0..=SECTORS {
            let a = sector as f32 / SECTORS as f32 * TAU;
            let (sin, cos) = a.sin_cos();
            positions.push([rx * cos, y, rz * sin]);
            normals.push(normal);
            uvs.push([
                (0.5 + cos * 0.5) * rx / CLOTH_TILE_M,
                (0.5 + sin * 0.5) * rx / CLOTH_TILE_M,
            ]);
        }
        for sector in 0..SECTORS as u32 {
            let (a, b) = (first_rim + sector, first_rim + sector + 1);
            if up {
                indices.extend_from_slice(&[center, a, b]);
            } else {
                indices.extend_from_slice(&[center, b, a]);
            }
        }
    }
    mesh_from_parts(positions, normals, uvs, indices)
}

/// Azimuthal face projection: maps a unit direction on the head sphere into
/// the face image. The front pole (−Z) is the image centre; the frame edge
/// lands [`FACE_EDGE_ANGLE_RAD`] away; everything beyond runs off past [0,1]
/// where the clamped sampler paints the image's uniform skin-tone border.
fn face_uv(direction: Vec3) -> Vec2 {
    let theta = (-direction.z).clamp(-1.0, 1.0).acos();
    let rho = (theta / FACE_EDGE_ANGLE_RAD).min(FACE_UV_RHO_MAX);
    let phi = direction.y.atan2(direction.x);
    Vec2::new(
        0.5 - 0.5 * rho * phi.cos(),
        0.5 - 0.5 * rho * phi.sin(),
    )
}

/// A sphere reads as a ball, not a head, so the ovoid is shaped on four axes:
/// taller than it is wide, tapered below the cheeks to a chin, its crown rounded
/// a touch narrower than the temples, and — the cue that sells it in profile —
/// the face plane (−Z) flattened while the occiput (+Z) fills out behind. The
/// width tapers all sit below the head centre, where no headgear rides, so hats
/// and hoods keep their authored fit.
const HEAD_VERTICAL_STRETCH: f32 = 1.18;
const HEAD_JAW_TAPER: f32 = 0.40;
const HEAD_CROWN_TAPER: f32 = 0.07;
const HEAD_FACE_FLATTEN: f32 = 0.90;
const HEAD_OCCIPUT_BULGE: f32 = 1.08;

/// The head — an ovoid with the painted face planar-projected onto its front
/// (−Z) hemisphere. Origin at the neck joint; the shape is centred
/// [`HEAD_CENTER_ABOVE_NECK`] above it so the head pivots at the neck. Only the
/// vertex *positions* are shaped (see [`HEAD_VERTICAL_STRETCH`] &c.); face UVs
/// come from the unit `direction`, so the projection (and its orientation
/// contract) is independent of the shape. Normals are recomputed from the
/// shaped geometry rather than derived analytically, so the multi-axis warp
/// still shades smoothly.
fn head_mesh() -> Mesh {
    const SECTORS: usize = 24;
    const STACKS: usize = 16;

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for stack in 0..=STACKS {
        let polar = stack as f32 / STACKS as f32 * PI;
        let (ring_r, y) = (polar.sin(), polar.cos());
        for sector in 0..=SECTORS {
            let a = sector as f32 / SECTORS as f32 * TAU;
            let direction = Vec3::new(ring_r * a.cos(), y, ring_r * a.sin());
            // Jaw taper: full width through the cranium (y ≥ 0), narrowing over
            // the lower head (smoothstep in −y) to a chin; the crown rounds off
            // a little narrower than the temples above the centre.
            let below = (-direction.y).clamp(0.0, 1.0);
            let jaw = 1.0 - HEAD_JAW_TAPER * below * below * (3.0 - 2.0 * below);
            let above = direction.y.clamp(0.0, 1.0);
            let crown = 1.0 - HEAD_CROWN_TAPER * above * above;
            let width = jaw * crown;
            // Depth: the face plane (−Z) sits flatter, the occiput (+Z) fuller,
            // so the silhouette reads as a head and not a globe.
            let front = (-direction.z).clamp(0.0, 1.0);
            let back = direction.z.clamp(0.0, 1.0);
            let depth = width
                * (1.0 - (1.0 - HEAD_FACE_FLATTEN) * front + (HEAD_OCCIPUT_BULGE - 1.0) * back);
            let shaped = Vec3::new(
                direction.x * width,
                direction.y * HEAD_VERTICAL_STRETCH,
                direction.z * depth,
            );
            let position = shaped * HEAD_RADIUS + Vec3::Y * HEAD_CENTER_ABOVE_NECK;
            positions.push([position.x, position.y, position.z]);
            let uv = face_uv(direction);
            uvs.push([uv.x, uv.y]);
        }
    }
    let stride = (SECTORS + 1) as u32;
    for stack in 0..STACKS as u32 {
        for sector in 0..SECTORS as u32 {
            let a = stack * stride + sector;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_indices(Indices::U32(indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_computed_smooth_normals()
}

/// A surface of revolution around +Y from a `(radius, y)` profile, with the
/// rim optionally sheared downward toward the back (+Z) so a hood can cover
/// the nape while leaving the face open. Origin at the head-sphere centre.
fn revolved_cap(profile: &[(f32, f32)], sectors: usize, back_drop: f32, uv_span: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let crown_y = profile[0].1;
    let rim_y = profile[profile.len() - 1].1;
    let depth = (crown_y - rim_y).max(1e-4);

    for (ring, (radius, y)) in profile.iter().enumerate() {
        // Ring slope for normals.
        let (prev, next) = (
            profile[ring.saturating_sub(1)],
            profile[(ring + 1).min(profile.len() - 1)],
        );
        let (dr, dy) = (next.0 - prev.0, next.1 - prev.1);
        let n = Vec2::new(-dy, dr).normalize_or(Vec2::Y);
        // How far down the cap this ring sits (0 crown, 1 rim) — the shear
        // fades in over it so the crown stays put.
        let shear_weight = (crown_y - y) / depth;
        for sector in 0..=sectors {
            let a = sector as f32 / sectors as f32 * TAU;
            let (sin, cos) = a.sin_cos();
            // Backness: 1 behind the head (+Z), 0 at the face (−Z).
            let backness = (1.0 + sin) * 0.5;
            let y_sheared = y - back_drop * backness * shear_weight;
            positions.push([radius * cos, y_sheared, radius * sin]);
            let normal = Vec3::new(n.x * cos, n.y, n.x * sin).normalize_or(Vec3::Y);
            normals.push([normal.x, normal.y, normal.z]);
            uvs.push([
                a / TAU * uv_span,
                shear_weight * uv_span,
            ]);
        }
    }
    let stride = (sectors + 1) as u32;
    for ring in 0..profile.len() as u32 - 1 {
        for sector in 0..sectors as u32 {
            let a = ring * stride + sector;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    mesh_from_parts(positions, normals, uvs, indices)
}

/// Hood: a bulky draped cowl, open at the face, draped low over the nape.
fn hood_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.305),
            (0.10, 0.295),
            (0.19, 0.262),
            (0.26, 0.203),
            (0.295, 0.135),
            (0.302, 0.100),
        ],
        20,
        0.22,
        0.6,
    )
}

/// Coif: a close-fitting linen cap tied under the skull.
fn coif_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.262),
            (0.10, 0.252),
            (0.175, 0.222),
            (0.228, 0.168),
            (0.252, 0.105),
        ],
        20,
        0.16,
        0.5,
    )
}

/// Brimmed hat: low felt crown on a wide disc.
fn brim_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.300),
            (0.12, 0.290),
            (0.185, 0.245),
            (0.195, 0.130),
            (0.350, 0.105),
        ],
        20,
        0.0,
        0.5,
    )
}

/// Kettle helm: an iron dome flaring into a sloped brim.
fn kettle_helm_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.320),
            (0.10, 0.312),
            (0.19, 0.276),
            (0.252, 0.202),
            (0.262, 0.135),
            (0.360, 0.085),
        ],
        20,
        0.0,
        0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Orientation contract for the face projection: image centre on the
    /// front pole, image top up, portrait's right on the wearer's right
    /// (+X → image left half, exactly like looking at a person).
    #[test]
    fn face_projection_puts_the_face_upright_on_the_front() {
        let center = face_uv(Vec3::NEG_Z);
        assert!((center - Vec2::splat(0.5)).length() < 1e-6);

        // Up on the head samples the top of the image (v < 0.5).
        let up_front = face_uv(Vec3::new(0.0, 0.5, -0.5).normalize());
        assert!(up_front.y < 0.45, "image is upside down: {up_front:?}");
        assert!((up_front.x - 0.5).abs() < 1e-6);

        // The wearer's right (+X) shows the image's left half.
        let right_front = face_uv(Vec3::new(0.5, 0.0, -0.5).normalize());
        assert!(right_front.x < 0.45, "face is mirrored: {right_front:?}");

        // Everything behind the head leaves [0,1]² for the clamped border …
        let back = face_uv(Vec3::Z);
        assert!(back.x < 0.0 || back.x > 1.0);
        // … and the whole rear cap stays far enough out that no triangle
        // chord across sectors can cut back into the face frame.
        let rear_rim = face_uv(Vec3::new(0.4, 0.4, 0.82).normalize());
        let radial = (rear_rim - Vec2::splat(0.5)).length();
        assert!(radial > 0.72, "rear cap too close to the face: {radial}");
    }

    /// The variety axes are deterministic and bounded: every seed lands in
    /// the shared band, and tint/face draw from different bits.
    #[test]
    fn tint_and_face_picks_are_deterministic_and_in_band() {
        for seed in [0_u32, 1, 0x80, 0xFFFF_FFFF, 0xDEAD_BEEF] {
            assert!(tint_index(seed) < OUTFIT_TINT_BAND);
            assert!(face_index(seed) < FACE_COUNT);
            assert_eq!(tint_index(seed), tint_index(seed));
            assert_eq!(face_index(seed), face_index(seed));
        }
        // Bit 7 (the sim's headgear variance bit) never changes the tint.
        assert_eq!(tint_index(0x80), tint_index(0x00));
    }

    /// §5's walk factor: settled is 0, a committed walk is 1, in between
    /// blends monotonically.
    #[test]
    fn walk_factor_blends_between_settled_and_brisk() {
        assert_eq!(walk_factor(0.0), 0.0);
        assert_eq!(walk_factor(SETTLED_SPEED_MPS as f32), 0.0);
        assert_eq!(walk_factor(WALK_FULL_SPEED_MPS), 1.0);
        assert_eq!(walk_factor(1.8), 1.0);
        let mid = walk_factor(0.375);
        assert!(mid > 0.2 && mid < 0.8, "mid blend out of band: {mid}");
    }

    /// §4's override rule: weight 1 owns the joint, weight 0 leaves it, a
    /// partial weight drags it toward the layer's pose.
    #[test]
    fn layer_blending_overrides_by_weight() {
        let target = JointDelta {
            rotation: Quat::from_rotation_x(1.0),
            translation: Vec3::new(0.0, 0.2, 0.0),
            scale: Vec3::splat(1.1),
        };
        let mut owned = JointDelta::default();
        owned.blend_over(target, 1.0);
        assert_eq!(owned, target);

        let mut untouched = JointDelta::default();
        untouched.blend_over(target, 0.0);
        assert_eq!(untouched, JointDelta::default());

        let mut half = JointDelta::default();
        half.blend_over(target, 0.5);
        assert!((half.translation.y - 0.1).abs() < 1e-6);
        assert!(half.rotation.angle_between(Quat::IDENTITY) > 0.4);
        assert!(half.rotation.angle_between(target.rotation) > 0.4);
    }

    /// The L0 contract at peak swing (phase 0.25): legs in opposite phase,
    /// arms counter-swinging, torso dipped. Positive local-X rotation swings
    /// a hanging limb toward −Z (the facing direction).
    #[test]
    fn locomotion_swings_legs_opposite_and_arms_counter() {
        let mut pose = PoseDeltas::default();
        apply_locomotion(&mut pose, 1.0, 0.25, 0.0, Carriage::default(), 0.0, 0);

        let forwardness = |delta: &JointDelta| (delta.rotation * Vec3::NEG_Y).z;
        assert!(forwardness(&pose.left_thigh) < -0.3, "left thigh not forward");
        assert!(forwardness(&pose.right_thigh) > 0.3, "right thigh not back");
        assert!(
            forwardness(&pose.left_upper_arm) > 0.1,
            "left arm must counter-swing its own leg"
        );
        assert!(forwardness(&pose.right_upper_arm) < -0.1);
        // Legs spread = the torso's low point.
        assert!(pose.torso.translation.y < -0.005);
        // Legs passing (phase 0) = the high point, and no leg swing.
        let mut passing = PoseDeltas::default();
        apply_locomotion(&mut passing, 1.0, 0.0, 0.0, Carriage::default(), 0.0, 0);
        assert!(passing.torso.translation.y > 0.005);
        assert!(forwardness(&passing.left_thigh).abs() < 1e-3);
    }

    /// Turn lean tips the torso into the turn and is clamped.
    #[test]
    fn turn_lean_follows_yaw_rate_and_clamps() {
        let roll = |yaw_rate: f32| {
            let mut pose = PoseDeltas::default();
            apply_locomotion(&mut pose, 1.0, 0.0, yaw_rate, Carriage::default(), 0.0, 0);
            pose.torso.rotation.to_euler(EulerRot::ZYX).0
        };
        let gentle = roll(0.5) - roll(0.0);
        let hard = roll(10.0) - roll(0.0);
        assert!(gentle > 0.01, "no lean into a left turn: {gentle}");
        assert!(
            hard <= TURN_LEAN_MAX_RAD + 1e-4,
            "lean must clamp: {hard}"
        );
        assert!(roll(-0.5) - roll(0.0) < -0.01);
    }

    /// L1 idle life is per-actor desynchronized (phases/periods from the id
    /// seed) and actually oscillates over time.
    #[test]
    fn idle_life_desyncs_across_actors_and_breathes() {
        let sample = |seed: u32, t: f64| {
            let mut pose = PoseDeltas::default();
            apply_idle(&mut pose, 1.0, t, seed, Carriage::default());
            pose.torso.scale.x
        };
        let times = [0.0, 0.7, 1.4, 2.1, 2.8];
        let a: Vec<f32> = times.iter().map(|t| sample(0xAAAA_1111, *t)).collect();
        let b: Vec<f32> = times.iter().map(|t| sample(0x5555_2222, *t)).collect();
        assert!(
            a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 1e-4),
            "two actors breathe in lockstep: {a:?}"
        );
        assert!(
            a.iter().any(|x| (x - a[0]).abs() > 1e-4),
            "breathing never moves: {a:?}"
        );
    }

    // ------------------------------------------------ §8 carriage mapping

    /// Weariness drops the arm swing toward 0.3× monotonically; rested (w = 0)
    /// is the identity amplitude.
    #[test]
    fn weariness_drops_the_arm_swing() {
        assert_eq!(weary_arm_swing(ARM_SWING_RAD, 0.0), ARM_SWING_RAD);
        assert!((weary_arm_swing(ARM_SWING_RAD, 1.0) - 0.3 * ARM_SWING_RAD).abs() < 1e-6);
        assert!(weary_arm_swing(ARM_SWING_RAD, 0.5) < weary_arm_swing(ARM_SWING_RAD, 0.0));
        assert!(weary_arm_swing(ARM_SWING_RAD, 1.0) < weary_arm_swing(ARM_SWING_RAD, 0.5));
    }

    /// A default `Carriage` adds nothing (§8 "d = 0 → identity"): the torso
    /// offset is exactly zero, and an L0 walk keeps its full arm swing with no
    /// forward stoop — a byte-for-byte sober walk.
    #[test]
    fn carriage_is_identity_when_sober_and_rested() {
        let sober = Carriage::default();
        for &t in &[0.0, 0.5, 1.3, 4.7, 20.0] {
            assert_eq!(carriage_torso(sober, t, 0xABCD), (0.0, 0.0));
        }
        let mut pose = PoseDeltas::default();
        apply_locomotion(&mut pose, 1.0, 0.1, 0.0, sober, 3.0, 7);
        let swing = (0.1 * TAU).sin();
        let expected = Quat::from_rotation_x(ARM_SWING_RAD * swing);
        assert!(pose.right_upper_arm.rotation.angle_between(expected) < 1e-6);
        // Only the walk roll (Z) is present; no stoop (X).
        let (_, _, pitch) = pose.torso.rotation.to_euler(EulerRot::ZYX);
        assert!(pitch.abs() < 1e-4, "sober torso must not stoop: {pitch}");
    }

    /// Drunkenness (d = 0.8) sways the torso: the roll wanders over time yet
    /// stays inside the clamp, the phase wobble is bounded to its two
    /// amplitudes, and the staggered phase actually moves the legs versus sober.
    #[test]
    fn drunkenness_sways_within_the_clamp_and_staggers_the_phase() {
        let drunk = Carriage {
            drunkenness: 0.8,
            weariness: 0.0,
        };
        let mut rolls = Vec::new();
        let mut max_wobble = 0.0_f32;
        for i in 0..400 {
            let t = i as f64 * 0.05;
            let (roll, pitch) = carriage_torso(drunk, t, 0x1357);
            assert!(roll.abs() <= DRUNK_ROLL_MAX_RAD + 1e-6, "roll exceeds clamp: {roll}");
            assert_eq!(pitch, 0.0, "no stoop without weariness");
            rolls.push(roll);
            max_wobble = max_wobble.max((0.8 * drunk_phase_wobble(t, 0x1357)).abs());
        }
        let max = rolls.iter().copied().fold(f32::MIN, f32::max);
        let min = rolls.iter().copied().fold(f32::MAX, f32::min);
        assert!(max > 0.03 && min < -0.03, "drunk torso barely sways: {min}..{max}");
        assert!(
            max_wobble <= 0.8 * (DRUNK_PHASE_NOISE_CYCLES + DRUNK_CADENCE_CYCLES) + 1e-6,
            "phase wobble exceeds its amplitude: {max_wobble}"
        );
        // Across a stretch of time the staggered phase visibly displaces the
        // legs versus a sober walk at the same gait phase (the wobble passes
        // through zero, so it is the peak displacement that must show).
        let mut max_leg_shift = 0.0_f32;
        for i in 0..200 {
            let t = i as f64 * 0.05;
            let mut sober_pose = PoseDeltas::default();
            let mut drunk_pose = PoseDeltas::default();
            apply_locomotion(&mut sober_pose, 1.0, 0.3, 0.0, Carriage::default(), t, 9);
            apply_locomotion(&mut drunk_pose, 1.0, 0.3, 0.0, drunk, t, 9);
            max_leg_shift = max_leg_shift.max(
                sober_pose
                    .left_thigh
                    .rotation
                    .angle_between(drunk_pose.left_thigh.rotation),
            );
        }
        assert!(
            max_leg_shift > 0.05,
            "the drunk phase stagger must move the legs: {max_leg_shift}"
        );
    }

    /// Weariness (w = 1) folds the torso forward (a negative X-rotation, like
    /// the bow) and touches neither the roll nor the arm phase.
    #[test]
    fn weariness_stoops_the_torso_forward() {
        let weary = Carriage {
            drunkenness: 0.0,
            weariness: 1.0,
        };
        let (roll, pitch) = carriage_torso(weary, 2.0, 0x2468);
        assert_eq!(roll, 0.0, "weariness does not sway");
        assert!((pitch + WEARY_STOOP_RAD).abs() < 1e-6, "full weariness folds the torso");
        let mut pose = PoseDeltas::default();
        apply_locomotion(&mut pose, 1.0, 0.0, 0.0, weary, 2.0, 0x2468);
        let (_, _, x) = pose.torso.rotation.to_euler(EulerRot::ZYX);
        assert!(x < -0.1, "the weary torso pitches forward: {x}");
    }

    /// `Carriage::from_statuses` maps the snapshot slice onto the two axes and
    /// ignores anything it has no pose for.
    #[test]
    fn carriage_reads_the_snapshot_statuses() {
        use cathedral_sim::StatusKind;
        assert_eq!(Carriage::from_statuses(&[]), Carriage::default());
        assert_eq!(
            Carriage::from_statuses(&[
                (StatusKind::Drunkenness, 0.7),
                (StatusKind::Weariness, 0.4)
            ]),
            Carriage {
                drunkenness: 0.7,
                weariness: 0.4,
            }
        );
    }

    /// §5/§6: an arm owned by L2 at full weight loses its locomotion swing —
    /// the carry replaces the left arm outright while the right keeps swinging.
    #[test]
    fn carry_owns_the_left_arm_over_the_walk_swing() {
        let mut pose = PoseDeltas::default();
        apply_locomotion(&mut pose, 1.0, 0.25, 0.0, Carriage::default(), 0.0, 0);
        let swinging_right = pose.right_upper_arm;
        apply_carry(&mut pose, 1.0);

        let expected = Quat::from_rotation_x(CARRY_UPPER_PITCH_RAD);
        assert!(pose.left_upper_arm.rotation.angle_between(expected) < 1e-5);
        assert!(
            (pose.left_forearm.rotation.angle_between(Quat::from_rotation_x(
                CARRY_FOREARM_BEND_RAD
            ))) < 1e-5,
            "the basket-carry bends the left elbow"
        );
        assert_eq!(
            pose.right_upper_arm, swinging_right,
            "the free arm keeps its walk swing"
        );
    }

    /// §6: the offer extends the right arm toward the aim — straight ahead at
    /// yaw 0, and yawed toward a recipient standing to the side.
    #[test]
    fn offer_extends_the_right_arm_toward_the_recipient() {
        let mut pose = PoseDeltas::default();
        apply_offer(&mut pose, 1.0, 0.0, 0.0);
        // The hanging arm axis (−Y) must end up mostly forward (−Z).
        let direction = pose.right_upper_arm.rotation * Vec3::NEG_Y;
        assert!(direction.z < -0.8, "arm not extended forward: {direction}");

        // A recipient on the actor's right (+X world, facing −Z) pulls the arm
        // right: `offer_aim` yields a negative yaw, and the extended arm tips
        // toward +X.
        let root = Transform::from_xyz(0.0, 0.91, 0.0);
        let (yaw, pitch) = offer_aim(&root, Vec3::new(3.0, 0.91, -1.0));
        assert!(yaw < -0.5, "rightward recipient must yaw negative: {yaw}");
        assert!(pitch.abs() < 0.1, "level recipient, level pitch: {pitch}");
        let mut aimed = PoseDeltas::default();
        apply_offer(&mut aimed, 1.0, yaw, pitch);
        let aimed_direction = aimed.right_upper_arm.rotation * Vec3::NEG_Y;
        assert!(aimed_direction.x > 0.3, "arm must tip toward the recipient");

        // Aim clamps: a recipient behind the shoulder cannot wrap the arm.
        let (behind, _) = offer_aim(&root, Vec3::new(0.0, 0.91, 5.0));
        assert!(behind.abs() <= OFFER_YAW_CLAMP_RAD + 1e-6);
    }

    /// §7's habit-tier one-shots: the nod dips the face downward, the shake
    /// sweeps it side to side, and the envelope is zero at both ends.
    #[test]
    fn nod_dips_and_shake_sweeps_with_a_closed_envelope() {
        let face_of = |pose: &PoseDeltas| pose.head.rotation * Vec3::NEG_Z;

        let mut nod = PoseDeltas::default();
        apply_one_shot(&mut nod, OneShotGesture::Nod, 0.25, 1.0, 0.0);
        assert!(face_of(&nod).y < -0.15, "a nod looks down");
        let mut nod_late = PoseDeltas::default();
        apply_one_shot(&mut nod_late, OneShotGesture::Nod, 0.75, 1.0, 0.0);
        assert!(face_of(&nod_late).y < -0.15, "the second beat also dips");

        let mut left = PoseDeltas::default();
        apply_one_shot(&mut left, OneShotGesture::ShakeHead, 1.0 / 6.0, 1.0, 0.0);
        let mut right = PoseDeltas::default();
        apply_one_shot(&mut right, OneShotGesture::ShakeHead, 0.5, 1.0, 0.0);
        assert!(
            face_of(&left).x * face_of(&right).x < 0.0,
            "the shake crosses the midline"
        );

        let duration = OneShotGesture::Nod.duration_seconds();
        assert!(one_shot_weight(0.0, duration) < 1e-6);
        assert!(one_shot_weight(duration, duration) < 1e-6);
        assert!(one_shot_weight(duration * 0.5, duration) > 0.99);
    }

    /// M4's deliberate catalog: every sim kind maps to a pose (dance excepted),
    /// the arm kinds move the upper body and leave the legs alone, point aims
    /// by `face_yaw`, and bow folds the torso forward.
    #[test]
    fn the_deliberate_catalog_maps_and_poses_the_upper_body() {
        use cathedral_sim::GestureKind as K;
        for (kind, expected) in [
            (K::Nod, Some(OneShotGesture::Nod)),
            (K::ShakeHead, Some(OneShotGesture::ShakeHead)),
            (K::Wave, Some(OneShotGesture::Wave)),
            (K::Beckon, Some(OneShotGesture::Beckon)),
            (K::Shrug, Some(OneShotGesture::Shrug)),
            (K::Point, Some(OneShotGesture::Point)),
            (K::Bow, Some(OneShotGesture::Bow)),
            (K::Dance, None),
        ] {
            assert_eq!(OneShotGesture::from_kind(kind), expected, "{kind:?}");
        }

        // A wave works the right arm; the legs are untouched (upper-body only,
        // so a walking wave still walks).
        let mut wave = PoseDeltas::default();
        apply_one_shot(&mut wave, OneShotGesture::Wave, 0.5, 1.0, 0.0);
        assert!(wave.right_upper_arm.rotation.angle_between(Quat::IDENTITY) > 0.5);
        assert_eq!(wave.left_thigh.rotation, Quat::IDENTITY);
        assert_eq!(wave.right_thigh.rotation, Quat::IDENTITY);

        // Point aims the right arm: a non-zero face_yaw swings it off centre.
        let mut ahead = PoseDeltas::default();
        apply_one_shot(&mut ahead, OneShotGesture::Point, 0.5, 1.0, 0.0);
        let mut aimed = PoseDeltas::default();
        apply_one_shot(&mut aimed, OneShotGesture::Point, 0.5, 1.0, 0.5);
        assert!(
            ahead
                .right_upper_arm
                .rotation
                .angle_between(aimed.right_upper_arm.rotation)
                > 0.1,
            "point turns toward its target"
        );

        // Bow folds the torso forward (its top tilts toward model-forward, −Z);
        // shrug lifts both shoulders.
        let mut bow = PoseDeltas::default();
        apply_one_shot(&mut bow, OneShotGesture::Bow, 0.5, 1.0, 0.0);
        assert!((bow.torso.rotation * Vec3::Y).z < -0.1, "the torso folds forward");
        let mut shrug = PoseDeltas::default();
        apply_one_shot(&mut shrug, OneShotGesture::Shrug, 0.5, 1.0, 0.0);
        assert!(shrug.left_upper_arm.rotation.angle_between(Quat::IDENTITY) > 0.1);
        assert!(shrug.right_upper_arm.rotation.angle_between(Quat::IDENTITY) > 0.1);
    }

    /// The looping dance is snapshot-driven and upper-body: it sways the torso
    /// and lifts the arms, moves over time, and never touches the legs.
    #[test]
    fn the_dance_sways_the_upper_body_over_time_and_leaves_the_legs() {
        let mut early = PoseDeltas::default();
        apply_dance(&mut early, 1.0, 0.10, 7);
        let mut later = PoseDeltas::default();
        apply_dance(&mut later, 1.0, 0.45, 7);

        assert!(early.left_upper_arm.rotation.angle_between(Quat::IDENTITY) > 0.5);
        assert!(early.torso.rotation.angle_between(Quat::IDENTITY) > 0.05);
        assert_eq!(early.left_thigh.rotation, Quat::IDENTITY);
        assert_eq!(early.right_shin.rotation, Quat::IDENTITY);
        assert!(
            early.torso.rotation.angle_between(later.torso.rotation) > 1e-3,
            "the sway advances with time"
        );

        // The blend flag rides the pose state.
        let rest = rest_pose(&build_scales(cathedral_sim::Build::Male));
        let mut state = BodyPoseState::new(&ActorId("dancer".into()), rest);
        assert!(!state.dance);
        state.set_dance(true);
        assert!(state.dance);
    }

    /// The M2 entry points: activity targets ramp the blends inside the pose
    /// system, a pulse expires on its own, and a finished gesture clears.
    #[test]
    fn hand_activity_and_gestures_ride_the_pose_state() {
        let rest = rest_pose(&build_scales(cathedral_sim::Build::Female));
        let mut state = BodyPoseState::new(&ActorId("someone".into()), rest);
        state.set_hand_activity(true, Some(Vec3::new(0.0, 0.91, -2.0)));
        assert!(state.carry_target);
        assert!(state.offer_target.is_some());

        state.pulse_offer(Vec3::ZERO, 10.0);
        let (_, until) = state.offer_pulse.unwrap();
        assert!((until - (10.0 + STALL_PULSE_SECONDS)).abs() < 1e-9);

        state.start_gesture(OneShotGesture::Nod, None, 5.0);
        assert!(state.gesture.is_some());
    }

    /// M3's talk bookkeeping: the deadline follows the bubble formula
    /// (`speech_text_seconds`), a follow-up line extends it, and pruning
    /// drops expired speakers and stale sounds.
    #[test]
    fn talk_deadlines_follow_the_bubble_formula_and_prune() {
        let mut reflex = ReflexState::default();
        let ilse = ActorId("ilse".into());

        // A short line talks for the 3 s floor.
        reflex.note_speech(ilse.clone(), None, "Hello.", 10.0);
        assert!(reflex.is_talking(&ilse, 12.9));
        assert!(!reflex.is_talking(&ilse, 13.1));

        // A long line caps at 10 s, and re-speaking extends the deadline.
        reflex.note_speech(ilse.clone(), None, &"x".repeat(500), 12.0);
        assert!(reflex.is_talking(&ilse, 21.9));
        assert!(!reflex.is_talking(&ilse, 22.1));

        // Pruning drops the expired speaker and old sounds; the ring caps.
        reflex.note_sound(Vec3::ZERO, 50.0, 12.0);
        reflex.prune(23.0);
        assert!(reflex.talkers.is_empty());
        assert!(reflex.sounds.is_empty(), "an old sound must age out");
        for index in 0..(MAX_RECENT_SOUNDS + 3) {
            reflex.note_sound(Vec3::splat(index as f32), 50.0, 23.0);
        }
        assert_eq!(reflex.sounds.len(), MAX_RECENT_SOUNDS);
    }

    /// M3's gaze priority: active conversation (own partner while talking,
    /// else the nearest live speaker in range) > own offer recipient > a
    /// recent loud sound in earshot > none — and the sound branch only draws
    /// idle actors within the sound's own audible distance.
    #[test]
    fn gaze_priority_runs_conversation_offer_sound() {
        let me = ActorId("me".into());
        let partner = ActorId("partner".into());
        let other = ActorId("other".into());
        let partner_point = Vec3::new(5.0, 1.57, 0.0);
        let other_point = Vec3::new(0.0, 1.57, 8.0);
        let resolve = move |id: &ActorId| -> Option<Vec3> {
            if id.0 == "partner" {
                Some(partner_point)
            } else if id.0 == "other" {
                Some(other_point)
            } else {
                None
            }
        };
        let offer_point = Vec3::new(-3.0, 0.91, 0.0);
        let query = |offer: bool, idle: bool| GazeQuery {
            position: Vec3::new(0.0, 0.91, 0.0),
            offer_at: offer.then_some(offer_point),
            idle,
            seed: 0xBEEF,
        };

        let mut reflex = ReflexState::default();
        reflex.note_sound(Vec3::new(0.0, 2.0, -30.0), 600.0, 0.0);
        // My line runs 3 s (the floor); the town crier's long one runs 10 s.
        reflex.note_speech(me.clone(), Some(partner.clone()), "Hello there!", 0.0);
        reflex.note_speech(other.clone(), None, &"y".repeat(200), 0.0);

        // Talking to a partner beats everything.
        assert_eq!(
            reflex.gaze_point(&me, &query(true, true), 1.0, &resolve),
            Some(partner_point)
        );
        // Once my own line expires I listen to the live speaker nearby …
        assert_eq!(
            reflex.gaze_point(&me, &query(true, true), 3.5, &resolve),
            Some(other_point)
        );
        // … unless every speaker is out of social range: then my own offer.
        let mut far = ReflexState::default();
        far.note_sound(Vec3::new(0.0, 2.0, -30.0), 600.0, 0.0);
        far.note_speech(other.clone(), None, &"y".repeat(200), 0.0);
        let far_resolve = |id: &ActorId| {
            (id.0 == "other").then_some(Vec3::new(0.0, 1.57, LISTEN_RADIUS_M + 5.0))
        };
        let offer_gaze = far.gaze_point(&me, &query(true, true), 1.0, &far_resolve);
        assert!(offer_gaze.is_some());
        assert_eq!(offer_gaze.unwrap().x, offer_point.x);
        // No offer either: the recent sound draws an idle glance …
        assert_eq!(
            far.gaze_point(&me, &query(false, true), 1.0, &far_resolve),
            Some(Vec3::new(0.0, 2.0, -30.0))
        );
        // … but not from a committed walker, not out of the sound's earshot,
        // and not forever.
        assert_eq!(far.gaze_point(&me, &query(false, false), 1.0, &far_resolve), None);
        let mut quiet = ReflexState::default();
        quiet.note_sound(Vec3::new(0.0, 2.0, -30.0), 10.0, 0.0);
        assert_eq!(quiet.gaze_point(&me, &query(false, true), 1.0, &resolve), None);
        assert_eq!(far.gaze_point(&me, &query(false, true), 30.0, &far_resolve), None);
    }

    /// M3's neck limits: yaw clamps at ±70° off the torso facing, pitch stays
    /// in its band, and a target essentially behind the actor drops the gaze
    /// instead of pinning the head at the clamp.
    #[test]
    fn gaze_aim_clamps_yaw_and_gives_up_behind() {
        let root = Transform::from_xyz(0.0, 0.91, 0.0);

        // Dead ahead: no yaw, level pitch.
        let (yaw, pitch) = gaze_aim(&root, Vec3::new(0.0, 0.91 + GAZE_EYE_OFFSET_Y, -4.0)).unwrap();
        assert!(yaw.abs() < 1e-6 && pitch.abs() < 1e-6);

        // Far off to the side: clamped to the ±70° limit, not further.
        let (yaw, _) = gaze_aim(&root, Vec3::new(6.0, 1.5, -0.3)).unwrap();
        assert!((yaw.abs() - GAZE_YAW_CLAMP_RAD).abs() < 1e-5, "yaw {yaw}");

        // High overhead: pitch clamps.
        let (_, pitch) = gaze_aim(&root, Vec3::new(0.0, 12.0, -2.0)).unwrap();
        assert!((pitch - GAZE_PITCH_CLAMP_RAD).abs() < 1e-6);

        // Directly behind: no gaze at all.
        assert_eq!(gaze_aim(&root, Vec3::new(0.0, 1.5, 5.0)), None);

        // The clamp is measured off the torso facing, not the world axes: a
        // rotated root looking at the same world point aims straight ahead.
        let turned = Transform::from_xyz(0.0, 0.91, 0.0)
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2));
        let (yaw, _) = gaze_aim(&turned, Vec3::new(-4.0, 1.57, 0.0)).unwrap();
        assert!(yaw.abs() < 1e-5, "relative-to-torso yaw, got {yaw}");
    }

    /// M3's talk gesticulation: forearms lift off the rest hang while
    /// talking, an arm owned by L2 stays suppressed, the head bobs, and two
    /// different speaker ids gesture with different energy.
    #[test]
    fn talk_gesticulation_lifts_forearms_and_differs_per_speaker() {
        let mut pose = PoseDeltas::default();
        apply_talk(&mut pose, 1.0, 1.0, 1.0, 2.0, 0xA11CE);
        assert!(
            pose.left_forearm.rotation.angle_between(Quat::IDENTITY) > 0.2,
            "the left forearm must lift while talking"
        );
        assert!(
            pose.right_forearm.rotation.angle_between(Quat::IDENTITY) > 0.2,
            "the right forearm must lift while talking"
        );

        // A carried/offered arm is L2's: zero arm weight leaves it alone.
        let mut owned = PoseDeltas::default();
        apply_talk(&mut owned, 1.0, 0.0, 1.0, 2.0, 0xA11CE);
        assert_eq!(owned.left_forearm, JointDelta::default());
        assert!(owned.right_forearm.rotation.angle_between(Quat::IDENTITY) > 0.2);

        // The head bobs at some point of the cycle.
        let bobbed = (0..20).any(|step| {
            let mut pose = PoseDeltas::default();
            apply_talk(&mut pose, 1.0, 1.0, 1.0, step as f64 * 0.1, 0xA11CE);
            pose.head.rotation.angle_between(Quat::IDENTITY) > 0.005
        });
        assert!(bobbed, "the talk head bob never moved the head");

        // Two ids never share an energy curve (the id-seeded noise).
        let differs = (0..8).any(|step| {
            let t = step as f64 * 0.7;
            (talk_energy(t, 0x1111_2222) - talk_energy(t, 0x3333_4444)).abs() > 1e-3
        });
        assert!(differs, "two speakers gesture in lockstep");
    }

    /// M3 end to end: a `PresentSpeech` for an actor turns their talk layer
    /// and player-directed gaze on (the head visibly turns toward the
    /// camera), and both decay after the bubble deadline expires.
    #[test]
    fn speech_turns_the_head_and_talk_layer_on_and_they_decay() {
        use std::time::Duration;

        use bevy::asset::AssetPlugin;
        use bevy::time::TimeUpdateStrategy;

        use crate::smart_actors::actors::reconcile_actor_views;
        use crate::smart_actors::model::{
            ActorControl, ActorSnapshot, Position, WorldMirror, WorldSnapshot,
        };
        use crate::smart_actors::sound::PlaySoundEffect;
        use crate::smart_actors::speech::PresentSpeech;

        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    ActorSnapshot {
                        id: ActorId("talker".into()),
                        name_for_player: "Talker".into(),
                        control: ActorControl::Llm,
                        position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                    },
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(3.0, 0.91, -3.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
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
            .init_asset::<Image>()
            .insert_resource(mirror)
            .insert_resource(MovementInbox::default())
            .init_resource::<ReflexState>()
            .add_message::<PresentSpeech>()
            .add_message::<PlaySoundEffect>()
            // Deterministic clock: 0.2 s per update.
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                200,
            )))
            .add_systems(Startup, setup_body_assets)
            .add_systems(
                Update,
                (reconcile_actor_views, track_reflex_signals, animate_body_pose).chain(),
            );
        // The camera front-right of the talker: the gaze yaw toward it is
        // distinctly nonzero and inside the clamp.
        app.world_mut().spawn((
            PlayerCamera,
            GlobalTransform::from(Transform::from_xyz(3.0, 1.7, -3.0)),
        ));
        app.update();

        // The talker speaks to the player: a 3 s (floor) deadline opens.
        app.world_mut().write_message(PresentSpeech {
            event_seq: 1,
            event_id: "speech-1".into(),
            speaker_id: ActorId("talker".into()),
            speaker_label: "Talker".into(),
            target_id: Some(ActorId("player".into())),
            text: "Hello!".into(),
            speaker_position: Vec3::ZERO,
            recipient_count: 1,
            expect_audio: false,
        });
        for _ in 0..4 {
            app.update();
        }

        let world = app.world_mut();
        let (rig, state) = world
            .query::<(&ActorId, &BodyRig, &BodyPoseState)>()
            .iter(world)
            .find(|(id, ..)| id.0 == "talker")
            .map(|(_, rig, state)| (rig.head, (state.talk_blend, state.gaze_blend)))
            .expect("the talker has a rig");
        assert!(state.0 > 0.9, "talk layer must be on, blend {}", state.0);
        assert!(state.1 > 0.9, "gaze layer must be on, blend {}", state.1);
        let head = world.entity(rig).get::<Transform>().unwrap().rotation;
        let face = head * Vec3::NEG_Z;
        assert!(
            face.x > 0.4,
            "the head must turn toward the camera on +X, faces {face}"
        );

        // Past the deadline both layers ramp back out and the head returns.
        for _ in 0..30 {
            app.update();
        }
        let world = app.world_mut();
        let (rig, state) = world
            .query::<(&ActorId, &BodyRig, &BodyPoseState)>()
            .iter(world)
            .find(|(id, ..)| id.0 == "talker")
            .map(|(_, rig, state)| (rig.head, (state.talk_blend, state.gaze_blend)))
            .unwrap();
        assert!(state.0 < 1e-3, "talk layer must decay, blend {}", state.0);
        assert!(state.1 < 1e-3, "gaze layer must decay, blend {}", state.1);
        let head = world.entity(rig).get::<Transform>().unwrap().rotation;
        let face = head * Vec3::NEG_Z;
        assert!(
            face.x.abs() < 0.35,
            "the head must hand control back after the deadline, faces {face}"
        );
    }

    /// M4 end to end: a snapshot carrying `active_gesture: dance` drives the
    /// looping dance on the body (blend ramps up, stops when the snapshot
    /// clears it), and a `PresentGesture` trigger starts a one-shot pose.
    #[test]
    fn a_dance_snapshot_drives_the_loop_and_a_trigger_starts_a_one_shot() {
        use std::time::Duration;

        use bevy::asset::AssetPlugin;
        use bevy::time::TimeUpdateStrategy;

        use crate::smart_actors::actors::reconcile_actor_views;
        use crate::smart_actors::model::{
            ActorControl, ActorSnapshot, Position, WorldMirror, WorldSnapshot,
        };

        let snapshot = |dance: Option<cathedral_sim::GestureKind>| WorldSnapshot {
            world_revision: 1,
            player_id: ActorId("player".into()),
            actors: vec![
                ActorSnapshot {
                    id: ActorId("dancer".into()),
                    name_for_player: "Dancer".into(),
                    control: ActorControl::Llm,
                    position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
                    facing_yaw: 0.0,
                    appearance: Default::default(),
                    holds: vec![],
                    active_gesture: dance,
                    statuses: Vec::new(),
                },
                ActorSnapshot {
                    id: ActorId("player".into()),
                    name_for_player: "You".into(),
                    control: ActorControl::Player,
                    position_m: Position::new(1.0, 0.91, -1.0).unwrap(),
                    facing_yaw: 0.0,
                    appearance: Default::default(),
                    holds: vec![],
                    active_gesture: None,
                    statuses: Vec::new(),
                },
            ],
            items: vec![],
            offers: vec![],
        };

        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(snapshot(Some(cathedral_sim::GestureKind::Dance)))
            .unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(mirror)
            .insert_resource(MovementInbox::default())
            .init_resource::<ReflexState>()
            .add_message::<PresentGesture>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)))
            .add_systems(Startup, setup_body_assets)
            .add_systems(
                Update,
                (reconcile_actor_views, drive_gesture_pose, animate_body_pose).chain(),
            );
        app.world_mut().spawn((
            PlayerCamera,
            GlobalTransform::from(Transform::from_xyz(1.0, 1.7, -1.0)),
        ));

        let dancer_state = |app: &mut App| -> (bool, f32, bool) {
            let world = app.world_mut();
            world
                .query::<(&ActorId, &BodyPoseState)>()
                .iter(world)
                .find(|(id, _)| id.0 == "dancer")
                .map(|(_, state)| (state.dance, state.dance_blend, state.gesture.is_some()))
                .expect("the dancer has a pose state")
        };

        // The snapshot's dance drives the loop; its blend ramps up.
        for _ in 0..8 {
            app.update();
        }
        let (dancing, blend, _) = dancer_state(&mut app);
        assert!(dancing, "the snapshot's active_gesture drives the loop");
        assert!(blend > 0.5, "the dance blend ramped up, {blend}");

        // Clearing active_gesture stops it — the blend ramps back down.
        app.world_mut()
            .resource_mut::<WorldMirror>()
            .replace_snapshot(snapshot(None))
            .unwrap();
        for _ in 0..8 {
            app.update();
        }
        let (dancing, blend, _) = dancer_state(&mut app);
        assert!(!dancing, "clearing the snapshot stops the dance");
        assert!(blend < 1e-2, "the dance blend ramped down, {blend}");

        // A one-shot trigger starts a pose on the same body.
        app.world_mut().write_message(PresentGesture {
            actor_id: ActorId("dancer".into()),
            kind: cathedral_sim::GestureKind::Wave,
            target_id: Some(ActorId("player".into())),
        });
        app.update();
        let (_, _, gesturing) = dancer_state(&mut app);
        assert!(gesturing, "the wave trigger started a one-shot");
    }

    /// §9's Tier A cut keeps only the nearest cap's worth of actors.
    #[test]
    fn tier_a_cut_caps_at_the_nearest_sixty_four() {
        let mut few: Vec<(f32, u32)> = (0..10).map(|i| (i as f32, i)).collect();
        assert_eq!(tier_a_cut(&mut few).len(), 10);

        let mut crowd: Vec<(f32, u32)> = (0..200).rev().map(|i| (i as f32, i)).collect();
        let cut = tier_a_cut(&mut crowd);
        assert_eq!(cut.len(), TIER_A_CAP);
        for nearest in 0..TIER_A_CAP as u32 {
            assert!(cut.contains(&nearest), "nearest actor {nearest} dropped");
        }
        assert!(!cut.contains(&(TIER_A_CAP as u32)));
    }

    /// End-to-end wiring: a walking actor's thighs actually swing, the idle
    /// figure's don't, and the pose system never touches the ROOT transform —
    /// that stays the snapshot's (reconcile + drive own it).
    #[test]
    fn pose_system_animates_walkers_and_leaves_the_root_alone() {
        use bevy::asset::AssetPlugin;

        use crate::smart_actors::actors::reconcile_actor_views;
        use crate::smart_actors::model::{
            ActorControl, ActorSnapshot, MotionSample, Position, WorldMirror, WorldSnapshot,
        };

        let actor = |id: &str, x: f32| ActorSnapshot {
            id: ActorId(id.into()),
            name_for_player: id.into(),
            control: ActorControl::Llm,
            position_m: Position::new(x, 0.91, 0.0).unwrap(),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: vec![],
            active_gesture: None,
            statuses: Vec::new(),
        };
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    actor("walker", 0.0),
                    actor("idler", 2.0),
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.91, 10.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
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
            .init_asset::<Image>()
            .insert_resource(mirror)
            .insert_resource(MovementInbox::default())
            .init_resource::<ReflexState>()
            .add_systems(Startup, setup_body_assets)
            .add_systems(Update, (reconcile_actor_views, animate_body_pose).chain());
        app.world_mut().spawn((
            PlayerCamera,
            GlobalTransform::from(Transform::from_xyz(0.0, 1.7, 10.0)),
        ));
        app.update();

        let world = app.world_mut();
        let walker_root = world
            .query::<(Entity, &ActorId, &BodyRig)>()
            .iter(world)
            .find(|(_, id, _)| id.0 == "walker")
            .map(|(entity, _, _)| entity)
            .expect("walker spawned");
        // A fresh mid-stride sample at peak swing; walk_blend is preset so the
        // 0.25 s idle→walk ramp doesn't need to be simulated in wall time.
        world
            .resource_mut::<MovementInbox>()
            .0
            .insert(ActorId("walker".into()), MotionSample {
                position: Vec3::new(0.0, 0.91, 0.0),
                facing_yaw: 0.0,
                speed: 1.8,
                gait_phase: 0.25,
                seq: 1,
            });
        world
            .entity_mut(walker_root)
            .get_mut::<BodyPoseState>()
            .unwrap()
            .walk_blend = 1.0;
        app.update();

        let world = app.world_mut();
        let mut rigs = world.query::<(&ActorId, &BodyRig, &BodyPoseState, &Transform)>();
        let thigh_forwardness = |world: &mut World, actor: &str| {
            let mut rigs = world.query::<(&ActorId, &BodyRig)>();
            let thigh = rigs
                .iter(world)
                .find(|(id, _)| id.0 == actor)
                .map(|(_, rig)| rig.left_thigh)
                .unwrap();
            let rotation = world.entity(thigh).get::<Transform>().unwrap().rotation;
            (rotation * Vec3::NEG_Y).z
        };
        assert!(
            thigh_forwardness(world, "walker") < -0.3,
            "the walker's thigh must swing forward at phase 0.25"
        );
        assert!(
            thigh_forwardness(world, "idler").abs() < 0.05,
            "the idler's legs stay at rest"
        );
        for (id, _, state, transform) in rigs.iter(world) {
            assert_eq!(state.tier, PoseTier::A, "{} should be on stage", id.0);
            // The pose never moves the ROOT: still exactly the snapshot pose.
            let expected_x = if id.0 == "walker" { 0.0 } else { 2.0 };
            assert_eq!(transform.translation, Vec3::new(expected_x, 0.91, 0.0));
            assert_eq!(transform.scale, Vec3::ONE);
        }

        // Walking far out of range settles the puppet back to rest (Tier C
        // writes once on transition, then never again).
        let camera = world
            .query_filtered::<Entity, With<PlayerCamera>>()
            .single(world)
            .unwrap();
        world
            .entity_mut(camera)
            .insert(GlobalTransform::from(Transform::from_xyz(0.0, 1.7, 500.0)));
        app.update();
        let world = app.world_mut();
        assert!(
            thigh_forwardness(world, "walker").abs() < 0.05,
            "leaving the animated tiers must settle the legs to rest"
        );
    }

    /// §9 frame-budget measurement at the Tier A cap — the live world never
    /// gathers 64 bodies in 40 m, so the cap case is built synthetically:
    /// 514 walking actors, 100 of them inside the Tier A radius. Run manually:
    ///
    /// ```sh
    /// cargo test --release pose_cost_at_tier_a_cap -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "manual budget measurement; run in --release"]
    fn pose_cost_at_tier_a_cap() {
        use bevy::asset::AssetPlugin;
        use bevy::ecs::system::SystemId;

        use crate::smart_actors::actors::reconcile_actor_views;
        use crate::smart_actors::model::{
            ActorControl, ActorSnapshot, MotionSample, Position, WorldMirror, WorldSnapshot,
        };

        let mut actors: Vec<ActorSnapshot> = (0..514)
            .map(|index| {
                // 100 inside the 40 m Tier A radius, the rest ringed outward
                // through Tier B into the fade.
                let angle = index as f32 * 0.7;
                let radius = if index < 100 {
                    5.0 + (index as f32 % 30.0)
                } else {
                    45.0 + (index - 100) as f32 * 1.4
                };
                ActorSnapshot {
                    id: ActorId(format!("actor-{index:03}")),
                    name_for_player: format!("actor-{index:03}"),
                    control: ActorControl::Llm,
                    position_m: Position::new(
                        radius * angle.cos(),
                        0.91,
                        radius * angle.sin(),
                    )
                    .unwrap(),
                    facing_yaw: 0.0,
                    appearance: Default::default(),
                    holds: vec![],
                    active_gesture: None,
                    statuses: Vec::new(),
                }
            })
            .collect();
        actors.push(ActorSnapshot {
            id: ActorId("player".into()),
            name_for_player: "You".into(),
            control: ActorControl::Player,
            position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: vec![],
            active_gesture: None,
            statuses: Vec::new(),
        });
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors,
                items: vec![],
                offers: vec![],
            })
            .unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(mirror)
            .insert_resource(MovementInbox::default())
            .init_resource::<ReflexState>()
            .add_systems(Startup, setup_body_assets)
            .add_systems(Update, reconcile_actor_views);
        app.world_mut().spawn((
            PlayerCamera,
            GlobalTransform::from(Transform::from_xyz(0.0, 1.7, 0.0)),
        ));
        app.update();

        // Everyone walks, phases spread, blends warm.
        let world = app.world_mut();
        let ids: Vec<ActorId> = world
            .query::<&ActorId>()
            .iter(world)
            .cloned()
            .collect();
        {
            let mut inbox = world.resource_mut::<MovementInbox>();
            for (index, id) in ids.iter().enumerate() {
                inbox.0.insert(id.clone(), MotionSample {
                    position: Vec3::ZERO,
                    facing_yaw: 0.1,
                    speed: 1.8,
                    gait_phase: index as f32 * 0.173,
                    seq: 1,
                });
            }
        }
        let mut states = world.query::<&mut BodyPoseState>();
        for mut state in states.iter_mut(world) {
            state.walk_blend = 1.0;
        }

        let system: SystemId = world.register_system(animate_body_pose);
        for _ in 0..10 {
            world.run_system(system).unwrap();
        }
        let runs: u64 = 200;
        let started = Instant::now();
        for run in 0..runs {
            // Fresh samples so the history-shift path is exercised too.
            let mut inbox = world.resource_mut::<MovementInbox>();
            for sample in inbox.0.values_mut() {
                sample.seq = 2 + run;
                sample.gait_phase += 0.06;
            }
            world.run_system(system).unwrap();
        }
        let per_run_us = started.elapsed().as_secs_f64() * 1e6 / runs as f64;
        eprintln!(
            "[pose budget] {per_run_us:.0} us per frame at the Tier A cap \
             (514 walking actors, 100 Tier A candidates capped to {TIER_A_CAP})"
        );
    }

    /// The whole shared set stays bounded: building the assets registers a
    /// fixed handful of meshes and materials, not one per actor.
    #[test]
    fn body_assets_are_a_bounded_shared_set() {
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .add_systems(Startup, setup_body_assets);
        app.update();

        assert!(app.world().get_resource::<BodyAssets>().is_some());
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        // 7×4 outfit band + 3 bespoke + 24 faces + 4 headgear = 59.
        assert_eq!(materials.len(), 59);
        let meshes = app.world().resource::<Assets<Mesh>>();
        assert_eq!(meshes.len(), 11);

        // The bespoke majors and the band are all distinct shared handles.
        let assets = app.world().resource::<BodyAssets>();
        let sven = AppearanceSnapshot {
            bespoke: Some("sven".into()),
            ..Default::default()
        };
        assert_eq!(
            assets.outfit_material(&sven),
            assets.outfit_material(&sven)
        );
        assert_ne!(
            assets.outfit_material(&sven),
            assets.outfit_material(&AppearanceSnapshot::default())
        );
    }
}
