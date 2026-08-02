//! Chalk you can see (`features/implemented/chalking_the_walls.md` M3).
//!
//! The sim owns every mark and publishes a point, a kind, a strength and a
//! stroke count. It does **not** publish an orientation, and cannot: a
//! `PlaceEntry` is a walkable point and nothing more, so the sim has no idea
//! which way a door faces. Working that out is this module's whole job — it is
//! the half of the seam that owns the city's geometry.
//!
//! Two decisions worth stating, because both look like shortcuts and are not:
//!
//! * **A mark is drawn as geometry, not as a texture on a quad.** A cross is
//!   two crossed bars; a tally is one bar per stroke; a ward-sign is a chevron.
//!   That costs a handful of quads each, needs no art asset, and reads at a
//!   distance — where an alpha-masked glyph would be a grey smudge. It also
//!   means the stroke count is *legible*: four notches are four bars, and the
//!   prompt's "four strokes" and the wall agree.
//! * **A mark is laid on the surface it belongs to, never billboarded.** Chalk
//!   seen from an overhead bridge must not turn to face you. The wall is found
//!   by probing the collision world outward from the anchor; nothing found
//!   means no wall, and the mark lies flat on the paving instead, which is
//!   exactly right for a tally on a well's kerb.
//!
//! Like the vermin, the whole set is one entity with one batched mesh
//! ([`crate::mesh_batch`]) and `NotShadowCaster`. Unlike the vermin, marks are
//! *slow*: the batch is rebuilt only when the world revision moves, not every
//! frame.

use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};
use cathedral_sim::marks::MarkKind;

use crate::{
    controller::CollisionWorld,
    mesh_batch::{idle_batch_mesh, write_batch_mesh},
    smart_actors::model::{MarkSnapshot, WorldMirror},
};

/// How high up the wall a mark sits, above the walkable point the sim gave.
/// "A chalk cross at knee height" is the catalog's own words.
const MARK_HEIGHT_M: f32 = 0.55;

/// How wide a mark is on the wall.
const MARK_SIZE_M: f32 = 0.34;

/// How thick one chalked bar is.
const MARK_STROKE_M: f32 = 0.035;

/// How far off the surface the geometry sits, so it never z-fights the wall it
/// is drawn on.
const MARK_SURFACE_OFFSET_M: f32 = 0.006;

/// How far to look for a wall before giving up and lying flat on the paving.
const MARK_WALL_PROBE_M: f32 = 2.6;

/// Chalk white — a little warm, because it is chalk on limestone and not paint.
const CHALK: [f32; 3] = [0.94, 0.93, 0.88];

/// The eight directions a mark probes for a wall, in order. Cardinals first so
/// a mark on a square-on façade — nearly all of them — lands on the first four
/// tries and never sees a diagonal.
const PROBES: [Vec2; 8] = [
    Vec2::new(1.0, 0.0),
    Vec2::new(-1.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(0.0, -1.0),
    Vec2::new(0.707, 0.707),
    Vec2::new(-0.707, 0.707),
    Vec2::new(0.707, -0.707),
    Vec2::new(-0.707, -0.707),
];

/// How near the crosshair must come to a mark for the HUD to read it out.
const MARK_READ_RADIUS_M: f32 = 4.0;

/// How far off the aim ray a mark may sit and still be the one you are looking
/// at. Roughly the mark's own size, so you have to point at it rather than
/// past it.
const MARK_AIM_TOLERANCE_M: f32 = 0.4;

/// What the crosshair is resting on, if it is resting on chalk.
///
/// A separate focus from [`crate::smart_actors::targeting::ActorFocus`] on
/// purpose: that one queries entities carrying an `ActorId`, and a mark has
/// none. Giving one a fake id to reuse the existing pipeline would poison
/// `focus_hint`, the offer path, gaze and the actor sheet — the mark would
/// become a person everywhere except in the sim.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct MarkFocus {
    /// The line the HUD shows: the label, then what it means.
    pub read_line: Option<String>,
    /// Which mark, for the press-and-hold scrub.
    pub mark_id: Option<u64>,
    /// How far the player is standing from it.
    pub distance_m: f32,
}

/// How long the player must hold the key to finish a mark, in real seconds.
/// Chalking is a deliberate act, not a keypress: releasing early aborts with
/// nothing drawn and nothing scrubbed.
const CHALK_HOLD_SECONDS: f32 = 1.4;

