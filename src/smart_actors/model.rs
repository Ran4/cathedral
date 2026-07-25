//! The read-only semantic world mirror the ECS queries, and its types.
//!
//! `cathedral_sim` owns the world. This module deliberately contains no
//! mutation helpers for inventory or offers: a complete snapshot is the only
//! way semantic state enters the Bevy application.
//!
//! The reconciliation machinery is gone with the sidecar. Session ids, event
//! sequence gaps, stale revisions and resynchronization existed because a
//! snapshot could be lost, reordered or truncated in a pipe. The engine now
//! hands its snapshots over a typed in-process channel, so the mirror is a
//! projection and nothing more.
//!
//! What survives is [`ValidatedSnapshot`]: the actors' names, appearances and
//! item labels are written by an LLM, and this is where that text is bounded
//! and shape-checked before it reaches the ECS or a UI node.

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
};

use bevy::prelude::{Component, Resource, Vec3};
use serde::{Deserialize, Deserializer, Serialize, de};

/// Maximum id length the projection accepts.
pub const MAX_ID_CHARS: usize = 128;
const MAX_ACTORS: usize = 1_024;
const MAX_ITEMS: usize = 4_096;
const MAX_OFFERS: usize = 4_096;
const MAX_LABEL_CHARS: usize = 256;
/// A stack's metadata is a handful of short catalog-declared descriptors; a
/// projection carrying more is malformed.
const MAX_METADATA_ENTRIES: usize = 16;
/// The slot model's capacity (`features/extra_pockets.md`): two stack-units per
/// cavity, total. The sim enforces it; the mirror is the gate for every other
/// snapshot source, and the inventory screen lays out against it.
pub(super) const MAX_POCKETED_PER_SLOT: usize = 2;

/// Stable, opaque identity of an actor in the engine's world.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActorId(pub String);

/// Stable, opaque identity of an item in the engine's world.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

/// One mover's latest pose off the engine's hot channel — the host-side twin of
/// `cathedral_sim::ActorMotion`, projected out of [`cathedral_sim::EngineMessage::Movement`]
/// exactly as the clock is projected out of its own message: a plain per-poll
/// value, never a snapshot, never a revision bump (features/movement/06_engineering.md,
/// the hot/cold split). `seq` bumps on every fresh sample so the interpolator can
/// tell a new 20 Hz tick from a re-read of the same one.
#[derive(Debug, Clone, Copy, Default)]
pub struct MotionSample {
    pub position: Vec3,
    pub facing_yaw: f32,
    pub speed: f32,
    pub gait_phase: f32,
    pub seq: u64,
}

/// The latest [`MotionSample`] for each mover, written in the `Movement` arm of
/// `process_engine_message` and read by `actors::drive_npc_bodies`. Non-movers
/// never appear here, so the interpolator leaves them entirely to the snapshot
/// reconcile pass.
#[derive(Resource, Default, Debug)]
pub struct MovementInbox(pub HashMap<ActorId, MotionSample>);

/// A protocol position, in metres.
///
/// Deserialization is intentionally stricter than serde_json's default: NaN
/// and infinities never become part of a snapshot, even if another serde input
/// format happens to support them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Result<Self, InvalidPosition> {
        let position = Self { x, y, z };
        position.validate()?;
        Ok(position)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn validate(self) -> Result<(), InvalidPosition> {
        if self.is_finite() {
            Ok(())
        } else {
            Err(InvalidPosition)
        }
    }

    pub fn try_from_vec3(value: Vec3) -> Result<Self, InvalidPosition> {
        Self::new(value.x, value.y, value.z)
    }
}

impl<'de> Deserialize<'de> for Position {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WirePosition {
            x: f32,
            y: f32,
            z: f32,
        }

        let wire = WirePosition::deserialize(deserializer)?;
        Self::new(wire.x, wire.y, wire.z).map_err(de::Error::custom)
    }
}

impl TryFrom<Vec3> for Position {
    type Error = InvalidPosition;

    fn try_from(value: Vec3) -> Result<Self, Self::Error> {
        Self::try_from_vec3(value)
    }
}

