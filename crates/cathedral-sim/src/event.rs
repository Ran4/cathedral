//! Domain events (`sim.py:133-157`): the structured history an authoritative
//! action produces. Emitted by [`crate::World::emit`], drained by the host
//! every poll. The sim never drops or filters them.

use serde::{Deserialize, Serialize};

use crate::{
    ids::{ActorId, ItemId, SpeechEventId},
    math::Vec3,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Speech,
    WorldEvent,
    Sound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainEvent {
    /// 1-based; assigned by `World::emit`.
    pub sequence: i64,
    pub event_type: EventType,
    /// `say`, a world-event verb, or the sound's class.
    pub kind: String,
    /// `None` only for world sounds (the town bell).
    pub actor_id: Option<ActorId>,
    pub target_id: Option<ActorId>,
    pub item_id: Option<ItemId>,
    /// How many units the item event moved (offer/accept/eat). 1 for every
    /// non-item event and for single-unit item traffic, so the HUD toast can
    /// pluralize ("You accept the 3 sparks").
    pub quantity: u32,
    /// Stripped speech text only.
    pub text: Option<String>,
    /// Actor position for speech/world events, origin for sounds.
    pub position_m: Option<Vec3>,
    /// Everyone in radius, in distance-then-id order, players included.
    pub recipient_ids: Vec<ActorId>,
    pub sound_id: Option<String>,
    pub audible_distance: Option<f64>,
    /// Sound events only; a subsequence of `recipient_ids`.
    pub witness_ids: Vec<ActorId>,
}

impl DomainEvent {
    /// `speech-{n}` / `sound-{n}` / `world-{n}` — note that the `world_event`
    /// type gets the `world` prefix, not `world_event`.
    pub fn event_id(&self) -> String {
        let prefix = match self.event_type {
            EventType::Speech => "speech",
            EventType::Sound => "sound",
            EventType::WorldEvent => "world",
        };
        format!("{prefix}-{}", self.sequence)
    }

    /// The typed id of a speech event. Panics on any other event type.
    pub fn speech_event_id(&self) -> SpeechEventId {
        assert_eq!(
            self.event_type,
            EventType::Speech,
            "speech_event_id on a non-speech event"
        );
        SpeechEventId(self.event_id())
    }

    fn blank(event_type: EventType, kind: impl Into<String>) -> Self {
        Self {
            sequence: 0,
            event_type,
            kind: kind.into(),
            actor_id: None,
            target_id: None,
            item_id: None,
            quantity: 1,
            text: None,
            position_m: None,
            recipient_ids: Vec::new(),
            sound_id: None,
            audible_distance: None,
            witness_ids: Vec::new(),
        }
    }

    pub fn speech(
        actor_id: ActorId,
        target_id: Option<ActorId>,
        text: String,
        position_m: Vec3,
        recipient_ids: Vec<ActorId>,
    ) -> Self {
        Self {
            actor_id: Some(actor_id),
            target_id,
            text: Some(text),
            position_m: Some(position_m),
            recipient_ids,
            ..Self::blank(EventType::Speech, "say")
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn world_event(
        kind: impl Into<String>,
        actor_id: ActorId,
        target_id: Option<ActorId>,
        item_id: Option<ItemId>,
        quantity: u32,
        position_m: Vec3,
        recipient_ids: Vec<ActorId>,
    ) -> Self {
        Self {
            actor_id: Some(actor_id),
            target_id,
            item_id,
            quantity,
            position_m: Some(position_m),
            recipient_ids,
            ..Self::blank(EventType::WorldEvent, kind)
        }
    }

    pub fn sound(
        sound_class: impl Into<String>,
        actor_id: Option<ActorId>,
        sound_id: impl Into<String>,
        audible_distance: f64,
        position_m: Vec3,
        recipient_ids: Vec<ActorId>,
        witness_ids: Vec<ActorId>,
    ) -> Self {
        Self {
            actor_id,
            position_m: Some(position_m),
            recipient_ids,
            sound_id: Some(sound_id.into()),
            audible_distance: Some(audible_distance),
            witness_ids,
            ..Self::blank(EventType::Sound, sound_class)
        }
    }
}
