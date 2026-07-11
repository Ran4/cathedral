//! Optional, continuously armed microphone capture with voice activation.
//!
//! The CPAL device and WAV writer live on a dedicated worker. The real-time
//! callback only converts a device buffer and tries to copy it into a bounded
//! channel; it never performs disk, protocol, or Bevy work.

use std::{
    collections::VecDeque,
    path::{Component, Path, PathBuf},
    thread::{self, JoinHandle},
    time::Duration,
};

use bevy::prelude::Resource;
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded, select};

use super::PLAYER_SPEECH_MAX_SECONDS;

const AUDIO_BUFFER_COUNT: usize = 64;
const WORKER_SHUTDOWN_GRACE: Duration = Duration::from_millis(300);
const PRE_ROLL: Duration = Duration::from_millis(250);
const START_CONFIRMATION: Duration = Duration::from_millis(80);
const MINIMUM_VOICE: Duration = Duration::from_millis(140);
const TRAILING_SILENCE: Duration = Duration::from_millis(700);
const INITIAL_NOISE_RMS: f32 = 0.0015;
const MINIMUM_START_RMS: f32 = 0.006;
const MINIMUM_START_PEAK: f32 = 0.035;
const MINIMUM_CONTINUE_RMS: f32 = 0.0035;
const MINIMUM_CONTINUE_PEAK: f32 = 0.030;
// A single moderately noisy device callback must not make ordinary speech
// unreachable. Keep learning the actual floor, but cap how far that estimate
// may raise the detector. The absolute RMS/peak gates and sustained
// confirmation still reject steady room ambience and brief impulses.
const MAXIMUM_START_NOISE_RMS: f32 = 0.003;
const MAXIMUM_CONTINUE_NOISE_RMS: f32 = 0.006;
const CONFIDENT_UNCALIBRATED_RMS: f32 = 0.012;
const CONFIDENT_UNCALIBRATED_PEAK: f32 = 0.040;

#[derive(Debug)]
pub enum MicrophoneCommand {
    /// Persistently enable voice-activated player input.
    Enable,
    /// Persistently disable player input until the next `Enable`.
    Disable,
    /// Temporarily stop capture, without changing the player's preference.
    /// Used while synthesized NPC speech is playing to avoid feedback.
    Suspend {
        acknowledged: Sender<()>,
    },
    /// Remove a temporary suspension. This only rearms an enabled microphone.
    Resume,
    Discard {
        wav_basename: String,
    },
}

#[derive(Debug, Clone)]
pub enum MicrophoneEvent {
    Available,
    Unavailable(String),
    RecordingStarted { wav_basename: String },
    RecordingFinished { wav_basename: String, silent: bool },
    RecordingCancelled { wav_basename: String },
    RecordingFailed(String),
}

pub enum MicrophonePoll {
    Event(MicrophoneEvent),
    Empty,
    Disconnected,
}

/// Handle used by Bevy systems. Sending and polling are always non-blocking.
#[derive(Resource)]
pub struct MicrophoneService {
    commands: Sender<MicrophoneCommand>,
    events: Receiver<MicrophoneEvent>,
    shutdown: Sender<()>,
    stopped: Receiver<()>,
    worker: Option<JoinHandle<()>>,
    cleanup_dir: Option<PathBuf>,
}

impl MicrophoneService {
    pub fn spawn(runtime_dir: PathBuf) -> Self {
        let cleanup_dir = runtime_dir.clone();
        let (commands_tx, commands_rx) = bounded(8);
        let (events_tx, events_rx) = bounded(16);
        let (shutdown_tx, shutdown_rx) = bounded(1);
        let (stopped_tx, stopped_rx) = bounded(1);
        let worker_events = events_tx.clone();
        let worker = match thread::Builder::new()
            .name("smart-actor-microphone".into())
            .spawn(move || {
                microphone_worker(runtime_dir, commands_rx, shutdown_rx, worker_events);
                let _ = stopped_tx.try_send(());
            }) {
            Ok(worker) => Some(worker),
            Err(error) => {
                let _ = events_tx.try_send(MicrophoneEvent::Unavailable(format!(
                    "could not start microphone worker: {error}"
                )));
                None
            }
        };

        Self {
            commands: commands_tx,
            events: events_rx,
            shutdown: shutdown_tx,
            stopped: stopped_rx,
            worker,
            cleanup_dir: Some(cleanup_dir),
        }
    }

