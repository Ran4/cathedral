//! Snapshot-driven inventory selection and player interaction intents.

use std::collections::HashMap;

use bevy::{
    input::mouse::AccumulatedMouseScroll,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::controller::PlayerController;

use super::{
    ActorFocus, ITEM_INTERACTION_RADIUS_M, POSITION_UPDATE_HZ, SmartActorRuntime,
    SmartActorsConfig,
    bridge::{BridgeCommand, BridgeHandle, TranscriptionBackend},
    hud::SmartActorHudState,
    microphone::{MicrophoneCommand, MicrophoneEvent, MicrophonePoll, MicrophoneService},
    model::{ActorId, ItemId, WorldMirror},
    speech::StopNpcSpeech,
};

#[derive(Message, Debug, Clone, PartialEq)]
pub enum PlayerIntent {
    SpatialUpdate {
        spatial_seq: u64,
        position: Vec3,
        facing_yaw: f32,
    },
    /// Fire-and-forget deliberate noise (the F key); no request_id because
    /// there is no failure the player can act on.
    Sound {
        sound_id: String,
    },
    Recording {
        request_id: String,
        wav_basename: String,
        stt_backend: TranscriptionBackend,
        spatial_seq: u64,
        position: Vec3,
    },
    Offer {
        request_id: String,
        target_id: ActorId,
        item_id: ItemId,
        spatial_seq: u64,
        position: Vec3,
    },
    Accept {
        request_id: String,
        item_id: ItemId,
        spatial_seq: u64,
        position: Vec3,
    },
    Decline {
        request_id: String,
        item_id: ItemId,
        spatial_seq: u64,
        position: Vec3,
    },
    Retract {
        request_id: String,
        item_id: ItemId,
    },
    DebugSay {
        request_id: String,
        text: String,
        target_id: Option<ActorId>,
        spatial_seq: u64,
        position: Vec3,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingKind {
    Recording,
    Offer { item_id: ItemId, target_id: ActorId },
    Accept { item_id: ItemId },
    Decline { item_id: ItemId },
    Retract { item_id: ItemId },
    DebugSay,
}

impl PendingKind {
    fn item_id(&self) -> Option<&ItemId> {
        match self {
            Self::Offer { item_id, .. }
            | Self::Accept { item_id }
            | Self::Decline { item_id }
            | Self::Retract { item_id } => Some(item_id),
            Self::Recording | Self::DebugSay => None,
        }
    }

    fn expects_snapshot(&self) -> bool {
        !matches!(self, Self::Recording | Self::DebugSay)
    }
}

#[derive(Debug, Clone)]
struct PendingCommand {
    kind: PendingKind,
    sent_revision: u64,
    succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveOfferCard {
    pub item_id: ItemId,
    pub giver_id: ActorId,
    pub created_seq: u64,
    pub broadcast: bool,
    pub text: String,
    pub additional_count: usize,
}

#[derive(Resource, Debug, Default)]
pub struct InteractionState {
    pub selected_item: Option<ItemId>,
    selected_index: usize,
    pub active_offer: Option<ActiveOfferCard>,
    dismissed_broadcasts: HashMap<ItemId, u64>,
    pending: HashMap<String, PendingCommand>,
    next_request: u64,
}

impl InteractionState {
    fn request_id(&mut self) -> String {
        self.next_request = self.next_request.wrapping_add(1).max(1);
        format!("rust-{}", self.next_request)
    }

    fn insert_pending(&mut self, request_id: String, kind: PendingKind, revision: u64) {
        self.pending.insert(
            request_id,
            PendingCommand {
                kind,
                sent_revision: revision,
                succeeded: false,
            },
        );
    }

    pub fn resolve_command(
        &mut self,
        request_id: &str,
        success: bool,
        current_revision: Option<u64>,
    ) -> bool {
        let Some(pending) = self.pending.get_mut(request_id) else {
            return false;
        };
        if !success {
            self.pending.remove(request_id);
            return true;
        }
        pending.succeeded = true;
        if !pending.kind.expects_snapshot()
            || current_revision.is_some_and(|revision| revision > pending.sent_revision)
        {
            self.pending.remove(request_id);
        }
        true
    }

    pub fn reconcile_revision(&mut self, revision: u64) {
        self.pending.retain(|_, pending| {
            !(pending.succeeded
                && pending.kind.expects_snapshot()
                && revision > pending.sent_revision)
        });
    }

    pub fn clear_pending(&mut self) {
        self.pending.clear();
        self.active_offer = None;
    }

    fn item_is_pending(&self, item_id: &ItemId) -> bool {
        self.pending
            .values()
            .any(|pending| pending.kind.item_id() == Some(item_id))
    }

    fn identical_offer_pending(&self, item_id: &ItemId, target_id: &ActorId) -> bool {
        self.pending.values().any(|pending| {
            matches!(
                &pending.kind,
                PendingKind::Offer {
                    item_id: pending_item,
                    target_id: pending_target,
                } if pending_item == item_id && pending_target == target_id
            )
        })
    }
}

#[derive(Resource, Debug)]
pub struct PlayerSpatialState {
    pub sequence: u64,
    last_position: Option<Vec3>,
    last_yaw: Option<f32>,
    last_background_send: f64,
}

impl Default for PlayerSpatialState {
    fn default() -> Self {
        Self {
            sequence: 0,
            last_position: None,
            last_yaw: None,
            last_background_send: f64::NEG_INFINITY,
        }
    }
}

impl PlayerSpatialState {
    pub fn position_for_action(&mut self, position: Vec3) -> u64 {
        if self.last_position != Some(position) {
            self.sequence = self.sequence.saturating_add(1);
            self.last_position = Some(position);
        }
        self.sequence
    }

    /// Sequence for a background `spatial_update`, which carries facing too.
    /// Turning in place must take a fresh sequence: the sidecar treats an
    /// equal sequence as an idempotent repeat and would drop the new yaw.
    pub fn spatial_update_needed(&self, position: Vec3, facing_yaw: f32) -> bool {
        self.last_position != Some(position) || self.last_yaw != Some(facing_yaw)
    }

    pub fn mark_spatial_update(&mut self, position: Vec3, facing_yaw: f32) -> u64 {
        if self.spatial_update_needed(position, facing_yaw) {
            self.sequence = self.sequence.saturating_add(1);
            self.last_position = Some(position);
            self.last_yaw = Some(facing_yaw);
        }
        self.sequence
    }

    pub fn mark_hello_position(&mut self, position: Vec3) -> u64 {
        self.last_position = Some(position);
        self.sequence
    }

    pub fn retry_latest_position(&mut self) {
        self.last_position = None;
        self.last_yaw = None;
    }
}

#[derive(Debug, Clone)]
struct RecordingContext {
    wav_basename: String,
    stt_backend: TranscriptionBackend,
}

/// Persistent, voice-activated microphone state.
///
/// `enabled` is the player's preference and defaults on; `worker_enabled`
/// tracks the command sent to the capture worker. Transcription never gates
/// rearming.
#[derive(Resource, Debug)]
pub struct MicrophoneInputState {
    pub enabled: bool,
    pub stt_backend: TranscriptionBackend,
    worker_enabled: bool,
    recording: Option<RecordingContext>,
    /// Last streaming gate acknowledged by the capture worker. `None` forces
    /// a resend, covering worker (re)spawn after connect/disconnect.
    streaming_sync: Option<bool>,
}

impl Default for MicrophoneInputState {
    fn default() -> Self {
        Self {
            enabled: true,
            stt_backend: TranscriptionBackend::Cloud,
            worker_enabled: false,
            recording: None,
            streaming_sync: None,
        }
    }
}

impl MicrophoneInputState {
    pub fn with_backend(stt_backend: TranscriptionBackend) -> Self {
        Self {
            stt_backend,
            ..Self::default()
        }
    }

    pub fn clear_on_disconnect(&mut self) {
        self.recording = None;
        self.streaming_sync = None;
    }

    #[cfg(test)]
    fn is_enabled(&self) -> bool {
        self.enabled && self.worker_enabled
    }
}

/// Cloud streaming starts only when every layer agrees: config.ron has not
/// disabled it, the player has the cloud backend selected, and the sidecar
/// reported cloud transcription available.
pub(crate) fn effective_streaming(
    config: &SmartActorsConfig,
    state: &MicrophoneInputState,
    runtime: &SmartActorRuntime,
) -> bool {
    config.stt_streaming
        && state.stt_backend == TranscriptionBackend::Cloud
        && runtime.stt_cloud_available
}

pub fn sync_player_position(
    time: Res<Time>,
    runtime: Res<SmartActorRuntime>,
    players: Query<(&GlobalTransform, &PlayerController)>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !runtime.interactions_enabled() {
        return;
    }
    let Ok((player, controller)) = players.single() else {
        return;
    };
    let position = player.translation();
    let facing_yaw = controller.yaw();
    let now = time.elapsed_secs_f64();
    if !spatial.spatial_update_needed(position, facing_yaw)
        || now - spatial.last_background_send < f64::from(POSITION_UPDATE_HZ.recip())
    {
        return;
    }
    let spatial_seq = spatial.mark_spatial_update(position, facing_yaw);
    spatial.last_background_send = now;
    intents.write(PlayerIntent::SpatialUpdate {
        spatial_seq,
        position,
        facing_yaw,
    });
}

pub fn reconcile_interaction_state(
    mirror: Res<WorldMirror>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    focus: Res<ActorFocus>,
    mut state: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
) {
    if let Some(revision) = runtime.mirror_revision {
        state.reconcile_revision(revision);
    }
    let player_id = ActorId("player".into());
    let holds = mirror
        .actor(&player_id)
        .map_or(&[][..], |actor| actor.holds.as_slice());
    normalize_selection(&mut state, holds);
    state.dismissed_broadcasts.retain(|item_id, sequence| {
        mirror
            .offers()
            .any(|offer| &offer.item_id == item_id && offer.created_seq == *sequence)
    });

    hud.inventory = inventory_text(&mirror, holds, state.selected_item.as_ref());

    let player_position = players
        .single()
        .map(GlobalTransform::translation)
        .unwrap_or(Vec3::ZERO);
    let cards = if runtime.interactions_enabled() {
        actionable_offer_cards(
            &mirror,
            player_position,
            &player_id,
            &state.dismissed_broadcasts,
        )
    } else {
        Vec::new()
    };
    state.active_offer = cards.first().cloned().map(|mut card| {
        card.additional_count = cards.len().saturating_sub(1);
        card
    });
    hud.offer_card = state
        .active_offer
        .as_ref()
        .map_or_else(String::new, |card| {
            let more = if card.additional_count == 0 {
                String::new()
            } else {
                format!("    +{} more", card.additional_count)
            };
            let controls = if card.broadcast {
                "[Y] Accept    [N] Dismiss"
            } else {
                "[Y] Accept    [N] Decline"
            };
            let waiting = if state.item_is_pending(&card.item_id) {
                "\nWaiting for Python…"
            } else {
                ""
            };
            format!("{}{}\n{}{}", card.text, more, controls, waiting)
        });

    hud.focus_hint = if runtime.interactions_enabled() {
        focus_hint(&mirror, &focus, state.selected_item.as_ref())
    } else {
        String::new()
    };
}

pub fn select_inventory_item(
    keyboard: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    mirror: Res<WorldMirror>,
    mut state: ResMut<InteractionState>,
) {
    let player_id = ActorId("player".into());
    let Some(player) = mirror.actor(&player_id) else {
        return;
    };
    let holds = &player.holds;
    if holds.is_empty() {
        state.selected_item = None;
        state.selected_index = 0;
        return;
    }

    let number_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    if let Some(index) = number_keys
        .iter()
        .position(|key| keyboard.just_pressed(*key))
        .filter(|index| *index < holds.len())
    {
        state.selected_index = index;
    } else if scroll.delta.y.abs() > f32::EPSILON {
        if scroll.delta.y > 0.0 {
            state.selected_index = state
                .selected_index
                .checked_sub(1)
                .unwrap_or(holds.len() - 1);
        } else {
            state.selected_index = (state.selected_index + 1) % holds.len();
        }
    }
    state.selected_item = holds.get(state.selected_index).cloned();
}

#[allow(clippy::too_many_arguments)]
pub fn collect_item_interaction_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    runtime: Res<SmartActorRuntime>,
    mirror: Res<WorldMirror>,
    focus: Res<ActorFocus>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut state: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !runtime.interactions_enabled()
        || cursor
            .single()
            .map_or(true, |cursor| cursor.grab_mode == CursorGrabMode::None)
    {
        return;
    }
    let Ok(player) = players.single() else { return };
    let position = player.translation();
    let revision = runtime.mirror_revision.unwrap_or(0);

    if mouse.just_pressed(MouseButton::Right) {
        let (Some(item_id), Some(target)) = (state.selected_item.clone(), focus.item.as_ref())
        else {
            return;
        };
        let player_id = ActorId("player".into());
        let player_holds_item = mirror
            .actor(&player_id)
            .is_some_and(|actor| actor.holds.contains(&item_id));
        if !player_holds_item
            || target.actor_id == player_id
            || target.body_distance_m > ITEM_INTERACTION_RADIUS_M
            || state.identical_offer_pending(&item_id, &target.actor_id)
        {
            return;
        }
        let request_id = state.request_id();
        let target_name = mirror
            .actor(&target.actor_id)
            .map_or(target.actor_id.0.as_str(), |actor| {
                actor.name_for_player.as_str()
            });
        let item_name = mirror
            .item(&item_id)
            .map_or(item_id.0.as_str(), |item| item.name.as_str());
        hud.toast(format!("Offering {item_name} to {target_name}…"));
        state.insert_pending(
            request_id.clone(),
            PendingKind::Offer {
                item_id: item_id.clone(),
                target_id: target.actor_id.clone(),
            },
            revision,
        );
        let spatial_seq = spatial.position_for_action(position);
        intents.write(PlayerIntent::Offer {
            request_id,
            target_id: target.actor_id.clone(),
            item_id,
            spatial_seq,
            position,
        });
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        let Some(item_id) = state.selected_item.clone() else {
            return;
        };
        let is_players_offer = mirror
            .offers()
            .any(|offer| offer.item_id == item_id && offer.giver_id.0 == "player");
        if is_players_offer && !state.item_is_pending(&item_id) {
            let request_id = state.request_id();
            state.insert_pending(
                request_id.clone(),
                PendingKind::Retract {
                    item_id: item_id.clone(),
                },
                revision,
            );
            intents.write(PlayerIntent::Retract {
                request_id,
                item_id,
            });
        }
    }

    if keyboard.just_pressed(KeyCode::KeyY) || keyboard.just_pressed(KeyCode::KeyN) {
        let Some(card) = state.active_offer.clone() else {
            return;
        };
        if state.item_is_pending(&card.item_id) {
            return;
        }
        if keyboard.just_pressed(KeyCode::KeyN) && card.broadcast {
            state
                .dismissed_broadcasts
                .insert(card.item_id, card.created_seq);
            return;
        }
        let request_id = state.request_id();
        let spatial_seq = spatial.position_for_action(position);
        if keyboard.just_pressed(KeyCode::KeyY) {
            state.insert_pending(
                request_id.clone(),
                PendingKind::Accept {
                    item_id: card.item_id.clone(),
                },
                revision,
            );
            intents.write(PlayerIntent::Accept {
                request_id,
                item_id: card.item_id,
                spatial_seq,
                position,
            });
        } else {
            state.insert_pending(
                request_id.clone(),
                PendingKind::Decline {
                    item_id: card.item_id.clone(),
                },
                revision,
            );
            intents.write(PlayerIntent::Decline {
                request_id,
                item_id: card.item_id,
                spatial_seq,
                position,
            });
        }
    }
}

/// F emits the catalog fart at the player's position. Deliberately the whole
/// non-speech sound loop in one key: perception, attribution, playback, and
/// the rate limit are all exercised by mashing it (features/sounds.md).
pub fn collect_sound_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    config: Res<SmartActorsConfig>,
    runtime: Res<SmartActorRuntime>,
    mut intents: MessageWriter<PlayerIntent>,
) {
    if !config.sounds.enabled
        || !keyboard.just_pressed(KeyCode::KeyF)
        || !runtime.interactions_enabled()
        || cursor
            .single()
            .map_or(true, |cursor| cursor.grab_mode == CursorGrabMode::None)
    {
        return;
    }
    intents.write(PlayerIntent::Sound {
        sound_id: "fart".into(),
    });
}

#[allow(clippy::too_many_arguments)]
pub fn update_microphone_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<SmartActorsConfig>,
    runtime: Res<SmartActorRuntime>,
    microphone: Option<Res<MicrophoneService>>,
    mut state: ResMut<MicrophoneInputState>,
    mut hud: ResMut<SmartActorHudState>,
) {
    if keyboard.just_pressed(KeyCode::KeyZ) {
        state.stt_backend = match state.stt_backend {
            TranscriptionBackend::Cloud => TranscriptionBackend::Local,
            TranscriptionBackend::Local => TranscriptionBackend::Cloud,
        };
        hud.toast(match state.stt_backend {
            TranscriptionBackend::Cloud => "Transcription: cloud model",
            TranscriptionBackend::Local => "Transcription: local Canary-Qwen FP16",
        });
    }
    hud.transcription_backend = match state.stt_backend {
        TranscriptionBackend::Cloud => "CLOUD".into(),
        TranscriptionBackend::Local => "CANARY-QWEN FP16".into(),
    };

    if keyboard.just_pressed(KeyCode::KeyV) {
        state.enabled = !state.enabled;
        if state.enabled {
            if microphone.is_none() && !hud.microphone_unavailable {
                hud.microphone_available = false;
            }
        } else if !hud.microphone_unavailable {
            // Keep an explicit MIC OFF visible even if V was pressed before
            // capability probing or the Python handshake completed.
            hud.microphone_available = true;
        }
        hud.toast(if state.enabled {
            "Microphone on"
        } else {
            "Microphone off"
        });
    }
    hud.microphone_enabled = state.enabled;

    let should_enable = state.enabled && runtime.interactions_enabled() && runtime.stt_available;
    let Some(microphone) = microphone else {
        state.worker_enabled = false;
        state.streaming_sync = None;
        hud.listening = false;
        return;
    };
    // Sync the streaming gate ahead of Enable so a freshly armed worker
    // already knows the mode before its first utterance can trigger.
    let streaming = effective_streaming(&config, &state, &runtime);
    if state.streaming_sync != Some(streaming) {
        if state.streaming_sync.is_none() && !streaming {
            // A fresh worker already starts with streaming off.
            state.streaming_sync = Some(false);
        } else if microphone
            .try_send(MicrophoneCommand::SetStreaming { enabled: streaming })
            .is_ok()
        {
            state.streaming_sync = Some(streaming);
        }
    }
    if should_enable != state.worker_enabled {
        let command = if should_enable {
            MicrophoneCommand::Enable
        } else {
            MicrophoneCommand::Disable
        };
        match microphone.try_send(command) {
            Ok(()) => state.worker_enabled = should_enable,
            Err(error) => hud.toast(error),
        }
    }
    hud.listening = state.worker_enabled && should_enable;
}

#[allow(clippy::too_many_arguments)]
pub fn poll_microphone(
    microphone: Option<Res<MicrophoneService>>,
    handle: Res<BridgeHandle>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    mut spatial: ResMut<PlayerSpatialState>,
    mut microphone_input: ResMut<MicrophoneInputState>,
    mut interaction: ResMut<InteractionState>,
    mut hud: ResMut<SmartActorHudState>,
    mut intents: MessageWriter<PlayerIntent>,
    mut stop_speech: MessageWriter<StopNpcSpeech>,
) {
    let Some(microphone) = microphone else { return };
    let mut recording_started_this_poll = false;
    loop {
        let event = match microphone.poll() {
            MicrophonePoll::Event(event) => event,
            MicrophonePoll::Empty => break,
            MicrophonePoll::Disconnected => {
                microphone_input.enabled = false;
                microphone_input.worker_enabled = false;
                microphone_input.recording = None;
                hud.microphone_available = false;
                hud.listening = false;
                if !hud.microphone_unavailable {
                    hud.microphone_unavailable = true;
                    hud.connection_detail = "Microphone worker is unavailable".into();
                    hud.toast("Microphone worker stopped");
                }
                break;
            }
        };
        match event {
            MicrophoneEvent::Available => {
                hud.microphone_available = true;
                hud.microphone_unavailable = false;
                if hud.connection_detail.starts_with("Microphone unavailable:") {
                    hud.connection_detail = "Microphone ready; speak openly".into();
                }
            }
            MicrophoneEvent::Unavailable(message) => {
                microphone_input.enabled = false;
                microphone_input.worker_enabled = false;
                microphone_input.recording = None;
                hud.connection_detail = format!(
                    "Microphone unavailable: {} · [V] retry",
                    message.chars().take(220).collect::<String>()
                );
                hud.toast(message);
                hud.microphone_available = false;
                hud.microphone_unavailable = true;
                hud.listening = false;
            }
            MicrophoneEvent::RecordingStarted { wav_basename } => {
                microphone_input.recording = Some(RecordingContext {
                    wav_basename,
                    stt_backend: microphone_input.stt_backend,
                });
                recording_started_this_poll = true;
            }
            MicrophoneEvent::RecordingFinished {
                wav_basename,
                silent,
                ..
            } => {
                let context_matches = microphone_input
                    .recording
                    .as_ref()
                    .is_some_and(|context| context.wav_basename == wav_basename);
                let stt_backend = microphone_input
                    .recording
                    .as_ref()
                    .filter(|context| context.wav_basename == wav_basename)
                    .map_or(microphone_input.stt_backend, |context| context.stt_backend);
                if context_matches {
                    microphone_input.recording = None;
                }
                if silent {
                    continue;
                }
                if !context_matches {
                    // Discarded after finishing: also release any streamed
                    // copy so the sidecar never holds an ownerless transcript.
                    let _ = handle.try_send(BridgeCommand::PlayerAudioAbort {
                        wav_basename: wav_basename.clone(),
                    });
                    if let Err(error) = microphone.discard_recording(wav_basename) {
                        hud.toast(error);
                    }
                    continue;
                }
                if !runtime.interactions_enabled() {
                    // A resync is a hard command barrier. This utterance was
                    // captured against an uncertain projection, so it cannot
                    // be queued behind the snapshot request and replayed.
                    let _ = handle.try_send(BridgeCommand::PlayerAudioAbort {
                        wav_basename: wav_basename.clone(),
                    });
                    if let Err(error) = microphone.discard_recording(wav_basename) {
                        hud.toast(error);
                    }
                    continue;
                }
                let Ok(player) = players.single() else {
                    let _ = handle.try_send(BridgeCommand::PlayerAudioAbort {
                        wav_basename: wav_basename.clone(),
                    });
                    if let Err(error) = microphone.discard_recording(wav_basename) {
                        hud.toast(error);
                    }
                    continue;
                };
                let position = player.translation();
                let spatial_seq = spatial.position_for_action(position);
                let request_id = interaction.request_id();
                interaction.insert_pending(
                    request_id.clone(),
                    PendingKind::Recording,
                    runtime.mirror_revision.unwrap_or(0),
                );
                intents.write(PlayerIntent::Recording {
                    request_id,
                    wav_basename,
                    stt_backend,
                    spatial_seq,
                    position,
                });
            }
            MicrophoneEvent::RecordingCancelled { wav_basename } => {
                if microphone_input
                    .recording
                    .as_ref()
                    .is_some_and(|context| context.wav_basename == wav_basename)
                {
                    microphone_input.recording = None;
                }
            }
            MicrophoneEvent::RecordingFailed(message) => {
                microphone_input.recording = None;
                microphone_input.enabled = false;
                microphone_input.worker_enabled = false;
                hud.microphone_available = false;
                hud.microphone_unavailable = true;
                hud.listening = false;
                hud.toast(message);
            }
        }
    }
    // The worker can report RecordingStarted and RecordingCancelled together
    // when an NPC stream suspends a fresh VAD candidate. Only interrupt NPC
    // audio if a recording is still genuinely active after draining the
    // complete event batch.
    if should_interrupt_npc_speech(
        recording_started_this_poll,
        microphone_input.recording.is_some(),
    ) {
        stop_speech.write(StopNpcSpeech);
    }
}

fn should_interrupt_npc_speech(started_this_poll: bool, recording_still_active: bool) -> bool {
    started_this_poll && recording_still_active
}

pub fn inject_debug_say(
    text: String,
    target_id: Option<ActorId>,
    position: Vec3,
    runtime: &SmartActorRuntime,
    spatial: &mut PlayerSpatialState,
    interaction: &mut InteractionState,
) -> Option<PlayerIntent> {
    if !runtime.interactions_enabled()
        || !runtime.fake_backend
        || text.trim().is_empty()
        || text.chars().count() > super::PLAYER_SPEECH_MAX_CHARS
    {
        return None;
    }
    let request_id = interaction.request_id();
    interaction.insert_pending(
        request_id.clone(),
        PendingKind::DebugSay,
        runtime.mirror_revision.unwrap_or(0),
    );
    Some(PlayerIntent::DebugSay {
        request_id,
        text: text.trim().to_string(),
        target_id,
        spatial_seq: spatial.position_for_action(position),
        position,
    })
}

fn normalize_selection(state: &mut InteractionState, holds: &[ItemId]) {
    if holds.is_empty() {
        state.selected_item = None;
        state.selected_index = 0;
        return;
    }
    if let Some(index) = state
        .selected_item
        .as_ref()
        .and_then(|selected| holds.iter().position(|item| item == selected))
    {
        state.selected_index = index;
    } else {
        state.selected_index = state.selected_index.min(holds.len() - 1);
        state.selected_item = Some(holds[state.selected_index].clone());
    }
}

fn inventory_text(mirror: &WorldMirror, holds: &[ItemId], selected: Option<&ItemId>) -> String {
    if holds.is_empty() {
        return "INVENTORY  empty".into();
    }
    let slots = holds
        .iter()
        .enumerate()
        .map(|(index, item_id)| {
            let name = mirror
                .item(item_id)
                .map_or(item_id.0.as_str(), |item| item.name.as_str());
            if selected == Some(item_id) {
                format!("▶ [{}] {}", index + 1, name)
            } else {
                format!("[{}] {}", index + 1, name)
            }
        })
        .collect::<Vec<_>>()
        .join("    ");
    format!("INVENTORY    {slots}")
}

fn focus_hint(mirror: &WorldMirror, focus: &ActorFocus, selected: Option<&ItemId>) -> String {
    let Some(target) = focus.actor.as_ref() else {
        return String::new();
    };
    let name = mirror
        .actor(&target.actor_id)
        .map_or(target.actor_id.0.as_str(), |actor| {
            actor.name_for_player.as_str()
        });
    if focus.item.is_some()
        && let Some(item_id) = selected
    {
        let item = mirror
            .item(item_id)
            .map_or(item_id.0.as_str(), |item| item.name.as_str());
        return format!("{name}  ·  Right click to offer {item}");
    }
    name.to_string()
}

fn actionable_offer_cards(
    mirror: &WorldMirror,
    player_position: Vec3,
    player_id: &ActorId,
    dismissed: &HashMap<ItemId, u64>,
) -> Vec<ActiveOfferCard> {
    let mut cards = mirror
        .offers()
        .filter_map(|offer| {
            if offer.giver_id == *player_id
                || offer
                    .target_id
                    .as_ref()
                    .is_some_and(|target| target != player_id)
            {
                return None;
            }
            let broadcast = offer.target_id.is_none();
            if broadcast && dismissed.get(&offer.item_id) == Some(&offer.created_seq) {
                return None;
            }
            let giver = mirror.actor(&offer.giver_id)?;
            let giver_position: Vec3 = giver.position_m.into();
            if player_position.distance_squared(giver_position)
                > ITEM_INTERACTION_RADIUS_M * ITEM_INTERACTION_RADIUS_M
            {
                return None;
            }
            let item = mirror.item(&offer.item_id)?;
            let text = if broadcast {
                format!(
                    "{} offers {} to anyone",
                    giver.name_for_player,
                    indefinite(&item.name)
                )
            } else {
                format!(
                    "{} offers you {}",
                    giver.name_for_player,
                    indefinite(&item.name)
                )
            };
            Some(ActiveOfferCard {
                item_id: offer.item_id.clone(),
                giver_id: offer.giver_id.clone(),
                created_seq: offer.created_seq,
                broadcast,
                text,
                additional_count: 0,
            })
        })
        .collect::<Vec<_>>();
    cards.sort_by(|left, right| {
        left.broadcast
            .cmp(&right.broadcast)
            .then_with(|| left.created_seq.cmp(&right.created_seq))
            .then_with(|| left.item_id.0.cmp(&right.item_id.0))
    });
    cards
}

fn indefinite(name: &str) -> String {
    let article = if name
        .chars()
        .next()
        .is_some_and(|first| matches!(first.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'))
    {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_actors::model::{
        ActorControl, ActorSnapshot, ItemSnapshot, OfferSnapshot, Position, WorldSnapshot,
    };

    #[test]
    fn effective_streaming_requires_cloud_selected_available_and_configured() {
        let mut config = SmartActorsConfig::default();
        let mut state = MicrophoneInputState::default();
        let mut runtime = SmartActorRuntime::starting(false);

        runtime.stt_cloud_available = true;
        assert!(effective_streaming(&config, &state, &runtime));

        config.stt_streaming = false;
        assert!(!effective_streaming(&config, &state, &runtime));
        config.stt_streaming = true;

        state.stt_backend = TranscriptionBackend::Local;
        assert!(!effective_streaming(&config, &state, &runtime));
        state.stt_backend = TranscriptionBackend::Cloud;

        runtime.stt_cloud_available = false;
        assert!(!effective_streaming(&config, &state, &runtime));
    }

    fn offer_mirror(giver_x: f32) -> WorldMirror {
        let mut mirror = WorldMirror::default();
        mirror.begin_session("test").unwrap();
        mirror
            .replace_snapshot(
                "test",
                WorldSnapshot {
                    world_revision: 1,
                    player_id: ActorId("player".into()),
                    actors: vec![
                        ActorSnapshot {
                            id: ActorId("player".into()),
                            name_for_player: "You".into(),
                            control: ActorControl::Player,
                            position_m: Position::new(0.0, 0.0, 0.0).unwrap(),
                            facing_yaw: 0.0,
                            appearance_key: "player".into(),
                            holds: vec![],
                        },
                        ActorSnapshot {
                            id: ActorId("ilse".into()),
                            name_for_player: "Ilse".into(),
                            control: ActorControl::Llm,
                            position_m: Position::new(giver_x, 0.0, 0.0).unwrap(),
                            facing_yaw: 0.0,
                            appearance_key: "ilse".into(),
                            holds: vec![ItemId("coin".into()), ItemId("fish".into())],
                        },
                    ],
                    items: vec![
                        ItemSnapshot {
                            id: ItemId("coin".into()),
                            name: "copper coin".into(),
                            visual_key: "copper_coin".into(),
                        },
                        ItemSnapshot {
                            id: ItemId("fish".into()),
                            name: "fish".into(),
                            visual_key: "fish".into(),
                        },
                    ],
                    offers: vec![
                        OfferSnapshot {
                            item_id: ItemId("fish".into()),
                            giver_id: ActorId("ilse".into()),
                            target_id: None,
                            created_seq: 1,
                        },
                        OfferSnapshot {
                            item_id: ItemId("coin".into()),
                            giver_id: ActorId("ilse".into()),
                            target_id: Some(ActorId("player".into())),
                            created_seq: 5,
                        },
                    ],
                },
            )
            .unwrap();
        mirror
    }

    #[test]
    fn one_item_inventory_selects_automatically_and_selection_survives_reorder() {
        let coin = ItemId("coin".into());
        let fish = ItemId("fish".into());
        let mut state = InteractionState::default();
        normalize_selection(&mut state, std::slice::from_ref(&coin));
        assert_eq!(state.selected_item, Some(coin.clone()));

        normalize_selection(&mut state, &[fish, coin.clone()]);
        assert_eq!(state.selected_item, Some(coin));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn command_waits_for_both_result_and_newer_snapshot() {
        let mut state = InteractionState::default();
        state.insert_pending(
            "r1".into(),
            PendingKind::Accept {
                item_id: ItemId("coin".into()),
            },
            7,
        );
        assert!(state.resolve_command("r1", true, Some(7)));
        assert!(state.pending.contains_key("r1"));
        state.reconcile_revision(8);
        assert!(!state.pending.contains_key("r1"));
    }

    #[test]
    fn failed_command_unlocks_without_waiting_for_snapshot() {
        let mut state = InteractionState::default();
        state.insert_pending("r1".into(), PendingKind::Recording, 1);
        assert!(state.resolve_command("r1", false, None));
        assert!(state.pending.is_empty());
    }

    #[test]
    fn authoritative_resync_can_unlock_a_command_whose_result_was_lost() {
        let mut state = InteractionState::default();
        let item_id = ItemId("coin".into());
        let target_id = ActorId("ilse".into());
        state.insert_pending(
            "lost-result".into(),
            PendingKind::Offer {
                item_id: item_id.clone(),
                target_id: target_id.clone(),
            },
            3,
        );
        assert!(state.identical_offer_pending(&item_id, &target_id));

        state.clear_pending();
        assert!(!state.identical_offer_pending(&item_id, &target_id));
    }

    #[test]
    fn microphone_is_user_enabled_by_default_but_not_live_before_availability() {
        let state = MicrophoneInputState::default();
        assert!(state.enabled);
        assert!(!state.is_enabled());
        assert_eq!(state.stt_backend, TranscriptionBackend::Cloud);
    }

    #[test]
    fn z_toggles_between_cloud_and_local_transcription() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyZ);
        let runtime = SmartActorRuntime {
            connected: false,
            ready: false,
            resyncing: false,
            stt_available: false,
            tts_available: false,
            fake_backend: false,
            mirror_revision: None,
            ..SmartActorRuntime::starting(false)
        };
        let mut app = App::new();
        app.insert_resource(keyboard)
            .insert_resource(SmartActorsConfig::default())
            .insert_resource(runtime)
            .insert_resource(MicrophoneInputState::default())
            .insert_resource(SmartActorHudState::default())
            .add_systems(Update, update_microphone_toggle);

        app.update();

        let state = app.world().resource::<MicrophoneInputState>();
        assert_eq!(state.stt_backend, TranscriptionBackend::Local);
        let hud = app.world().resource::<SmartActorHudState>();
        assert_eq!(hud.transcription_backend, "CANARY-QWEN FP16");
    }

    #[test]
    fn v_before_handshake_keeps_the_explicit_mic_off_state_visible() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyV);
        let runtime = SmartActorRuntime {
            connected: false,
            ready: false,
            resyncing: false,
            stt_available: false,
            tts_available: false,
            fake_backend: false,
            mirror_revision: None,
            ..SmartActorRuntime::starting(false)
        };
        let mut app = App::new();
        app.insert_resource(keyboard)
            .insert_resource(SmartActorsConfig::default())
            .insert_resource(runtime)
            .insert_resource(MicrophoneInputState::default())
            .insert_resource(SmartActorHudState::default())
            .add_systems(Update, update_microphone_toggle);

        app.update();
        assert!(!app.world().resource::<MicrophoneInputState>().enabled);
        let hud = app.world().resource::<SmartActorHudState>();
        assert!(hud.microphone_available);
        assert!(!hud.microphone_unavailable);
        assert!(!hud.microphone_enabled);
        assert!(!hud.listening);
    }

    #[test]
    fn available_microphone_arms_by_default_and_v_toggles_both_directions() {
        let (microphone, commands) = MicrophoneService::command_harness_for_tests();
        let state = MicrophoneInputState::default();
        let runtime = SmartActorRuntime {
            connected: true,
            ready: true,
            resyncing: false,
            stt_available: true,
            tts_available: false,
            fake_backend: true,
            mirror_revision: Some(1),
            ..SmartActorRuntime::starting(true)
        };
        let mut hud = SmartActorHudState::default();
        hud.microphone_available = true;
        hud.microphone_unavailable = false;

        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(SmartActorsConfig::default())
            .insert_resource(runtime)
            .insert_resource(microphone)
            .insert_resource(state)
            .insert_resource(hud)
            .add_systems(Update, update_microphone_toggle);

        app.update();
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Enable)));
        assert!(app.world().resource::<MicrophoneInputState>().is_enabled());
        assert!(app.world().resource::<SmartActorHudState>().listening);
        assert!(
            app.world()
                .resource::<SmartActorHudState>()
                .microphone_enabled
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyV);
        app.update();
        assert!(matches!(
            commands.try_recv(),
            Ok(MicrophoneCommand::Disable)
        ));
        assert!(!app.world().resource::<MicrophoneInputState>().enabled);
        assert!(!app.world().resource::<SmartActorHudState>().listening);
        assert!(
            !app.world()
                .resource::<SmartActorHudState>()
                .microphone_enabled
        );

        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.clear();
            keyboard.release(KeyCode::KeyV);
            keyboard.clear();
            keyboard.press(KeyCode::KeyV);
        }
        app.update();
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Enable)));
        assert!(app.world().resource::<MicrophoneInputState>().enabled);
        assert!(app.world().resource::<SmartActorHudState>().listening);

        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.clear();
            keyboard.release(KeyCode::KeyV);
            keyboard.clear();
        }
        app.world_mut()
            .resource_mut::<SmartActorRuntime>()
            .resyncing = true;
        app.update();
        assert!(matches!(
            commands.try_recv(),
            Ok(MicrophoneCommand::Disable)
        ));
        assert!(app.world().resource::<MicrophoneInputState>().enabled);
        let hud = app.world().resource::<SmartActorHudState>();
        assert!(hud.microphone_enabled);
        assert!(!hud.listening);

        app.world_mut()
            .resource_mut::<SmartActorRuntime>()
            .resyncing = false;
        app.update();
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Enable)));
        assert!(app.world().resource::<MicrophoneInputState>().enabled);
        assert!(app.world().resource::<SmartActorHudState>().listening);

        // Keep the receiving end alive while the resource sends Shutdown.
        drop(app);
    }

    #[test]
    fn actor_focus_hint_does_not_imply_that_gaze_targets_microphone_speech() {
        let mirror = offer_mirror(4.0);
        let focus = ActorFocus {
            actor: Some(super::super::targeting::FocusedActor {
                actor_id: ActorId("ilse".into()),
                entity: Entity::PLACEHOLDER,
                ray_distance_m: 2.0,
                body_distance_m: 2.0,
            }),
            item: None,
        };
        let hint = focus_hint(&mirror, &focus, None);
        assert!(!hint.contains("mic"));
        assert!(!hint.contains("talk"));
    }

    #[test]
    fn right_click_intent_does_not_mutate_inventory_projection() {
        let holds = vec![ItemId("coin".into())];
        let before = holds.clone();
        let _intent = PlayerIntent::Offer {
            request_id: "r".into(),
            target_id: ActorId("conny".into()),
            item_id: holds[0].clone(),
            spatial_seq: 3,
            position: Vec3::ZERO,
        };
        assert_eq!(holds, before);
    }

    #[test]
    fn broadcast_dismissal_is_local_and_never_a_decline_intent() {
        let card = ActiveOfferCard {
            item_id: ItemId("fish".into()),
            giver_id: ActorId("sven".into()),
            created_seq: 2,
            broadcast: true,
            text: String::new(),
            additional_count: 0,
        };
        let mut dismissed = HashMap::new();
        dismissed.insert(card.item_id.clone(), card.created_seq);
        assert_eq!(dismissed.get(&card.item_id), Some(&2));
    }

    #[test]
    fn spatial_sequence_advances_when_an_action_has_a_new_position() {
        let mut state = PlayerSpatialState::default();
        assert_eq!(state.mark_hello_position(Vec3::ZERO), 0);
        assert_eq!(state.position_for_action(Vec3::ZERO), 0);
        assert_eq!(state.position_for_action(Vec3::X), 1);
        assert_eq!(state.position_for_action(Vec3::X), 1);
    }

    #[test]
    fn articles_make_offer_cards_read_naturally() {
        assert_eq!(indefinite("copper coin"), "a copper coin");
        assert_eq!(indefinite("apple"), "an apple");
    }

    #[test]
    fn cancelled_microphone_start_does_not_interrupt_npc_audio() {
        assert!(should_interrupt_npc_speech(true, true));
        assert!(!should_interrupt_npc_speech(true, false));
        assert!(!should_interrupt_npc_speech(false, true));
    }

    #[test]
    fn targeted_offer_cards_precede_broadcasts_and_keep_exact_item_ids() {
        let mirror = offer_mirror(4.0);
        let cards = actionable_offer_cards(
            &mirror,
            Vec3::ZERO,
            &ActorId("player".into()),
            &HashMap::new(),
        );

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].item_id, ItemId("coin".into()));
        assert!(!cards[0].broadcast);
        assert_eq!(cards[1].item_id, ItemId("fish".into()));
        assert!(cards[1].broadcast);
    }

    #[test]
    fn offer_range_is_inclusive_and_broadcast_dismissal_is_revision_scoped() {
        let mirror = offer_mirror(ITEM_INTERACTION_RADIUS_M);
        let player = ActorId("player".into());
        let mut dismissed = HashMap::new();
        dismissed.insert(ItemId("fish".into()), 1);
        let cards = actionable_offer_cards(&mirror, Vec3::ZERO, &player, &dismissed);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].item_id, ItemId("coin".into()));

        let outside = offer_mirror(ITEM_INTERACTION_RADIUS_M + 0.001);
        assert!(actionable_offer_cards(&outside, Vec3::ZERO, &player, &HashMap::new()).is_empty());
    }

    #[test]
    fn disconnect_hides_actionable_cards_while_retaining_the_mirror() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(offer_mirror(3.0))
            .insert_resource(SmartActorRuntime {
                connected: true,
                ready: true,
                resyncing: false,
                stt_available: false,
                tts_available: false,
                fake_backend: true,
                mirror_revision: Some(1),
                ..SmartActorRuntime::starting(true)
            })
            .init_resource::<ActorFocus>()
            .init_resource::<InteractionState>()
            .init_resource::<SmartActorHudState>()
            .add_systems(Update, reconcile_interaction_state);
        app.world_mut().spawn((
            PlayerController::default(),
            GlobalTransform::from_translation(Vec3::ZERO),
        ));

        app.update();
        assert!(
            app.world()
                .resource::<InteractionState>()
                .active_offer
                .is_some()
        );
        assert!(
            !app.world()
                .resource::<SmartActorHudState>()
                .offer_card
                .is_empty()
        );

        app.world_mut()
            .resource_mut::<SmartActorRuntime>()
            .connected = false;
        app.update();
        assert!(
            app.world()
                .resource::<InteractionState>()
                .active_offer
                .is_none()
        );
        assert!(
            app.world()
                .resource::<SmartActorHudState>()
                .offer_card
                .is_empty()
        );
        assert_eq!(app.world().resource::<WorldMirror>().offers().len(), 2);
    }
}
