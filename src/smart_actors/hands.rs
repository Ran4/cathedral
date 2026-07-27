//! Hands: carried and offered item props (`features/npc_bodies.md` §6, M2).
//!
//! The successor of the retired above-head offer fan. The first held
//! non-currency item rides the LEFT hand anchor (the basket-carry), the oldest
//! standing offer's item rides the RIGHT hand at the end of the extended arm
//! (`body.rs` L2 owns that arm), and the consent dance the sim already runs —
//! accept / decline / retract — plus the silent `sale` hand-over
//! choreography: a short prop flight between hands and a habit-tier nod or
//! head-shake. Everything here is presentation: props reconcile from the
//! authoritative snapshot exactly like the fan did (no command intent removes
//! one), and the flights are keyed on world events the sim already emitted.
//!
//! One reach here is not a hand-over at all: custody's grip
//! (`features/law_and_order.md` M4c). It borrows the same extended arm, but it
//! is a *state* rather than a beat — the hand stays on the prisoner's upper arm,
//! tracking them, until the law lets go.

use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use cathedral_sim::custody::CUSTODY_REACH_M;

use super::body::{self, BodyPoseState, BodySide, HandAnchor, OneShotGesture};
use super::custody::PlayerCustodyState;
use super::model::{ActorControl, ActorId, ActorSnapshot, ItemId, WorldMirror};
use crate::controller::{PhysicalPosition, PlayerController};

/// How long a hand-over prop flies giver-hand → recipient-hand (§6: ~0.3 s).
const HANDOVER_FLIGHT_SECONDS: f64 = 0.3;
/// The flight's small upward arc, so a pass reads as a toss-and-take.
const FLIGHT_ARC_M: f32 = 0.12;
/// Prop offset below the hand anchor, so the item sits in the palm rather
/// than on the wrist.
const PROP_IN_HAND_Y: f32 = -0.05;
/// Where a flight lands on an actor with no body (the player is a disembodied
/// camera): chest height over the mirror position.
const HANDLESS_HAND_HEIGHT_M: f32 = 0.35;
/// Where an untargeted "to anyone" offer aims: a point this far ahead.
const OPEN_OFFER_AHEAD_M: f32 = 1.5;
/// How far below the shoulder joint a hand lands when the law takes hold: the
/// top of the upper arm, which is where somebody actually grips you — not the
/// wrist, and not the chest a hand-over aims at (`law_and_order.md` M4c).
const GRIP_BELOW_SHOULDER_M: f32 = 0.10;
/// How far the pair may drift before a held arm gives up. Every way a hold ends
/// *without* a world event — the sim's dead-man timer, a station's four
/// minutes, an escort who left the city — leaves the two of them standing apart,
/// and an arm cannot follow past its own length ([`CUSTODY_REACH_M`], with a
/// pace of slack so an ordinary step never flickers it).
const GRIP_BREAKS_AT_M: f32 = CUSTODY_REACH_M as f32 + 1.0;

/// A renderer-only prop parented to one hand anchor.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct HeldProp {
    pub actor: ActorId,
    pub side: BodySide,
    pub item_id: ItemId,
    pub visual_key: String,
}

/// A prop in flight between two hands after a hand-over event.
#[derive(Component, Debug, Clone)]
pub(crate) struct HandoverFlight {
    item_id: ItemId,
    to_actor: ActorId,
    from: Vec3,
    started_at: f64,
}

/// One world event the hands choreograph, forwarded from the engine drain
/// (`process_engine_message`'s WorldEvent arm). The HUD toast text for these
/// events is produced there and stays untouched.
#[derive(Message, Debug, Clone)]
pub(crate) enum HandoverFeedback {
    /// `accept_offered_item`: the prop flies giver-hand → recipient-hand and
    /// the recipient nods.
    Accepted {
        giver: ActorId,
        recipient: ActorId,
        item: ItemId,
    },
    /// `decline_offer`: the decliner shakes their head; the giver's arm
    /// retracts on its own as the offer leaves the snapshot.
    Declined { decliner: ActorId, giver: ActorId },
    /// `sale`: the silent purchase — vendor arm pulse, prop flight,
    /// buyer nod. Same choreography, no standing offer behind it.
    StallSale {
        vendor: ActorId,
        buyer: ActorId,
        item: ItemId,
    },
    /// `grab` (`law_and_order.md` M4c): the only reach in this file that is not
    /// a beat. The holder's arm goes to the prisoner's upper arm and **stays**
    /// there, tracking them, for as long as the law has hold.
    TookHold { holder: ActorId, prisoner: ActorId },
    /// `release` / `broke_free`: every hand comes off at once. The sim's hold is
    /// refcounted per holder, but both of the events that reach here end the
    /// custody itself, so there is no half-let-go to present.
    HandsOff { prisoner: ActorId },
}

/// Who has a hand on whom, keyed on the holder — the presentation twin of the
/// sim's `CustodyRecord::holders` (`law_and_order.md` M4c).
///
/// Unlike every other hand state in this file it is fed by events rather than
/// reconciled from a snapshot, and that is forced: custody is projected to the
/// host for the **player** only ([`PlayerCustodyState`]), so an officer taking
/// hold of an NPC exists in no snapshot the mirror ever sees. The arm therefore
/// learns of a grip from the `grab` world event, and lets go on `release`, on
/// `broke_free`, or when the two of them are simply too far apart to be holding
/// each other ([`GRIP_BREAKS_AT_M`]) — which is the backstop for the release
/// paths the sim takes without saying anything (its dead-man timer, the station
/// cap, arrival).
#[derive(Resource, Debug, Default)]
pub(crate) struct GripHolds {
    by_holder: HashMap<ActorId, ActorId>,
}

