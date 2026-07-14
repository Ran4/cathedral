//! Engine-authoritative smart actors and their non-blocking Bevy projection.
//!
//! The authority is [`local_engine`], an in-process `cathedral_sim::Engine`.
//! The game writes [`bridge::BridgeCommand`]s into its queue and reads typed
//! `cathedral_sim::EngineMessage`s back out of its inbox; [`model::WorldMirror`]
//! projects the snapshots, and everything else here turns the engine's messages
//! into HUD toasts, speech bubbles and sound effects.

pub mod actors;
pub mod bridge;
pub mod local_engine;
pub mod model;

mod area_debug;
mod config_menu;
mod hud;
mod interaction;
mod microphone;
mod sound;
mod speech;
mod targeting;

use bevy::audio::AddAudioSource;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use cathedral_sim::{Capabilities, EngineMessage, StatusEvent, TtsBackendKind};
use serde::{Deserialize, Serialize};

pub use config_menu::ConfigMenuState;
pub use targeting::ActorFocus;

/// The one actor the game itself controls.
const PLAYER_ID: &str = "player";

pub const HEARING_RADIUS_M: f32 = 20.0;
pub const ITEM_INTERACTION_RADIUS_M: f32 = 4.0;
pub const PLAYER_SPEECH_MAX_SECONDS: u32 = 15;
pub const PLAYER_SPEECH_MAX_CHARS: usize = 500;
pub const POSITION_UPDATE_HZ: f32 = 10.0;

/// Non-secret client-side actor settings loaded from `config.ron`.
#[derive(Resource, Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SmartActorsConfig {
    pub enabled: bool,
    pub fake_backend: bool,
    /// Still needed: the local speech models (Canary-Qwen, Pocket TTS) run as
    /// `uv` worker subprocesses. Nothing else is spawned any more.
    pub uv_binary: String,
    pub tts_backend: String,
    /// Player transcription at startup: "cloud" (OpenAI) or "local"
    /// (Canary-Qwen FP16).
    pub stt_backend: String,
    pub pause_microphone_during_npc_voice: bool,
    /// Stream cloud transcription audio while the player is still speaking.
    pub stt_streaming: bool,
    /// Silence that ends an utterance. Clamped to a window where speech still
    /// ends promptly but deliberate mid-sentence pauses rarely split it.
    pub stt_trailing_silence_ms: u32,
    /// Non-speech sound percepts (features/sounds.md).
    pub sounds: SoundsConfig,
}

/// Settings for non-speech sound percepts. Perception runs in the engine;
/// these values configure it at construction.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SoundsConfig {
    pub enabled: bool,
    /// Total horizontal FOV for the "saw who did it" test. 135° is a guess —
    /// the one number in this feature only play-testing can settle.
    pub view_cone_degrees: f32,
    /// Engine-side rate limit: sounds inside the cooldown are dropped
    /// silently, so holding F cannot flood NPC inboxes (and the LLM bill).
    pub min_seconds_between_player_sounds: f32,
}

impl Default for SoundsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            view_cone_degrees: 135.0,
            min_seconds_between_player_sounds: 2.0,
        }
    }
}

impl Default for SmartActorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fake_backend: false,
            uv_binary: "uv".into(),
            tts_backend: "local".into(),
            stt_backend: "cloud".into(),
            pause_microphone_during_npc_voice: true,
            stt_streaming: true,
            stt_trailing_silence_ms: 400,
            sounds: SoundsConfig::default(),
        }
    }
}