    #[cfg(test)]
    pub fn unavailable_for_tests() -> Self {
        let (commands, _) = bounded(1);
        let (events_tx, events) = bounded(1);
        let _ = events_tx.try_send(MicrophoneEvent::Unavailable(
            "no input device (test)".into(),
        ));
        Self {
            commands,
            events,
            shutdown: bounded(1).0,
            stopped: bounded(1).1,
            worker: None,
            cleanup_dir: None,
        }
    }

    #[cfg(test)]
    pub fn command_harness_for_tests() -> (Self, Receiver<MicrophoneCommand>) {
        let (commands, received_commands) = bounded(8);
        let (_events_tx, events) = bounded(1);
        (
            Self {
                commands,
                events,
                shutdown: bounded(1).0,
                stopped: bounded(1).1,
                worker: None,
                cleanup_dir: None,
            },
            received_commands,
        )
    }

    pub fn try_send(&self, command: MicrophoneCommand) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => "microphone command queue is busy".into(),
                TrySendError::Disconnected(_) => "microphone worker is unavailable".into(),
            })
    }

    /// Relinquish a completed recording. If the bounded worker queue cannot
    /// accept the cleanup command, remove the confined file directly so a
    /// resync or bridge failure cannot accumulate orphaned utterances.
    pub fn discard_recording(&self, wav_basename: String) -> Result<(), String> {
        let delivery_error = match self.try_send(MicrophoneCommand::Discard {
            wav_basename: wav_basename.clone(),
        }) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        let Some(runtime_dir) = self.cleanup_dir.as_deref() else {
            return Err(delivery_error);
        };
        remove_recording_checked(runtime_dir, &wav_basename).map_err(|cleanup_error| {
            format!("{delivery_error}; fallback cleanup failed: {cleanup_error}")
        })
    }

    pub fn poll(&self) -> MicrophonePoll {
        match self.events.try_recv() {
            Ok(event) => MicrophonePoll::Event(event),
            Err(crossbeam_channel::TryRecvError::Empty) => MicrophonePoll::Empty,
            Err(crossbeam_channel::TryRecvError::Disconnected) => MicrophonePoll::Disconnected,
        }
    }
}

impl Drop for MicrophoneService {
    fn drop(&mut self) {
        let _ = self.shutdown.try_send(());
        // Dropping a JoinHandle detaches a driver call that did not return
        // inside the bounded grace period; app shutdown must not freeze.
        if let Some(worker) = self.worker.take()
            && self.stopped.recv_timeout(WORKER_SHUTDOWN_GRACE).is_ok()
        {
            let _ = worker.join();
        }
    }
}

fn microphone_worker(
    runtime_dir: PathBuf,
    commands: Receiver<MicrophoneCommand>,
    shutdown: Receiver<()>,
    events: Sender<MicrophoneEvent>,
) {
    let mut available = false;
    let mut enabled = false;
    let mut suspended = false;
    let mut next_recording = 1_u64;

    loop {
        let command = select! {
            recv(shutdown) -> _ => break,
            recv(commands) -> command => match command {
                Ok(command) => command,
                Err(_) => break,
            },
        };
        match command {
            MicrophoneCommand::Enable => enabled = true,
            MicrophoneCommand::Disable => enabled = false,
            MicrophoneCommand::Suspend { acknowledged } => {
                suspended = true;
                let _ = acknowledged.try_send(());
            }
            MicrophoneCommand::Resume => suspended = false,
            MicrophoneCommand::Discard { wav_basename } => {
                remove_recording(&runtime_dir, &wav_basename);
            }
        }

        if enabled && !suspended && !available {
            match probe_microphone() {
                Ok(()) => {
                    available = true;
                    if events.send(MicrophoneEvent::Available).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    enabled = false;
                    if events.send(MicrophoneEvent::Unavailable(error)).is_err() {
                        return;
                    }
                }
            }
        }

        while available && enabled && !suspended {
            match listen_for_utterances(
                &runtime_dir,
                &commands,
                &shutdown,
                &events,
                &mut next_recording,
            ) {
                ListenOutcome::Disabled => enabled = false,
                ListenOutcome::Suspended(acknowledgements) => {
                    suspended = true;
                    for acknowledged in acknowledgements {
                        let _ = acknowledged.try_send(());
                    }
                }
                ListenOutcome::Shutdown => return,
                ListenOutcome::Failed => {
                    // Opening or retaining the input stream failed. Wait for a
                    // fresh user toggle instead of retrying in a hot loop.
                    enabled = false;
                    available = false;
                }
            }
        }
    }
}

