//! The water network of `lore/wells_and_water.md`.
//!
//! Ombreval's speech distinguishes the structures an outsider lazily calls a
//! well, and so does this module. A **well** is a lined shaft into groundwater
//! whose permanent public bucket is raised by rope or chain; a **cistern** is a
//! roof-fed store reached through a small draw hatch; a **trough** is an open
//! working supply that nobody should cook from. Every source is assembled from
//! the same parts — curb, shaft, water, lifting gear, trough, apron, drain,
//! catch roof, gutter, settling box — arranged differently, because the city
//! repaired each one separately over four centuries.
//!
//! Two rules hold everywhere. The shaft is genuinely open: you can lean over a
//! curb and see the water below, because a hollow ring is the mesh and the
//! water is a real surface, not a stone lid. And the collision follows the
//! stonework, not the footprint: the curb, posts, troughs and vault stop you,
//! while the queue space, apron and roof shelter stay walkable.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::controller::CollisionWorld;

use super::{
    CityMaterials, CityMeshes, MeshData, add_rotated_box_collider, add_rotated_box_collider_at,
    spawn_box_named, spawn_cylinder, spawn_mesh_named, spawn_rotated_box_named,
};

/// The hollow curb's inner radius, as a fraction of its outer radius. The mouth
/// has to be wide enough to lower a bucket and to see the water from a standing
/// eye height, and narrow enough that the stone still reads as a thick rim.
const CURB_INNER_FRACTION: f32 = 0.66;

/// Water sits just above the ground plane. The shaft's darkness, not its
/// geometry, carries the depth: the city's ground is a single opaque surface,
/// so a shaft sunk below it would show earth rather than water when you look in.
const WATER_SURFACE_Y: f32 = 0.05;

/// A looping water sound placed at a source. The city only marks the spot; the
/// smart-actor audio layer owns playback (`smart_actors::sound`), so this
/// module never touches an `AudioSource` — which is also what keeps the city's
/// headless tests free of the audio plugins.
#[derive(Component, Debug, Clone)]
pub struct WaterAmbience {
    /// A `[[ambients]]` id from `assets/sounds/catalog.toml`.
    pub sound_id: &'static str,
    pub audible_distance: f32,
}

/// How a source lifts its water. Rope wears out and is cheap; chain survives
/// heavy, nearly continuous drawing and costs almost as much as a small bell.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lift {
    Rope,
    Chain,
}

/// The five authored mechanisms that can be driven by an authoritative well
/// draw. Other wells keep their static lifting gear until their simulation has
/// a corresponding activity source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimatedWell {
    Ford,
    Chain,
    ThreeCurb(u8),
}

impl AnimatedWell {
    fn index(self) -> usize {
        match self {
            Self::Ford => 0,
            Self::Chain => 1,
            Self::ThreeCurb(mouth) => 2 + usize::from(mouth.min(2)),
        }
    }

    fn phase_offset(self) -> f32 {
        match self {
            Self::Ford | Self::Chain => 0.0,
            Self::ThreeCurb(0) => 0.0,
            Self::ThreeCurb(1) => TAU / 3.0,
            Self::ThreeCurb(_) => 2.0 * TAU / 3.0,
        }
    }

    fn angular_speed(self) -> f32 {
        match self {
            Self::Ford => 1.55,
            Self::Chain => 1.25,
            Self::ThreeCurb(0) => 1.42,
            Self::ThreeCurb(1) => 1.34,
            Self::ThreeCurb(_) => 1.49,
        }
    }

    fn lift_distance(self) -> f32 {
        match self {
            Self::Ford => 0.24,
            Self::Chain => 0.19,
            Self::ThreeCurb(_) => 0.17,
        }
    }

    fn pause_jerk(self) -> f32 {
        match self {
            Self::ThreeCurb(0) => 0.22,
            Self::ThreeCurb(1) => -0.18,
            Self::ThreeCurb(_) => 0.14,
            Self::Ford | Self::Chain => 0.0,
        }
    }