impl SmartActorsConfig {
    fn initial_stt_backend(&self) -> bridge::TranscriptionBackend {
        if self.stt_backend.eq_ignore_ascii_case("local") {
            bridge::TranscriptionBackend::Local
        } else {
            bridge::TranscriptionBackend::Cloud
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
}

pub struct SmartActorsPlugin {
    config: SmartActorsConfig,
}

/// Connection/capability state shared by input and presentation systems.
#[derive(Resource, Debug, Clone)]
pub struct SmartActorRuntime {
    pub connected: bool,
    pub ready: bool,
    pub stt_available: bool,
    pub stt_cloud_available: bool,
    pub stt_local_available: bool,
    pub tts_available: bool,
    pub tts_cloud_available: bool,
    pub tts_local_available: bool,
    pub tts_selected: bridge::TtsBackend,
    tts_selection_pending: Option<(String, bridge::TtsBackend)>,
    /// A user-requested backend change was confirmed and awaits persistence
    /// to `config.ron`.
    tts_selection_dirty: bool,
    next_tts_request: u64,
    pub fake_backend: bool,
    pub mirror_revision: Option<u64>,
    thinking_actor_id: Option<model::ActorId>,
}

impl SmartActorRuntime {
    fn starting(fake_backend: bool) -> Self {
        Self {
            connected: false,
            ready: false,
            stt_available: false,
            stt_cloud_available: false,
            stt_local_available: false,
            tts_available: false,
            tts_cloud_available: false,
            tts_local_available: false,
            tts_selected: bridge::TtsBackend::Off,
            tts_selection_pending: None,
            tts_selection_dirty: false,
            next_tts_request: 0,
            fake_backend,
            mirror_revision: None,
            thinking_actor_id: None,
        }
    }

    pub fn interactions_enabled(&self) -> bool {
        self.connected && self.ready
    }

    fn thinking_actor(&self) -> Option<&model::ActorId> {
        self.thinking_actor_id.as_ref()
    }

    /// The scheduler has one global request slot. Actor-specific terminal rows
    /// only clear the actor they name, so a stale row cannot hide a newer turn.
    fn observe_llm_status(&mut self, state: &str, actor_id: Option<&model::ActorId>) {
        if state == "thinking" {
            self.thinking_actor_id = actor_id.cloned();
        } else if actor_id.is_some_and(|actor_id| self.thinking_actor() == Some(actor_id))
            || (actor_id.is_none() && state == "unavailable")
        {
            self.thinking_actor_id = None;
        }
    }
}

impl SmartActorsPlugin {
    pub fn new(config: SmartActorsConfig) -> Self {
        Self { config }
    }
}

impl Plugin for SmartActorsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        if app.is_plugin_added::<bevy::audio::AudioPlugin>() {
            app.add_audio_source::<speech::StreamingPcmSource>();
        } else {
            // Headless tests have no audio output, but still need the asset
            // storage used by the presentation systems.
            app.init_asset::<speech::StreamingPcmSource>();
        }
        app.init_resource::<hud::SmartActorHudState>()
            .add_systems(Startup, hud::spawn_smart_actor_hud);

        // The Esc settings menu exists even when smart actors are disabled;
        // its rows then report the disabled state instead of toggling.
        app.init_resource::<config_menu::ConfigMenuState>()
            .add_systems(Startup, config_menu::spawn_config_menu)
            .add_systems(
                Update,
                (
                    config_menu::toggle_config_menu,
                    config_menu::handle_config_menu_buttons,
                    config_menu::persist_backend_selections,
                    config_menu::update_config_menu,
                )
                    .chain(),
            );

        if !self.config.enabled {
            let mut hud = hud::SmartActorHudState::default();
            hud.connection = hud::ConnectionUiState::Disabled;
            hud.connection_detail = "Disabled in config.ron".into();
            hud.set_transcription_capabilities(false, false);
            hud.set_npc_voice_backend(bridge::TtsBackend::Off);
            app.insert_resource(hud)
                .add_systems(Update, hud::update_smart_actor_hud);
            return;
        }

        let (handle, inbox, worker, engine) = local_engine::spawn(&self.config);
        app.insert_non_send(engine);

        app.insert_resource(handle)
            .insert_resource(inbox)
            .insert_resource(worker)
            .init_resource::<model::WorldMirror>()
            .insert_resource(SmartActorRuntime::starting(self.config.fake_backend))
            .init_resource::<area_debug::AreaDebugState>()
            .init_resource::<ActorFocus>()
            .init_resource::<interaction::InteractionState>()
            .init_resource::<interaction::PlayerSpatialState>()
            .insert_resource(interaction::MicrophoneInputState::with_backend(
                self.config.initial_stt_backend(),
            ))
            .init_resource::<speech::SpeechPresentationState>()
            .add_message::<interaction::PlayerIntent>()
            .add_message::<InjectPlayerTranscript>()
            .add_message::<speech::PresentSpeech>()
            .add_message::<speech::TtsClipReady>()
            .add_message::<speech::TtsClipFailed>()
            .add_message::<speech::TtsPcmChunkReady>()
            .add_message::<speech::TtsStreamFinished>()
            .add_message::<speech::StopNpcSpeech>()
            .add_message::<speech::ClearSpeechPresentation>()
            .add_message::<sound::PlaySoundEffect>()
            .configure_sets(
                PostUpdate,
                (
                    SmartActorSet::DrainBridge,
                    SmartActorSet::ReconcileMirror,
                    SmartActorSet::UpdateFocus,
                    SmartActorSet::CollectInput,
                    SmartActorSet::Present,
                )
                    .chain()
                    .after(TransformSystems::Propagate),
            )
            .add_systems(
                Startup,
                (
                    actors::setup_actor_visual_assets,
                    area_debug::spawn_area_debug_ui,
                )
                    .after(hud::spawn_smart_actor_hud),
            )
            .add_systems(
                PostUpdate,
                // The engine polls first: a command written in this frame's
                // CollectInput is answered no later than the next frame's drain,
                // the same latency the sidecar had.
                (local_engine::pump_local_engine, drain_bridge_messages)
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
                    interaction::collect_sound_input,
                    interaction::update_microphone_toggle,
                    update_tts_backend_toggle,
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
                    actors::update_thinking_indicators,
                    actors::animate_offered_items,
                    speech::receive_speech_events,
                    speech::receive_tts_clips,
                    speech::receive_tts_pcm_chunks,
                    speech::receive_tts_stream_ends,
                    speech::receive_tts_failures,
                    speech::clear_speech_presentation,
                    speech::stop_npc_speech_for_capture,
                    speech::update_speech_bubbles,
                    speech::update_subtitle_hud,
                    sound::play_sound_effects,
                    sound::expire_stalled_sound_effects,
                    area_debug::update_area_debug_ui,
                    hud::update_smart_actor_hud,
                )
                    .chain()
                    .in_set(SmartActorSet::Present),
            )
            .add_systems(
                Update,
                // Creating the player before PostUpdate guarantees Bevy's
                // audio playback systems see it in that same frame. When this
                // lived alongside those systems, a completed source handoff
                // could occasionally miss sink attachment.
                speech::start_ready_audio,
            );
        if app.is_plugin_added::<bevy::gizmos::GizmoPlugin>() {
            app.add_systems(
                PostUpdate,
                area_debug::draw_area_boxes
                    .in_set(SmartActorSet::Present)
                    .after(area_debug::update_area_debug_ui),
            );
        }
    }
}