/// The player's half-finished stroke.
#[derive(Resource, Debug, Default)]
pub struct ChalkHold {
    /// `0.0`..`1.0` — what the HUD's progress bar shows.
    pub progress: f32,
    /// What this hold will do when it completes, decided when it *starts* so
    /// that walking the crosshair off a mark mid-hold cannot silently change
    /// the act.
    pub intent: Option<ChalkIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChalkIntent {
    /// Scrub the mark the crosshair was resting on.
    Scrub(u64),
}

/// The embedded mark catalog, so the host can spell a label and a meaning
/// without either crossing the wire per mark.
#[derive(Resource)]
pub struct MarkCatalogRes(pub cathedral_sim::marks::MarkCatalog);

impl Default for MarkCatalogRes {
    fn default() -> Self {
        Self(cathedral_sim::marks::MarkCatalog::default())
    }
}

/// The whole city's chalk: one entity, one mesh, rebuilt when the sim says the
/// marks moved.
#[derive(Component, Default)]
pub(super) struct Marks {
    /// The revision the current batch was built from, so a frame that changed
    /// nothing costs one integer comparison.
    built_revision: Option<u64>,
    /// Whether the batch has *ever* been built.
    ///
    /// Without this the guard compares `None` (never built) against `None` (no
    /// snapshot yet) and matches, so a world whose mirror carries no revision
    /// never draws its chalk at all — including every test harness, and the
    /// game itself in the window between spawn and the first snapshot.
    built: bool,
}

/// Where a mark ended up once the geometry was consulted: a point on a real
/// surface, and the two axes of that surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Placement {
    pub origin: Vec3,
    /// Along the wall, horizontal.
    pub right: Vec3,
    /// Up the wall — or "north" when the mark is lying flat.
    pub up: Vec3,
}

pub(super) fn spawn_marks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Chalk marks"),
        Mesh3d(meshes.add(idle_batch_mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            // Unlike the vermin batch, this one really does want blending: a
            // half-washed mark fades rather than vanishing, and the alpha is
            // the strength the sim published.
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.95,
            reflectance: 0.02,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        // The batch spans the city and is rewritten wholesale; a baked AABB
        // would lie.
        NoFrustumCulling,
        NotShadowCaster,
        Marks::default(),
    ));
}

/// Rebuild the batch when — and only when — the sim's revision has moved.
pub(super) fn sync_marks(
    mirror: Option<Res<WorldMirror>>,
    collision: Res<CollisionWorld>,
    mut marks: Query<(&mut Marks, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mirror) = mirror else {
        return;
    };
    let Ok((mut state, mesh_handle)) = marks.single_mut() else {
        return;
    };
    let revision = mirror.revision();
    if state.built && state.built_revision == revision {
        return;
    }
    state.built = true;
    state.built_revision = revision;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    for mark in mirror.marks() {
        let Some(placement) = place(&collision, mark) else {
            continue;
        };
        let alpha = f32::from(mark.strength_pct) / 100.0;
        push_glyph(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            mark,
            placement,
            [CHALK[0], CHALK[1], CHALK[2], alpha.clamp(0.0, 1.0)],
        );
    }

    write_batch_mesh(
        &mut meshes,
        &mesh_handle.0,
        positions,
        normals,
        uvs,
        colors,
        indices,
    );
}

/// Find the surface a mark belongs on.
///
/// Probes outward from the anchor at chalk height and takes the nearest wall.
/// With nothing within [`MARK_WALL_PROBE_M`] the mark lies flat on the paving —
/// which is not a fallback so much as the right answer for a tally on a kerb.
pub(super) fn place(collision: &CollisionWorld, mark: &MarkSnapshot) -> Option<Placement> {
    let anchor = Vec3::new(mark.point.x, mark.point.y, mark.point.z);
    if !anchor.is_finite() {
        return None;
    }
    let eye = anchor + Vec3::Y * MARK_HEIGHT_M;
    let nearest = PROBES
        .iter()
        .filter_map(|probe| {
            let direction = Vec3::new(probe.x, 0.0, probe.y);
            collision
                .nearest_ray_hit(eye, direction, MARK_WALL_PROBE_M)
                .map(|distance| (distance, direction))
        })
        .min_by(|(left, _), (right, _)| left.total_cmp(right));

    match nearest {
        Some((distance, direction)) => {
            // Back off the wall by a few millimetres along its own normal.
            let normal = -direction;
            let origin = eye + direction * distance + normal * MARK_SURFACE_OFFSET_M;
            // Any horizontal vector in the wall's plane will do for "right";
            // the glyphs are symmetric enough that which way round it runs does
            // not read as wrong.
            let right = Vec3::Y.cross(normal).normalize_or(Vec3::X);
            Some(Placement {
                origin,
                right,
                up: Vec3::Y,
            })
        }
        None => Some(Placement {
            origin: anchor + Vec3::Y * MARK_SURFACE_OFFSET_M,
            right: Vec3::X,
            up: Vec3::Z,
        }),
    }
}