    fn paused_sway(self) -> f32 {
        match self {
            Self::ThreeCurb(0) => -0.085,
            Self::ThreeCurb(1) => 0.075,
            Self::ThreeCurb(_) => -0.055,
            Self::Ford | Self::Chain => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MechanismPart {
    Rotor,
    Line,
    Bucket,
}

/// Semantic metadata attached to the already-rendered barrel, crank, line and
/// bucket pieces. `rest` is the exact authored transform; animation is always
/// reconstructed from it, so floating-point drift cannot accumulate.
#[derive(Component, Debug, Clone)]
pub(super) struct WellMechanismPart {
    well: AnimatedWell,
    part: MechanismPart,
    pivot: Vec3,
    axis: Vec3,
    rest: Transform,
}

#[derive(Debug, Clone, Copy, Default)]
struct MechanismPhase {
    angle: f32,
    blend: f32,
    paused: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct MechanismPose {
    spin: f32,
    lift: f32,
    sway: f32,
}

/// Per-system animation memory. It starts a newly active mechanism at rest,
/// advances only while the soundscape says a draw is underway, and freezes at
/// an intentionally awkward pose during Three-Curb's crossed-bucket pause.
#[derive(Debug, Default)]
pub(super) struct WellAnimationState {
    phases: [MechanismPhase; 5],
}

impl WellAnimationState {
    fn pose(
        &mut self,
        well: AnimatedWell,
        delta_seconds: f32,
        active: bool,
        paused: bool,
    ) -> MechanismPose {
        let state = &mut self.phases[well.index()];
        if !active {
            *state = MechanismPhase::default();
            return MechanismPose::default();
        }

        let dt = if delta_seconds.is_finite() {
            delta_seconds.max(0.0)
        } else {
            0.0
        };
        if paused {
            if !state.paused {
                // The first paused frame visibly snatches each handle a little
                // differently; subsequent frames hold it absolutely still.
                state.angle = (state.angle + well.pause_jerk()).rem_euclid(TAU);
            }
            state.paused = true;
        } else {
            state.paused = false;
            state.blend = (state.blend + dt / 0.3).min(1.0);
            state.angle = (state.angle + dt * well.angular_speed()).rem_euclid(TAU);
        }

        let phase = (state.angle + well.phase_offset() * state.blend).rem_euclid(TAU);
        let wave = 0.5 - 0.5 * phase.cos();
        MechanismPose {
            spin: phase,
            lift: well.lift_distance() * wave * state.blend,
            sway: 0.035 * phase.sin() * state.blend + if paused { well.paused_sway() } else { 0.0 },
        }
    }
}

/// Project the soundscape's read-only draw activity onto the authored lifting
/// gear. Absence of the soundscape resource is deliberately equivalent to no
/// activity, which keeps `CityPlugin` useful in headless geometry tests.
pub(super) fn animate_well_mechanisms(
    time: Res<Time>,
    activity: Option<Res<crate::soundscape::WellMechanismActivity>>,
    mut animation: Local<WellAnimationState>,
    mut parts: Query<(&WellMechanismPart, &mut Transform)>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Water);
    let dt = time.delta_secs();
    let ford_active = activity
        .as_deref()
        .is_some_and(|activity| activity.ford_active());
    let chain_active = activity
        .as_deref()
        .is_some_and(|activity| activity.chain_active());
    let three_active = activity
        .as_deref()
        .is_some_and(|activity| activity.three_curb_active());
    let three_paused = activity
        .as_deref()
        .is_some_and(|activity| activity.three_curb_conflict());

    let poses = [
        animation.pose(AnimatedWell::Ford, dt, ford_active, false),
        animation.pose(AnimatedWell::Chain, dt, chain_active, false),
        animation.pose(AnimatedWell::ThreeCurb(0), dt, three_active, three_paused),
        animation.pose(AnimatedWell::ThreeCurb(1), dt, three_active, three_paused),
        animation.pose(AnimatedWell::ThreeCurb(2), dt, three_active, three_paused),
    ];

    for (part, mut transform) in &mut parts {
        let pose = poses[part.well.index()];
        if pose == MechanismPose::default() {
            // An idle mechanism sits at rest; rewriting rest every frame
            // would keep every part (and its subtree) change-flagged forever.
            if *transform != part.rest {
                *transform = part.rest;
            }
            continue;
        }
        *transform = part.rest;

        let sway_axis = part.axis.cross(Vec3::Y).normalize_or(Vec3::Z);
        match part.part {
            MechanismPart::Rotor => {
                let turn = Quat::from_axis_angle(part.axis, pose.spin);
                transform.translation = part.pivot + turn * (part.rest.translation - part.pivot);
                transform.rotation = turn * part.rest.rotation;
            }
            MechanismPart::Line => {
                // Shorten from the bucket end while keeping the axle end fixed,
                // and lean the line just enough to meet the swaying bucket.
                let rest_bottom = part.rest.translation - Vec3::Y * (part.rest.scale.y * 0.5);
                let bottom = rest_bottom + Vec3::Y * pose.lift + sway_axis * pose.sway;
                let line = part.pivot - bottom;
                let length = line.length().max(0.01);
                transform.translation = (part.pivot + bottom) * 0.5;
                transform.rotation = Quat::from_rotation_arc(Vec3::Y, line / length);
                transform.scale.y = length;
            }
            MechanismPart::Bucket => {
                transform.translation += Vec3::Y * pose.lift + sway_axis * pose.sway;
                transform.rotation =
                    Quat::from_axis_angle(part.axis, pose.sway * 0.8) * part.rest.rotation;
            }
        }
    }
}

/// The curb's masonry. Three-Curb's mouths deliberately do not match: one is
/// worn old stone, one a neat post-Rains rebuilding, the third a blunt
/// Hammering repair made from whatever sound blocks came through the lane.
#[derive(Clone, Copy)]
enum Curbstone {
    Dressed,
    Old,
    Repair,
}

/// The ring mesh for a hollow well mouth: a unit cylinder (radius 1, height 1,
/// centred on the origin) with its middle taken out, so it scales exactly like
/// `CityMeshes::cylinder`.
pub(super) fn curb_ring_mesh() -> Mesh {
    const SEGMENTS: usize = 20;
    let mut data = MeshData::default();
    let (top, bottom) = (0.5, -0.5);
    for segment in 0..SEGMENTS {
        let (a, b) = (
            segment as f32 / SEGMENTS as f32 * TAU,
            (segment + 1) as f32 / SEGMENTS as f32 * TAU,
        );
        let (outer_a, outer_b) = (Vec2::from_angle(a), Vec2::from_angle(b));
        let (inner_a, inner_b) = (outer_a * CURB_INNER_FRACTION, outer_b * CURB_INNER_FRACTION);
        let point = |xz: Vec2, y: f32| Vec3::new(xz.x, y, xz.y);
        let u = |xz: Vec2| Vec2::new(xz.x * 0.5 + 0.5, xz.y * 0.5 + 0.5);

        // Outer face, then the inner face wound the other way so it is lit from
        // inside the shaft, then the rim you rest a pail on.
        data.quad(
            [
                point(outer_a, bottom),
                point(outer_b, bottom),
                point(outer_b, top),
                point(outer_a, top),
            ],
            point(outer_a + outer_b, 0.0).normalize_or(Vec3::X),
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        );
        data.quad(
            [
                point(inner_b, bottom),
                point(inner_a, bottom),
                point(inner_a, top),
                point(inner_b, top),
            ],
            -point(inner_a + inner_b, 0.0).normalize_or(Vec3::X),
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        );
        data.quad(
            [
                point(inner_a, top),
                point(inner_b, top),
                point(outer_b, top),
                point(outer_a, top),
            ],
            Vec3::Y,
            [u(inner_a), u(inner_b), u(outer_b), u(outer_a)],
        );
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_indices(Indices::U32(data.indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
}

/// Dispatch a plan fixture to its source. The plan says what each thing *is*
/// (`lore/places/ombreval_buildings.json`); the flourishes that distinguish two
/// wells of the same kind — Ford's heavy roof and Vhairé relief, Bitter's bare
/// curb, the Shambles' raised apron — are chosen from the stable fixture id.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_water_fixture(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision_world: &mut CollisionWorld,
    id: &str,
    kind: &str,
    position: Vec3,
    size: Vec2,
    angle: f32,
) {
    match kind {
        "well" => match id {
            "ford_well" => ford_well(commands, meshes, materials, collision_world, position),
            "bitter_well" => bitter_well(commands, meshes, materials, collision_world, position),
            "shambles_well" => {
                shambles_well(commands, meshes, materials, collision_world, position)
            }
            other => warn!("unrendered well '{other}'"),
        },
        "chain_well" => chain_well(commands, meshes, materials, collision_world, position),
        "three_curb_well" => three_curb(commands, meshes, materials, collision_world, position),
        "lodge_well" => lodge_well(commands, meshes, materials, collision_world, position),
        "cistern" => match id {
            "slate_cistern" => {
                slate_cistern(commands, meshes, materials, collision_world, position)
            }
            "tenter_cistern" => {
                tenter_cistern(commands, meshes, materials, collision_world, position)
            }
            "reed_cistern" => reed_cistern(commands, meshes, materials, collision_world, position),
            other => warn!("unrendered cistern '{other}'"),
        },
        "step_cistern" => step_cistern(commands, meshes, materials, collision_world, position),
        "fire_tanks" => fire_tanks(
            commands,
            meshes,
            materials,
            collision_world,
            position,
            size,
            angle,
        ),
        other => warn!("unrendered water fixture kind: {other}"),
    }
}

// ---------------------------------------------------------------- the sources

/// Ford Well: central, deep, reliable and crowded. A steep tiled roof shelters
/// a heavy windlass, pale stone curb, fixed public bucket, drinking lip, animal
/// trough, paved drain apron, queue space and the keeper's niche. The spill runs
/// into a covered street drain; there is no ornamental basin and no water left
/// standing merely to mirror the sky.
fn ford_well(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    apron(commands, meshes, materials, base, Vec2::new(11.0, 9.0), 0.0);
    well_mouth(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.55,
        1.0,
        Curbstone::Dressed,
        "Ford Well curb",
    );
    windlass(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.55,
        1.0,
        Lift::Rope,
        0.0,
        "Ford Well",
        Some(AnimatedWell::Ford),
    );
    roof_on_posts(
        commands,
        meshes,
        materials,
        collision,
        base,
        Vec2::new(5.6, 5.0),
        3.1,
        &materials.slate,
        "Ford Well",
    );

    // The worn relief of Saint Vhairé, because the ground is associated with her
    // crossing. It stands on its own pier beside the keeper's niche, clear of
    // the drawing space: travellers touch its lower edge with a wet thumb and
    // leave a wick on the dry ledge, and the water is not thereby made holy.
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        base + Vec3::new(3.9, 1.15, 1.1),
        Vec3::new(0.5, 2.3, 1.1),
        "Ford Well: Saint Vhairé's pier",
    );
    add_rotated_box_collider(
        collision,
        base + Vec3::new(3.9, 0.0, 1.1),
        Vec2::new(0.5, 1.1),
        0.0,
        2.3,
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        base + Vec3::new(3.6, 1.5, 1.1),
        Vec3::new(0.1, 1.1, 0.75),
        "Ford Well: the worn relief of Saint Vhairé",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        base + Vec3::new(3.58, 0.9, 1.1),
        Vec3::new(0.22, 0.1, 0.8),
        "Ford Well: the dry ledge",
    );

    trough(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, 3.5),
        Vec2::new(3.6, 1.1),
        0.0,
        "Ford Well animal trough",
    );
    drain(
        commands,
        meshes,
        materials,
        base + Vec3::new(0.0, 0.0, 4.6),
        Vec2::new(3.6, 0.5),
        0.0,
    );

    // The keeper's niche: a low stone bench and hood against the outer wall,
    // where the first white-glazed bowl of the morning is drawn and smelled.
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        base + Vec3::new(3.9, 0.45, -1.6),
        Vec3::new(1.1, 0.9, 2.2),
        "Ford Well keeper's niche",
    );
    add_rotated_box_collider(
        collision,
        base + Vec3::new(3.9, 0.0, -1.6),
        Vec2::new(1.1, 2.2),
        0.0,
        0.9,
    );

    ambience(commands, base, "well_trough", 26.0, "Ford Well");
}

/// Bitter Well: a deep, narrow shaft with a rope windlass, stone curb, small
/// animal trough and unusually little queue shelter. Sibbe Quern keeps it, and
/// the outer strands of the raising rope are fraying where they cross the upper
/// guide — ordinary, urgent maintenance, not sabotage.
fn bitter_well(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    apron(commands, meshes, materials, base, Vec2::new(6.0, 5.0), 0.0);
    well_mouth(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.05,
        0.95,
        Curbstone::Old,
        "Bitter Well curb",
    );
    windlass(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.05,
        0.95,
        Lift::Rope,
        0.0,
        "Bitter Well",
        None,
    );

    // The unusually little queue shelter: two posts and a scrap of pent roof.
    for side in [-1.0, 1.0] {
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            base + Vec3::new(side * 1.6, 1.35, -1.7),
            Vec3::new(0.16, 2.7, 0.16),
            "Bitter Well shelter post",
        );
    }
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.thatch,
        base + Vec3::new(0.0, 2.78, -1.3),
        Vec3::new(3.6, 0.14, 1.6),
        0.0,
        "Bitter Well shelter",
    );