/// Developer/test-only transcript injection. The engine accepts it exclusively
/// in deterministic fake mode and still applies the normal `say` validator;
/// production has no typed-chat path.
#[derive(Message, Debug, Clone)]
pub struct InjectPlayerTranscript {
    pub text: String,
    pub target_id: Option<model::ActorId>,
}

#[derive(SystemParam)]
struct BridgePresentationWriters<'w> {
    speech: MessageWriter<'w, speech::PresentSpeech>,
    wav: MessageWriter<'w, speech::TtsClipReady>,
    pcm: MessageWriter<'w, speech::TtsPcmChunkReady>,
    failure: MessageWriter<'w, speech::TtsClipFailed>,
    stream_end: MessageWriter<'w, speech::TtsStreamFinished>,
    clear: MessageWriter<'w, speech::ClearSpeechPresentation>,
    sound_effects: MessageWriter<'w, sound::PlaySoundEffect>,
}

#[allow(clippy::too_many_arguments)]
fn drain_bridge_messages(
    mut commands: Commands,
    config: Res<SmartActorsConfig>,
    inbox: Res<bridge::BridgeInbox>,
    handle: Res<bridge::BridgeHandle>,
    players: Query<&GlobalTransform, With<crate::controller::PlayerController>>,
    microphone: Option<Res<microphone::MicrophoneService>>,
    mut mirror: ResMut<model::WorldMirror>,
    mut runtime: ResMut<SmartActorRuntime>,
    mut hud: ResMut<hud::SmartActorHudState>,
    mut interaction: ResMut<interaction::InteractionState>,
    mut microphone_input: ResMut<interaction::MicrophoneInputState>,
    mut spatial: ResMut<interaction::PlayerSpatialState>,
    mut presentation: BridgePresentationWriters,
    // Speech presentation dedupes and orders by this (speech.rs), and the
    // engine's messages no longer carry a sequence of their own. Counting them
    // here gives the same monotonic, gap-free stream the envelope did.
    mut message_seq: Local<u64>,
) {
    // Resource insertions/removals are deferred. Track what this drain pass
    // has queued so several buffered engine messages cannot spawn several
    // microphone workers before commands are applied.
    let mut microphone_present = microphone.is_some();
    while let Some(event) = inbox.try_recv() {
        match event {
            bridge::BridgeEvent::ProcessStarted => {
                runtime.connected = true;
                runtime.ready = false;
                runtime.thinking_actor_id = None;
                hud.connection = hud::ConnectionUiState::Starting;
                hud.connection_detail = "Actor engine starting; handshaking…".into();
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
            bridge::BridgeEvent::Message(message) => {
                // Once the engine is known dead, never let a buffered late
                // message revive interactions against stale state; this slice
                // has no in-place restart.
                if !runtime.connected {
                    continue;
                }
                *message_seq += 1;
                process_engine_message(
                    *message,
                    *message_seq,
                    &mut mirror,
                    &mut runtime,
                    &mut hud,
                    &mut interaction,
                    &mut presentation,
                );
                // Do not open the default input device before the engine
                // handshake confirms that transcription is configured.
                if runtime.ready && runtime.stt_available && !microphone_present {
                    commands.insert_resource(microphone::MicrophoneService::spawn(
                        handle.runtime_dir().to_path_buf(),
                        handle.command_sender(),
                        microphone::clamped_trailing_silence(config.stt_trailing_silence_ms),
                    ));
                    microphone_present = true;
                }
            }
            bridge::BridgeEvent::Disconnected(message) => {
                runtime.connected = false;
                runtime.ready = false;
                runtime.stt_available = false;
                runtime.stt_cloud_available = false;
                runtime.stt_local_available = false;
                runtime.tts_available = false;
                runtime.tts_cloud_available = false;
                runtime.tts_local_available = false;
                runtime.tts_selected = bridge::TtsBackend::Off;
                runtime.tts_selection_pending = None;
                runtime.thinking_actor_id = None;
                interaction.clear_pending();
                microphone_input.clear_on_disconnect();
                hud.clear_transients_on_disconnect(truncate_owned(message, 300));
                presentation.clear.write(speech::ClearSpeechPresentation);
                if microphone_present {
                    commands.remove_resource::<microphone::MicrophoneService>();
                    microphone_present = false;
                }
            }
        }
    }
}

/// One authoritative message, typed.
///
/// Every arm used to begin by deserializing a `serde_json::Value` and toasting
/// on failure. The message is now the engine's own value, so what remains is
/// the *sanitation* the projection still owes the UI: an NPC's line and a
/// backend's error string are the two texts nobody in this process wrote.
#[allow(clippy::too_many_arguments)]
fn process_engine_message(
    message: EngineMessage,
    message_seq: u64,
    mirror: &mut model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    interaction: &mut interaction::InteractionState,
    presentation: &mut BridgePresentationWriters,
) {
    match message {
        EngineMessage::Ready {
            capabilities,
            snapshot,
        } => {
            runtime.thinking_actor_id = None;
            // A rejected first snapshot means the seeded world itself is
            // unrenderable — a sim bug, not a lost message, and there is no
            // resync left to ask for. The handshake simply never completes and
            // the HUD keeps saying so.
            if accept_snapshot(mirror, runtime, hud, &snapshot) {
                apply_ready_capabilities(runtime, hud, capabilities);
            }
        }
        EngineMessage::Snapshot(snapshot) => {
            accept_snapshot(mirror, runtime, hud, &snapshot);
        }
        EngineMessage::Speech {
            event_id,
            speaker_id,
            target_id: _,
            text,
            speaker_position_m,
            recipient_ids,
            speaker_name_for_player,
        } => {
            let speaker_id = model::actor_id_from_sim(&speaker_id);
            let recipients: Vec<model::ActorId> =
                recipient_ids.iter().map(model::actor_id_from_sim).collect();
            if !valid_ui_text(&speaker_name_for_player, 256)
                || !valid_ui_text(&text, PLAYER_SPEECH_MAX_CHARS)
                || mirror.actor(&speaker_id).is_none()
            {
                hud.toast("Discarded invalid speech data from the actor engine");
                return;
            }
            let player_heard =
                speaker_id.0 == PLAYER_ID || recipients.iter().any(|id| id.0 == PLAYER_ID);
            if !player_heard {
                return;
            }
            let recipient_count = recipients
                .iter()
                .filter(|recipient| **recipient != speaker_id && mirror.actor(recipient).is_some())
                .count();
            presentation.speech.write(speech::PresentSpeech {
                event_seq: message_seq,
                event_id: event_id.0,
                speaker_id: speaker_id.clone(),
                speaker_label: speaker_name_for_player,
                text,
                speaker_position: model::vec3_from_sim(speaker_position_m),
                recipient_count,
                expect_audio: tts_selection_is_usable(runtime) && speaker_id.0 != PLAYER_ID,
            });
        }
        EngineMessage::Sound {
            event_id: _,
            sound_id,
            sound_class: _,
            actor_id,
            position_m,
            audible_distance,
            recipient_ids,
            witness_ids: _,
            text_for_player,
        } => {
            let audible_distance = audible_distance as f32;
            if !sound::valid_sound_id(&sound_id)
                || !audible_distance.is_finite()
                || audible_distance <= 0.0
                || audible_distance > 10_000.0
                || text_for_player
                    .as_deref()
                    .is_some_and(|text| !valid_ui_text(text, 300))
            {
                hud.toast("Discarded invalid sound data from the actor engine");
                return;
            }
            // The engine rendered the player's percept (or None when the player
            // is out of range); Bevy never decides what is known.
            if let Some(text) = text_for_player {
                hud.toast(text);
            }
            let player_made_it = actor_id
                .as_ref()
                .is_some_and(|actor_id| actor_id.as_str() == PLAYER_ID);
            let player_heard = player_made_it
                || recipient_ids
                    .iter()
                    .any(|recipient| recipient.as_str() == PLAYER_ID);
            if player_heard {
                presentation.sound_effects.write(sound::PlaySoundEffect {
                    sound_id,
                    position: model::vec3_from_sim(position_m),
                    audible_distance,
                });
            }
        }
        EngineMessage::WorldEvent {
            event_id: _,
            kind,
            actor_id,
            target_id,
            item_id,
            recipient_ids,
        } => {
            // Presentation feedback only. Offers and ownership still reconcile
            // exclusively from authoritative snapshots.
            let text = describe_world_event(
                &kind,
                &model::actor_id_from_sim(&actor_id),
                target_id.as_ref().map(model::actor_id_from_sim).as_ref(),
                item_id.as_ref().map(model::item_id_from_sim).as_ref(),
                &recipient_ids
                    .iter()
                    .map(model::actor_id_from_sim)
                    .collect::<Vec<_>>(),
                mirror,
            );
            if let Some(text) = text {
                hud.toast(text);
            }
        }
        EngineMessage::TranscriptionResult {
            request_id: _,
            text,
            error,
        } => {
            if let Some(text) = text.filter(|text| valid_ui_text(text, 500)) {
                // This is the earliest exact confirmation of what STT
                // understood. It has a dedicated bottom caption so later
                // status/world-event toasts cannot overwrite it.
                hud.show_player_transcript(&text);
            } else if let Some(error) = error {
                hud.toast(truncate_owned(error, 300));
            }
        }
        EngineMessage::CommandResult {
            request_id,
            success,
            error_code,
            message,
        } => {
            if runtime
                .tts_selection_pending
                .as_ref()
                .is_some_and(|(pending, _)| pending == &request_id)
            {
                let (_, requested) = runtime
                    .tts_selection_pending
                    .take()
                    .expect("pending selection was checked");
                if success {
                    runtime.tts_selected = requested;
                    runtime.tts_selection_dirty = true;
                    hud.set_npc_voice_backend(requested);
                    hud.toast(format!("NPC voices: {}", requested.name().to_uppercase()));
                } else {
                    let code = truncate_owned(error_code.unwrap_or_else(|| "error".into()), 64);
                    hud.toast(format!("{code}: {}", truncate_owned(message, 260)));
                }
                return;
            }
            let known = interaction.resolve_command(&request_id, success, runtime.mirror_revision);
            if known && !success {
                let code = truncate_owned(error_code.unwrap_or_else(|| "error".into()), 64);
                hud.toast(format!("{code}: {}", truncate_owned(message, 260)));
            }
        }
        EngineMessage::Status(status) => apply_status(status, mirror, runtime, hud),
        EngineMessage::TtsReady {
            event_id,
            wav_bytes,
        } => {
            presentation.wav.write(speech::TtsClipReady {
                event_id: event_id.0,
                wav_bytes,
            });
        }
        EngineMessage::TtsChunk {
            event_id,
            chunk_seq,
            sample_rate,
            samples,
            backend,
        } => {
            presentation.pcm.write(speech::TtsPcmChunkReady {
                event_id: event_id.0,
                chunk_seq,
                sample_rate,
                samples,
                backend,
            });
        }
        EngineMessage::TtsStreamEnd {
            event_id,
            chunk_count,
            first_chunk_ms,
        } => {
            if chunk_count > 0 && first_chunk_ms <= 600_000 {
                presentation.stream_end.write(speech::TtsStreamFinished {
                    event_id: event_id.0,
                    chunk_count,
                    first_chunk_ms,
                });
            } else {
                hud.toast("Discarded invalid NPC stream completion data");
            }
        }
        EngineMessage::TtsFailed { event_id, reason } => {
            if valid_ui_text(&reason, 160) {
                presentation.failure.write(speech::TtsClipFailed {
                    event_id: event_id.0,
                    reason,
                });
            } else {
                hud.toast("Discarded invalid NPC voice failure data");
            }
        }
        // Both are the host's business and never reach the ECS: `local_engine`
        // writes the prompt archive and the session log itself.
        EngineMessage::PromptExchange { .. } | EngineMessage::Diagnostic(_) => {}
    }
}

fn tts_selection_is_usable(runtime: &SmartActorRuntime) -> bool {
    match runtime.tts_selected {
        bridge::TtsBackend::Cloud => runtime.tts_cloud_available,
        bridge::TtsBackend::Local => runtime.tts_local_available,
        bridge::TtsBackend::Off => false,
    }
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
        "Local actor engine connected".into()
    } else {
        format!("Connected; unavailable: {}", unavailable.join(", "))
    }
}