/// Draw the mark's shape: a cross is two crossed bars, a tally is one bar per
/// stroke, a ward-sign is a chevron.
#[allow(clippy::too_many_arguments)]
fn push_glyph(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    mark: &MarkSnapshot,
    placement: Placement,
    color: [f32; 4],
) {
    let half = MARK_SIZE_M * 0.5;
    let mut bar = |from: Vec2, to: Vec2| {
        push_bar(
            positions, normals, uvs, colors, indices, placement, from, to, color,
        );
    };
    match mark.kind {
        MarkKind::ChalkCross => {
            bar(Vec2::new(-half, -half), Vec2::new(half, half));
            bar(Vec2::new(-half, half), Vec2::new(half, -half));
        }
        MarkKind::WellTally => {
            // One upright per notch, left to right, packed to the mark's width
            // however many there are — so a busy well is visibly busier.
            let count = mark.strokes.max(1) as f32;
            let gap = MARK_SIZE_M / count.max(1.0);
            for index in 0..mark.strokes.max(1) {
                let x = -half + gap * (f32::from(index) + 0.5);
                bar(Vec2::new(x, -half), Vec2::new(x, half));
            }
        }
        MarkKind::WardSign => {
            // A chevron: come here.
            bar(Vec2::new(-half, -half), Vec2::new(0.0, half));
            bar(Vec2::new(0.0, half), Vec2::new(half, -half));
        }
    }
}

/// One chalked stroke, from `from` to `to` in the surface's own 2-D frame.
#[allow(clippy::too_many_arguments)]
fn push_bar(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[f32; 4]>,
    indices: &mut Vec<u32>,
    placement: Placement,
    from: Vec2,
    to: Vec2,
    color: [f32; 4],
) {
    let to_world =
        |point: Vec2| placement.origin + placement.right * point.x + placement.up * point.y;
    let along = (to - from).normalize_or(Vec2::X);
    // Thicken perpendicular to the stroke's own direction, so a diagonal bar is
    // as thick as an upright one rather than wider.
    let across = Vec2::new(-along.y, along.x) * (MARK_STROKE_M * 0.5);
    let corners = [
        to_world(from - across),
        to_world(to - across),
        to_world(to + across),
        to_world(from + across),
    ];
    let normal = (corners[1] - corners[0])
        .cross(corners[3] - corners[0])
        .normalize_or(Vec3::Y);
    let first = positions.len() as u32;
    positions.extend(corners.map(|corner| corner.to_array()));
    normals.extend([normal.to_array(); 4]);
    uvs.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    colors.extend([color; 4]);
    indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
}

/// Work out which mark, if any, the crosshair is resting on, and compose the
/// line the HUD shows for it.
///
/// Deliberately a point-to-ray test rather than a mesh raycast: a mark is a few
/// flat bars, and the question the HUD is answering is "are you looking at
/// this", not "did a ray strike this triangle".
pub(super) fn update_mark_focus(
    mut focus: ResMut<MarkFocus>,
    catalog: Res<MarkCatalogRes>,
    mirror: Option<Res<WorldMirror>>,
    collision: Res<CollisionWorld>,
    cameras: Query<&GlobalTransform, With<crate::controller::PlayerCamera>>,
) {
    let next = compute_mark_focus(
        &catalog.0,
        mirror.as_deref(),
        &collision,
        cameras.single().ok(),
    );
    if *focus != next {
        *focus = next;
    }
}

