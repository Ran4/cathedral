//! The command/event vocabulary the game speaks to its actor authority.
//!
//! The authority is [`super::local_engine`] — an in-process `cathedral_sim`
//! engine. This module is now only the channel/type layer between it and the
//! ECS: the game writes [`BridgeCommand`]s into a bounded queue and drains
//! [`BridgeEvent`]s from an inbox.
//!
//! There is no wire any more. The Python sidecar, its JSON-lines protocol, the
//! reader/writer threads and the envelope with its session id, message id and
//! event sequence are gone; a typed in-process channel cannot drop, reorder or
//! corrupt a message, so everything that existed to detect that went with them.
//! What the engine says now arrives as a [`cathedral_sim::EngineMessage`],
//! unserialized.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::prelude::Resource;
use cathedral_sim::EngineMessage;
use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError};
use serde::{Deserialize, Serialize};

use super::model::{ActorId, ItemId, Position};

/// Fixed provider format for streamed microphone audio; the capture worker
/// resamples every device rate down to this before chunking.
pub const STREAM_SAMPLE_RATE: u32 = 24_000;
pub(super) const COMMAND_QUEUE_CAPACITY: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionBackend {
    Cloud,
    Local,
}

impl TranscriptionBackend {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsBackend {
    Cloud,
    Local,
    Off,
}

impl TtsBackend {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BridgeCommand {
    /// The player's spawn. It is what starts the engine: the world needs the
    /// real position before it renders its first snapshot.
    Hello {
        position_m: Position,
        spatial_seq: u64,
    },
    SpatialUpdate {
        position_m: Position,
        spatial_seq: u64,
        /// Player compass bearing in radians (yaw 0 faces -Z). The engine runs
        /// the identical witness cone test against the player.
        facing_yaw: f32,
    },
    PlayerRecording {
        request_id: String,
        wav_basename: String,
        stt_backend: TranscriptionBackend,
        position_m: Position,
        spatial_seq: u64,
    },
    /// Start of a streamed copy of the utterance being recorded to
    /// `wav_basename`; chunks follow while the player is still speaking.
    PlayerAudioBegin {
        wav_basename: String,
    },
    PlayerAudioChunk {
        wav_basename: String,
        seq: u32,
        samples: Arc<[i16]>,
    },
    PlayerAudioEnd {
        wav_basename: String,
        chunk_count: u32,
        silent: bool,
    },
    PlayerAudioAbort {
        wav_basename: String,
    },
    DebugPlayerSay {
        request_id: String,
        text: String,
        target_id: Option<ActorId>,
        position_m: Position,
        spatial_seq: u64,
    },
    /// The typed-chat box (the Enter key): the player's text spoken as a real
    /// `say`, in any mode — `DebugPlayerSay` stays the fake-mode test hook.
    PlayerSay {
        request_id: String,
        text: String,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerOffer {
        request_id: String,
        target_id: ActorId,
        item_id: ItemId,
        /// Units of a stack to offer — `None` offers the whole stack. Set only
        /// for the coin purse's count picker (05_the_llm_seam.md §7).
        quantity: Option<u32>,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerAccept {
        request_id: String,
        item_id: ItemId,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerDecline {
        request_id: String,
        item_id: ItemId,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerRetract {
        request_id: String,
        item_id: ItemId,
    },
    // ------------------------------------------------ the body pockets
    // (`features/extra_pockets.md`). The sim enum crosses as-is, like
    // `DebugStatus`'s `StatusKind`: mirroring three variants would only be
    // drift risk.
    PlayerPocket {
        request_id: String,
        item_id: ItemId,
        slot: cathedral_sim::BodySlot,
    },
    PlayerRetrieve {
        request_id: String,
        item_id: ItemId,
    },
    PlayerSwallow {
        request_id: String,
        item_id: ItemId,
    },
    /// Aimed at somebody within reach, so it carries a position like an offer.
    PlayerSpit {
        request_id: String,
        item_id: ItemId,
        target_id: ActorId,
        position_m: Position,
        spatial_seq: u64,
    },
    PlayerGargle {
        request_id: String,
        item_id: ItemId,
    },
    PlayerExpel {
        request_id: String,
    },
    PlayerEat {
        request_id: String,
        item_id: ItemId,
    },
    /// Fire-and-forget deliberate player noise (the F key). No request_id and
    /// no command_result: there is no failure the player can act on, and
    /// rate-limited sounds are dropped silently at the engine.
    PlayerSound {
        sound_id: String,
    },
    /// The grab reflex fired (`law_and_order.md` M4c). Host-side, because the
    /// sim reads the player at 10 Hz — 1.2 m of travel per sample at run speed
    /// — and a 3 m radius decided over there would be wrong by most of its own
    /// radius. Fire-and-forget: a grab is not a request.
    PlayerGrabbed {
        holder_id: ActorId,
    },
    /// The player has started pulling against the hands on them, and (once)
    /// that they got free. Two commands across a whole struggle, never a
    /// stream: there is exactly one LLM turn in flight across the entire cast.
    /// The player finished a press-and-hold over a mark
    /// (`features/implemented/chalking_the_walls.md` M3). Goes through the same
    /// `scrub_mark` the LLM verb does, so what the hand does on screen and
    /// what a character does on a sheet are one code path.
    PlayerScrubMark {
        mark_id: u64,
    },
    /// …and the writing half of it. `anchor` is the sim's own handle out of
    /// [`cathedral_sim::EngineMessage::ChalkStanding`], carried back untouched:
    /// the host has no places registry and never resolves one itself.
    PlayerDrawMark {
        kind: cathedral_sim::marks::MarkKind,
        anchor: String,
    },
    PlayerStruggling,
    PlayerBrokeFree,
    /// CATHEDRAL_DRIVE `seize` action (`law_and_order.md` M4): stage an arrest so
    /// the tether, the reflex and the strain meter can be looked at. Every
    /// judgement above `seize` is an LLM's, which is right, and which is exactly
    /// why a scripted run cannot otherwise reach one. A developer poke, not a
    /// modelled cause — like `DebugStatus`.
    /// CATHEDRAL_DRIVE `chalk` / `scrub` (`features/implemented/chalking_the_walls.md` M2).
    DebugChalk {
        kind: String,
        anchor: String,
    },
    DebugScrub {
        anchor: String,
    },
    DebugSeize {
        officer: String,
        target: Option<String>,
    },
    /// CATHEDRAL_DRIVE `commit` action (`law_and_order.md` M5): finish the
    /// escort at the Stone House. `seize` alone only ever shows the walk, and
    /// the cell is the half of M5 worth looking at.
    DebugCommit { target: Option<String> },
    /// CATHEDRAL_DRIVE stand-in for world sounds the sim cannot cause yet
    /// (nothing rings the town bell: no clock, no calendar).
    DebugSound {
        sound_id: String,
        position_m: Position,
    },
    /// A world sound the *game* really does cause: the nightly rat boil
    /// (`features/rats.md` M2, `src/city/vermin.rs`). The same unattributed
    /// funnel `DebugSound` reaches, under its own name — a shipped feature is
    /// not a developer poke. Fire-and-forget: no `CommandResult` ever.
    WorldSound {
        sound_id: String,
        position_m: Position,
    },
    /// CATHEDRAL_DRIVE `status` action (`features/npc_bodies.md` §8): set a body
    /// carriage status on the named character so a drunk/weary walker can be
    /// eyeballed. A developer poke, not a modelled cause — like `DebugSound`.
    DebugStatus {
        name: String,
        kind: cathedral_sim::StatusKind,
        value: f64,
    },
    /// Advance the debug world-clock time scale (the `T` key). Fire-and-forget:
    /// the new scale comes back on the next `EngineMessage::Clock`.
    CycleTimeScale,
    SetWeatherOverride {
        kind: cathedral_sim::WeatherKind,
        intensity: Option<f64>,
    },
    ClearWeatherOverride,
    /// Fire-and-forget notice that a speech event's audio presentation reached
    /// a terminal state (played, skipped, dropped, failed, or cut off). The
    /// engine frees the conversation floor on it; its failsafe covers a lost
    /// send.
    SpeechPresented {
        speech_event_id: String,
    },
    SetTtsBackend {
        request_id: String,
        backend: TtsBackend,
    },
}

impl BridgeCommand {
    /// Only a position update may be dropped when the queue is full: the next
    /// one supersedes it. Everything else — including a streamed audio chunk —
    /// is a fact the engine has to see.
    fn is_redundant_spatial(&self) -> bool {
        matches!(self, Self::SpatialUpdate { .. })
    }
}

#[derive(Debug, Clone)]
pub enum BridgeEvent {
    /// The engine was constructed; the game answers with [`BridgeCommand::Hello`].
    ProcessStarted,
    /// One authoritative message, typed. No envelope, no JSON, no sequence
    /// number: the channel guarantees what the protocol used to have to check.
    Message(Box<EngineMessage>),
    /// The engine is gone (a failed start, or a panic caught in the pump). Per-
    /// subsystem trouble is *not* this: a dead speech worker arrives as an
    /// `EngineMessage::Status` with `state: "degraded"`, and only that subsystem
    /// goes dark.
    Disconnected(String),
}

/// Non-blocking command endpoint plus the session's private audio directory.
#[derive(Resource)]
pub struct BridgeHandle {
    commands: Sender<BridgeCommand>,
    runtime_dir: PathBuf,
}

impl BridgeHandle {
    /// The endpoint the authority hands to the ECS. `commands` is the producer
    /// half of [`super::local_engine`]'s inbox. Crate-visible so tests outside
    /// `smart_actors` (vermin's swarm-percept regression) can stand a receiver
    /// on the far end of the real endpoint.
    pub(crate) fn new(commands: Sender<BridgeCommand>, runtime_dir: PathBuf) -> Self {
        Self {
            commands,
            runtime_dir,
        }
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    /// Clone of the bounded command sender for non-ECS producers. The
    /// microphone worker streams utterance audio through the same queue so
    /// its chunks and Bevy's later `player_recording` stay strictly ordered.
    pub fn command_sender(&self) -> Sender<BridgeCommand> {
        self.commands.clone()
    }

    /// Enqueue without ever waiting on the engine.
    pub fn try_send(&self, command: BridgeCommand) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(command) if command.is_redundant_spatial() => {
                    "spatial update coalesced because the bridge is busy".into()
                }
                TrySendError::Full(_) => "smart-actor command queue is full".into(),
                TrySendError::Disconnected(_) => "smart-actor engine is offline".into(),
            })
    }
}

/// Polled once per frame; receiving never waits.
#[derive(Resource)]
pub struct BridgeInbox {
    events: Receiver<BridgeEvent>,
    disconnect_reported: AtomicBool,
}

impl BridgeInbox {
    pub(super) fn new(events: Receiver<BridgeEvent>) -> Self {
        Self {
            events,
            disconnect_reported: AtomicBool::new(false),
        }
    }

    pub fn try_recv(&self) -> Option<BridgeEvent> {
        match self.events.try_recv() {
            Ok(event) => {
                if matches!(event, BridgeEvent::Disconnected(_)) {
                    self.disconnect_reported.store(true, Ordering::Release);
                }
                Some(event)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                if self.disconnect_reported.swap(true, Ordering::AcqRel) {
                    None
                } else {
                    Some(BridgeEvent::Disconnected(
                        "the smart-actor engine stopped unexpectedly".into(),
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    fn position() -> Position {
        Position::new(1.0, 2.0, 3.0).unwrap()
    }

    #[test]
    fn streaming_commands_are_never_coalesced_as_spatials() {
        assert!(
            !BridgeCommand::PlayerAudioChunk {
                wav_basename: "player-recording-1.wav".into(),
                seq: 0,
                samples: vec![0_i16].into(),
            }
            .is_redundant_spatial()
        );
        assert!(
            !BridgeCommand::PlayerAudioEnd {
                wav_basename: "player-recording-1.wav".into(),
                chunk_count: 1,
                silent: false,
            }
            .is_redundant_spatial()
        );
    }

    #[test]
    fn spatial_updates_are_explicitly_safe_to_coalesce() {
        assert!(
            BridgeCommand::SpatialUpdate {
                position_m: position(),
                spatial_seq: 2,
                facing_yaw: 0.0,
            }
            .is_redundant_spatial()
        );
        assert!(
            !BridgeCommand::PlayerAccept {
                request_id: "r".into(),
                item_id: ItemId("coin".into()),
                position_m: position(),
                spatial_seq: 2,
            }
            .is_redundant_spatial()
        );
    }

    #[test]
    fn a_full_queue_names_the_coalesced_spatial_apart_from_a_lost_command() {
        let (sender, receiver) = bounded(1);
        let handle = BridgeHandle::new(sender, PathBuf::from("/tmp"));
        handle
            .try_send(BridgeCommand::PlayerSound {
                sound_id: "fart".into(),
            })
            .expect("the first command fits");

        let coalesced = handle
            .try_send(BridgeCommand::SpatialUpdate {
                position_m: position(),
                spatial_seq: 1,
                facing_yaw: 0.0,
            })
            .expect_err("the queue is full");
        assert!(coalesced.contains("coalesced"));

        let lost = handle
            .try_send(BridgeCommand::PlayerRetract {
                request_id: "r".into(),
                item_id: ItemId("coin".into()),
            })
            .expect_err("the queue is full");
        assert!(lost.contains("queue is full"));
        drop(receiver);
    }

    #[test]
    fn inbox_reports_engine_channel_disconnection_exactly_once() {
        let (sender, receiver) = bounded(1);
        let inbox = BridgeInbox::new(receiver);
        drop(sender);

        assert!(matches!(
            inbox.try_recv(),
            Some(BridgeEvent::Disconnected(_))
        ));
        assert!(inbox.try_recv().is_none());
    }
}