impl GripHolds {
    fn took_hold(&mut self, holder: &ActorId, prisoner: &ActorId) {
        self.by_holder.insert(holder.clone(), prisoner.clone());
    }

    /// Every hand off one person. A holder with no hand on anybody leaves the
    /// map entirely, so "is anybody being held" stays one `is_empty` call.
    fn hands_off(&mut self, prisoner: &ActorId) {
        self.by_holder.retain(|_, held| held != prisoner);
    }

    fn is_empty(&self) -> bool {
        self.by_holder.is_empty()
    }
}

/// The shared item-prop meshes and palette (moved out of the offer fan).
#[derive(Resource)]
pub(crate) struct ItemPropAssets {
    fish_body_mesh: Handle<Mesh>,
    fish_tail_mesh: Handle<Mesh>,
    coin_mesh: Handle<Mesh>,
    generic_item_mesh: Handle<Mesh>,
    fish: Handle<StandardMaterial>,
    fish_fin: Handle<StandardMaterial>,
    copper: Handle<StandardMaterial>,
    generic_item: Handle<StandardMaterial>,
    apple_red: Handle<StandardMaterial>,
    pale_wax: Handle<StandardMaterial>,
    parchment: Handle<StandardMaterial>,
}

/// Creates the bounded shared handles every hand prop draws from. The body
/// itself draws from [`super::body::BodyAssets`].
pub(crate) fn setup_item_prop_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let matte = |color: Color| StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.78,
        ..default()
    };

    commands.insert_resource(ItemPropAssets {
        fish_body_mesh: meshes.add(Sphere::new(1.0).mesh().uv(16, 10)),
        fish_tail_mesh: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4)),
        coin_mesh: meshes.add(Cylinder::new(0.20, 0.055)),
        generic_item_mesh: meshes.add(Cuboid::new(0.30, 0.30, 0.30)),
        fish: materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.61, 0.64),
            metallic: 0.12,
            perceptual_roughness: 0.42,
            ..default()
        }),
        fish_fin: materials.add(matte(Color::srgb(0.20, 0.39, 0.45))),
        copper: materials.add(StandardMaterial {
            base_color: Color::srgb(0.76, 0.34, 0.12),
            metallic: 0.72,
            perceptual_roughness: 0.30,
            ..default()
        }),
        generic_item: materials.add(matte(Color::srgb(0.74, 0.66, 0.24))),
        apple_red: materials.add(matte(Color::srgb(0.62, 0.22, 0.14))),
        pale_wax: materials.add(matte(Color::srgb(0.85, 0.80, 0.62))),
        parchment: materials.add(matte(Color::srgb(0.82, 0.76, 0.60))),
    });
}

/// Spawns the prop hierarchy for one visual key under a fresh root and returns
/// it. The same prop vocabulary the offer fan used — fish (two parts), coin,
/// loaf, stew, generic — every mesh and material a shared handle, every mesh
/// carrying the crowd fade so a distant prop leaves the render with its owner.
fn spawn_prop_root(
    commands: &mut Commands,
    assets: &ItemPropAssets,
    name: String,
    visual_key: &str,
    transform: Transform,
) -> Entity {
    let fade = body::crowd_fade();
    let root = commands
        .spawn((Name::new(name), transform, Visibility::default()))
        .id();
    commands
        .entity(root)
        .with_children(|prop| match visual_key {
            "fish" => {
                prop.spawn((
                    Name::new("Fish body"),
                    Mesh3d(assets.fish_body_mesh.clone()),
                    MeshMaterial3d(assets.fish.clone()),
                    Transform::from_scale(Vec3::new(0.30, 0.13, 0.12)),
                    fade.clone(),
                ));
                prop.spawn((
                    Name::new("Fish tail"),
                    Mesh3d(assets.fish_tail_mesh.clone()),
                    MeshMaterial3d(assets.fish_fin.clone()),
                    Transform::from_xyz(-0.31, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_z(-FRAC_PI_2))
                        .with_scale(Vec3::new(0.13, 0.19, 0.13)),
                    fade.clone(),
                ));
            }
            "copper_coin" | "coin" => {
                prop.spawn((
                    Name::new("Coin"),
                    Mesh3d(assets.coin_mesh.clone()),
                    MeshMaterial3d(assets.copper.clone()),
                    Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2))
                        .with_scale(Vec3::splat(0.7)),
                    fade.clone(),
                ));
            }
            "loaf" => {
                // A flattened brown block — a loaf on the palm, life-size now
                // that it sits in a hand instead of floating overhead.
                prop.spawn((
                    Name::new("Loaf"),
                    Mesh3d(assets.generic_item_mesh.clone()),
                    MeshMaterial3d(assets.generic_item.clone()),
                    Transform::from_scale(Vec3::new(0.90, 0.45, 0.60)),
                    fade.clone(),
                ));
            }
            "stew" | "ale_pot" | "cup" | "bowl" => {
                // A squat cylinder standing in for a bowl, pot or cup.
                prop.spawn((
                    Name::new("Bowl of stew"),
                    Mesh3d(assets.coin_mesh.clone()),
                    MeshMaterial3d(assets.generic_item.clone()),
                    Transform::from_scale(Vec3::new(0.9, 0.7, 0.9)),
                    fade.clone(),
                ));
            }
            "apple" => {
                prop.spawn((
                    Name::new("Apple"),
                    Mesh3d(assets.fish_body_mesh.clone()),
                    MeshMaterial3d(assets.apple_red.clone()),
                    Transform::from_scale(Vec3::splat(0.05)),
                    fade.clone(),
                ));
            }
            "candle" => {
                // A thin upright stick of wax — the carried light of the
                // Wickmarket chandlers (unlit; lamplight stays the lamps' job).
                prop.spawn((
                    Name::new("Candle"),
                    Mesh3d(assets.coin_mesh.clone()),
                    MeshMaterial3d(assets.pale_wax.clone()),
                    Transform::from_scale(Vec3::new(0.12, 3.6, 0.12)),
                    fade.clone(),
                ));
            }
            "letter" | "page" | "book" => {
                // A flat parchment leaf on the palm.
                prop.spawn((
                    Name::new("Paper"),
                    Mesh3d(assets.generic_item_mesh.clone()),
                    MeshMaterial3d(assets.parchment.clone()),
                    Transform::from_scale(Vec3::new(0.75, 0.06, 1.0)),
                    fade.clone(),
                ));
            }
            _ => {
                prop.spawn((
                    Name::new("Generic held item"),
                    Mesh3d(assets.generic_item_mesh.clone()),
                    MeshMaterial3d(assets.generic_item.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(0.28))
                        .with_scale(Vec3::splat(0.7)),
                    fade.clone(),
                ));
            }
        });
    root
}