/// The engine's capability set is consistent by construction
/// (`Capabilities::new` derives the `stt`/`tts` ORs from the four probes), so
/// there is nothing left to validate here — only to apply.
fn apply_ready_capabilities(
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    capabilities: Capabilities,
) {
    runtime.ready = true;
    runtime.connected = true;
    runtime.stt_available = capabilities.stt;
    runtime.stt_cloud_available = capabilities.stt_cloud;
    runtime.stt_local_available = capabilities.stt_local;
    runtime.tts_available = capabilities.tts;
    runtime.tts_cloud_available = capabilities.tts_cloud;
    runtime.tts_local_available = capabilities.tts_local;
    runtime.tts_selected = tts_backend_of(capabilities.tts_selected);
    runtime.tts_selection_pending = None;
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
    hud.set_transcription_capabilities(capabilities.stt_cloud, capabilities.stt_local);
    hud.set_npc_voice_backend(runtime.tts_selected);
    hud.connection_detail =
        connection_detail_for_capabilities(capabilities.llm, capabilities.stt, capabilities.tts);
}

fn tts_backend_of(kind: TtsBackendKind) -> bridge::TtsBackend {
    match kind {
        TtsBackendKind::Cloud => bridge::TtsBackend::Cloud,
        TtsBackendKind::Local => bridge::TtsBackend::Local,
        TtsBackendKind::Off => bridge::TtsBackend::Off,
    }
}