fn probe_microphone() -> Result<(), String> {
    cpal::default_host()
        .default_input_device()
        .ok_or_else(|| "no default microphone was found".to_string())?
        .default_input_config()
        .map(|_| ())
        .map_err(|error| format!("microphone configuration failed: {error}"))
}

#[derive(Debug)]
enum ListenOutcome {
    Disabled,
    Suspended(Vec<Sender<()>>),
    Shutdown,
    Failed,
}

fn listen_for_utterances(
    runtime_dir: &Path,
    commands: &Receiver<MicrophoneCommand>,
    shutdown: &Receiver<()>,
    events: &Sender<MicrophoneEvent>,
    next_recording: &mut u64,
) -> ListenOutcome {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        let _ = events.send(MicrophoneEvent::Unavailable(
            "no default microphone was found".into(),
        ));
        return ListenOutcome::Failed;
    };
    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            let _ = events.send(MicrophoneEvent::Unavailable(format!(
                "microphone configuration failed: {error}"
            )));
            return ListenOutcome::Failed;
        }
    };

    let spec = hound::WavSpec {
        // Speech models consume mono waveforms. The input callback downmixes
        // the device's native channels before samples reach this worker.
        channels: 1,
        sample_rate: config.sample_rate(),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let (audio_tx, audio_rx) = bounded(AUDIO_BUFFER_COUNT);
    let (error_tx, error_rx) = bounded(1);
    let stream = match build_input_stream(&device, &config, audio_tx, error_tx) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = events.send(MicrophoneEvent::Unavailable(error));
            return ListenOutcome::Failed;
        }
    };
    if let Err(error) = stream.play() {
        let _ = events.send(MicrophoneEvent::Unavailable(format!(
            "could not start microphone: {error}"
        )));
        return ListenOutcome::Failed;
    }

    let sample_rate = u64::from(config.sample_rate());
    let pre_roll_limit = duration_frames(PRE_ROLL, sample_rate)
        .try_into()
        .unwrap_or(usize::MAX);
    let max_samples = sample_rate.saturating_mul(u64::from(PLAYER_SPEECH_MAX_SECONDS));
    let trailing_silence_frames = duration_frames(TRAILING_SILENCE, sample_rate);
    let minimum_voice_frames = duration_frames(MINIMUM_VOICE, sample_rate);
    let mut pre_roll = VecDeque::with_capacity(pre_roll_limit);
    let mut detector = VoiceActivityDetector::new(sample_rate);
    let mut recording: Option<ActiveRecording> = None;
    loop {
        select! {
            recv(shutdown) -> _ => {
                cancel_recording(&mut recording, events);
                return ListenOutcome::Shutdown;
            },
            recv(commands) -> command => match command {
                Ok(MicrophoneCommand::Disable) => {
                    cancel_recording(&mut recording, events);
                    return ListenOutcome::Disabled;
                }
                Ok(MicrophoneCommand::Suspend { acknowledged }) => {
                    // Once an NPC reply is ready, any simultaneous capture is
                    // either a stale VAD candidate or a new utterance racing
                    // that reply. Cancel it so output cannot feed back into an
                    // open input stream and acknowledge suspension promptly.
                    if recording.is_some() {
                        println!(
                            "[smart actors/audio] cancelling active microphone recording before NPC playback"
                        );
                        cancel_recording(&mut recording, events);
                    }
                    return ListenOutcome::Suspended(vec![acknowledged]);
                }
                Err(_) => {
                    cancel_recording(&mut recording, events);
                    return ListenOutcome::Shutdown;
                }
                Ok(MicrophoneCommand::Discard { wav_basename }) => {
                    if recording
                        .as_ref()
                        .is_some_and(|active| active.wav_basename == wav_basename)
                    {
                        cancel_recording(&mut recording, events);
                    } else {
                        remove_recording(runtime_dir, &wav_basename);
                    }
                }
                Ok(MicrophoneCommand::Resume) => {}
                Ok(MicrophoneCommand::Enable) => {}
            },
            recv(error_rx) -> error => {
                let error = error.unwrap_or_else(|_| "microphone error channel disconnected".into());
                fail_recording(&mut recording, events, error.clone());
                let _ = events.send(MicrophoneEvent::Unavailable(error));
                return ListenOutcome::Failed;
            },
            recv(audio_rx) -> captured => match captured {
                Ok(samples) => {
                    let levels = AudioLevels::from_samples(&samples);
                    let frames = samples.len() as u64;
                    if let Some(active) = recording.as_mut() {
                        let voice = detector.continues_voice(levels);
                        if voice {
                            active.silent_frames = 0;
                            active.voiced_frames = active.voiced_frames.saturating_add(frames);
                        } else {
                            active.silent_frames = active.silent_frames.saturating_add(frames);
                        }

                        let remaining = max_samples.saturating_sub(active.sample_count);
                        let write_len = samples.len().min(remaining.try_into().unwrap_or(usize::MAX));
                        if let Err(error) = write_samples(&mut active.writer, &samples[..write_len]) {
                            fail_recording(&mut recording, events, error);
                            return ListenOutcome::Failed;
                        }
                        active.sample_count = active.sample_count.saturating_add(write_len as u64);

                        if active.sample_count >= max_samples
                            || active.silent_frames >= trailing_silence_frames
                        {
                            let active = recording.take().expect("recording was just borrowed");
                            if !finish_recording(
                                active,
                                events,
                                minimum_voice_frames,
                            ) {
                                return ListenOutcome::Failed;
                            }
                            detector.reset_candidate();
                            pre_roll.clear();
                        }
                    } else {
                        push_pre_roll(&mut pre_roll, &samples, pre_roll_limit);
                        if let Some(confirmed_voice_frames) =
                            detector.observe_idle(levels, frames)
                        {
                            match begin_recording(
                                runtime_dir,
                                spec,
                                next_recording,
                                &pre_roll,
                                confirmed_voice_frames,
                            ) {
                                Ok(active) => {
                                    let started = MicrophoneEvent::RecordingStarted {
                                        wav_basename: active.wav_basename.clone(),
                                    };
                                    if events.send(started).is_ok() {
                                        recording = Some(active);
                                    } else {
                                        let path = active.path.clone();
                                        drop(active);
                                        let _ = std::fs::remove_file(path);
                                        return ListenOutcome::Shutdown;
                                    }
                                    pre_roll.clear();
                                }
                                Err(error) => {
                                    let _ = events.send(MicrophoneEvent::RecordingFailed(error));
                                    return ListenOutcome::Failed;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    let error = "microphone stream disconnected".to_string();
                    fail_recording(&mut recording, events, error.clone());
                    let _ = events.send(MicrophoneEvent::Unavailable(error));
                    return ListenOutcome::Failed;
                }
            },
        }
    }
}

struct ActiveRecording {
    wav_basename: String,
    path: PathBuf,
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    sample_count: u64,
    voiced_frames: u64,
    silent_frames: u64,
}

fn begin_recording(
    runtime_dir: &Path,
    spec: hound::WavSpec,
    next_recording: &mut u64,
    pre_roll: &VecDeque<f32>,
    confirmed_voice_frames: u64,
) -> Result<ActiveRecording, String> {
    let wav_basename = format!("player-recording-{}.wav", *next_recording);
    *next_recording = next_recording.wrapping_add(1).max(1);
    let path = safe_audio_path(runtime_dir, &wav_basename)
        .ok_or_else(|| "generated an invalid recording filename".to_string())?;
    let mut writer = hound::WavWriter::create(&path, spec)
        .map_err(|error| format!("could not create recording: {error}"))?;
    for sample in pre_roll {
        if let Err(error) = writer.write_sample(*sample) {
            drop(writer);
            let _ = std::fs::remove_file(&path);
            return Err(format!("could not write microphone samples: {error}"));
        }
    }
    Ok(ActiveRecording {
        wav_basename,
        path,
        writer,
        sample_count: pre_roll.len() as u64,
        voiced_frames: confirmed_voice_frames,
        silent_frames: 0,
    })
}

fn finish_recording(
    active: ActiveRecording,
    events: &Sender<MicrophoneEvent>,
    minimum_voice_frames: u64,
) -> bool {
    let ActiveRecording {
        wav_basename,
        path,
        writer,
        voiced_frames,
        ..
    } = active;
    let silent = voiced_frames < minimum_voice_frames;
    if let Err(error) = writer.finalize() {
        let _ = std::fs::remove_file(&path);
        let _ = events.send(MicrophoneEvent::RecordingFailed(format!(
            "could not finish microphone recording: {error}"
        )));
        return false;
    }
    if silent {
        let _ = std::fs::remove_file(&path);
    }
    let finished = MicrophoneEvent::RecordingFinished {
        wav_basename,
        silent,
    };
    if events.send(finished).is_err() && !silent {
        // A recording no longer has an owner if Bevy cannot learn about it.
        let _ = std::fs::remove_file(path);
    }
    true
}

fn cancel_recording(recording: &mut Option<ActiveRecording>, events: &Sender<MicrophoneEvent>) {
    let Some(active) = recording.take() else {
        return;
    };
    let wav_basename = active.wav_basename.clone();
    let path = active.path.clone();
    drop(active);
    let _ = std::fs::remove_file(path);
    let _ = events.send(MicrophoneEvent::RecordingCancelled { wav_basename });
}

fn fail_recording(
    recording: &mut Option<ActiveRecording>,
    events: &Sender<MicrophoneEvent>,
    error: String,
) {
    if let Some(active) = recording.take() {
        let path = active.path.clone();
        drop(active);
        let _ = std::fs::remove_file(path);
    }
    let _ = events.send(MicrophoneEvent::RecordingFailed(error));
}

fn remove_recording(runtime_dir: &Path, wav_basename: &str) {
    let _ = remove_recording_checked(runtime_dir, wav_basename);
}

fn remove_recording_checked(runtime_dir: &Path, wav_basename: &str) -> Result<(), String> {
    let path = safe_audio_path(runtime_dir, wav_basename)
        .ok_or_else(|| "recording filename is invalid".to_string())?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not remove recording: {error}")),
    }
}

fn write_samples(
    writer: &mut hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    samples: &[f32],
) -> Result<(), String> {
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|error| format!("could not write microphone samples: {error}"))?;
    }
    Ok(())
}

