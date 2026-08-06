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
use cathedral_sim::{actions::CHALK_REACH_M, marks::MarkKind};

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

/// How far beyond the nearest static hit a focused mark may still sit. The
/// glyph floats [`MARK_SURFACE_OFFSET_M`] off its own wall, so a square look
/// at a mark lands the wall hit millimetres *behind* it — but oblique views
/// wobble both distances, and without slack a mark mounted on the very
/// surface the ray struck would flicker. Kept well under any real wall's
/// thickness, so chalk across one can never bleed through.
const MARK_OCCLUSION_EPSILON_M: f32 = 0.05;

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

impl MarkFocus {
    /// The mark a hold could actually scrub: the id, but only within the sim's
    /// own [`CHALK_REACH_M`]. Reading carries to [`MARK_READ_RADIUS_M`], twice
    /// as far — the authoritative `scrub_mark` refuses past arm's length, so
    /// offering the hold out there would be promising a refusal.
    fn scrub_target(&self) -> Option<u64> {
        let mark_id = self.mark_id?;
        (f64::from(self.distance_m) <= CHALK_REACH_M).then_some(mark_id)
    }
}

/// How long the player must hold the key to finish a mark, in real seconds.
/// Chalking is a deliberate act, not a keypress: releasing early aborts with
/// nothing drawn and nothing scrubbed.
const CHALK_HOLD_SECONDS: f32 = 1.4;

/// The key held to draw or to scrub.
const CHALK_KEY: KeyCode = KeyCode::KeyC;

/// The key that steps to the next sign within reach. Only ever does anything
/// where more than one is legal — at a well, which takes both a tally and a
/// ward-sign, and never at a door, which takes only a cross.
const CHALK_CYCLE_KEY: KeyCode = KeyCode::KeyG;

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
    /// Chalk `kind` on the anchor `handle` names — the sim's own handle,
    /// carried through untouched, because the host has no places registry to
    /// resolve it against.
    Draw { kind: MarkKind, handle: String },
}

/// What the player's hand could chalk from where it is standing: a projection
/// of [`cathedral_sim::EngineMessage::ChalkStanding`] and nothing more.
///
/// Deliberately not computed here, though the host owns the geometry. Which
/// *wall* a mark lies on is a question only the host can answer; which *door*
/// is within arm's reach is one only the sim can, because a place entry never
/// crosses the seam. The two halves each keep their own.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct ChalkStanding {
    /// Whether the player is holding anything to write with.
    pub pen: bool,
    /// Everything within the sim's chalk reach, nearest first.
    pub anchors: Vec<ChalkableAnchor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChalkableAnchor {
    /// The sim's handle for it, handed straight back with the command.
    pub handle: String,
    /// How the HUD names it, unknown-people rule already applied sim-side.
    pub label: String,
    /// The signs still drawable here, never empty.
    pub kinds: Vec<MarkKind>,
}

impl ChalkStanding {
    /// The anchors' kinds laid end to end: what the cycle key steps through.
    fn option_count(&self) -> usize {
        self.anchors.iter().map(|anchor| anchor.kinds.len()).sum()
    }

    /// The `index`th (anchor, sign) pair, nearest anchor first. An iterator
    /// rather than a built `Vec` because this is read every frame and the
    /// answer is almost always "nothing within reach".
    fn option(&self, index: usize) -> Option<(&ChalkableAnchor, MarkKind)> {
        let mut index = index;
        for anchor in &self.anchors {
            match anchor.kinds.get(index) {
                Some(kind) => return Some((anchor, *kind)),
                None => index -= anchor.kinds.len(),
            }
        }
        None
    }
}

/// Which sign within reach the next hold would draw.
///
/// A free-running counter taken modulo the option count at every read, so a
/// list that changes size under it can never index out of bounds — and reset
/// whenever the sim republishes what is in reach, which makes the default at
/// every new door the nearest anchor's first sign rather than whatever was
/// picked two streets ago.
#[derive(Resource, Debug, Default)]
pub struct ChalkChoice {
    step: usize,
}

impl ChalkChoice {
    fn selected<'a>(&self, standing: &'a ChalkStanding) -> Option<(&'a ChalkableAnchor, MarkKind)> {
        let count = standing.option_count();
        if count == 0 {
            return None;
        }
        standing.option(self.step % count)
    }
}

