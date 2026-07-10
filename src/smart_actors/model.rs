//! Typed protocol data and the validated, read-only semantic world mirror.
//!
//! Python owns the world.  This module deliberately contains no mutation
//! helpers for inventory or offers: a complete, validated snapshot is the only
//! way semantic state enters the Bevy application.

use std::{collections::HashMap, error::Error, fmt};

use bevy::prelude::{Component, Resource, Vec3};
use serde::{Deserialize, Deserializer, Serialize, de};

/// Maximum ID length accepted by the version-one protocol.
pub const MAX_ID_CHARS: usize = 128;
const MAX_ACTORS: usize = 1_024;
const MAX_ITEMS: usize = 4_096;
const MAX_OFFERS: usize = 4_096;
const MAX_LABEL_CHARS: usize = 256;

/// Stable, opaque identity of an actor in the Python world.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActorId(pub String);

/// Stable, opaque identity of an item in the Python world.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

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
    pub appearance_key: String,
    pub holds: Vec<ItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSnapshot {
    pub id: ItemId,
    pub name: String,
    pub visual_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferSnapshot {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub target_id: Option<ActorId>,
    pub created_seq: u64,
}

/// Complete public semantic state sent by Python.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSnapshot {
    pub world_revision: u64,
    pub player_id: ActorId,
    pub actors: Vec<ActorSnapshot>,
    pub items: Vec<ItemSnapshot>,
    pub offers: Vec<OfferSnapshot>,
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
    NonFinitePosition(ActorId),
    UnknownHeldItem {
        actor_id: ActorId,
        item_id: ItemId,
    },
    DuplicateHeldItem {
        actor_id: ActorId,
        item_id: ItemId,
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
            Self::NonFinitePosition(id) => {
                write!(formatter, "actor {:?} has a non-finite position", id.0)
            }
            Self::UnknownHeldItem { actor_id, item_id } => write!(
                formatter,
                "actor {:?} holds unknown item {:?}",
                actor_id.0, item_id.0
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
        }
    }
}

impl Error for SnapshotError {}

/// Failure to apply or order data for the semantic mirror.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorError {
    InvalidSessionId,
    NoActiveSession,
    SessionMismatch { expected: String, received: String },
    StaleRevision { current: u64, received: u64 },
    MalformedSnapshot(SnapshotError),
    StaleEventSequence { current: u64, received: u64 },
    EventSequenceGap { expected: u64, received: u64 },
}

impl MirrorError {
    /// Whether the bridge should request a complete authoritative snapshot.
    pub fn requires_resync(&self) -> bool {
        matches!(
            self,
            Self::MalformedSnapshot(_) | Self::EventSequenceGap { .. }
        )
    }
}

impl fmt::Display for MirrorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSessionId => formatter.write_str("invalid session id"),
            Self::NoActiveSession => formatter.write_str("no smart-actor session is active"),
            Self::SessionMismatch { expected, received } => write!(
                formatter,
                "message belongs to session {received:?}, expected {expected:?}"
            ),
            Self::StaleRevision { current, received } => write!(
                formatter,
                "snapshot revision {received} is not newer than revision {current}"
            ),
            Self::MalformedSnapshot(error) => write!(formatter, "malformed snapshot: {error}"),
            Self::StaleEventSequence { current, received } => write!(
                formatter,
                "event sequence {received} is not newer than {current}"
            ),
            Self::EventSequenceGap { expected, received } => write!(
                formatter,
                "event sequence gap: expected {expected}, received {received}"
            ),
        }
    }
}