fn compute_mark_focus(
    catalog: &cathedral_sim::marks::MarkCatalog,
    mirror: Option<&WorldMirror>,
    collision: &CollisionWorld,
    camera: Option<&GlobalTransform>,
) -> MarkFocus {
    let (Some(mirror), Some(camera)) = (mirror, camera) else {
        return MarkFocus::default();
    };
    let origin = camera.translation();
    let forward = camera.forward().as_vec3();
    let mut best: Option<(f32, &MarkSnapshot, Vec3)> = None;
    for mark in mirror.marks() {
        let Some(placement) = place(collision, mark) else {
            continue;
        };
        let to_mark = placement.origin - origin;
        let along = to_mark.dot(forward);
        if along <= 0.0 || along > MARK_READ_RADIUS_M {
            continue;
        }
        let off_axis = (to_mark - forward * along).length();
        if off_axis > MARK_AIM_TOLERANCE_M {
            continue;
        }
        if best.is_none_or(|(best_along, _, _)| along < best_along) {
            best = Some((along, mark, placement.origin));
        }
    }
    let Some((distance_m, mark, _)) = best else {
        return MarkFocus::default();
    };
    // The label and the meaning both come out of the same catalog the sim
    // compiled in, so the wall and the sheet can never say different things.
    let Some(spec) = catalog.spec(mark.kind) else {
        return MarkFocus::default();
    };
    let faint = f64::from(mark.strength_pct) / 100.0 < spec.faint_below;
    let label = if faint {
        &spec.faint_label
    } else {
        &spec.label
    };
    let strokes = match mark.kind {
        MarkKind::WellTally => format!(" ({} of them)", mark.strokes.max(1)),
        _ => String::new(),
    };
    MarkFocus {
        read_line: Some(format!("{label}{strokes} — {}.", spec.meaning)),
        mark_id: Some(mark.id),
        distance_m,
    }
}