impl From<Position> for Vec3 {
    fn from(value: Position) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<&Position> for Vec3 {
    fn from(value: &Position) -> Self {
        (*value).into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPosition;

impl fmt::Display for InvalidPosition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("position coordinates must all be finite")
    }
}

impl Error for InvalidPosition {}

/// Which side controls an actor's decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorControl {
    Llm,
    Player,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorSnapshot {
    pub id: ActorId,
    pub name_for_player: String,
    pub control: ActorControl,
    pub position_m: Position,
    /// Static seeded NPC orientation, radians (yaw 0 faces -Z). The render is
    /// the only place the player can read the sound witness rule from, so the
    /// actor view must rotate to exactly this.
    #[serde(default)]
    pub facing_yaw: f32,
    /// The sim-composed body appearance (`features/npc_bodies.md` §2). The
    /// enums are the sim's own renderer-facing vocabulary — mirroring them
    /// here would only be drift risk, so the type crosses as-is.
    #[serde(default)]
    pub appearance: cathedral_sim::AppearanceSnapshot,
    pub holds: Vec<ItemId>,
    /// The looping gesture the actor is holding, or `None`
    /// (`features/npc_bodies.md` §7). Only `dance` loops; the host drives the
    /// dance pose from this field, so a player who arrives mid-loop still sees
    /// it. The sim enum crosses as-is for the same reason `appearance` does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_gesture: Option<cathedral_sim::GestureKind>,
    /// Publicly-visible carriage axes (`features/npc_bodies.md` §8): drunkenness,
    /// weariness, each a finite `0..=1`. `body.rs` reads them to dress the walk
    /// (sway, stoop) without moving the actor. The sim enums cross as-is like
    /// `appearance`; skipped when empty — the universal case — so the mirror is
    /// byte-identical to before M5 for every actor no debug hook touched.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<(cathedral_sim::StatusKind, f32)>,
    /// Pocketed stack-units (`features/extra_pockets.md`): one entry per unit
    /// riding a body cavity, naming the slot and the held stack it came out of.
    /// The unit stays in `holds` — a pocket entry is a reservation, not a move —
    /// so the host reads this to know a carry prop must *not* render and to
    /// build the inventory screen's body sections. The sim enum crosses as-is
    /// like `statuses`; skipped when empty, which is the universal case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pockets: Vec<(cathedral_sim::BodySlot, ItemId)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSnapshot {
    pub id: ItemId,
    /// The catalog kind ("spark", "loaf"); the display name below is derived
    /// from it sim-side so the host never needs the catalog.
    pub kind: String,
    /// The catalog-derived display name shown to the player.
    pub name: String,
    /// The catalog-derived plural noun phrase, so the host can pluralize counts
    /// (including irregulars like "loaves") without the catalog.
    pub display_plural: String,
    pub visual_key: String,
    /// How many units in this stack (always ≥ 1).
    pub quantity: u32,
    /// The catalog-declared descriptors that are part of stack identity.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferSnapshot {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub target_id: Option<ActorId>,
    pub created_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadCartSnapshot {
    pub party_id: String,
    pub leader_id: ActorId,
    pub load: Vec<cathedral_sim::CartLoadKind>,
}

/// Complete public semantic state, as the engine last published it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSnapshot {
    pub world_revision: u64,
    pub player_id: ActorId,
    pub actors: Vec<ActorSnapshot>,
    pub items: Vec<ItemSnapshot>,
    pub offers: Vec<OfferSnapshot>,
    #[serde(default)]
    pub road_carts: Vec<RoadCartSnapshot>,
}

// ------------------------------------------------- the sim → renderer boundary
//
// The sim is f64 and its ids and sequences are its own types (D1). This is the
// one place those become the f32, Bevy-shaped values the ECS and the UI read.
// The conversion is lossy by design and deliberately *not* validating: what it
// produces is fed straight into [`WorldMirror::replace_snapshot`], which is the
// gate.

/// Metres, f64 → f32.
pub fn position_from_sim(position: cathedral_sim::Vec3) -> Position {
    Position {
        x: position.x as f32,
        y: position.y as f32,
        z: position.z as f32,
    }
}

/// Metres, f64 → the renderer's own vector.
pub fn vec3_from_sim(position: cathedral_sim::Vec3) -> Vec3 {
    Vec3::new(position.x as f32, position.y as f32, position.z as f32)
}

pub fn actor_id_from_sim(id: &cathedral_sim::ActorId) -> ActorId {
    ActorId(id.as_str().to_owned())
}

/// A carriage status value forced to a finite `0..=1` (`features/npc_bodies.md`
/// §8). Non-finite reads as 0 — no carriage rather than an undefined pose.
fn clamp_status(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub fn item_id_from_sim(id: &cathedral_sim::ItemId) -> ItemId {
    ItemId(id.as_str().to_owned())
}

impl From<cathedral_sim::Control> for ActorControl {
    fn from(value: cathedral_sim::Control) -> Self {
        match value {
            cathedral_sim::Control::Llm => Self::Llm,
            cathedral_sim::Control::Player => Self::Player,
        }
    }
}

impl From<&cathedral_sim::PublicSnapshot> for WorldSnapshot {
    fn from(snapshot: &cathedral_sim::PublicSnapshot) -> Self {
        Self {
            // The sim's revisions and sequences count up from zero and never
            // go back; the saturating cast is a formality.
            world_revision: snapshot.world_revision.max(0) as u64,
            player_id: actor_id_from_sim(&snapshot.player_id),
            actors: snapshot
                .actors
                .iter()
                .map(|actor| ActorSnapshot {
                    id: actor_id_from_sim(&actor.id),
                    name_for_player: actor.name_for_player.clone(),
                    control: actor.control.into(),
                    position_m: position_from_sim(actor.position_m),
                    facing_yaw: actor.facing_yaw as f32,
                    appearance: actor.appearance.clone(),
                    holds: actor.holds.iter().map(item_id_from_sim).collect(),
                    active_gesture: actor.active_gesture,
                    // Re-clamp on the way in: the sim already bounds these, but
                    // the mirror is the gate for every snapshot source, so a
                    // stray value never reaches the pose math.
                    statuses: actor
                        .statuses
                        .iter()
                        .map(|&(kind, value)| (kind, clamp_status(value)))
                        .collect(),
                    pockets: actor
                        .pockets
                        .iter()
                        .map(|(slot, item_id)| (*slot, item_id_from_sim(item_id)))
                        .collect(),
                })
                .collect(),
            items: snapshot
                .items
                .iter()
                .map(|item| ItemSnapshot {
                    id: item_id_from_sim(&item.id),
                    kind: item.kind.as_str().to_owned(),
                    name: item.display_name.clone(),
                    display_plural: item.display_plural.clone(),
                    visual_key: item.visual_key.clone(),
                    quantity: item.quantity,
                    metadata: item.metadata.clone(),
                })
                .collect(),
            offers: snapshot
                .offers
                .iter()
                .map(|offer| OfferSnapshot {
                    item_id: item_id_from_sim(&offer.item_id),
                    giver_id: actor_id_from_sim(&offer.giver_id),
                    target_id: offer.target_id.as_ref().map(actor_id_from_sim),
                    created_seq: offer.created_seq.max(0) as u64,
                })
                .collect(),
            road_carts: snapshot
                .road_carts
                .iter()
                .map(|cart| RoadCartSnapshot {
                    party_id: cart.party_id.as_str().to_owned(),
                    leader_id: actor_id_from_sim(&cart.leader_id),
                    load: cart.load.clone(),
                })
                .collect(),
        }
    }
}

/// A detailed invariant failure inside an otherwise decoded snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    LimitExceeded(&'static str),
    InvalidText {
        field: &'static str,
        owner_id: String,
    },
    InvalidActorId(ActorId),
    InvalidItemId(ItemId),
    DuplicateActor(ActorId),
    DuplicateItem(ItemId),
    /// A stack with quantity 0 — unrepresentable; the sim removes such stacks.
    NonPositiveQuantity(ItemId),
    NonFinitePosition(ActorId),
    /// A carriage status outside a finite `0..=1` (`features/npc_bodies.md` §8).
    InvalidStatus(ActorId),
    UnknownHeldItem {
        actor_id: ActorId,
        item_id: ItemId,
    },
    DuplicateHeldItem {
        actor_id: ActorId,
        item_id: ItemId,
    },
    /// A pocket entry naming a stack the actor does not hold
    /// (`features/extra_pockets.md`: a pocketed unit never leaves `holds`).
    PocketedItemNotHeld {
        actor_id: ActorId,
        item_id: ItemId,
    },
    /// More units in one cavity than the slot model allows (two, total).
    PocketOverfull {
        actor_id: ActorId,
        slot: cathedral_sim::BodySlot,
    },
    MultipleOwners {
        item_id: ItemId,
        first_owner: ActorId,
        second_owner: ActorId,
    },
    UnownedItem(ItemId),
    UnknownPlayer(ActorId),
    PlayerHasWrongControl(ActorId),
    MultiplePlayerActors {
        declared: ActorId,
        additional: ActorId,
    },
    DuplicateOffer(ItemId),
    UnknownOfferedItem(ItemId),
    UnknownOfferGiver(ActorId),
    UnknownOfferTarget(ActorId),
    OfferGiverDoesNotOwnItem {
        item_id: ItemId,
        giver_id: ActorId,
    },
    OfferTargetsGiver {
        item_id: ItemId,
        giver_id: ActorId,
    },
    InvalidOfferSequence(ItemId),
    InvalidRoadPartyId(String),
    DuplicateRoadCart(String),
    UnknownRoadCartLeader(ActorId),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded(kind) => write!(formatter, "snapshot contains too many {kind}"),
            Self::InvalidText { field, owner_id } => {
                write!(formatter, "{field} for {owner_id:?} is invalid")
            }
            Self::InvalidActorId(id) => write!(formatter, "invalid actor id {:?}", id.0),
            Self::InvalidItemId(id) => write!(formatter, "invalid item id {:?}", id.0),
            Self::DuplicateActor(id) => write!(formatter, "duplicate actor id {:?}", id.0),
            Self::DuplicateItem(id) => write!(formatter, "duplicate item id {:?}", id.0),
            Self::NonPositiveQuantity(id) => {
                write!(formatter, "item {:?} has a non-positive quantity", id.0)
            }
            Self::NonFinitePosition(id) => {
                write!(formatter, "actor {:?} has a non-finite position", id.0)
            }
            Self::InvalidStatus(id) => {
                write!(formatter, "actor {:?} has a status outside 0..=1", id.0)
            }
            Self::UnknownHeldItem { actor_id, item_id } => write!(
                formatter,
                "actor {:?} holds unknown item {:?}",
                actor_id.0, item_id.0
            ),
            Self::PocketedItemNotHeld { actor_id, item_id } => write!(
                formatter,
                "actor {:?} pockets item {:?} it does not hold",
                actor_id.0, item_id.0
            ),
            Self::PocketOverfull { actor_id, slot } => write!(
                formatter,
                "actor {:?} has more than {MAX_POCKETED_PER_SLOT} units in its {} slot",
                actor_id.0,
                slot.as_str()
            ),
            Self::DuplicateHeldItem { actor_id, item_id } => write!(
                formatter,
                "actor {:?} holds item {:?} more than once",
                actor_id.0, item_id.0
            ),
            Self::MultipleOwners {
                item_id,
                first_owner,
                second_owner,
            } => write!(
                formatter,
                "item {:?} is held by both {:?} and {:?}",
                item_id.0, first_owner.0, second_owner.0
            ),
            Self::UnownedItem(item_id) => {
                write!(formatter, "item {:?} has no owner", item_id.0)
            }
            Self::UnknownPlayer(id) => {
                write!(formatter, "declared player {:?} is not an actor", id.0)
            }
            Self::PlayerHasWrongControl(id) => write!(
                formatter,
                "declared player {:?} is not player-controlled",
                id.0
            ),
            Self::MultiplePlayerActors {
                declared,
                additional,
            } => write!(
                formatter,
                "snapshot declares player {:?} but actor {:?} is also player-controlled",
                declared.0, additional.0
            ),
            Self::DuplicateOffer(id) => {
                write!(formatter, "item {:?} has more than one offer", id.0)
            }
            Self::UnknownOfferedItem(id) => {
                write!(formatter, "offer refers to unknown item {:?}", id.0)
            }
            Self::UnknownOfferGiver(id) => {
                write!(formatter, "offer refers to unknown giver {:?}", id.0)
            }
            Self::UnknownOfferTarget(id) => {
                write!(formatter, "offer refers to unknown target {:?}", id.0)
            }
            Self::OfferGiverDoesNotOwnItem { item_id, giver_id } => write!(
                formatter,
                "offer giver {:?} does not hold item {:?}",
                giver_id.0, item_id.0
            ),
            Self::OfferTargetsGiver { item_id, giver_id } => write!(
                formatter,
                "offer for item {:?} targets its giver {:?}",
                item_id.0, giver_id.0
            ),
            Self::InvalidOfferSequence(id) => {
                write!(formatter, "offer for item {:?} has sequence zero", id.0)
            }
            Self::InvalidRoadPartyId(id) => write!(formatter, "invalid road party id {id:?}"),
            Self::DuplicateRoadCart(id) => write!(formatter, "duplicate road cart {id:?}"),
            Self::UnknownRoadCartLeader(id) => {
                write!(formatter, "road cart leader {:?} is not present", id.0)
            }
        }
    }
}