/// One hand's desired content: the item and its prop vocabulary key.
type HandContent = Option<(ItemId, String)>;

/// How many units of `item_id` this actor has pocketed
/// (`features/extra_pockets.md`). One entry per unit, so a stack of two with
/// both cheeks full counts 2.
fn pocketed_units(actor: &ActorSnapshot, item_id: &ItemId) -> u32 {
    actor
        .pockets
        .iter()
        .filter(|(_, pocketed)| pocketed == item_id)
        .count()
        .min(u32::MAX as usize) as u32
}

/// What each hand of one actor should hold, from the authoritative snapshot:
/// the oldest standing offer's item in the RIGHT hand (the rest of several
/// simultaneous offers exist only in text — accepted loss, §6), and the first
/// held non-currency item that is not that offer in the LEFT. Spark stacks
/// never render as carry — the whole cast holds a wallet, and nobody walks
/// around with their purse in their fist. A stack whose every unit is pocketed
/// is out of sight by definition (`features/extra_pockets.md`: "others see
/// nothing while an item is pocketed"), so it renders no carry prop either; a
/// partially-pocketed stack still has units in the open and does.
fn desired_hand_props(mirror: &WorldMirror, actor: &ActorSnapshot) -> (HandContent, HandContent) {
    let offered = mirror
        .offers()
        .filter(|offer| offer.giver_id == actor.id)
        .min_by(|a, b| {
            a.created_seq
                .cmp(&b.created_seq)
                .then_with(|| a.item_id.0.cmp(&b.item_id.0))
        })
        .map(|offer| offer.item_id.clone());
    let right = offered.as_ref().and_then(|item_id| {
        mirror
            .item(item_id)
            .map(|item| (item_id.clone(), item.visual_key.clone()))
    });
    let left = actor.holds.iter().find_map(|item_id| {
        if Some(item_id) == offered.as_ref() {
            return None;
        }
        let item = mirror.item(item_id)?;
        if item.kind == "spark" {
            return None;
        }
        if pocketed_units(actor, item_id) >= item.quantity {
            return None;
        }
        Some((item_id.clone(), item.visual_key.clone()))
    });
    (left, right)
}

/// The offer fan's Create/Keep/Replace idea, kept for the in-hand prop: a
/// changed item or look replaces the entity, anything else keeps it, so an
/// unchanged snapshot never respawns a prop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropDisposition {
    Create,
    Keep,
    Replace,
}

fn prop_disposition(
    current: Option<&HeldProp>,
    item_id: &ItemId,
    visual_key: &str,
) -> PropDisposition {
    match current {
        None => PropDisposition::Create,
        Some(current) if current.item_id != *item_id || current.visual_key != visual_key => {
            PropDisposition::Replace
        }
        Some(_) => PropDisposition::Keep,
    }
}

/// Where the extended offer arm aims for one giver: the recipient's mirror
/// position, or a point ahead of the giver for an open "to anyone" offer.
fn offer_aim_point(mirror: &WorldMirror, actor: &ActorSnapshot) -> Option<Vec3> {
    let offer = mirror
        .offers()
        .filter(|offer| offer.giver_id == actor.id)
        .min_by(|a, b| {
            a.created_seq
                .cmp(&b.created_seq)
                .then_with(|| a.item_id.0.cmp(&b.item_id.0))
        })?;
    match offer.target_id.as_ref().and_then(|id| mirror.actor(id)) {
        Some(target) => Some(target.position_m.into()),
        None => {
            let position: Vec3 = actor.position_m.into();
            let ahead = Quat::from_rotation_y(actor.facing_yaw) * (Vec3::NEG_Z * OPEN_OFFER_AHEAD_M);
            Some(position + ahead)
        }
    }
}