/// Project one authoritative snapshot. It is replaced whole or not at all; a
/// snapshot the projection rejects is a sim bug, and the next revision — the
/// engine publishes one per change — is the only recovery there is.
fn accept_snapshot(
    mirror: &mut model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
    snapshot: &cathedral_sim::PublicSnapshot,
) -> bool {
    match mirror.replace_snapshot(snapshot.into()) {
        Ok(revision) => {
            runtime.mirror_revision = Some(revision);
            true
        }
        Err(error) => {
            hud.toast(format!("malformed snapshot: {error}"));
            false
        }
    }
}

fn next_tts_backend(runtime: &SmartActorRuntime) -> bridge::TtsBackend {
    use bridge::TtsBackend::{Cloud, Local, Off};
    let modes = [Cloud, Local, Off];
    let current = modes
        .iter()
        .position(|mode| *mode == runtime.tts_selected)
        .unwrap_or(2);
    for offset in 1..=modes.len() {
        let candidate = modes[(current + offset) % modes.len()];
        let available = match candidate {
            Cloud => runtime.tts_cloud_available,
            Local => runtime.tts_local_available,
            Off => true,
        };
        if available {
            return candidate;
        }
    }
    Off
}

fn update_tts_backend_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    handle: Res<bridge::BridgeHandle>,
    mut runtime: ResMut<SmartActorRuntime>,
    mut hud: ResMut<hud::SmartActorHudState>,
) {
    if !keyboard.just_pressed(KeyCode::KeyX) {
        return;
    }
    let backend = next_tts_backend(&runtime);
    request_tts_backend(&mut runtime, &handle, &mut hud, backend);
}

