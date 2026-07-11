//! Python-authoritative smart actors and their non-blocking Bevy projection.

pub mod actors;
pub mod bridge;
pub mod model;

mod hud;
mod interaction;
mod microphone;
mod speech;
mod targeting;

use bevy::prelude::*;
use bevy::transform::TransformSystems;
use serde::Deserialize;
use std::path::PathBuf;

pub use targeting::ActorFocus;

pub const HEARING_RADIUS_M: f32 = 20.0;
pub const ITEM_INTERACTION_RADIUS_M: f32 = 4.0;
pub const PLAYER_SPEECH_MAX_SECONDS: u32 = 15;
pub const PLAYER_SPEECH_MAX_CHARS: usize = 500;
pub const POSITION_UPDATE_HZ: f32 = 10.0;

/// Non-secret client-side sidecar settings loaded from `config.ron`.
#[derive(Resource, Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SmartActorsConfig {
    pub enabled: bool,
    pub fake_backend: bool,
    pub uv_binary: String,
    pub server_script: String,
}

impl Default for SmartActorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fake_backend: false,
            uv_binary: "uv".into(),
            server_script: "prompt_playgound/server.py".into(),
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SmartActorSet {
    DrainBridge,
    ReconcileMirror,
    UpdateFocus,
    CollectInput,
    Present,
    StartAudio,
}

pub struct SmartActorsPlugin {
    config: SmartActorsConfig,
}

/// Connection/capability state shared by input and presentation systems.
#[derive(Resource, Debug, Clone)]
pub struct SmartActorRuntime {
    pub connected: bool,
    pub ready: bool,
    /// True while the semantic mirror is waiting for an authoritative
    /// replacement snapshot. No player command may cross this barrier.
    pub resyncing: bool,
    pub stt_available: bool,
    pub tts_available: bool,
    pub fake_backend: bool,
    pub mirror_revision: Option<u64>,
}

impl SmartActorRuntime {
    fn starting(fake_backend: bool) -> Self {
        Self {
            connected: false,
            ready: false,
            resyncing: false,
            stt_available: false,
            tts_available: false,
            fake_backend,
            mirror_revision: None,
        }
    }

    pub fn interactions_enabled(&self) -> bool {
        self.connected && self.ready && !self.resyncing
    }
}

impl SmartActorsPlugin {
    pub fn new(config: SmartActorsConfig) -> Self {
        Self { config }
    }
}

impl Plugin for SmartActorsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .init_resource::<hud::SmartActorHudState>()
            .add_systems(Startup, hud::spawn_smart_actor_hud);

        if !self.config.enabled {
            let mut hud = hud::SmartActorHudState::default();
            hud.connection = hud::ConnectionUiState::Disabled;
            hud.connection_detail = "Disabled in config.ron".into();
            app.insert_resource(hud)
                .add_systems(Update, hud::update_smart_actor_hud);
            return;
        }

        let script = PathBuf::from(&self.config.server_script);
        let script = if script.is_absolute() {
            script
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(script)
        };
        let (handle, inbox, worker) = bridge::spawn_sidecar(bridge::BridgeLaunchConfig {
            uv_binary: self.config.uv_binary.clone(),
            server_script: script,
            fake_backend: self.config.fake_backend,
        });
        let mut mirror = model::WorldMirror::default();
        mirror
            .begin_session(handle.session_id().to_owned())
            .expect("generated session IDs are valid");

        app.insert_resource(handle)
            .insert_resource(inbox)
            .insert_resource(worker)
            .insert_resource(mirror)
            .insert_resource(SmartActorRuntime::starting(self.config.fake_backend))
            .init_resource::<HandshakeRecovery>()
            .init_resource::<ActorFocus>()
            .init_resource::<interaction::InteractionState>()
            .init_resource::<interaction::PlayerSpatialState>()
            .init_resource::<interaction::MicrophoneInputState>()
            .init_resource::<speech::SpeechPresentationState>()
            .add_message::<interaction::PlayerIntent>()
            .add_message::<InjectPlayerTranscript>()
            .add_message::<speech::PresentSpeech>()
            .add_message::<speech::TtsClipReady>()
            .add_message::<speech::StopNpcSpeech>()
            .add_message::<speech::ClearSpeechPresentation>()
            .configure_sets(
                PostUpdate,
                (
                    SmartActorSet::DrainBridge,
                    SmartActorSet::ReconcileMirror,
                    SmartActorSet::UpdateFocus,
                    SmartActorSet::CollectInput,
                    SmartActorSet::Present,
                    SmartActorSet::StartAudio,
                )
                    .chain()
                    .after(TransformSystems::Propagate),
            )
            .add_systems(
                Startup,
                actors::setup_actor_visual_assets.after(hud::spawn_smart_actor_hud),
            )
            .add_systems(
                PostUpdate,
                (drain_bridge_messages, retry_pending_resync)
                    .chain()
                    .in_set(SmartActorSet::DrainBridge),
            )
            .add_systems(
                PostUpdate,
                (
                    actors::reconcile_actor_views,
                    actors::reconcile_offered_item_views,
                    interaction::reconcile_interaction_state,
                )
                    .chain()
                    .in_set(SmartActorSet::ReconcileMirror),
            )
            .add_systems(
                PostUpdate,
                targeting::update_actor_focus.in_set(SmartActorSet::UpdateFocus),
            )
            .add_systems(
                PostUpdate,
                (
                    interaction::select_inventory_item,
                    interaction::sync_player_position,
                    interaction::poll_microphone,
                    interaction::collect_item_interaction_input,
                    interaction::update_microphone_toggle,
                    collect_injected_transcripts,
                    forward_player_intents,
                )
                    .chain()
                    .in_set(SmartActorSet::CollectInput),
            )
            .add_systems(
                PostUpdate,
                (
                    actors::position_actor_name_labels,
                    actors::animate_offered_items,
                    speech::receive_speech_events,
                    speech::receive_tts_clips,
                    speech::clear_speech_presentation,
                    speech::stop_npc_speech_for_capture,
                    speech::update_speech_bubbles,
                    speech::update_subtitle_hud,
                    hud::update_smart_actor_hud,
                )
                    .chain()
                    .in_set(SmartActorSet::Present),
            )
            .add_systems(
                PostUpdate,
                speech::start_ready_audio.in_set(SmartActorSet::StartAudio),
            );
    }
}