/// Reconciles one prop per occupied hand against the latest snapshot and
/// feeds the L2 activity targets (carry / offer aim) to the pose state.
///
/// No command intent calls this system directly, so accepting, declining, or
/// retracting an offer cannot make a prop disappear before the engine
/// confirms it — the fan's rule, inherited unchanged.
pub(crate) fn reconcile_hand_props(
    mut commands: Commands,
    mirror: Res<WorldMirror>,
    assets: Res<ItemPropAssets>,
    anchors: Query<(Entity, &HandAnchor)>,
    props: Query<(Entity, &HeldProp)>,
    flights: Query<&HandoverFlight>,
    mut removed_flights: RemovedComponents<HandoverFlight>,
    mut poses: Query<(&ActorId, &mut BodyPoseState)>,
) {
    // Hand contents only change with a snapshot or around a handover flight
    // (its 0.3 s of existence, plus the landing frame where the flight
    // component disappears). Every other frame this reconcile would rebuild
    // three N-entry maps and re-flag every pose just to conclude nothing moved.
    if !mirror.is_changed() && flights.is_empty() && removed_flights.read().next().is_none() {
        return;
    }
    // While a hand-over prop is still flying, the receiving hand stays empty —
    // reconcile would otherwise double the item for the flight's 0.3 s.
    let in_flight: HashSet<(ActorId, ItemId)> = flights
        .iter()
        .map(|flight| (flight.to_actor.clone(), flight.item_id.clone()))
        .collect();
    let anchor_by_hand: HashMap<(ActorId, BodySide), Entity> = anchors
        .iter()
        .map(|(entity, anchor)| ((anchor.actor.clone(), anchor.side), entity))
        .collect();

    let mut desired: HashMap<(ActorId, BodySide), (ItemId, String)> = HashMap::new();
    let mut activity: HashMap<ActorId, (bool, Option<Vec3>)> = HashMap::new();
    for actor in mirror
        .actors()
        .filter(|actor| actor.control == ActorControl::Llm)
    {
        let (left, right) = desired_hand_props(&mirror, actor);
        // The carry arm starts posing while the flight is inbound; only the
        // prop itself waits for the landing.
        let carrying = left.is_some();
        if let Some((item_id, visual_key)) = left
            && !in_flight.contains(&(actor.id.clone(), item_id.clone()))
        {
            desired.insert((actor.id.clone(), BodySide::Left), (item_id, visual_key));
        }
        let offer_at = right
            .is_some()
            .then(|| offer_aim_point(&mirror, actor))
            .flatten();
        if let Some((item_id, visual_key)) = right {
            desired.insert((actor.id.clone(), BodySide::Right), (item_id, visual_key));
        }
        activity.insert(actor.id.clone(), (carrying, offer_at));
    }

    for (actor_id, mut pose) in &mut poses {
        let (carry, offer_at) = activity
            .get(actor_id)
            .copied()
            .unwrap_or((false, None));
        pose.set_hand_activity(carry, offer_at);
    }

    let mut existing: HashMap<(ActorId, BodySide), Entity> = HashMap::new();
    for (entity, prop) in &props {
        let key = (prop.actor.clone(), prop.side);
        match desired.get(&key) {
            Some((item_id, visual_key))
                if prop_disposition(Some(prop), item_id, visual_key) == PropDisposition::Keep =>
            {
                existing.insert(key, entity);
            }
            // Replace and stale alike: the snapshot no longer backs this prop.
            _ => {
                commands.entity(entity).despawn();
            }
        }
    }

    for ((actor, side), (item_id, visual_key)) in desired {
        if existing.contains_key(&(actor.clone(), side)) {
            continue;
        }
        let Some(anchor) = anchor_by_hand.get(&(actor.clone(), side)).copied() else {
            continue; // no body (the player), or the rig is not spawned yet
        };
        let root = spawn_prop_root(
            &mut commands,
            &assets,
            format!("Held item: {}", item_id.0),
            &visual_key,
            Transform::from_xyz(0.0, PROP_IN_HAND_Y, 0.0),
        );
        commands.entity(root).insert((
            HeldProp {
                actor,
                side,
                item_id,
                visual_key,
            },
            ChildOf(anchor),
        ));
    }
}

/// Plays the hand-over feedback the engine drain forwarded: prop flights on
/// accept and stall sales, the habit-tier nod / head-shake, and the vendor's
/// arm pulse. Runs after `reconcile_actor_views` (bodies exist) and before
/// `reconcile_hand_props` (the giver's prop is still in hand to launch from).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_handover_feedback(
    mut commands: Commands,
    time: Res<Time>,
    mirror: Res<WorldMirror>,
    assets: Res<ItemPropAssets>,
    mut feedback: MessageReader<HandoverFeedback>,
    mut grips: ResMut<GripHolds>,
    props: Query<(Entity, &HeldProp, &GlobalTransform)>,
    anchors: Query<(&HandAnchor, &GlobalTransform)>,
    mut poses: Query<(&ActorId, &mut BodyPoseState)>,
) {
    let now = time.elapsed_secs_f64();
    for event in feedback.read() {
        match event {
            HandoverFeedback::Accepted {
                giver,
                recipient,
                item,
            } => {
                launch_flight(
                    &mut commands, &assets, &mirror, &props, &anchors, giver, recipient, item, now,
                );
                let face = chest_point(&mirror, giver);
                start_gesture_for(&mut poses, recipient, OneShotGesture::Nod, face, now);
            }
            HandoverFeedback::Declined { decliner, giver } => {
                let face = chest_point(&mirror, giver);
                start_gesture_for(&mut poses, decliner, OneShotGesture::ShakeHead, face, now);
            }
            HandoverFeedback::StallSale {
                vendor,
                buyer,
                item,
            } => {
                // The vendor has no standing offer to key the arm on; pulse it
                // toward the buyer for the hand-over beat.
                if let Some(at) = chest_point(&mirror, buyer) {
                    pulse_offer_for(&mut poses, vendor, at, now);
                }
                launch_flight(
                    &mut commands, &assets, &mirror, &props, &anchors, vendor, buyer, item, now,
                );
                let face = chest_point(&mirror, vendor);
                start_gesture_for(&mut poses, buyer, OneShotGesture::Nod, face, now);
            }
            // The hold itself is stateful, so these two only move the register;
            // the arm is aimed every frame by `hold_the_seized`, which is where
            // a walking escort is tracked and a broken hold is noticed.
            HandoverFeedback::TookHold { holder, prisoner } => {
                grips.took_hold(holder, prisoner);
            }
            HandoverFeedback::HandsOff { prisoner } => {
                grips.hands_off(prisoner);
            }
        }
    }
}

