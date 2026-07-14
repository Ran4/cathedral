//! Ordered speech presentation and dynamic spatial WAV/streaming PCM playback.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{
    audio::{
        AudioPlayer, AudioSinkPlayback, AudioSource, ChannelCount, Decodable, PlaybackSettings,
        SampleRate, Source, SpatialAudioSink, SpatialScale, Volume,
    },
    prelude::*,
};
use cathedral_sim::TtsBackendKind;
use crossbeam_channel::{Receiver, TryRecvError, bounded};

use crate::{controller::PlayerController, fonts::CathedralFonts, smart_actors::HEARING_RADIUS_M};

use super::{
    SmartActorsConfig, bridge,
    hud::SmartActorHudState,
    microphone::{MicrophoneCommand, MicrophoneService},
    model::ActorId,
};

const MAX_WAV_BYTES: usize = 16 * 1024 * 1024;
// Initial setup may still be downloading while the persistent Pocket worker
// warms in the background. A keyed failure releases the line immediately;
// this is only a last-resort guard for a worker that never responds.
const TTS_WAIT_SECONDS: f64 = 600.0;
const AUDIO_SINK_START_TIMEOUT_SECONDS: f64 = 2.0;
const AUDIO_PLAYBACK_TIMEOUT_SECONDS: f64 = 60.0;
const MICROPHONE_SUSPEND_TIMEOUT_SECONDS: f64 = 2.0;
const LOCAL_TTS_PLAYBACK_SPEED: f32 = 1.05;
// Rodio applies inverse-square distance attenuation to spatial sources. Our
// own speech_gain curve already defines the complete 3-20 m loudness policy,
// so fit the whole hearing radius inside Rodio's unattenuated unit sphere and
// retain its stereo panning without multiplying in a second falloff.
const NPC_VOICE_SPATIAL_SCALE: f32 = 1.0 / HEARING_RADIUS_M;
/// Neutral backing for the world-projected UI panel. This remains translucent
/// so dialogue feels attached to the world while staying legible against
/// stone, sky, and the actors' light skin tones.
const DIALOGUE_BACKDROP: Color = Color::srgba(0.10, 0.105, 0.11, 0.82);

fn streaming_playback_speed(backend: Option<TtsBackendKind>) -> f32 {
    if backend == Some(TtsBackendKind::Local) {
        LOCAL_TTS_PLAYBACK_SPEED
    } else {
        1.0
    }
}

/// A validated speech event that the bridge determined was heard by the player.
#[derive(Message, Debug, Clone)]
pub struct PresentSpeech {
    pub event_seq: u64,
    pub event_id: String,
    pub speaker_id: ActorId,
    pub speaker_label: String,
    pub text: String,
    pub speaker_position: Vec3,
    pub recipient_count: usize,
    pub expect_audio: bool,
}

/// WAV bytes copied and path-validated by the bridge worker.
#[derive(Message, Debug, Clone)]
pub struct TtsClipReady {
    pub event_id: String,
    pub wav_bytes: Arc<[u8]>,
}

#[derive(Message, Debug, Clone)]
pub struct TtsClipFailed {
    pub event_id: String,
    pub reason: String,
}

#[derive(Message, Debug, Clone)]
pub struct TtsPcmChunkReady {
    pub event_id: String,
    pub chunk_seq: u32,
    pub sample_rate: u32,
    pub samples: Arc<[i16]>,
    pub backend: Option<TtsBackendKind>,
}

#[derive(Message, Debug, Clone)]
pub struct TtsStreamFinished {
    pub event_id: String,
    pub chunk_count: u32,
    pub first_chunk_ms: u32,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct StopNpcSpeech;

#[derive(Message, Debug, Clone, Copy)]
pub struct ClearSpeechPresentation;

#[derive(Debug)]
struct SubtitleLine {
    event_id: String,
    text: String,
    minimum_seconds: f64,
    visible_since: Option<f64>,
    audio_playing: bool,
}

#[derive(Debug)]
struct AudioExpectation {
    event_id: String,
    position: Vec3,
    queued_at: f64,
}

#[derive(Debug)]
struct ReadyClip {
    bytes: Arc<[u8]>,
}

#[derive(Debug, Default)]
struct PcmBuffer {
    samples: VecDeque<f32>,
    finished: bool,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub(super) struct StreamingPcmSource {
    buffer: Arc<Mutex<PcmBuffer>>,
    sample_rate: u32,
}

impl StreamingPcmSource {
    fn new(sample_rate: u32) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(PcmBuffer::default())),
            sample_rate,
        }
    }

    fn push(&self, samples: &[i16]) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer
                .samples
                .extend(samples.iter().map(|sample| f32::from(*sample) / 32768.0));
        }
    }

    fn finish(&self) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.finished = true;
        }
    }
}

pub(super) struct StreamingPcmDecoder {
    buffer: Arc<Mutex<PcmBuffer>>,
    sample_rate: u32,
}

impl Iterator for StreamingPcmDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let Ok(mut buffer) = self.buffer.lock() else {
            return None;
        };
        if let Some(sample) = buffer.samples.pop_front() {
            Some(sample)
        } else if buffer.finished {
            None
        } else {
            // Never block the audio callback. Streaming TTS should run faster
            // than real-time after its first chunk, so this is only an
            // underrun guard and normally emits no audible gap.
            Some(0.0)
        }
    }
}