fn push_pre_roll(buffer: &mut VecDeque<f32>, samples: &[f32], limit: usize) {
    if limit == 0 {
        return;
    }
    if samples.len() >= limit {
        buffer.clear();
        buffer.extend(samples[samples.len() - limit..].iter().copied());
        return;
    }
    while buffer.len() + samples.len() > limit {
        buffer.pop_front();
    }
    buffer.extend(samples.iter().copied());
}

fn duration_frames(duration: Duration, sample_rate: u64) -> u64 {
    duration
        .as_secs()
        .saturating_mul(sample_rate)
        .saturating_add(
            u64::from(duration.subsec_nanos()).saturating_mul(sample_rate) / 1_000_000_000,
        )
}

#[derive(Debug, Clone, Copy)]
struct AudioLevels {
    peak: f32,
    rms: f32,
}

impl AudioLevels {
    fn from_samples(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self {
                peak: 0.0,
                rms: 0.0,
            };
        }
        let mut peak = 0.0_f32;
        let mut square_sum = 0.0_f64;
        for sample in samples.iter().copied().filter(|sample| sample.is_finite()) {
            peak = peak.max(sample.abs());
            square_sum += f64::from(sample) * f64::from(sample);
        }
        Self {
            peak,
            rms: (square_sum / samples.len() as f64).sqrt() as f32,
        }
    }
}

