//! Visible machinery and cargo handoffs for the trade soundscape cues.
//!
//! These props remain presentation-only: the authoritative event supplies a
//! world-space contact point, and this module gives that event a short-lived,
//! bounded visual counterpart.

use std::{collections::HashMap, f32::consts::TAU};

use bevy::prelude::*;

use crate::soundscape::{CargoHandoffKind, SoundscapeCue};

use super::{CityMaterials, CityMeshes};

const WEIGHBEAM_CUE_RADIUS_M: f32 = 4.0;
const WEIGHBEAM_AMPLITUDE_RADIANS: f32 = 0.085;
const WEIGHBEAM_DAMPING: f32 = 2.7;
const WEIGHBEAM_FREQUENCY: f32 = 12.5;
const WEIGHBEAM_SETTLE_SECONDS: f32 = 2.4;

/// The first frame on which the prop reaches the cue's authored contact point.
/// Sound scheduling uses these same delays so the recording lands on the
/// visible impact rather than on the beginning of the lowering motion.
const SACK_CONTACT_SECONDS: f32 = 0.20;
const CRATE_CONTACT_SECONDS: f32 = 0.48;
const CARGO_LIFETIME_SECONDS: f32 = 5.0;
const CARGO_FADE_SECONDS: f32 = 0.35;
const CARGO_CUE_COOLDOWN_SECONDS: f64 = 0.8;
const MAX_LIVE_CARGO_PROPS: usize = 12;

#[derive(Component, Debug)]
pub(super) struct TradeWeighbeamRig {
    anchor: Vec3,
    yaw: f32,
    elapsed: f32,
    direction: f32,
    active: bool,
}

impl TradeWeighbeamRig {
    fn new(anchor: Vec3, yaw: f32) -> Self {
        Self {
            anchor,
            yaw,
            elapsed: 0.0,
            direction: 1.0,
            active: false,
        }
    }

    fn kick(&mut self) {
        self.elapsed = 0.0;
        self.direction = -self.direction;
        self.active = true;
    }
}

/// Inspectable semantic parts keep the authored apparatus understandable in
/// entity inspection without coupling animation to child ordering.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "semantic fixture metadata is read by inspectors")]
enum WeighbeamPart {
    BalanceArm,
    Chain,
    Pan,
    Indicator,
    Fulcrum,
}

#[derive(Resource, Clone)]
pub(super) struct TradePropAssets {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    sphere: Handle<Mesh>,
    timber: Handle<StandardMaterial>,
    dark_wood: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    sack_cloth: Handle<StandardMaterial>,
}