    trough(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, 2.3),
        Vec2::new(2.2, 0.9),
        0.0,
        "Bitter Well animal trough",
    );
    ambience(commands, base, "well_trough", 20.0, "Bitter Well");
}

/// The Shambles well: uphill of the slaughter courts on a raised apron, with its
/// drain led away from the shaft. Marked work buckets; animals drink at separate
/// troughs. A well can be sound while the work around it is unpleasant.
fn shambles_well(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    // The raised apron is the point of the thing: it keeps the slaughter courts'
    // wash below the shaft instead of running back into it.
    let plinth = 0.4;
    spawn_box_named(
        commands,
        meshes,
        &materials.paving,
        base + Vec3::Y * plinth * 0.5,
        Vec3::new(8.0, plinth, 7.0),
        "Shambles well raised apron",
    );
    add_rotated_box_collider(collision, base, Vec2::new(8.0, 7.0), 0.0, plinth);

    let head = base + Vec3::Y * plinth;
    well_mouth(
        commands,
        meshes,
        materials,
        collision,
        head,
        1.2,
        0.95,
        Curbstone::Dressed,
        "Shambles well curb",
    );
    windlass(
        commands,
        meshes,
        materials,
        collision,
        head,
        1.2,
        0.95,
        Lift::Rope,
        0.0,
        "Shambles well",
        None,
    );
    roof_on_posts(
        commands,
        meshes,
        materials,
        collision,
        head,
        Vec2::new(4.4, 4.0),
        2.9,
        &materials.terracotta,
        "Shambles well",
    );

    // Animals drink at separate troughs, downhill of the curb.
    trough(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(-4.6, 0.0, 1.2),
        Vec2::new(1.0, 3.0),
        0.0,
        "Shambles beast trough",
    );
    drain(
        commands,
        meshes,
        materials,
        base + Vec3::new(-5.6, 0.0, 1.2),
        Vec2::new(0.5, 3.4),
        0.0,
    );
    ambience(commands, base, "well_trough", 22.0, "The Shambles well");
}

/// Chain Well: a deep shaft under a broad windlass with an iron chain and two
/// iron-bound public buckets, adopted because heavy drawing wore rope quickly
/// and loose buckets left with freight crews. Its slow clank, bucket knock and
/// windlass pawl can be heard around a corner before the curb is visible.
fn chain_well(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    apron(commands, meshes, materials, base, Vec2::new(7.0, 7.0), 0.0);
    well_mouth(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.3,
        1.0,
        Curbstone::Dressed,
        "Chain Well curb",
    );
    windlass(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.3,
        1.0,
        Lift::Chain,
        0.0,
        "Chain Well",
        Some(AnimatedWell::Chain),
    );

    // The second iron-bound bucket waits on the curb; the keeper's clean leather
    // guard sits between the bearings and the mouth so a greased axle cannot
    // reach the water.
    spawn_cylinder(
        commands,
        meshes,
        &materials.dark_wood,
        base + Vec3::new(1.55, 1.25, 0.9),
        0.24,
        0.5,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.iron,
        base + Vec3::new(1.55, 1.42, 0.9),
        0.26,
        0.06,
    );

    trough(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, 2.9),
        Vec2::new(3.2, 1.0),
        0.0,
        "Chain Well trough",
    );
    drain(
        commands,
        meshes,
        materials,
        base + Vec3::new(0.0, 0.0, 3.9),
        Vec2::new(3.2, 0.5),
        0.0,
    );
    ambience(commands, base, "chain_draw", 30.0, "Chain Well");
}

