//! Fixed-size procedural pose state. Contacts follow the authoritative root;
//! no mesh mutation, navigation query, random allocation, or per-frame assets.
use super::*;
use crate::controller::PlayerCamera;

const STRIDE_PHASE_SCALE: f32 = 1.55;
#[cfg(test)]
const PAW_HEIGHT: f32 = 0.030;

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Quiet,
    Sniff,
    Watch,
    Shake,
    Sit,
    Yawn,
}

#[derive(Clone, Copy, Default)]
struct SitPose {
    pelvis: f32,
    reach: f32,
    prepare: f32,
    feet: [Vec3; 4],
}

#[derive(Clone, Copy)]
struct SitRelease {
    pose: SitPose,
    elapsed: f32,
    duration: f32,
    step_duration: f32,
    gradual_start: f32,
}

impl SitRelease {
    fn progress(self) -> f32 {
        (self.elapsed / self.duration).min(1.0)
    }

    fn step(self, leg: usize) -> (f32, f32) {
        // The chest reaches onto the forefeet first. Each rear foot stays
        // loaded through the first part of hip extension, then steps in turn.
        let (quick_start, quick_end) =
            [(0.02, 0.24), (0.27, 0.57), (0.40, 0.70), (0.56, 0.88)][leg];
        let (push_start, push_end) = [(0.20, 0.43), (0.49, 0.72), (0.52, 0.79), (0.64, 0.91)][leg];
        let start = quick_start + (push_start - quick_start) * self.gradual_start;
        let end = quick_end + (push_end - quick_end) * self.gradual_start;
        step_offset(self.elapsed / self.step_duration, start, end)
    }

    fn pose(self) -> SitPose {
        let t = self.progress();
        let release = 1.0
            - smoothstep(
                0.17 - 0.07 * self.gradual_start,
                0.74 - 0.04 * self.gradual_start,
                t,
            );
        SitPose {
            pelvis: self.pose.pelvis * release,
            reach: self.pose.reach * release
                + smoothstep(0.0, 0.20, t) * (1.0 - smoothstep(0.55, 1.0, t)) * 0.85,
            prepare: self.pose.prepare * release,
            feet: std::array::from_fn(|leg| self.pose.feet[leg] * (1.0 - self.step(leg).0)),
        }
    }
}

#[derive(Clone, Copy)]
struct StopPose {
    elapsed: f32,
    since_deceleration: f32,
    feet: [Vec3; 4],
    airborne: [bool; 4],
    order: [usize; 4],
}

#[derive(Clone, Copy)]
struct FootReplacement {
    from: Vec3,
    elapsed: f32,
    duration: f32,
}

impl StopPose {
    fn new(feet: [Vec3; 4], stance: [bool; 4]) -> Self {
        let mut order = [0; 4];
        let mut next = 0;
        for leg in [0, 3, 1, 2] {
            if stance[leg] {
                order[leg] = next;
                next += 1;
            }
        }
        Self {
            elapsed: 0.0,
            since_deceleration: 0.0,
            feet,
            airborne: stance.map(|planted| !planted),
            order,
        }
    }

    fn step(self, leg: usize) -> (f32, f32) {
        if self.airborne[leg] {
            step_offset(self.elapsed, 0.0, 0.18)
        } else {
            let start = 0.22 + self.order[leg] as f32 * 0.22;
            step_offset(self.elapsed, start, start + 0.19)
        }
    }
}

impl SitPose {
    fn body_shift(self) -> Vec3 {
        Vec3::new(
            0.0,
            -self.pelvis * 0.135 - self.reach * 0.009 - self.prepare * 0.004,
            -self.pelvis * 0.060 - self.reach * 0.028 - self.prepare * 0.014,
        )
    }
    fn body_pitch(self) -> f32 {
        self.pelvis * 0.62 - self.reach * 0.06 - self.prepare * 0.025
    }
}

fn step_offset(clock: f32, start: f32, end: f32) -> (f32, f32) {
    let t = ((clock - start) / (end - start)).clamp(0.0, 1.0);
    (smoothstep(0.0, 1.0, t), (PI * t).sin().max(0.0).powi(2))
}

/// Each rear paw lifts and tucks before the pelvis settles. A rise first
/// reaches the head/chest forward over the forepaws, then extends the hips,
/// then replaces the hind feet. The two directions have different timing.
fn sitting_pose(clock: f32, duration: f32) -> SitPose {
    let rise = duration - 2.0;
    let mut pose = SitPose {
        pelvis: smoothstep(0.62, 1.5, clock) * (1.0 - smoothstep(rise + 0.42, rise + 1.28, clock)),
        reach: smoothstep(rise, rise + 0.34, clock)
            * (1.0 - smoothstep(rise + 1.25, rise + 1.85, clock)),
        prepare: smoothstep(0.0, 0.18, clock) * (1.0 - smoothstep(0.72, 1.05, clock)),
        ..default()
    };
    for side in 0..2 {
        let stagger = side as f32 * 0.30;
        let (tuck, lift) = step_offset(clock, 0.17 + stagger, 0.55 + stagger);
        let (replace, rise_lift) = step_offset(clock, rise + 1.04 + stagger, rise + 1.42 + stagger);
        let planted = tuck - replace;
        pose.feet[side + 2] = Vec3::new(
            if side == 0 { 0.020 } else { -0.020 } * planted,
            0.038 * lift + 0.032 * rise_lift,
            -0.140 * planted,
        );
    }
    // A small forepaw adjustment while the other three contacts support the
    // forward reach; it lands before the hind feet leave their tucked sites.
    let (_, lift) = step_offset(clock, rise + 0.24, rise + 0.70);
    pose.feet[0] = Vec3::new(0.0, 0.027 * lift, -0.025 * lift);
    pose
}

pub(super) struct State {
    seed: u32,
    action: Action,
    clock: f32,
    duration: f32,
    strength: f32,
    next_action: u32,
    attention: Vec2,
    blink_clock: f32,
    blink_wait: f32,
    contacts: [Vec3; 4],
    foot_positions: [Vec3; 4],
    stance: [bool; 4],
    phase_shift: [f32; 4],
    sit_release: Option<SitRelease>,
    stopping: Option<StopPose>,
    previous_speed: f32,
    previous_rotation: Option<Quat>,
    turn_phase: f32,
    replacements: [Option<FootReplacement>; 4],
}

