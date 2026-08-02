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
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
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
// (`WALK_Y` = 0.91 above ground), so ground is [`GROUND_Y`] = −0.91 and the
// head top ends at ≈ +0.80 (≈ 1.71 m tall) — the silhouette height the
// retired capsule had, so doors, bridges and streets keep their scale.
//
// The skeleton is laid out from human proportions for a 1.71 m figure rather
// than by eye: hip joint at 0.53 of stature, shoulder joint 0.82, elbow 0.63,
// wrist 0.48, knee 0.285, ankle 0.045. Each constant below is that height
// minus the 0.91 m root, which is why almost none of them are round.
// ---------------------------------------------------------------------------

/// Ground level, root-local. The soles are authored to land exactly here, so
/// a standing puppet never floats or sinks.
const GROUND_Y: f32 = -0.91;

/// Pelvis: authored root-local (its joint transform is the identity), from the
/// crotch up to the waist, where the torso takes over.
const PELVIS_CROTCH_Y: f32 = -0.125;
const PELVIS_WAIST_Y: f32 = 0.125;

/// Waist joint, in the pelvis: where the torso pivots. The iliac crest sits at
/// 0.60 of stature, i.e. 0.12 above a root that is itself at the hip joint.
const TORSO_JOINT_Y: f32 = 0.12;
/// Waist → collar rim, torso-local. Stops below the chin on purpose: a torso
/// that runs up to the jaw is what made the old puppet read as a sack with a
/// head balanced on it.
const TORSO_HEIGHT: f32 = 0.435;
/// Shoulder (glenohumeral) joint, torso-local. Its X is the half-span between
/// the two arm sockets, not the outer shoulder width — the deltoid swells
/// past it to the ≈0.45 m shoulder breadth of a 1.71 m figure.
const SHOULDER_X: f32 = 0.160;
const SHOULDER_Y: f32 = 0.345;
/// Neck pivot (torso-local): the head nods around this, not its centre.
const NECK_JOINT_Y: f32 = 0.445;
/// Head centre above the neck pivot (baked into the head mesh).
const HEAD_CENTER_ABOVE_NECK: f32 = 0.119;
/// The head ovoid's three half-axes: narrow across, deeper front-to-back,
/// tallest of all. (A sphere reads as a ball; these are a skull.) Sized from
/// head breadth 0.152 m / length 0.195 m / height 0.224 m at this stature.
const HEAD_HALF_WIDTH: f32 = 0.077;
const HEAD_HALF_DEPTH: f32 = 0.096;
const HEAD_HALF_HEIGHT: f32 = 0.112;

const UPPER_ARM_LENGTH: f32 = 0.30;
/// Elbow, upper-arm-local.
const ELBOW_Y: f32 = -UPPER_ARM_LENGTH;
const FOREARM_LENGTH: f32 = 0.26;
/// Wrist, forearm-local: where the hand mesh is seated.
const WRIST_Y: f32 = -FOREARM_LENGTH;
/// The prop grip, forearm-local — a little past the wrist so a carried loaf
/// sits in the fingers rather than on the cuff.
const HAND_ANCHOR_Y: f32 = -0.30;

const HIP_X: f32 = 0.093;
const HIP_Y: f32 = -0.02;
const THIGH_LENGTH: f32 = 0.42;
/// Knee, thigh-local.
const KNEE_Y: f32 = -THIGH_LENGTH;
const SHIN_LENGTH: f32 = 0.39;
/// Ankle, shin-local: where the foot is seated. Sole to ground:
/// −0.02 − 0.42 − 0.39 = −0.83, and the boot is 0.08 deep.
const ANKLE_Y: f32 = -SHIN_LENGTH;
const FOOT_HEIGHT: f32 = -(GROUND_Y - (HIP_Y + KNEE_Y + ANKLE_Y));

/// Root-local height of the shoulder joint (the waist joint plus the
/// torso-local socket). Read by `hands.rs`, which aims a custody grip just
/// below it — the upper arm is a body landmark, not a number the hands may
/// invent.
pub(super) const SHOULDER_ROOT_Y: f32 = TORSO_JOINT_Y + SHOULDER_Y;

/// Neutral standing pose: arms hang with a slight outward tilt and elbow bend,
/// thighs splay a touch so the feet stand a little apart.
const ARM_HANG_TILT_RAD: f32 = 0.10;
const ELBOW_BEND_RAD: f32 = 0.16;
const THIGH_SPLAY_RAD: f32 = 0.025;

/// Physical metres one outfit-cloth texture tile covers. Part UVs are scaled
/// by their physical size over this so the weave reads consistently at body
/// scale across torso, limbs and pelvis. The artwork is a macro photograph of
/// cloth, so the span has to be small enough that a thread reads as a thread
/// and not as sacking rope.
const CLOTH_TILE_M: f32 = 0.35;

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

/// Hose are dyed separately from the tunic they are worn under, so the legs
/// get their own small tint band over the same class weave. This is the single
/// cheapest thing that stops a crowd reading as people in one-piece sacks: a
/// tunic over contrasting hose, cinched at the belt, above dark shoes *is* the
/// medieval silhouette, and it is legible from across a square.
const HOSE_TINT_BAND: usize = 4;
const HOSE_TINTS: [[f32; 3]; HOSE_TINT_BAND] = [
    [0.52, 0.46, 0.39], // shadowed wool
    [0.41, 0.44, 0.51], // slate
    [0.60, 0.48, 0.32], // tan
    [0.38, 0.41, 0.36], // moss
];

/// Skin tone of each painted face, sampled from the portrait's forehead,
/// cheeks and chin by `scripts/generate_npc_surface_maps.py`. The neck and
/// hands are untextured geometry, so they take their colour from here and
/// match whichever face the actor drew — a mismatched hand is the single most
/// obvious tell that a body is assembled from parts.
const FACE_SKIN_TONES: [[f32; 3]; FACE_COUNT] = [
    [0.567, 0.451, 0.312],
    [0.524, 0.417, 0.308],
    [0.684, 0.558, 0.402],
    [0.645, 0.508, 0.344],
    [0.539, 0.414, 0.284],
    [0.661, 0.513, 0.347],
    [0.652, 0.535, 0.392],
    [0.727, 0.580, 0.427],
    [0.644, 0.518, 0.364],
    [0.621, 0.492, 0.358],
    [0.663, 0.512, 0.373],
    [0.691, 0.543, 0.369],
    [0.664, 0.490, 0.308],
    [0.656, 0.524, 0.331],
    [0.707, 0.539, 0.372],
    [0.667, 0.534, 0.375],
    [0.645, 0.522, 0.369],
    [0.547, 0.433, 0.306],
    [0.633, 0.450, 0.320],
    [0.590, 0.482, 0.353],
    [0.683, 0.568, 0.413],
    [0.655, 0.492, 0.319],
    [0.588, 0.471, 0.338],
    [0.425, 0.337, 0.229],
];

/// Hair colours, in the engraving's muted register. Uncovered heads used to
/// read bald from behind (the clamped face texture paints the whole rear cap
/// in flat skin); a hair shell fixes that and gives the crowd another cheap
/// axis of variety.
const HAIR_COLORS: [[f32; 3]; 6] = [
    [0.062, 0.052, 0.046], // black
    [0.118, 0.084, 0.058], // dark brown
    [0.198, 0.136, 0.084], // brown
    [0.268, 0.160, 0.086], // auburn
    [0.352, 0.288, 0.216], // ash blond
    [0.402, 0.388, 0.362], // grey
];

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