/// Developer/test-only transcript injection. The Python service accepts it
/// exclusively in deterministic fake mode and still applies the normal `say`
/// validator; production has no typed-chat path.
#[derive(Message, Debug, Clone)]
pub struct InjectPlayerTranscript {
    pub text: String,
    pub target_id: Option<model::ActorId>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilitiesWire {
    llm: bool,
    stt: bool,
    tts: bool,
}

#[derive(Resource, Default)]
struct HandshakeRecovery {
    capabilities: Option<CapabilitiesWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadyWire {
    capabilities: CapabilitiesWire,
    snapshot: model::WorldSnapshot,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeechWire {
    event_id: String,
    speaker_id: model::ActorId,
    target_id: Option<model::ActorId>,
    text: String,
    speaker_position_m: model::Position,
    recipient_ids: Vec<model::ActorId>,
    speaker_name_for_player: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldEventWire {
    event_id: String,
    kind: String,
    actor_id: model::ActorId,
    target_id: Option<model::ActorId>,
    item_id: Option<model::ItemId>,
    recipient_ids: Vec<model::ActorId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TranscriptionWire {
    request_id: String,
    text: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandResultWire {
    request_id: String,
    success: bool,
    error_code: Option<String>,
    message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusWire {
    subsystem: String,
    state: String,
    actor_id: Option<model::ActorId>,
    message: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn drain_bridge_messages(
    mut commands: Commands,
    inbox: Res<bridge::BridgeInbox>,
    handle: Res<bridge::BridgeHandle>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    microphone: Option<Res<microphone::MicrophoneService>>,
    mut mirror: ResMut<model::WorldMirror>,
    mut runtime: ResMut<SmartActorRuntime>,
    mut handshake_recovery: ResMut<HandshakeRecovery>,
    mut hud: ResMut<hud::SmartActorHudState>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut microphone_input: ResMut<interaction::MicrophoneInputState>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut speech_messages: MessageWriter<speech::PresentSpeech>,
    mut audio_messages: MessageWriter<speech::TtsClipReady>,
    mut clear_speech: MessageWriter<speech::ClearSpeechPresentation>,
) {
    // Resource insertions/removals are deferred. Track what this drain pass
    // has queued so several buffered server messages cannot spawn several
    // microphone workers before commands are applied.
    let mut microphone_present = microphone.is_some();
    while let Some(event) = inbox.try_recv() {
        match event {
            bridge::BridgeEvent::ProcessStarted => {
                runtime.connected = true;
                runtime.ready = false;
                runtime.resyncing = false;
                handshake_recovery.capabilities = None;
                hud.connection = hud::ConnectionUiState::Starting;
                hud.connection_detail = "Python started; handshaking…".into();
                let Ok(player) = players.single() else {
                    hud.clear_transients_on_disconnect("Player transform is unavailable");
                    continue;
                };
                let position = player.translation();
                let Ok(position_m) = model::Position::try_from(position) else {
                    hud.clear_transients_on_disconnect("Player position is invalid");
                    continue;
                };
                let spatial_seq = spatial.mark_hello_position(position);
                if let Err(error) = handle.try_send(bridge::BridgeCommand::Hello {
                    position_m,
                    spatial_seq,
                }) {
                    hud.clear_transients_on_disconnect(error);
                }
            }
            bridge::BridgeEvent::Message(envelope) => {
                // Lifecycle and protocol events come from separate worker
                // threads. Once the sole child is known dead, never let a
                // buffered late message revive interactions against stale
                // state; this slice has no in-place restart.
                if !runtime.connected {
                    continue;
                }
                let is_snapshot =
                    matches!(envelope.message_type.as_str(), "ready" | "world_snapshot");
                match mirror.observe_event(handle.session_id(), envelope.event_seq) {
                    Ok(()) => {}
                    Err(error @ model::MirrorError::EventSequenceGap { .. }) => {
                        runtime.resyncing = true;
                        request_resync(&handle, &mirror);
                        hud.toast(error.to_string());
                        if !is_snapshot {
                            continue;
                        }
                    }
                    Err(model::MirrorError::StaleEventSequence { .. }) => continue,
                    Err(error) => {
                        hud.toast(error.to_string());
                        continue;
                    }
                }
                if mirror.needs_resync() && !is_snapshot {
                    continue;
                }
                process_server_message(
                    envelope,
                    &handle,
                    &mut mirror,
                    &mut runtime,
                    &mut handshake_recovery,
                    &mut hud,
                    &mut interaction,
                    &mut speech_messages,
                );
                // Do not open the default input device before the Python
                // handshake confirms that transcription is configured.
                if runtime.ready && runtime.stt_available && !microphone_present {
                    commands.insert_resource(microphone::MicrophoneService::spawn(
                        handle.runtime_dir().to_path_buf(),
                    ));
                    microphone_present = true;
                }
            }
            bridge::BridgeEvent::TtsAudio {
                speech_event_id,
                wav_bytes,
            } => {
                if runtime.connected {
                    audio_messages.write(speech::TtsClipReady {
                        event_id: speech_event_id,
                        wav_bytes,
                    });
                }
            }
            bridge::BridgeEvent::Degraded(message) => {
                if runtime.connected {
                    hud.connection_detail = truncate_owned(message, 300);
                }
            }
            bridge::BridgeEvent::Disconnected(message) => {
                runtime.connected = false;
                runtime.ready = false;
                runtime.resyncing = false;
                runtime.stt_available = false;
                runtime.tts_available = false;
                handshake_recovery.capabilities = None;
                interaction.clear_pending();
                microphone_input.clear_on_disconnect();
                hud.clear_transients_on_disconnect(truncate_owned(message, 300));
                clear_speech.write(speech::ClearSpeechPresentation);
                if microphone_present {
                    commands.remove_resource::<microphone::MicrophoneService>();
                    microphone_present = false;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_server_message(
    envelope: bridge::ServerEnvelope,
    handle: &bridge::BridgeHandle,
    mirror: &mut model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    handshake_recovery: &mut HandshakeRecovery,
    hud: &mut hud::SmartActorHudState,
    interaction: &mut interaction::InteractionState,
    speech_messages: &mut MessageWriter<speech::PresentSpeech>,
) {
    let message_type = envelope.message_type;
    let payload = envelope.payload;
    match message_type.as_str() {
        "ready" => {
            // Keep independently valid capabilities even if the bundled
            // snapshot is malformed. The requested replacement snapshot does
            // not repeat them, so this is what lets the initial handshake
            // recover instead of remaining permanently offline.
            let recoverable_capabilities = recoverable_ready_capabilities(&payload);
            match serde_json::from_value::<ReadyWire>(payload) {
                Ok(ready) => {
                    handshake_recovery.capabilities = Some(ready.capabilities);
                    if accept_snapshot(handle, mirror, runtime, hud, ready.snapshot) {
                        apply_ready_capabilities(runtime, hud, ready.capabilities);
                        handshake_recovery.capabilities = None;
                    }
                }
                Err(error) => {
                    if let Some(capabilities) = recoverable_capabilities {
                        handshake_recovery.capabilities = Some(capabilities);
                    }
                    malformed_payload(handle, mirror, runtime, hud, "ready", error);
                }
            }
        }
        "world_snapshot" => match serde_json::from_value::<model::WorldSnapshot>(payload) {
            Ok(snapshot) => {
                let completes_resync = mirror.needs_resync();
                if accept_snapshot(handle, mirror, runtime, hud, snapshot) {
                    let handshake_failed = if completes_resync && !runtime.ready {
                        if let Some(capabilities) = handshake_recovery.capabilities {
                            apply_ready_capabilities(runtime, hud, capabilities);
                            false
                        } else {
                            mark_handshake_unrecoverable(runtime, hud);
                            true
                        }
                    } else {
                        false
                    };
                    if completes_resync {
                        handshake_recovery.capabilities = None;
                        // The authoritative snapshot supersedes every
                        // uncertain in-flight projection. A command result may
                        // have been the event lost at the sequence gap, so
                        // never leave controls locked waiting for it.
                        interaction.clear_pending();
                        if !handshake_failed {
                            hud.toast("Actor world resynchronized");
                        }
                    }
                }
            }
            Err(error) => malformed_payload(handle, mirror, runtime, hud, "world_snapshot", error),
        },
        "speech" => match serde_json::from_value::<SpeechWire>(payload) {
            Ok(speech) if valid_speech(&speech, mirror) => {
                let player_heard = speech.speaker_id.0 == "player"
                    || speech
                        .recipient_ids
                        .iter()
                        .any(|actor_id| actor_id.0 == "player");
                if player_heard {
                    let recipient_count = speech
                        .recipient_ids
                        .iter()
                        .filter(|recipient| {
                            *recipient != &speech.speaker_id && mirror.actor(recipient).is_some()
                        })
                        .count();
                    speech_messages.write(speech::PresentSpeech {
                        event_seq: envelope.event_seq,
                        event_id: speech.event_id,
                        speaker_id: speech.speaker_id.clone(),
                        speaker_label: speech.speaker_name_for_player,
                        text: speech.text,
                        speaker_position: speech.speaker_position_m.into(),
                        recipient_count,
                        expect_audio: runtime.tts_available && speech.speaker_id.0 != "player",
                    });
                }
            }
            Ok(_) => hud.toast("Discarded invalid speech data from Python"),
            Err(error) => hud.toast(format!("Malformed speech event: {error}")),
        },
        "world_event" => {
            match serde_json::from_value::<WorldEventWire>(payload) {
                Ok(event) => {
                    // This is presentation feedback only. Offers and ownership
                    // still reconcile exclusively from authoritative snapshots.
                    if let Some(text) = describe_world_event(&event, mirror) {
                        hud.toast(text);
                    }
                }
                Err(error) => hud.toast(format!("Malformed world event: {error}")),
            }
        }
        "transcription_result" => match serde_json::from_value::<TranscriptionWire>(payload) {
            Ok(result) => {
                if let Some(text) = result.text.filter(|text| valid_ui_text(text, 500)) {
                    // This is the earliest exact confirmation of what STT
                    // understood. It has a dedicated bottom caption so later
                    // status/world-event toasts cannot overwrite it.
                    hud.show_player_transcript(&text);
                } else if let Some(error) = result.error {
                    hud.toast(truncate_owned(error, 300));
                }
                let _ = result.request_id;
            }
            Err(error) => hud.toast(format!("Malformed transcription result: {error}")),
        },
        "command_result" => {
            let request_id_hint = payload
                .get("request_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            match serde_json::from_value::<CommandResultWire>(payload) {
                Ok(result) => {
                    let known = interaction.resolve_command(
                        &result.request_id,
                        result.success,
                        runtime.mirror_revision,
                    );
                    if known && !result.success {
                        let code =
                            truncate_owned(result.error_code.unwrap_or_else(|| "error".into()), 64);
                        hud.toast(format!("{}: {}", code, truncate_owned(result.message, 260)));
                    }
                }
                Err(error) => {
                    if let Some(request_id) = request_id_hint {
                        interaction.resolve_command(&request_id, false, None);
                    } else {
                        interaction.clear_pending();
                    }
                    hud.toast(format!("Malformed command result: {error}"));
                }
            }
        }
        "status" => match serde_json::from_value::<StatusWire>(payload) {
            Ok(status) => apply_status(status, mirror, hud),
            Err(error) => hud.toast(format!("Malformed actor status: {error}")),
        },
        "tts_ready" => {}
        _ => {
            hud.connection_detail = format!("Ignored unknown actor message: {message_type}");
        }
    }
}

fn recoverable_ready_capabilities(payload: &serde_json::Value) -> Option<CapabilitiesWire> {
    payload
        .get("capabilities")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn connection_detail_for_capabilities(llm: bool, stt: bool, tts: bool) -> String {
    let mut unavailable = Vec::new();
    if !llm {
        unavailable.push("NPC cognition");
    }
    if !stt {
        unavailable.push("microphone transcription");
    }
    if !tts {
        unavailable.push("NPC voice audio");
    }
    if unavailable.is_empty() {
        "Local Python authority connected".into()
    } else {
        format!("Connected; unavailable: {}", unavailable.join(", "))
    }
}

fn apply_ready_capabilities(
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    capabilities: CapabilitiesWire,
) {
    runtime.ready = true;
    runtime.connected = true;
    runtime.resyncing = false;
    runtime.stt_available = capabilities.stt;
    runtime.tts_available = capabilities.tts;
    hud.connection = hud::ConnectionUiState::Online;
    if capabilities.stt {
        // Preserve an explicit pre-handshake MIC OFF choice; otherwise the
        // worker's Available event reveals the default-on state.
        hud.microphone_unavailable = false;
    } else {
        hud.microphone_available = false;
        hud.microphone_unavailable = true;
    }
    hud.listening = false;
    hud.connection_detail =
        connection_detail_for_capabilities(capabilities.llm, capabilities.stt, capabilities.tts);
}

fn mark_handshake_unrecoverable(
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
) {
    runtime.ready = false;
    runtime.stt_available = false;
    runtime.tts_available = false;
    hud.connection = hud::ConnectionUiState::Offline;
    hud.connection_detail =
        "Actor handshake failed: ready capabilities were missing or invalid".into();
    hud.microphone_available = false;
    hud.microphone_unavailable = true;
    hud.listening = false;
    hud.toast("Actor handshake could not recover");
}

fn accept_snapshot(
    handle: &bridge::BridgeHandle,
    mirror: &mut model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    snapshot: model::WorldSnapshot,
) -> bool {
    match mirror.replace_snapshot(handle.session_id(), snapshot) {
        Ok(revision) => {
            runtime.mirror_revision = Some(revision);
            runtime.resyncing = false;
            true
        }
        Err(model::MirrorError::StaleRevision { .. }) => false,
        Err(error) => {
            if error.requires_resync() {
                runtime.resyncing = true;
                request_resync(handle, mirror);
            }
            hud.toast(error.to_string());
            false
        }
    }
}

fn malformed_payload(
    handle: &bridge::BridgeHandle,
    mirror: &mut model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    message_type: &str,
    error: serde_json::Error,
) {
    mirror.mark_resync_needed();
    runtime.resyncing = true;
    request_resync(handle, mirror);
    hud.toast(format!("Malformed {message_type}: {error}"));
}

fn request_resync(handle: &bridge::BridgeHandle, mirror: &model::WorldMirror) {
    let _ = handle.try_send(bridge::BridgeCommand::ResyncRequest {
        last_world_revision: mirror.revision().unwrap_or(0),
    });
}

/// A full command queue must not strand the mirror in resync mode forever.
/// Retry at a modest cadence until an authoritative snapshot clears the flag.
fn retry_pending_resync(
    time: Res<Time>,
    handle: Res<bridge::BridgeHandle>,
    mirror: Res<model::WorldMirror>,
    runtime: Res<SmartActorRuntime>,
    mut next_attempt_at: Local<f64>,
) {
    if !mirror.needs_resync() || !runtime.connected {
        *next_attempt_at = 0.0;
        return;
    }
    let now = time.elapsed_secs_f64();
    if now < *next_attempt_at {
        return;
    }
    let retry_delay = if handle
        .try_send(bridge::BridgeCommand::ResyncRequest {
            last_world_revision: mirror.revision().unwrap_or(0),
        })
        .is_ok()
    {
        0.5
    } else {
        0.1
    };
    *next_attempt_at = now + retry_delay;
}

fn apply_status(
    mut status: StatusWire,
    mirror: &model::WorldMirror,
    hud: &mut hud::SmartActorHudState,
) {
    status.subsystem = truncate_owned(status.subsystem, 64);
    status.state = truncate_owned(status.state, 64);
    status.message = status.message.map(|message| truncate_owned(message, 300));
    let actor = status
        .actor_id
        .as_ref()
        .and_then(|id| mirror.actor(id))
        .map(|actor| actor.name_for_player.as_str());
    hud.connection_detail = match (status.subsystem.as_str(), status.state.as_str(), actor) {
        ("llm", "thinking", Some(actor)) => format!("{actor} is thinking…"),
        ("stt", "transcribing", _) => "Transcribing your speech…".into(),
        ("tts", "synthesizing", Some(actor)) => format!("Preparing {actor}'s voice…"),
        (_, state, _) => status
            .message
            .clone()
            .unwrap_or_else(|| format!("{}: {state}", status.subsystem)),
    };
    hud.connection_detail = truncate_owned(std::mem::take(&mut hud.connection_detail), 300);
    if matches!(status.state.as_str(), "degraded" | "unavailable")
        && let Some(message) = status.message
    {
        hud.toast(truncate_owned(message, 300));
    }
}

fn valid_speech(speech: &SpeechWire, mirror: &model::WorldMirror) -> bool {
    valid_wire_id(&speech.event_id)
        && valid_wire_id(&speech.speaker_id.0)
        && speech
            .target_id
            .as_ref()
            .is_none_or(|target| valid_wire_id(&target.0))
        && speech
            .recipient_ids
            .iter()
            .all(|recipient| valid_wire_id(&recipient.0))
        && mirror.actor(&speech.speaker_id).is_some()
        && valid_ui_text(&speech.speaker_name_for_player, 256)
        && valid_ui_text(&speech.text, PLAYER_SPEECH_MAX_CHARS)
}

fn describe_world_event(event: &WorldEventWire, mirror: &model::WorldMirror) -> Option<String> {
    if !valid_wire_id(&event.event_id)
        || !valid_wire_id(&event.actor_id.0)
        || event.kind.len() > 64
        || event
            .recipient_ids
            .iter()
            .any(|recipient| !valid_wire_id(&recipient.0))
    {
        return None;
    }
    let player_id = mirror.player_id()?;
    if &event.actor_id != player_id && !event.recipient_ids.contains(player_id) {
        return None;
    }
    let player_acted = &event.actor_id == player_id;
    let actor = if player_acted {
        "You"
    } else {
        mirror.actor(&event.actor_id)?.name_for_player.as_str()
    };
    let item = event
        .item_id
        .as_ref()
        .and_then(|item_id| mirror.item(item_id))
        .map(|item| item.name.as_str())?;
    let offer_verb = if player_acted { "offer" } else { "offers" };
    match event.kind.as_str() {
        "offer_item" if event.target_id.as_ref() == Some(player_id) => {
            Some(format!("{actor} {offer_verb} you the {item}"))
        }
        "offer_item" if event.target_id.is_none() => {
            Some(format!("{actor} {offer_verb} the {item} openly"))
        }
        // A targeted offer between other actors is not feedback for the
        // player. In particular, do not let it overwrite the preceding
        // retract event when an offer is redirected away from the player.
        "offer_item" => None,
        "accept_offered_item" => Some(if player_acted {
            format!("You accept the {item}")
        } else {
            format!("{actor} accepts the {item}")
        }),
        "decline_offer" => Some(if player_acted {
            format!("You decline the {item}")
        } else {
            format!("{actor} declines the {item}")
        }),
        "retract_offer" => Some(if player_acted {
            format!("You withdraw the {item} offer")
        } else {
            format!("{actor} withdraws the {item} offer")
        }),
        "eat" => Some(if player_acted {
            format!("You eat the {item}")
        } else {
            format!("{actor} eats the {item}")
        }),
        _ => None,
    }
}

fn valid_wire_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn valid_ui_text(value: &str, max_chars: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= max_chars
        && value
            .chars()
            .all(|character| !character.is_control() || matches!(character, '\n' | '\t'))
}

fn truncate_owned(mut value: String, maximum_chars: usize) -> String {
    if let Some((byte, _)) = value.char_indices().nth(maximum_chars) {
        value.truncate(byte);
    }
    value
}

fn forward_player_intents(
    handle: Res<bridge::BridgeHandle>,
    microphone: Option<Res<microphone::MicrophoneService>>,
    runtime: Res<SmartActorRuntime>,
    mut intents: MessageReader<interaction::PlayerIntent>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut hud: ResMut<hud::SmartActorHudState>,
) {
    for intent in intents.read() {
        let request_id = intent_request_id(intent).map(str::to_owned);
        let is_spatial = matches!(intent, interaction::PlayerIntent::SpatialUpdate { .. });
        let failed_recording = match intent {
            interaction::PlayerIntent::Recording { wav_basename, .. } => Some(wav_basename.clone()),
            _ => None,
        };
        if !runtime.interactions_enabled() {
            if is_spatial {
                spatial.retry_latest_position();
            }
            if let Some(request_id) = request_id {
                interaction.resolve_command(&request_id, false, None);
            }
            if let (Some(wav_basename), Some(microphone)) = (failed_recording, &microphone)
                && let Err(error) = microphone.discard_recording(wav_basename)
            {
                hud.toast(error);
            }
            continue;
        }
        let delivery = intent_to_command(intent).and_then(|command| handle.try_send(command));
        if let Err(error) = delivery {
            if is_spatial {
                spatial.retry_latest_position();
            }
            if let Some(request_id) = request_id {
                interaction.resolve_command(&request_id, false, None);
            }
            if let (Some(wav_basename), Some(microphone)) = (failed_recording, &microphone)
                && let Err(cleanup_error) = microphone.discard_recording(wav_basename)
            {
                hud.toast(cleanup_error);
            }
            if !error.contains("spatial update coalesced") {
                hud.toast(error);
            }
        }
    }
}

fn intent_to_command(intent: &interaction::PlayerIntent) -> Result<bridge::BridgeCommand, String> {
    let position = |value| {
        model::Position::try_from(value).map_err(|_| "player position is invalid".to_string())
    };
    Ok(match intent {
        interaction::PlayerIntent::SpatialUpdate {
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::SpatialUpdate {
            spatial_seq: *spatial_seq,
            position_m: position(*value)?,
        },
        interaction::PlayerIntent::Recording {
            request_id,
            wav_basename,
            stt_backend,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerRecording {
            request_id: request_id.clone(),
            wav_basename: wav_basename.clone(),
            stt_backend: *stt_backend,
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Offer {
            request_id,
            target_id,
            item_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerOffer {
            request_id: request_id.clone(),
            target_id: target_id.clone(),
            item_id: item_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Accept {
            request_id,
            item_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerAccept {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Decline {
            request_id,
            item_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::PlayerDecline {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
        interaction::PlayerIntent::Retract {
            request_id,
            item_id,
        } => bridge::BridgeCommand::PlayerRetract {
            request_id: request_id.clone(),
            item_id: item_id.clone(),
        },
        interaction::PlayerIntent::DebugSay {
            request_id,
            text,
            target_id,
            spatial_seq,
            position: value,
        } => bridge::BridgeCommand::DebugPlayerSay {
            request_id: request_id.clone(),
            text: text.clone(),
            target_id: target_id.clone(),
            position_m: position(*value)?,
            spatial_seq: *spatial_seq,
        },
    })
}

fn intent_request_id(intent: &interaction::PlayerIntent) -> Option<&str> {
    match intent {
        interaction::PlayerIntent::SpatialUpdate { .. } => None,
        interaction::PlayerIntent::Recording { request_id, .. }
        | interaction::PlayerIntent::Offer { request_id, .. }
        | interaction::PlayerIntent::Accept { request_id, .. }
        | interaction::PlayerIntent::Decline { request_id, .. }
        | interaction::PlayerIntent::Retract { request_id, .. }
        | interaction::PlayerIntent::DebugSay { request_id, .. } => Some(request_id),
    }
}

fn collect_injected_transcripts(
    mut injected: MessageReader<InjectPlayerTranscript>,
    runtime: Res<SmartActorRuntime>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut intents: MessageWriter<interaction::PlayerIntent>,
) {
    let Ok(player) = players.single() else { return };
    for injection in injected.read() {
        if let Some(intent) = interaction::inject_debug_say(
            injection.text.clone(),
            injection.target_id.clone(),
            player.translation(),
            &runtime,
            &mut spatial,
            &mut interaction,
        ) {
            intents.write(intent);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{process::Stdio, thread, time::Duration};

    use bevy::{
        asset::{AssetApp, AssetPlugin},
        audio::AudioSource,
        input::mouse::AccumulatedMouseScroll,
        transform::TransformPlugin,
        window::{CursorGrabMode, CursorOptions, PrimaryWindow},
    };

    use super::*;

    #[test]
    fn settled_spatial_constants_match_the_protocol_contract() {
        assert_eq!(HEARING_RADIUS_M, 20.0);
        assert_eq!(targeting::ACTOR_FOCUS_RADIUS_M, 20.0);
        assert_eq!(ITEM_INTERACTION_RADIUS_M, targeting::ITEM_FOCUS_RADIUS_M);
        assert_eq!(PLAYER_SPEECH_MAX_SECONDS, 15);
        assert_eq!(PLAYER_SPEECH_MAX_CHARS, 500);
        assert_eq!(POSITION_UPDATE_HZ, 10.0);
    }

    #[test]
    fn ready_hud_names_each_independently_missing_capability() {
        assert_eq!(
            connection_detail_for_capabilities(true, true, true),
            "Local Python authority connected"
        );
        assert_eq!(
            connection_detail_for_capabilities(true, false, false),
            "Connected; unavailable: microphone transcription, NPC voice audio"
        );
        assert!(connection_detail_for_capabilities(false, true, true).contains("NPC cognition"));
    }

    #[test]
    fn resync_is_a_hard_interaction_barrier() {
        let mut runtime = SmartActorRuntime::starting(true);
        runtime.connected = true;
        runtime.ready = true;
        assert!(runtime.interactions_enabled());

        runtime.resyncing = true;
        assert!(!runtime.interactions_enabled());

        runtime.resyncing = false;
        assert!(runtime.interactions_enabled());
    }

    #[test]
    fn capabilities_survive_a_malformed_initial_ready_snapshot() {
        let payload = serde_json::json!({
            "capabilities": {"llm": true, "stt": true, "tts": false},
            "snapshot": {"world_revision": "not-a-number"}
        });
        assert!(serde_json::from_value::<ReadyWire>(payload.clone()).is_err());

        let capabilities = recoverable_ready_capabilities(&payload)
            .expect("independently valid capabilities should be recoverable");
        let mut runtime = SmartActorRuntime::starting(false);
        runtime.connected = true;
        runtime.resyncing = true;
        let mut recovery = HandshakeRecovery {
            capabilities: Some(capabilities),
        };
        let mut hud = hud::SmartActorHudState::default();

        // This is the state transition used after the replacement snapshot is
        // accepted; the sidecar does not repeat capabilities in that message.
        apply_ready_capabilities(&mut runtime, &mut hud, capabilities);
        recovery.capabilities = None;

        assert!(runtime.ready);
        assert!(runtime.stt_available);
        assert!(!runtime.tts_available);
        assert!(!runtime.resyncing);
        assert!(recovery.capabilities.is_none());
        assert_eq!(hud.connection, hud::ConnectionUiState::Online);
    }

    #[test]
    fn replacement_snapshot_without_valid_ready_capabilities_fails_visibly() {
        let mut runtime = SmartActorRuntime::starting(false);
        runtime.connected = true;
        runtime.resyncing = false;
        let mut hud = hud::SmartActorHudState::default();

        mark_handshake_unrecoverable(&mut runtime, &mut hud);

        assert!(runtime.connected);
        assert!(!runtime.ready);
        assert_eq!(hud.connection, hud::ConnectionUiState::Offline);
        assert!(hud.connection_detail.contains("capabilities"));
        assert!(hud.microphone_unavailable);
    }

    #[test]
    fn redirected_offer_keeps_player_withdrawal_feedback_visible() {
        let player = model::ActorId("player".into());
        let giver = model::ActorId("giver".into());
        let other = model::ActorId("other".into());
        let coin = model::ItemId("coin".into());
        let mut mirror = model::WorldMirror::default();
        mirror.begin_session("test-session").unwrap();
        mirror
            .replace_snapshot(
                "test-session",
                model::WorldSnapshot {
                    world_revision: 1,
                    player_id: player.clone(),
                    actors: vec![
                        model::ActorSnapshot {
                            id: player.clone(),
                            name_for_player: "You".into(),
                            control: model::ActorControl::Player,
                            position_m: model::Position::new(0.0, 0.0, 0.0).unwrap(),
                            appearance_key: "player".into(),
                            holds: vec![],
                        },
                        model::ActorSnapshot {
                            id: giver.clone(),
                            name_for_player: "Ilse".into(),
                            control: model::ActorControl::Llm,
                            position_m: model::Position::new(1.0, 0.0, 0.0).unwrap(),
                            appearance_key: "ilse".into(),
                            holds: vec![coin.clone()],
                        },
                        model::ActorSnapshot {
                            id: other.clone(),
                            name_for_player: "Frans".into(),
                            control: model::ActorControl::Llm,
                            position_m: model::Position::new(2.0, 0.0, 0.0).unwrap(),
                            appearance_key: "frans".into(),
                            holds: vec![],
                        },
                    ],
                    items: vec![model::ItemSnapshot {
                        id: coin.clone(),
                        name: "copper coin".into(),
                        visual_key: "coin".into(),
                    }],
                    offers: vec![],
                },
            )
            .unwrap();

        let retract = WorldEventWire {
            event_id: "withdrawal".into(),
            kind: "retract_offer".into(),
            actor_id: giver.clone(),
            target_id: Some(player.clone()),
            item_id: Some(coin.clone()),
            recipient_ids: vec![player.clone()],
        };
        assert_eq!(
            describe_world_event(&retract, &mirror).as_deref(),
            Some("Ilse withdraws the copper coin offer")
        );

        let redirected = WorldEventWire {
            event_id: "replacement".into(),
            kind: "offer_item".into(),
            actor_id: giver,
            target_id: Some(other.clone()),
            item_id: Some(coin),
            // Python broadcasts observational world events to nearby actors,
            // including the player, even when the offer targets someone else.
            recipient_ids: vec![player, other],
        };
        assert_eq!(describe_world_event(&redirected, &mirror), None);
    }

    #[test]
    fn complete_plugin_reaches_ready_and_spawns_the_cast_headlessly() {
        if std::process::Command::new("uv")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<AudioSource>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseScroll>()
            .init_resource::<crate::controller::CollisionWorld>();
        app.world_mut().spawn((
            crate::controller::PlayerController::default(),
            Transform::from_xyz(0.0, 0.91, 111.0),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.91, 111.0)),
        ));
        let camera_transform = Transform::from_xyz(0.0, 1.56, 111.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        app.world_mut().spawn((
            crate::controller::PlayerCamera,
            camera_transform,
            GlobalTransform::from(camera_transform),
        ));
        app.world_mut().spawn((
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                ..default()
            },
        ));
        app.add_plugins(SmartActorsPlugin::new(SmartActorsConfig {
            enabled: true,
            fake_backend: true,
            uv_binary: "uv".into(),
            server_script: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("prompt_playgound/server.py")
                .display()
                .to_string(),
        }));

        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < deadline
            && !app.world().resource::<SmartActorRuntime>().ready
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(app.world().resource::<SmartActorRuntime>().ready);
        app.update();

        let world = app.world_mut();
        let actor_count = world
            .query_filtered::<Entity, With<actors::ActorView>>()
            .iter(world)
            .count();
        assert_eq!(actor_count, 3);
        let runtime_dir = world
            .resource::<bridge::BridgeHandle>()
            .runtime_dir()
            .to_path_buf();

        app.world_mut().write_message(InjectPlayerTranscript {
            text: "What's your name?".into(),
            target_id: None,
        });
        let reply_deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut reply_bubble_seen = false;
        while std::time::Instant::now() < reply_deadline && !reply_bubble_seen {
            app.update();
            let world = app.world_mut();
            reply_bubble_seen = world
                .query_filtered::<Entity, With<speech::SpeechBubble>>()
                .iter(world)
                .next()
                .is_some();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(reply_bubble_seen, "Ilse's fake reply was not presented");
        assert!(
            !app.world()
                .resource::<hud::SmartActorHudState>()
                .subtitle
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<hud::SmartActorHudState>()
                .player_transcript_text(),
            Some("You: What's your name?  ·  heard by 3 nearby people")
        );

        app.world_mut().write_message(InjectPlayerTranscript {
            text: "Please offer me your coin".into(),
            target_id: None,
        });
        let offer_deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < offer_deadline
            && app
                .world()
                .resource::<model::WorldMirror>()
                .offer(&model::ItemId("c0prs".into()))
                .is_none()
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        app.update();
        let mirror = app.world().resource::<model::WorldMirror>();
        let coin_offer = mirror
            .offer(&model::ItemId("c0prs".into()))
            .expect("Ilse did not offer the coin");
        assert_eq!(coin_offer.giver_id.0, "k0fb1");
        assert_eq!(
            coin_offer.target_id.as_ref().map(|id| id.0.as_str()),
            Some("player")
        );
        assert!(
            mirror
                .actor(&model::ActorId("k0fb1".into()))
                .is_some_and(|ilse| ilse.holds.contains(&model::ItemId("c0prs".into())))
        );
        let world = app.world_mut();
        let offered_coin_visuals = world
            .query::<&actors::OfferedItemVisual>()
            .iter(world)
            .filter(|visual| visual.item_id.0 == "c0prs")
            .count();
        assert_eq!(offered_coin_visuals, 1);
        assert!(
            app.world()
                .resource::<hud::SmartActorHudState>()
                .offer_card
                .contains("copper coin")
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyY);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        let accept_deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < accept_deadline
            && !app
                .world()
                .resource::<model::WorldMirror>()
                .actor(&model::ActorId("player".into()))
                .is_some_and(|player| player.holds.contains(&model::ItemId("c0prs".into())))
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let mirror = app.world().resource::<model::WorldMirror>();
        assert!(
            mirror
                .actor(&model::ActorId("player".into()))
                .is_some_and(|player| player.holds.contains(&model::ItemId("c0prs".into())))
        );
        assert!(mirror.offer(&model::ItemId("c0prs".into())).is_none());

        app.update();
        assert_eq!(
            app.world()
                .resource::<ActorFocus>()
                .item
                .as_ref()
                .map(|focus| focus.actor_id.0.as_str()),
            Some("cb947")
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        let reoffer_deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < reoffer_deadline
            && !app
                .world()
                .resource::<model::WorldMirror>()
                .offer(&model::ItemId("c0prs".into()))
                .is_some_and(|offer| {
                    offer.giver_id.0 == "player"
                        && offer.target_id.as_ref().is_some_and(|id| id.0 == "cb947")
                })
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let mirror = app.world().resource::<model::WorldMirror>();
        assert!(
            mirror
                .offer(&model::ItemId("c0prs".into()))
                .is_some_and(|offer| {
                    offer.giver_id.0 == "player"
                        && offer.target_id.as_ref().is_some_and(|id| id.0 == "cb947")
                })
        );
        assert!(
            mirror
                .actor(&model::ActorId("player".into()))
                .is_some_and(|player| player.holds.contains(&model::ItemId("c0prs".into())))
        );

        drop(app);
        let cleanup_deadline = std::time::Instant::now() + Duration::from_secs(2);
        while runtime_dir.exists() && std::time::Instant::now() < cleanup_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!runtime_dir.exists());
    }
}