impl State {
    pub(super) fn new(id: &str) -> Self {
        let seed = id.bytes().fold(2166136261_u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(16777619)
        });
        let mut result = Self {
            seed,
            action: Action::Quiet,
            clock: 0.0,
            duration: 2.0,
            strength: 0.0,
            next_action: 0,
            attention: Vec2::ZERO,
            blink_clock: 0.0,
            blink_wait: 2.0,
            contacts: [Vec3::ZERO; 4],
            foot_positions: [Vec3::ZERO; 4],
            stance: [false; 4],
            phase_shift: [0.0; 4],
            sit_release: None,
            stopping: None,
            previous_speed: 0.0,
            previous_rotation: None,
            turn_phase: 0.0,
            replacements: [None; 4],
        };
        result.duration = 1.8 + result.random() * 3.5;
        result.blink_wait = 1.0 + result.random() * 3.0;
        result.next_action = seed % 3;
        result
    }
    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / 16_777_216.0
    }
    fn update(&mut self, dt: f32, moving: bool, relative_speed: f32) {
        if moving && self.action == Action::Sit && self.sit_release.is_none() {
            self.sit_release = Some(SitRelease {
                pose: sitting_pose(self.clock, self.duration),
                elapsed: 0.0,
                // Bound root travel during support transfer for abrupt starts
                // as well as small dogs; paths and their distance clock stay
                // authoritative. The hips extend over a separate interval:
                // fast starts need quicker steps, not a snapped body pose.
                duration: 0.74 - 0.14 * smoothstep(0.8, 1.7, relative_speed),
                step_duration: 0.74 * 0.8 / relative_speed.max(0.8),
                gradual_start: 1.0 - smoothstep(0.15, 0.65, relative_speed),
            });
        }
        if let Some(release) = &mut self.sit_release {
            release.elapsed += dt;
            if release.elapsed >= release.duration {
                self.sit_release = None;
                self.action = Action::Quiet;
                self.clock = 0.0;
            }
        }
        self.strength = move_toward(
            self.strength,
            if moving { 0.0 } else { 1.0 },
            dt / if moving { 0.22 } else { 0.7 },
        );
        if moving {
            if self.strength <= 0.0 && self.sit_release.is_none() {
                self.action = Action::Quiet;
                self.clock = 0.0;
            }
        } else if self.sit_release.is_none() && self.stopping.is_none() {
            self.clock += dt;
            if self.clock >= self.duration {
                self.clock = 0.0;
                if self.action == Action::Quiet {
                    self.action = match self.next_action % 5 {
                        0 => Action::Sniff,
                        1 => Action::Watch,
                        2 => Action::Shake,
                        3 => Action::Sit,
                        _ => Action::Yawn,
                    };
                    self.next_action += 1;
                    self.duration = match self.action {
                        Action::Sniff => 5.2,
                        Action::Watch => 4.0,
                        Action::Shake => 1.9,
                        Action::Sit => 8.0,
                        Action::Yawn => 2.1,
                        _ => 3.0,
                    };
                } else {
                    self.action = Action::Quiet;
                    self.duration = 2.8 + self.random() * 3.5;
                }
            }
        }
        self.blink_clock += dt;
        if self.blink_clock > self.blink_wait + 0.24 {
            self.blink_clock = 0.0;
            self.blink_wait = 2.8 + self.random() * 4.8;
        }
    }
    fn envelope(&self, enter: f32, exit: f32) -> f32 {
        smoothstep(0.0, enter, self.clock)
            * (1.0 - smoothstep(self.duration - exit, self.duration, self.clock))
            * self.strength
    }
    fn sit_pose(&self) -> SitPose {
        if let Some(release) = self.sit_release {
            release.pose()
        } else if self.action == Action::Sit {
            sitting_pose(self.clock, self.duration)
        } else {
            SitPose::default()
        }
    }
    fn yawn(&self) -> f32 {
        let clock = match self.action {
            Action::Yawn => self.clock,
            Action::Sit => self.clock - 2.5,
            _ => return 0.0,
        };
        smoothstep(0.25, 0.80, clock) * (1.0 - smoothstep(1.10, 1.75, clock)) * self.strength
    }
}

/// Sole contact is 30 mm below each named paw joint in that joint's local Y.
/// The full sole remains horizontal throughout stance; its broad pad vertices
/// retain the accepted authored 1 mm ground clearance.
pub(super) fn ankle(rear: bool) -> Vec3 {
    if rear {
        Vec3::new(0.0, -0.255, 0.106)
    } else {
        Vec3::new(0.0, -0.225, -0.025)
    }
}
pub(super) fn elbow(rear: bool) -> Vec3 {
    if rear {
        Vec3::new(0.0, -0.17, -0.07)
    } else {
        Vec3::new(0.0, -0.19, 0.02)
    }
}

pub(super) fn hock() -> Vec3 {
    Vec3::new(0.0, -0.174, 0.149)
}

/// Two-bone IK in the leg's bend plane; front elbows point back, stifles
/// forward. The near-straight clamp prevents a knee from flipping at extension.
fn leg_pose(origin: Vec3, target: Vec3, rear: bool) -> (Quat, Quat) {
    solve_two_bones(origin, target, elbow(rear), ankle(rear), rear)
}

fn solve_two_bones(
    origin: Vec3,
    target: Vec3,
    upper: Vec3,
    lower: Vec3,
    rear: bool,
) -> (Quat, Quat) {
    let a = upper.length();
    let b = lower.length();
    let delta = target - origin;
    let distance = delta.length().clamp((a - b).abs() + 0.0001, a + b - 0.0001);
    let direction = delta.normalize_or(Vec3::NEG_Y);
    let bend = Vec3::X.cross(direction).normalize_or(Vec3::Z) * if rear { 1.0 } else { -1.0 };
    let cosine = ((a * a + distance * distance - b * b) / (2.0 * a * distance)).clamp(-1.0, 1.0);
    let knee = direction * (a * cosine) + bend * (a * (1.0 - cosine * cosine).max(0.0).sqrt());
    let upper_rotation = Quat::from_rotation_arc(upper.normalize(), knee.normalize());
    let lower_rotation =
        Quat::from_rotation_arc(lower.normalize(), (direction * distance - knee).normalize());
    (upper_rotation, upper_rotation.inverse() * lower_rotation)
}

fn rear_pose(origin: Vec3, target: Vec3, fold: f32) -> (Quat, Quat, Quat) {
    let (upper, lower) = leg_pose(origin, target, true);
    if fold <= 0.0001 {
        return (upper, lower, Quat::IDENTITY);
    }
    let distal = ankle(true) - hock();
    let rest_direction = -(upper * lower * distal).normalize();
    let folded_direction = Vec3::new(
        0.0,
        0.035,
        (distal.length_squared() - 0.035_f32.powi(2)).sqrt(),
    )
    .normalize();
    let direction = rest_direction
        .lerp(folded_direction, fold)
        .normalize_or(Vec3::Z);
    let hock_target = target + direction * distal.length();
    let (upper, lower) = solve_two_bones(origin, hock_target, elbow(true), hock(), true);
    let distal_rotation = Quat::from_rotation_arc(distal.normalize(), -direction);
    (upper, lower, (upper * lower).inverse() * distal_rotation)
}