struct VoiceActivityDetector {
    noise_rms: f32,
    baseline_established: bool,
    candidate_frames: u64,
    confirmation_frames: u64,
}

impl VoiceActivityDetector {
    fn new(sample_rate: u64) -> Self {
        Self {
            noise_rms: INITIAL_NOISE_RMS,
            baseline_established: false,
            candidate_frames: 0,
            confirmation_frames: duration_frames(START_CONFIRMATION, sample_rate),
        }
    }

    fn observe_idle(&mut self, levels: AudioLevels, frames: u64) -> Option<u64> {
        if !self.baseline_established {
            let confident_voice = levels.rms >= CONFIDENT_UNCALIBRATED_RMS
                || levels.peak >= CONFIDENT_UNCALIBRATED_PEAK;
            self.baseline_established = true;
            if !confident_voice {
                self.noise_rms = levels.rms.clamp(0.0001, 0.05);
                self.candidate_frames = 0;
                return None;
            }
        }
        let effective_noise_rms = self.noise_rms.min(MAXIMUM_START_NOISE_RMS);
        let starts_voice = levels.rms >= (effective_noise_rms * 4.0).max(MINIMUM_START_RMS)
            || levels.peak >= (effective_noise_rms * 8.0).max(MINIMUM_START_PEAK);
        if starts_voice {
            self.candidate_frames = self.candidate_frames.saturating_add(frames);
            if self.candidate_frames >= self.confirmation_frames {
                let confirmed = self.candidate_frames;
                self.candidate_frames = 0;
                return Some(confirmed);
            }
        } else {
            self.candidate_frames = 0;
            if levels.rms.is_finite() {
                self.noise_rms = (self.noise_rms * 0.98 + levels.rms * 0.02).clamp(0.0001, 0.05);
            }
        }
        None
    }

