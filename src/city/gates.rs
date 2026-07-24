//! Working leaves for the two great freight gates.
//!
//! The cadastral plan owns the gatehouse masonry, but masonry alone left two
//! conspicuous, permanently open holes in the wall.  This module supplies the
//! timber mechanisms and projects the authoritative office clock onto them.
//! Clock observations only *request* a state change; motion itself is measured
//! in real seconds so the generated mechanical recordings and the geometry
//! cannot drift apart when the simulation clock is accelerated.

use std::f32::consts::FRAC_PI_2;

use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};
use cathedral_sim::Office;

use crate::{controller::DynamicBarrier, smart_actors::WorldClockState, soundscape::SoundscapeCue};

const STONE_MIN_Z: f32 = 82.5;
const STONE_MAX_Z: f32 = 106.5;
const RIVER_MIN_Z: f32 = -111.0;
const RIVER_MAX_Z: f32 = -78.0;

const LEAF_HEIGHT: f32 = 7.2;
const LEAF_THICKNESS: f32 = 0.46;
const MEETING_GAP: f32 = 0.16;
/// Exact decoded duration of `snd_062_stone_gate_closing.mp3`.
const LEAF_SWING_SECONDS: f32 = 5.512;
/// Exact decoded duration of `snd_063_river_gate_bar_lift.mp3`.
const RIVER_BAR_LIFT_SECONDS: f32 = 3.527;
const RIVER_BAR_LOWER_SECONDS: f32 = 1.8;
// Just inside the eastern (city-facing) leaf surface, where the oak actually
// bears on the ledgers rather than hovering in front of them.
const RIVER_BAR_X: f32 = -352.92;
const RIVER_BAR_LOCKED_Y: f32 = 3.25;
const RIVER_BAR_RAISED_Y: f32 = 7.82;
const CLOSED_BARRIER_THRESHOLD: f32 = 0.015;
const CLOCK_BOUNDARY_EPSILON: f64 = 1.0e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateKind {
    Stone,
    River,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GateLeaf {
    gate: GateKind,
    open_yaw: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct RiverGateBar;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GateBarrier(GateKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatePosition {
    Open,
    Closed,
}

impl GatePosition {
    fn at_office(office: Office) -> Self {
        if matches!(office, Office::Snuffing | Office::Watch | Office::Kindling) {
            Self::Closed
        } else {
            Self::Open
        }
    }

    fn openness(self) -> f32 {
        match self {
            Self::Open => 1.0,
            Self::Closed => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ClockSample {
    day: i64,
    fraction: f64,
    office: Office,
}

impl ClockSample {
    fn total_days(self) -> Option<f64> {
        let total = self.day as f64 + self.fraction;
        (self.fraction.is_finite() && (0.0..1.0).contains(&self.fraction) && total.is_finite())
            .then_some(total)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduleAction {
    /// First valid clock observation: geometry snaps without making a sound.
    Initialize(GatePosition),
    /// A forward crossing of Dayspring or Snuffing.
    Transition(GatePosition),
    /// A rewind or inconsistent clock correction: reconcile silently.
    Reconcile(GatePosition),
}

#[derive(Debug, Default)]
struct GateSchedule {
    previous: Option<(f64, GatePosition)>,
}

impl GateSchedule {
    fn observe(&mut self, sample: Option<ClockSample>) -> Option<ScheduleAction> {
        let sample = sample?;
        let now = sample.total_days()?;
        let position = GatePosition::at_office(sample.office);
        let Some((previous, previous_position)) = self.previous.replace((now, position)) else {
            return Some(ScheduleAction::Initialize(position));
        };

        if now <= previous {
            return (position != previous_position).then_some(ScheduleAction::Reconcile(position));
        }

        let crossed = latest_gate_boundary(previous, now);
        match crossed {
            Some(boundary_position)
                if boundary_position == position && position != previous_position =>
            {
                Some(ScheduleAction::Transition(position))
            }
            Some(boundary_position) if boundary_position != position => {
                Some(ScheduleAction::Reconcile(position))
            }
            None if position != previous_position => Some(ScheduleAction::Reconcile(position)),
            _ => None,
        }
    }
}

/// Return only the most recent relevant boundary in `(previous, now]`.  This
/// catches an office skipped at high clock scale without replaying a backlog of
/// obsolete gate noises after a multi-day debug jump.
fn latest_gate_boundary(previous: f64, now: f64) -> Option<GatePosition> {
    [
        (Office::Dayspring.start_fraction(), GatePosition::Open),
        (Office::Snuffing.start_fraction(), GatePosition::Closed),
    ]
    .into_iter()
    .filter_map(|(fraction, position)| {
        // Addition of the absolute day can put an exact office fraction a few
        // ulps below itself after subtraction.  Bias only the day selection;
        // the open-left interval still prevents duplicate transitions.
        let boundary = (now - fraction + CLOCK_BOUNDARY_EPSILON).floor() + fraction;
        (boundary > previous + CLOCK_BOUNDARY_EPSILON && boundary <= now + CLOCK_BOUNDARY_EPSILON)
            .then_some((boundary, position))
    })
    .max_by(|(a, _), (b, _)| a.total_cmp(b))
    .map(|(_, position)| position)
}

#[derive(Debug, Clone, Copy)]
struct Motion {
    value: f32,
    start: f32,
    target: f32,
    elapsed: f32,
    duration: f32,
}

impl Motion {
    const fn at(value: f32) -> Self {
        Self {
            value,
            start: value,
            target: value,
            elapsed: 0.0,
            duration: 0.0,
        }
    }

    fn snap(&mut self, value: f32) {
        *self = Self::at(value);
    }

    fn start(&mut self, target: f32, full_duration: f32) {
        if (self.value - target).abs() <= f32::EPSILON {
            self.snap(target);
            return;
        }
        self.start = self.value;
        self.target = target;
        self.elapsed = 0.0;
        // Interruptions retain roughly the same angular / lifting speed.
        self.duration = full_duration * (self.target - self.start).abs();
    }

    /// Advance and return time left after this motion reached its stop.
    fn advance(&mut self, seconds: f32) -> f32 {
        if self.duration <= 0.0 || self.elapsed >= self.duration {
            self.value = self.target;
            return seconds.max(0.0);
        }
        let usable = seconds.max(0.0);
        let remaining = self.duration - self.elapsed;
        let consumed = usable.min(remaining);
        self.elapsed += consumed;
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        // Heavy leaves ease out of their stops; the recording's final impact
        // lands exactly when this interpolation reaches one.
        let eased = t * t * (3.0 - 2.0 * t);
        self.value = self.start + (self.target - self.start) * eased;
        if self.elapsed >= self.duration {
            self.value = self.target;
        }
        usable - consumed
    }

    fn is_at(&self, target: f32) -> bool {
        (self.value - target).abs() <= 1.0e-4 && self.elapsed >= self.duration
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateCue {
    StoneClosing,
    RiverBarLift,
}

#[derive(Resource, Debug)]
pub(super) struct GateRuntime {
    schedule: GateSchedule,
    initialized: bool,
    position: GatePosition,
    stone_leaves: Motion,
    river_leaves: Motion,
    river_bar: Motion,
}

impl Default for GateRuntime {
    fn default() -> Self {
        // Open is the fail-safe while the in-process sim starts: players can
        // never be trapped by a gate whose clock has not arrived.
        Self {
            schedule: GateSchedule::default(),
            initialized: false,
            position: GatePosition::Open,
            stone_leaves: Motion::at(1.0),
            river_leaves: Motion::at(1.0),
            river_bar: Motion::at(1.0),
        }
    }
}

impl GateRuntime {
    fn apply(&mut self, action: ScheduleAction) -> Option<GateCue> {
        match action {
            ScheduleAction::Initialize(position) | ScheduleAction::Reconcile(position) => {
                self.initialized = true;
                self.position = position;
                let openness = position.openness();
                self.stone_leaves.snap(openness);
                self.river_leaves.snap(openness);
                self.river_bar.snap(openness);
                None
            }
            ScheduleAction::Transition(position) if self.position == position => None,
            ScheduleAction::Transition(GatePosition::Closed) => {
                self.initialized = true;
                self.position = GatePosition::Closed;
                self.stone_leaves.start(0.0, LEAF_SWING_SECONDS);
                self.river_leaves.start(0.0, LEAF_SWING_SECONDS);
                // Leave the River beam safely overhead until its leaves meet.
                self.river_bar.start(1.0, RIVER_BAR_LIFT_SECONDS);
                Some(GateCue::StoneClosing)
            }
            ScheduleAction::Transition(GatePosition::Open) => {
                self.initialized = true;
                self.position = GatePosition::Open;
                self.stone_leaves.start(1.0, LEAF_SWING_SECONDS);
                // River's leaves remain shut until the recording and visible
                // beam lift have both completed.
                self.river_bar.start(1.0, RIVER_BAR_LIFT_SECONDS);
                Some(GateCue::RiverBarLift)
            }
        }
    }

    fn advance(&mut self, seconds: f32) {
        let _ = self.stone_leaves.advance(seconds);
        match self.position {
            GatePosition::Open => {
                let after_bar = self.river_bar.advance(seconds);
                if self.river_bar.is_at(1.0) {
                    if self.river_leaves.target != 1.0 {
                        self.river_leaves.start(1.0, LEAF_SWING_SECONDS);
                    }
                    let _ = self.river_leaves.advance(after_bar);
                }
            }
            GatePosition::Closed => {
                let after_leaves = self.river_leaves.advance(seconds);
                if self.river_leaves.is_at(0.0) {
                    if self.river_bar.target != 0.0 {
                        self.river_bar.start(0.0, RIVER_BAR_LOWER_SECONDS);
                    }
                    let _ = self.river_bar.advance(after_leaves);
                }
            }
        }
    }
}

pub(super) fn animate_gate_mechanisms(
    real_time: Res<Time<Real>>,
    clock: Option<Res<WorldClockState>>,
    mut runtime: ResMut<GateRuntime>,
    mut cues: MessageWriter<SoundscapeCue>,
    mut leaves: Query<(&GateLeaf, &mut Transform), Without<RiverGateBar>>,
    mut bars: Query<&mut Transform, (With<RiverGateBar>, Without<GateLeaf>)>,
    mut barriers: Query<(&GateBarrier, &mut DynamicBarrier)>,
) {
    let sample = clock.as_deref().and_then(|clock| {
        clock.present.then_some(ClockSample {
            day: clock.day,
            fraction: clock.fraction,
            office: clock.office,
        })
    });
    if let Some(action) = runtime.schedule.observe(sample) {
        match runtime.apply(action) {
            Some(GateCue::StoneClosing) => {
                cues.write(SoundscapeCue::StoneGateClosing);
            }
            Some(GateCue::RiverBarLift) => {
                cues.write(SoundscapeCue::RiverGateBarLift);
            }
            None => {}
        }
    }

    runtime.advance(real_time.delta_secs());

    for (leaf, mut transform) in &mut leaves {
        let openness = match leaf.gate {
            GateKind::Stone => runtime.stone_leaves.value,
            GateKind::River => runtime.river_leaves.value,
        };
        let rotation = Quat::from_rotation_y(leaf.open_yaw * openness);
        // Settled gates would otherwise re-flag their ~20-plank subtrees for
        // transform propagation every frame of the day.
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
    for mut transform in &mut bars {
        let y = RIVER_BAR_LOCKED_Y
            + (RIVER_BAR_RAISED_Y - RIVER_BAR_LOCKED_Y) * runtime.river_bar.value;
        if transform.translation.y != y {
            transform.translation.y = y;
        }
    }
    for (marker, mut barrier) in &mut barriers {
        let openness = match marker.0 {
            GateKind::Stone => runtime.stone_leaves.value,
            GateKind::River => runtime.river_leaves.value,
        };
        let should_block = runtime.initialized
            && runtime.position == GatePosition::Closed
            && openness <= CLOSED_BARRIER_THRESHOLD;
        if barrier.active != should_block {
            barrier.active = should_block;
        }
    }
}

pub(super) fn spawn_gate_mechanisms(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    cylinder: &Handle<Mesh>,
    timber: &Handle<StandardMaterial>,
    dark_wood: &Handle<StandardMaterial>,
    iron: &Handle<StandardMaterial>,
) {
    spawn_paired_leaves(
        commands,
        cube,
        cylinder,
        timber,
        dark_wood,
        iron,
        GateKind::Stone,
        346.5,
        STONE_MIN_Z,
        STONE_MAX_Z,
        -1.0,
    );
    spawn_paired_leaves(
        commands,
        cube,
        cylinder,
        timber,
        dark_wood,
        iron,
        GateKind::River,
        -353.5,
        RIVER_MIN_Z,
        RIVER_MAX_Z,
        1.0,
    );
    spawn_river_bar(commands, cube, timber, iron);

    for (gate, x, min_z, max_z) in [
        (GateKind::Stone, 346.5, STONE_MIN_Z, STONE_MAX_Z),
        (GateKind::River, -353.5, RIVER_MIN_Z, RIVER_MAX_Z),
    ] {
        commands.spawn((
            Name::new(match gate {
                GateKind::Stone => "Stone Gate dynamic barrier",
                GateKind::River => "River Gate dynamic barrier",
            }),
            GateBarrier(gate),
            DynamicBarrier {
                half_size: Vec3::new(0.52, LEAF_HEIGHT * 0.5, (max_z - min_z) * 0.5),
                active: false,
            },
            Transform::from_xyz(x, LEAF_HEIGHT * 0.5, (min_z + max_z) * 0.5),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_paired_leaves(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    cylinder: &Handle<Mesh>,
    timber: &Handle<StandardMaterial>,
    dark_wood: &Handle<StandardMaterial>,
    iron: &Handle<StandardMaterial>,
    gate: GateKind,
    x: f32,
    min_z: f32,
    max_z: f32,
    interior_x: f32,
) {
    let leaf_length = (max_z - min_z) * 0.5 - MEETING_GAP * 0.5;
    for (side, hinge_z, side_name) in [(1.0, min_z, "south"), (-1.0, max_z, "north")] {
        let open_yaw = interior_x * side * FRAC_PI_2;
        let gate_name = match gate {
            GateKind::Stone => "Stone Gate",
            GateKind::River => "River Gate",
        };
        commands
            .spawn((
                Name::new(format!("{gate_name} {side_name} leaf hinge")),
                GateLeaf { gate, open_yaw },
                // Spawn fail-safe open.  The first authoritative clock sample
                // snaps this silently to the correct office.
                Transform::from_xyz(x, 0.0, hinge_z).with_rotation(Quat::from_rotation_y(open_yaw)),
                Visibility::default(),
            ))
            .with_children(|leaf| {
                spawn_leaf_woodwork(
                    leaf,
                    cube,
                    cylinder,
                    timber,
                    dark_wood,
                    iron,
                    side,
                    leaf_length,
                );
            });
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_leaf_woodwork(
    leaf: &mut ChildSpawnerCommands,
    cube: &Handle<Mesh>,
    cylinder: &Handle<Mesh>,
    timber: &Handle<StandardMaterial>,
    dark_wood: &Handle<StandardMaterial>,
    iron: &Handle<StandardMaterial>,
    side: f32,
    length: f32,
) {
    let plank_count = (length / 1.15).ceil() as usize;
    let pitch = length / plank_count as f32;
    for plank in 0..plank_count {
        leaf.spawn((
            Name::new("Gate leaf vertical oak plank"),
            Mesh3d(cube.clone()),
            MeshMaterial3d(dark_wood.clone()),
            Transform::from_xyz(0.0, LEAF_HEIGHT * 0.5, side * (plank as f32 + 0.5) * pitch)
                .with_scale(Vec3::new(LEAF_THICKNESS, LEAF_HEIGHT, pitch - 0.025)),
        ));
    }

    let center_z = side * length * 0.5;
    for y in [0.75, LEAF_HEIGHT * 0.5, LEAF_HEIGHT - 0.75] {
        leaf.spawn((
            Name::new("Gate leaf oak ledger"),
            Mesh3d(cube.clone()),
            MeshMaterial3d(timber.clone()),
            Transform::from_xyz(0.29, y, center_z).with_scale(Vec3::new(0.22, 0.34, length - 0.35)),
        ));
        leaf.spawn((
            Name::new("Gate leaf iron strap"),
            Mesh3d(cube.clone()),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0.415, y, side * length * 0.28).with_scale(Vec3::new(
                0.055,
                0.14,
                length * 0.54,
            )),
        ));
    }

    let brace_length = ((length - 1.2).powi(2) + (LEAF_HEIGHT - 1.7).powi(2)).sqrt();
    let brace_angle = ((LEAF_HEIGHT - 1.7) / (length - 1.2)).atan();
    leaf.spawn((
        Name::new("Gate leaf diagonal oak brace"),
        Mesh3d(cube.clone()),
        MeshMaterial3d(timber.clone()),
        Transform::from_xyz(0.31, LEAF_HEIGHT * 0.5, center_z)
            .with_rotation(Quat::from_rotation_x(side * brace_angle))
            .with_scale(Vec3::new(0.24, 0.3, brace_length)),
    ));

    for y in [1.05, LEAF_HEIGHT - 1.05] {
        leaf.spawn((
            Name::new("Gate leaf iron hinge pin"),
            Mesh3d(cylinder.clone()),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0.0, y, side * 0.12).with_scale(Vec3::new(0.17, 0.58, 0.17)),
        ));
    }
}

fn spawn_river_bar(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    timber: &Handle<StandardMaterial>,
    iron: &Handle<StandardMaterial>,
) {
    let span = RIVER_MAX_Z - RIVER_MIN_Z;
    commands.spawn((
        Name::new("River Gate lifting oak locking bar"),
        RiverGateBar,
        Mesh3d(cube.clone()),
        MeshMaterial3d(timber.clone()),
        Transform::from_xyz(
            RIVER_BAR_X,
            RIVER_BAR_RAISED_Y,
            (RIVER_MIN_Z + RIVER_MAX_Z) * 0.5,
        )
        .with_scale(Vec3::new(0.62, 0.48, span + 1.8)),
    ));

    for z in [RIVER_MIN_Z - 0.32, RIVER_MAX_Z + 0.32] {
        spawn_box(
            commands,
            cube,
            iron,
            Vec3::new(RIVER_BAR_X, RIVER_BAR_LOCKED_Y, z),
            Vec3::new(0.82, 0.9, 0.32),
            "River Gate lower bar socket",
        );
        spawn_box(
            commands,
            cube,
            iron,
            Vec3::new(RIVER_BAR_X, RIVER_BAR_RAISED_Y - 0.3, z),
            Vec3::new(0.82, 0.82, 0.28),
            "River Gate raised-bar bracket",
        );
        spawn_box(
            commands,
            cube,
            iron,
            Vec3::new(RIVER_BAR_X + 0.22, RIVER_BAR_RAISED_Y - 0.38, z),
            Vec3::new(1.05, 0.16, 0.72),
            "River Gate raised-bar bracket shelf",
        );
    }
}

fn spawn_box(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    name: &'static str,
) {
    commands.spawn((
        Name::new(name),
        Mesh3d(cube.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(center).with_scale(size),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(day: i64, hour: f64, office: Office) -> Option<ClockSample> {
        Some(ClockSample {
            day,
            fraction: hour / 24.0,
            office,
        })
    }

    #[test]
    fn first_valid_clock_initializes_silently_and_absence_does_not_count() {
        let mut schedule = GateSchedule::default();
        assert_eq!(schedule.observe(None), None);
        assert_eq!(
            schedule.observe(sample(3, 12.0, Office::HighWick)),
            Some(ScheduleAction::Initialize(GatePosition::Open))
        );
        assert_eq!(schedule.observe(sample(3, 12.1, Office::HighWick)), None);
    }

    #[test]
    fn boundaries_are_one_shot_and_survive_missing_clock_frames() {
        let mut schedule = GateSchedule::default();
        assert_eq!(
            schedule.observe(sample(4, 18.1, Office::Lamplight)),
            Some(ScheduleAction::Initialize(GatePosition::Open))
        );
        assert_eq!(schedule.observe(None), None);
        assert_eq!(
            schedule.observe(sample(4, 22.0, Office::Snuffing)),
            Some(ScheduleAction::Transition(GatePosition::Closed))
        );
        assert_eq!(schedule.observe(sample(4, 23.0, Office::Snuffing)), None);
        assert_eq!(schedule.observe(sample(5, 5.2, Office::Kindling)), None);
        assert_eq!(
            schedule.observe(sample(5, 7.0, Office::Dayspring)),
            Some(ScheduleAction::Transition(GatePosition::Open))
        );
        assert_eq!(schedule.observe(sample(5, 8.0, Office::Dayspring)), None);
    }

    #[test]
    fn skipped_offices_still_cross_the_relevant_boundary() {
        let mut schedule = GateSchedule::default();
        let _ = schedule.observe(sample(8, 18.0, Office::Lamplight));
        assert_eq!(
            schedule.observe(sample(9, 2.2, Office::Watch)),
            Some(ScheduleAction::Transition(GatePosition::Closed))
        );
        assert_eq!(
            schedule.observe(sample(9, 12.0, Office::HighWick)),
            Some(ScheduleAction::Transition(GatePosition::Open))
        );
    }

    #[test]
    fn rewind_reconciles_without_a_transition() {
        let mut schedule = GateSchedule::default();
        let _ = schedule.observe(sample(6, 12.0, Office::HighWick));
        assert_eq!(
            schedule.observe(sample(5, 22.0, Office::Snuffing)),
            Some(ScheduleAction::Reconcile(GatePosition::Closed))
        );
    }

    #[test]
    fn runtime_emits_no_initial_cue_and_one_cue_per_real_transition() {
        let mut runtime = GateRuntime::default();
        assert_eq!(
            runtime.apply(ScheduleAction::Initialize(GatePosition::Closed)),
            None
        );
        assert_eq!(
            runtime.apply(ScheduleAction::Transition(GatePosition::Open)),
            Some(GateCue::RiverBarLift)
        );
        assert_eq!(
            runtime.apply(ScheduleAction::Transition(GatePosition::Open)),
            None
        );
        assert_eq!(
            runtime.apply(ScheduleAction::Transition(GatePosition::Closed)),
            Some(GateCue::StoneClosing)
        );
        assert_eq!(
            runtime.apply(ScheduleAction::Transition(GatePosition::Closed)),
            None
        );
    }

    #[test]
    fn full_motion_durations_end_exactly_on_the_audio_boundaries() {
        let mut leaves = Motion::at(1.0);
        leaves.start(0.0, LEAF_SWING_SECONDS);
        assert!(!leaves.is_at(0.0));
        assert_eq!(leaves.advance(LEAF_SWING_SECONDS), 0.0);
        assert!(leaves.is_at(0.0));

        let mut bar = Motion::at(0.0);
        bar.start(1.0, RIVER_BAR_LIFT_SECONDS);
        assert_eq!(bar.advance(RIVER_BAR_LIFT_SECONDS), 0.0);
        assert!(bar.is_at(1.0));
    }
}