/// Marks a cloth part that wears the separately-dyed hose rather than the
/// tunic. It still carries [`ActorOutfit`] — the hose is derived from the
/// appearance, so an appearance swap must repaint it — and this only tells
/// reconcile which of the two materials to resolve.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HosePart;

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
    neck_mesh: Handle<Mesh>,
    head_mesh: Handle<Mesh>,
    hair_mesh: Handle<Mesh>,
    upper_arm_mesh: Handle<Mesh>,
    forearm_mesh: Handle<Mesh>,
    left_hand_mesh: Handle<Mesh>,
    right_hand_mesh: Handle<Mesh>,
    thigh_mesh: Handle<Mesh>,
    shin_mesh: Handle<Mesh>,
    foot_mesh: Handle<Mesh>,
    belt_mesh: Handle<Mesh>,
    mantle_mesh: Handle<Mesh>,
    /// One per [`GarmentCut`]: robe, tunic, short tunic.
    skirt_meshes: [Handle<Mesh>; 3],
    hood_mesh: Handle<Mesh>,
    coif_mesh: Handle<Mesh>,
    brim_mesh: Handle<Mesh>,
    kettle_helm_mesh: Handle<Mesh>,
    /// `[class][tint]` — 7 textured cloth bands × the quantized tint band.
    outfits: [[Handle<StandardMaterial>; OUTFIT_TINT_BAND]; 7],
    /// The same weaves dyed for the legs, on their own tint band.
    hose: [[Handle<StandardMaterial>; HOSE_TINT_BAND]; 7],
    /// The named majors' legacy colors as tints over their class texture.
    sven_outfit: Handle<StandardMaterial>,
    conny_outfit: Handle<StandardMaterial>,
    ilse_outfit: Handle<StandardMaterial>,
    faces: Vec<Handle<StandardMaterial>>,
    /// One flat skin material per face, tinted to that portrait's tone, for
    /// the neck and hands.
    skins: Vec<Handle<StandardMaterial>>,
    hair: Vec<Handle<StandardMaterial>>,
    leather_material: Handle<StandardMaterial>,
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

    /// The bare-skin material matching this appearance's face — neck, hands.
    fn skin_material(&self, appearance: &AppearanceSnapshot) -> Handle<StandardMaterial> {
        self.skins[face_index(appearance.palette_seed)].clone()
    }

    /// The hose material for this appearance's legs.
    pub(crate) fn hose_material(
        &self,
        appearance: &AppearanceSnapshot,
    ) -> Handle<StandardMaterial> {
        self.hose[class_index(appearance.outfit)][hose_index(appearance.palette_seed)].clone()
    }

    /// Mesh + material for this appearance's hair, or `None` when the
    /// headgear covers the scalp anyway (a hood, a coif, a helm) — the shell
    /// would only ever be geometry inside a hat.
    fn hair_visual(
        &self,
        appearance: &AppearanceSnapshot,
    ) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
        match appearance.headgear {
            Headgear::None | Headgear::Brim => Some((
                self.hair_mesh.clone(),
                self.hair[hair_index(appearance.palette_seed)].clone(),
            )),
            Headgear::Hood | Headgear::Coif | Headgear::KettleHelm => None,
        }
    }

    /// The skirt mesh for this appearance's garment cut.
    fn skirt_mesh(&self, appearance: &AppearanceSnapshot) -> Handle<Mesh> {
        self.skirt_meshes[garment_index(appearance.outfit)].clone()
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

/// Deterministic hair pick, on a third rehash so hair, face and tint vary
/// independently across the crowd.
fn hair_index(palette_seed: u32) -> usize {
    (palette_seed.wrapping_mul(0x85EB_CA6B) >> 13) as usize % HAIR_COLORS.len()
}

/// Deterministic hose pick, on a fourth rehash — a tunic tint and a hose dye
/// have nothing to do with each other, and correlating them would put the
/// whole crowd in matching suits.
fn hose_index(palette_seed: u32) -> usize {
    (palette_seed.wrapping_mul(0xC2B2_AE35) >> 19) as usize % HOSE_TINT_BAND
}

/// Which classes work with their sleeves pushed up. Bare forearms are a class
/// cue at conversational range that costs nothing — the skin material is
/// already loaded for the hands.
fn bares_forearms(class: OutfitClass) -> bool {
    matches!(class, OutfitClass::Laborer | OutfitClass::Craftsman)
}

/// Which garment cut an outfit class wears. The robe is the vestment/rank
/// silhouette, the short tunic is cut to work in, and everything between is
/// the city's ordinary tunic — occupation legible at 30 m from the outline
/// alone (§1's readability target), before palette or headgear.
fn garment_index(class: OutfitClass) -> usize {
    match class {
        OutfitClass::Cleric | OutfitClass::Notable => 0,
        OutfitClass::Merchant | OutfitClass::Craftsman | OutfitClass::Watch | OutfitClass::Poor => 1,
        OutfitClass::Laborer => 2,
    }
}

/// The three cuts, indexed by [`garment_index`].
const GARMENT_CUTS: [GarmentCut; 3] = [ROBE, TUNIC, SHORT_TUNIC];

/// A mantle over the shoulders marks the two classes that wear rank.
fn wears_mantle(class: OutfitClass) -> bool {
    matches!(class, OutfitClass::Cleric | OutfitClass::Notable)
}

/// Builds the bounded mesh/material set shared by all actor puppets.
pub(crate) fn setup_body_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Cloth carries a normal map derived from its own weave
    // (`scripts/generate_npc_surface_maps.py`): sunlight then rakes across the
    // fabric instead of sliding over a flat decal, which is most of what
    // separates a clothed body from a painted box at conversational range.
    let cloth = |texture: Handle<Image>, normal: Handle<Image>, tint: Color| StandardMaterial {
        base_color: tint,
        base_color_texture: Some(texture),
        normal_map_texture: Some(normal),
        perceptual_roughness: 0.88,
        reflectance: 0.24,
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
    const OUTFIT_NORMALS: [&str; 7] = [
        "textures/npc/outfit_cleric_normal.png",
        "textures/npc/outfit_merchant_normal.png",
        "textures/npc/outfit_craftsman_normal.png",
        "textures/npc/outfit_laborer_normal.png",
        "textures/npc/outfit_watch_normal.png",
        "textures/npc/outfit_notable_normal.png",
        "textures/npc/outfit_poor_normal.png",
    ];
    let textures: Vec<Handle<Image>> = OUTFIT_TEXTURES
        .iter()
        .map(|path| load_repeating_texture(&asset_server, path))
        .collect();
    // Normal maps must not be colour-managed — they carry vectors, not sRGB.
    let normals: Vec<Handle<Image>> = OUTFIT_NORMALS
        .iter()
        .map(|path| load_linear_repeating_texture(&asset_server, path))
        .collect();
    let outfits = std::array::from_fn(|class| {
        std::array::from_fn(|tint| {
            materials.add(cloth(
                textures[class].clone(),
                normals[class].clone(),
                tint_band[tint],
            ))
        })
    });
    let hose = std::array::from_fn(|class| {
        std::array::from_fn(|tint| {
            let [r, g, b] = HOSE_TINTS[tint];
            materials.add(cloth(
                textures[class].clone(),
                normals[class].clone(),
                Color::srgb(r, g, b),
            ))
        })
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
    let bespoke = |class: OutfitClass, color: Color| {
        let index = class_index(class);
        cloth(
            textures[index].clone(),
            normals[index].clone(),
            lift(color),
        )
    };
    let sven_outfit =
        materials.add(bespoke(OutfitClass::Craftsman, Color::srgb(0.19, 0.28, 0.36)));
    let conny_outfit =
        materials.add(bespoke(OutfitClass::Merchant, Color::srgb(0.16, 0.42, 0.49)));
    let ilse_outfit = materials.add(bespoke(OutfitClass::Cleric, Color::srgb(0.50, 0.24, 0.18)));

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

    // Bare skin for the neck and hands, one tone per face so they always
    // match the portrait the actor drew. Darkened a little against the
    // sampled tone: the portrait is a *painted* surface carrying its own
    // shading, while these are flat albedo taking the sun full on, so
    // matching the numbers exactly makes a neck glow next to its own face.
    const SKIN_SHADE: f32 = 0.84;
    let skins = FACE_SKIN_TONES
        .iter()
        .map(|[r, g, b]| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(r * SKIN_SHADE, g * SKIN_SHADE, b * SKIN_SHADE),
                perceptual_roughness: 0.70,
                reflectance: 0.22,
                ..default()
            })
        })
        .collect();
    let hair = HAIR_COLORS
        .iter()
        .map(|[r, g, b]| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(*r, *g, *b),
                perceptual_roughness: 0.80,
                reflectance: 0.20,
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
        neck_mesh: meshes.add(neck_mesh()),
        head_mesh: meshes.add(head_mesh()),
        hair_mesh: meshes.add(hair_mesh()),
        upper_arm_mesh: meshes.add(upper_arm_mesh()),
        forearm_mesh: meshes.add(forearm_mesh()),
        left_hand_mesh: meshes.add(hand_mesh(BodySide::Left)),
        right_hand_mesh: meshes.add(hand_mesh(BodySide::Right)),
        thigh_mesh: meshes.add(thigh_mesh()),
        shin_mesh: meshes.add(shin_mesh()),
        foot_mesh: meshes.add(foot_mesh()),
        belt_mesh: meshes.add(belt_mesh()),
        mantle_mesh: meshes.add(mantle_mesh()),
        skirt_meshes: GARMENT_CUTS.map(|cut| meshes.add(skirt_mesh(cut))),
        hood_mesh: meshes.add(hood_mesh()),
        coif_mesh: meshes.add(coif_mesh()),
        brim_mesh: meshes.add(brim_mesh()),
        kettle_helm_mesh: meshes.add(kettle_helm_mesh()),
        outfits,
        hose,
        sven_outfit,
        conny_outfit,
        ilse_outfit,
        faces,
        skins,
        hair,
        leather_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.20, 0.145, 0.100),
            perceptual_roughness: 0.62,
            reflectance: 0.34,
            ..default()
        }),
        hood_material: materials.add(shell(Color::srgb(0.30, 0.27, 0.24), 0.92, 0.0)),
        coif_material: materials.add(shell(Color::srgb(0.78, 0.74, 0.66), 0.90, 0.0)),
        felt_material: materials.add(shell(Color::srgb(0.16, 0.15, 0.14), 0.85, 0.0)),
        iron_material: materials.add(shell(Color::srgb(0.33, 0.335, 0.345), 0.52, 0.32)),
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

/// Like [`load_repeating_texture`], but without sRGB decoding: a normal map's
/// channels are a vector, and colour-managing them tilts every lighting
/// calculation on the cloth.
fn load_linear_repeating_texture(
    asset_server: &AssetServer,
    path: &'static str,
) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.is_srgb = false;
            let mut sampler = ImageSamplerDescriptor::linear();
            sampler
                .set_address_mode(ImageAddressMode::Repeat)
                .set_anisotropic_filter(8);
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
        head: Transform::from_xyz(0.0, NECK_JOINT_Y, 0.0),
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
/// below. Every mesh part carries the cloned `VisibilityRange` fade, and the
/// parts whose colour is derived from the appearance carry [`ActorOutfit`] so
/// the reconcile hot-swap reaches them — the legs additionally [`HosePart`],
/// which sends reconcile to the hose material instead. Skin, hair and leather
/// carry neither: a hot-swap must not repaint a hand.
///
/// Everything *structural* is spawn-time — headgear, hair, the garment cut, the
/// mantle, whether the forearms are sleeved — because an appearance never
/// restructures after creation today (same rule the M0 rig shipped under).
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
    let skin = assets.skin_material(appearance);

    let hose = assets.hose_material(appearance);
    let dressed_part = |name: &'static str,
                        mesh: &Handle<Mesh>,
                        material: &Handle<StandardMaterial>,
                        transform: Transform| {
        (
            Name::new(name),
            ActorOutfit(actor_id.clone()),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            transform,
            fade.clone(),
        )
    };
    let cloth_part = |name: &'static str, mesh: &Handle<Mesh>, transform: Transform| {
        dressed_part(name, mesh, &outfit, transform)
    };
    // Legs wear the separately-dyed hose, and reconcile has to know that: a
    // `HosePart` still carries `ActorOutfit` (it is outfit-derived, so an
    // appearance swap must repaint it) but resolves to a different material.
    let hose_part = |name: &'static str, mesh: &Handle<Mesh>, transform: Transform| {
        (dressed_part(name, mesh, &hose, transform), HosePart)
    };
    // Skin, leather and hair are not outfit-tinted, so they carry no
    // `ActorOutfit` — an appearance hot-swap must not repaint a hand.
    let fixed_part = |name: &'static str,
                      mesh: &Handle<Mesh>,
                      material: &Handle<StandardMaterial>,
                      transform: Transform| {
        (
            Name::new(name),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
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
    // The garment hangs off the hips, not the ribs, so it rides the pelvis.
    commands.spawn((
        cloth_part(
            "Body skirt",
            &assets.skirt_mesh(appearance),
            Transform::IDENTITY,
        ),
        ChildOf(pelvis),
    ));
    commands.spawn((
        fixed_part(
            "Body belt",
            &assets.belt_mesh,
            &assets.leather_material,
            Transform::IDENTITY,
        ),
        ChildOf(pelvis),
    ));
    let torso = commands
        .spawn((
            cloth_part("Body torso", &assets.torso_mesh, rest.torso),
            ChildOf(pelvis),
        ))
        .id();
    if wears_mantle(appearance.outfit) {
        commands.spawn((
            cloth_part("Body mantle", &assets.mantle_mesh, Transform::IDENTITY),
            ChildOf(torso),
        ));
    }
    commands.spawn((
        fixed_part("Body neck", &assets.neck_mesh, &skin, Transform::IDENTITY),
        ChildOf(torso),
    ));
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
    if let Some((mesh, material)) = assets.hair_visual(appearance) {
        commands.spawn((
            fixed_part("Body hair", &mesh, &material, Transform::IDENTITY),
            ChildOf(head),
        ));
    }
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
        // Sleeves pushed up for the trades: the forearm is skin, not cloth.
        let forearm = if bares_forearms(appearance.outfit) {
            commands
                .spawn((
                    fixed_part(fore_name, &assets.forearm_mesh, &skin, fore_rest),
                    ChildOf(upper),
                ))
                .id()
        } else {
            commands
                .spawn((
                    cloth_part(fore_name, &assets.forearm_mesh, fore_rest),
                    ChildOf(upper),
                ))
                .id()
        };
        let (hand_mesh, hand_name) = match side {
            BodySide::Left => (&assets.left_hand_mesh, "Left hand"),
            BodySide::Right => (&assets.right_hand_mesh, "Right hand"),
        };
        commands.spawn((
            fixed_part(
                hand_name,
                hand_mesh,
                &skin,
                Transform::from_xyz(0.0, WRIST_Y, 0.0),
            ),
            ChildOf(forearm),
        ));
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
                hose_part(thigh_name, &assets.thigh_mesh, thigh_rest),
                ChildOf(pelvis),
            ))
            .id();
        let shin = commands
            .spawn((
                hose_part(shin_name, &assets.shin_mesh, shin_rest),
                ChildOf(thigh),
            ))
            .id();
        commands.spawn((
            fixed_part(
                match side {
                    BodySide::Left => "Left foot",
                    BodySide::Right => "Right foot",
                },
                &assets.foot_mesh,
                &assets.leather_material,
                Transform::from_xyz(0.0, ANKLE_Y, 0.0),
            ),
            ChildOf(shin),
        ));
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
// The body lineup (dev only)
// ---------------------------------------------------------------------------