/// Three-Curb: one broad older shaft that three rebuilt courts each gave its own
/// stone mouth and windlass. All three mouths reach the same water — it is
/// neither three springs nor three separate shafts — and the three little roofs
/// meet badly above them, so rain finds the joints and ropes cross when drawers
/// are careless.
fn three_curb(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    apron(commands, meshes, materials, base, Vec2::new(9.0, 9.0), 0.0);

    // The three mouths sit around the shared head. Their masonry does not match,
    // and neither do their roofs.
    let mouths = [
        (0.0_f32, Curbstone::Old, 2.72_f32, 3.0_f32),
        (TAU / 3.0, Curbstone::Dressed, 2.95, 3.2),
        (2.0 * TAU / 3.0, Curbstone::Repair, 2.60, 2.85),
    ];
    for (index, (bearing, stone, ridge, post_height)) in mouths.into_iter().enumerate() {
        let offset = Vec3::new(bearing.cos() * 1.85, 0.0, bearing.sin() * 1.85);
        let head = base + offset;
        let court = index + 1;
        well_mouth(
            commands,
            meshes,
            materials,
            collision,
            head,
            0.95,
            0.9,
            stone,
            &format!("Three-Curb: the {} court's mouth", ordinal(court)),
        );
        windlass(
            commands,
            meshes,
            materials,
            collision,
            head,
            0.95,
            0.9,
            Lift::Rope,
            bearing,
            &format!("Three-Curb: the {} court's", ordinal(court)),
            Some(AnimatedWell::ThreeCurb(index as u8)),
        );
        // A little roof of its own, pitched to its own court and fitting none of
        // its neighbours.
        for side in [-1.0, 1.0] {
            let post = Vec3::new(
                (bearing + FRAC_PI_2).cos() * side * 1.35,
                post_height * 0.5,
                (bearing + FRAC_PI_2).sin() * side * 1.35,
            );
            spawn_box_named(
                commands,
                meshes,
                &materials.timber,
                head + post,
                Vec3::new(0.15, post_height, 0.15),
                "Three-Curb roof post",
            );
        }
        spawn_mesh_named(
            commands,
            &meshes.pyramid,
            if index == 1 {
                &materials.slate
            } else {
                &materials.terracotta
            },
            Transform::from_translation(head + Vec3::Y * (ridge + 0.35))
                .with_rotation(Quat::from_rotation_y(bearing + PI / 4.0))
                .with_scale(Vec3::new(2.5, 0.9, 2.5)),
            "Three-Curb roof",
        );
    }
    ambience(commands, base, "well_trough", 24.0, "Three-Curb");
}

/// Lodge Well: the roofed working well near the masons' lodge. Its public face
/// opens toward the outer lane; its work face feeds covered troughs inside the
/// yard, so households draw without walking between hoists and banker benches.
/// One trough is kept free of slurry at day's end, because water already above
/// ground is what answers a fire.
fn lodge_well(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    apron(commands, meshes, materials, base, Vec2::new(9.0, 7.0), 0.0);
    well_mouth(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.35,
        1.0,
        Curbstone::Dressed,
        "Lodge Well curb",
    );
    windlass(
        commands,
        meshes,
        materials,
        collision,
        base,
        1.35,
        1.0,
        Lift::Rope,
        FRAC_PI_2,
        "Lodge Well",
        None,
    );
    roof_on_posts(
        commands,
        meshes,
        materials,
        collision,
        base,
        Vec2::new(5.0, 4.4),
        3.0,
        &materials.terracotta,
        "Lodge Well",
    );

    // The work face: the lodge's covered lime and stone-wetting troughs, under
    // their own lids, on the yard side.
    for (index, offset) in [-1.1_f32, 1.1].into_iter().enumerate() {
        let centre = base + Vec3::new(3.6, 0.0, offset * 1.6);
        trough(
            commands,
            meshes,
            materials,
            collision,
            centre,
            Vec2::new(1.0, 2.6),
            0.0,
            if index == 0 {
                "Lodge Well: the lime-slaking trough"
            } else {
                "Lodge Well: the trough kept clear for fire"
            },
        );
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            centre + Vec3::Y * 0.72,
            Vec3::new(1.1, 0.08, 2.7),
            "Lodge trough lid",
        );
    }
    ambience(commands, base, "well_trough", 24.0, "Lodge Well");
}

/// Slate Cistern: long slate catch roofs feed a grated stone settling box; a
/// hinged board sends the first dirty run of a new rain to the street drain, and
/// once the roofs have washed, clean flow enters the vaulted cistern. Part of it
/// is a fire reserve, sealed with cord and the ward hand's impressed clay.
fn slate_cistern(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    cistern_vault(
        commands,
        meshes,
        materials,
        collision,
        base,
        Vec2::new(5.0, 4.0),
        0.0,
        "Slate Cistern",
    );
    catch_roof(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, -3.6),
        Vec2::new(6.4, 3.2),
        &materials.slate,
        "Slate Cistern",
    );
    settling_box(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, -1.9),
        "Slate Cistern",
    );
    drain(
        commands,
        meshes,
        materials,
        base + Vec3::new(-3.6, 0.0, -1.9),
        Vec2::new(2.6, 0.5),
        0.0,
    );

    // The sealed fire reserve: a second, smaller hatch, bound and bearing the
    // ward hand's clay. Breaking that seal is lawful during a visible fire and
    // otherwise owes an account to the Common Bench.
    spawn_box_named(
        commands,
        meshes,
        &materials.timber,
        base + Vec3::new(1.7, 0.94, 1.3),
        Vec3::new(1.1, 0.12, 1.1),
        "Slate Cistern: the sealed fire reserve hatch",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        base + Vec3::new(1.7, 1.01, 1.3),
        Vec3::new(1.2, 0.05, 0.12),
        "Fire reserve seal",
    );
    ambience(commands, base, "cistern_drip", 20.0, "Slate Cistern");
}

/// Tenter Cistern: a broad tiled catch roof and several lawfully shared gutters
/// feed a small settling chamber that can be scrubbed, then the main draw
/// chamber. The overflow fills covered work troughs; used fulling, washing and
/// dye water leaves by controlled culverts and never returns to the clean
/// chamber.
fn tenter_cistern(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    cistern_vault(
        commands,
        meshes,
        materials,
        collision,
        base,
        Vec2::new(5.6, 4.2),
        0.0,
        "Tenter Cistern",
    );
    catch_roof(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, -3.8),
        Vec2::new(7.6, 3.6),
        &materials.terracotta,
        "Tenter Cistern",
    );
    settling_box(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(-1.4, 0.0, -2.0),
        "Tenter Cistern",
    );

    // The covered work troughs the overflow fills, and the culvert that carries
    // fulling and dye water away from the clean chamber.
    for offset in [-1.4_f32, 1.4] {
        trough(
            commands,
            meshes,
            materials,
            collision,
            base + Vec3::new(offset, 0.0, 3.1),
            Vec2::new(2.4, 1.0),
            0.0,
            "Tenter Cistern work trough",
        );
    }
    drain(
        commands,
        meshes,
        materials,
        base + Vec3::new(0.0, 0.0, 4.2),
        Vec2::new(6.0, 0.6),
        0.0,
    );
    ambience(commands, base, "cistern_drip", 22.0, "Tenter Cistern");
}