fn foot_path(phase: f32, duty: f32, stride: f32, clearance: f32) -> (f32, f32, bool) {
    let p = phase.rem_euclid(1.0);
    let reach = stride * duty * 0.5;
    if p < duty {
        (-reach + stride * p, 0.0, true)
    } else {
        let t = (p - duty) / (1.0 - duty);
        // Ease horizontal velocity at lift and landing. Lift has a broad arc,
        // and comes down before the next loaded stance begins.
        (
            reach * (1.0 - 2.0 * smoothstep(0.0, 1.0, t)),
            clearance * (PI * t).sin().powi(2),
            false,
        )
    }
}

fn rotation_delta(previous: Quat, current: Quat) -> f32 {
    if previous == current || previous == -current {
        return 0.0;
    }
    // atan2 of the relative quaternion remains accurate near zero, where
    // acos of a rounded dot product can invent motion for identical poses.
    let relative = previous.conjugate() * current;
    2.0 * Vec3::new(relative.x, relative.y, relative.z)
        .length()
        .atan2(relative.w.abs())
}

pub fn animate_dog_gait(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<PlayerCamera>>,
    mut dogs: Query<(&DogRig, &mut DogGait, &Transform, Option<&DogMotion>), Without<DogPart>>,
    mut parts: Query<&mut Transform, With<DogPart>>,
) {
    let _span = crate::perf::span(crate::perf::Probe::Dogs);
    let now = time.elapsed_secs();
    let dt = time.delta_secs().min(0.1);
    if dt <= 0.0 {
        return;
    }
    let camera = cameras.iter().next();
    for (rig, mut gait, root, motion) in &mut dogs {
        let (phase, travel_speed) = match motion {
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
        let angle = gait
            .animation
            .previous_rotation
            .map_or(0.0, |previous| rotation_delta(previous, root.rotation));
        gait.animation.previous_rotation = Some(root.rotation);
        // Angular motion has its own local foot clock. Navigation's distance
        // phase remains unchanged when a dog turns before starting its route.
        gait.animation.turn_phase +=
            angle * 0.24 * cathedral_sim::dogs::DOG_GAIT_CADENCE as f32 * STRIDE_PHASE_SCALE;
        let speed = travel_speed + angle / dt.max(0.001) * 0.24 * rig.build;
        let moving = speed > 0.04;
        gait.blend = move_toward(
            gait.blend,
            if moving { 1.0 } else { 0.0 },
            dt / TROT_BLEND_SECONDS,
        );
        let blend = gait.blend;
        let seed = gait.wag_seed;
        let state = &mut gait.animation;
        let starting = moving && state.previous_speed <= 0.04;
        let slowing = speed + 0.001 < state.previous_speed;
        if state.stopping.is_none()
            && state.previous_speed > 0.04
            && (!moving || (slowing && speed / rig.build < 0.70))
        {
            state.stopping = Some(StopPose::new(state.foot_positions, state.stance));
            state.replacements = [None; 4];
        }
        if let Some(stop) = &mut state.stopping {
            stop.elapsed += dt;
            stop.since_deceleration = if slowing {
                0.0
            } else {
                stop.since_deceleration + dt
            };
            let resumed = speed > state.previous_speed + 0.01
                || (speed > 0.04 && stop.since_deceleration > 0.14);
            if resumed || stop.elapsed > 1.10 {
                state.stopping = None;
            }
        }
        state.previous_speed = speed;
        state.update(dt, moving, speed / rig.build);
        let trot = smoothstep(0.65, 1.3, travel_speed);
        let cycle = phase * STRIDE_PHASE_SCALE / rig.build + state.turn_phase;
        let beat = cycle * TAU;
        let sniff = if state.action == Action::Sniff {
            state.envelope(1.1, 1.2)
        } else {
            0.0
        };
        let sit_pose = state.sit_pose();
        let sit = sit_pose.pelvis;
        let yawn = state.yawn();
        let watch = if state.action == Action::Watch {
            state.envelope(0.55, 0.8)
        } else {
            0.0
        };
        let shake = if state.action == Action::Shake {
            state.envelope(0.30, 0.65)
        } else {
            0.0
        };
        let shake_phase = state.clock * TAU * 4.3;
        let quiet = state.strength
            * (1.0 - sniff)
            * (1.0 - shake)
            * (1.0 - sit_pose.reach)
            * if state.stopping.is_some() { 0.0 } else { 1.0 };
        let quiet_clock = (now + seed * 1.7).rem_euclid(9.7);
        let quiet_side = if ((now + seed * 1.7) / 9.7).floor() as i32 % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        let settle = smoothstep(0.4, 1.1, quiet_clock) * (1.0 - smoothstep(2.8, 3.6, quiet_clock));
        let glance = smoothstep(1.8, 2.3, quiet_clock) * (1.0 - smoothstep(3.3, 4.2, quiet_clock));
        let breath = (now * 1.9 + seed).sin();
        let body_drop = blend * (1.0 - sit) * (-0.045 + trot * 0.017 - 0.005 * (beat * 2.0).cos());
        let mut shift = sit_pose.body_shift()
            + Vec3::new(
                (now * 0.63 + seed).sin() * 0.003 * state.strength
                    + quiet * settle * quiet_side * 0.009 * (1.0 - sit * 0.7),
                body_drop - sniff * 0.035 - quiet * settle * 0.008 * (1.0 - sit),
                0.0,
            );
        let body_rotation = Quat::from_euler(
            EulerRot::XYZ,
            -sniff * 0.12 + sit_pose.body_pitch() + (beat * 2.0).sin() * 0.008 * blend,
            0.0,
            shake * (shake_phase - 0.5).sin() * 0.09
                + beat.sin() * 0.013 * blend * (1.0 - trot)
                + quiet * settle * quiet_side * 0.015 * (1.0 - sit),
        );
        let duty = 0.68 - trot * 0.16;
        let stride = 1.0 / (cathedral_sim::dogs::DOG_GAIT_CADENCE as f32 * STRIDE_PHASE_SCALE);
        let frame_origin = root.translation + Vec3::Y * FRAME_Y;
        // Paws are attached to explicit ground positions. During a loaded
        // stance, retain the world contact through 20 Hz root interpolation
        // and turning. Swing placements are calculated in the root's frame.
        let mut targets = [Vec3::ZERO; 4];
        let mut lifts = [0.0_f32; 4];
        for i in 0..4 {
            let rear = i >= 2;
            let walk_offset = [0.0, 0.5, 0.75, 0.25][i];
            let trot_offset = [0.0, 0.5, 0.5, 0.0][i];
            let base_phase = cycle + walk_offset + (trot_offset - walk_offset) * trot;
            if starting && state.sit_release.is_none() {
                // A stand has no meaningful distance-clock phase. Begin a
                // staggered first step from its existing neutral contacts.
                let departure_phase = [duty + 0.08, duty * 0.60, duty * 0.75, duty + 0.02][i];
                state.phase_shift[i] = (departure_phase - base_phase + 0.5).rem_euclid(1.0) - 0.5;
            }
            let leg_phase = base_phase + state.phase_shift[i];
            let (forward, lift, _) = foot_path(leg_phase, duty, stride, 0.045 + trot * 0.035);
            let station = rig.stations[i];
            let rest_foot = station + elbow(rear) + ankle(rear) + sit_pose.feet[i];
            let travel_blend =
                blend * (1.0 - sit * 0.85) * (travel_speed / speed.max(0.04)).clamp(0.0, 1.0);
            let mut target = rest_foot + Vec3::new(0.0, lift * blend, forward * travel_blend);
            if !moving
                && matches!(state.action, Action::Quiet | Action::Watch)
                && i == if quiet_side > 0.0 { 1 } else { 0 }
            {
                let (_, paw_lift) = step_offset(quiet_clock, 1.3, 1.8);
                target += Vec3::new(0.0, 0.023 * paw_lift, -0.016 * paw_lift) * quiet;
            }
            let release_step = state.sit_release.filter(|release| release.step(i).0 < 1.0);
            if let Some(release) = release_step {
                let (step, clearance) = release.step(i);
                let from = root.rotation.inverse() * (state.contacts[i] - frame_origin) / rig.build;
                let landing = station
                    + elbow(rear)
                    + ankle(rear)
                    + Vec3::new(0.0, 0.0, -stride * duty * 0.25);
                // Depart from the actual supporting contact, rather than
                // snapping a newly lifted foot back into the moving frame.
                target = from.lerp(landing, step);
                target.y = from.y.max(0.030) * (1.0 - step)
                    + 0.030 * step
                    + clearance * if rear { 0.060 } else { 0.047 };
            }
            if let Some(stop) = state.stopping {
                let (step, lift) = stop.step(i);
                let from = root.rotation.inverse() * (stop.feet[i] - frame_origin) / rig.build;
                let landing = station + elbow(rear) + ankle(rear);
                target = from.lerp(landing, step);
                target.y = from.y * (1.0 - step)
                    + 0.030 * step
                    + lift * if stop.airborne[i] { 0.014 } else { 0.036 };
            }
            if moving && state.sit_release.is_none() && state.stopping.is_none() {
                let from = root.rotation.inverse() * (state.contacts[i] - frame_origin) / rig.build;
                if state.replacements[i].is_none()
                    && state.stance[i]
                    && (from - station).xz().length() > 0.22
                {
                    state.replacements[i] = Some(FootReplacement {
                        from: state.contacts[i],
                        elapsed: 0.0,
                        duration: (0.18 * 0.8 / (speed / rig.build).max(0.8)).max(0.10),
                    });
                }
            }
            let mut replacing = false;
            if let Some(replacement) = &mut state.replacements[i] {
                replacing = true;
                replacement.elapsed += dt;
                let (step, lift) = step_offset(replacement.elapsed, 0.0, replacement.duration);
                let from = root.rotation.inverse() * (replacement.from - frame_origin) / rig.build;
                let landing = station
                    + elbow(rear)
                    + ankle(rear)
                    + Vec3::new(0.0, 0.0, -stride * duty * 0.25 * travel_blend);
                target = from.lerp(landing, step);
                target.y = 0.030 + lift * 0.045;
                if step >= 1.0 {
                    state.replacements[i] = None;
                }
            }
            // Lift the pad vertically before transferring it forward. Keep
            // the same world contact through the first departure samples;
            // waiting for full gait blend lets loaded feet slide at launch.
            let grounded = target.y <= 0.034;
            if grounded {
                if !state.stance[i] {
                    let planted = Vec3::new(target.x, 0.030, target.z);
                    state.contacts[i] = frame_origin + root.rotation * (planted * rig.build);
                    if release_step.is_some() || replacing {
                        // Continue this new contact into its own stance phase;
                        // a shared gait clock may currently expect a swing.
                        state.phase_shift[i] =
                            (duty * 0.25 - base_phase + 0.5).rem_euclid(1.0) - 0.5;
                    }
                }
                target = root.rotation.inverse() * (state.contacts[i] - frame_origin) / rig.build;
                state.stance[i] = true;
            } else {
                state.stance[i] = false;
                if release_step.is_none() {
                    // Restore normal walk/diagonal timing only while unloaded.
                    state.phase_shift[i] = move_toward(state.phase_shift[i], 0.0, dt * 0.5);
                }
            }
            targets[i] = target;
            lifts[i] = lift;
            state.foot_positions[i] = frame_origin + root.rotation * (target * rig.build);
        }
        if moving || state.stopping.is_some() {
            // Let the trunk follow its current base of support through a
            // turn. The feet retain their world contacts while the shoulders
            // and hips settle a little toward those contacts.
            if sit < 0.05 {
                let mut support_offset = Vec3::ZERO;
                let mut count = 0.0_f32;
                for i in 0..4 {
                    if state.stance[i] {
                        support_offset +=
                            targets[i] - (rig.stations[i] + elbow(i >= 2) + ankle(i >= 2));
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    support_offset /= count;
                    shift.x += (support_offset.x * 0.35).clamp(-0.035, 0.035);
                    shift.z += (support_offset.z * 0.20).clamp(-0.025, 0.025);
                }
            }
            // Solve body support against the same final contacts used by IK.
            // Flexing over a loaded leg keeps the rise within its reach.
            let mut support_drop = 0.0_f32;
            for i in 0..4 {
                if !state.stance[i] {
                    continue;
                }
                let rear = i >= 2;
                let reach = elbow(rear).length() + ankle(rear).length()
                    - 0.003
                    - if rear { sit * 0.060 } else { 0.0 };
                let origin =
                    body_rotation * (rig.stations[i] - Vec3::Y * BODY_Y) + Vec3::Y * BODY_Y + shift;
                let horizontal = (origin - targets[i]).xz().length_squared();
                let supported_y = targets[i].y + (reach * reach - horizontal).max(0.0).sqrt();
                support_drop = support_drop.max(origin.y - supported_y);
            }
            shift.y -= support_drop.clamp(0.0, 0.120);
        }
        if let Ok(mut body) = parts.get_mut(rig.body) {
            body.translation = Vec3::new(0.0, BODY_Y, 0.0) + shift;
            body.rotation = body_rotation;
            body.scale = Vec3::new(1.0 + breath * 0.0035 * (1.0 - blend), 1.0, 1.0);
        }
        for i in 0..4 {
            let rear = i >= 2;
            let station = rig.stations[i];
            let target = targets[i];
            let lift = lifts[i];
            let origin = body_rotation * (station - Vec3::Y * BODY_Y) + Vec3::Y * BODY_Y + shift;
            let (upper_rotation, lower_rotation, hock_rotation) = if rear {
                rear_pose(origin, target, sit)
            } else {
                let (a, b) = leg_pose(origin, target, false);
                (a, b, Quat::IDENTITY)
            };
            if let Ok(mut upper) = parts.get_mut(rig.uppers[i]) {
                upper.translation = origin;
                upper.rotation = upper_rotation;
            }
            if let Ok(mut lower) = parts.get_mut(rig.lowers[i]) {
                lower.rotation = lower_rotation;
            }
            if rear && let Ok(mut hock) = parts.get_mut(rig.hocks[i - 2]) {
                hock.rotation = hock_rotation;
            }
            if let Ok(mut paw) = parts.get_mut(rig.paws[i]) {
                // The planted digital pad stays level as the shin changes
                // angle. Swing flex clears toes; it is zero at both contacts.
                let curl = if state.stance[i] {
                    0.0
                } else {
                    -0.28 * (lift / 0.08) * blend
                };
                paw.rotation = (upper_rotation * lower_rotation * hock_rotation).inverse()
                    * Quat::from_rotation_x(curl);
            }
        }
        let look_gate =
            (1.0 - sniff) * (1.0 - shake) * (1.0 - blend * 0.8) * (1.0 - sit_pose.reach * 0.8);
        let mut attention_target = Vec2::ZERO;
        if let Some(camera) = camera {
            let eye = frame_origin + root.rotation * (Vec3::new(0.0, 0.61, -0.40) * rig.build);
            let offset = camera.translation() - eye;
            let distance = offset.length();
            let facing = camera.forward().dot(-offset.normalize_or(Vec3::Z));
            let local = root.rotation.inverse() * offset;
            let yaw = (-local.x).atan2(-local.z);
            if distance < 6.5 && distance > 0.35 && yaw.abs() < 1.3 && facing > 0.88 {
                attention_target = Vec2::new(
                    yaw.clamp(-0.52, 0.52),
                    (local.y / local.xz().length().max(0.5))
                        .atan()
                        .clamp(-0.22, 0.3),
                ) * look_gate;
            }
        }
        state.attention = state.attention.lerp(
            attention_target.lerp(
                Vec2::new(quiet_side * 0.28, -0.08),
                glance * quiet * (1.0 - yawn),
            ),
            1.0 - (-dt * 3.5).exp(),
        );
        let sniff_search = (state.clock * 2.5).sin() * 0.12 * sniff;
        let neck_yaw = state.attention.x * 0.40 + sniff_search * 0.40;
        if let Ok(mut neck) = parts.get_mut(rig.neck) {
            neck.rotation = Quat::from_euler(
                EulerRot::XYZ,
                -1.20 * sniff - sit * 0.47 - sit_pose.reach * 0.27 - sit_pose.prepare * 0.12
                    + state.attention.y * 0.45
                    + 0.025 * watch
                    + yawn * 0.045,
                neck_yaw,
                shake * (shake_phase - 0.22).sin() * 0.13,
            );
        }
        let watch_scan = watch * (state.clock * 0.9).sin() * 0.22;
        let tilt = watch
            * smoothstep(0.4, 1.1, state.clock)
            * (1.0 - smoothstep(2.1, 2.7, state.clock))
            * 0.16;
        if let Ok(mut head) = parts.get_mut(rig.head) {
            head.rotation = Quat::from_euler(
                EulerRot::XYZ,
                -0.23 * sniff - sit * 0.18
                    + sit_pose.reach * 0.12
                    + sniff * (state.clock * 15.0).sin() * 0.012
                    + state.attention.y * 0.55
                    + (beat * 2.0).sin() * 0.025 * blend
                    + yawn * 0.035,
                state.attention.x * 0.60 + sniff_search * 0.60 + watch_scan - yawn * 0.12,
                tilt + shake * shake_phase.sin() * 0.40 + glance * quiet * quiet_side * 0.055,
            );
        }
        if let Ok(mut jaw) = parts.get_mut(rig.jaw) {
            jaw.rotation = Quat::from_rotation_x(-yawn * 0.30);
        }
        for (index, sign) in [-1.0, 1.0].into_iter().enumerate() {
            let ear_clock =
                (now + seed * 1.7 + index as f32 * 3.4).rem_euclid(8.2 + index as f32 * 1.1);
            let flick =
                smoothstep(0.0, 0.08, ear_clock) * (1.0 - smoothstep(0.10, 0.30, ear_clock));
            if let Ok(mut ear) = parts.get_mut(rig.ears[index]) {
                ear.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    flick * 0.20
                        + shake * (shake_phase - 0.85 - index as f32 * 0.25).sin() * 0.5
                        + beat.sin() * 0.07 * blend,
                    sign * (watch * 0.10 + flick * 0.12) + state.attention.x * 0.12,
                    sign * -0.12 + flick * sign * 0.15,
                );
            }
        }
        let blink = if state.blink_clock > state.blink_wait {
            let p = (state.blink_clock - state.blink_wait) / 0.24;
            smoothstep(0.0, 0.25, p) * (1.0 - smoothstep(0.45, 1.0, p))
        } else {
            0.0
        };
        if let Ok(mut eyes) = parts.get_mut(rig.eyes) {
            eyes.scale.y = 1.0 - (blink * 0.96).max(yawn * 0.65);
            eyes.translation.y = 0.187 * (1.0 - eyes.scale.y);
        }
        if let Ok(mut tail) = parts.get_mut(rig.tail) {
            let interest = (state.attention.length() * 1.8 + watch * 0.5).min(1.0);
            let wag_burst = smoothstep(-0.1, 0.5, (now * 0.62 + seed).sin());
            let wag = (now * (1.5 + interest) * TAU + seed).sin()
                * (0.05 + interest * 0.22)
                * wag_burst
                * (1.0 - sniff * 0.7);
            tail.rotation = Quat::from_euler(
                EulerRot::XYZ,
                // Counter the raised spine so the seated tail lies behind
                // the haunches instead of being driven through the floor.
                -sit * 0.90 - interest * 0.10,
                wag + shake * (shake_phase - 1.1).sin() * 0.2 + beat.sin() * 0.025 * blend,
                0.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn yawn_opens_and_closes_completely_and_closes_on_interruption() {
        let mut state = State::new("yawn_test");
        state.action = Action::Yawn;
        state.duration = 2.1;
        state.strength = 1.0;
        assert_eq!(state.yawn(), 0.0);
        let mut maximum = 0.0_f32;
        for frame in 0..=126 {
            state.clock = frame as f32 / 60.0;
            let amount = state.yawn();
            assert!(amount.is_finite() && (0.0..=1.0).contains(&amount));
            maximum = maximum.max(amount);
        }
        assert!(maximum > 0.99);
        assert_eq!(state.yawn(), 0.0);
        state.clock = 0.9;
        for _ in 0..15 {
            state.update(1.0 / 60.0, true, 0.8);
        }
        assert_eq!(state.yawn(), 0.0);
        assert!(state.action == Action::Quiet);
    }
    #[test]
    fn jaw_skin_preserves_the_skull_and_opens_from_its_hinge() {
        use bevy::mesh::VertexAttributeValues;
        let mut meshes = Assets::<Mesh>::default();
        let features = meshes.add(appearance::face_mesh());
        let eyes = meshes.add(appearance::eye_mesh());
        let form = appearance::build(DogCoat::Brindle, &mut meshes, features, eyes);
        let mesh = meshes.get(&form.head).unwrap();
        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            unreachable!()
        };
        let VertexAttributeValues::Float32x4(weights) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT).unwrap()
        else {
            unreachable!()
        };
        let opened = Mat4::from_translation(JAW_PIVOT)
            * Mat4::from_quat(Quat::from_rotation_x(-0.30))
            * Mat4::from_translation(-JAW_PIVOT);
        let mut muzzle_drop = 0.0_f32;
        for (point, weights) in positions.iter().zip(weights) {
            let p = Vec3::from_array(*point);
            assert!((weights.iter().sum::<f32>() - 1.0).abs() < 0.00001);
            let q = p * weights[0] + opened.transform_point3(p) * weights[1];
            assert!(q.is_finite());
            if weights[0] == 1.0 {
                assert_eq!(p, q);
            }
            if p.z < -0.28 && weights[1] == 1.0 {
                muzzle_drop = muzzle_drop.max(p.y - q.y);
            }
        }
        assert!(muzzle_drop > 0.040 && muzzle_drop < 0.060);
    }
    #[test]
    fn sit_repositions_support_before_lowering_and_reaches_before_rising() {
        let first_step = sitting_pose(0.36, 8.0);
        assert_eq!(first_step.pelvis, 0.0);
        assert!(first_step.feet[2].y > 0.03 && first_step.feet[2].z < -0.02);
        assert_eq!(first_step.feet[3], Vec3::ZERO);
        let seated = sitting_pose(2.0, 8.0);
        assert_eq!(seated.pelvis, 1.0);
        assert!(seated.feet.iter().all(|p| p.y.abs() < 0.0001));
        assert!(seated.feet[2].z < -0.07 && seated.feet[3].z < -0.07);
        let preparing_to_rise = sitting_pose(6.35, 8.0);
        assert_eq!(preparing_to_rise.pelvis, 1.0);
        assert!(preparing_to_rise.reach > 0.99);
        let stand = sitting_pose(8.0, 8.0);
        assert_eq!(stand.pelvis, 0.0);
        assert!(stand.feet.iter().all(|p| p.length() < 0.0001));
    }
    #[test]
    fn folded_hocks_keep_the_paws_grounded_and_the_knees_below_the_hips() {
        for frame in 0..=480 {
            let pose = sitting_pose(frame as f32 / 60.0, 8.0);
            let rotation = Quat::from_rotation_x(pose.body_pitch());
            let shift = pose.body_shift();
            let station = Vec3::new(0.09, 0.455, 0.255);
            let origin = rotation * (station - Vec3::Y * BODY_Y) + Vec3::Y * BODY_Y + shift;
            let target = station + elbow(true) + ankle(true) + pose.feet[2];
            let (a, b, c) = rear_pose(origin, target, pose.pelvis);
            let knee = origin + a * elbow(true);
            let heel = knee + a * b * hock();
            let solved = heel + a * b * c * (ankle(true) - hock());
            assert!(
                solved.distance(target) < 0.001,
                "frame {frame}: {solved:?} != {target:?}"
            );
            assert!(
                knee.y > 0.025 && heel.y > 0.025,
                "frame {frame}: knee {knee:?}, hock {heel:?}"
            );
            if pose.pelvis > 0.99 {
                assert!(
                    knee.y < origin.y,
                    "seated stifle must not project above the hip"
                );
            }
        }
    }
    #[test]
    fn interrupting_a_seated_hold_keeps_a_complete_short_departure() {
        let mut state = State::new("seated_departure_test");
        state.action = Action::Sit;
        state.clock = 3.0;
        state.duration = 8.0;
        state.strength = 1.0;
        state.update(1.0 / 60.0, true, 0.8);
        assert!(state.sit_pose().pelvis > 0.98);
        for _ in 0..48 {
            state.update(1.0 / 60.0, true, 0.8);
        }
        assert!(state.action == Action::Quiet);
        assert_eq!(state.sit_pose().pelvis, 0.0);
        assert!(state.sit_pose().feet.iter().all(|p| *p == Vec3::ZERO));
    }
    #[test]
    fn interrupted_rise_shifts_weight_before_extending_and_then_steps_the_hindfeet() {
        let mut release = SitRelease {
            pose: sitting_pose(3.0, 8.0),
            elapsed: 0.74 * 0.17,
            duration: 0.74,
            step_duration: 0.74,
            gradual_start: 0.0,
        };
        assert_eq!(release.pose().pelvis, 1.0);
        assert!(release.pose().reach > 0.75);
        assert_eq!(release.step(2), (0.0, 0.0));
        assert_eq!(release.step(3), (0.0, 0.0));
        release.elapsed = 0.74 * 0.40;
        assert!(release.pose().pelvis < 0.65 && release.pose().pelvis > 0.60);
        assert_eq!(release.step(2), (0.0, 0.0));
        assert_eq!(release.step(3), (0.0, 0.0));
        release.elapsed = 0.74 * 0.56;
        assert!(release.pose().pelvis < 0.25);
        assert!(release.step(2).1 > 0.9);
        assert_eq!(release.step(3), (0.0, 0.0));
        release.gradual_start = 1.0;
        release.elapsed = 0.74 * 0.52;
        assert!(release.pose().pelvis < 0.25);
        assert_eq!(release.step(2), (0.0, 0.0));
        assert_eq!(release.step(3), (0.0, 0.0));
    }
    #[test]
    fn constant_nonzero_heading_and_paused_frames_do_not_interrupt_a_seated_rest() {
        for yaw in [-1.85627055_f32, 1.2853221, 2.6971734] {
            let rotation = Quat::from_rotation_y(yaw);
            assert_eq!(rotation_delta(rotation, rotation), 0.0);
            assert_eq!(rotation_delta(rotation, -rotation), 0.0);
            assert!(
                (rotation_delta(rotation, rotation * Quat::from_rotation_y(0.0001)) - 0.0001).abs()
                    < 0.0000002
            );
            let mut app = App::new();
            app.add_plugins(TransformPlugin)
                .init_resource::<Time>()
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<Image>>()
                .init_resource::<Assets<StandardMaterial>>()
                .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
                .init_resource::<DogInbox>()
                .add_systems(Update, (sync_dogs, animate_dog_gait).chain());
            app.world_mut().resource_mut::<DogInbox>().0.insert(
                "stationary".into(),
                DogSample {
                    name: "Stationary".into(),
                    coat: DogCoat::Brindle,
                    build: 1.0,
                    position: Vec3::new(0.0, -FRAME_Y, 0.0),
                    facing_yaw: yaw,
                    speed: 0.0,
                    gait_phase: 0.0,
                    seq: 1,
                },
            );
            app.update();
            let root = app
                .world_mut()
                .query_filtered::<Entity, With<DogGait>>()
                .single(app.world())
                .unwrap();
            {
                let mut gait = app.world_mut().get_mut::<DogGait>(root).unwrap();
                gait.animation.action = Action::Sit;
                gait.animation.clock = 3.0;
                gait.animation.duration = 100.0;
                gait.animation.strength = 1.0;
            }
            for frame in 0..240 {
                let paused = frame % 2 == 1;
                let before = {
                    let gait = app.world().get::<DogGait>(root).unwrap();
                    (
                        gait.animation.clock,
                        gait.animation.blink_clock,
                        gait.animation.contacts,
                        gait.animation.foot_positions,
                    )
                };
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(if paused {
                        std::time::Duration::ZERO
                    } else {
                        std::time::Duration::from_secs_f32(1.0 / 60.0)
                    });
                app.update();
                let gait = app.world().get::<DogGait>(root).unwrap();
                let state = &gait.animation;
                assert_eq!(state.turn_phase, 0.0, "yaw {yaw}, frame {frame}");
                assert_eq!(state.previous_speed, 0.0);
                assert_eq!(gait.blend, 0.0);
                assert!(state.action == Action::Sit);
                assert!(state.sit_release.is_none() && state.stopping.is_none());
                assert_eq!(state.strength, 1.0);
                if paused {
                    assert_eq!(
                        before,
                        (
                            state.clock,
                            state.blink_clock,
                            state.contacts,
                            state.foot_positions
                        )
                    );
                }
            }
        }
    }
    #[test]
    fn starts_turns_and_stops_keep_actual_loaded_paws_planted_across_builds_and_speeds() {
        for build in [0.72, 1.0, 1.2] {
            for (cruise_speed, acceleration) in [(0.8, 0.0), (1.7, 0.0), (0.8, 1.8), (1.7, 1.8)] {
                let mut app = App::new();
                app.add_plugins(TransformPlugin)
                    .init_resource::<Time>()
                    .init_resource::<Assets<Mesh>>()
                    .init_resource::<Assets<Image>>()
                    .init_resource::<Assets<StandardMaterial>>()
                    .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
                    .init_resource::<DogInbox>()
                    .add_systems(Update, (sync_dogs, animate_dog_gait).chain());
                app.world_mut().resource_mut::<DogInbox>().0.insert(
                    "test".into(),
                    DogSample {
                        name: "Test".into(),
                        coat: DogCoat::Brindle,
                        build,
                        position: Vec3::new(0.0, -FRAME_Y, 0.0),
                        facing_yaw: 0.0,
                        speed: 0.0,
                        gait_phase: 0.0,
                        seq: 1,
                    },
                );
                app.update();
                let root = app
                    .world_mut()
                    .query_filtered::<Entity, With<DogGait>>()
                    .single(app.world())
                    .unwrap();
                {
                    let mut gait = app.world_mut().get_mut::<DogGait>(root).unwrap();
                    gait.animation.action = Action::Sit;
                    gait.animation.clock = 3.0;
                    gait.animation.duration = 8.0;
                    gait.animation.strength = 1.0;
                    // This fixture starts already seated; seed fresh contacts
                    // for that pose rather than retaining the spawned stand.
                    gait.animation.stance = [false; 4];
                }
                app.world_mut()
                    .resource_mut::<Time>()
                    .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
                app.update();
                let mut previous = [Vec3::ZERO; 4];
                let mut previous_stance = [false; 4];
                let mut position = Vec3::new(0.0, -FRAME_Y, 0.0);
                let mut distance = 0.0;
                let mut previous_speed = 0.0;
                for frame in 0..600 {
                    let clock = frame as f32 / 60.0;
                    let speed = if clock >= 7.5 {
                        if acceleration > 0.0 {
                            ((clock - 7.5) * acceleration).min(cruise_speed)
                        } else {
                            cruise_speed
                        }
                    } else if clock >= 3.5 {
                        if acceleration > 0.0 {
                            (cruise_speed - 2.8 * (clock - 3.5)).max(0.0)
                        } else {
                            0.0
                        }
                    } else if acceleration > 0.0 {
                        (clock * acceleration).min(cruise_speed)
                    } else {
                        cruise_speed
                    };
                    let yaw = smoothstep(1.8, 3.3, clock) * std::f32::consts::FRAC_PI_2
                        + ((clock - 4.7) / 2.0).clamp(0.0, 1.0) * PI;
                    let rotation = Quat::from_rotation_y(yaw);
                    if frame > 0 {
                        let step = (speed + previous_speed) * 0.5 / 60.0;
                        distance += step;
                        position += rotation * Vec3::new(0.0, 0.0, -step);
                    }
                    previous_speed = speed;
                    app.world_mut()
                        .resource_mut::<Time>()
                        .advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
                    *app.world_mut().get_mut::<Transform>(root).unwrap() =
                        Transform::from_translation(position).with_rotation(rotation);
                    let phase = distance * cathedral_sim::dogs::DOG_GAIT_CADENCE as f32;
                    app.world_mut().entity_mut(root).insert(DogMotion {
                        previous: Vec3::ZERO,
                        current: Vec3::ZERO,
                        prev_yaw: 0.0,
                        cur_yaw: 0.0,
                        prev_phase: phase,
                        cur_phase: phase,
                        speed,
                        t0: clock as f64,
                        seq: 1,
                    });
                    app.update();
                    let rig = app.world().get::<DogRig>(root).unwrap();
                    let state = &app.world().get::<DogGait>(root).unwrap().animation;
                    if state.sit_release.is_some() && state.sit_pose().pelvis > 0.10 {
                        assert!(
                            state.stance[0] || state.stance[1],
                            "build {build}, speed {speed}, frame {frame}: rise lost both forepaw supports"
                        );
                    }
                    for i in 0..4 {
                        let paw = app
                            .world()
                            .get::<GlobalTransform>(rig.paws[i])
                            .unwrap()
                            .translation();
                        if state.stance[i] {
                            assert!(
                                (paw.y - 0.030 * build).abs() < 0.001,
                                "build {build}, speed {speed}, frame {frame}, leg {i}: loaded paw left floor: {paw:?}"
                            );
                            if previous_stance[i] {
                                assert!(
                                    paw.distance(previous[i]) < 0.001,
                                    "build {build}, speed {speed}, frame {frame}, leg {i}: loaded paw slid from {:?} to {paw:?}",
                                    previous[i]
                                );
                            }
                        }
                        previous[i] = paw;
                        previous_stance[i] = state.stance[i];
                    }
                }
            }
        }
    }
    #[test]
    fn sitting_hindleg_surface_keeps_its_pad_and_flesh_above_the_floor() {
        use bevy::mesh::VertexAttributeValues;
        let mut meshes = Assets::<Mesh>::default();
        let features = meshes.add(appearance::face_mesh());
        let eyes = meshes.add(appearance::eye_mesh());
        for coat in [
            DogCoat::Brindle,
            DogCoat::Black,
            DogCoat::Grey,
            DogCoat::White,
            DogCoat::Fawn,
            DogCoat::Pied,
        ] {
            let form = appearance::build(coat, &mut meshes, features.clone(), eyes.clone());
            for side in 0..2 {
                let body_rotation = Quat::from_rotation_x(sitting_pose(2.0, 8.0).body_pitch());
                let shift = sitting_pose(2.0, 8.0).body_shift();
                let station = Vec3::new(
                    if side == 0 { 0.09 } else { -0.09 } * form.width,
                    0.455,
                    0.255 * form.length,
                );
                let origin =
                    body_rotation * (station - Vec3::Y * BODY_Y) + Vec3::Y * BODY_Y + shift;
                let target =
                    station + elbow(true) + ankle(true) + sitting_pose(2.0, 8.0).feet[side + 2];
                let (a, b, c) = rear_pose(origin, target, 1.0);
                let upper = Mat4::from_rotation_translation(a, origin);
                let lower = upper * Mat4::from_rotation_translation(b, elbow(true));
                let heel = lower * Mat4::from_rotation_translation(c, hock());
                let paw = heel
                    * Mat4::from_rotation_translation((a * b * c).inverse(), ankle(true) - hock());
                let body = Mat4::from_rotation_translation(body_rotation, Vec3::Y * BODY_Y + shift);
                for (surface, (handle, palette)) in [
                    (
                        &form.rear_upper[side],
                        vec![
                            body * Mat4::from_translation(station - Vec3::Y * BODY_Y),
                            upper,
                            lower * Mat4::from_translation(-elbow(true)),
                        ],
                    ),
                    (
                        &form.rear_lower,
                        vec![
                            lower,
                            heel * Mat4::from_translation(-hock()),
                            paw * Mat4::from_translation(-ankle(true)),
                        ],
                    ),
                    (&form.body, vec![body, body]),
                    (
                        &form.tail,
                        vec![
                            body * Mat4::from_translation(Vec3::new(0.0, 0.06, 0.34 * form.length))
                                * Mat4::from_quat(Quat::from_rotation_x(-0.90)),
                        ],
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let mesh = meshes.get(handle).unwrap();
                    let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        unreachable!()
                    };
                    let weights =
                        mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
                            .and_then(|attribute| match attribute {
                                VertexAttributeValues::Float32x4(weights) => Some(weights),
                                _ => None,
                            });
                    let minimum = positions
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let w = weights.map_or(&[1.0, 0.0, 0.0, 0.0], |weights| &weights[i]);
                            palette
                                .iter()
                                .zip(w)
                                .map(|(matrix, weight)| {
                                    matrix.transform_point3(Vec3::from_array(*p)).y * weight
                                })
                                .sum::<f32>()
                        })
                        .fold(f32::INFINITY, f32::min);
                    for build in [0.72, 1.0, 1.2] {
                        assert!(
                            minimum * build >= -0.001,
                            "{coat:?} side {side}, surface {surface}, build {build}: sitting skin penetrates floor: {}",
                            minimum * build
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn inverse_kinematics_keeps_planted_soles_at_the_target() {
        for rear in [false, true] {
            let origin = Vec3::new(0.0, if rear { 0.410 } else { 0.400 }, 0.0);
            for step in 0..101 {
                let z = -0.16 + step as f32 * 0.0032;
                let target = Vec3::new(0.0, PAW_HEIGHT, z);
                let (a, b) = leg_pose(origin, target, rear);
                let solved = origin + a * elbow(rear) + a * b * ankle(rear);
                assert!(
                    solved.distance(target) < 0.001,
                    "rear={rear} target={target:?}, solved={solved:?}"
                );
                let sole = solved + (a * b * (a * b).inverse()) * Vec3::new(0.0, -PAW_HEIGHT, 0.0);
                assert!(sole.y.abs() < 0.001);
            }
        }
    }
    #[test]
    fn stance_travel_matches_the_distance_clock_and_swing_clears_the_floor() {
        let stride = 1.0 / (cathedral_sim::dogs::DOG_GAIT_CADENCE as f32 * STRIDE_PHASE_SCALE);
        for duty in [0.52, 0.68] {
            let a = foot_path(0.1, duty, stride, 0.08);
            let b = foot_path(0.2, duty, stride, 0.08);
            assert!((b.0 - a.0 - stride * 0.1).abs() < 0.00001);
            assert_eq!((a.1, b.1), (0.0, 0.0));
            for step in 0..1000 {
                let (_, y, _) = foot_path(step as f32 / 1000.0, duty, stride, 0.08);
                assert!(y >= 0.0 && y <= 0.08001);
            }
        }
    }
}