/// Validated NPC-voice switch shared by the X key and the settings menu. The
/// selection only commits (and persists) once the engine confirms it.
fn request_tts_backend(
    runtime: &mut SmartActorRuntime,
    handle: &bridge::BridgeHandle,
    hud: &mut hud::SmartActorHudState,
    backend: bridge::TtsBackend,
) {
    if !runtime.interactions_enabled() {
        hud.toast("NPC voice selection is unavailable while actors are offline");
        return;
    }
    if runtime.tts_selection_pending.is_some() {
        hud.toast("NPC voice selection is still changing");
        return;
    }
    let available = match backend {
        bridge::TtsBackend::Cloud => runtime.tts_cloud_available,
        bridge::TtsBackend::Local => runtime.tts_local_available,
        bridge::TtsBackend::Off => true,
    };
    if !available {
        hud.toast(format!(
            "{} NPC voices are not available",
            backend.name().to_uppercase()
        ));
        return;
    }
    if runtime.tts_selected == backend {
        return;
    }
    runtime.next_tts_request = runtime.next_tts_request.wrapping_add(1).max(1);
    let request_id = format!("tts-mode-{}", runtime.next_tts_request);
    match handle.try_send(bridge::BridgeCommand::SetTtsBackend {
        request_id: request_id.clone(),
        backend,
    }) {
        Ok(()) => {
            runtime.tts_selection_pending = Some((request_id, backend));
            hud.toast(format!(
                "Switching NPC voices to {}…",
                backend.name().to_uppercase()
            ));
        }
        Err(error) => hud.toast(error),
    }
}

/// The engine's status rows drive the connection line and the STT pills.
///
/// `state` stays a free-form string: the speech backends add rows of their own
/// (`synthesizing`, `loading`, `selected`, …) and the HUD matches on them.
fn apply_status(
    status: StatusEvent,
    mirror: &model::WorldMirror,
    runtime: &mut SmartActorRuntime,
    hud: &mut hud::SmartActorHudState,
) {
    let subsystem = status.subsystem.as_str();
    let state = truncate_owned(status.state, 64);
    let message = status.message.map(|message| truncate_owned(message, 300));
    let backend = status.backend.map(|backend| truncate_owned(backend, 16));
    let actor_id = status.actor_id.as_ref().map(model::actor_id_from_sim);
    if subsystem == "llm" {
        runtime.observe_llm_status(&state, actor_id.as_ref());
    }
    let actor = actor_id.as_ref().and_then(|id| mirror.actor(id));
    let actor_name = actor.map(|actor| actor.name_for_player.clone());
    let nearby_thinking_actor = actor
        .filter(|actor| actor_is_near_player(actor, mirror))
        .map(|actor| actor.name_for_player.clone());
    hud.connection_detail = match (subsystem, state.as_str(), actor_name.as_deref()) {
        ("llm", "thinking", _) => nearby_thinking_actor
            .map(|actor| format!("{actor} is thinking…"))
            .unwrap_or_else(|| "Background actors are thinking…".into()),
        ("stt", "transcribing", _) => "Transcribing your speech…".into(),
        ("tts", "synthesizing", Some(actor)) => format!("Preparing {actor}'s voice…"),
        (_, state, _) => message
            .clone()
            .unwrap_or_else(|| format!("{subsystem}: {state}")),
    };
    hud.connection_detail = truncate_owned(std::mem::take(&mut hud.connection_detail), 300);
    if subsystem == "stt"
        && let Some(backend) = backend.as_deref()
    {
        hud.apply_transcription_status(backend, &state, message.as_deref());
    }
    if matches!(state.as_str(), "degraded" | "unavailable")
        && let Some(message) = message
    {
        hud.toast(truncate_owned(message, 300));
    }
}

fn actor_is_near_player(actor: &model::ActorSnapshot, mirror: &model::WorldMirror) -> bool {
    let Some(player) = mirror
        .player_id()
        .and_then(|player_id| mirror.actor(player_id))
    else {
        return false;
    };
    let actor_position: Vec3 = actor.position_m.into();
    let player_position: Vec3 = player.position_m.into();
    actor_position.distance_squared(player_position) <= HEARING_RADIUS_M * HEARING_RADIUS_M
}