/// Reed Cistern: raised on a paved apron, deliberately plain, its low tiled
/// catch-shed feeding a sealed tank through grates and a settling box, with the
/// draw hatch above known flood marks. The ward trusts a tank it can inspect
/// more than a pretty basin on low ground — and it is roof-fed, raised and
/// sealed from the churchyard, whatever the outsiders' "grave water" means by it.
fn reed_cistern(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    let plinth = 0.55;
    spawn_box_named(
        commands,
        meshes,
        &materials.paving,
        base + Vec3::Y * plinth * 0.5,
        Vec3::new(9.0, plinth, 7.0),
        "Reed Cistern raised apron",
    );
    add_rotated_box_collider(collision, base, Vec2::new(9.0, 7.0), 0.0, plinth);

    let head = base + Vec3::Y * plinth;
    cistern_vault(
        commands,
        meshes,
        materials,
        collision,
        head,
        Vec2::new(5.0, 4.0),
        0.0,
        "Reed Cistern",
    );
    catch_roof(
        commands,
        meshes,
        materials,
        collision,
        head + Vec3::new(0.0, 0.0, -3.2),
        Vec2::new(6.2, 3.0),
        &materials.terracotta,
        "Reed Cistern",
    );
    settling_box(
        commands,
        meshes,
        materials,
        collision,
        head + Vec3::new(0.0, 0.0, -1.7),
        "Reed Cistern",
    );

    // The overflow trough the fish sellers may use, after household turns.
    trough(
        commands,
        meshes,
        materials,
        collision,
        base + Vec3::new(0.0, 0.0, 4.6),
        Vec2::new(3.0, 1.0),
        0.0,
        "Reed Cistern overflow trough",
    );
    ambience(commands, base, "cistern_drip", 22.0, "Reed Cistern");
}

/// Step Cistern: the domestic draw lobby is reached down three broad steps under
/// a pent roof; a second opening admits roof water through a settling bay, and a
/// third locked gate gives emergency access to the reserve. The keeper's three
/// keys are mundane and publicly accounted: clean draw, settling and drain, fire
/// reserve. Pail rims ring on the lowest step, and none of it proves a tunnel.
fn step_cistern(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
) {
    // The court is a terrace cut into the slope behind the Bellstand. The
    // terrace is what you stand on; the draw lobby is the hole in it, and the
    // three broad steps are how you get down to the water.
    const TERRACE: f32 = 0.66;
    let lobby = Vec2::new(2.8, 3.4);
    let terrace = Vec2::new(7.0, 6.0);

    // The terrace, laid as a ring of paving around the open lobby, so the lobby
    // is genuinely open to the water rather than a slab with a lid painted on.
    for (dx, dz, sx, sz) in [
        (
            0.0,
            -(terrace.y + lobby.y) * 0.25,
            terrace.x,
            (terrace.y - lobby.y) * 0.5,
        ),
        (
            0.0,
            (terrace.y + lobby.y) * 0.25,
            terrace.x,
            (terrace.y - lobby.y) * 0.5,
        ),
        (
            -(terrace.x + lobby.x) * 0.25,
            0.0,
            (terrace.x - lobby.x) * 0.5,
            lobby.y,
        ),
        (
            (terrace.x + lobby.x) * 0.25,
            0.0,
            (terrace.x - lobby.x) * 0.5,
            lobby.y,
        ),
    ] {
        spawn_box_named(
            commands,
            meshes,
            &materials.paving,
            base + Vec3::new(dx, TERRACE * 0.5, dz),
            Vec3::new(sx, TERRACE, sz),
            "Step Cistern terrace",
        );
        add_rotated_box_collider(
            collision,
            base + Vec3::new(dx, 0.0, dz),
            Vec2::new(sx, sz),
            0.0,
            TERRACE,
        );
    }

    // Three broad steps down from the terrace into the draw lobby. Pail rims
    // ring on the lowest one.
    for (index, top) in [0.50_f32, 0.34, 0.18].into_iter().enumerate() {
        let depth = 0.5;
        spawn_box_named(
            commands,
            meshes,
            &materials.limestone,
            base + Vec3::new(
                0.0,
                top * 0.5,
                -lobby.y * 0.5 + depth * (index as f32 + 0.5),
            ),
            Vec3::new(lobby.x, top, depth),
            "Step Cistern step",
        );
        add_rotated_box_collider(
            collision,
            base + Vec3::new(0.0, 0.0, -lobby.y * 0.5 + depth * (index as f32 + 0.5)),
            Vec2::new(lobby.x, depth),
            0.0,
            top,
        );
    }

    // The water at the foot of the steps: cool, and slightly stony from the tank
    // lining.
    spawn_box_named(
        commands,
        meshes,
        &materials.well_water,
        base + Vec3::new(0.0, WATER_SURFACE_Y, 0.75),
        Vec3::new(lobby.x - 0.1, 0.05, lobby.y - 1.6),
        "Step Cistern: the water in the draw lobby",
    );

    // The pent roof over the lobby, on four posts standing on the terrace.
    let eave = 2.7;
    for x in [-1.0_f32, 1.0] {
        for z in [-1.0_f32, 1.0] {
            let post = base
                + Vec3::new(
                    x * (lobby.x * 0.5 + 0.5),
                    TERRACE,
                    z * (lobby.y * 0.5 + 0.5),
                );
            spawn_box_named(
                commands,
                meshes,
                &materials.timber,
                post + Vec3::Y * eave * 0.5,
                Vec3::new(0.18, eave, 0.18),
                "Step Cistern roof post",
            );
            add_rotated_box_collider_at(
                collision,
                post + Vec3::Y * eave * 0.5,
                Vec3::new(0.18, eave, 0.18),
                0.0,
            );
        }
    }
    spawn_mesh_named(
        commands,
        &meshes.cube,
        &materials.slate,
        Transform::from_translation(base + Vec3::new(0.0, TERRACE + eave + 0.3, 0.0))
            .with_rotation(Quat::from_rotation_x(0.17))
            .with_scale(Vec3::new(lobby.x + 1.8, 0.14, lobby.y + 1.8)),
        "Step Cistern pent roof",
    );

    // The vault head behind the lobby carries the keeper's other two openings —
    // the settling bay and the locked reserve gate. Three keys, publicly
    // accounted: clean draw, settling and drain, fire reserve.
    spawn_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        base + Vec3::new(0.0, TERRACE + 0.85, terrace.y * 0.5 + 0.9),
        Vec3::new(4.6, 1.7, 1.8),
        "Step Cistern vault head",
    );
    add_rotated_box_collider(
        collision,
        base + Vec3::new(0.0, 0.0, terrace.y * 0.5 + 0.9),
        Vec2::new(4.6, 1.8),
        0.0,
        TERRACE + 1.7,
    );
    for (offset, name) in [
        (-1.2_f32, "Step Cistern: the settling-bay opening"),
        (1.2, "Step Cistern: the locked fire-reserve gate"),
    ] {
        spawn_box_named(
            commands,
            meshes,
            &materials.iron,
            base + Vec3::new(offset, TERRACE + 0.85, terrace.y * 0.5 - 0.04),
            Vec3::new(1.0, 1.0, 0.1),
            name,
        );
    }

    drain(
        commands,
        meshes,
        materials,
        base + Vec3::new(-terrace.x * 0.5 - 0.6, 0.0, 0.0),
        Vec2::new(0.5, 3.2),
        0.0,
    );
    ambience(commands, base, "cistern_drip", 20.0, "Step Cistern");
}