    fn continues_voice(&self, levels: AudioLevels) -> bool {
        let effective_noise_rms = self.noise_rms.min(MAXIMUM_CONTINUE_NOISE_RMS);
        levels.rms >= (effective_noise_rms * 2.0).max(MINIMUM_CONTINUE_RMS)
            || levels.peak >= (effective_noise_rms * 4.0).max(MINIMUM_CONTINUE_PEAK)
    }

    fn reset_candidate(&mut self) {
        self.candidate_frames = 0;
    }
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    captured: Sender<Vec<f32>>,
    errors: Sender<String>,
) -> Result<cpal::Stream, String> {
    macro_rules! stream {
        ($sample:ty) => {
            build_typed_input_stream::<$sample>(device, &config.clone().into(), captured, errors)
        };
    }

    match config.sample_format() {
        SampleFormat::I8 => stream!(i8),
        SampleFormat::I16 => stream!(i16),
        SampleFormat::I24 => stream!(cpal::I24),
        SampleFormat::I32 => stream!(i32),
        SampleFormat::I64 => stream!(i64),
        SampleFormat::U8 => stream!(u8),
        SampleFormat::U16 => stream!(u16),
        SampleFormat::U24 => stream!(cpal::U24),
        SampleFormat::U32 => stream!(u32),
        SampleFormat::U64 => stream!(u64),
        SampleFormat::F32 => stream!(f32),
        SampleFormat::F64 => stream!(f64),
        unsupported => Err(format!(
            "unsupported microphone sample format: {unsupported}"
        )),
    }
}