impl Error for SnapshotError {}

/// Validated projection of the engine's current world.
///
/// A projection, not a reconciler: the engine publishes a complete snapshot on
/// every revision increase and the channel delivers each one exactly once, in
/// order. The only thing that can still go wrong is the *content* — actor
/// labels and item names are LLM-authored — so the replacement is atomic and
/// validated, and a snapshot that fails leaves the previous one standing.
#[derive(Resource, Debug, Default)]
pub struct WorldMirror {
    revision: Option<u64>,
    player_id: Option<ActorId>,
    actors: Vec<ActorSnapshot>,
    actor_indices: HashMap<ActorId, usize>,
    items: Vec<ItemSnapshot>,
    item_indices: HashMap<ItemId, usize>,
    offers: Vec<OfferSnapshot>,
    road_carts: Vec<RoadCartSnapshot>,
}

impl WorldMirror {
    /// The revision the ECS is currently projecting. `interaction.rs` carries it
    /// on every command so a result that lands after the world moved on can be
    /// recognized as stale.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "queried by tests and future consumers")
    )]
    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    pub fn player_id(&self) -> Option<&ActorId> {
        self.player_id.as_ref()
    }

    pub fn actors(&self) -> impl ExactSizeIterator<Item = &ActorSnapshot> {
        self.actors.iter()
    }

    pub fn offers(&self) -> impl ExactSizeIterator<Item = &OfferSnapshot> {
        self.offers.iter()
    }

    pub fn road_carts(&self) -> impl ExactSizeIterator<Item = &RoadCartSnapshot> {
        self.road_carts.iter()
    }

    pub fn actor(&self, id: &ActorId) -> Option<&ActorSnapshot> {
        self.actor_indices
            .get(id)
            .and_then(|index| self.actors.get(*index))
    }

    pub fn item(&self, id: &ItemId) -> Option<&ItemSnapshot> {
        self.item_indices
            .get(id)
            .and_then(|index| self.items.get(*index))
    }

    /// Offers are few; a linear scan beats a second index.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "queried by tests and future consumers")
    )]
    pub fn offer(&self, id: &ItemId) -> Option<&OfferSnapshot> {
        self.offers.iter().find(|offer| &offer.item_id == id)
    }

    /// Atomically validates and replaces the entire semantic projection.
    pub fn replace_snapshot(&mut self, snapshot: WorldSnapshot) -> Result<u64, SnapshotError> {
        let validated = ValidatedSnapshot::new(snapshot)?;

        let revision = validated.snapshot.world_revision;
        self.revision = Some(revision);
        self.player_id = Some(validated.snapshot.player_id);
        self.actors = validated.snapshot.actors;
        self.actor_indices = validated.actor_indices;
        self.items = validated.snapshot.items;
        self.item_indices = validated.item_indices;
        self.offers = validated.snapshot.offers;
        self.road_carts = validated.snapshot.road_carts;
        Ok(revision)
    }
}