impl Source for StreamingPcmDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).expect("one channel is non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.sample_rate).expect("validated sample rate is non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for StreamingPcmSource {
    type Decoder = StreamingPcmDecoder;

    fn decoder(&self) -> Self::Decoder {
        StreamingPcmDecoder {
            buffer: self.buffer.clone(),
            sample_rate: self.sample_rate,
        }
    }
}

#[derive(Debug)]
struct PendingPcmStream {
    source: StreamingPcmSource,
    next_chunk_seq: u32,
    backend: Option<TtsBackendKind>,
}

#[derive(Debug)]
struct ActiveVoice {
    entity: Entity,
    event_id: String,
    started_at: f64,
}

/// Ordered presentation queues. They are transient and cleared on disconnect.
#[derive(Resource, Debug, Default)]
pub struct SpeechPresentationState {
    last_event_seq: Option<u64>,
    subtitles: VecDeque<SubtitleLine>,
    audio_order: VecDeque<AudioExpectation>,
    ready_audio: HashMap<String, ReadyClip>,
    pcm_streams: HashMap<String, PendingPcmStream>,
    /// One auto-layout root per NPC; its children retain accepted event order.
    bubble_stacks: HashMap<ActorId, Entity>,
    active_voice: Option<ActiveVoice>,
    microphone_suspended_for_voice: bool,
    microphone_suspend_ack: Option<Receiver<()>>,
    microphone_suspend_started_at: Option<f64>,
}

impl SpeechPresentationState {
    pub fn clear(&mut self) {
        self.last_event_seq = None;
        self.subtitles.clear();
        self.audio_order.clear();
        self.ready_audio.clear();
        for stream in self.pcm_streams.values() {
            stream.source.finish();
        }
        self.pcm_streams.clear();
        self.bubble_stacks.clear();
        self.active_voice = None;
        self.microphone_suspended_for_voice = false;
        self.microphone_suspend_ack = None;
        self.microphone_suspend_started_at = None;
    }

    fn observe_speech_sequence(&mut self, event_seq: u64) -> bool {
        if self.last_event_seq.is_some_and(|last| event_seq <= last) {
            return false;
        }
        self.last_event_seq = Some(event_seq);
        true
    }
}

#[derive(Component, Debug)]
pub(super) struct SpeechBubbleStack {
    world_position: Vec3,
}

#[derive(Component, Debug)]
pub(super) struct SpeechBubble {
    expires_at: f64,
    event_id: String,
}

#[derive(Component)]
pub(super) struct NpcVoice;

pub fn receive_speech_events(
    mut commands: Commands,
    time: Res<Time>,
    mut messages: MessageReader<PresentSpeech>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
    fonts: Option<Res<CathedralFonts>>,
) {
    let now = time.elapsed_secs_f64();
    let speech_font = fonts
        .as_deref()
        .map(CathedralFonts::body)
        .unwrap_or_default();
    for message in messages.read() {
        if !state.observe_speech_sequence(message.event_seq) {
            continue;
        }
        let text = message.text.trim();
        if text.is_empty() || text.chars().count() > super::PLAYER_SPEECH_MAX_CHARS {
            continue;
        }
        if message.speaker_id.0 == "player" {
            // Player STT has its own tiny, immediate bottom caption. Keeping
            // it out of the NPC subtitle/TTS queue prevents an earlier voiced
            // line from hiding confirmation that the microphone worked.
            hud.show_player_transcript_delivery(text, message.recipient_count);
            continue;
        }
        let duration = speech_text_seconds(text);
        let label = message.speaker_label.as_str();
        let first_subtitle = state.subtitles.is_empty();
        state.subtitles.push_back(SubtitleLine {
            event_id: message.event_id.clone(),
            text: format!("{label}: {text}"),
            minimum_seconds: f64::from(duration),
            visible_since: first_subtitle.then_some(now),
            audio_playing: false,
        });

        spawn_speech_bubble(
            &mut commands,
            &mut state.bubble_stacks,
            &message.speaker_id,
            message.speaker_position,
            text,
            &message.event_id,
            now + f64::from(duration),
            speech_font.clone(),
        );
        if message.expect_audio {
            state.audio_order.push_back(AudioExpectation {
                event_id: message.event_id.clone(),
                position: message.speaker_position,
                queued_at: now,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_speech_bubble(
    commands: &mut Commands,
    bubble_stacks: &mut HashMap<ActorId, Entity>,
    speaker_id: &ActorId,
    speaker: Vec3,
    text: &str,
    event_id: &str,
    expires_at: f64,
    font: FontSource,
) {
    let world_position = speaker + Vec3::Y * 1.35;
    // `Commands::spawn` reserves the entity immediately, so even several
    // messages read in this frame can attach to the same speaker stack.
    let stack_entity = if let Some(entity) = bubble_stacks.get(speaker_id).copied() {
        commands
            .entity(entity)
            .insert(SpeechBubbleStack { world_position });
        entity
    } else {
        let entity = commands
            .spawn((
                Name::new(format!("NPC speech stack: {}", speaker_id.0)),
                SpeechBubbleStack { world_position },
                Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(6),
                    ..default()
                },
                UiTransform::from_xy(percent(-50), percent(-100)),
                ZIndex(9),
                Visibility::Hidden,
            ))
            .id();
        bubble_stacks.insert(speaker_id.clone(), entity);
        entity
    };

    commands.entity(stack_entity).with_child((
        Name::new("NPC speech bubble"),
        SpeechBubble {
            expires_at,
            event_id: event_id.to_owned(),
        },
        Text::new(text),
        TextFont {
            font,
            font_size: FontSize::Px(28.8),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.97, 0.87)),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node {
            max_width: px(600),
            padding: UiRect::axes(px(10), px(6)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(DIALOGUE_BACKDROP),
    ));
}

pub fn update_speech_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<SpeechPresentationState>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::controller::PlayerCamera>>,
    bubbles: Query<(Entity, &SpeechBubble)>,
    mut stacks: Query<(&SpeechBubbleStack, &mut Node, &mut Visibility)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, bubble) in &bubbles {
        let audio_is_live = state
            .audio_order
            .iter()
            .any(|expected| expected.event_id == bubble.event_id)
            || state
                .active_voice
                .as_ref()
                .is_some_and(|voice| voice.event_id == bubble.event_id);
        if now >= bubble.expires_at && !audio_is_live {
            commands.entity(entity).despawn();
        }
    }

    let camera = cameras.single().ok();
    for (stack, mut node, mut visibility) in &mut stacks {
        let Some((camera, camera_transform)) = camera else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(viewport_position) =
            camera.world_to_viewport(camera_transform, stack.world_position)
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        node.left = px(viewport_position.x);
        node.top = px(viewport_position.y);
        *visibility = Visibility::Inherited;
    }
}

pub fn receive_tts_clips(
    mut messages: MessageReader<TtsClipReady>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: Option<ResMut<SmartActorHudState>>,
) {
    for message in messages.read() {
        if !valid_wav(&message.wav_bytes) {
            voice_toast(
                hud.as_deref_mut(),
                "NPC voice WAV was invalid; text remains available",
            );
            continue;
        }
        if !state
            .audio_order
            .iter()
            .any(|expected| expected.event_id == message.event_id)
        {
            voice_toast(
                hud.as_deref_mut(),
                "NPC voice arrived too late; text remains available",
            );
            continue;
        }
        state.ready_audio.insert(
            message.event_id.clone(),
            ReadyClip {
                bytes: message.wav_bytes.clone(),
            },
        );
    }
}

pub fn receive_tts_failures(
    mut messages: MessageReader<TtsClipFailed>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
    handle: Option<Res<bridge::BridgeHandle>>,
) {
    for message in messages.read() {
        let before = state.audio_order.len();
        state
            .audio_order
            .retain(|expected| expected.event_id != message.event_id);
        state.ready_audio.remove(&message.event_id);
        if let Some(stream) = state.pcm_streams.remove(&message.event_id) {
            stream.source.finish();
        }
        if state.audio_order.len() != before {
            notify_speech_presented(handle.as_deref(), &message.event_id);
            voice_toast(
                Some(&mut hud),
                format!(
                    "NPC voice failed: {}; text remains available",
                    message.reason
                ),
            );
        }
    }
}

pub fn receive_tts_pcm_chunks(
    mut messages: MessageReader<TtsPcmChunkReady>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
    handle: Option<Res<bridge::BridgeHandle>>,
) {
    for message in messages.read() {
        let stream_is_live = state.pcm_streams.contains_key(&message.event_id);
        let audio_is_waiting = state
            .audio_order
            .iter()
            .any(|expected| expected.event_id == message.event_id);
        if message.samples.is_empty()
            || !(8_000..=48_000).contains(&message.sample_rate)
            || (!audio_is_waiting && !stream_is_live)
        {
            continue;
        }
        let invalid = {
            let stream = state
                .pcm_streams
                .entry(message.event_id.clone())
                .or_insert_with(|| PendingPcmStream {
                    source: StreamingPcmSource::new(message.sample_rate),
                    next_chunk_seq: 0,
                    backend: message.backend,
                });
            if stream.next_chunk_seq != message.chunk_seq
                || stream.source.sample_rate != message.sample_rate
                || stream.backend != message.backend
            {
                true
            } else {
                stream.source.push(&message.samples);
                stream.next_chunk_seq += 1;
                false
            }
        };
        if invalid {
            if let Some(stream) = state.pcm_streams.remove(&message.event_id) {
                stream.source.finish();
            }
            let before = state.audio_order.len();
            state
                .audio_order
                .retain(|expected| expected.event_id != message.event_id);
            if state.audio_order.len() != before {
                notify_speech_presented(handle.as_deref(), &message.event_id);
            }
            voice_toast(
                Some(&mut hud),
                "NPC PCM stream was out of order; text remains available",
            );
        }
    }
}

pub fn receive_tts_stream_ends(
    mut messages: MessageReader<TtsStreamFinished>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
    handle: Option<Res<bridge::BridgeHandle>>,
) {
    for message in messages.read() {
        let Some(received_chunks) = state
            .pcm_streams
            .get(&message.event_id)
            .map(|stream| stream.next_chunk_seq)
        else {
            continue;
        };
        if received_chunks != message.chunk_count || message.chunk_count == 0 {
            if let Some(stream) = state.pcm_streams.remove(&message.event_id) {
                stream.source.finish();
            }
            let before = state.audio_order.len();
            state
                .audio_order
                .retain(|expected| expected.event_id != message.event_id);
            if state.audio_order.len() != before {
                notify_speech_presented(handle.as_deref(), &message.event_id);
            }
            voice_toast(
                Some(&mut hud),
                "NPC PCM stream ended incorrectly; text remains available",
            );
            continue;
        }
        if let Some(stream) = state.pcm_streams.get(&message.event_id) {
            stream.source.finish();
        }
        let backend = state
            .pcm_streams
            .get(&message.event_id)
            .and_then(|stream| stream.backend);
        if backend == Some(TtsBackendKind::Local) && message.first_chunk_ms > 300 {
            voice_toast(
                Some(&mut hud),
                format!(
                    "Local voice first PCM took {} ms (target: <300 ms)",
                    message.first_chunk_ms
                ),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_ready_audio(
    mut commands: Commands,
    time: Res<Time>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut streaming_sources: Option<ResMut<Assets<StreamingPcmSource>>>,
    mut state: ResMut<SpeechPresentationState>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    sinks: Query<&SpatialAudioSink, With<NpcVoice>>,
    microphone: Option<Res<MicrophoneService>>,
    config: Res<SmartActorsConfig>,
    mut hud: Option<ResMut<SmartActorHudState>>,
    handle: Option<Res<bridge::BridgeHandle>>,
) {
    let now = time.elapsed_secs_f64();
    if !config.pause_microphone_during_npc_voice {
        let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
    }
    if let Some(active) = state.active_voice.as_ref() {
        let age = now - active.started_at;
        let sink = sinks.get(active.entity);
        let sink_start_timed_out = sink.is_err() && age >= AUDIO_SINK_START_TIMEOUT_SECONDS;
        let playback_timed_out = age >= AUDIO_PLAYBACK_TIMEOUT_SECONDS;
        let finished = sink.is_ok_and(AudioSinkPlayback::empty)
            || commands.get_entity(active.entity).is_err()
            || sink_start_timed_out
            || playback_timed_out;
        if !finished {
            return;
        }
        if sink_start_timed_out || playback_timed_out {
            voice_toast(
                hud.as_deref_mut(),
                if sink_start_timed_out {
                    "NPC voice could not start; text remains available"
                } else {
                    "NPC voice playback timed out; text remains available"
                },
            );
            if let Ok(sink) = sink {
                sink.stop();
            }
            commands.entity(active.entity).try_despawn();
        }
        let event_id = active.event_id.clone();
        state.active_voice = None;
        state.pcm_streams.remove(&event_id);
        notify_speech_presented(handle.as_deref(), &event_id);
        if let Some(line) = state
            .subtitles
            .iter_mut()
            .find(|line| line.event_id == event_id)
        {
            line.audio_playing = false;
        }
    }

    loop {
        let Some(expected) = state.audio_order.front() else {
            let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
            return;
        };
        let subtitle_is_current = state
            .subtitles
            .front()
            .is_some_and(|line| line.event_id == expected.event_id);
        if !subtitle_is_current {
            let still_queued = state
                .subtitles
                .iter()
                .any(|line| line.event_id == expected.event_id);
            if still_queued && now - expected.queued_at <= TTS_WAIT_SECONDS {
                let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
                return;
            }
            let stale = state.audio_order.pop_front().expect("front exists");
            state.ready_audio.remove(&stale.event_id);
            if let Some(stream) = state.pcm_streams.remove(&stale.event_id) {
                stream.source.finish();
            }
            notify_speech_presented(handle.as_deref(), &stale.event_id);
            continue;
        }

        let timed_out = now - expected.queued_at > TTS_WAIT_SECONDS;
        let event_id = expected.event_id.clone();
        let position = expected.position;
        let pcm_ready = state
            .pcm_streams
            .get(&event_id)
            .is_some_and(|stream| stream.next_chunk_seq > 0);
        if !state.ready_audio.contains_key(&event_id) && !pcm_ready {
            if timed_out {
                state.audio_order.pop_front();
                state.pcm_streams.remove(&event_id);
                notify_speech_presented(handle.as_deref(), &event_id);
                continue;
            }
            let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
            return;
        }

        let Ok(player) = players.single() else {
            let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
            return;
        };
        let distance = player.translation().distance(position);
        let Some(gain) = speech_gain(distance) else {
            state.ready_audio.remove(&event_id);
            if let Some(stream) = state.pcm_streams.remove(&event_id) {
                stream.source.finish();
            }
            state.audio_order.pop_front();
            notify_speech_presented(handle.as_deref(), &event_id);
            let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
            return;
        };
        match if config.pause_microphone_during_npc_voice {
            microphone_suspension_readiness(&mut state, microphone.as_deref(), now)
        } else {
            MicrophoneSuspensionReadiness::Ready
        } {
            MicrophoneSuspensionReadiness::Ready => {}
            MicrophoneSuspensionReadiness::Waiting => {
                // Wait for the worker to confirm that the input stream has
                // stopped before any NPC audio reaches the output device.
                return;
            }
            MicrophoneSuspensionReadiness::Failed(reason) => {
                // A dead audio worker must not strand subtitles or the mic.
                // Drop this optional TTS clip and keep the complete text path.
                state.ready_audio.remove(&event_id);
                if let Some(stream) = state.pcm_streams.remove(&event_id) {
                    stream.source.finish();
                }
                state.audio_order.pop_front();
                notify_speech_presented(handle.as_deref(), &event_id);
                abandon_microphone_suspension(&mut state, microphone.as_deref());
                voice_toast(
                    hud.as_deref_mut(),
                    format!("NPC voice {event_id} was skipped: {reason}; text remains available"),
                );
                return;
            }
        }
        state.audio_order.pop_front();
        let settings = PlaybackSettings::DESPAWN
            .with_spatial(true)
            .with_spatial_scale(SpatialScale::new(NPC_VOICE_SPATIAL_SCALE))
            .with_volume(Volume::Linear(gain));
        let entity = if let Some(clip) = state.ready_audio.remove(&event_id) {
            let source = audio_sources.add(AudioSource { bytes: clip.bytes });
            commands
                .spawn((
                    Name::new("NPC spatial voice"),
                    NpcVoice,
                    AudioPlayer::new(source),
                    settings,
                    Transform::from_translation(position),
                ))
                .id()
        } else {
            let Some(streaming_sources) = streaming_sources.as_deref_mut() else {
                state.pcm_streams.remove(&event_id);
                notify_speech_presented(handle.as_deref(), &event_id);
                abandon_microphone_suspension(&mut state, microphone.as_deref());
                voice_toast(
                    hud.as_deref_mut(),
                    "Streaming audio output is unavailable; text remains available",
                );
                return;
            };
            let stream = state
                .pcm_streams
                .get(&event_id)
                .expect("PCM readiness was checked");
            let playback_speed = streaming_playback_speed(stream.backend);
            let source = stream.source.clone();
            let source = streaming_sources.add(source);
            commands
                .spawn((
                    Name::new("NPC streaming spatial voice"),
                    NpcVoice,
                    AudioPlayer(source),
                    settings.with_speed(playback_speed),
                    Transform::from_translation(position),
                ))
                .id()
        };
        if let Some(line) = state.subtitles.front_mut() {
            line.audio_playing = true;
        }
        state.active_voice = Some(ActiveVoice {
            entity,
            event_id: event_id.clone(),
            started_at: now,
        });
        println!(
            "[smart actors/audio] starting NPC voice {event_id}: distance={distance:.2}m gain={gain:.3}"
        );
        return;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MicrophoneSuspensionReadiness {
    Ready,
    Waiting,
    Failed(&'static str),
}

fn microphone_suspension_readiness(
    state: &mut SpeechPresentationState,
    microphone: Option<&MicrophoneService>,
    now: f64,
) -> MicrophoneSuspensionReadiness {
    let Some(microphone) = microphone else {
        state.microphone_suspended_for_voice = false;
        state.microphone_suspend_ack = None;
        state.microphone_suspend_started_at = None;
        return MicrophoneSuspensionReadiness::Ready;
    };

    let started_at = *state.microphone_suspend_started_at.get_or_insert(now);

    if !state.microphone_suspended_for_voice {
        let (acknowledged, acknowledgement) = bounded(1);
        if microphone
            .try_send(MicrophoneCommand::Suspend { acknowledged })
            .is_err()
        {
            if now - started_at >= MICROPHONE_SUSPEND_TIMEOUT_SECONDS {
                return MicrophoneSuspensionReadiness::Failed(
                    "the microphone command queue did not accept Suspend within two seconds",
                );
            }
            return MicrophoneSuspensionReadiness::Waiting;
        }
        state.microphone_suspended_for_voice = true;
        state.microphone_suspend_ack = Some(acknowledgement);
        return MicrophoneSuspensionReadiness::Waiting;
    }

    let Some(acknowledgement) = state.microphone_suspend_ack.as_ref() else {
        state.microphone_suspend_started_at = None;
        return MicrophoneSuspensionReadiness::Ready;
    };
    match acknowledgement.try_recv() {
        Ok(()) => {
            state.microphone_suspend_ack = None;
            state.microphone_suspend_started_at = None;
            println!("[smart actors/audio] microphone suspended for NPC playback");
            MicrophoneSuspensionReadiness::Ready
        }
        Err(TryRecvError::Empty) => {
            if now - started_at >= MICROPHONE_SUSPEND_TIMEOUT_SECONDS {
                state.microphone_suspend_ack = None;
                state.microphone_suspend_started_at = None;
                MicrophoneSuspensionReadiness::Failed(
                    "the microphone worker did not acknowledge Suspend within two seconds",
                )
            } else {
                MicrophoneSuspensionReadiness::Waiting
            }
        }
        Err(TryRecvError::Disconnected) => {
            state.microphone_suspend_ack = None;
            state.microphone_suspend_started_at = None;
            MicrophoneSuspensionReadiness::Failed(
                "the microphone worker dropped its Suspend acknowledgement",
            )
        }
    }
}

fn abandon_microphone_suspension(
    state: &mut SpeechPresentationState,
    microphone: Option<&MicrophoneService>,
) {
    state.microphone_suspend_ack = None;
    state.microphone_suspend_started_at = None;
    if !state.microphone_suspended_for_voice {
        return;
    }
    if microphone.is_none_or(|microphone| microphone.try_send(MicrophoneCommand::Resume).is_ok()) {
        state.microphone_suspended_for_voice = false;
    }
}

fn resume_microphone_after_voice(
    state: &mut SpeechPresentationState,
    microphone: Option<&MicrophoneService>,
) -> bool {
    if !state.microphone_suspended_for_voice {
        return true;
    }
    let resumed =
        microphone.is_none_or(|microphone| microphone.try_send(MicrophoneCommand::Resume).is_ok());
    if resumed {
        println!("[smart actors/audio] microphone resumed after NPC playback");
        state.microphone_suspended_for_voice = false;
        state.microphone_suspend_ack = None;
        state.microphone_suspend_started_at = None;
    }
    resumed
}

pub fn update_subtitle_hud(
    time: Res<Time>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
) {
    let now = time.elapsed_secs_f64();
    advance_subtitle_queue(&mut state, now);
    hud.subtitle = state
        .subtitles
        .front()
        .map_or_else(String::new, |line| line.text.clone());
}

/// Starts each line's readable lifetime only when it reaches the front. A line
/// waiting behind another utterance therefore cannot expire unseen. When TTS
/// is expected, keep the current subtitle until its clip arrives or the audio
/// wait window elapses.
fn advance_subtitle_queue(state: &mut SpeechPresentationState, now: f64) {
    loop {
        let Some(front) = state.subtitles.front_mut() else {
            return;
        };
        let visible_since = *front.visible_since.get_or_insert(now);
        let event_id = front.event_id.clone();
        let minimum_elapsed = now - visible_since >= front.minimum_seconds;
        let audio_playing = front.audio_playing;
        let waiting_for_audio = state.audio_order.iter().any(|expected| {
            expected.event_id == event_id && now - expected.queued_at <= TTS_WAIT_SECONDS
        });
        if !minimum_elapsed || audio_playing || waiting_for_audio {
            return;
        }
        state.subtitles.pop_front();
        if let Some(next) = state.subtitles.front_mut() {
            next.visible_since = Some(now);
        }
    }
}

pub fn stop_npc_speech_for_capture(
    mut commands: Commands,
    mut messages: MessageReader<StopNpcSpeech>,
    mut state: ResMut<SpeechPresentationState>,
    sinks: Query<&SpatialAudioSink, With<NpcVoice>>,
    microphone: Option<Res<MicrophoneService>>,
    config: Res<SmartActorsConfig>,
    handle: Option<Res<bridge::BridgeHandle>>,
) {
    if messages.read().next().is_none() {
        return;
    }
    if !config.pause_microphone_during_npc_voice {
        return;
    }
    if let Some(active) = state.active_voice.take() {
        if let Ok(sink) = sinks.get(active.entity) {
            sink.stop();
        }
        commands.entity(active.entity).try_despawn();
        notify_speech_presented(handle.as_deref(), &active.event_id);
        if let Some(line) = state
            .subtitles
            .iter_mut()
            .find(|line| line.event_id == active.event_id)
        {
            line.audio_playing = false;
        }
    }
    // Cut-off and never-started lines are equally terminal for the floor.
    for expected in state.audio_order.drain(..) {
        notify_speech_presented(handle.as_deref(), &expected.event_id);
    }
    state.ready_audio.clear();
    let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
}

#[allow(clippy::type_complexity)]
pub fn clear_speech_presentation(
    mut commands: Commands,
    mut messages: MessageReader<ClearSpeechPresentation>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
    transient_entities: Query<
        Entity,
        Or<(With<SpeechBubbleStack>, With<SpeechBubble>, With<NpcVoice>)>,
    >,
    microphone: Option<Res<MicrophoneService>>,
) {
    if messages.read().next().is_none() {
        return;
    }
    for entity in &transient_entities {
        commands.entity(entity).try_despawn();
    }
    let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
    state.clear();
    hud.subtitle.clear();
}

/// Best-effort notice to the engine that this event's audio presentation reached a
/// terminal state (played, skipped, dropped, failed, or cut off), freeing the
/// conversation floor. Errors are ignored: a lost message only delays the next
/// NPC line until the server-side failsafe deadline expires.
fn notify_speech_presented(handle: Option<&bridge::BridgeHandle>, event_id: &str) {
    let Some(handle) = handle else {
        return;
    };
    let _ = handle.try_send(bridge::BridgeCommand::SpeechPresented {
        speech_event_id: event_id.to_owned(),
    });
}

fn speech_text_seconds(text: &str) -> f32 {
    (2.0 + text.chars().count() as f32 / 15.0).clamp(3.0, 10.0)
}

fn speech_gain(distance_m: f32) -> Option<f32> {
    if !distance_m.is_finite() || !(0.0..=HEARING_RADIUS_M).contains(&distance_m) {
        return None;
    }
    if distance_m <= 3.0 {
        return Some(1.0);
    }
    let t = ((HEARING_RADIUS_M - distance_m) / (HEARING_RADIUS_M - 3.0)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    Some(0.015 + 0.985 * smooth)
}

fn valid_wav(bytes: &[u8]) -> bool {
    (12..=MAX_WAV_BYTES).contains(&bytes.len())
        && bytes.get(0..4) == Some(b"RIFF")
        && bytes.get(8..12) == Some(b"WAVE")
}

fn voice_toast(hud: Option<&mut SmartActorHudState>, message: impl Into<String>) {
    let message = message.into();
    println!("[smart actors/audio] {message}");
    if let Some(hud) = hud {
        hud.toast(message);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::{TimeUpdateStrategy, Virtual};

    use super::*;

    fn speech_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SpeechPresentationState>()
            .init_resource::<SmartActorHudState>()
            .add_message::<PresentSpeech>()
            .add_systems(Update, receive_speech_events);
        app
    }

    fn npc_speech(event_seq: u64, speaker_id: &str, text: impl Into<String>) -> PresentSpeech {
        PresentSpeech {
            event_seq,
            event_id: format!("speech-{event_seq}"),
            speaker_id: ActorId(speaker_id.into()),
            speaker_label: speaker_id.into(),
            text: text.into(),
            speaker_position: Vec3::new(event_seq as f32, 0.0, 0.0),
            recipient_count: 1,
            expect_audio: false,
        }
    }

    fn bubble_count(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut bubbles = world.query_filtered::<Entity, With<SpeechBubble>>();
        bubbles.iter(world).count()
    }

    #[test]
    fn projected_dialogue_uses_a_padded_neutral_text_backdrop() {
        const DIALOGUE: &str = "I have no phone, stranger—only a smith's hands and an empty purse.";

        fn spawn_test_bubble(mut commands: Commands, mut state: ResMut<SpeechPresentationState>) {
            spawn_speech_bubble(
                &mut commands,
                &mut state.bubble_stacks,
                &ActorId("speaker".into()),
                Vec3::ZERO,
                DIALOGUE,
                "test-event",
                10.0,
                FontSource::default(),
            );
        }

        let mut app = App::new();
        app.init_resource::<SpeechPresentationState>()
            .add_systems(Startup, spawn_test_bubble);
        app.update();

        let stack_entity = {
            let mut query = app.world_mut().query_filtered::<
                (&Text, &BackgroundColor, &TextShadow, &Node, &ChildOf),
                With<SpeechBubble>,
            >();
            let (text, background, _shadow, node, parent) = query
                .iter(app.world())
                .next()
                .expect("speech bubble exists");
            assert_eq!(text.0, DIALOGUE, "wrapping must not inject hard newlines");
            assert_eq!(background.0, DIALOGUE_BACKDROP);
            assert_eq!(node.position_type, PositionType::Relative);
            assert_eq!(node.max_width, px(600));
            parent.parent()
        };
        let stack_node = app
            .world()
            .get::<Node>(stack_entity)
            .expect("speech stack is a UI node");
        let stack_transform = app
            .world()
            .get::<UiTransform>(stack_entity)
            .expect("speech stack is projected from its lower centre");
        assert_eq!(stack_node.position_type, PositionType::Absolute);
        assert_eq!(stack_node.flex_direction, FlexDirection::Column);
        assert_eq!(stack_node.align_items, AlignItems::Center);
        assert_eq!(stack_node.row_gap, px(6));
        assert_eq!(stack_transform.translation.x, percent(-50));
        assert_eq!(stack_transform.translation.y, percent(-100));
    }

    #[test]
    fn same_speaker_same_frame_uses_one_ordered_non_overlapping_stack() {
        let mut app = speech_test_app();
        app.world_mut()
            .write_message(npc_speech(1, "sven", "first line"));
        app.world_mut()
            .write_message(npc_speech(2, "sven", "second line"));

        app.update();

        let stack_entity = {
            let state = app.world().resource::<SpeechPresentationState>();
            assert_eq!(state.bubble_stacks.len(), 1);
            state.bubble_stacks[&ActorId("sven".into())]
        };
        let stack_node = app.world().get::<Node>(stack_entity).unwrap();
        assert_eq!(stack_node.flex_direction, FlexDirection::Column);
        assert_eq!(stack_node.row_gap, px(6));

        let children = app.world().get::<Children>(stack_entity).unwrap();
        let ordered_text: Vec<_> = children
            .iter()
            .map(|child| {
                assert_eq!(
                    app.world().get::<Node>(child).unwrap().position_type,
                    PositionType::Relative
                );
                app.world().get::<Text>(child).unwrap().0.as_str()
            })
            .collect();
        assert_eq!(ordered_text, ["first line", "second line"]);
    }

    #[test]
    fn distinct_speakers_use_distinct_stacks() {
        let mut app = speech_test_app();
        app.world_mut()
            .write_message(npc_speech(1, "sven", "from Sven"));
        app.world_mut()
            .write_message(npc_speech(2, "conny", "from Conny"));

        app.update();

        let (sven_stack, conny_stack) = {
            let state = app.world().resource::<SpeechPresentationState>();
            assert_eq!(state.bubble_stacks.len(), 2);
            (
                state.bubble_stacks[&ActorId("sven".into())],
                state.bubble_stacks[&ActorId("conny".into())],
            )
        };
        assert_ne!(sven_stack, conny_stack);
        assert_eq!(app.world().get::<Children>(sven_stack).unwrap().len(), 1);
        assert_eq!(app.world().get::<Children>(conny_stack).unwrap().len(), 1);
    }

    #[test]
    fn stacked_bubbles_keep_individual_expiry_times() {
        let mut app = speech_test_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .add_systems(Update, update_speech_bubbles.after(receive_speech_events));
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(10));
        app.world_mut()
            .write_message(npc_speech(1, "sven", "short"));
        app.world_mut()
            .write_message(npc_speech(2, "sven", "x".repeat(120)));

        app.update();
        assert_eq!(bubble_count(&mut app), 2);

        *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
            TimeUpdateStrategy::ManualDuration(Duration::from_secs(4));
        app.update();
        assert_eq!(bubble_count(&mut app), 1);

        *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
            TimeUpdateStrategy::ManualDuration(Duration::from_secs(6));
        app.update();
        assert_eq!(bubble_count(&mut app), 0);
    }

    #[test]
    fn expected_audio_holds_its_bubble_past_minimum_reading_time() {
        let mut app = speech_test_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .add_systems(Update, update_speech_bubbles.after(receive_speech_events));
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(10));
        let mut message = npc_speech(1, "sven", "short");
        message.expect_audio = true;
        app.world_mut().write_message(message);
        app.update();

        *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
            TimeUpdateStrategy::ManualDuration(Duration::from_secs(4));
        app.update();
        assert_eq!(bubble_count(&mut app), 1);

        app.world_mut()
            .resource_mut::<SpeechPresentationState>()
            .audio_order
            .clear();
        *app.world_mut().resource_mut::<TimeUpdateStrategy>() =
            TimeUpdateStrategy::ManualDuration(Duration::ZERO);
        app.update();
        assert_eq!(bubble_count(&mut app), 0);
    }

    #[test]
    fn clearing_speech_removes_stacks_children_and_tracking() {
        let mut app = speech_test_app();
        app.add_message::<ClearSpeechPresentation>().add_systems(
            Update,
            clear_speech_presentation.after(receive_speech_events),
        );
        app.world_mut()
            .write_message(npc_speech(1, "sven", "temporary"));
        app.update();
        assert_eq!(bubble_count(&mut app), 1);

        app.world_mut().write_message(ClearSpeechPresentation);
        app.update();

        assert_eq!(bubble_count(&mut app), 0);
        let world = app.world_mut();
        let mut stacks = world.query_filtered::<Entity, With<SpeechBubbleStack>>();
        assert_eq!(stacks.iter(world).count(), 0);
        assert!(
            world
                .resource::<SpeechPresentationState>()
                .bubble_stacks
                .is_empty()
        );
    }

    #[test]
    fn bubble_duration_matches_the_settled_formula() {
        assert_eq!(speech_text_seconds("hi"), 3.0);
        assert_eq!(speech_text_seconds(&"x".repeat(45)), 5.0);
        assert_eq!(speech_text_seconds(&"x".repeat(500)), 10.0);
    }

    #[test]
    fn audio_has_a_hard_inclusive_hearing_gate_and_smooth_gain() {
        assert_eq!(speech_gain(0.0), Some(1.0));
        assert!(speech_gain(10.0).unwrap() < 1.0);
        assert_eq!(speech_gain(20.0), Some(0.015));
        assert_eq!(speech_gain(20.001), None);
        assert_eq!(speech_gain(f32::NAN), None);
        assert_eq!(HEARING_RADIUS_M * NPC_VOICE_SPATIAL_SCALE, 1.0);
    }

    #[test]
    fn npc_audio_waits_for_confirmed_microphone_suspension_and_resumes_once() {
        let (microphone, commands) = MicrophoneService::command_harness_for_tests();
        let mut state = SpeechPresentationState::default();

        assert_eq!(
            microphone_suspension_readiness(&mut state, Some(&microphone), 0.0),
            MicrophoneSuspensionReadiness::Waiting
        );
        let acknowledged = match commands.try_recv().expect("suspend command") {
            MicrophoneCommand::Suspend { acknowledged } => acknowledged,
            other => panic!("expected suspend command, got {other:?}"),
        };
        acknowledged.send(()).unwrap();
        assert_eq!(
            microphone_suspension_readiness(&mut state, Some(&microphone), 0.1),
            MicrophoneSuspensionReadiness::Ready
        );
        assert!(commands.try_recv().is_err(), "suspend was sent twice");

        assert!(resume_microphone_after_voice(&mut state, Some(&microphone)));
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Resume)));
    }

    #[test]
    fn dead_microphone_worker_drops_optional_tts_after_a_bounded_wait() {
        let (microphone, commands) = MicrophoneService::command_harness_for_tests();
        let mut state = SpeechPresentationState::default();
        assert_eq!(
            microphone_suspension_readiness(&mut state, Some(&microphone), 4.0),
            MicrophoneSuspensionReadiness::Waiting
        );
        let _pending = commands.try_recv().expect("suspend command");
        assert!(matches!(
            microphone_suspension_readiness(
                &mut state,
                Some(&microphone),
                4.0 + MICROPHONE_SUSPEND_TIMEOUT_SECONDS,
            ),
            MicrophoneSuspensionReadiness::Failed(_)
        ));
        abandon_microphone_suspension(&mut state, Some(&microphone));
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Resume)));
    }

    #[test]
    fn malformed_audio_is_rejected_before_bevy_decodes_it() {
        assert!(!valid_wav(b"not a wave"));
        assert!(valid_wav(b"RIFF\0\0\0\0WAVE"));
    }

    #[test]
    fn streaming_pcm_decoder_starts_immediately_and_finishes_after_buffer_drains() {
        let source = StreamingPcmSource::new(24_000);
        source.push(&[i16::MIN, 0, i16::MAX]);
        let mut decoder = source.decoder();

        assert_eq!(decoder.next(), Some(-1.0));
        assert_eq!(decoder.next(), Some(0.0));
        assert_eq!(decoder.next(), Some(i16::MAX as f32 / 32768.0));
        assert_eq!(
            decoder.next(),
            Some(0.0),
            "an underrun must not block audio"
        );
        source.finish();
        assert_eq!(decoder.next(), None);
    }

    #[test]
    fn only_local_streaming_voice_gets_the_pocket_speedup() {
        assert_eq!(streaming_playback_speed(Some(TtsBackendKind::Local)), 1.05);
        assert_eq!(streaming_playback_speed(Some(TtsBackendKind::Cloud)), 1.0);
        assert_eq!(streaming_playback_speed(None), 1.0);
    }

    #[test]
    fn out_of_order_pcm_chunk_releases_the_waiting_subtitle() {
        let mut app = App::new();
        let mut state = SpeechPresentationState::default();
        state.audio_order.push_back(AudioExpectation {
            event_id: "speech-1".into(),
            position: Vec3::ZERO,
            queued_at: 0.0,
        });
        app.insert_resource(state)
            .init_resource::<SmartActorHudState>()
            .add_message::<TtsPcmChunkReady>()
            .add_systems(Update, receive_tts_pcm_chunks);
        app.world_mut().write_message(TtsPcmChunkReady {
            event_id: "speech-1".into(),
            chunk_seq: 0,
            sample_rate: 24_000,
            samples: Arc::from([1_i16, 2]),
            backend: Some(TtsBackendKind::Local),
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<SpeechPresentationState>()
                .pcm_streams["speech-1"]
                .next_chunk_seq,
            1
        );

        app.world_mut().write_message(TtsPcmChunkReady {
            event_id: "speech-1".into(),
            chunk_seq: 2,
            sample_rate: 24_000,
            samples: Arc::from([3_i16, 4]),
            backend: Some(TtsBackendKind::Local),
        });
        app.update();
        assert!(
            app.world()
                .resource::<SpeechPresentationState>()
                .audio_order
                .is_empty()
        );
    }

    #[test]
    fn live_pcm_stream_keeps_accepting_chunks_after_playback_starts() {
        let mut app = App::new();
        let mut state = SpeechPresentationState::default();
        let source = StreamingPcmSource::new(24_000);
        source.push(&[1]);
        state.pcm_streams.insert(
            "speech-1".into(),
            PendingPcmStream {
                source,
                next_chunk_seq: 1,
                backend: Some(TtsBackendKind::Local),
            },
        );
        // start_ready_audio removes the event from audio_order as soon as
        // chunk zero is handed to the audio sink. Later chunks still belong
        // to the live stream stored above.
        app.insert_resource(state)
            .init_resource::<SmartActorHudState>()
            .add_message::<TtsPcmChunkReady>()
            .add_systems(Update, receive_tts_pcm_chunks);
        app.world_mut().write_message(TtsPcmChunkReady {
            event_id: "speech-1".into(),
            chunk_seq: 1,
            sample_rate: 24_000,
            samples: Arc::from([2_i16, 3]),
            backend: Some(TtsBackendKind::Local),
        });

        app.update();

        assert_eq!(
            app.world()
                .resource::<SpeechPresentationState>()
                .pcm_streams["speech-1"]
                .next_chunk_seq,
            2
        );
    }

    #[test]
    fn subtitle_stream_rejects_stale_or_duplicate_event_sequences() {
        let mut state = SpeechPresentationState::default();
        assert!(state.observe_speech_sequence(4));
        assert!(!state.observe_speech_sequence(4));
        assert!(!state.observe_speech_sequence(3));
        assert!(state.observe_speech_sequence(5));
    }

    #[test]
    fn player_speech_uses_the_tiny_caption_instead_of_npc_subtitle_queue() {
        let mut app = speech_test_app();
        app.world_mut().write_message(PresentSpeech {
            event_seq: 1,
            event_id: "speech-1".into(),
            speaker_id: ActorId("player".into()),
            speaker_label: "You".into(),
            text: "Can anyone hear me?".into(),
            speaker_position: Vec3::ZERO,
            recipient_count: 3,
            expect_audio: false,
        });

        app.update();

        assert!(
            app.world()
                .resource::<SpeechPresentationState>()
                .subtitles
                .is_empty()
        );
        assert_eq!(
            app.world()
                .resource::<SmartActorHudState>()
                .player_transcript_text(),
            Some("You: Can anyone hear me?  ·  heard by 3 nearby people")
        );
    }

    #[test]
    fn queued_subtitles_receive_their_full_visible_duration() {
        let mut state = SpeechPresentationState::default();
        state.subtitles.push_back(SubtitleLine {
            event_id: "speech-1".into(),
            text: "Ilse: first".into(),
            minimum_seconds: 3.0,
            visible_since: Some(0.0),
            audio_playing: false,
        });
        state.subtitles.push_back(SubtitleLine {
            event_id: "speech-2".into(),
            text: "Conny: second".into(),
            minimum_seconds: 3.0,
            visible_since: None,
            audio_playing: false,
        });

        advance_subtitle_queue(&mut state, 4.0);
        assert_eq!(state.subtitles.front().unwrap().event_id, "speech-2");
        assert_eq!(state.subtitles.front().unwrap().visible_since, Some(4.0));
        advance_subtitle_queue(&mut state, 6.9);
        assert_eq!(state.subtitles.len(), 1);
        advance_subtitle_queue(&mut state, 7.0);
        assert!(state.subtitles.is_empty());
    }

    #[test]
    fn expected_audio_holds_the_current_subtitle_until_timeout() {
        let mut state = SpeechPresentationState::default();
        state.subtitles.push_back(SubtitleLine {
            event_id: "speech-1".into(),
            text: "Ilse: hello".into(),
            minimum_seconds: 3.0,
            visible_since: Some(0.0),
            audio_playing: false,
        });
        state.audio_order.push_back(AudioExpectation {
            event_id: "speech-1".into(),
            position: Vec3::ZERO,
            queued_at: 0.0,
        });

        advance_subtitle_queue(&mut state, 599.9);
        assert_eq!(state.subtitles.len(), 1);
        advance_subtitle_queue(&mut state, 600.1);
        assert!(state.subtitles.is_empty());
    }

    #[test]
    fn keyed_tts_failure_releases_audio_wait_immediately() {
        let mut app = App::new();
        let mut state = SpeechPresentationState::default();
        state.audio_order.push_back(AudioExpectation {
            event_id: "speech-1".into(),
            position: Vec3::ZERO,
            queued_at: 0.0,
        });
        app.insert_resource(state)
            .init_resource::<SmartActorHudState>()
            .add_message::<TtsClipFailed>()
            .add_systems(Update, receive_tts_failures);
        app.world_mut().write_message(TtsClipFailed {
            event_id: "speech-1".into(),
            reason: "local model failed".into(),
        });

        app.update();

        assert!(
            app.world()
                .resource::<SpeechPresentationState>()
                .audio_order
                .is_empty()
        );
    }
}