/// The player-facing sentence one world event deserves, or `None` when it is
/// none of his business.
fn describe_world_event(
    kind: &str,
    actor_id: &model::ActorId,
    target_id: Option<&model::ActorId>,
    item_id: Option<&model::ItemId>,
    recipient_ids: &[model::ActorId],
    mirror: &model::WorldMirror,
) -> Option<String> {
    let player_id = mirror.player_id()?;
    if actor_id != player_id && !recipient_ids.contains(player_id) {
        return None;
    }
    let player_acted = actor_id == player_id;
    let actor = if player_acted {
        "You"
    } else {
        mirror.actor(actor_id)?.name_for_player.as_str()
    };
    let item = item_id
        .and_then(|item_id| mirror.item(item_id))
        .map(|item| item.name.as_str())?;
    let offer_verb = if player_acted { "offer" } else { "offers" };
    match kind {
        "offer_item" if target_id == Some(player_id) => {
            Some(format!("{actor} {offer_verb} you the {item}"))
        }
        "offer_item" if target_id.is_none() => {
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

/// LLM-authored text is the only untrusted input left. Bound it before it
/// reaches a UI node.
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
            if let (Some(wav_basename), Some(microphone)) = (failed_recording, &microphone) {
                // Best-effort: release any streamed copy the engine holds.
                let _ = handle.try_send(bridge::BridgeCommand::PlayerAudioAbort {
                    wav_basename: wav_basename.clone(),
                });
                if let Err(error) = microphone.discard_recording(wav_basename) {
                    hud.toast(error);
                }
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
            if let (Some(wav_basename), Some(microphone)) = (failed_recording, &microphone) {
                let _ = handle.try_send(bridge::BridgeCommand::PlayerAudioAbort {
                    wav_basename: wav_basename.clone(),
                });
                if let Err(cleanup_error) = microphone.discard_recording(wav_basename) {
                    hud.toast(cleanup_error);
                }
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
            facing_yaw,
        } => bridge::BridgeCommand::SpatialUpdate {
            spatial_seq: *spatial_seq,
            position_m: position(*value)?,
            facing_yaw: if facing_yaw.is_finite() {
                *facing_yaw
            } else {
                return Err("player facing is invalid".into());
            },
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
        interaction::PlayerIntent::Sound { sound_id } => bridge::BridgeCommand::PlayerSound {
            sound_id: sound_id.clone(),
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
        interaction::PlayerIntent::SpatialUpdate { .. }
        | interaction::PlayerIntent::Sound { .. } => None,
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
    use std::{thread, time::Duration};

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
            "Local actor engine connected"
        );
        assert_eq!(
            connection_detail_for_capabilities(true, false, false),
            "Connected; unavailable: microphone transcription, NPC voice audio"
        );
        assert!(connection_detail_for_capabilities(false, true, true).contains("NPC cognition"));
    }

    #[test]
    fn llm_status_tracks_the_actor_for_the_overhead_thinking_indicator() {
        let mirror = model::WorldMirror::default();
        let mut runtime = SmartActorRuntime::starting(false);
        let mut hud = hud::SmartActorHudState::default();
        let ilse = cathedral_sim::ActorId::from_raw("ilse");
        let sven = cathedral_sim::ActorId::from_raw("sven");

        apply_status(
            StatusEvent::llm("thinking", Some(ilse.clone()), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(
            runtime.thinking_actor().map(|id| id.0.as_str()),
            Some("ilse")
        );

        // A late terminal row for a different actor cannot hide Ilse's newer
        // turn, but Ilse's own terminal row does.
        apply_status(
            StatusEvent::llm("idle", Some(sven), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(
            runtime.thinking_actor().map(|id| id.0.as_str()),
            Some("ilse")
        );
        apply_status(
            StatusEvent::llm("idle", Some(ilse.clone()), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert!(runtime.thinking_actor().is_none());

        apply_status(
            StatusEvent::llm("thinking", Some(ilse), None),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        apply_status(
            StatusEvent::llm("unavailable", None, Some("offline".into())),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert!(runtime.thinking_actor().is_none());
    }

    #[test]
    fn thinking_hud_names_only_actors_within_conversation_range() {
        let mut mirror = model::WorldMirror::default();
        mirror
            .replace_snapshot(model::WorldSnapshot {
                world_revision: 1,
                player_id: model::ActorId("player".into()),
                actors: vec![
                    model::ActorSnapshot {
                        id: model::ActorId("player".into()),
                        name_for_player: "You".into(),
                        control: model::ActorControl::Player,
                        position_m: model::Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "player".into(),
                        holds: vec![],
                    },
                    model::ActorSnapshot {
                        id: model::ActorId("near".into()),
                        name_for_player: "Near".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(HEARING_RADIUS_M, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "near".into(),
                        holds: vec![],
                    },
                    model::ActorSnapshot {
                        id: model::ActorId("far".into()),
                        name_for_player: "Far".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(HEARING_RADIUS_M + 0.01, 0.0, 0.0)
                            .unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "far".into(),
                        holds: vec![],
                    },
                ],
                items: vec![],
                offers: vec![],
            })
            .unwrap();
        let mut runtime = SmartActorRuntime::starting(false);
        let mut hud = hud::SmartActorHudState::default();

        apply_status(
            StatusEvent::llm(
                "thinking",
                Some(cathedral_sim::ActorId::from_raw("far")),
                None,
            ),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(hud.connection_detail, "Background actors are thinking…");

        apply_status(
            StatusEvent::llm(
                "thinking",
                Some(cathedral_sim::ActorId::from_raw("near")),
                None,
            ),
            &mirror,
            &mut runtime,
            &mut hud,
        );
        assert_eq!(hud.connection_detail, "Near is thinking…");
    }

    /// The cloud and local ears are independent: the streaming gate keys off
    /// the cloud flag alone, and the pill row off both.
    #[test]
    fn ready_capabilities_split_stt_availability_for_the_streaming_gate() {
        let mut runtime = SmartActorRuntime::starting(false);
        let mut hud = hud::SmartActorHudState::default();
        apply_ready_capabilities(
            &mut runtime,
            &mut hud,
            Capabilities::new(true, true, false, false, false, TtsBackendKind::Off),
        );

        assert!(runtime.ready && runtime.connected);
        assert!(runtime.stt_available && runtime.stt_cloud_available);
        assert!(!runtime.stt_local_available);
        assert!(!runtime.tts_available);
        assert_eq!(runtime.tts_selected, bridge::TtsBackend::Off);
        assert_eq!(hud.connection, hud::ConnectionUiState::Online);
        assert!(runtime.interactions_enabled());
    }

    #[test]
    fn npc_voice_cycle_skips_unavailable_backends_and_includes_off() {
        let mut runtime = SmartActorRuntime::starting(false);
        runtime.tts_selected = bridge::TtsBackend::Cloud;
        runtime.tts_cloud_available = true;
        runtime.tts_local_available = false;
        assert_eq!(next_tts_backend(&runtime), bridge::TtsBackend::Off);

        runtime.tts_selected = bridge::TtsBackend::Off;
        assert_eq!(next_tts_backend(&runtime), bridge::TtsBackend::Cloud);

        runtime.tts_local_available = true;
        runtime.tts_selected = bridge::TtsBackend::Cloud;
        assert_eq!(next_tts_backend(&runtime), bridge::TtsBackend::Local);
    }

    #[test]
    fn npc_speech_only_expects_audio_from_the_acknowledged_usable_mode() {
        let mut runtime = SmartActorRuntime::starting(false);
        runtime.tts_cloud_available = true;
        runtime.tts_local_available = false;
        runtime.tts_selected = bridge::TtsBackend::Off;
        assert!(!tts_selection_is_usable(&runtime));
        runtime.tts_selected = bridge::TtsBackend::Local;
        assert!(!tts_selection_is_usable(&runtime));
        runtime.tts_selected = bridge::TtsBackend::Cloud;
        assert!(tts_selection_is_usable(&runtime));
    }

    #[test]
    fn redirected_offer_keeps_player_withdrawal_feedback_visible() {
        let player = model::ActorId("player".into());
        let giver = model::ActorId("giver".into());
        let other = model::ActorId("other".into());
        let coin = model::ItemId("coin".into());
        let mut mirror = model::WorldMirror::default();
        mirror
            .replace_snapshot(model::WorldSnapshot {
                world_revision: 1,
                player_id: player.clone(),
                actors: vec![
                    model::ActorSnapshot {
                        id: player.clone(),
                        name_for_player: "You".into(),
                        control: model::ActorControl::Player,
                        position_m: model::Position::new(0.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "player".into(),
                        holds: vec![],
                    },
                    model::ActorSnapshot {
                        id: giver.clone(),
                        name_for_player: "Ilse".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(1.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
                        appearance_key: "ilse".into(),
                        holds: vec![coin.clone()],
                    },
                    model::ActorSnapshot {
                        id: other.clone(),
                        name_for_player: "Frans".into(),
                        control: model::ActorControl::Llm,
                        position_m: model::Position::new(2.0, 0.0, 0.0).unwrap(),
                        facing_yaw: 0.0,
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
            })
            .unwrap();

        assert_eq!(
            describe_world_event(
                "retract_offer",
                &giver,
                Some(&player),
                Some(&coin),
                std::slice::from_ref(&player),
                &mirror,
            )
            .as_deref(),
            Some("Ilse withdraws the copper coin offer")
        );

        // The engine broadcasts observational world events to nearby actors,
        // including the player, even when the offer targets someone else — and
        // that must not overwrite the withdrawal toast above.
        assert_eq!(
            describe_world_event(
                "offer_item",
                &giver,
                Some(&other),
                Some(&coin),
                &[player, other.clone()],
                &mirror,
            ),
            None
        );
    }

    /// The seam's acceptance test: the whole plugin, the in-process engine, and
    /// the fake backends — no subprocess, no network, no `uv`.
    #[test]
    fn complete_plugin_reaches_ready_and_spawns_the_cast_headlessly() {
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
            tts_backend: "local".into(),
            pause_microphone_during_npc_voice: true,
            ..SmartActorsConfig::default()
        }));
        // Fake mode reports transcription available, so the plugin spawns the
        // capture worker — but a test may not open the developer's microphone
        // and put the room's noise into the scripted conversation. The worker
        // probes no device until it is enabled, so an explicit OFF (the V state)
        // keeps it inert while everything downstream of it stays wired.
        app.world_mut()
            .resource_mut::<interaction::MicrophoneInputState>()
            .enabled = false;

        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < deadline
            && !app.world().resource::<SmartActorRuntime>().ready
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(app.world().resource::<SmartActorRuntime>().ready);
        app.update();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert!(
            app.world()
                .resource::<area_debug::AreaDebugState>()
                .is_enabled()
        );
        assert_eq!(
            app.world()
                .resource::<area_debug::AreaDebugState>()
                .visible_area_ids()
                .len(),
            8
        );
        let expected_box_labels = app
            .world()
            .non_send::<local_engine::LocalEngine>()
            .area_map()
            .expect("the area map is loaded")
            .areas
            .iter()
            .map(|area| area.boxes.len())
            .sum::<usize>();
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&area_debug::AreaBoxLabel>()
                .iter(world)
                .count(),
            expected_box_labels
        );
        let (location_text, visibility) = world
            .query_filtered::<(&Text, &Visibility), With<area_debug::PlayerAreaDescription>>()
            .single(world)
            .expect("the area debug player label exists");
        assert!(location_text.0.contains("The Gradine"));
        assert_eq!(*visibility, Visibility::Inherited);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert!(
            !app.world()
                .resource::<area_debug::AreaDebugState>()
                .is_enabled()
        );
        let world = app.world_mut();
        let (_, visibility) = world
            .query_filtered::<(&Text, &Visibility), With<area_debug::PlayerAreaDescription>>()
            .single(world)
            .expect("the area debug player label remains available");
        assert_eq!(*visibility, Visibility::Hidden);

        let world = app.world_mut();
        let actor_count = world
            .query_filtered::<Entity, With<actors::ActorView>>()
            .iter(world)
            .count();
        assert_eq!(actor_count, 103);
        let runtime_dir = world
            .resource::<bridge::BridgeHandle>()
            .runtime_dir()
            .to_path_buf();

        app.world_mut().write_message(InjectPlayerTranscript {
            text: "What's your name?".into(),
            target_id: Some(model::ActorId("k0fb1".into())),
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
            target_id: Some(model::ActorId("k0fb1".into())),
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
