//! Ordered speech presentation and dynamic spatial WAV playback.

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use bevy::{
    audio::{
        AudioPlayer, AudioSinkPlayback, AudioSource, PlaybackSettings, SpatialAudioSink, Volume,
    },
    prelude::*,
};
use crossbeam_channel::{Receiver, TryRecvError, bounded};

use crate::{controller::PlayerController, smart_actors::HEARING_RADIUS_M};

use super::{
    hud::SmartActorHudState,
    microphone::{MicrophoneCommand, MicrophoneService},
    model::ActorId,
};

const MAX_WAV_BYTES: usize = 16 * 1024 * 1024;
const TTS_WAIT_SECONDS: f64 = 10.0;
const AUDIO_SINK_START_TIMEOUT_SECONDS: f64 = 2.0;
const AUDIO_PLAYBACK_TIMEOUT_SECONDS: f64 = 60.0;
const MICROPHONE_SUSPEND_TIMEOUT_SECONDS: f64 = 2.0;
/// Neutral backing for the world-projected UI panel. This remains translucent
/// so dialogue feels attached to the world while staying legible against
/// stone, sky, and the actors' light skin tones.
const DIALOGUE_BACKDROP: Color = Color::srgba(0.10, 0.105, 0.11, 0.82);

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

#[derive(Component)]
pub(super) struct SpeechBubble {
    expires_at: f64,
    world_position: Vec3,
}

#[derive(Component)]
pub(super) struct NpcVoice;

pub fn receive_speech_events(
    mut commands: Commands,
    time: Res<Time>,
    mut messages: MessageReader<PresentSpeech>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
) {
    let now = time.elapsed_secs_f64();
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
            message.speaker_position,
            text,
            now + f64::from(duration),
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

fn spawn_speech_bubble(commands: &mut Commands, speaker: Vec3, text: &str, expires_at: f64) {
    commands.spawn((
        Name::new("NPC speech bubble"),
        SpeechBubble {
            expires_at,
            world_position: speaker + Vec3::Y * 2.75,
        },
        Text::new(wrap_dialogue(text, 42)),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.97, 0.87)),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            max_width: px(420),
            padding: UiRect::axes(px(10), px(6)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        UiTransform::from_xy(percent(-50), percent(-100)),
        BackgroundColor(DIALOGUE_BACKDROP),
        ZIndex(9),
        Visibility::Hidden,
    ));
}