struct ValidatedSnapshot {
    snapshot: WorldSnapshot,
    actor_indices: HashMap<ActorId, usize>,
    item_indices: HashMap<ItemId, usize>,
}

impl ValidatedSnapshot {
    fn new(snapshot: WorldSnapshot) -> Result<Self, SnapshotError> {
        if snapshot.actors.len() > MAX_ACTORS {
            return Err(SnapshotError::LimitExceeded("actors"));
        }
        if snapshot.items.len() > MAX_ITEMS {
            return Err(SnapshotError::LimitExceeded("items"));
        }
        if snapshot.offers.len() > MAX_OFFERS {
            return Err(SnapshotError::LimitExceeded("offers"));
        }
        if !valid_id(&snapshot.player_id.0) {
            return Err(SnapshotError::InvalidActorId(snapshot.player_id.clone()));
        }

        let mut actor_indices = HashMap::with_capacity(snapshot.actors.len());
        for (index, actor) in snapshot.actors.iter().enumerate() {
            if !valid_id(&actor.id.0) {
                return Err(SnapshotError::InvalidActorId(actor.id.clone()));
            }
            if !actor.position_m.is_finite() {
                return Err(SnapshotError::NonFinitePosition(actor.id.clone()));
            }
            // JSON has no NaN literal, but serde_json parses overflowing
            // numbers to infinity; a non-finite yaw must not reach a Quat.
            if !actor.facing_yaw.is_finite() {
                return Err(SnapshotError::NonFinitePosition(actor.id.clone()));
            }
            if !valid_projection_text(&actor.name_for_player, MAX_LABEL_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "actor label",
                    owner_id: actor.id.0.clone(),
                });
            }
            // The structured appearance is closed enums plus a numeric seed;
            // only the optional bespoke tag is free text worth bounding.
            if let Some(bespoke) = &actor.appearance.bespoke
                && !valid_projection_text(bespoke, MAX_ID_CHARS)
            {
                return Err(SnapshotError::InvalidText {
                    field: "bespoke appearance",
                    owner_id: actor.id.0.clone(),
                });
            }
            if actor.holds.len() > MAX_ITEMS {
                return Err(SnapshotError::LimitExceeded("held-item records"));
            }
            // Carriage statuses (§8) drive the pose math, so a non-finite or
            // out-of-range value must never reach it. The sim and the `From`
            // clamp already; this is the gate for any other snapshot source.
            if actor
                .statuses
                .iter()
                .any(|&(_, value)| !value.is_finite() || !(0.0..=1.0).contains(&value))
            {
                return Err(SnapshotError::InvalidStatus(actor.id.clone()));
            }
            if actor_indices.insert(actor.id.clone(), index).is_some() {
                return Err(SnapshotError::DuplicateActor(actor.id.clone()));
            }
        }

        let mut item_indices = HashMap::with_capacity(snapshot.items.len());
        for (index, item) in snapshot.items.iter().enumerate() {
            if !valid_id(&item.id.0) {
                return Err(SnapshotError::InvalidItemId(item.id.clone()));
            }
            if item_indices.insert(item.id.clone(), index).is_some() {
                return Err(SnapshotError::DuplicateItem(item.id.clone()));
            }
            if item.quantity < 1 {
                return Err(SnapshotError::NonPositiveQuantity(item.id.clone()));
            }
            if !valid_projection_text(&item.name, MAX_LABEL_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "item name",
                    owner_id: item.id.0.clone(),
                });
            }
            if !valid_projection_text(&item.display_plural, MAX_LABEL_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "item plural",
                    owner_id: item.id.0.clone(),
                });
            }
            if !valid_projection_text(&item.visual_key, MAX_ID_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "item visual key",
                    owner_id: item.id.0.clone(),
                });
            }
            if !item.kind.is_empty() && !valid_projection_text(&item.kind, MAX_ID_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "item kind",
                    owner_id: item.id.0.clone(),
                });
            }
            if item.metadata.len() > MAX_METADATA_ENTRIES {
                return Err(SnapshotError::LimitExceeded("item metadata entries"));
            }
            for (key, value) in &item.metadata {
                if !valid_projection_text(key, MAX_LABEL_CHARS)
                    || !valid_projection_text(value, MAX_LABEL_CHARS)
                {
                    return Err(SnapshotError::InvalidText {
                        field: "item metadata",
                        owner_id: item.id.0.clone(),
                    });
                }
            }
        }

        let Some(player_index) = actor_indices.get(&snapshot.player_id).copied() else {
            return Err(SnapshotError::UnknownPlayer(snapshot.player_id.clone()));
        };
        let mut cart_parties = std::collections::BTreeSet::new();
        for cart in &snapshot.road_carts {
            if !valid_id(&cart.party_id) {
                return Err(SnapshotError::InvalidRoadPartyId(cart.party_id.clone()));
            }
            if !cart_parties.insert(cart.party_id.clone()) {
                return Err(SnapshotError::DuplicateRoadCart(cart.party_id.clone()));
            }
            if !actor_indices.contains_key(&cart.leader_id) {
                return Err(SnapshotError::UnknownRoadCartLeader(cart.leader_id.clone()));
            }
        }
        if snapshot.actors[player_index].control != ActorControl::Player {
            return Err(SnapshotError::PlayerHasWrongControl(
                snapshot.player_id.clone(),
            ));
        }
        if let Some(additional) = snapshot
            .actors
            .iter()
            .find(|actor| actor.control == ActorControl::Player && actor.id != snapshot.player_id)
        {
            return Err(SnapshotError::MultiplePlayerActors {
                declared: snapshot.player_id.clone(),
                additional: additional.id.clone(),
            });
        }

        let mut owners: HashMap<ItemId, ActorId> = HashMap::new();
        for actor in &snapshot.actors {
            let mut held_by_actor = std::collections::HashSet::with_capacity(actor.holds.len());
            for item_id in &actor.holds {
                if !valid_id(&item_id.0) {
                    return Err(SnapshotError::InvalidItemId(item_id.clone()));
                }
                if !item_indices.contains_key(item_id) {
                    return Err(SnapshotError::UnknownHeldItem {
                        actor_id: actor.id.clone(),
                        item_id: item_id.clone(),
                    });
                }
                if !held_by_actor.insert(item_id.clone()) {
                    return Err(SnapshotError::DuplicateHeldItem {
                        actor_id: actor.id.clone(),
                        item_id: item_id.clone(),
                    });
                }
                if let Some(first_owner) = owners.insert(item_id.clone(), actor.id.clone()) {
                    return Err(SnapshotError::MultipleOwners {
                        item_id: item_id.clone(),
                        first_owner,
                        second_owner: actor.id.clone(),
                    });
                }
            }
            // A pocketed unit is a *reservation* against a stack the actor
            // still holds (`features/extra_pockets.md`), so every entry must
            // name one of the ids just collected — and no cavity may carry
            // more than the slot model's two units.
            let mut per_slot: HashMap<cathedral_sim::BodySlot, usize> = HashMap::new();
            for (slot, item_id) in &actor.pockets {
                if !held_by_actor.contains(item_id) {
                    return Err(SnapshotError::PocketedItemNotHeld {
                        actor_id: actor.id.clone(),
                        item_id: item_id.clone(),
                    });
                }
                let count = per_slot.entry(*slot).or_default();
                *count += 1;
                if *count > MAX_POCKETED_PER_SLOT {
                    return Err(SnapshotError::PocketOverfull {
                        actor_id: actor.id.clone(),
                        slot: *slot,
                    });
                }
            }
        }

        if let Some(item) = snapshot
            .items
            .iter()
            .find(|item| !owners.contains_key(&item.id))
        {
            return Err(SnapshotError::UnownedItem(item.id.clone()));
        }

        let mut offered_items = std::collections::HashSet::with_capacity(snapshot.offers.len());
        for offer in &snapshot.offers {
            if !valid_id(&offer.item_id.0) {
                return Err(SnapshotError::InvalidItemId(offer.item_id.clone()));
            }
            if !valid_id(&offer.giver_id.0) {
                return Err(SnapshotError::InvalidActorId(offer.giver_id.clone()));
            }
            if let Some(target_id) = &offer.target_id
                && !valid_id(&target_id.0)
            {
                return Err(SnapshotError::InvalidActorId(target_id.clone()));
            }
            if !offered_items.insert(offer.item_id.clone()) {
                return Err(SnapshotError::DuplicateOffer(offer.item_id.clone()));
            }
            if !item_indices.contains_key(&offer.item_id) {
                return Err(SnapshotError::UnknownOfferedItem(offer.item_id.clone()));
            }
            if !actor_indices.contains_key(&offer.giver_id) {
                return Err(SnapshotError::UnknownOfferGiver(offer.giver_id.clone()));
            }
            if let Some(target_id) = &offer.target_id {
                if !actor_indices.contains_key(target_id) {
                    return Err(SnapshotError::UnknownOfferTarget(target_id.clone()));
                }
                if target_id == &offer.giver_id {
                    return Err(SnapshotError::OfferTargetsGiver {
                        item_id: offer.item_id.clone(),
                        giver_id: offer.giver_id.clone(),
                    });
                }
            }
            if owners.get(&offer.item_id) != Some(&offer.giver_id) {
                return Err(SnapshotError::OfferGiverDoesNotOwnItem {
                    item_id: offer.item_id.clone(),
                    giver_id: offer.giver_id.clone(),
                });
            }
            if offer.created_seq == 0 {
                return Err(SnapshotError::InvalidOfferSequence(offer.item_id.clone()));
            }
        }

        Ok(Self {
            snapshot,
            actor_indices,
            item_indices,
        })
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_ID_CHARS
        && !value.chars().any(char::is_control)
}