/// The press-and-hold that scrubs a mark.
///
/// Copied wholesale from the custody strain meter (`smart_actors/custody.rs`),
/// which is the only accumulator-with-a-latch in the tree: `+dt/fill` while
/// held, a hard reset when not, and one command at completion. The *intent* is
/// captured when the hold begins, so drifting the crosshair off the mark
/// half-way through cancels rather than quietly scrubbing a different one.
pub(super) fn chalk_hold(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    focus: Res<MarkFocus>,
    mut hold: ResMut<ChalkHold>,
    handle: Option<Res<crate::smart_actors::bridge::BridgeHandle>>,
) {
    let held = keyboard.pressed(KeyCode::KeyC);
    if !held {
        if hold.progress != 0.0 || hold.intent.is_some() {
            // Released early: nothing drawn, nothing scrubbed.
            hold.progress = 0.0;
            hold.intent = None;
        }
        return;
    }
    if hold.intent.is_none() {
        let Some(mark_id) = focus.mark_id else {
            return;
        };
        hold.intent = Some(ChalkIntent::Scrub(mark_id));
        hold.progress = 0.0;
    }
    hold.progress = (hold.progress + time.delta_secs() / CHALK_HOLD_SECONDS).min(1.0);
    if hold.progress < 1.0 {
        return;
    }
    let intent = hold.intent.take();
    hold.progress = 0.0;
    let (Some(handle), Some(ChalkIntent::Scrub(mark_id))) = (handle, intent) else {
        return;
    };
    if let Err(error) =
        handle.try_send(crate::smart_actors::bridge::BridgeCommand::PlayerScrubMark { mark_id })
    {
        debug!("[marks] scrub not sent: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cathedral_sim::marks::MarkCatalog;

    fn mark(kind: MarkKind, at: Vec3, strength_pct: u8, strokes: u8) -> MarkSnapshot {
        MarkSnapshot {
            id: 1,
            kind,
            point: crate::smart_actors::model::Position {
                x: at.x,
                y: at.y,
                z: at.z,
            },
            strength_pct,
            strokes,
        }
    }

    /// The whole reason this module exists: the sim publishes a walkable point
    /// and no orientation, so the host finds the wall.
    #[test]
    fn a_mark_lands_on_the_nearest_wall_and_faces_out_of_it() {
        let mut collision = CollisionWorld::default();
        // A façade one metre north of the anchor, running east-west.
        collision.add_box(Vec3::new(-5.0, 0.0, 1.0), Vec3::new(5.0, 4.0, 1.5));

        let placed = place(
            &collision,
            &mark(MarkKind::ChalkCross, Vec3::new(0.0, 0.91, 0.0), 100, 1),
        )
        .expect("a finite anchor always places");

        assert!(
            (placed.origin.z - 1.0).abs() < 0.05,
            "the mark should sit on the façade at z≈1.0, got {}",
            placed.origin.z
        );
        assert!(
            (placed.origin.y - (0.91 + MARK_HEIGHT_M)).abs() < 1e-4,
            "and at knee height above the walkable point, got {}",
            placed.origin.y
        );
        // The surface frame must be perpendicular and upright.
        assert!(placed.up.abs_diff_eq(Vec3::Y, 1e-4), "up is up on a wall");
        assert!(
            placed.right.dot(Vec3::Y).abs() < 1e-4,
            "right runs along the wall, not up it"
        );
    }

    /// No wall within reach is not a failure — it is a tally on a well's kerb.
    #[test]
    fn a_mark_with_no_wall_near_it_lies_flat_on_the_paving() {
        let collision = CollisionWorld::default();
        let placed = place(
            &collision,
            &mark(MarkKind::WellTally, Vec3::new(0.0, 0.91, 0.0), 100, 3),
        )
        .expect("places");
        assert!(
            (placed.origin.y - 0.91).abs() < 0.02,
            "flat on the ground, not floating at knee height: {}",
            placed.origin.y
        );
        assert!(
            placed.up.abs_diff_eq(Vec3::Z, 1e-4),
            "a flat mark's 'up' runs along the ground"
        );
    }

    #[test]
    fn a_non_finite_anchor_is_dropped_rather_than_drawn() {
        let collision = CollisionWorld::default();
        assert!(
            place(
                &collision,
                &mark(MarkKind::ChalkCross, Vec3::new(f32::NAN, 0.91, 0.0), 100, 1),
            )
            .is_none()
        );
    }

    /// A cross is two bars, a tally is one bar per notch — so the wall and the
    /// prompt's "four strokes" agree, and a busy well is visibly busier.
    #[test]
    fn the_glyph_geometry_counts_the_strokes() {
        let placement = Placement {
            origin: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
        };
        let quads = |kind: MarkKind, strokes: u8| -> usize {
            let (mut p, mut n, mut u, mut c, mut i) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
            push_glyph(
                &mut p,
                &mut n,
                &mut u,
                &mut c,
                &mut i,
                &mark(kind, Vec3::ZERO, 100, strokes),
                placement,
                [1.0, 1.0, 1.0, 1.0],
            );
            p.len() / 4
        };
        assert_eq!(quads(MarkKind::ChalkCross, 1), 2, "a cross is two bars");
        assert_eq!(quads(MarkKind::WardSign, 1), 2, "a ward-sign is a chevron");
        assert_eq!(quads(MarkKind::WellTally, 1), 1);
        assert_eq!(quads(MarkKind::WellTally, 4), 4, "four notches, four bars");
        assert_eq!(quads(MarkKind::WellTally, 12), 12);
        assert_eq!(
            quads(MarkKind::WellTally, 0),
            1,
            "a tally with no notches still draws something rather than nothing"
        );
    }

    /// Strength drives opacity, so a half-washed mark is visibly half-washed.
    #[test]
    fn strength_reaches_the_vertex_alpha() {
        let placement = Placement {
            origin: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
        };
        let (mut p, mut n, mut u, mut colors, mut i) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        push_glyph(
            &mut p,
            &mut n,
            &mut u,
            &mut colors,
            &mut i,
            &mark(MarkKind::ChalkCross, Vec3::ZERO, 40, 1),
            placement,
            [CHALK[0], CHALK[1], CHALK[2], 0.4],
        );
        assert!(!colors.is_empty());
        assert!(colors.iter().all(|color| (color[3] - 0.4).abs() < 1e-6));
    }

    /// M3's HUD criterion: aiming at a mark composes a line carrying the label
    /// and the meaning, both out of the same catalog the sim compiled in.
    #[test]
    fn aiming_at_a_mark_reads_out_its_meaning() {
        let catalog = MarkCatalog::default();
        let mut collision = CollisionWorld::default();
        collision.add_box(Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 4.0, 2.5));

        let mut mirror = WorldMirror::default();
        mirror.debug_set_marks(vec![mark(
            MarkKind::ChalkCross,
            Vec3::new(0.0, 0.91, 0.0),
            100,
            1,
        )]);

        // Standing back from the wall at eye height, looking straight at it.
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.91 + MARK_HEIGHT_M, 0.0).looking_to(Vec3::Z, Vec3::Y),
        );
        let focus = compute_mark_focus(&catalog, Some(&mirror), &collision, Some(&camera));

        let line = focus.read_line.expect("the crosshair is on the cross");
        assert!(
            line.contains("a chalk cross at knee height"),
            "the label is missing: {line}"
        );
        assert!(
            line.contains("this household owes and has not paid"),
            "the meaning is missing: {line}"
        );
        assert_eq!(focus.mark_id, Some(1));

        // …and looking the other way reads nothing at all.
        let away = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.91 + MARK_HEIGHT_M, 0.0).looking_to(-Vec3::Z, Vec3::Y),
        );
        assert!(
            compute_mark_focus(&catalog, Some(&mirror), &collision, Some(&away))
                .read_line
                .is_none(),
            "chalk behind you is not chalk you are looking at"
        );
    }

    /// A half-washed mark says so, out of the catalog's own `faint_label`.
    #[test]
    fn a_faint_mark_reads_as_half_washed() {
        let catalog = MarkCatalog::default();
        let mut collision = CollisionWorld::default();
        collision.add_box(Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 4.0, 2.5));
        let mut mirror = WorldMirror::default();
        mirror.debug_set_marks(vec![mark(
            MarkKind::ChalkCross,
            Vec3::new(0.0, 0.91, 0.0),
            10,
            1,
        )]);
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.91 + MARK_HEIGHT_M, 0.0).looking_to(Vec3::Z, Vec3::Y),
        );
        let line = compute_mark_focus(&catalog, Some(&mirror), &collision, Some(&camera))
            .read_line
            .expect("still visible");
        assert!(line.contains("half-washed"), "a faint mark says so: {line}");
    }
}