/// Seven Lofts: covered work tanks and roof-fed fire casks, because grain dust,
/// timber roofs, dry stores and hoists make delay dangerous. A compound supply,
/// not a public drinking source: a deep well cannot answer a roof fire if the
/// first hundred efforts are spent raising buckets.
fn fire_tanks(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    angle: f32,
) {
    let right = Quat::from_rotation_y(angle) * Vec3::X;
    let forward = Quat::from_rotation_y(angle) * Vec3::Z;

    // Two covered work tanks, lids on, kept full before any bell rings.
    for side in [-1.0_f32, 1.0] {
        let centre = base + right * side * (size.x * 0.25);
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.fieldstone,
            centre + Vec3::Y * 0.55,
            Vec3::new(size.x * 0.4, 1.1, 2.2),
            angle,
            "Seven Lofts covered work tank",
        );
        add_rotated_box_collider(collision, centre, Vec2::new(size.x * 0.4, 2.2), angle, 1.1);
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.timber,
            centre + Vec3::Y * 1.14,
            Vec3::new(size.x * 0.42, 0.1, 2.3),
            angle,
            "Work tank lid",
        );
    }

    // The roof-fed fire casks, under the loft downpipe.
    for index in 0..3 {
        let centre = base + right * ((index as f32 - 1.0) * 1.5) + forward * 2.2;
        spawn_cylinder(
            commands,
            meshes,
            &materials.dark_wood,
            centre + Vec3::Y * 0.55,
            0.5,
            1.1,
        );
        spawn_cylinder(
            commands,
            meshes,
            &materials.iron,
            centre + Vec3::Y * 0.95,
            0.53,
            0.07,
        );
        add_rotated_box_collider(collision, centre, Vec2::new(1.0, 1.0), 0.0, 1.1);
    }
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        base + forward * 2.2 + right * -1.5 + Vec3::Y * 2.4,
        Vec3::new(0.16, 3.6, 0.16),
        "Seven Lofts downpipe",
    );
    ambience(
        commands,
        base,
        "cistern_drip",
        18.0,
        "Seven Lofts fire tanks",
    );
}

// ------------------------------------------------------------------ the parts

/// A lined shaft with a hollow stone curb: the ring you can lean over, the dark
/// lining below it, and the water at the bottom. No personal vessel is ever
/// meant to be lowered into it — that is what the permanent public bucket is for.
#[allow(clippy::too_many_arguments)]
fn well_mouth(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    radius: f32,
    height: f32,
    stone: Curbstone,
    name: &str,
) {
    let curb = match stone {
        Curbstone::Dressed => &materials.limestone,
        Curbstone::Old => &materials.fieldstone,
        Curbstone::Repair => &materials.plaster,
    };
    spawn_mesh_named(
        commands,
        &meshes.curb_ring,
        curb,
        Transform::from_translation(base + Vec3::Y * height * 0.5)
            .with_scale(Vec3::new(radius, height, radius)),
        name.to_string(),
    );

    // The lining you see when you look in: dark, wet stone all the way down.
    let inner = radius * CURB_INNER_FRACTION;
    spawn_cylinder(
        commands,
        meshes,
        &materials.well_shaft,
        base + Vec3::Y * (height * 0.5 - 0.02),
        inner - 0.03,
        height - 0.02,
    );
    spawn_cylinder(
        commands,
        meshes,
        &materials.well_water,
        base + Vec3::Y * WATER_SURFACE_Y,
        inner - 0.08,
        0.04,
    );

    // The curb is solid to a walker: an octagon, not the whole fixture footprint.
    let octagon: Vec<[f32; 2]> = (0..8)
        .map(|corner| {
            let bearing = corner as f32 / 8.0 * TAU + TAU / 16.0;
            [
                base.x + bearing.cos() * radius * 1.04,
                base.z + bearing.sin() * radius * 1.04,
            ]
        })
        .collect();
    collision.add_convex_prism(&octagon, base.y, base.y + height);
}