fn valid_projection_text(value: &str, maximum_chars: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= maximum_chars
        && value
            .chars()
            .all(|character| !character.is_control() || matches!(character, '\n' | '\t'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(id: &str, control: ActorControl, holds: &[&str]) -> ActorSnapshot {
        ActorSnapshot {
            id: ActorId(id.into()),
            name_for_player: id.into(),
            control,
            position_m: Position::new(1.0, 2.0, 3.0).unwrap(),
            facing_yaw: 0.0,
            appearance: Default::default(),
            holds: holds.iter().map(|id| ItemId((*id).into())).collect(),
            active_gesture: None,
            statuses: Vec::new(),
            pockets: Vec::new(),
        }
    }

    fn item(id: &str) -> ItemSnapshot {
        ItemSnapshot {
            id: ItemId(id.into()),
            kind: "generic".into(),
            name: id.into(),
            display_plural: format!("{id}s"),
            visual_key: id.into(),
            quantity: 1,
            metadata: BTreeMap::new(),
        }
    }

    fn snapshot(revision: u64) -> WorldSnapshot {
        WorldSnapshot {
            world_revision: revision,
            player_id: ActorId("player".into()),
            actors: vec![
                actor("npc", ActorControl::Llm, &["fish"]),
                actor("player", ActorControl::Player, &[]),
            ],
            items: vec![item("fish")],
            offers: vec![OfferSnapshot {
                item_id: ItemId("fish".into()),
                giver_id: ActorId("npc".into()),
                target_id: Some(ActorId("player".into())),
                created_seq: 4,
            }],
            road_carts: Vec::new(),
        }
    }

    #[test]
    fn typed_ids_serialize_as_bare_strings() {
        assert_eq!(
            serde_json::to_string(&ActorId("npc".into())).unwrap(),
            r#""npc""#
        );
        assert_eq!(
            serde_json::from_str::<ItemId>(r#""fish""#).unwrap(),
            ItemId("fish".into())
        );
    }

    #[test]
    fn position_rejects_non_finite_values_and_converts_to_bevy() {
        assert!(Position::new(f32::INFINITY, 0.0, 0.0).is_err());
        assert!(Position::try_from(Vec3::new(0.0, f32::NAN, 0.0)).is_err());
        assert!(serde_json::from_str::<Position>(r#"{"x":1.0,"y":1e40,"z":3.0}"#).is_err());
        let bevy: Vec3 = Position::new(1.0, 2.0, 3.0).unwrap().into();
        assert_eq!(bevy, Vec3::new(1.0, 2.0, 3.0));
    }

    /// The projection is replaced whole or not at all. There is no resync any
    /// more — a rejected snapshot simply does not land, and the next one (the
    /// engine publishes one per revision) supersedes it.
    #[test]
    fn a_rejected_snapshot_leaves_the_previous_projection_standing() {
        let mut mirror = WorldMirror::default();
        assert_eq!(mirror.replace_snapshot(snapshot(7)).unwrap(), 7);

        let mut broken = snapshot(8);
        broken.actors[1].holds.push(ItemId("fish".into()));
        assert!(matches!(
            mirror.replace_snapshot(broken).unwrap_err(),
            SnapshotError::MultipleOwners { .. }
        ));
        assert_eq!(mirror.revision(), Some(7));
        assert_eq!(mirror.actor(&ActorId("npc".into())).unwrap().holds.len(), 1);

        assert_eq!(mirror.replace_snapshot(snapshot(8)).unwrap(), 8);
    }

    #[test]
    fn malformed_offer_does_not_replace_the_good_projection() {
        let mut mirror = WorldMirror::default();
        mirror.replace_snapshot(snapshot(1)).unwrap();
        let mut invalid = snapshot(2);
        invalid.offers[0].giver_id = ActorId("player".into());
        invalid.offers[0].target_id = Some(ActorId("npc".into()));
        assert!(matches!(
            mirror.replace_snapshot(invalid),
            Err(SnapshotError::OfferGiverDoesNotOwnItem { .. })
        ));
        assert_eq!(
            mirror.offers().next().unwrap().giver_id,
            ActorId("npc".into())
        );
    }

    #[test]
    fn projection_text_is_bounded_before_reaching_ecs_or_ui() {
        let mut mirror = WorldMirror::default();
        let mut invalid = snapshot(1);
        invalid.actors[0].name_for_player = "x".repeat(MAX_LABEL_CHARS + 1);
        assert!(matches!(
            mirror.replace_snapshot(invalid),
            Err(SnapshotError::InvalidText { .. })
        ));
        assert_eq!(mirror.revision(), None);
    }

    /// Definition of done (host side): an item nobody holds is rejected wholesale
    /// — the mirror never carries a ground item.
    #[test]
    fn an_unowned_item_is_rejected_by_the_projection() {
        let mut mirror = WorldMirror::default();
        mirror.replace_snapshot(snapshot(1)).unwrap();
        let mut invalid = snapshot(2);
        // Nobody holds the item, and there is no offer keeping it live.
        invalid.actors[0].holds.clear();
        invalid.offers.clear();
        assert!(matches!(
            mirror.replace_snapshot(invalid),
            Err(SnapshotError::UnownedItem(_))
        ));
        // The good projection stands.
        assert_eq!(mirror.revision(), Some(1));
    }

    /// Definition of done (host side): quantity 0 is unrepresentable — the
    /// projection rejects it, the twin of the unowned-item rejection.
    #[test]
    fn a_zero_quantity_stack_is_rejected_by_the_projection() {
        let mut mirror = WorldMirror::default();
        mirror.replace_snapshot(snapshot(1)).unwrap();
        let mut invalid = snapshot(2);
        invalid.items[0].quantity = 0;
        assert!(matches!(
            mirror.replace_snapshot(invalid),
            Err(SnapshotError::NonPositiveQuantity(_))
        ));
        assert_eq!(mirror.revision(), Some(1));
    }
}