/// The visible hand (`law_and_order.md` M4c): the holder's arm on the
/// prisoner's upper arm, held there for as long as the law has hold of them.
///
/// It runs every frame while anybody is held, and only then — a grip has to
/// track a prisoner being walked to a station, and the positions that are
/// current between snapshots are the live body transforms the hot movement
/// channel drives (`actors::drive_npc_bodies`), never the mirror's. The player
/// has no body at all, so their own held arm is read from the controller's
/// authoritative position, which is also what the tether clamps.
///
/// The two grip sources are deliberately different in kind: the player's comes
/// from [`PlayerCustodyState`], which the sim republishes on every change and is
/// therefore never stale, and the cast's from [`GripHolds`], which is all the
/// host is told about an arrest it is not part of.
pub(crate) fn hold_the_seized(
    mut grips: ResMut<GripHolds>,
    law: Option<Res<PlayerCustodyState>>,
    mirror: Option<Res<WorldMirror>>,
    player: Option<Single<&PhysicalPosition, With<PlayerController>>>,
    bodies: Query<(&ActorId, &Transform)>,
    mut poses: Query<(&ActorId, &mut BodyPoseState)>,
    mut holding: Local<bool>,
) {
    let player_id = mirror.as_ref().and_then(|mirror| mirror.player_id().cloned());
    let player_at = player.map(|position| position.current);
    let mut aims: HashMap<ActorId, Vec3> = HashMap::new();

    // The player's holders first: the sim's own word, and the case M4c exists
    // for. Their prisoner is always the player.
    if let Some(law) = law.as_ref()
        && let Some(custody) = law.custody.as_ref().filter(|custody| custody.held)
        && let Some(at) = player_at.map(upper_arm_of)
    {
        for holder in &custody.holder_ids {
            aims.insert(holder.clone(), at);
        }
    }
    // Then the cast's own arrests, dropping any pair that is no longer close
    // enough to be holding each other at all.
    if !grips.is_empty() {
        let mut broken: Vec<ActorId> = Vec::new();
        for (holder, prisoner) in &grips.by_holder {
            let held_at = if Some(prisoner) == player_id.as_ref() {
                player_at
            } else {
                body_position(&bodies, prisoner)
            };
            let holder_at = body_position(&bodies, holder);
            match (holder_at, held_at) {
                (Some(holder_at), Some(held_at))
                    if holder_at.distance(held_at) <= GRIP_BREAKS_AT_M =>
                {
                    aims.insert(holder.clone(), upper_arm_of(held_at));
                }
                // Out of reach, or one of them is no longer rendered: either way
                // there is no arm left to draw.
                _ => broken.push(prisoner.clone()),
            }
        }
        for prisoner in broken {
            grips.hands_off(&prisoner);
        }
    }

    // Sweeping every pose costs an iteration over the whole visible cast, so it
    // happens only while a hold is live — and once more on the frame the last
    // one ends, to put the arm down.
    let any = !aims.is_empty();
    if !any && !*holding {
        return;
    }
    *holding = any;
    for (actor_id, mut pose) in poses.iter_mut() {
        pose.set_grip(aims.get(actor_id).copied());
    }
}

/// The grip point on one body: just below the shoulder joint, in world space.
fn upper_arm_of(position: Vec3) -> Vec3 {
    position + Vec3::Y * (body::SHOULDER_ROOT_Y - GRIP_BELOW_SHOULDER_M)
}

/// Where a body actually is this frame, from its own (parentless) root
/// transform rather than the mirror snapshot the movement channel has already
/// overtaken.
fn body_position(bodies: &Query<(&ActorId, &Transform)>, id: &ActorId) -> Option<Vec3> {
    bodies
        .iter()
        .find(|(actor_id, _)| *actor_id == id)
        .map(|(_, transform)| transform.translation)
}

/// Chest height over an actor's mirror position — the gaze/flight fallback
/// that also works for the bodiless player.
fn chest_point(mirror: &WorldMirror, id: &ActorId) -> Option<Vec3> {
    mirror
        .actor(id)
        .map(|actor| Vec3::from(actor.position_m) + Vec3::Y * HANDLESS_HAND_HEIGHT_M)
}

fn start_gesture_for(
    poses: &mut Query<(&ActorId, &mut BodyPoseState)>,
    id: &ActorId,
    kind: OneShotGesture,
    face: Option<Vec3>,
    now: f64,
) {
    for (actor, mut pose) in poses.iter_mut() {
        if actor == id {
            pose.start_gesture(kind, face, now);
            return;
        }
    }
}

fn pulse_offer_for(
    poses: &mut Query<(&ActorId, &mut BodyPoseState)>,
    id: &ActorId,
    at: Vec3,
    now: f64,
) {
    for (actor, mut pose) in poses.iter_mut() {
        if actor == id {
            pose.pulse_offer(at, now);
            return;
        }
    }
}