#[cfg(test)]
mod system_tests {
    use super::*;
    use crate::mesh_batch::IDLE_BATCH_VERTICES;

    fn app_with(marks: Vec<MarkSnapshot>, collision: CollisionWorld) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<MarkFocus>()
            .init_resource::<MarkCatalogRes>()
            .insert_resource(collision);
        let mut mirror = WorldMirror::default();
        mirror.debug_set_marks(marks);
        app.insert_resource(mirror);
        app.add_systems(Startup, spawn_marks)
            .add_systems(Update, sync_marks);
        app
    }

    fn batch_vertices(app: &App) -> usize {
        let handle = app
            .world()
            .iter_entities()
            .find_map(|entity| entity.get::<Mesh3d>().cloned())
            .expect("the marks batch entity exists");
        app.world()
            .resource::<Assets<Mesh>>()
            .get(&handle.0)
            .map(|mesh| mesh.count_vertices())
            .expect("the batch mesh exists")
    }

    /// An unchalked city parks on the idle triangle and costs nothing.
    #[test]
    fn a_bare_city_draws_no_chalk() {
        let mut app = app_with(Vec::new(), CollisionWorld::default());
        app.update();
        assert_eq!(batch_vertices(&app), IDLE_BATCH_VERTICES);
    }

    /// The system test the drive screenshots could not settle: a mark in the
    /// mirror really does become geometry in the batch.
    #[test]
    fn a_mark_in_the_mirror_becomes_geometry() {
        let mark = MarkSnapshot {
            id: 1,
            kind: MarkKind::ChalkCross,
            point: crate::smart_actors::model::Position {
                x: 0.0,
                y: 0.91,
                z: 0.0,
            },
            strength_pct: 100,
            strokes: 1,
        };
        let mut collision = CollisionWorld::default();
        collision.add_box(Vec3::new(-5.0, 0.0, 1.0), Vec3::new(5.0, 4.0, 1.5));
        let mut app = app_with(vec![mark], collision);
        app.update();
        // A cross is two bars, four vertices each.
        assert_eq!(batch_vertices(&app), 8, "two crossed bars reach the batch");
    }

    /// …and with no wall anywhere it still draws, lying flat on the paving.
    /// This is the case the well tally hits, and the one a naive "only draw on
    /// walls" placement would silently lose.
    #[test]
    fn a_mark_with_no_wall_still_reaches_the_batch() {
        let mark = MarkSnapshot {
            id: 1,
            kind: MarkKind::WellTally,
            point: crate::smart_actors::model::Position {
                x: 0.0,
                y: 0.91,
                z: 0.0,
            },
            strength_pct: 100,
            strokes: 4,
        };
        let mut app = app_with(vec![mark], CollisionWorld::default());
        app.update();
        assert_eq!(batch_vertices(&app), 16, "four notches, four quads");
    }

    /// The whole point of the revision gate: a frame that changed nothing must
    /// not rewrite the mesh.
    #[test]
    fn an_unchanged_revision_does_not_rebuild() {
        let mut app = app_with(Vec::new(), CollisionWorld::default());
        app.update();
        let built = app
            .world()
            .iter_entities()
            .find_map(|entity| entity.get::<Marks>().map(|marks| marks.built_revision));
        app.update();
        let again = app
            .world()
            .iter_entities()
            .find_map(|entity| entity.get::<Marks>().map(|marks| marks.built_revision));
        assert_eq!(built, again, "a quiet frame is a no-op");
    }
}