impl Error for MirrorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MalformedSnapshot(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SnapshotError> for MirrorError {
    fn from(value: SnapshotError) -> Self {
        Self::MalformedSnapshot(value)
    }
}

/// Validated projection of the current Python world.
#[derive(Resource, Debug, Default)]
pub struct WorldMirror {
    session_id: Option<String>,
    revision: Option<u64>,
    player_id: Option<ActorId>,
    actors: Vec<ActorSnapshot>,
    actor_indices: HashMap<ActorId, usize>,
    items: Vec<ItemSnapshot>,
    item_indices: HashMap<ItemId, usize>,
    offers: Vec<OfferSnapshot>,
    last_event_seq: Option<u64>,
    resync_needed: bool,
}

#[allow(dead_code)]
impl WorldMirror {
    /// Starts a fresh sidecar session and drops all state from the old one.
    pub fn begin_session(&mut self, session_id: impl Into<String>) -> Result<(), MirrorError> {
        let session_id = session_id.into();
        if !valid_id(&session_id) {
            return Err(MirrorError::InvalidSessionId);
        }

        *self = Self {
            session_id: Some(session_id),
            ..Self::default()
        };
        Ok(())
    }

    pub fn session(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// Alias matching the name used on the wire.
    pub fn world_revision(&self) -> Option<u64> {
        self.revision()
    }

    pub fn player_id(&self) -> Option<&ActorId> {
        self.player_id.as_ref()
    }

    pub fn last_event_seq(&self) -> Option<u64> {
        self.last_event_seq
    }

    pub fn needs_resync(&self) -> bool {
        self.resync_needed
    }

    pub fn mark_resync_needed(&mut self) {
        self.resync_needed = true;
    }

    pub fn actors(&self) -> impl ExactSizeIterator<Item = &ActorSnapshot> {
        self.actors.iter()
    }

    pub fn items(&self) -> impl ExactSizeIterator<Item = &ItemSnapshot> {
        self.items.iter()
    }

    pub fn offers(&self) -> impl ExactSizeIterator<Item = &OfferSnapshot> {
        self.offers.iter()
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

    pub fn offer(&self, id: &ItemId) -> Option<&OfferSnapshot> {
        self.offers.iter().find(|offer| &offer.item_id == id)
    }

    /// Records one server event and detects loss or reordering.
    ///
    /// A gap advances the observed watermark before returning the error.  This
    /// lets the bridge request one resync and continue consuming the ordered
    /// stream while it waits for that snapshot, rather than reporting the same
    /// missing sequence for every subsequent event.
    pub fn observe_event(&mut self, session_id: &str, event_seq: u64) -> Result<(), MirrorError> {
        self.ensure_session(session_id)?;

        let expected = self.last_event_seq.map_or(1, |last| last.saturating_add(1));
        if self
            .last_event_seq
            .is_some_and(|current| event_seq <= current)
        {
            return Err(MirrorError::StaleEventSequence {
                current: self.last_event_seq.expect("checked above"),
                received: event_seq,
            });
        }

        self.last_event_seq = Some(event_seq);
        if event_seq != expected {
            self.resync_needed = true;
            return Err(MirrorError::EventSequenceGap {
                expected,
                received: event_seq,
            });
        }
        Ok(())
    }

    /// More explicit alias useful at protocol call sites.
    pub fn observe_event_sequence(
        &mut self,
        session_id: &str,
        event_seq: u64,
    ) -> Result<(), MirrorError> {
        self.observe_event(session_id, event_seq)
    }

    /// Atomically validates and replaces the entire semantic projection.
    pub fn replace_snapshot(
        &mut self,
        session_id: &str,
        snapshot: WorldSnapshot,
    ) -> Result<u64, MirrorError> {
        self.ensure_session(session_id)?;
        if let Some(current) = self.revision {
            let stale = snapshot.world_revision < current
                || (snapshot.world_revision == current && !self.resync_needed);
            if stale {
                return Err(MirrorError::StaleRevision {
                    current,
                    received: snapshot.world_revision,
                });
            }
        }

        let validated = match ValidatedSnapshot::new(snapshot) {
            Ok(validated) => validated,
            Err(error) => {
                self.resync_needed = true;
                return Err(MirrorError::MalformedSnapshot(error));
            }
        };

        let revision = validated.snapshot.world_revision;
        self.revision = Some(revision);
        self.player_id = Some(validated.snapshot.player_id);
        self.actors = validated.snapshot.actors;
        self.actor_indices = validated.actor_indices;
        self.items = validated.snapshot.items;
        self.item_indices = validated.item_indices;
        self.offers = validated.snapshot.offers;
        self.resync_needed = false;
        Ok(revision)
    }

    fn ensure_session(&self, received: &str) -> Result<(), MirrorError> {
        let Some(expected) = self.session_id.as_deref() else {
            return Err(MirrorError::NoActiveSession);
        };
        if received != expected {
            return Err(MirrorError::SessionMismatch {
                expected: expected.to_owned(),
                received: received.to_owned(),
            });
        }
        Ok(())
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
            if !valid_projection_text(&actor.name_for_player, MAX_LABEL_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "actor label",
                    owner_id: actor.id.0.clone(),
                });
            }
            if !valid_projection_text(&actor.appearance_key, MAX_ID_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "appearance key",
                    owner_id: actor.id.0.clone(),
                });
            }
            if actor.holds.len() > MAX_ITEMS {
                return Err(SnapshotError::LimitExceeded("held-item records"));
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
            if !valid_projection_text(&item.name, MAX_LABEL_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "item name",
                    owner_id: item.id.0.clone(),
                });
            }
            if !valid_projection_text(&item.visual_key, MAX_ID_CHARS) {
                return Err(SnapshotError::InvalidText {
                    field: "item visual key",
                    owner_id: item.id.0.clone(),
                });
            }
        }

        let Some(player_index) = actor_indices.get(&snapshot.player_id).copied() else {
            return Err(SnapshotError::UnknownPlayer(snapshot.player_id.clone()));
        };
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
            appearance_key: id.into(),
            holds: holds.iter().map(|id| ItemId((*id).into())).collect(),
        }
    }

    fn item(id: &str) -> ItemSnapshot {
        ItemSnapshot {
            id: ItemId(id.into()),
            name: id.into(),
            visual_key: id.into(),
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
        }
    }

    #[test]
    fn typed_ids_are_transparent_on_the_wire() {
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

    #[test]
    fn replacement_is_atomic_and_rejects_stale_revisions() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        assert_eq!(mirror.replace_snapshot("one", snapshot(7)).unwrap(), 7);

        let mut broken = snapshot(8);
        broken.actors[1].holds.push(ItemId("fish".into()));
        let error = mirror.replace_snapshot("one", broken).unwrap_err();
        assert!(matches!(
            error,
            MirrorError::MalformedSnapshot(SnapshotError::MultipleOwners { .. })
        ));
        assert_eq!(mirror.revision(), Some(7));
        assert_eq!(mirror.actor(&ActorId("npc".into())).unwrap().holds.len(), 1);
        assert!(mirror.needs_resync());

        // A malformed snapshot requested a resync, so an equal-revision full
        // snapshot is the one intentional exception to normal stale rejection.
        assert_eq!(mirror.replace_snapshot("one", snapshot(7)).unwrap(), 7);
        assert!(!mirror.needs_resync());
        assert!(matches!(
            mirror.replace_snapshot("one", snapshot(7)),
            Err(MirrorError::StaleRevision { .. })
        ));
    }

    #[test]
    fn replacement_rejects_wrong_session_without_changing_state() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        mirror.replace_snapshot("one", snapshot(2)).unwrap();
        assert!(matches!(
            mirror.replace_snapshot("two", snapshot(3)),
            Err(MirrorError::SessionMismatch { .. })
        ));
        assert_eq!(mirror.revision(), Some(2));
    }

    #[test]
    fn begin_session_clears_old_projection_and_sequence() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        mirror.observe_event("one", 1).unwrap();
        mirror.replace_snapshot("one", snapshot(2)).unwrap();
        mirror.begin_session("two").unwrap();
        assert_eq!(mirror.session(), Some("two"));
        assert_eq!(mirror.revision(), None);
        assert_eq!(mirror.last_event_seq(), None);
        assert_eq!(mirror.actors().len(), 0);
    }

    #[test]
    fn event_sequence_gap_is_observed_and_requests_resync() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        mirror.observe_event("one", 1).unwrap();
        let error = mirror.observe_event("one", 3).unwrap_err();
        assert_eq!(
            error,
            MirrorError::EventSequenceGap {
                expected: 2,
                received: 3,
            }
        );
        assert!(error.requires_resync());
        assert!(mirror.needs_resync());
        assert_eq!(mirror.last_event_seq(), Some(3));
        mirror.observe_event("one", 4).unwrap();
    }

    #[test]
    fn equal_revision_full_snapshot_completes_requested_resync() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        mirror.replace_snapshot("one", snapshot(7)).unwrap();
        mirror.observe_event("one", 1).unwrap();
        assert!(matches!(
            mirror.observe_event("one", 3),
            Err(MirrorError::EventSequenceGap { .. })
        ));
        assert!(mirror.needs_resync());

        assert_eq!(mirror.replace_snapshot("one", snapshot(7)).unwrap(), 7);
        assert!(!mirror.needs_resync());
    }

    #[test]
    fn malformed_offer_does_not_replace_the_good_projection() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        mirror.replace_snapshot("one", snapshot(1)).unwrap();
        let mut invalid = snapshot(2);
        invalid.offers[0].giver_id = ActorId("player".into());
        invalid.offers[0].target_id = Some(ActorId("npc".into()));
        assert!(matches!(
            mirror.replace_snapshot("one", invalid),
            Err(MirrorError::MalformedSnapshot(
                SnapshotError::OfferGiverDoesNotOwnItem { .. }
            ))
        ));
        assert_eq!(
            mirror.offers().next().unwrap().giver_id,
            ActorId("npc".into())
        );
    }

    #[test]
    fn projection_text_is_bounded_before_reaching_ecs_or_ui() {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("one").unwrap();
        let mut invalid = snapshot(1);
        invalid.actors[0].name_for_player = "x".repeat(MAX_LABEL_CHARS + 1);
        assert!(matches!(
            mirror.replace_snapshot("one", invalid),
            Err(MirrorError::MalformedSnapshot(
                SnapshotError::InvalidText { .. }
            ))
        ));
        assert_eq!(mirror.revision(), None);
    }
}