/// Set `CATHEDRAL_BODY_LINEUP=1` to stand a rank of puppets on the western
/// approach, one per (outfit class × build), for eyeballing the model itself.
pub const BODY_LINEUP_ENV: &str = "CATHEDRAL_BODY_LINEUP";
/// Where the rank stands: on the open paving north of the player spawn
/// (`PLAYER_SPAWN` = (0, 0.91, 95)), so a `tp 0 1.5 <z> 180` looks straight
/// down it. Root y matches the sim's walk plane so the feet land on the ground.
const LINEUP_CENTER: Vec3 = Vec3::new(0.0, 0.91, 100.0);
const LINEUP_SPACING_M: f32 = 1.25;

/// A fixed cast of appearances covering every outfit class, both builds and
/// every headgear — the comparison rig for body work.
///
/// These puppets carry no [`ActorView`], so reconcile never sees them and the
/// pose pipeline never animates them: they hold the authored rest pose, which
/// is exactly what a model review wants. Nothing about them reaches the sim.
fn lineup_cast() -> Vec<AppearanceSnapshot> {
    const CLASSES: [OutfitClass; 7] = [
        OutfitClass::Cleric,
        OutfitClass::Merchant,
        OutfitClass::Craftsman,
        OutfitClass::Laborer,
        OutfitClass::Watch,
        OutfitClass::Notable,
        OutfitClass::Poor,
    ];
    const HEADGEAR: [Headgear; 5] = [
        Headgear::Hood,
        Headgear::None,
        Headgear::Brim,
        Headgear::None,
        Headgear::KettleHelm,
    ];
    let mut cast = Vec::new();
    for (index, class) in CLASSES.into_iter().enumerate() {
        for (build_index, build) in [Build::Female, Build::Male].into_iter().enumerate() {
            cast.push(AppearanceSnapshot {
                build,
                outfit: class,
                headgear: if index == 0 && build_index == 1 {
                    Headgear::Coif
                } else {
                    HEADGEAR[(index * 2 + build_index) % HEADGEAR.len()]
                },
                // Spread over the tint band and the face set deterministically.
                palette_seed: (index as u32 * 2 + build_index as u32)
                    .wrapping_mul(0x27D4_EB2D)
                    .wrapping_add(0x1234_5678),
                bespoke: None,
            });
        }
    }
    cast
}