/// Detach the giver's in-hand prop (or conjure one at the giving hand) and
/// send it flying toward the recipient's receiving hand.
#[allow(clippy::too_many_arguments)]
fn launch_flight(
    commands: &mut Commands,
    assets: &ItemPropAssets,
    mirror: &WorldMirror,
    props: &Query<(Entity, &HeldProp, &GlobalTransform)>,
    anchors: &Query<(&HandAnchor, &GlobalTransform)>,
    giver: &ActorId,
    recipient: &ActorId,
    item: &ItemId,
    now: f64,
) {
    // Launch point, best first: the actual offered prop in the giver's right
    // hand, the giver's right hand anchor, the giver's mirror position (the
    // player has no body but can still give things).
    let mut visual_key = mirror
        .item(item)
        .map(|item| item.visual_key.clone())
        .unwrap_or_else(|| "generic".into());
    let mut from = None;
    if let Some((entity, prop, global)) = props
        .iter()
        .find(|(_, prop, _)| prop.actor == *giver && prop.item_id == *item)
    {
        from = Some(global.translation());
        visual_key.clone_from(&prop.visual_key);
        commands.entity(entity).despawn();
    }
    let from = from
        .or_else(|| {
            anchors
                .iter()
                .find(|(anchor, _)| anchor.actor == *giver && anchor.side == BodySide::Right)
                .map(|(_, global)| global.translation())
        })
        .or_else(|| {
            mirror
                .actor(giver)
                .map(|actor| Vec3::from(actor.position_m) + Vec3::Y * HANDLESS_HAND_HEIGHT_M)
        });
    let Some(from) = from else {
        return;
    };
    let root = spawn_prop_root(
        commands,
        assets,
        format!("Handover: {}", item.0),
        &visual_key,
        Transform::from_translation(from),
    );
    commands.entity(root).insert(HandoverFlight {
        item_id: item.clone(),
        to_actor: recipient.clone(),
        from,
        started_at: now,
    });
}