/// The lifting gear over a mouth: two uprights, the barrel, the crank, the rope
/// or chain, and the permanent public bucket hanging in the shaft.
#[allow(clippy::too_many_arguments)]
fn windlass(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    radius: f32,
    curb_height: f32,
    lift: Lift,
    angle: f32,
    name: &str,
    mechanism: Option<AnimatedWell>,
) {
    let broad = lift == Lift::Chain;
    let axle_y = curb_height + 1.0;
    let right = Quat::from_rotation_y(angle) * Vec3::X;
    let reach = radius + 0.18;

    for side in [-1.0_f32, 1.0] {
        let post = base + right * side * reach;
        spawn_box_named(
            commands,
            meshes,
            &materials.timber,
            post + Vec3::Y * axle_y * 0.5,
            Vec3::new(0.2, axle_y, 0.2),
            format!("{name} windlass upright"),
        );
        add_rotated_box_collider(collision, post, Vec2::new(0.2, 0.2), angle, axle_y);
    }

    // The barrel lies across the mouth, so it is a cylinder turned on its side.
    spawn_windlass_part(
        commands,
        &meshes.cylinder,
        if broad {
            &materials.dark_wood
        } else {
            &materials.timber
        },
        Transform::from_translation(base + Vec3::Y * axle_y)
            .with_rotation(Quat::from_rotation_y(angle) * Quat::from_rotation_z(FRAC_PI_2))
            .with_scale(Vec3::new(
                if broad { 0.26 } else { 0.19 },
                reach * 2.0,
                if broad { 0.26 } else { 0.19 },
            )),
        format!("{name} windlass barrel"),
        mechanism,
        MechanismPart::Rotor,
        base + Vec3::Y * axle_y,
        right,
    );

    // The crank, and — on a chain windlass — the pawl that clicks against the
    // ratchet and gives the Weigh Ward its most recognisable noise.
    let crank = base + right * (reach + 0.28) + Vec3::Y * axle_y;
    spawn_windlass_part(
        commands,
        &meshes.cube,
        &materials.iron,
        Transform::from_translation(crank).with_scale(Vec3::new(0.36, 0.08, 0.08)),
        format!("{name} windlass crank"),
        mechanism,
        MechanismPart::Rotor,
        base + Vec3::Y * axle_y,
        right,
    );
    spawn_windlass_part(
        commands,
        &meshes.cube,
        &materials.dark_wood,
        Transform::from_translation(crank + Vec3::new(0.0, -0.3, 0.0))
            .with_scale(Vec3::new(0.08, 0.5, 0.08)),
        format!("{name} crank handle"),
        mechanism,
        MechanismPart::Rotor,
        base + Vec3::Y * axle_y,
        right,
    );
    if broad {
        spawn_box_named(
            commands,
            meshes,
            &materials.iron,
            base - right * (reach + 0.2) + Vec3::Y * (axle_y - 0.3),
            Vec3::new(0.08, 0.6, 0.1),
            format!("{name} windlass pawl"),
        );
    }

    // The rope or chain, and the bucket it holds over the water. An iron-bound
    // bucket for a chain; a plain coopered one for a rope.
    let drop = axle_y - curb_height - 0.35;
    spawn_windlass_part(
        commands,
        &meshes.cylinder,
        if broad {
            &materials.iron
        } else {
            &materials.timber
        },
        Transform::from_translation(base + Vec3::Y * (curb_height + 0.35 + drop * 0.5)).with_scale(
            Vec3::new(
                if broad { 0.05 } else { 0.035 },
                drop,
                if broad { 0.05 } else { 0.035 },
            ),
        ),
        format!(
            "{name} {}",
            if broad {
                "drawing chain"
            } else {
                "drawing rope"
            }
        ),
        mechanism,
        MechanismPart::Line,
        base + Vec3::Y * axle_y,
        right,
    );
    let bucket = base + Vec3::Y * (curb_height + 0.12);
    spawn_windlass_part(
        commands,
        &meshes.cylinder,
        &materials.timber,
        Transform::from_translation(bucket).with_scale(Vec3::new(0.26, 0.42, 0.26)),
        format!("{name} public bucket"),
        mechanism,
        MechanismPart::Bucket,
        base + Vec3::Y * axle_y,
        right,
    );
    spawn_windlass_part(
        commands,
        &meshes.cylinder,
        &materials.iron,
        Transform::from_translation(bucket + Vec3::Y * 0.17)
            .with_scale(Vec3::new(0.28, 0.05, 0.28)),
        format!("{name} public bucket iron band"),
        mechanism,
        MechanismPart::Bucket,
        base + Vec3::Y * axle_y,
        right,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_windlass_part(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    transform: Transform,
    name: String,
    mechanism: Option<AnimatedWell>,
    part: MechanismPart,
    pivot: Vec3,
    axis: Vec3,
) {
    let rest = transform;
    let mut entity = commands.spawn((
        Name::new(name),
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        transform,
    ));
    if let Some(well) = mechanism {
        entity.insert(WellMechanismPart {
            well,
            part,
            pivot,
            axis: axis.normalize_or(Vec3::X),
            rest,
        });
    }
}

/// The roof over a well: four posts and a steep pitch. The posts are solid, the
/// shelter under them is not — a queue has to be able to stand there.
#[allow(clippy::too_many_arguments)]
fn roof_on_posts(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    eave: f32,
    tiles: &Handle<StandardMaterial>,
    name: &str,
) {
    for x in [-1.0_f32, 1.0] {
        for z in [-1.0_f32, 1.0] {
            let post = base + Vec3::new(x * size.x * 0.5, 0.0, z * size.y * 0.5);
            spawn_box_named(
                commands,
                meshes,
                &materials.timber,
                post + Vec3::Y * eave * 0.5,
                Vec3::new(0.28, eave, 0.28),
                format!("{name} roof post"),
            );
            add_rotated_box_collider(collision, post, Vec2::new(0.28, 0.28), 0.0, eave);
        }
    }
    spawn_mesh_named(
        commands,
        &meshes.pyramid,
        tiles,
        Transform::from_translation(base + Vec3::Y * (eave + 0.9)).with_scale(Vec3::new(
            size.x * 0.86,
            1.8,
            size.y * 0.86,
        )),
        format!("{name} roof"),
    );
}

/// A trough: an open working supply for animals, fire, cooling or a named trade.
/// It holds real water, and nobody should claim that water remains fit for a
/// cooking pot after mouths, tools, leather or lime buckets have entered it.
#[allow(clippy::too_many_arguments)]
fn trough(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    angle: f32,
    name: &str,
) {
    let height = 0.68;
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.fieldstone,
        base + Vec3::Y * height * 0.5,
        Vec3::new(size.x, height, size.y),
        angle,
        name.to_string(),
    );
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.water,
        base + Vec3::Y * (height - 0.06),
        Vec3::new(size.x - 0.28, 0.05, size.y - 0.28),
        angle,
        format!("{name}: standing water"),
    );
    add_rotated_box_collider(collision, base, size, angle, height);
}

/// The paved apron and queue space around a source. It is walkable — the whole
/// point of an apron is that people and animals stand on it.
fn apron(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    base: Vec3,
    size: Vec2,
    angle: f32,
) {
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.paving,
        base + Vec3::Y * 0.03,
        Vec3::new(size.x, 0.06, size.y),
        angle,
        "Drain apron and queue space",
    );
}

/// A covered street drain. A drain carries used water away; it is not a supply,
/// and the distinction is obvious until someone is short of water.
fn drain(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    base: Vec3,
    size: Vec2,
    angle: f32,
) {
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.iron,
        base + Vec3::Y * 0.05,
        Vec3::new(size.x, 0.08, size.y),
        angle,
        "Covered street drain",
    );
}

/// The vaulted store behind a draw hatch, with the hatch standing open on the
/// dark water below it. Better examples have a leaf grate, a settling bay, and a
/// way to send the first dirty roof water to the drain.
#[allow(clippy::too_many_arguments)]
fn cistern_vault(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    angle: f32,
    name: &str,
) {
    let height = 0.9;
    let (sin, cos) = angle.sin_cos();

    // The tank's stone is a rim: four walls around the water, so the open hatch
    // shows the chamber rather than the top of a solid block.
    for (dx, dz, wx, wz) in [
        (0.0, -(size.y - 0.6) * 0.5, size.x, 0.6),
        (0.0, (size.y - 0.6) * 0.5, size.x, 0.6),
        (-(size.x - 0.6) * 0.5, 0.0, 0.6, size.y - 1.2),
        ((size.x - 0.6) * 0.5, 0.0, 0.6, size.y - 1.2),
    ] {
        let offset = Vec3::new(dx * cos - dz * sin, 0.0, dx * sin + dz * cos);
        spawn_rotated_box_named(
            commands,
            meshes,
            &materials.limestone,
            base + offset + Vec3::Y * height * 0.5,
            Vec3::new(wx, height, wz),
            angle,
            format!("{name}: the vault"),
        );
        add_rotated_box_collider_at(
            collision,
            base + offset + Vec3::Y * height * 0.5,
            Vec3::new(wx, height, wz),
            angle,
        );
    }

    // The stored water, seen through the open hatch.
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.well_water,
        base + Vec3::Y * (height - 0.18),
        Vec3::new(size.x - 1.2, 0.05, size.y - 1.2),
        angle,
        format!("{name}: the stored water"),
    );

    // The draw hatch, small and standing open on its hinge.
    let hatch = Vec3::new(-size.x * 0.18 * cos, 0.0, -size.x * 0.18 * sin);
    spawn_mesh_named(
        commands,
        &meshes.cube,
        &materials.timber,
        Transform::from_translation(base + hatch + Vec3::new(0.0, height + 0.55, 0.0))
            .with_rotation(Quat::from_rotation_y(angle) * Quat::from_rotation_x(-1.15))
            .with_scale(Vec3::new(1.4, 0.1, 1.3)),
        format!("{name}: the draw hatch"),
    );
    spawn_rotated_box_named(
        commands,
        meshes,
        &materials.iron,
        base + hatch + Vec3::Y * (height + 0.02),
        Vec3::new(1.5, 0.06, 0.1),
        angle,
        "Hatch hinge",
    );
}