/// The line the HUD shows about chalk: what this hold would do, or how far
/// through it is. Composed here rather than in `interaction.rs` because the
/// catalog's prose lives on this side.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct ChalkPrompt(pub String);

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
    // The nearest static obstruction along the crosshair, computed once per
    // frame exactly as actor targeting does (`smart_actors/targeting.rs`).
    // Chalk across a wall must neither read out nor scrub — the focus is
    // where the hold's intent comes from, so this one gate covers both the
    // information and the sleeve.
    let wall = collision.nearest_ray_hit(origin, forward, MARK_READ_RADIUS_M);
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
        if wall.is_some_and(|wall| wall + MARK_OCCLUSION_EPSILON_M < along) {
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
    let faint = spec.faint_at_pct(mark.strength_pct);
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

/// Step to the next sign within reach, and start again from the nearest one
/// whenever the sim says what is in reach has changed.
pub(super) fn cycle_chalk_kind(
    keyboard: Res<ButtonInput<KeyCode>>,
    standing: Res<ChalkStanding>,
    mut choice: ResMut<ChalkChoice>,
) {
    if standing.is_changed() {
        choice.step = 0;
    }
    if keyboard.just_pressed(CHALK_CYCLE_KEY) && standing.option_count() > 1 {
        choice.step = choice.step.wrapping_add(1);
    }
}

/// What a hold begun right now would do.
///
/// Scrubbing wins over drawing whenever the crosshair is actually resting on
/// chalk: standing at a marked door with a pen, the thing you are pointing at
/// is the more specific answer, and a door already carrying its only legal sign
/// offers no draw anyway.
fn intent_now(
    focus: &MarkFocus,
    standing: &ChalkStanding,
    choice: &ChalkChoice,
) -> Option<ChalkIntent> {
    if let Some(mark_id) = focus.scrub_target() {
        return Some(ChalkIntent::Scrub(mark_id));
    }
    if !standing.pen {
        return None;
    }
    let (anchor, kind) = choice.selected(standing)?;
    Some(ChalkIntent::Draw {
        kind,
        handle: anchor.handle.clone(),
    })
}

/// The press-and-hold that draws a mark, or scrubs one.
///
/// Copied wholesale from the custody strain meter (`smart_actors/custody.rs`),
/// which is the only accumulator-with-a-latch in the tree: `+dt/fill` while
/// held, a hard reset when not, and one command at completion. The *intent* is
/// captured when the hold begins, so drifting the crosshair off the mark
/// half-way through — or stepping out of reach of the door — cancels rather
/// than quietly doing something else.
pub(super) fn chalk_hold(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    focus: Res<MarkFocus>,
    standing: Res<ChalkStanding>,
    choice: Res<ChalkChoice>,
    mut hold: ResMut<ChalkHold>,
    handle: Option<Res<crate::smart_actors::bridge::BridgeHandle>>,
) {
    let held = keyboard.pressed(CHALK_KEY);
    if !held {
        if hold.progress != 0.0 || hold.intent.is_some() {
            // Released early: nothing drawn, nothing scrubbed.
            hold.progress = 0.0;
            hold.intent = None;
        }
        return;
    }
    if hold.intent.is_none() {
        let Some(intent) = intent_now(&focus, &standing, &choice) else {
            return;
        };
        hold.intent = Some(intent);
        hold.progress = 0.0;
    }
    hold.progress = (hold.progress + time.delta_secs() / CHALK_HOLD_SECONDS).min(1.0);
    if hold.progress < 1.0 {
        return;
    }
    let intent = hold.intent.take();
    hold.progress = 0.0;
    let (Some(handle), Some(intent)) = (handle, intent) else {
        return;
    };
    // Both go back to the sim as commands and are refused there if the world
    // moved under the hold — the pen pocketed, the mark already gone. Nothing
    // about a mark is decided on this side.
    let command = match intent {
        ChalkIntent::Scrub(mark_id) => {
            crate::smart_actors::bridge::BridgeCommand::PlayerScrubMark { mark_id }
        }
        ChalkIntent::Draw { kind, handle } => {
            crate::smart_actors::bridge::BridgeCommand::PlayerDrawMark {
                kind,
                anchor: handle,
            }
        }
    };
    if let Err(error) = handle.try_send(command) {
        debug!("[marks] chalk not sent: {error}");
    }
}

/// Compose the chalk line the HUD shows, under whatever the crosshair is
/// already reading out.
pub(super) fn update_chalk_prompt(
    catalog: Res<MarkCatalogRes>,
    focus: Res<MarkFocus>,
    standing: Res<ChalkStanding>,
    choice: Res<ChalkChoice>,
    hold: Res<ChalkHold>,
    mut prompt: ResMut<ChalkPrompt>,
) {
    let next = chalk_prompt_line(&catalog.0, &focus, &standing, &choice, &hold);
    if prompt.0 != next {
        prompt.0 = next;
    }
}

fn chalk_prompt_line(
    catalog: &cathedral_sim::marks::MarkCatalog,
    focus: &MarkFocus,
    standing: &ChalkStanding,
    choice: &ChalkChoice,
    hold: &ChalkHold,
) -> String {
    // The same ten-cell meter the custody strain bar draws, because this is the
    // same gesture: a key held down until something happens.
    let meter = || {
        let filled = (hold.progress.clamp(0.0, 1.0) * 10.0).round() as usize;
        format!("[{}{}]", "#".repeat(filled), "-".repeat(10 - filled))
    };
    // A sign's prose is the catalog's, never spelled here — the same rule the
    // read-line follows.
    let sign = |kind: MarkKind| {
        catalog
            .spec(kind)
            .map_or_else(|| kind.as_str().to_string(), |spec| spec.label.clone())
    };
    match &hold.intent {
        Some(ChalkIntent::Scrub(_)) => return format!("Scrubbing  {}", meter()),
        Some(ChalkIntent::Draw { kind, .. }) => {
            return format!("Chalking {}  {}", sign(*kind), meter());
        }
        None => {}
    }
    if focus.scrub_target().is_some() {
        return "Hold C to scrub it off".to_string();
    }
    let Some((anchor, kind)) = choice.selected(standing) else {
        return String::new();
    };
    if !standing.pen {
        return format!("{} — nothing in hand to chalk with", anchor.label);
    }
    let another = if standing.option_count() > 1 {
        "    G for another sign"
    } else {
        ""
    };
    format!(
        "Hold C to chalk {} on {}{another}",
        sign(kind),
        anchor.label
    )
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

    /// Reading carries to [`MARK_READ_RADIUS_M`]; the sleeve only to the sim's
    /// [`CHALK_REACH_M`]. In between, the HUD must still read the mark out but
    /// never offer a hold the authoritative `scrub_mark` is bound to refuse.
    #[test]
    fn a_mark_read_from_beyond_arms_reach_offers_no_scrub() {
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
        // The glyph sits on the façade at z≈2.0; stand back off it by `d`.
        let from = |d: f32| {
            let camera = GlobalTransform::from(
                Transform::from_xyz(0.0, 0.91 + MARK_HEIGHT_M, 2.0 - d)
                    .looking_to(Vec3::Z, Vec3::Y),
            );
            compute_mark_focus(&catalog, Some(&mirror), &collision, Some(&camera))
        };
        let standing = ChalkStanding::default();
        let choice = ChalkChoice::default();
        let idle = ChalkHold::default();

        // Within arm's reach: read it out, and offer the hold.
        let near = from(1.9);
        assert!(near.read_line.is_some());
        assert_eq!(
            intent_now(&near, &standing, &choice),
            Some(ChalkIntent::Scrub(1))
        );
        assert_eq!(
            chalk_prompt_line(&catalog, &near, &standing, &choice, &idle),
            "Hold C to scrub it off"
        );

        // Readable but out of reach: the meaning, and no hold — completing it
        // could only ever end in an out-of-range refusal.
        let mid = from(3.0);
        assert!(mid.read_line.is_some(), "reading carries past arm's reach");
        assert_eq!(intent_now(&mid, &standing, &choice), None);
        assert_eq!(
            chalk_prompt_line(&catalog, &mid, &standing, &choice, &idle),
            ""
        );

        // Past reading range: nothing at all.
        assert_eq!(from(4.1), MarkFocus::default());
    }

    /// Chalk across a wall is not chalk you are looking at: a mark aligned
    /// behind an intervening collider must neither read out nor offer the
    /// scrub hold, while the same geometry with the blocker gone does both.
    /// The unblocked half also pins the epsilon: the aim ray strikes the
    /// mark's *own* façade only [`MARK_SURFACE_OFFSET_M`] behind the glyph,
    /// and a mark mounted on the hit surface itself must stay actionable.
    #[test]
    fn a_wall_between_the_crosshair_and_a_mark_blocks_read_and_scrub() {
        let catalog = MarkCatalog::default();
        let mut mirror = WorldMirror::default();
        mirror.debug_set_marks(vec![mark(
            MarkKind::ChalkCross,
            Vec3::new(0.0, 0.91, 0.0),
            100,
            1,
        )]);
        // The mark's own façade: the glyph lands on it at z≈1.994.
        let facade = |collision: &mut CollisionWorld| {
            collision.add_box(Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 4.0, 2.5));
        };
        // Within arm's reach, aimed square at the glyph — from a step aside,
        // so the sight line crosses (0.5, ·, ≈1.0) where the screen will sit.
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.8, 0.91 + MARK_HEIGHT_M, 0.4).looking_at(
                Vec3::new(0.0, 0.91 + MARK_HEIGHT_M, 2.0 - MARK_SURFACE_OFFSET_M),
                Vec3::Y,
            ),
        );
        let standing = ChalkStanding::default();
        let choice = ChalkChoice::default();

        // A thin screen between camera and mark. Its footprint dodges all
        // eight of `place`'s probe azimuths, so the mark still belongs to its
        // own façade and the screen only *occludes*.
        let mut blocked = CollisionWorld::default();
        facade(&mut blocked);
        blocked.add_box(Vec3::new(0.3, 0.0, 0.95), Vec3::new(0.7, 4.0, 1.05));
        let focus = compute_mark_focus(&catalog, Some(&mirror), &blocked, Some(&camera));
        assert_eq!(focus, MarkFocus::default(), "chalk across a wall is silent");
        assert_eq!(intent_now(&focus, &standing, &choice), None);

        // The same geometry with the screen gone: the read line, and the hold.
        let mut open = CollisionWorld::default();
        facade(&mut open);
        let focus = compute_mark_focus(&catalog, Some(&mirror), &open, Some(&camera));
        assert!(focus.read_line.is_some(), "unblocked, the mark reads");
        assert_eq!(
            intent_now(&focus, &standing, &choice),
            Some(ChalkIntent::Scrub(1))
        );
    }

    // ----------------------------------------------------------- the hand

    fn anchor(handle: &str, label: &str, kinds: &[MarkKind]) -> ChalkableAnchor {
        ChalkableAnchor {
            handle: handle.to_string(),
            label: label.to_string(),
            kinds: kinds.to_vec(),
        }
    }

    fn at_a_well() -> ChalkStanding {
        ChalkStanding {
            pen: true,
            anchors: vec![
                anchor(
                    "Chain Well",
                    "Chain Well",
                    &[MarkKind::WellTally, MarkKind::WardSign],
                ),
                anchor("k0fb1", "Ilse's door", &[MarkKind::ChalkCross]),
            ],
        }
    }

    fn focused_on_a_mark() -> MarkFocus {
        MarkFocus {
            read_line: Some("a chalk cross — they owe.".into()),
            mark_id: Some(7),
            distance_m: 1.0,
        }
    }

    /// The cycle key walks every sign of every anchor in reach, nearest anchor
    /// first, and wraps — a free-running counter taken modulo the count, so it
    /// can never index past a list that shrank under it.
    #[test]
    fn the_cycle_key_steps_through_every_sign_within_reach() {
        let standing = at_a_well();
        let picked: Vec<_> = (0..5)
            .map(|step| {
                let choice = ChalkChoice { step };
                let (anchor, kind) = choice.selected(&standing).expect("something is in reach");
                (anchor.handle.clone(), kind)
            })
            .collect();
        assert_eq!(
            picked,
            vec![
                ("Chain Well".to_string(), MarkKind::WellTally),
                ("Chain Well".to_string(), MarkKind::WardSign),
                ("k0fb1".to_string(), MarkKind::ChalkCross),
                ("Chain Well".to_string(), MarkKind::WellTally),
                ("Chain Well".to_string(), MarkKind::WardSign),
            ]
        );
        assert_eq!(
            ChalkChoice { step: 3 }.selected(&ChalkStanding::default()),
            None,
            "nothing in reach picks nothing, whatever the counter says"
        );
    }

    /// Scrubbing wins over drawing whenever the crosshair is actually resting
    /// on chalk: the thing you are pointing at is the more specific answer.
    #[test]
    fn what_a_hold_would_do_depends_on_the_crosshair_and_the_pen() {
        let standing = at_a_well();
        let choice = ChalkChoice::default();
        assert_eq!(
            intent_now(&focused_on_a_mark(), &standing, &choice),
            Some(ChalkIntent::Scrub(7))
        );
        assert_eq!(
            intent_now(&MarkFocus::default(), &standing, &choice),
            Some(ChalkIntent::Draw {
                kind: MarkKind::WellTally,
                handle: "Chain Well".into(),
            })
        );

        // No pen is no drawing — but it is still no obstacle to scrubbing, which
        // takes nothing but a wet sleeve.
        let empty_handed = ChalkStanding {
            pen: false,
            ..at_a_well()
        };
        assert_eq!(
            intent_now(&MarkFocus::default(), &empty_handed, &choice),
            None
        );
        assert_eq!(
            intent_now(&focused_on_a_mark(), &empty_handed, &choice),
            Some(ChalkIntent::Scrub(7))
        );
        assert_eq!(
            intent_now(&MarkFocus::default(), &ChalkStanding::default(), &choice),
            None,
            "a blank wall is not chalkable — §2.2"
        );
    }

    /// The line the player actually reads. Every sign's prose comes out of the
    /// catalog, never spelled here.
    #[test]
    fn the_prompt_says_what_the_hold_would_do() {
        let catalog = MarkCatalog::default();
        let standing = at_a_well();
        let idle = ChalkHold::default();

        let line = chalk_prompt_line(
            &catalog,
            &MarkFocus::default(),
            &standing,
            &ChalkChoice::default(),
            &idle,
        );
        assert!(line.starts_with("Hold C to chalk"), "unexpected: {line}");
        assert!(
            line.contains("tally strokes"),
            "the catalog's own words: {line}"
        );
        assert!(line.contains("Chain Well"), "and what it goes on: {line}");
        assert!(
            line.contains('G'),
            "two signs are legal here, so say how to reach the other: {line}"
        );

        // At a door only the cross is legal, so there is nothing to cycle to.
        let at_a_door = ChalkStanding {
            pen: true,
            anchors: vec![anchor("k0fb1", "Ilse's door", &[MarkKind::ChalkCross])],
        };
        let line = chalk_prompt_line(
            &catalog,
            &MarkFocus::default(),
            &at_a_door,
            &ChalkChoice::default(),
            &idle,
        );
        assert!(line.contains("Ilse's door"), "unexpected: {line}");
        assert!(
            !line.contains('G'),
            "one sign, no picker to advertise: {line}"
        );

        // The crosshair on chalk offers the sleeve instead.
        assert_eq!(
            chalk_prompt_line(
                &catalog,
                &focused_on_a_mark(),
                &ChalkStanding::default(),
                &ChalkChoice::default(),
                &idle
            ),
            "Hold C to scrub it off"
        );

        // Mid-hold it becomes a meter, and says which of the two acts is under
        // way — the custody strain bar's shape, because it is the same gesture.
        let halfway = ChalkHold {
            progress: 0.5,
            intent: Some(ChalkIntent::Draw {
                kind: MarkKind::WardSign,
                handle: "Chain Well".into(),
            }),
        };
        let line = chalk_prompt_line(
            &catalog,
            &MarkFocus::default(),
            &standing,
            &ChalkChoice::default(),
            &halfway,
        );
        assert_eq!(line, "Chalking a ward-sign  [#####-----]");

        // …and nothing in reach with nothing under the crosshair says nothing.
        assert_eq!(
            chalk_prompt_line(
                &catalog,
                &MarkFocus::default(),
                &ChalkStanding::default(),
                &ChalkChoice::default(),
                &idle
            ),
            ""
        );
    }

    /// A door within reach and no pen is worth saying out loud: the player is
    /// otherwise left holding a key that does nothing, with no reason given.
    #[test]
    fn an_empty_hand_is_told_why_nothing_happens() {
        let standing = ChalkStanding {
            pen: false,
            anchors: vec![anchor("k0fb1", "Ilse's door", &[MarkKind::ChalkCross])],
        };
        let line = chalk_prompt_line(
            &MarkCatalog::default(),
            &MarkFocus::default(),
            &standing,
            &ChalkChoice::default(),
            &ChalkHold::default(),
        );
        assert_eq!(line, "Ilse's door — nothing in hand to chalk with");
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

    /// The wall flips to the faint label exactly at the catalog's boundary on
    /// the *published* percent — the same `faint_at_pct` the sim's rule
    /// readers judge by, so the wall and the prompt agree about a mark that
    /// quantized right onto the threshold.
    #[test]
    fn the_wall_label_flips_at_the_published_boundary() {
        let catalog = MarkCatalog::default();
        let mut collision = CollisionWorld::default();
        collision.add_box(Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 4.0, 2.5));
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.91 + MARK_HEIGHT_M, 0.0).looking_to(Vec3::Z, Vec3::Y),
        );
        // 35% is what a raw strength of 0.345..0.35 publishes; the sim calls
        // all of it binding, so the wall must read it fresh — and half-washed
        // one percent below.
        for (pct, half_washed) in [(35, false), (34, true)] {
            let mut mirror = WorldMirror::default();
            mirror.debug_set_marks(vec![mark(
                MarkKind::ChalkCross,
                Vec3::new(0.0, 0.91, 0.0),
                pct,
                1,
            )]);
            let line = compute_mark_focus(&catalog, Some(&mirror), &collision, Some(&camera))
                .read_line
                .expect("still visible");
            assert_eq!(
                line.contains("half-washed"),
                half_washed,
                "at {pct}%: {line}"
            );
        }
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

    /// The picker steps on the key — and starts again from the nearest sign
    /// whenever the sim republishes what is in reach, so walking up to a new
    /// door always defaults to the obvious thing rather than to whatever was
    /// chosen two streets ago.
    #[test]
    fn the_picker_steps_on_the_key_and_resets_when_the_reach_moves() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ChalkChoice>()
            .insert_resource(ChalkStanding {
                pen: true,
                anchors: vec![ChalkableAnchor {
                    handle: "Chain Well".into(),
                    label: "Chain Well".into(),
                    kinds: vec![MarkKind::WellTally, MarkKind::WardSign],
                }],
            })
            .add_systems(Update, cycle_chalk_kind);
        // The first update sees the freshly inserted standing as changed.
        app.update();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(CHALK_CYCLE_KEY);
        app.update();
        assert_eq!(app.world().resource::<ChalkChoice>().step, 1);

        // Held rather than pressed again: one step per press.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        assert_eq!(app.world().resource::<ChalkChoice>().step, 1);

        // A new door within reach starts the choice over.
        app.world_mut().resource_mut::<ChalkStanding>().pen = false;
        app.update();
        assert_eq!(app.world().resource::<ChalkChoice>().step, 0);
    }

    /// The accumulator's contract, now that it has two things it can be
    /// accumulating towards: the intent is latched when the hold *starts* and
    /// thrown away whole on an early release.
    #[test]
    fn releasing_early_draws_nothing() {
        let mut app = App::new();
        // No `TimePlugin`: it would rewrite `Time` from the real clock every
        // update, and this test needs a delta it chose.
        app.init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<MarkFocus>()
            .init_resource::<ChalkChoice>()
            .init_resource::<ChalkHold>()
            .insert_resource(ChalkStanding {
                pen: true,
                anchors: vec![ChalkableAnchor {
                    handle: "k0fb1".into(),
                    label: "Ilse's door".into(),
                    kinds: vec![MarkKind::ChalkCross],
                }],
            })
            .add_systems(Update, chalk_hold);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(CHALK_KEY);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(300));
        app.update();
        let hold = app.world().resource::<ChalkHold>();
        assert_eq!(
            hold.intent,
            Some(ChalkIntent::Draw {
                kind: MarkKind::ChalkCross,
                handle: "k0fb1".into(),
            }),
            "the act is decided when the hold begins"
        );
        assert!(
            hold.progress > 0.0 && hold.progress < 1.0,
            "and it is under way"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(CHALK_KEY);
        app.update();
        let hold = app.world().resource::<ChalkHold>();
        assert_eq!(hold.intent, None, "released early: nothing drawn");
        assert_eq!(hold.progress, 0.0);
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