/// Sweeps every in-flight prop along its giver-hand → recipient-hand arc and
/// retires it on landing (the reconciled carry prop takes over from there).
pub(crate) fn animate_handover_flights(
    mut commands: Commands,
    time: Res<Time>,
    mirror: Res<WorldMirror>,
    anchors: Query<(&HandAnchor, &GlobalTransform)>,
    mut flights: Query<(Entity, &HandoverFlight, &mut Transform)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, flight, mut transform) in &mut flights {
        let t = ((now - flight.started_at) / HANDOVER_FLIGHT_SECONDS) as f32;
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // The receiving left hand tracked live; a bodiless recipient (the
        // player) catches at chest height over their mirror position.
        let target = anchors
            .iter()
            .find(|(anchor, _)| {
                anchor.actor == flight.to_actor && anchor.side == BodySide::Left
            })
            .map(|(_, global)| global.translation())
            .or_else(|| {
                mirror
                    .actor(&flight.to_actor)
                    .map(|actor| Vec3::from(actor.position_m) + Vec3::Y * HANDLESS_HAND_HEIGHT_M)
            });
        let Some(target) = target else {
            commands.entity(entity).despawn();
            continue;
        };
        let eased = t * t * (3.0 - 2.0 * t);
        transform.translation =
            flight.from.lerp(target, eased) + Vec3::Y * (FLIGHT_ARC_M * (PI * t).sin());
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;
    use crate::smart_actors::actors::reconcile_actor_views;
    use crate::smart_actors::model::{
        ActorControl, ActorSnapshot, ItemSnapshot, OfferSnapshot, Position, WorldSnapshot,
    };

    fn item(id: &str, kind: &str, visual_key: &str) -> ItemSnapshot {
        ItemSnapshot {
            id: ItemId(id.into()),
            kind: kind.into(),
            name: id.into(),
            display_plural: format!("{id}s"),
            visual_key: visual_key.into(),
            quantity: 1,
            metadata: Default::default(),
        }
    }

    fn actor(id: &str, holds: &[&str]) -> ActorSnapshot {
        ActorSnapshot {
            id: ActorId(id.into()),
            name_for_player: id.into(),
            control: ActorControl::Llm,
            position_m: Position::new(0.0, 0.91, 0.0).unwrap(),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: holds.iter().map(|held| ItemId((*held).into())).collect(),
            active_gesture: None,
            statuses: Vec::new(),
            pockets: Vec::new(),
        }
    }

    /// `actor`, plus `count` units of `item` tucked into the cheek
    /// (`features/extra_pockets.md`: one pocket entry per stack-unit).
    fn actor_pocketing(id: &str, holds: &[&str], item: &str, count: usize) -> ActorSnapshot {
        let mut snapshot = actor(id, holds);
        snapshot.pockets = (0..count)
            .map(|_| (cathedral_sim::BodySlot::Mouth, ItemId(item.into())))
            .collect();
        snapshot
    }

    fn stack(id: &str, kind: &str, visual_key: &str, quantity: u32) -> ItemSnapshot {
        ItemSnapshot {
            quantity,
            ..item(id, kind, visual_key)
        }
    }

    fn mirror_with(
        actors: Vec<ActorSnapshot>,
        items: Vec<ItemSnapshot>,
        offers: Vec<OfferSnapshot>,
    ) -> WorldMirror {
        let mut player = actor("player", &[]);
        player.control = ActorControl::Player;
        let mut all = vec![player];
        all.extend(actors);
        let mut mirror = WorldMirror::default();
        mirror
            .replace_snapshot(WorldSnapshot {
                world_revision: 1,
                player_id: ActorId("player".into()),
                actors: all,
                items,
                offers,
                road_carts: vec![],
            })
            .unwrap();
        mirror
    }

    /// §6's hand assignment: the offered item sits in the RIGHT hand, the
    /// carry falls to the first held item that is neither the offer nor a
    /// spark wallet.
    #[test]
    fn offered_item_takes_the_right_hand_and_carry_skips_it() {
        let mirror = mirror_with(
            vec![actor("ilse", &["wallet", "fish", "loaf"])],
            vec![
                item("wallet", "spark", "copper_coin"),
                item("fish", "herring", "fish"),
                item("loaf", "loaf", "loaf"),
            ],
            vec![OfferSnapshot {
                item_id: ItemId("fish".into()),
                giver_id: ActorId("ilse".into()),
                target_id: Some(ActorId("player".into())),
                created_seq: 5,
            }],
        );
        let snapshot = mirror.actor(&ActorId("ilse".into())).unwrap().clone();
        let (left, right) = desired_hand_props(&mirror, &snapshot);
        assert_eq!(right, Some((ItemId("fish".into()), "fish".into())));
        assert_eq!(
            left,
            Some((ItemId("loaf".into()), "loaf".into())),
            "carry skips the wallet and the offered fish"
        );
    }

    /// Concealment (`features/extra_pockets.md`): a stack with every unit
    /// pocketed shows no carry prop — that is the entire point of a cheek —
    /// while a stack with units still in the open keeps its prop.
    #[test]
    fn a_fully_pocketed_stack_renders_no_carry_prop() {
        let hidden = mirror_with(
            vec![actor_pocketing("ilse", &["loaf"], "loaf", 1)],
            vec![item("loaf", "loaf", "loaf")],
            vec![],
        );
        let snapshot = hidden.actor(&ActorId("ilse".into())).unwrap().clone();
        let (left, right) = desired_hand_props(&hidden, &snapshot);
        assert_eq!(left, None, "a pocketed loaf is out of sight");
        assert_eq!(right, None);

        let partial = mirror_with(
            vec![actor_pocketing("ilse", &["herrings"], "herrings", 1)],
            vec![stack("herrings", "herring", "fish", 2)],
            vec![],
        );
        let snapshot = partial.actor(&ActorId("ilse".into())).unwrap().clone();
        let (left, _) = desired_hand_props(&partial, &snapshot);
        assert_eq!(
            left,
            Some((ItemId("herrings".into()), "fish".into())),
            "one of two herrings is still in the open"
        );
    }

    /// The whole cast holds a spark wallet; it must never render as carry.
    #[test]
    fn a_lone_wallet_leaves_the_hands_empty() {
        let mirror = mirror_with(
            vec![actor("anyone", &["wallet"])],
            vec![item("wallet", "spark", "copper_coin")],
            vec![],
        );
        let snapshot = mirror.actor(&ActorId("anyone".into())).unwrap().clone();
        let (left, right) = desired_hand_props(&mirror, &snapshot);
        assert_eq!(left, None);
        assert_eq!(right, None);
    }

    /// §6's accepted loss: of several simultaneous offers only the oldest is
    /// in-hand; the rest exist only in text.
    #[test]
    fn only_the_oldest_offer_is_in_hand() {
        let offer = |item: &str, seq: u64| OfferSnapshot {
            item_id: ItemId(item.into()),
            giver_id: ActorId("ilse".into()),
            target_id: None,
            created_seq: seq,
        };
        let mirror = mirror_with(
            vec![actor("ilse", &["fish", "loaf"])],
            vec![item("fish", "herring", "fish"), item("loaf", "loaf", "loaf")],
            vec![offer("loaf", 9), offer("fish", 4)],
        );
        let snapshot = mirror.actor(&ActorId("ilse".into())).unwrap().clone();
        let (left, right) = desired_hand_props(&mirror, &snapshot);
        assert_eq!(
            right,
            Some((ItemId("fish".into()), "fish".into())),
            "the older offer wins the hand"
        );
        assert_eq!(
            left,
            Some((ItemId("loaf".into()), "loaf".into())),
            "the newer offer's item stays an ordinary carry"
        );
    }

    /// The fan's Create/Keep/Replace contract, kept for the in-hand prop.
    #[test]
    fn prop_create_keep_and_replace_are_distinct() {
        let current = HeldProp {
            actor: ActorId("ilse".into()),
            side: BodySide::Left,
            item_id: ItemId("fish".into()),
            visual_key: "fish".into(),
        };
        assert_eq!(
            prop_disposition(None, &ItemId("fish".into()), "fish"),
            PropDisposition::Create
        );
        assert_eq!(
            prop_disposition(Some(&current), &ItemId("fish".into()), "fish"),
            PropDisposition::Keep
        );
        assert_eq!(
            prop_disposition(Some(&current), &ItemId("eel".into()), "fish"),
            PropDisposition::Replace
        );
        assert_eq!(
            prop_disposition(Some(&current), &ItemId("fish".into()), "generic"),
            PropDisposition::Replace
        );
    }

    /// End-to-end through the real systems: a carried loaf grows a prop under
    /// the LEFT hand anchor, an offered fish under the RIGHT; dropping the
    /// offer from the snapshot retires its prop (snapshot omission stays the
    /// only removal signal).
    #[test]
    fn hand_props_reconcile_from_the_snapshot() {
        let snapshot = |world_revision: u64, offers: Vec<OfferSnapshot>| WorldSnapshot {
            world_revision,
            player_id: ActorId("player".into()),
            actors: vec![
                {
                    let mut player = actor("player", &[]);
                    player.control = ActorControl::Player;
                    player
                },
                actor("ilse", &["loaf", "fish"]),
            ],
            items: vec![
                item("loaf", "loaf", "loaf"),
                item("fish", "herring", "fish"),
            ],
            offers,
            road_carts: vec![],
        };
        let offer = vec![OfferSnapshot {
            item_id: ItemId("fish".into()),
            giver_id: ActorId("ilse".into()),
            target_id: Some(ActorId("player".into())),
            created_seq: 2,
        }];
        let mut mirror = WorldMirror::default();
        mirror.replace_snapshot(snapshot(1, offer)).unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .insert_resource(mirror)
            .add_systems(
                Startup,
                (
                    setup_item_prop_assets,
                    crate::smart_actors::body::setup_body_assets,
                ),
            )
            .add_systems(
                Update,
                (reconcile_actor_views, reconcile_hand_props).chain(),
            );
        app.update();

        let world = app.world_mut();
        let mut held: Vec<(ActorId, BodySide, ItemId, Entity)> = world
            .query::<(Entity, &HeldProp)>()
            .iter(world)
            .map(|(entity, prop)| {
                (prop.actor.clone(), prop.side, prop.item_id.clone(), entity)
            })
            .collect();
        held.sort_by_key(|(_, side, ..)| *side == BodySide::Right);
        assert_eq!(held.len(), 2, "one prop per occupied hand");
        assert_eq!(
            (&held[0].0.0[..], held[0].1, &held[0].2.0[..]),
            ("ilse", BodySide::Left, "loaf")
        );
        assert_eq!(
            (&held[1].0.0[..], held[1].1, &held[1].2.0[..]),
            ("ilse", BodySide::Right, "fish")
        );
        // Each prop is parented to the matching hand anchor.
        for (actor_id, side, _, entity) in &held {
            let anchor = world.entity(*entity).get::<ChildOf>().unwrap().parent();
            let hand = world.entity(anchor).get::<HandAnchor>().unwrap();
            assert_eq!(&hand.actor, actor_id);
            assert_eq!(hand.side, *side);
        }
        // The pose state received the L2 targets: carrying, offering.
        let mut poses = world.query::<(&ActorId, &BodyPoseState)>();
        assert!(
            poses
                .iter(world)
                .any(|(id, _)| id.0 == "ilse"),
            "the puppet has a pose state"
        );

        // Retract: the offer leaves the snapshot; only the carry prop stays
        // (the fish drops back to being an in-holds carry candidate, but the
        // loaf is first in holds).
        app.world_mut()
            .resource_mut::<WorldMirror>()
            .replace_snapshot(snapshot(2, vec![]))
            .unwrap();
        app.update();
        app.update();
        let world = app.world_mut();
        let remaining: Vec<BodySide> = world
            .query::<&HeldProp>()
            .iter(world)
            .map(|prop| prop.side)
            .collect();
        assert_eq!(
            remaining,
            vec![BodySide::Left],
            "the offered prop retires with its offer"
        );
    }

    /// The visible hand (`law_and_order.md` M4c). Unlike every other reach in
    /// this file, a grip is not a beat: it goes onto the prisoner's upper arm,
    /// *stays* there frame after frame, and comes off on the answering event —
    /// or, for the release paths the sim takes silently, when the two of them
    /// are no longer close enough to be holding each other at all.
    #[test]
    fn a_grip_holds_the_arm_until_the_law_lets_go() {
        let standing = |world_revision: u64, apart_m: f32| WorldSnapshot {
            world_revision,
            player_id: ActorId("player".into()),
            actors: vec![
                {
                    let mut player = actor("player", &[]);
                    player.control = ActorControl::Player;
                    player
                },
                actor("ashe", &[]),
                {
                    let mut thief = actor("thief", &[]);
                    thief.position_m = Position::new(apart_m, 0.91, 0.0).unwrap();
                    thief
                },
            ],
            items: vec![],
            offers: vec![],
            road_carts: vec![],
        };
        let mut mirror = WorldMirror::default();
        mirror.replace_snapshot(standing(1, 1.0)).unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>()
            .add_message::<HandoverFeedback>()
            .init_resource::<GripHolds>()
            .insert_resource(mirror)
            .add_systems(
                Startup,
                (
                    setup_item_prop_assets,
                    crate::smart_actors::body::setup_body_assets,
                ),
            )
            .add_systems(
                Update,
                (
                    reconcile_actor_views,
                    apply_handover_feedback,
                    hold_the_seized,
                )
                    .chain(),
            );
        app.update();

        let grip_of = |app: &mut App, id: &str| -> Option<Vec3> {
            let world = app.world_mut();
            world
                .query::<(&ActorId, &BodyPoseState)>()
                .iter(world)
                .find(|(actor_id, _)| actor_id.0 == id)
                .and_then(|(_, pose)| pose.grip())
        };
        assert_eq!(grip_of(&mut app, "ashe"), None, "nobody is held yet");

        app.world_mut().write_message(HandoverFeedback::TookHold {
            holder: ActorId("ashe".into()),
            prisoner: ActorId("thief".into()),
        });
        app.update();
        let arm =
            Vec3::new(1.0, 0.91, 0.0) + Vec3::Y * (body::SHOULDER_ROOT_Y - GRIP_BELOW_SHOULDER_M);
        assert_eq!(
            grip_of(&mut app, "ashe"),
            Some(arm),
            "the hand lands just below the prisoner's shoulder"
        );

        // The whole point of M4c's hand: it is still there next frame, with no
        // event to renew it.
        app.update();
        assert_eq!(grip_of(&mut app, "ashe"), Some(arm), "and it stays");
        assert_eq!(grip_of(&mut app, "thief"), None, "the held arm is not theirs");

        // An arm cannot follow past its own length: whatever ended this hold
        // without saying so (the dead-man timer, a station's four minutes), the
        // two of them standing apart ends the presentation of it.
        app.world_mut()
            .resource_mut::<WorldMirror>()
            .replace_snapshot(standing(2, GRIP_BREAKS_AT_M + 1.0))
            .unwrap();
        app.update();
        assert_eq!(grip_of(&mut app, "ashe"), None, "out of reach, so out of hand");

        // And the ordinary path: `release` takes every hand off at once.
        app.world_mut()
            .resource_mut::<WorldMirror>()
            .replace_snapshot(standing(3, 1.0))
            .unwrap();
        app.world_mut().write_message(HandoverFeedback::TookHold {
            holder: ActorId("ashe".into()),
            prisoner: ActorId("thief".into()),
        });
        app.update();
        assert_eq!(grip_of(&mut app, "ashe"), Some(arm));
        app.world_mut().write_message(HandoverFeedback::HandsOff {
            prisoner: ActorId("thief".into()),
        });
        app.update();
        assert_eq!(grip_of(&mut app, "ashe"), None, "the law let go");
    }
}