pub fn update_speech_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::controller::PlayerCamera>>,
    mut bubbles: Query<(Entity, &SpeechBubble, &mut Node, &mut Visibility)>,
) {
    let now = time.elapsed_secs_f64();
    let camera = cameras.single().ok();
    for (entity, bubble, mut node, mut visibility) in &mut bubbles {
        if now >= bubble.expires_at {
            commands.entity(entity).despawn();
            continue;
        }
        let Some((camera, camera_transform)) = camera else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(viewport_position) =
            camera.world_to_viewport(camera_transform, bubble.world_position)
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
) {
    for message in messages.read() {
        if !valid_wav(&message.wav_bytes)
            || !state
                .audio_order
                .iter()
                .any(|expected| expected.event_id == message.event_id)
        {
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

#[allow(clippy::too_many_arguments)]
pub fn start_ready_audio(
    mut commands: Commands,
    time: Res<Time>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut state: ResMut<SpeechPresentationState>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    sinks: Query<&SpatialAudioSink, With<NpcVoice>>,
    microphone: Option<Res<MicrophoneService>>,
) {
    let now = time.elapsed_secs_f64();
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
            if let Ok(sink) = sink {
                sink.stop();
            }
            commands.entity(active.entity).try_despawn();
        }
        let event_id = active.event_id.clone();
        state.active_voice = None;
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
            continue;
        }

        let timed_out = now - expected.queued_at > TTS_WAIT_SECONDS;
        let event_id = expected.event_id.clone();
        let position = expected.position;
        if !state.ready_audio.contains_key(&event_id) {
            if timed_out {
                state.audio_order.pop_front();
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
            state.audio_order.pop_front();
            let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
            return;
        };
        match microphone_suspension_readiness(&mut state, microphone.as_deref(), now) {
            MicrophoneSuspensionReadiness::Ready => {}
            MicrophoneSuspensionReadiness::Waiting => {
                // Wait for the worker to confirm that the input stream has
                // stopped before any NPC audio reaches the output device.
                return;
            }
            MicrophoneSuspensionReadiness::Failed => {
                // A dead audio worker must not strand subtitles or the mic.
                // Drop this optional TTS clip and keep the complete text path.
                state.ready_audio.remove(&event_id);
                state.audio_order.pop_front();
                abandon_microphone_suspension(&mut state, microphone.as_deref());
                return;
            }
        }
        let clip = state
            .ready_audio
            .remove(&event_id)
            .expect("ready clip was checked above");
        state.audio_order.pop_front();
        let source = audio_sources.add(AudioSource { bytes: clip.bytes });
        let entity = commands
            .spawn((
                Name::new("NPC spatial voice"),
                NpcVoice,
                AudioPlayer::new(source),
                PlaybackSettings::DESPAWN
                    .with_spatial(true)
                    .with_volume(Volume::Linear(gain)),
                Transform::from_translation(position),
            ))
            .id();
        if let Some(line) = state.subtitles.front_mut() {
            line.audio_playing = true;
        }
        state.active_voice = Some(ActiveVoice {
            entity,
            event_id,
            started_at: now,
        });
        return;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MicrophoneSuspensionReadiness {
    Ready,
    Waiting,
    Failed,
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
    if now - started_at >= MICROPHONE_SUSPEND_TIMEOUT_SECONDS {
        return MicrophoneSuspensionReadiness::Failed;
    }

    if !state.microphone_suspended_for_voice {
        let (acknowledged, acknowledgement) = bounded(1);
        if microphone
            .try_send(MicrophoneCommand::Suspend { acknowledged })
            .is_err()
        {
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
            MicrophoneSuspensionReadiness::Ready
        }
        Err(TryRecvError::Empty) => MicrophoneSuspensionReadiness::Waiting,
        Err(TryRecvError::Disconnected) => {
            state.microphone_suspended_for_voice = false;
            state.microphone_suspend_ack = None;
            state.microphone_suspend_started_at = None;
            MicrophoneSuspensionReadiness::Failed
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
) {
    if messages.read().next().is_none() {
        return;
    }
    if let Some(active) = state.active_voice.take() {
        if let Ok(sink) = sinks.get(active.entity) {
            sink.stop();
        }
        commands.entity(active.entity).try_despawn();
        if let Some(line) = state
            .subtitles
            .iter_mut()
            .find(|line| line.event_id == active.event_id)
        {
            line.audio_playing = false;
        }
    }
    state.audio_order.clear();
    state.ready_audio.clear();
    let _ = resume_microphone_after_voice(&mut state, microphone.as_deref());
}

#[allow(clippy::type_complexity)]
pub fn clear_speech_presentation(
    mut commands: Commands,
    mut messages: MessageReader<ClearSpeechPresentation>,
    mut state: ResMut<SpeechPresentationState>,
    mut hud: ResMut<SmartActorHudState>,
    transient_entities: Query<Entity, Or<(With<SpeechBubble>, With<NpcVoice>)>>,
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

fn wrap_dialogue(text: &str, width: usize) -> String {
    let mut output = String::with_capacity(text.len());
    let mut line_len = 0;
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if line_len > 0 && line_len + 1 + word_len > width {
            output.push('\n');
            line_len = 0;
        } else if line_len > 0 {
            output.push(' ');
            line_len += 1;
        }
        output.push_str(word);
        line_len += word_len;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_dialogue_uses_a_padded_neutral_text_backdrop() {
        fn spawn_test_bubble(mut commands: Commands) {
            spawn_speech_bubble(&mut commands, Vec3::ZERO, "Readable dialogue", 10.0);
        }

        let mut app = App::new();
        app.add_systems(Startup, spawn_test_bubble);
        app.update();

        let mut query = app.world_mut().query_filtered::<
            (&BackgroundColor, &TextShadow, &Node, &UiTransform),
            With<SpeechBubble>,
        >();
        let (background, _shadow, node, transform) = query
            .iter(app.world())
            .next()
            .expect("speech bubble exists");
        assert_eq!(background.0, DIALOGUE_BACKDROP);
        assert_eq!(node.max_width, px(420));
        assert_eq!(transform.translation.x, percent(-50));
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
        assert_eq!(
            microphone_suspension_readiness(
                &mut state,
                Some(&microphone),
                4.0 + MICROPHONE_SUSPEND_TIMEOUT_SECONDS,
            ),
            MicrophoneSuspensionReadiness::Failed
        );
        abandon_microphone_suspension(&mut state, Some(&microphone));
        assert!(matches!(commands.try_recv(), Ok(MicrophoneCommand::Resume)));
    }

    #[test]
    fn malformed_audio_is_rejected_before_bevy_decodes_it() {
        assert!(!valid_wav(b"not a wave"));
        assert!(valid_wav(b"RIFF\0\0\0\0WAVE"));
    }

    #[test]
    fn wrapping_preserves_dialogue_as_plain_text() {
        assert_eq!(wrap_dialogue("one two three", 7), "one two\nthree");
        assert_eq!(wrap_dialogue("<b>literal</b>", 42), "<b>literal</b>");
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
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SpeechPresentationState>()
            .init_resource::<SmartActorHudState>()
            .add_message::<PresentSpeech>()
            .add_systems(Update, receive_speech_events);
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

        advance_subtitle_queue(&mut state, 9.9);
        assert_eq!(state.subtitles.len(), 1);
        advance_subtitle_queue(&mut state, 10.1);
        assert!(state.subtitles.is_empty());
    }
}