/// Spawns the lineup when its env var is set; a no-op otherwise (and always in
/// a normal run, so there is zero cost or behavior change without the flag).
pub(crate) fn spawn_body_lineup(mut commands: Commands, assets: Option<Res<BodyAssets>>) {
    let Some(assets) = assets else {
        return;
    };
    if std::env::var(BODY_LINEUP_ENV).is_err() {
        return;
    }
    let cast = lineup_cast();
    let fade = crowd_fade();
    let span = (cast.len() as f32 - 1.0) * LINEUP_SPACING_M;
    for (index, appearance) in cast.iter().enumerate() {
        let x = LINEUP_CENTER.x - span * 0.5 + index as f32 * LINEUP_SPACING_M;
        let actor_id = ActorId(format!("lineup{index:02}"));
        let root = commands
            .spawn((
                Name::new(format!("Body lineup {index:02}: {:?}", appearance.outfit)),
                Transform::from_xyz(x, LINEUP_CENTER.y, LINEUP_CENTER.z),
                Visibility::default(),
            ))
            .id();
        let rig = spawn_body(&mut commands, root, &assets, &actor_id, appearance, &fade);
        commands.entity(root).insert(rig);
    }
    info!(
        "[body lineup] {} puppets at ({}, {}) — tp 0 1.5 {} 180 to view",
        cast.len(),
        LINEUP_CENTER.x,
        LINEUP_CENTER.z,
        LINEUP_CENTER.z - 4.0
    );
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
/// Urgency (`features/extra_pockets.md` M3's poop clock) presses the walk into
/// a mincing hurry: the visual cadence quickens by up to 40 %. Like the drunk
/// stagger this reshapes only the phase the sim already advanced — the root
/// stays exactly where the sim put it, so quick little steps are a look, not a
/// moved actor.
const URGENT_CADENCE_GAIN: f32 = 0.40;
/// Urgency drops the arm swing toward 0.55× (1 − 0.45): the arms are held in,
/// not swung. Composed with the weary drop, so a tired *and* pressed body has
/// barely any swing left.
const URGENT_ARM_DROP: f32 = 0.45;
/// Urgency folds the torso forward — the clenched half-crouch. Deliberately
/// smaller than `WEARY_STOOP_RAD`: this is a body holding itself, not one
/// giving up.
const URGENT_STOOP_RAD: f32 = 0.12;
/// Urgency draws the thighs together (an adduction about the leg's local Z,
/// mirrored per side). Small on purpose — legible from a few metres, never a
/// pantomime.
const URGENT_KNEE_PINCH_RAD: f32 = 0.11;

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
/// The stall hand-over pulse (M2, `sale`): no standing offer exists for
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
    /// `law_and_order.md` M4c: the arm of somebody this body has taken hold of.
    /// It borrows the offer arm rather than growing a second one — the pose is
    /// the same extended reach — but it outranks both an offer and a pulse,
    /// because a hand that is on a person is not free to hold anything out.
    grip_target: Option<Vec3>,
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
    /// The stride phase urgency's quickened cadence has wound on ahead of the
    /// sim's own, in cycles, integrated frame by frame by
    /// [`advance_urgent_phase`] and wrapped into one cycle. It lives here rather
    /// than being derived inside `apply_locomotion` because a rate only becomes
    /// a phase by remembering the phase it has already produced.
    urgent_phase: f32,
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
            grip_target: None,
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
            urgent_phase: 0.0,
        }
    }

    /// The habit tier's L2 targets (§6): `carry` fills the basket-carry left
    /// arm; `offer_at` extends the right arm toward the world point.
    pub(super) fn set_hand_activity(&mut self, carry: bool, offer_at: Option<Vec3>) {
        self.carry_target = carry;
        self.offer_target = offer_at;
    }

    /// A short vendor hand-over pulse toward `at` — the `sale`
    /// choreography, where no standing offer exists to key the arm on.
    pub(super) fn pulse_offer(&mut self, at: Vec3, now: f64) {
        self.offer_pulse = Some((at, now + STALL_PULSE_SECONDS));
    }

    /// The one reach with no envelope of its own (`law_and_order.md` M4c): a
    /// hand on somebody's upper arm stays there until the law lets go, so this
    /// target is held across frames by [`super::hands::hold_the_seized`] rather
    /// than expiring like the hand-over pulse.
    pub(super) fn set_grip(&mut self, at: Option<Vec3>) {
        self.grip_target = at;
    }

    #[cfg(test)]
    pub(super) fn grip(&self) -> Option<Vec3> {
        self.grip_target
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

    // The snapshot-driven pose inputs, synced when a snapshot actually
    // arrives: the looping-dance flag (authoritative so a late-arriving
    // player sees the loop and it stops the frame `active_gesture` clears)
    // and the §8 carriage statuses (so a walker with drunkenness/weariness
    // sways and stoops). One mirror lookup per body serves both. Between
    // snapshots the mirror cannot answer differently, so skip the whole-cast
    // pass (and its per-body change flags) on unchanged frames.
    if !mirror.is_changed() {
        return;
    }
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

/// §8 carriage: the publicly-visible statuses that reshape the walk L0/L1
/// already computed. Fed from [`crate::smart_actors::model::ActorSnapshot`]
/// via [`BodyPoseState::set_carriage`]. `default` is sober, rested and
/// unpressed, and every field below is scaled by a status, so a defaulted
/// `Carriage` leaves the pose byte-identical.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Carriage {
    /// `0..=1`. Staggering phase noise, lateral sway, a wandering lean.
    drunkenness: f32,
    /// `0..=1`. Drops the arm swing and adds a forward stoop.
    weariness: f32,
    /// `0..=1`. Quickens the cadence, pinches the knees, drops the arm swing
    /// and adds a small forward stoop — the pressed walk of a full gut
    /// (`features/extra_pockets.md`).
    urgency: f32,
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
                K::Urgency => carriage.urgency = value,
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

/// Arm-swing amplitude after urgency clenches it toward 0.55× at `u = 1`.
/// Identity at `u = 0`; composed on top of [`weary_arm_swing`].
fn urgent_arm_swing(base: f32, urgency: f32) -> f32 {
    base * (1.0 - URGENT_ARM_DROP * urgency)
}

/// The visual cadence multiplier: urgency winds the stride up to 1.4× at
/// `u = 1` — the same "visual only, the root is the sim's" licence the drunk
/// phase wobble takes. Exactly 1.0 at `u = 0`. It multiplies a *rate*, never a
/// phase; [`advance_urgent_phase`] is the only thing entitled to apply it.
fn urgent_cadence(urgency: f32) -> f32 {
    1.0 + URGENT_CADENCE_GAIN * urgency
}

/// One frame of the surplus stride phase urgency has wound on ahead of the sim
/// — the extra cycles, in stride-cycle units, added to `gait_phase` before the
/// cycle is taken.
///
/// A cadence is a rate, so it has to be *integrated*. `gait_phase` is the sim's
/// unbounded accumulator (`world.rs` only ever adds to it, and `set_route`
/// deliberately carries it across route legs so the gait is seamless), so
/// scaling that absolute value by the cadence would displace the whole stride
/// by `gait_phase · Δk` the moment the pressure changed: 100 cycles into a walk
/// one sixteenth of urgency is a 2.5-cycle jump — half a stride, which swaps
/// which leg is forward — and the poop clock steps sixteen of them. Accruing
/// the surplus at the sim's own phase rate instead leaves the legs exactly
/// where they are and changes only how fast they go from here.
///
/// Wrapped into one cycle so the offset stays bounded however far anyone walks;
/// a whole cycle is invisible to the sine and cosine downstream.
fn advance_urgent_phase(current: f32, phase_rate: f32, dt: f32, urgency: f32) -> f32 {
    (current + phase_rate * dt * (urgent_cadence(urgency) - 1.0)).rem_euclid(1.0)
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
/// stoop plus urgency's smaller one, negative like the bow). All vanish at
/// status 0, so a defaulted `Carriage` returns `(0.0, 0.0)` and the walk/idle
/// torso is untouched.
fn carriage_torso(carriage: Carriage, now: f64, seed: u32) -> (f32, f32) {
    use std::f64::consts::TAU as TAU64;
    let d = carriage.drunkenness;
    let sway =
        d * DRUNK_SWAY_RAD * ((now * DRUNK_SWAY_HZ + f64::from(hash01(seed, 42))) * TAU64).sin() as f32;
    let lean = d * DRUNK_LEAN_RAD * slow_wander(now, seed, 43);
    let roll = (sway + lean).clamp(-DRUNK_ROLL_MAX_RAD, DRUNK_ROLL_MAX_RAD);
    // Both stoops fold the same way (negative X, like the bow) and simply add:
    // a tired body pressed for a privy is the most bent of all.
    let stoop = -(WEARY_STOOP_RAD * carriage.weariness + URGENT_STOOP_RAD * carriage.urgency);
    (roll, stoop)
}

/// L0 locomotion (§5): legs in opposite phase, lagged knee folds, arm
/// counter-swing, torso bob at double frequency plus lateral roll and turn
/// lean. Positive local-X rotation swings a hanging limb toward −Z (forward).
/// `carriage` (§8) is read here, not as a separate layer: drunkenness staggers
/// the visual phase and sways/leans the torso, weariness drops the arm swing
/// and stoops the torso, urgency pinches the knees and clenches the arms — all
/// zero at a default `Carriage`, so a sober walk is byte-identical to before M5.
/// Urgency's quickened cadence is the one carriage effect that cannot be
/// evaluated from a single frame: it arrives already integrated into
/// `gait_phase` by the caller (see [`advance_urgent_phase`]).
fn apply_locomotion(
    pose: &mut PoseDeltas,
    weight: f32,
    gait_phase: f32,
    yaw_rate: f32,
    carriage: Carriage,
    now: f64,
    seed: u32,
) {
    // Only bounded *offsets* are laid on the phase, never a scale of it — the
    // phase is an accumulator, so a factor on it is a teleport. Sober, the
    // drunk offset is 0 and the cycle is exactly the phase handed in.
    let cycle = (gait_phase + carriage.drunkenness * drunk_phase_wobble(now, seed)) * TAU;
    let swing = cycle.sin();

    // Urgency draws the thighs together — mirrored per side, so the pinch is
    // adduction rather than a lean. Zero at `u = 0`.
    let pinch = URGENT_KNEE_PINCH_RAD * carriage.urgency;
    pose.left_thigh.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_z(pinch) * Quat::from_rotation_x(THIGH_SWING_RAD * swing),
        ),
        weight,
    );
    pose.right_thigh.blend_over(
        JointDelta::from_rotation(
            Quat::from_rotation_z(-pinch) * Quat::from_rotation_x(-THIGH_SWING_RAD * swing),
        ),
        weight,
    );
    pose.left_shin.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(-SHIN_BEND_RAD * shin_gate(cycle))),
        weight,
    );
    pose.right_shin.blend_over(
        JointDelta::from_rotation(Quat::from_rotation_x(-SHIN_BEND_RAD * shin_gate(cycle + PI))),
        weight,
    );
    // Weariness drops the swing toward 0.3× and urgency clenches it further
    // toward 0.55×; rested and unpressed, the amplitude is unchanged.
    let arm_swing = urgent_arm_swing(
        weary_arm_swing(ARM_SWING_RAD, carriage.weariness),
        carriage.urgency,
    );
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
/// weary — or pressed — body still stoops; zero at a default `Carriage`.
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
        let mut phase_rate = 0.0_f32;
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
                // A restart is not a gap, so the clock never catches it: the sim
                // only ever *adds* to `gait_phase`, so a sample whose phase has
                // gone backwards can only be one. `set_route` begins again at 0
                // whenever a route is laid while `movement` is None, and several
                // ladder sites clear the movement and re-decide on the very next
                // tick — well inside `SAMPLE_STALE_SECONDS` — while the tick a
                // walker *arrives* reports 0 too. Sweeping to 0 from a phase 80
                // cycles deep would whirl the legs backwards through all eighty:
                // exactly the thrash the stale branch exists to prevent, so snap
                // for the same reason.
                let restarted = sample.gait_phase < history.cur_phase;
                history.prev_phase = if stale || restarted {
                    sample.gait_phase
                } else {
                    history.cur_phase
                };
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
                // The sim's own stride rate, read off the same pair of samples
                // the yaw rate is; urgency's cadence surplus integrates against
                // it below. A snapped pair (stale or restarted) reads 0, which
                // is right: nothing was walked between two unrelated samples.
                phase_rate =
                    (history.cur_phase - history.prev_phase) / MOVEMENT_TICK_SECONDS as f32;
                target_yaw_rate = angle_delta(history.prev_yaw, history.cur_yaw)
                    / MOVEMENT_TICK_SECONDS as f32;
            }
        }
        state.yaw_rate +=
            (target_yaw_rate - state.yaw_rate) * (dt * YAW_RATE_SMOOTHING_PER_S).min(1.0);
        state.walk_blend = move_toward(state.walk_blend, walk_factor(speed), dt / WALK_BLEND_SECONDS);
        state.urgent_phase =
            advance_urgent_phase(state.urgent_phase, phase_rate, dt, state.carriage.urgency);

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
                gait_phase + state.urgent_phase,
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
            // A custody grip outranks both (M4c): the arm is already on
            // somebody, so it cannot be holding anything out to them.
            let offer_at = state
                .grip_target
                .or(state.offer_target)
                .or(match state.offer_pulse {
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
//
// Every part is a *loft*: a stack of cross-sections revolved into a surface.
// That single primitive is what separates a body from a pile of sausages — a
// calf can swell and taper to an ankle, a torso can have a waist and a
// shoulder line, a boot can be a rounded box with a flat sole, and a garment
// can be a closed tube of cloth with a visible hem. Capsules and cuboids can
// do none of that.
// ---------------------------------------------------------------------------

/// Sector counts. Limbs are seen in silhouette more than in the round, so they
/// get fewer; the head and the garments carry the read and get more.
const LIMB_SECTORS: usize = 12;
const TORSO_SECTORS: usize = 20;
const GARMENT_SECTORS: usize = 22;
const HEAD_SECTORS: usize = 26;
const HEAD_STACKS: usize = 20;

/// One cross-section of a lofted part: a superellipse of `half_width` (x) by
/// `half_depth` (z), centred `offset_x`/`offset_z` off the part's axis, at
/// height `y`.
///
/// `roundness` is the superellipse exponent — 2 is a plain ellipse (limbs,
/// necks), 3–4 squares the section off toward a rounded box (a boot, a chest,
/// a hand). Rings are authored in the part's own local frame, with the origin
/// at the joint the part rotates around, so pose systems can rotate the part
/// transform in place (the invariant the whole rig depends on).
#[derive(Debug, Clone, Copy)]
pub(super) struct Ring {
    y: f32,
    half_width: f32,
    half_depth: f32,
    offset_x: f32,
    offset_z: f32,
    roundness: f32,
}

impl Ring {
    pub(super) const fn new(y: f32, half_width: f32, half_depth: f32) -> Self {
        Self {
            y,
            half_width,
            half_depth,
            offset_x: 0.0,
            offset_z: 0.0,
            roundness: 2.0,
        }
    }

    /// Shift this section off the part axis — how a foot leans forward over
    /// its ankle, or a chest sits proud of the spine.
    pub(super) const fn at(mut self, offset_x: f32, offset_z: f32) -> Self {
        self.offset_x = offset_x;
        self.offset_z = offset_z;
        self
    }

    /// Square the section off toward a rounded box.
    pub(super) const fn boxy(mut self, roundness: f32) -> Self {
        self.roundness = roundness;
        self
    }

    /// Mean radius — used for UV scaling and for the profile arc length.
    fn mean_radius(&self) -> f32 {
        (self.half_width + self.half_depth) * 0.5
    }

    fn point(&self, angle: f32) -> Vec3 {
        let (sin, cos) = angle.sin_cos();
        // Superellipse: |x/a|^n + |z/b|^n = 1, parametrised so n = 2 is the
        // plain ellipse and larger n pushes the corners out toward a box.
        let shape = |value: f32| value.abs().powf(2.0 / self.roundness) * value.signum();
        Vec3::new(
            self.offset_x + self.half_width * shape(cos),
            self.y,
            self.offset_z + self.half_depth * shape(sin),
        )
    }
}

/// Which ends of a loft get a flat cap. Ends buried inside a parent part (a
/// thigh top inside the pelvis) need none; ends the player can see do.
#[derive(Debug, Clone, Copy)]
pub(super) struct Caps {
    bottom: bool,
    top: bool,
}

impl Caps {
    pub(super) const NONE: Self = Self {
        bottom: false,
        top: false,
    };
    pub(super) const BOTH: Self = Self {
        bottom: true,
        top: true,
    };
    pub(super) const TOP: Self = Self {
        bottom: false,
        top: true,
    };
    pub(super) const BOTTOM: Self = Self {
        bottom: true,
        top: false,
    };
}

/// Revolves `rings` into a smooth-shaded surface.
///
/// Normals come from the two surface tangents (around the ring, and along the
/// profile), which makes them exact for any cross-section and any offset — and
/// it means a profile that *descends* (the outside of a skirt, traced from the
/// hem back up to the waist) gets consistently outward normals and consistent
/// winding without a flag, because both flip together. That is what lets a
/// garment be one closed tube of cloth — down the inside, round the hem, up
/// the outside — instead of a one-sided cone that shows its back faces.
///
/// UVs run the weave at a physical `tile_m` metres per repeat: `u` around the
/// ring by arc length, `v` along the profile by arc length.
pub(super) fn loft(rings: &[Ring], sectors: usize, caps: Caps, tile_m: f32) -> Mesh {
    debug_assert!(rings.len() >= 2 && sectors >= 3);
    let stride = (sectors + 1) as u32;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Profile arc length per ring, for a v that never stretches.
    let mut v = vec![0.0_f32; rings.len()];
    for index in 1..rings.len() {
        let (previous, ring) = (rings[index - 1], rings[index]);
        let along = Vec2::new(
            ring.mean_radius() - previous.mean_radius(),
            ring.y - previous.y,
        );
        v[index] = v[index - 1] + along.length();
    }
    let circumference =
        TAU * rings.iter().map(Ring::mean_radius).sum::<f32>() / rings.len() as f32;

    let sample = TAU / sectors as f32 * 0.25;
    for (index, ring) in rings.iter().enumerate() {
        let previous = rings[index.saturating_sub(1)];
        let next = rings[(index + 1).min(rings.len() - 1)];
        for sector in 0..=sectors {
            let angle = sector as f32 / sectors as f32 * TAU;
            let position = ring.point(angle);
            let around = ring.point(angle + sample) - ring.point(angle - sample);
            let along = next.point(angle) - previous.point(angle);
            let normal = along.cross(around).normalize_or(Vec3::Y);
            positions.push(position.to_array());
            normals.push(normal.to_array());
            uvs.push([
                angle / TAU * circumference / tile_m,
                v[index] / tile_m,
            ]);
        }
    }
    for ring in 0..rings.len() as u32 - 1 {
        for sector in 0..sectors as u32 {
            let a = ring * stride + sector;
            let (b, c, d) = (a + 1, a + stride, a + stride + 1);
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    for (wants_cap, ring, up) in [
        (caps.bottom, rings[0], false),
        (caps.top, rings[rings.len() - 1], true),
    ] {
        if !wants_cap {
            continue;
        }
        let normal = [0.0, if up { 1.0 } else { -1.0 }, 0.0];
        let center = positions.len() as u32;
        positions.push([ring.offset_x, ring.y, ring.offset_z]);
        normals.push(normal);
        uvs.push([0.5, 0.5]);
        let rim = positions.len() as u32;
        for sector in 0..=sectors {
            let angle = sector as f32 / sectors as f32 * TAU;
            let point = ring.point(angle);
            positions.push(point.to_array());
            normals.push(normal);
            uvs.push([point.x / tile_m, point.z / tile_m]);
        }
        for sector in 0..sectors as u32 {
            let (a, b) = (rim + sector, rim + sector + 1);
            if up {
                indices.extend_from_slice(&[center, a, b]);
            } else {
                indices.extend_from_slice(&[center, b, a]);
            }
        }
    }
    mesh_from_parts(positions, normals, uvs, indices)
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

/// Concatenates meshes into one part (a hand and its thumb, a boot and its
/// sole). All inputs must carry position/normal/UV and U32 indices, which
/// everything [`loft`] builds does.
pub(super) fn merge_meshes(parts: impl IntoIterator<Item = Mesh>) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for part in parts {
        let base = positions.len() as u32;
        let Some(VertexAttributeValues::Float32x3(part_positions)) =
            part.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        positions.extend_from_slice(part_positions);
        if let Some(VertexAttributeValues::Float32x3(part_normals)) =
            part.attribute(Mesh::ATTRIBUTE_NORMAL)
        {
            normals.extend_from_slice(part_normals);
        }
        if let Some(VertexAttributeValues::Float32x2(part_uvs)) =
            part.attribute(Mesh::ATTRIBUTE_UV_0)
        {
            uvs.extend_from_slice(part_uvs);
        }
        if let Some(Indices::U32(part_indices)) = part.indices() {
            indices.extend(part_indices.iter().map(|index| index + base));
        }
    }
    mesh_from_parts(positions, normals, uvs, indices)
}

// --- Trunk -----------------------------------------------------------------

/// The pelvis: crotch to waist, flaring over the hip crests. Authored
/// root-local (its joint transform is the identity apart from the build's hip
/// scaling). Both ends are buried — the thighs plug the bottom, the torso the
/// top — so it needs no caps.
fn pelvis_mesh() -> Mesh {
    loft(
        &[
            Ring::new(PELVIS_CROTCH_Y, 0.104, 0.094).boxy(2.4),
            Ring::new(-0.055, 0.146, 0.113).boxy(2.6),
            Ring::new(0.020, 0.157, 0.116).boxy(2.6),
            Ring::new(PELVIS_WAIST_Y, 0.139, 0.101).boxy(2.4),
        ],
        TORSO_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// The torso: waist, ribcage, shoulder line, and a rim that closes around the
/// neck. The last two rings matter more than they look — without them the top
/// of the torso is a flat plate with a head hovering over it, which is exactly
/// how the old puppet read. Origin at the waist.
fn torso_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.000, 0.132, 0.096).boxy(2.4),
            Ring::new(0.080, 0.148, 0.105).boxy(2.6),
            Ring::new(0.170, 0.166, 0.113).boxy(2.9),
            Ring::new(0.250, 0.181, 0.114).boxy(3.2),
            Ring::new(0.310, 0.190, 0.106).boxy(3.4),
            // The acromion shelf and the trapezius slope off it. Without
            // these two the torso pinches straight in above the chest and the
            // deltoids read as two balls stuck to a sack.
            Ring::new(0.352, 0.186, 0.096).boxy(3.2),
            Ring::new(0.392, 0.142, 0.080).boxy(2.6),
            Ring::new(TORSO_HEIGHT, 0.078, 0.068).boxy(2.2),
        ],
        TORSO_SECTORS,
        Caps::BOTTOM,
        CLOTH_TILE_M,
    )
}

/// The neck — bare skin between the collar rim and the jaw. Static relative to
/// the torso (the head pivots on top of it), so it is a plain child part.
/// Origin at the torso's waist, like the torso itself; its base starts inside
/// the collar so no gap can open at the seam.
fn neck_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.340, 0.070, 0.066),
            Ring::new(0.400, 0.056, 0.053),
            Ring::new(0.440, 0.051, 0.050),
            // Stops under the jaw: run it any higher and the column shows in
            // front of the chin instead of behind it.
            Ring::new(0.468, 0.049, 0.051),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

// --- Limbs -----------------------------------------------------------------

/// Upper arm: deltoid cap at the shoulder, tapering to the elbow. Origin at
/// the shoulder joint, hanging down −Y — the pivot convention every part
/// follows.
fn upper_arm_mesh() -> Mesh {
    loft(
        &[
            // The top ring stays small so the arm continues the trapezius
            // slope instead of budding off it: outer edge 0.19 under a torso
            // that is 0.165 wide there, then 0.215 under a 0.187 chest.
            Ring::new(0.030, 0.030, 0.030),
            Ring::new(-0.005, 0.055, 0.054),
            Ring::new(-0.080, 0.053, 0.052),
            Ring::new(-0.190, 0.047, 0.046),
            Ring::new(ELBOW_Y, 0.042, 0.041),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// Forearm: the flexor swell below the elbow, tapering hard into the wrist.
fn forearm_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.020, 0.043, 0.042),
            Ring::new(-0.055, 0.047, 0.045),
            Ring::new(-0.150, 0.037, 0.036),
            Ring::new(WRIST_Y, 0.029, 0.026),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// A hand: flattened palm, a finger block that tapers to the tips, and a thumb
/// lobe set out to the side. Origin at the wrist, fingers hanging −Y.
///
/// `side` only moves the thumb; the rest is symmetric. At three metres the
/// thumb is what makes a handover read as a hand receiving something rather
/// than a stump touching it.
fn hand_mesh(side: BodySide) -> Mesh {
    let sign = match side {
        BodySide::Left => -1.0,
        BodySide::Right => 1.0,
    };
    let palm = loft(
        &[
            Ring::new(0.006, 0.029, 0.025).boxy(2.6),
            Ring::new(-0.030, 0.036, 0.025).boxy(3.0),
            Ring::new(-0.075, 0.040, 0.023).boxy(3.2),
            Ring::new(-0.120, 0.037, 0.020).boxy(3.0),
            Ring::new(-0.152, 0.028, 0.015).boxy(2.6),
            Ring::new(-0.168, 0.015, 0.009),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    );
    // The thumb leaves the palm forward and out, so it reads from the front.
    let thumb = loft(
        &[
            Ring::new(0.000, 0.016, 0.015),
            Ring::new(-0.030, 0.015, 0.014),
            Ring::new(-0.055, 0.011, 0.010),
        ],
        8,
        Caps::TOP,
        CLOTH_TILE_M,
    )
    .rotated_by(Quat::from_rotation_z(sign * 0.62) * Quat::from_rotation_x(-0.30))
    .translated_by(Vec3::new(sign * 0.026, -0.026, -0.012));
    merge_meshes([palm, thumb])
}

/// Thigh: hip to knee, thickest just below the hip.
fn thigh_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.035, 0.081, 0.084),
            Ring::new(-0.055, 0.088, 0.092),
            Ring::new(-0.200, 0.076, 0.080),
            Ring::new(-0.340, 0.062, 0.064),
            Ring::new(KNEE_Y, 0.055, 0.056),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// Shin: the calf swell, then a long taper to the ankle. The calf is the
/// silhouette cue that a leg is a leg and not a dowel.
fn shin_mesh() -> Mesh {
    loft(
        &[
            Ring::new(0.020, 0.054, 0.055),
            Ring::new(-0.065, 0.058, 0.064).at(0.0, -0.004),
            Ring::new(-0.150, 0.050, 0.053),
            Ring::new(-0.280, 0.040, 0.040),
            Ring::new(ANKLE_Y, 0.034, 0.035),
        ],
        LIMB_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// A turnshoe: heel, instep, toe, flat sole. Lofted along its own length and
/// then laid down, so the cross-sections run across the foot the way a
/// cobbler's would. Origin at the ankle, sole exactly on [`GROUND_Y`].
fn foot_mesh() -> Mesh {
    // Authored with +Y forward (toward the toe) and +Z up, then rotated so +Y
    // becomes −Z (the facing direction) and +Z becomes up.
    let sole = -FOOT_HEIGHT;
    let section = |forward: f32, half_width: f32, top: f32| {
        Ring::new(forward, half_width, (top - sole) * 0.5)
            .at(0.0, (top + sole) * 0.5)
            .boxy(3.4)
    };
    loft(
        &[
            section(-0.058, 0.030, 0.028),
            section(-0.028, 0.040, 0.046),
            section(0.018, 0.045, 0.026),
            section(0.082, 0.044, 0.000),
            section(0.136, 0.036, -0.022),
            section(0.168, 0.019, -0.044),
        ],
        LIMB_SECTORS,
        Caps::BOTH,
        CLOTH_TILE_M,
    )
    .rotated_by(Quat::from_rotation_x(-FRAC_PI_2))
}

// --- Head ------------------------------------------------------------------

/// Azimuthal face projection: maps a unit direction on the head ovoid into the
/// face image. The front pole (−Z) is the image centre; the frame edge lands
/// [`FACE_EDGE_ANGLE_RAD`] away; everything beyond runs off past [0,1] where
/// the clamped sampler paints the image's uniform skin-tone border.
///
/// Deliberately a function of the *undeformed* sphere direction: the head's
/// features (§ [`head_mesh`]) move vertex positions only, so the portrait
/// stays registered on the face however the skull is shaped, and the nose
/// geometry lands under the painted nose.
fn face_uv(direction: Vec3) -> Vec2 {
    let theta = (-direction.z).clamp(-1.0, 1.0).acos();
    let rho = (theta / FACE_EDGE_ANGLE_RAD).min(FACE_UV_RHO_MAX);
    let phi = direction.y.atan2(direction.x);
    Vec2::new(0.5 - 0.5 * rho * phi.cos(), 0.5 - 0.5 * rho * phi.sin())
}

/// Where the painted nose sits on the portrait, as an angle below the front
/// pole: the generated faces are cropped tight and centred, so the nose tip
/// lands a little under the image centre for all of them.
const NOSE_POLAR_RAD: f32 = 0.19;
/// Nose relief, and the angular half-widths of the ridge (narrow across, long
/// down the face).
const NOSE_PROJECTION_M: f32 = 0.023;
const NOSE_SPREAD_X_RAD: f32 = 0.24;
const NOSE_SPREAD_Y_RAD: f32 = 0.30;
/// The brow shelf: a wide, shallow ridge that catches the sun and puts the
/// eyes in shadow.
const BROW_POLAR_RAD: f32 = -0.30;
const BROW_PROJECTION_M: f32 = 0.008;
/// The chin, well below the front pole where the jaw taper has already pulled
/// the skull in.
const CHIN_POLAR_RAD: f32 = 0.86;
const CHIN_PROJECTION_M: f32 = 0.011;
/// Ears, at the sides just behind the widest point of the skull.
const EAR_PROJECTION_M: f32 = 0.013;

/// A sphere reads as a ball, not a head, so the ovoid is shaped on four axes:
/// tapered below the cheeks to a chin, its crown a touch narrower than the
/// temples, the face plane flattened and the occiput filled out behind. The
/// width tapers all sit below the head centre, where no headgear rides, so
/// hats and hoods keep their authored fit.
const HEAD_JAW_TAPER: f32 = 0.21;
const HEAD_CROWN_TAPER: f32 = 0.08;
const HEAD_FACE_FLATTEN: f32 = 0.94;
const HEAD_OCCIPUT_BULGE: f32 = 1.05;

/// The head: a shaped ovoid with the painted face projected onto its front
/// hemisphere and real relief for the nose, brow, chin and ears.
///
/// Origin at the neck joint; the shape is centred [`HEAD_CENTER_ABOVE_NECK`]
/// above it so the head pivots at the neck. Only vertex *positions* are
/// shaped — face UVs come from the unit `direction`, so the projection (and
/// its orientation contract) is independent of the shape, and the relief lands
/// under the painted feature rather than beside it. Normals are recomputed
/// from the shaped geometry so the warp still shades smoothly.
fn head_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for stack in 0..=HEAD_STACKS {
        let polar = stack as f32 / HEAD_STACKS as f32 * PI;
        let (ring_r, y) = (polar.sin(), polar.cos());
        for sector in 0..=HEAD_SECTORS {
            let angle = sector as f32 / HEAD_SECTORS as f32 * TAU;
            let direction = Vec3::new(ring_r * angle.cos(), y, ring_r * angle.sin());
            positions.push(head_point(direction).to_array());
            let uv = face_uv(direction);
            uvs.push([uv.x, uv.y]);
        }
    }
    let stride = (HEAD_SECTORS + 1) as u32;
    for stack in 0..HEAD_STACKS as u32 {
        for sector in 0..HEAD_SECTORS as u32 {
            let a = stack * stride + sector;
            let (b, c, d) = (a + 1, a + stride, a + stride + 1);
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_indices(Indices::U32(indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_computed_smooth_normals()
}

/// The skull surface for a unit sphere `direction`, head-local (origin at the
/// neck joint). Shared with the hair shell so the two can never drift apart.
fn head_point(direction: Vec3) -> Vec3 {
    // Jaw taper: full width through the cranium, narrowing over the lower head
    // to a chin; the crown rounds off a little narrower than the temples.
    let below = (-direction.y).clamp(0.0, 1.0);
    let jaw = 1.0 - HEAD_JAW_TAPER * below * below * (3.0 - 2.0 * below);
    let above = direction.y.clamp(0.0, 1.0);
    let crown = 1.0 - HEAD_CROWN_TAPER * above * above;
    let width = jaw * crown;
    // Depth: the face plane (−Z) sits flatter, the occiput (+Z) fuller.
    let front = (-direction.z).clamp(0.0, 1.0);
    let back = direction.z.clamp(0.0, 1.0);
    let depth =
        width * (1.0 - (1.0 - HEAD_FACE_FLATTEN) * front + (HEAD_OCCIPUT_BULGE - 1.0) * back);
    let mut point = Vec3::new(
        direction.x * width * HEAD_HALF_WIDTH,
        direction.y * HEAD_HALF_HEIGHT,
        direction.z * depth * HEAD_HALF_DEPTH,
    );

    // Features, as displacements along −Z (out of the face) and ±X (ears).
    // Each is an angular lobe around a point on the face, so it lands on the
    // painted feature and fades out smoothly instead of creasing.
    let facing = (-direction.z).clamp(-1.0, 1.0).acos();
    if facing < 1.4 {
        // Angular offsets from the front pole, in the portrait's frame.
        let across = direction.x.atan2(-direction.z);
        let down = -direction.y.atan2((direction.x * direction.x + direction.z * direction.z).sqrt());
        let lobe = |dx: f32, dy: f32, sx: f32, sy: f32| {
            (-(dx * dx) / (sx * sx) - (dy * dy) / (sy * sy)).exp()
        };
        let nose = lobe(
            across,
            down - NOSE_POLAR_RAD,
            NOSE_SPREAD_X_RAD,
            NOSE_SPREAD_Y_RAD,
        );
        let brow = lobe(across, down - BROW_POLAR_RAD, 0.62, 0.13);
        let chin = lobe(across, down - CHIN_POLAR_RAD, 0.40, 0.22);
        point.z -= NOSE_PROJECTION_M * nose
            + BROW_PROJECTION_M * brow
            + CHIN_PROJECTION_M * chin;
    }
    // Ears: lobes on both flanks, level with the eyes and set back a little.
    let side = direction.x.abs();
    let ear = (-((side - 0.97).powi(2)) / 0.0016
        - (direction.y - 0.02).powi(2) / 0.030
        - (direction.z - 0.16).powi(2) / 0.055)
        .exp();
    point.x += EAR_PROJECTION_M * ear * direction.x.signum();

    point + Vec3::Y * HEAD_CENTER_ABOVE_NECK
}

/// Hair: a shell hugging the skull, cut to a hairline that rides high over the
/// brow, drops past the ears and covers the nape. Origin at the neck joint,
/// like the head it sits on.
///
/// Solves the documented "uncovered heads read bald from behind" hole: the
/// face texture clamps to flat skin over the whole rear cap, and the answer is
/// to put hair there rather than to paint the back of 24 portraits.
fn hair_mesh() -> Mesh {
    /// Hair volume at the crown; it thins to nothing at the rim so the
    /// hairline is an edge in the shading rather than a shelf sticking out.
    const THICKNESS: f32 = 0.013;
    /// Minimum stand-off, so the shell never z-fights the scalp.
    const LIFT: f32 = 0.003;
    /// Amplitude of the wave that keeps the shell from shading like a helmet.
    const WAVE_M: f32 = 0.0035;
    const STACKS: usize = 16;

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // The hairline, as the polar angle from the crown where the shell stops:
    // it stops above the brow at the face (≈62°), passes over the ears at the
    // flanks (≈88°) and covers the nape well below the equator behind (≈122°).
    // Any lower at the front and it reads as a bathing cap, not hair.
    let hairline = |angle: f32| {
        let front = -angle.sin(); // 1 at the face (−Z), −1 at the nape
        let base = 1.54 - 0.46 * front.max(0.0) + 0.59 * (-front).max(0.0);
        // A clean arc across the forehead reads as the brim of a cap; a few
        // low harmonics make it a hairline.
        base * (1.0 + 0.055 * (angle * 3.0 + 1.1).cos() + 0.028 * (angle * 7.0).cos())
    };

    for stack in 0..=STACKS {
        let t = stack as f32 / STACKS as f32;
        // Full thickness over the crown, tapering out over the last quarter.
        let volume = LIFT + THICKNESS * (1.0 - t).min(0.25) / 0.25;
        for sector in 0..=HEAD_SECTORS {
            let angle = sector as f32 / HEAD_SECTORS as f32 * TAU;
            let polar = t * hairline(angle);
            let (sin, cos) = polar.sin_cos();
            let direction = Vec3::new(sin * angle.cos(), cos, sin * angle.sin());
            let scalp = head_point(direction);
            let outward = (scalp - Vec3::Y * HEAD_CENTER_ABOVE_NECK).normalize_or(Vec3::Y);
            // A shallow wave around the head and down it — a perfectly smooth
            // shell shades like a helmet, and this is the cheapest thing that
            // makes the highlight break up like hair instead.
            let wave = WAVE_M
                * (angle * 5.0).cos()
                * (t * PI).sin()
                * (0.6 + 0.4 * (polar * 4.0).cos());
            positions.push((scalp + outward * (volume + wave)).to_array());
            uvs.push([angle / TAU, t]);
        }
    }
    let stride = (HEAD_SECTORS + 1) as u32;
    for stack in 0..STACKS as u32 {
        for sector in 0..HEAD_SECTORS as u32 {
            let a = stack * stride + sector;
            let (b, c, d) = (a + 1, a + stride, a + stride + 1);
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    // Normals from the waved geometry, not from the sphere it started as —
    // the wave only reads if the shading follows it.
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_indices(Indices::U32(indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_computed_smooth_normals()
}

// --- Headgear --------------------------------------------------------------

/// A surface of revolution around +Y from a `(radius, y)` profile, with the
/// rim optionally sheared downward toward the back (+Z) so a hood can cover
/// the nape while leaving the face open. Origin at the head-ovoid centre —
/// headgear parents to the head at [`HEAD_CENTER_ABOVE_NECK`].
fn revolved_cap(profile: &[(f32, f32)], sectors: usize, back_drop: f32, uv_span: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let crown_y = profile[0].1;
    let rim_y = profile[profile.len() - 1].1;
    let depth = (crown_y - rim_y).max(1e-4);

    for (ring, (radius, y)) in profile.iter().enumerate() {
        let (previous, next) = (
            profile[ring.saturating_sub(1)],
            profile[(ring + 1).min(profile.len() - 1)],
        );
        let (dr, dy) = (next.0 - previous.0, next.1 - previous.1);
        let n = Vec2::new(-dy, dr).normalize_or(Vec2::Y);
        let shear_weight = (crown_y - y) / depth;
        for sector in 0..=sectors {
            let angle = sector as f32 / sectors as f32 * TAU;
            let (sin, cos) = angle.sin_cos();
            let backness = (1.0 + sin) * 0.5;
            let y_sheared = y - back_drop * backness * shear_weight;
            positions.push([radius * cos, y_sheared, radius * sin]);
            let normal = Vec3::new(n.x * cos, n.y, n.x * sin).normalize_or(Vec3::Y);
            normals.push([normal.x, normal.y, normal.z]);
            uvs.push([angle / TAU * uv_span, shear_weight * uv_span]);
        }
    }
    let stride = (sectors + 1) as u32;
    for ring in 0..profile.len() as u32 - 1 {
        for sector in 0..sectors as u32 {
            let a = ring * stride + sector;
            let (b, c, d) = (a + 1, a + stride, a + stride + 1);
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    mesh_from_parts(positions, normals, uvs, indices)
}

/// Hood: a bulky draped cowl, open at the face, draped low over the nape.
fn hood_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.163),
            (0.052, 0.158),
            (0.098, 0.140),
            (0.130, 0.104),
            (0.148, 0.058),
            (0.152, 0.026),
        ],
        20,
        0.115,
        0.6,
    )
}

/// Coif: a close-fitting linen cap tied under the skull. Deliberately snug —
/// any looser and it reads as a bonnet.
fn coif_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.128),
            (0.038, 0.125),
            (0.068, 0.110),
            (0.090, 0.078),
            (0.098, 0.040),
        ],
        20,
        0.062,
        0.5,
    )
}

/// Brimmed hat: low felt crown on a wide disc.
fn brim_mesh() -> Mesh {
    revolved_cap(
        &[
            (0.001, 0.168),
            (0.055, 0.163),
            (0.088, 0.136),
            (0.093, 0.062),
            (0.176, 0.048),
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
            (0.001, 0.172),
            (0.046, 0.168),
            (0.086, 0.148),
            (0.113, 0.104),
            (0.118, 0.062),
            (0.166, 0.032),
        ],
        20,
        0.0,
        0.5,
    )
}

// --- Garments --------------------------------------------------------------

/// How far below the waist a garment's hem falls, and how wide it flares
/// there. The hem must clear the leg it covers: a thigh swings ±27° at a
/// walk (`THIGH_SWING_RAD`), so the knee travels 0.19 m fore and aft, and a
/// hem narrower than that would saw through the leg every stride. Every
/// (drop, flare) pair below is checked against that in
/// `garment_hems_clear_the_leg_swing`.
#[derive(Debug, Clone, Copy)]
struct GarmentCut {
    /// Hem height, root-local.
    hem_y: f32,
    /// Hem half-width.
    flare: f32,
}

/// Below-the-knee robe: clerics and notables. Wide enough that a full stride
/// stays inside it, which is also simply what a habit looks like.
const ROBE: GarmentCut = GarmentCut {
    hem_y: -0.46,
    flare: 0.356,
};
/// Mid-thigh tunic: the working city's default.
const TUNIC: GarmentCut = GarmentCut {
    hem_y: -0.220,
    flare: 0.262,
};
/// Short tunic, cut for work: labourers.
const SHORT_TUNIC: GarmentCut = GarmentCut {
    hem_y: -0.168,
    flare: 0.245,
};

/// A garment skirt as a *closed tube* of cloth: down the inside from the
/// waist, round the hem, and back up the outside.
///
/// The point of the tube is that it needs no double-sided material — the
/// player sees cloth from every angle, including a real thickness at the hem —
/// and the loft's tangent-derived normals make the descending run come out
/// correct on its own. Origin at the root (it parents to the pelvis), so the
/// hips carry it.
fn skirt_mesh(cut: GarmentCut) -> Mesh {
    const THICKNESS: f32 = 0.012;
    let top = PELVIS_WAIST_Y + 0.02;
    let waist = 0.152;
    // Cloth hangs off the hips and only swings out low down, so the middle
    // ring sits well past halfway with barely any of the flare spent — the
    // difference between a tunic and a bell.
    let mid_y = top + (cut.hem_y - top) * 0.52;
    let mid = waist + (cut.flare - waist) * 0.44;
    loft(
        &[
            // Down the inside.
            Ring::new(top, waist - THICKNESS, waist * 0.80 - THICKNESS),
            Ring::new(mid_y, mid - THICKNESS, mid * 0.86 - THICKNESS),
            Ring::new(cut.hem_y + 0.012, cut.flare - THICKNESS, cut.flare * 0.9 - THICKNESS),
            // Round the hem.
            Ring::new(cut.hem_y, cut.flare - THICKNESS * 0.4, cut.flare * 0.9 - THICKNESS * 0.4),
            Ring::new(cut.hem_y - 0.004, cut.flare, cut.flare * 0.9),
            // Up the outside.
            Ring::new(cut.hem_y + 0.012, cut.flare, cut.flare * 0.9),
            Ring::new(mid_y, mid, mid * 0.86),
            Ring::new(top, waist, waist * 0.80),
        ],
        GARMENT_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// A belt: a narrow band around the waist, sitting proud of the garment it
/// cinches — hence radii a centimetre outside the skirt's at the same height,
/// not the body's. Origin at the root, like the skirt.
fn belt_mesh() -> Mesh {
    let y = PELVIS_WAIST_Y - 0.080;
    loft(
        &[
            Ring::new(y - 0.023, 0.168, 0.136).boxy(2.6),
            Ring::new(y - 0.014, 0.176, 0.144).boxy(2.6),
            Ring::new(y + 0.014, 0.176, 0.144).boxy(2.6),
            Ring::new(y + 0.023, 0.168, 0.136).boxy(2.6),
        ],
        TORSO_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
    )
}

/// A shoulder mantle — the short cape a cleric or a notable wears. Same
/// closed-tube construction as the skirt, hung from the torso, so rank reads
/// as a distinct garment layer at 30 m. It stops above the elbow, so the arms
/// clearly emerge from under it. Origin at the torso's waist.
fn mantle_mesh() -> Mesh {
    const THICKNESS: f32 = 0.011;
    loft(
        &[
            // Up the inside, from the hem.
            Ring::new(0.128, 0.203 - THICKNESS, 0.152 - THICKNESS).boxy(2.8),
            Ring::new(0.250, 0.213 - THICKNESS, 0.150 - THICKNESS).boxy(3.0),
            Ring::new(0.345, 0.194 - THICKNESS, 0.124 - THICKNESS).boxy(2.8),
            Ring::new(0.388, 0.132, 0.090).boxy(2.4),
            // Over the shoulder ridge and back down the outside.
            Ring::new(0.396, 0.130, 0.088).boxy(2.4),
            Ring::new(0.345, 0.194, 0.124).boxy(2.8),
            Ring::new(0.250, 0.213, 0.150).boxy(3.0),
            Ring::new(0.120, 0.203, 0.152).boxy(2.8),
        ],
        GARMENT_SECTORS,
        Caps::NONE,
        CLOTH_TILE_M,
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

    /// Reads a mesh's positions and normals back for the geometry contracts.
    fn geometry(mesh: &Mesh) -> (Vec<Vec3>, Vec<Vec3>) {
        let read = |attribute| match mesh.attribute(attribute) {
            Some(VertexAttributeValues::Float32x3(values)) => {
                values.iter().map(|v| Vec3::from_array(*v)).collect()
            }
            _ => Vec::new(),
        };
        (
            read(Mesh::ATTRIBUTE_POSITION),
            read(Mesh::ATTRIBUTE_NORMAL),
        )
    }

    /// The loft's normals must face *out* of the surface, and must do so
    /// whether the profile climbs or falls — that is the whole reason a
    /// garment can be one closed tube of cloth rather than a one-sided cone.
    #[test]
    fn loft_normals_point_outward_up_and_down_the_profile() {
        // A plain rising cone.
        let rising = loft(
            &[Ring::new(0.0, 0.20, 0.20), Ring::new(0.5, 0.10, 0.10)],
            12,
            Caps::NONE,
            1.0,
        );
        let (positions, normals) = geometry(&rising);
        for (position, normal) in positions.iter().zip(&normals) {
            let radial = Vec3::new(position.x, 0.0, position.z).normalize();
            assert!(
                normal.dot(radial) > 0.5,
                "rising wall points inward at {position:?}: {normal:?}"
            );
        }

        // The same wall authored top-down: the normals must flip with it, so
        // the *inside* of a tube comes out facing its cavity.
        let falling = loft(
            &[Ring::new(0.5, 0.10, 0.10), Ring::new(0.0, 0.20, 0.20)],
            12,
            Caps::NONE,
            1.0,
        );
        let (positions, normals) = geometry(&falling);
        for (position, normal) in positions.iter().zip(&normals) {
            let radial = Vec3::new(position.x, 0.0, position.z).normalize();
            assert!(
                normal.dot(radial) < -0.5,
                "descending wall points outward at {position:?}: {normal:?}"
            );
        }
    }

    /// A garment hem has to clear the leg it covers: the thigh swings
    /// `THIGH_SWING_RAD` either way at a walk, so a hem narrower than the
    /// knee's travel would saw through the leg every stride. This is the check
    /// the (drop, flare) pairs in [`ROBE`] &c. are chosen against.
    #[test]
    fn garment_hems_clear_the_leg_swing() {
        for (name, cut) in [
            ("robe", ROBE),
            ("tunic", TUNIC),
            ("short tunic", SHORT_TUNIC),
        ] {
            // How far down the leg the hem falls, and which bone is there.
            let below_hip = HIP_Y - cut.hem_y;
            let (along_bone, limb_radius) = if below_hip <= THIGH_LENGTH {
                // Thigh: interpolate its authored taper at that depth.
                let t = below_hip / THIGH_LENGTH;
                (below_hip, 0.088 - 0.033 * t)
            } else {
                (below_hip, 0.060)
            };
            let swing = HIP_X + along_bone * THIGH_SWING_RAD.sin() + limb_radius;
            assert!(
                cut.flare >= swing,
                "{name} hem {} is inside the leg's {swing:.3} m swing",
                cut.flare
            );
            // …but not comically wider than it needs to be, or it reads as a
            // crinoline rather than as clothing.
            assert!(
                cut.flare < swing + 0.05,
                "{name} hem {} is a bell, not a garment (needs {swing:.3})",
                cut.flare
            );
        }
    }

    /// The whole point of authoring against real proportions: a standing
    /// puppet's soles land exactly on the ground, and its crown lands at the
    /// silhouette height the streets and doors were built for.
    #[test]
    fn the_skeleton_stands_on_the_ground_at_the_authored_height() {
        let ankle = HIP_Y + KNEE_Y + ANKLE_Y;
        let (positions, _) = geometry(&foot_mesh());
        let sole = positions
            .iter()
            .fold(f32::MAX, |lowest, point| lowest.min(point.y));
        assert!(
            (ankle + sole - GROUND_Y).abs() < 1e-4,
            "sole lands at {}, not {GROUND_Y}",
            ankle + sole
        );

        let head_top = TORSO_JOINT_Y + NECK_JOINT_Y + HEAD_CENTER_ABOVE_NECK + HEAD_HALF_HEIGHT;
        let stature = head_top - GROUND_Y;
        assert!(
            (1.69..=1.73).contains(&stature),
            "silhouette height drifted to {stature} m"
        );
        // The chin has to clear the collar, or the head sits in the torso
        // with no neck — how the first cut of this rig read.
        let chin = TORSO_JOINT_Y + NECK_JOINT_Y + HEAD_CENTER_ABOVE_NECK - HEAD_HALF_HEIGHT;
        let collar = TORSO_JOINT_Y + TORSO_HEIGHT;
        assert!(
            (0.005..=0.06).contains(&(chin - collar)),
            "neck shows {} m of skin between collar and chin",
            chin - collar
        );
    }

    /// Hands mirror: same shape, thumbs on opposite sides.
    #[test]
    fn hands_mirror_across_the_body() {
        let (left, _) = geometry(&hand_mesh(BodySide::Left));
        let (right, _) = geometry(&hand_mesh(BodySide::Right));
        assert_eq!(left.len(), right.len());
        let reach = |hand: &[Vec3]| {
            hand.iter()
                .fold(f32::MIN, |widest, point| widest.max(point.x))
        };
        // The thumb is the only asymmetry, so each hand reaches further to
        // its own side than the other does.
        assert!(reach(&right) > reach(&left) + 0.01, "thumbs are not mirrored");
        // Both hang from the wrist down past the fingertips, same length.
        let drop = |hand: &[Vec3]| hand.iter().fold(f32::MAX, |low, p| low.min(p.y));
        assert!((drop(&left) - drop(&right)).abs() < 1e-5);
        assert!(drop(&left) < -0.15, "hand is too short to read as a hand");
    }

    /// The face relief has to land on the painted feature, not beside it:
    /// the nose is the one place the skull must stand proud of the ovoid.
    #[test]
    fn the_face_carries_relief_where_the_portrait_paints_it() {
        let on_sphere = |polar_down: f32, across: f32| {
            let (sin_d, cos_d) = polar_down.sin_cos();
            let (sin_a, cos_a) = across.sin_cos();
            Vec3::new(cos_d * sin_a, -sin_d, -cos_d * cos_a)
        };
        let plain = |direction: Vec3| {
            // The same ovoid without any feature displacement.
            direction.z * HEAD_HALF_DEPTH
        };
        let nose_tip = head_point(on_sphere(NOSE_POLAR_RAD, 0.0));
        assert!(
            nose_tip.z < plain(on_sphere(NOSE_POLAR_RAD, 0.0)) - 0.015,
            "no nose: {nose_tip:?}"
        );
        // …and the cheek beside it stays flat.
        let cheek = on_sphere(NOSE_POLAR_RAD, 0.9);
        assert!(
            (head_point(cheek).z - plain(cheek) * 0.94).abs() < 0.006,
            "the nose lobe has smeared across the cheek"
        );
        // The back of the head carries none of it.
        let back = head_point(Vec3::Z);
        assert!(back.z > HEAD_HALF_DEPTH, "occiput lost its bulge: {back:?}");
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
            urgency: 0.0,
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
            urgency: 0.0,
        };
        let (roll, pitch) = carriage_torso(weary, 2.0, 0x2468);
        assert_eq!(roll, 0.0, "weariness does not sway");
        assert!((pitch + WEARY_STOOP_RAD).abs() < 1e-6, "full weariness folds the torso");
        let mut pose = PoseDeltas::default();
        apply_locomotion(&mut pose, 1.0, 0.0, 0.0, weary, 2.0, 0x2468);
        let (_, _, x) = pose.torso.rotation.to_euler(EulerRot::ZYX);
        assert!(x < -0.1, "the weary torso pitches forward: {x}");
    }

    /// Urgency (u = 1) quickens the cadence, clenches the arm swing, pinches
    /// the thighs together and folds the torso a little — every one of them
    /// exactly identity at u = 0.
    #[test]
    fn urgency_quickens_clenches_and_stoops() {
        assert_eq!(urgent_cadence(0.0), 1.0);
        assert!((urgent_cadence(1.0) - (1.0 + URGENT_CADENCE_GAIN)).abs() < 1e-6);
        assert!(urgent_cadence(0.5) > urgent_cadence(0.0));

        assert_eq!(urgent_arm_swing(ARM_SWING_RAD, 0.0), ARM_SWING_RAD);
        assert!((urgent_arm_swing(ARM_SWING_RAD, 1.0) - 0.55 * ARM_SWING_RAD).abs() < 1e-6);
        assert!(urgent_arm_swing(ARM_SWING_RAD, 1.0) < urgent_arm_swing(ARM_SWING_RAD, 0.5));

        let urgent = Carriage {
            drunkenness: 0.0,
            weariness: 0.0,
            urgency: 1.0,
        };
        // A smaller fold than weariness's, and no sway at all.
        let (roll, pitch) = carriage_torso(urgent, 2.0, 0x9BDF);
        assert_eq!(roll, 0.0, "urgency does not sway");
        assert!((pitch + URGENT_STOOP_RAD).abs() < 1e-6, "urgency folds the torso: {pitch}");
        const { assert!(URGENT_STOOP_RAD < WEARY_STOOP_RAD) };

        // A second of walking at a brisk 1.2 cycles/s: the pressed body has
        // wound 0.4 of a stride on ahead of the sim's phase, the calm one
        // nothing at all. The cadence is integrated, so it is the *offset* that
        // grows — the phase handed to `apply_locomotion` is never scaled.
        let mut calm_phase = 0.0_f32;
        let mut pressed_phase = 0.0_f32;
        for _ in 0..50 {
            calm_phase = advance_urgent_phase(calm_phase, 1.2, 0.02, 0.0);
            pressed_phase = advance_urgent_phase(pressed_phase, 1.2, 0.02, 1.0);
        }
        assert_eq!(calm_phase, 0.0, "an unpressed body accrues no surplus");
        assert!(
            (pressed_phase - 1.2 * URGENT_CADENCE_GAIN).abs() < 1e-4,
            "a second of urgent walking is 0.4 of a stride ahead: {pressed_phase}"
        );

        // The walk: that surplus lands the legs further round the cycle, the
        // arms swing less, and the thighs are drawn together.
        let mut calm = PoseDeltas::default();
        let mut pressed = PoseDeltas::default();
        apply_locomotion(&mut calm, 1.0, 0.3, 0.0, Carriage::default(), 4.0, 0x9BDF);
        apply_locomotion(&mut pressed, 1.0, 0.3 + pressed_phase, 0.0, urgent, 4.0, 0x9BDF);
        assert!(
            calm.left_thigh
                .rotation
                .angle_between(pressed.left_thigh.rotation)
                > 0.05,
            "the quickened cadence must move the legs"
        );
        let arm_angle = |delta: &JointDelta| delta.rotation.to_euler(EulerRot::ZYX).2.abs();
        assert!(
            arm_angle(&pressed.right_upper_arm) < arm_angle(&calm.right_upper_arm),
            "urgency must clench the arm swing"
        );
        let (calm_z, _, _) = calm.left_thigh.rotation.to_euler(EulerRot::ZYX);
        let (pressed_z, _, _) = pressed.left_thigh.rotation.to_euler(EulerRot::ZYX);
        assert!(
            pressed_z > calm_z + 0.05,
            "the left thigh must adduct: {calm_z} -> {pressed_z}"
        );
    }

    /// A change of urgency must never *displace* the stride, however deep into a
    /// walk it lands. `gait_phase` is an unbounded accumulator, so the cadence
    /// may only be integrated on top of it — scaling it stepped the legs by
    /// `gait_phase · Δk`, half a stride at a time (which swaps which leg is
    /// forward), on each of the sixteen quantised steps the poop clock takes and
    /// instantly for `CATHEDRAL_DRIVE='status Ilse urgency 1'`.
    #[test]
    fn an_urgency_step_never_pops_the_legs_of_a_long_walk() {
        // ~150 m into one continuous route, and mid-stride rather than on a
        // cycle boundary, so any displacement is plainly visible.
        let gait_phase = 100.3_f32;
        let leg = |urgency: f32| {
            let mut pose = PoseDeltas::default();
            let carriage = Carriage {
                drunkenness: 0.0,
                weariness: 0.0,
                urgency,
            };
            apply_locomotion(&mut pose, 1.0, gait_phase, 0.0, carriage, 3.0, 0x5AA5);
            // The thigh's local X is the swing alone; its Z is urgency's
            // deliberate adduction, which is allowed to move.
            (
                pose.left_thigh.rotation.to_euler(EulerRot::ZYX).2,
                pose.left_shin.rotation,
            )
        };
        // Sixteenths, exactly as `Engine::ramp_urgency` quantises them.
        for step in 0..16 {
            let (before_swing, before_shin) = leg(step as f32 / 16.0);
            let (after_swing, after_shin) = leg((step + 1) as f32 / 16.0);
            assert!(
                (after_swing - before_swing).abs() < 1e-5,
                "urgency step {step} moved the thigh swing: {before_swing} -> {after_swing}"
            );
            assert!(
                before_shin.angle_between(after_shin) < 1e-5,
                "urgency step {step} moved the knee fold"
            );
        }
    }

    /// `Carriage::from_statuses` maps the snapshot slice onto its axes and
    /// ignores anything it has no pose for.
    #[test]
    fn carriage_reads_the_snapshot_statuses() {
        use cathedral_sim::StatusKind;
        assert_eq!(Carriage::from_statuses(&[]), Carriage::default());
        assert_eq!(
            Carriage::from_statuses(&[
                (StatusKind::Drunkenness, 0.7),
                (StatusKind::Weariness, 0.4),
                (StatusKind::Urgency, 0.9)
            ]),
            Carriage {
                drunkenness: 0.7,
                weariness: 0.4,
                urgency: 0.9,
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
                        pockets: Vec::new(),
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
                        pockets: Vec::new(),
                    },
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
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
                    pockets: Vec::new(),
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
                    pockets: Vec::new(),
                },
            ],
            items: vec![],
            offers: vec![],
            road_carts: vec![],
            marks: Vec::new(),
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
            pockets: Vec::new(),
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
                        pockets: Vec::new(),
                    },
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
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

    /// The sim restarts `gait_phase` at 0: `set_route` reads the old phase
    /// through `map_or(0.0, …)`, so a route laid while `movement` is None begins
    /// again — and several ladder sites clear the movement and re-decide at
    /// once, so the restart lands a tick later, nowhere near
    /// `SAMPLE_STALE_SECONDS`. The two-sample history has to recognise the
    /// restart itself; the clock cannot see it, and sweeping into it runs the
    /// legs backwards through every cycle already walked.
    #[test]
    fn a_restarted_gait_phase_snaps_instead_of_sweeping_backwards() {
        use bevy::asset::AssetPlugin;

        use crate::smart_actors::actors::reconcile_actor_views;
        use crate::smart_actors::model::{
            ActorControl, ActorSnapshot, MotionSample, Position, WorldMirror, WorldSnapshot,
        };

        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: vec![
                    ActorSnapshot {
                        id: ActorId("walker".into()),
                        name_for_player: "walker".into(),
                        control: ActorControl::Llm,
                        position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                    ActorSnapshot {
                        id: ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: ActorControl::Player,
                        position_m: Position::new(0.0, 0.91, 3.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance: Default::default(),
                        holds: vec![],
                        active_gesture: None,
                        statuses: Vec::new(),
                        pockets: Vec::new(),
                    },
                ],
                items: vec![],
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
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
            GlobalTransform::from(Transform::from_xyz(0.0, 1.7, 3.0)),
        ));
        app.update();

        let walker = ActorId("walker".into());
        let sample = |seq: u64, gait_phase: f32| MotionSample {
            position: Vec3::new(0.0, 0.91, 0.0),
            facing_yaw: 0.0,
            speed: 1.8,
            gait_phase,
            seq,
        };
        // 80 cycles — ~120 m — into one continuous errand.
        let world = app.world_mut();
        world
            .resource_mut::<MovementInbox>()
            .0
            .insert(walker.clone(), sample(1, 80.0));
        let root = world
            .query::<(Entity, &ActorId, &BodyRig)>()
            .iter(world)
            .find(|(_, id, _)| id.0 == "walker")
            .map(|(entity, _, _)| entity)
            .expect("walker spawned");
        world
            .entity_mut(root)
            .get_mut::<BodyPoseState>()
            .unwrap()
            .walk_blend = 1.0;
        app.update();
        let established_at = app.world().resource::<Time>().elapsed_secs_f64();

        // The errand is abandoned and the ladder lays a fresh route on the next
        // tick, so the next sample carries a phase of 0.25 milliseconds after
        // one of 80.
        app.world_mut()
            .resource_mut::<MovementInbox>()
            .0
            .insert(walker.clone(), sample(2, 0.25));
        app.update();
        let restarted_at = app.world().resource::<Time>().elapsed_secs_f64();
        assert!(
            restarted_at - established_at <= SAMPLE_STALE_SECONDS,
            "the restart must land inside the stale window or this proves nothing"
        );

        let history = app
            .world()
            .entity(root)
            .get::<BodyPoseState>()
            .unwrap()
            .history
            .expect("the walker has a gait history");
        assert_eq!(
            (history.prev_phase, history.cur_phase),
            (0.25, 0.25),
            "the restart must be snapped to, not swept into from 80 cycles"
        );

        // And the legs are where phase 0.25 puts them — mid-stride, thigh
        // forward — rather than back at the abandoned walk's phase.
        let world = app.world_mut();
        let thigh = world
            .query::<(&ActorId, &BodyRig)>()
            .iter(world)
            .find(|(id, _)| id.0 == "walker")
            .map(|(_, rig)| rig.left_thigh)
            .unwrap();
        let forwardness = (world.entity(thigh).get::<Transform>().unwrap().rotation * Vec3::NEG_Y).z;
        assert!(
            forwardness < -0.3,
            "the restarted stride must pose at its own phase: {forwardness}"
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
                    pockets: Vec::new(),
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
            pockets: Vec::new(),
        });
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors,
                items: vec![],
                offers: vec![],
                road_carts: vec![],
                marks: Vec::new(),
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
        // 7×4 outfit band + 7×4 hose band + 3 bespoke + 24 faces + 24 skins
        // + 6 hair + 1 leather + 4 headgear = 118.
        assert_eq!(materials.len(), 118);
        let meshes = app.world().resource::<Assets<Mesh>>();
        // pelvis, torso, neck, head, hair, upper arm, forearm, 2 hands,
        // thigh, shin, foot, belt, mantle, 3 skirt cuts, 4 headgear = 21.
        assert_eq!(meshes.len(), 21);

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