fn build_typed_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    captured: Sender<Vec<f32>>,
    errors: Sender<String>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Copy,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    device
        .build_input_stream(
            config,
            move |input: &[T], _| {
                let samples = downmix_to_mono(input, channels);
                let _ = captured.try_send(samples);
            },
            move |error| {
                let _ = errors.try_send(format!("microphone stream error: {error}"));
            },
            None,
        )
        .map_err(|error| format!("could not open microphone: {error}"))
}

fn downmix_to_mono<T>(input: &[T], channels: usize) -> Vec<f32>
where
    T: Copy,
    f32: FromSample<T>,
{
    if channels == 0 {
        return Vec::new();
    }
    input
        .chunks_exact(channels)
        .map(|frame| frame.iter().copied().map(f32::from_sample).sum::<f32>() / channels as f32)
        .collect()
}

fn safe_audio_path(runtime_dir: &Path, basename: &str) -> Option<PathBuf> {
    let candidate = Path::new(basename);
    if basename.is_empty()
        || basename.len() > 128
        || candidate.extension().and_then(|value| value.to_str()) != Some("wav")
        || candidate.components().count() != 1
        || !matches!(candidate.components().next(), Some(Component::Normal(_)))
    {
        return None;
    }
    Some(runtime_dir.join(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn audio_paths_are_confined_to_the_session_directory() {
        let runtime = Path::new("/tmp/smart-actors-test");
        assert_eq!(
            safe_audio_path(runtime, "recording-42.wav"),
            Some(runtime.join("recording-42.wav"))
        );
        for unsafe_name in ["", "../secret.wav", "/tmp/out.wav", "nested/a.wav", "a.mp3"] {
            assert!(
                safe_audio_path(runtime, unsafe_name).is_none(),
                "{unsafe_name}"
            );
        }
    }

    #[test]
    fn absent_microphone_is_a_nonfatal_capability_state() {
        let service = MicrophoneService::unavailable_for_tests();
        assert!(matches!(
            service.poll(),
            MicrophonePoll::Event(MicrophoneEvent::Unavailable(_))
        ));
    }

    #[test]
    fn discard_falls_back_to_confined_file_cleanup_when_command_queue_is_full() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let runtime_dir = std::env::temp_dir().join(format!(
            "cathedralbevy-microphone-cleanup-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let wav_basename = "orphan.wav";
        let wav_path = runtime_dir.join(wav_basename);
        std::fs::write(&wav_path, b"RIFF-test").unwrap();

        let (commands, _received_commands) = bounded(1);
        commands.try_send(MicrophoneCommand::Enable).unwrap();
        let (_events_tx, events) = bounded(1);
        let (shutdown, _shutdown_rx) = bounded(1);
        let (_stopped_tx, stopped) = bounded(1);
        let service = MicrophoneService {
            commands,
            events,
            shutdown,
            stopped,
            worker: None,
            cleanup_dir: Some(runtime_dir.clone()),
        };

        service
            .discard_recording(wav_basename.into())
            .expect("direct cleanup should recover from a full command queue");
        assert!(!wav_path.exists());
        drop(service);
        std::fs::remove_dir(runtime_dir).unwrap();
    }

    #[test]
    fn service_drop_is_bounded_when_a_driver_worker_does_not_stop() {
        let (commands, _received_commands) = bounded(1);
        let (_events_tx, events) = bounded(1);
        let (shutdown, _shutdown_rx) = bounded(1);
        let (_stopped_tx, stopped) = bounded(1);
        let (release, wait_for_release) = bounded::<()>(1);
        let worker = thread::spawn(move || {
            let _ = wait_for_release.recv();
        });
        let service = MicrophoneService {
            commands,
            events,
            shutdown,
            stopped,
            worker: Some(worker),
            cleanup_dir: None,
        };

        let started = Instant::now();
        drop(service);
        assert!(started.elapsed() < Duration::from_secs(1));
        let _ = release.try_send(());
    }

    #[test]
    fn vad_ignores_noise_then_requires_sustained_voice() {
        let mut detector = VoiceActivityDetector::new(1_000);
        let quiet = AudioLevels {
            peak: 0.004,
            rms: 0.001,
        };
        let voice = AudioLevels {
            peak: 0.08,
            rms: 0.03,
        };

        assert_eq!(detector.observe_idle(quiet, 100), None);
        assert_eq!(detector.observe_idle(voice, 40), None);
        assert_eq!(detector.observe_idle(voice, 39), None);
        assert_eq!(detector.observe_idle(voice, 1), Some(80));
        assert!(detector.continues_voice(voice));
        assert!(!detector.continues_voice(quiet));
    }

    #[test]
    fn first_moderate_noise_buffer_cannot_hide_ordinary_voice() {
        let mut detector = VoiceActivityDetector::new(1_000);
        let moderate_startup_noise = AudioLevels {
            peak: 0.027,
            rms: 0.009,
        };
        let ordinary_voice = AudioLevels {
            peak: 0.032,
            rms: 0.014,
        };

        assert_eq!(detector.observe_idle(moderate_startup_noise, 100), None);
        assert_eq!(detector.observe_idle(ordinary_voice, 79), None);
        assert_eq!(detector.observe_idle(ordinary_voice, 1), Some(80));
        assert!(detector.continues_voice(ordinary_voice));
    }

    #[test]
    fn quiet_baseline_still_detects_quiet_sustained_speech() {
        let mut detector = VoiceActivityDetector::new(1_000);
        let quiet = AudioLevels {
            peak: 0.001,
            rms: 0.0005,
        };
        let quiet_voice = AudioLevels {
            peak: 0.020,
            rms: 0.007,
        };

        for _ in 0..5 {
            assert_eq!(detector.observe_idle(quiet, 20), None);
        }
        assert_eq!(detector.observe_idle(quiet_voice, 79), None);
        assert_eq!(detector.observe_idle(quiet_voice, 1), Some(80));
        assert!(detector.continues_voice(quiet_voice));
    }

    #[test]
    fn interrupted_voice_candidate_does_not_trigger_an_utterance() {
        let mut detector = VoiceActivityDetector::new(1_000);
        let voice = AudioLevels {
            peak: 0.08,
            rms: 0.03,
        };
        let quiet = AudioLevels {
            peak: 0.002,
            rms: 0.001,
        };

        assert_eq!(detector.observe_idle(voice, 79), None);
        assert_eq!(detector.observe_idle(quiet, 10), None);
        assert_eq!(detector.observe_idle(voice, 1), None);
    }

    #[test]
    fn steady_startup_ambience_becomes_the_noise_floor_not_endless_speech() {
        let mut detector = VoiceActivityDetector::new(1_000);
        let ambience = AudioLevels {
            peak: 0.025,
            rms: 0.008,
        };
        for _ in 0..20 {
            assert_eq!(detector.observe_idle(ambience, 100), None);
        }

        let voice = AudioLevels {
            peak: 0.10,
            rms: 0.04,
        };
        assert_eq!(detector.observe_idle(voice, 79), None);
        assert_eq!(detector.observe_idle(voice, 1), Some(80));
    }

    #[test]
    fn pre_roll_is_bounded_and_retains_the_newest_samples() {
        let mut samples = VecDeque::new();
        push_pre_roll(&mut samples, &[1.0, 2.0, 3.0], 4);
        push_pre_roll(&mut samples, &[4.0, 5.0], 4);
        assert_eq!(
            samples.into_iter().collect::<Vec<_>>(),
            [2.0, 3.0, 4.0, 5.0]
        );
    }

    #[test]
    fn microphone_channels_are_downmixed_to_mono_frames() {
        assert_eq!(
            downmix_to_mono(&[0.5_f32, -0.25, 1.0, 0.0], 2),
            [0.125, 0.5]
        );
        assert_eq!(downmix_to_mono(&[0.25_f32, -0.5], 1), [0.25, -0.5]);
    }

    #[test]
    fn duration_frame_conversion_is_exact_for_vad_boundaries() {
        assert_eq!(duration_frames(Duration::from_millis(250), 48_000), 12_000);
        assert_eq!(duration_frames(Duration::from_millis(700), 44_100), 30_870);
    }
}