/// The catch roof, its gutters and its downpipe: the reason a cistern has water
/// at all. A gutter may cross three property lines before it reaches the
/// settling box, which is why a disputed pipe can occupy the Civic Measure Court
/// longer than a disputed bucket.
#[allow(clippy::too_many_arguments)]
fn catch_roof(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    size: Vec2,
    tiles: &Handle<StandardMaterial>,
    name: &str,
) {
    let eave = 3.0;
    for x in [-1.0_f32, 1.0] {
        for z in [-1.0_f32, 1.0] {
            let post = base + Vec3::new(x * (size.x * 0.5 - 0.2), 0.0, z * (size.y * 0.5 - 0.2));
            spawn_box_named(
                commands,
                meshes,
                &materials.timber,
                post + Vec3::Y * eave * 0.5,
                Vec3::new(0.2, eave, 0.2),
                format!("{name} catch-roof post"),
            );
            add_rotated_box_collider(collision, post, Vec2::new(0.2, 0.2), 0.0, eave);
        }
    }

    // The roof falls toward the cistern, so its water runs to the settling box.
    spawn_mesh_named(
        commands,
        &meshes.cube,
        tiles,
        Transform::from_translation(base + Vec3::new(0.0, eave + 0.45, 0.0))
            .with_rotation(Quat::from_rotation_x(-0.22))
            .with_scale(Vec3::new(size.x, 0.16, size.y)),
        format!("{name}: the catch roof"),
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        base + Vec3::new(0.0, eave + 0.05, size.y * 0.5 - 0.1),
        Vec3::new(size.x, 0.16, 0.22),
        format!("{name}: the catch-roof gutter"),
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        base + Vec3::new(size.x * 0.5 - 0.3, (eave + 0.05) * 0.5, size.y * 0.5 - 0.1),
        Vec3::new(0.14, eave + 0.05, 0.14),
        format!("{name}: the downpipe"),
    );
}

/// The grated stone settling box, with the hinged board that sends the first
/// dirty run of a new rain to the street drain. Once the roofs have washed, the
/// keeper shifts the board and clean flow enters the cistern.
fn settling_box(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    collision: &mut CollisionWorld,
    base: Vec3,
    name: &str,
) {
    spawn_box_named(
        commands,
        meshes,
        &materials.limestone,
        base + Vec3::Y * 0.3,
        Vec3::new(1.5, 0.6, 1.1),
        format!("{name}: the settling box"),
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.iron,
        base + Vec3::Y * 0.62,
        Vec3::new(1.3, 0.06, 0.9),
        "Leaf grate",
    );
    spawn_box_named(
        commands,
        meshes,
        &materials.timber,
        base + Vec3::new(-0.85, 0.5, 0.0),
        Vec3::new(0.1, 0.5, 1.0),
        format!("{name}: the first-flush board"),
    );
    add_rotated_box_collider(collision, base, Vec2::new(1.5, 1.1), 0.0, 0.6);
}

/// Mark a source so the audio layer can put a loop on it.
fn ambience(
    commands: &mut Commands,
    base: Vec3,
    sound_id: &'static str,
    audible_distance: f32,
    name: &str,
) {
    commands.spawn((
        Name::new(format!("{name}: water ambience")),
        WaterAmbience {
            sound_id,
            audible_distance,
        },
        Transform::from_translation(base + Vec3::Y * 1.0),
    ));
}

fn ordinal(court: usize) -> &'static str {
    match court {
        1 => "first",
        2 => "second",
        _ => "third",
    }
}

#[cfg(test)]
mod mechanism_tests {
    use super::*;

    #[test]
    fn inactive_mechanism_returns_to_exact_rest_phase() {
        let mut state = WellAnimationState::default();
        assert_ne!(
            state.pose(AnimatedWell::Ford, 0.2, true, false),
            MechanismPose::default()
        );
        assert_eq!(
            state.pose(AnimatedWell::Ford, 0.2, false, false),
            MechanismPose::default()
        );

        let resumed = state.pose(AnimatedWell::Ford, 0.1, true, false);
        let fresh = WellAnimationState::default().pose(AnimatedWell::Ford, 0.1, true, false);
        assert_eq!(resumed, fresh);
    }

    #[test]
    fn three_curb_mouths_have_deterministic_distinct_phases() {
        let mut first = WellAnimationState::default();
        let poses = [
            first.pose(AnimatedWell::ThreeCurb(0), 0.2, true, false),
            first.pose(AnimatedWell::ThreeCurb(1), 0.2, true, false),
            first.pose(AnimatedWell::ThreeCurb(2), 0.2, true, false),
        ];
        assert_ne!(poses[0].spin, poses[1].spin);
        assert_ne!(poses[1].spin, poses[2].spin);

        let mut replay = WellAnimationState::default();
        assert_eq!(
            poses,
            [
                replay.pose(AnimatedWell::ThreeCurb(0), 0.2, true, false),
                replay.pose(AnimatedWell::ThreeCurb(1), 0.2, true, false),
                replay.pose(AnimatedWell::ThreeCurb(2), 0.2, true, false),
            ]
        );
    }

    #[test]
    fn three_curb_conflict_jerks_once_then_holds() {
        let mut state = WellAnimationState::default();
        for _ in 0..4 {
            let _ = state.pose(AnimatedWell::ThreeCurb(1), 0.1, true, false);
        }
        let moving = state.pose(AnimatedWell::ThreeCurb(1), 0.1, true, false);
        let jerked = state.pose(AnimatedWell::ThreeCurb(1), 0.1, true, true);
        let held = state.pose(AnimatedWell::ThreeCurb(1), 0.25, true, true);

        assert_ne!(jerked.spin, moving.spin);
        assert_eq!(jerked, held);
        assert_ne!(held.sway, 0.0);
    }
}