impl TradePropAssets {
    fn from_city(meshes: &CityMeshes, materials: &CityMaterials) -> Self {
        Self {
            cube: meshes.cube.clone(),
            cylinder: meshes.cylinder.clone(),
            sphere: meshes.sphere.clone(),
            timber: materials.timber.clone(),
            dark_wood: materials.dark_wood.clone(),
            iron: materials.iron.clone(),
            sack_cloth: materials.canvas.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CargoCueKey {
    kind: CargoHandoffKind,
    x_m: i32,
    z_m: i32,
}

impl CargoCueKey {
    fn new(kind: CargoHandoffKind, position: Vec3) -> Self {
        Self {
            kind,
            x_m: position.x.round() as i32,
            z_m: position.z.round() as i32,
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct TradePropRuntime {
    last_handoff_at: HashMap<CargoCueKey, f64>,
}

impl TradePropRuntime {
    fn allow_handoff(&mut self, kind: CargoHandoffKind, position: Vec3, now: f64) -> bool {
        let key = CargoCueKey::new(kind, position);
        let allowed = self
            .last_handoff_at
            .get(&key)
            .is_none_or(|last| now - *last >= CARGO_CUE_COOLDOWN_SECONDS);
        if allowed {
            self.last_handoff_at.insert(key, now);
        }
        self.last_handoff_at
            .retain(|_, last| now >= *last && now - *last <= CARGO_LIFETIME_SECONDS as f64 * 2.0);
        allowed
    }
}

#[derive(Component, Debug)]
pub(super) struct CargoHandoffProp {
    kind: CargoHandoffKind,
    contact: Vec3,
    yaw: f32,
    started_at: f64,
}

impl CargoHandoffProp {
    fn new(kind: CargoHandoffKind, contact: Vec3, started_at: f64) -> Self {
        Self {
            kind,
            contact,
            yaw: cargo_yaw(kind, contact),
            started_at,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CargoPose {
    height: f32,
    pitch: f32,
    roll: f32,
    scale: Vec3,
}

pub(super) fn spawn_weighbeam_rig(
    commands: &mut Commands,
    meshes: &CityMeshes,
    materials: &CityMaterials,
    position: Vec3,
    angle: f32,
) {
    commands.insert_resource(TradePropAssets::from_city(meshes, materials));

    // The existing fourteen-metre timber is the fixed gantry. This moving
    // balance hangs immediately below and slightly forward of it, so its
    // silhouette remains legible from the market floor.
    let pivot =
        position + Vec3::Y * 4.48 + Quat::from_rotation_y(angle) * Vec3::new(0.0, 0.0, 0.38);
    commands
        .spawn((
            Name::new("Tallage balance mechanism"),
            TradeWeighbeamRig::new(position, angle),
            Transform::from_translation(pivot).with_rotation(Quat::from_rotation_y(angle)),
            Visibility::default(),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Tallage moving balance arm"),
                WeighbeamPart::BalanceArm,
                Mesh3d(meshes.cube.clone()),
                MeshMaterial3d(materials.dark_wood.clone()),
                Transform::from_scale(Vec3::new(8.8, 0.24, 0.28)),
            ));
            root.spawn((
                Name::new("Tallage bronze fulcrum"),
                WeighbeamPart::Fulcrum,
                Mesh3d(meshes.cylinder.clone()),
                MeshMaterial3d(materials.bronze.clone()),
                Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(0.23, 0.42, 0.23)),
            ));
            root.spawn((
                Name::new("Tallage balance indicator"),
                WeighbeamPart::Indicator,
                Mesh3d(meshes.pyramid.clone()),
                MeshMaterial3d(materials.bronze.clone()),
                Transform::from_xyz(0.0, -0.68, 0.0)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::PI))
                    .with_scale(Vec3::new(0.13, 1.18, 0.13)),
            ));

            for side in [-1.0_f32, 1.0] {
                let pan_center = Vec3::new(side * 3.55, -1.82, 0.0);
                let chain_top = Vec3::new(side * 3.7, -0.12, 0.0);
                for pan_edge in [
                    Vec3::new(side * 3.55 - 0.55, -1.67, -0.38),
                    Vec3::new(side * 3.55 + 0.55, -1.67, -0.38),
                    Vec3::new(side * 3.55, -1.67, 0.48),
                ] {
                    root.spawn((
                        Name::new("Tallage iron balance chain"),
                        WeighbeamPart::Chain,
                        Mesh3d(meshes.cylinder.clone()),
                        MeshMaterial3d(materials.iron.clone()),
                        segment_transform(chain_top, pan_edge, 0.025),
                    ));
                }
                root.spawn((
                    Name::new("Tallage bronze balance pan"),
                    WeighbeamPart::Pan,
                    Mesh3d(meshes.cylinder.clone()),
                    MeshMaterial3d(materials.bronze.clone()),
                    Transform::from_translation(pan_center)
                        .with_scale(Vec3::new(0.88, 0.075, 0.88)),
                ));
                root.spawn((
                    Name::new("Tallage iron pan rim"),
                    WeighbeamPart::Pan,
                    Mesh3d(meshes.cylinder.clone()),
                    MeshMaterial3d(materials.iron.clone()),
                    Transform::from_translation(pan_center + Vec3::Y * 0.055)
                        .with_scale(Vec3::new(0.94, 0.035, 0.94)),
                ));
            }
        });
}

fn segment_transform(from: Vec3, to: Vec3, radius: f32) -> Transform {
    let delta = to - from;
    Transform::from_translation((from + to) * 0.5)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, delta.normalize()))
        .with_scale(Vec3::new(radius, delta.length(), radius))
}

pub(super) fn handle_trade_cues(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<TradePropAssets>>,
    mut runtime: ResMut<TradePropRuntime>,
    mut cues: MessageReader<SoundscapeCue>,
    mut rigs: Query<&mut TradeWeighbeamRig>,
    cargo: Query<(), With<CargoHandoffProp>>,
) {
    let now = time.elapsed_secs_f64();
    let mut cargo_capacity = MAX_LIVE_CARGO_PROPS.saturating_sub(cargo.iter().count());

    for cue in cues.read().copied() {
        match cue {
            SoundscapeCue::MarketMeasurement { position } if position.is_finite() => {
                if let Some(mut rig) = rigs
                    .iter_mut()
                    .filter(|rig| {
                        rig.anchor.xz().distance_squared(position.xz())
                            <= WEIGHBEAM_CUE_RADIUS_M.powi(2)
                    })
                    .min_by(|a, b| {
                        a.anchor
                            .xz()
                            .distance_squared(position.xz())
                            .total_cmp(&b.anchor.xz().distance_squared(position.xz()))
                    })
                {
                    rig.kick();
                }
            }
            SoundscapeCue::CargoHandoff { position, kind }
                if position.is_finite() && cargo_capacity > 0 =>
            {
                let Some(assets) = assets.as_deref() else {
                    continue;
                };
                if runtime.allow_handoff(kind, position, now) {
                    spawn_cargo_handoff(&mut commands, assets, kind, position, now);
                    cargo_capacity -= 1;
                }
            }
            _ => {}
        }
    }
}

pub(super) fn animate_trade_props(
    mut commands: Commands,
    time: Res<Time>,
    mut rigs: Query<(&mut TradeWeighbeamRig, &mut Transform), Without<CargoHandoffProp>>,
    mut cargo: Query<(Entity, &CargoHandoffProp, &mut Transform), Without<TradeWeighbeamRig>>,
) {
    let dt = time.delta_secs();
    for (mut rig, mut transform) in &mut rigs {
        if !rig.active {
            continue;
        }
        rig.elapsed += dt;
        let tilt = weighbeam_tilt(rig.elapsed, rig.direction);
        transform.rotation = Quat::from_rotation_y(rig.yaw) * Quat::from_rotation_z(tilt);
        if rig.elapsed >= WEIGHBEAM_SETTLE_SECONDS {
            rig.active = false;
            transform.rotation = Quat::from_rotation_y(rig.yaw);
        }
    }

    let now = time.elapsed_secs_f64();
    for (entity, prop, mut transform) in &mut cargo {
        let elapsed = (now - prop.started_at).max(0.0) as f32;
        if elapsed >= CARGO_LIFETIME_SECONDS {
            commands.entity(entity).try_despawn();
            continue;
        }
        let pose = cargo_pose(prop.kind, elapsed);
        let fade = cargo_fade_scale(elapsed);
        transform.translation = prop.contact + Vec3::Y * pose.height;
        transform.rotation = Quat::from_rotation_y(prop.yaw)
            * Quat::from_rotation_x(pose.pitch)
            * Quat::from_rotation_z(pose.roll);
        transform.scale = pose.scale * fade;
    }
}

fn spawn_cargo_handoff(
    commands: &mut Commands,
    assets: &TradePropAssets,
    kind: CargoHandoffKind,
    contact: Vec3,
    started_at: f64,
) {
    let animation = CargoHandoffProp::new(kind, contact, started_at);
    let pose = cargo_pose(kind, 0.0);
    let transform = Transform::from_translation(contact + Vec3::Y * pose.height)
        .with_rotation(Quat::from_rotation_y(animation.yaw));
    let name = match kind {
        CargoHandoffKind::GrainSack => "Lowered grain sack",
        CargoHandoffKind::Crate => "Lowered nailed timber crate",
    };

    commands
        .spawn((Name::new(name), animation, transform, Visibility::default()))
        .with_children(|root| match kind {
            CargoHandoffKind::GrainSack => {
                root.spawn((
                    Name::new("Handoff grain sack"),
                    Mesh3d(assets.sphere.clone()),
                    MeshMaterial3d(assets.sack_cloth.clone()),
                    Transform::from_xyz(0.0, 0.62, 0.0).with_scale(Vec3::new(0.50, 0.62, 0.43)),
                ));
                root.spawn((
                    Name::new("Grain sack neck cord"),
                    Mesh3d(assets.cylinder.clone()),
                    MeshMaterial3d(assets.dark_wood.clone()),
                    Transform::from_xyz(0.0, 1.23, 0.0).with_scale(Vec3::new(0.085, 0.13, 0.085)),
                ));
            }
            CargoHandoffKind::Crate => {
                root.spawn((
                    Name::new("Handoff timber crate body"),
                    Mesh3d(assets.cube.clone()),
                    MeshMaterial3d(assets.timber.clone()),
                    Transform::from_xyz(0.0, 0.43, 0.0).with_scale(Vec3::new(0.96, 0.86, 0.84)),
                ));
                for x in [-0.44_f32, 0.44] {
                    for z in [-0.38_f32, 0.38] {
                        root.spawn((
                            Name::new("Crate corner batten"),
                            Mesh3d(assets.cube.clone()),
                            MeshMaterial3d(assets.dark_wood.clone()),
                            Transform::from_xyz(x, 0.44, z)
                                .with_scale(Vec3::new(0.075, 0.88, 0.075)),
                        ));
                    }
                }
                for y in [0.16_f32, 0.70] {
                    for z in [-0.43_f32, 0.43] {
                        root.spawn((
                            Name::new("Crate face batten"),
                            Mesh3d(assets.cube.clone()),
                            MeshMaterial3d(assets.dark_wood.clone()),
                            Transform::from_xyz(0.0, y, z)
                                .with_scale(Vec3::new(0.90, 0.075, 0.055)),
                        ));
                    }
                    for x in [-0.49_f32, 0.49] {
                        root.spawn((
                            Name::new("Crate side batten"),
                            Mesh3d(assets.cube.clone()),
                            MeshMaterial3d(assets.dark_wood.clone()),
                            Transform::from_xyz(x, y, 0.0)
                                .with_scale(Vec3::new(0.055, 0.075, 0.78)),
                        ));
                    }
                }
                for z in [-0.47_f32, 0.47] {
                    for x in [-0.34_f32, 0.34] {
                        for y in [0.17_f32, 0.69] {
                            root.spawn((
                                Name::new("Crate iron nail"),
                                Mesh3d(assets.sphere.clone()),
                                MeshMaterial3d(assets.iron.clone()),
                                Transform::from_xyz(x, y, z).with_scale(Vec3::splat(0.042)),
                            ));
                        }
                    }
                }
            }
        });
}

const fn cargo_contact_seconds(kind: CargoHandoffKind) -> f32 {
    match kind {
        CargoHandoffKind::GrainSack => SACK_CONTACT_SECONDS,
        CargoHandoffKind::Crate => CRATE_CONTACT_SECONDS,
    }
}

fn cargo_pose(kind: CargoHandoffKind, elapsed: f32) -> CargoPose {
    let elapsed = elapsed.max(0.0);
    let contact = cargo_contact_seconds(kind);
    let drop_height = match kind {
        CargoHandoffKind::GrainSack => 1.25,
        CargoHandoffKind::Crate => 1.60,
    };
    if elapsed < contact {
        let progress = (elapsed / contact).clamp(0.0, 1.0);
        let eased = progress * progress * (3.0 - 2.0 * progress);
        let sway = (progress * std::f32::consts::PI).sin();
        let sway_scale = match kind {
            CargoHandoffKind::GrainSack => 0.10,
            CargoHandoffKind::Crate => 0.055,
        };
        return CargoPose {
            height: drop_height * (1.0 - eased),
            pitch: sway * sway_scale * 0.45,
            roll: -sway * sway_scale,
            scale: Vec3::ONE,
        };
    }

    let settling = elapsed - contact;
    match kind {
        CargoHandoffKind::GrainSack => {
            let bounce = 0.10 * (-5.0 * settling).exp() * (18.0 * settling).sin().abs();
            let compression = (-7.0 * settling).exp() * (16.0 * settling).cos();
            CargoPose {
                height: bounce,
                pitch: 0.035 * (-4.0 * settling).exp() * (13.0 * settling).sin(),
                roll: -0.075 * (-4.5 * settling).exp() * (14.0 * settling).sin(),
                scale: Vec3::new(
                    1.0 + 0.10 * compression,
                    1.0 - 0.16 * compression,
                    1.0 + 0.08 * compression,
                ),
            }
        }
        CargoHandoffKind::Crate => CargoPose {
            height: 0.045 * (-6.0 * settling).exp() * (21.0 * settling).sin().abs(),
            pitch: 0.018 * (-4.5 * settling).exp() * (12.0 * settling).sin(),
            roll: 0.055 * (-4.0 * settling).exp() * (15.0 * settling).sin(),
            scale: Vec3::ONE,
        },
    }
}

fn cargo_fade_scale(elapsed: f32) -> f32 {
    let fade_start = CARGO_LIFETIME_SECONDS - CARGO_FADE_SECONDS;
    if elapsed <= fade_start {
        return 1.0;
    }
    let remaining = ((CARGO_LIFETIME_SECONDS - elapsed) / CARGO_FADE_SECONDS).clamp(0.0, 1.0);
    remaining * remaining * (3.0 - 2.0 * remaining)
}

fn cargo_yaw(kind: CargoHandoffKind, position: Vec3) -> f32 {
    let kind_salt = match kind {
        CargoHandoffKind::GrainSack => 0x9e37_79b9,
        CargoHandoffKind::Crate => 0x85eb_ca6b,
    };
    let mut bits = position.x.to_bits() ^ position.z.to_bits().rotate_left(13) ^ kind_salt;
    bits ^= bits >> 16;
    bits = bits.wrapping_mul(0x7feb_352d);
    bits ^= bits >> 15;
    bits as f32 / u32::MAX as f32 * TAU
}

fn weighbeam_tilt(elapsed: f32, direction: f32) -> f32 {
    if !(0.0..WEIGHBEAM_SETTLE_SECONDS).contains(&elapsed) {
        return 0.0;
    }
    direction.signum()
        * WEIGHBEAM_AMPLITUDE_RADIANS
        * (-WEIGHBEAM_DAMPING * elapsed).exp()
        * (WEIGHBEAM_FREQUENCY * elapsed).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.000_01
    }

    #[test]
    fn cargo_reaches_the_authored_contact_on_the_documented_delay() {
        for (kind, contact) in [
            (CargoHandoffKind::GrainSack, 0.20),
            (CargoHandoffKind::Crate, 0.48),
        ] {
            assert!(close(cargo_contact_seconds(kind), contact));
            assert!(cargo_pose(kind, contact - 0.001).height > 0.0);
            assert!(close(cargo_pose(kind, contact).height, 0.0));
        }
    }

    #[test]
    fn cargo_contact_bounce_decays_and_cleanup_is_bounded() {
        for kind in [CargoHandoffKind::GrainSack, CargoHandoffKind::Crate] {
            let contact = cargo_contact_seconds(kind);
            let early = cargo_pose(kind, contact + 0.05).height;
            let late = cargo_pose(kind, contact + 1.25).height;
            assert!(early > late, "{kind:?}: {early} should exceed {late}");
            assert!(cargo_pose(kind, contact + 1.25).scale.is_finite());
        }
        assert!(close(cargo_fade_scale(0.0), 1.0));
        assert!(close(cargo_fade_scale(CARGO_LIFETIME_SECONDS), 0.0));
    }

    #[test]
    fn weighbeam_oscillation_is_damped_directional_and_settles_to_level() {
        let early = weighbeam_tilt(0.12, 1.0);
        let late = weighbeam_tilt(1.20, 1.0);
        assert!(early.abs() > late.abs());
        assert!(close(early, -weighbeam_tilt(0.12, -1.0)));
        assert!(close(weighbeam_tilt(0.0, 1.0), 0.0));
        assert!(close(weighbeam_tilt(WEIGHBEAM_SETTLE_SECONDS, 1.0), 0.0));
    }
}
