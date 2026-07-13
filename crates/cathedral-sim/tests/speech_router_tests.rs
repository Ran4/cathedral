//! The speech router (`test_protocol_server.py` SPEECH rows 18-57, 63-67, 69-70).
//!
//! Python drove these through a whole stdio server with a monkeypatched clock and
//! monkeypatched speech backends. Here the clock is a field and the backends are
//! probes, so every assertion is about the state machine itself.
//!
//! The invariant behind almost every test in this file: **the microphone may
//! fail in any way it likes, and the player must still be heard**. A wrong sample
//! rate, a gap in the chunk sequence, a dead websocket, a full queue — each one
//! only decides *which road* the utterance takes to the transcript, never whether
//! it arrives. The second invariant is its mirror: nothing the microphone or the
//! voice backend does may leave the conversation floor held forever.

mod prompt_support;

use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

use cathedral_sim::{
    ActorId, Capabilities, Engine, EngineCommand, EngineConfig, EngineMessage, FakeCognition,
    NullSight, RealtimeResult, SpeechError, SpeechEventId, StatusEvent, SttBackendKind,
    SttSubmitError, Subsystem, Transcription, TranscriptionJobId, TranscriptionOutcome, Tts,
    TtsBackendKind, TtsOutcome, TtsRequest, TtsSubmitError, Vec3, WorldSeed, apply_action,
    speech_reading_seconds,
};
use prompt_support::{asset, catalog, prompt_env};
use serde_json::json;

// ---------------------------------------------------------------- test doubles

/// Everything the router asked a transcription backend to do.
#[derive(Default)]
struct SpeechState {
    stt_cloud: bool,
    stt_local: bool,
    /// What the next `submit_batch` returns.
    submit_error: Option<SttSubmitError>,
    batch: Vec<(TranscriptionJobId, PathBuf, SttBackendKind)>,
    realtime_begin_ok: bool,
    realtime_commit_ok: bool,
    /// `begin:key`, `append:key`, `commit:key`, `clear:key` in order.
    calls: Vec<String>,
    /// What the backend says the recording's WAV is worth in seconds.
    audio_seconds: Option<f64>,
    /// Every WAV the router asked the backend to delete.
    discarded: Vec<PathBuf>,
}

#[derive(Clone, Default)]
struct SpeechProbe(Rc<RefCell<SpeechState>>);

impl SpeechProbe {
    /// A working cloud pipeline: a realtime session that accepts everything, and
    /// a batch backend behind it.
    fn cloud() -> Self {
        let probe = Self::default();
        {
            let mut state = probe.0.borrow_mut();
            state.stt_cloud = true;
            state.realtime_begin_ok = true;
            state.realtime_commit_ok = true;
        }
        probe
    }

    /// Cloud batch only — the configuration with no `OPENAI_API_KEY` for the
    /// realtime endpoint, i.e. no session at all.
    fn batch_only() -> Self {
        let probe = Self::cloud();
        probe.0.borrow_mut().realtime_begin_ok = false;
        probe
    }

    fn with_local(self) -> Self {
        self.0.borrow_mut().stt_local = true;
        self
    }

    fn refusing(error: SttSubmitError) -> Self {
        let probe = Self::cloud();
        probe.0.borrow_mut().submit_error = Some(error);
        probe
    }

    fn batch_jobs(&self) -> Vec<(TranscriptionJobId, PathBuf, SttBackendKind)> {
        self.0.borrow().batch.clone()
    }

    /// The job id of the n-th `submit_batch`, for feeding the outcome back.
    fn job(&self, index: usize) -> TranscriptionJobId {
        self.batch_jobs()[index].0
    }

    fn calls(&self) -> Vec<String> {
        self.0.borrow().calls.clone()
    }

    /// A backend that can measure the recording it was handed.
    fn measuring(seconds: f64) -> Self {
        let probe = Self::cloud();
        probe.0.borrow_mut().audio_seconds = Some(seconds);
        probe
    }

    fn discarded(&self) -> Vec<PathBuf> {
        self.0.borrow().discarded.clone()
    }
}

impl Transcription for SpeechProbe {
    fn available(&self, kind: SttBackendKind) -> bool {
        let state = self.0.borrow();
        match kind {
            SttBackendKind::Cloud => state.stt_cloud,
            SttBackendKind::Local => state.stt_local,
        }
    }

    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        wav_path: PathBuf,
        kind: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        let mut state = self.0.borrow_mut();
        if let Some(error) = state.submit_error {
            return Err(error);
        }
        state.batch.push((job, wav_path, kind));
        Ok(())
    }

    fn realtime_begin(&mut self, key: &str) -> bool {
        let mut state = self.0.borrow_mut();
        state.calls.push(format!("begin:{key}"));
        state.realtime_begin_ok
    }

    fn realtime_append(&mut self, key: &str, _samples: &[i16]) -> bool {
        let mut state = self.0.borrow_mut();
        state.calls.push(format!("append:{key}"));
        state.realtime_begin_ok
    }

    fn realtime_commit(&mut self, key: &str) -> bool {
        let mut state = self.0.borrow_mut();
        state.calls.push(format!("commit:{key}"));
        state.realtime_commit_ok
    }

    fn realtime_clear(&mut self, key: &str) {
        self.0.borrow_mut().calls.push(format!("clear:{key}"));
    }

    fn recording_seconds(&self, _wav_path: &std::path::Path) -> Option<f64> {
        self.0.borrow().audio_seconds
    }

    fn discard_recording(&mut self, wav_path: &std::path::Path) {
        self.0.borrow_mut().discarded.push(wav_path.to_path_buf());
    }
}

#[derive(Default)]
struct VoiceState {
    available: bool,
    refuse: Option<TtsSubmitError>,
    submitted: Vec<TtsRequest>,
}

#[derive(Clone, Default)]
struct VoiceProbe(Rc<RefCell<VoiceState>>);

impl VoiceProbe {
    fn available() -> Self {
        let probe = Self::default();
        probe.0.borrow_mut().available = true;
        probe
    }

    fn refusing(error: TtsSubmitError) -> Self {
        let probe = Self::available();
        probe.0.borrow_mut().refuse = Some(error);
        probe
    }

    fn submitted(&self) -> Vec<TtsRequest> {
        self.0.borrow().submitted.clone()
    }
}

impl Tts for VoiceProbe {
    fn available(&self, kind: TtsBackendKind) -> bool {
        self.0.borrow().available && kind != TtsBackendKind::Off
    }

    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError> {
        let mut state = self.0.borrow_mut();
        if let Some(error) = state.refuse {
            return Err(error);
        }
        state.submitted.push(request);
        Ok(())
    }

    fn warm(&mut self, _kind: TtsBackendKind) {}
}

// -------------------------------------------------------------------- harness

/// Inside fart range of all three NPCs; nearest LLM hearer is Conny (1 m away).
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.91, 111.0);
const WAV: &str = "player-recording-1.wav";

struct Harness {
    engine: Engine,
    speech: SpeechProbe,
    now: f64,
    spatial_seq: i64,
}

struct Builder {
    fake_mode: bool,
    speech: SpeechProbe,
    tts: VoiceProbe,
    tts_selected: TtsBackendKind,
    grace_seconds: f64,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            fake_mode: false,
            speech: SpeechProbe::default(),
            tts: VoiceProbe::default(),
            tts_selected: TtsBackendKind::Off,
            grace_seconds: 2.0,
        }
    }
}

impl Builder {
    fn speech(mut self, speech: SpeechProbe) -> Self {
        self.speech = speech;
        self
    }

    fn fake(mut self) -> Self {
        self.fake_mode = true;
        self
    }

    fn voices(mut self, selected: TtsBackendKind, tts: VoiceProbe) -> Self {
        self.tts_selected = selected;
        self.tts = tts;
        self
    }

    fn grace(mut self, seconds: f64) -> Self {
        self.grace_seconds = seconds;
        self
    }

    fn build(self) -> Harness {
        let capabilities = Capabilities::new(
            // No cognition: nothing but the test moves the world, so every
            // message in `out` is the router's.
            false,
            self.speech.available(SttBackendKind::Cloud),
            self.speech.available(SttBackendKind::Local),
            false,
            false,
            self.tts_selected,
        );
        let engine = Engine::new(
            EngineConfig {
                fake_mode: self.fake_mode,
                turn_delay_seconds: 0.0,
                tts_selected: self.tts_selected,
                stt_stream_grace_seconds: self.grace_seconds,
                ..EngineConfig::default()
            },
            &seed(),
            catalog(),
            prompt_env(),
            Box::new(FakeCognition::default()),
            Box::new(self.speech.clone()),
            Box::new(self.tts.clone()),
            Box::new(NullSight),
            capabilities,
            (PLAYER_SPAWN, 0.0),
            0,
            0.0,
        )
        .expect("the seeded world has a player");
        let mut harness = Harness {
            engine,
            speech: self.speech,
            now: 0.0,
            spatial_seq: 0,
        };
        harness.poll(); // swallow `Ready`
        harness
    }
}

impl Harness {
    fn poll(&mut self) -> Vec<EngineMessage> {
        self.engine.poll(self.now, Vec::new())
    }

    fn send(&mut self, command: EngineCommand) -> Vec<EngineMessage> {
        self.engine.poll(self.now, vec![command])
    }

    fn advance(&mut self, seconds: f64) {
        self.now += seconds;
    }

    // ---- microphone shorthands

    fn begin(&mut self, rate: u32) -> Vec<EngineMessage> {
        self.send(EngineCommand::PlayerAudioBegin {
            wav_basename: WAV.into(),
            sample_rate: rate,
        })
    }

    fn chunk(&mut self, seq: u32) -> Vec<EngineMessage> {
        self.chunk_of(seq, 480)
    }

    fn chunk_of(&mut self, seq: u32, samples: usize) -> Vec<EngineMessage> {
        self.send(EngineCommand::PlayerAudioChunk {
            wav_basename: WAV.into(),
            seq,
            samples: Arc::from(vec![0i16; samples].into_boxed_slice()),
        })
    }

    fn end(&mut self, chunk_count: u32) -> Vec<EngineMessage> {
        self.send(EngineCommand::PlayerAudioEnd {
            wav_basename: WAV.into(),
            chunk_count,
            silent: false,
        })
    }

    fn recording(&mut self, backend: SttBackendKind) -> Vec<EngineMessage> {
        self.recording_at(backend, PLAYER_SPAWN)
    }

    fn recording_at(&mut self, backend: SttBackendKind, position_m: Vec3) -> Vec<EngineMessage> {
        self.spatial_seq += 1;
        self.send(EngineCommand::PlayerRecording {
            request_id: "rec-1".into(),
            wav_basename: WAV.into(),
            stt_backend: backend,
            position_m,
            spatial_seq: self.spatial_seq,
        })
    }

    /// One full streamed utterance: begin, two chunks, endpoint.
    fn stream_utterance(&mut self) {
        self.begin(24_000);
        self.chunk(0);
        self.chunk(1);
        self.end(2);
    }

    /// Hand a batch transcription back, as the host drains its channel.
    fn transcribed(&mut self, job: TranscriptionJobId, text: &str) -> Vec<EngineMessage> {
        self.send(EngineCommand::Transcription(TranscriptionOutcome::Done {
            job,
            result: Ok(text.to_string()),
        }))
    }

    fn transcription_failed(&mut self, job: TranscriptionJobId, why: &str) -> Vec<EngineMessage> {
        self.send(EngineCommand::Transcription(TranscriptionOutcome::Done {
            job,
            result: Err(SpeechError::new(why)),
        }))
    }

    fn realtime(&mut self, result: RealtimeResult) -> Vec<EngineMessage> {
        self.send(EngineCommand::Transcription(
            TranscriptionOutcome::Realtime(result),
        ))
    }

    fn npc(&mut self, actor: &str, verb: &str, args: serde_json::Value) {
        apply_action(
            self.engine.world_mut(),
            &ActorId::from_raw(actor),
            verb,
            &args,
        )
        .unwrap_or_else(|error| panic!("{actor} {verb}: {error}"));
    }
}

fn seed() -> WorldSeed {
    WorldSeed::from_json_str(&asset("world/seed.json")).expect("the shipped seed loads")
}

fn player() -> ActorId {
    ActorId::from_raw("player")
}

// ------------------------------------------------------------------- matchers

fn statuses(messages: &[EngineMessage]) -> Vec<&StatusEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            EngineMessage::Status(status) => Some(status),
            _ => None,
        })
        .collect()
}

/// The `stt degraded` messages — the stream-fallback notices.
fn degrades(messages: &[EngineMessage]) -> Vec<String> {
    statuses(messages)
        .into_iter()
        .filter(|status| status.subsystem == Subsystem::Stt && status.state == "degraded")
        .filter_map(|status| status.message.clone())
        .collect()
}

fn speeches(messages: &[EngineMessage]) -> Vec<&EngineMessage> {
    messages
        .iter()
        .filter(|message| matches!(message, EngineMessage::Speech { .. }))
        .collect()
}

fn command_results(messages: &[EngineMessage]) -> Vec<(bool, Option<String>, String)> {
    messages
        .iter()
        .filter_map(|message| match message {
            EngineMessage::CommandResult {
                success,
                error_code,
                message,
                ..
            } => Some((*success, error_code.clone(), message.clone())),
            _ => None,
        })
        .collect()
}

fn transcription_results(messages: &[EngineMessage]) -> Vec<(Option<String>, Option<String>)> {
    messages
        .iter()
        .filter_map(|message| match message {
            EngineMessage::TranscriptionResult { text, error, .. } => {
                Some((text.clone(), error.clone()))
            }
            _ => None,
        })
        .collect()
}

fn diagnostics(messages: &[EngineMessage]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            EngineMessage::Diagnostic(line) => Some(line.clone()),
            _ => None,
        })
        .collect()
}

/// The one `[smart actors/stt] …` latency line a resolved utterance logs.
fn timing_lines(messages: &[EngineMessage]) -> Vec<String> {
    diagnostics(messages)
        .into_iter()
        .filter(|line| line.starts_with("[smart actors/stt]"))
        .collect()
}

fn speech_event_id(messages: &[EngineMessage]) -> SpeechEventId {
    match speeches(messages).first().expect("a speech was emitted") {
        EngineMessage::Speech { event_id, .. } => event_id.clone(),
        _ => unreachable!(),
    }
}

// ======================================================= streamed player speech

/// 41. `test_streamed_utterance_resolves_through_result_pipeline`
///
/// Offline the batch backend *is* the provider: the utterance is transcribed at
/// its endpoint and the recording resolves from the held transcript — one
/// transcription for the whole utterance, and a timing line that says `stream`.
#[test]
fn a_streamed_utterance_resolves_from_its_held_transcript() {
    let mut harness = Builder::default()
        .fake()
        .speech(SpeechProbe::cloud())
        .build();

    harness.stream_utterance();
    assert_eq!(
        harness.speech.batch_jobs().len(),
        1,
        "the endpoint transcribes the stream"
    );
    let job = harness.speech.job(0);

    harness.advance(0.2);
    harness.transcribed(job, "What's your name?");

    harness.advance(0.1);
    let messages = harness.recording(SttBackendKind::Cloud);

    assert_eq!(
        harness.speech.batch_jobs().len(),
        1,
        "the held transcript is used; the WAV is never uploaded a second time"
    );
    assert_eq!(
        transcription_results(&messages),
        vec![(Some("What's your name?".to_string()), None)]
    );
    assert_eq!(speeches(&messages).len(), 1, "the player was heard");
    assert!(command_results(&messages)[0].0);

    let timing = timing_lines(&messages);
    assert_eq!(timing.len(), 1, "exactly one probe line per utterance");
    assert!(timing[0].contains("path=stream"), "{}", timing[0]);
    assert!(timing[0].contains("commit->transcript="), "{}", timing[0]);
    assert!(timing[0].contains("endpoint->say="), "{}", timing[0]);
}

/// Python unlinked the recording on **every** resolution path
/// (`_resolve_transcription`, `server.py:1594-1602`). The streamed roads — a held
/// transcript, and a parked recording the socket answers — hand the WAV to no
/// backend at all, so nothing else will ever delete it. With a key set, that is
/// the *normal* road: every utterance of a whole session would stay in `/tmp`.
#[test]
fn every_resolution_road_deletes_the_players_recording() {
    // Road one: the transcript was already held when the recording arrived.
    let mut harness = Builder::default()
        .fake()
        .speech(SpeechProbe::cloud())
        .build();
    harness.stream_utterance();
    let job = harness.speech.job(0);
    harness.transcribed(job, "What's your name?");
    let messages = harness.recording(SttBackendKind::Cloud);
    assert_eq!(speeches(&messages).len(), 1, "the player was heard");
    assert_eq!(
        harness
            .speech
            .discarded()
            .iter()
            .filter(|path| path.ends_with(WAV))
            .count(),
        1,
        "the WAV the batch pipeline never saw is deleted anyway"
    );

    // Road two: the recording parks, and the realtime socket answers it.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.stream_utterance();
    harness.recording(SttBackendKind::Cloud);
    assert!(
        harness.speech.discarded().is_empty(),
        "nothing is deleted while the utterance is still in flight"
    );
    let messages = harness.realtime(RealtimeResult::Transcript {
        key: WAV.to_string(),
        text: "Heard on the socket.".to_string(),
    });
    assert_eq!(speeches(&messages).len(), 1);
    assert_eq!(
        harness.speech.discarded().len(),
        1,
        "the parked recording goes with its transcript"
    );
    assert!(harness.speech.discarded()[0].ends_with(WAV));
}

/// The probe exists to normalise latency against utterance length, and Python
/// measured the WAV itself for every batch submission
/// (`_wav_duration_seconds` → `_begin_utterance_timing`, `server.py:1503-1508`).
/// `audio=?` on the plain-batch road — pressing Z, or running with no key at all
/// — is the probe with its one job removed.
#[test]
fn the_batch_probe_reports_how_long_the_recording_was() {
    let mut harness = Builder::default()
        .speech(SpeechProbe::measuring(4.2).with_local())
        .build();

    // No stream at all: local transcription never opens one.
    harness.recording(SttBackendKind::Local);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Local it is.");

    let timing = timing_lines(&messages);
    assert_eq!(timing.len(), 1);
    assert!(timing[0].contains("audio=4.20s"), "{}", timing[0]);
    assert!(timing[0].contains("path=batch"), "{}", timing[0]);

    // A backend that cannot measure the file still says so, and says nothing else.
    let mut harness = Builder::default()
        .speech(SpeechProbe::cloud().with_local())
        .build();
    harness.recording(SttBackendKind::Local);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Unmeasured.");
    assert!(
        timing_lines(&messages)[0].contains("audio=?"),
        "{}",
        timing_lines(&messages)[0]
    );
}

/// 42. `test_stream_messages_produce_no_command_result`
#[test]
fn stream_messages_are_silent_plumbing() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    let mut messages = harness.begin(24_000);
    messages.extend(harness.chunk(0));
    messages.extend(harness.end(1));

    assert!(command_results(&messages).is_empty());
    assert!(transcription_results(&messages).is_empty());
    assert!(speeches(&messages).is_empty());
}

/// 43. `test_begin_with_bad_format_degrades_to_batch_once`
#[test]
fn a_bad_sample_rate_degrades_once_and_the_batch_fallback_still_lands() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    let mut messages = harness.begin(16_000);
    messages.extend(harness.chunk(0));
    messages.extend(harness.end(1));

    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (bad_format)"],
        "reported once, not once per chunk"
    );
    assert!(
        harness.speech.calls().is_empty(),
        "a stream the session cannot parse is never opened"
    );

    let messages = harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    assert!(matches!(
        &harness.speech.batch_jobs()[0].2,
        SttBackendKind::Cloud
    ));
    assert!(command_results(&messages).is_empty(), "still in flight");

    let messages = harness.transcribed(job, "Heard anyway.");
    assert_eq!(speeches(&messages).len(), 1);
    let timing = timing_lines(&messages);
    assert!(
        timing[0].contains("path=batch(fallback:bad_format)"),
        "{}",
        timing[0]
    );
}

/// 44. `test_stream_violations_degrade_exactly_once_each`
#[test]
fn every_stream_violation_degrades_exactly_once() {
    // A gap in the sequence.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    harness.chunk(0);
    let mut messages = harness.chunk(2);
    messages.extend(harness.chunk(3)); // trailing chunks must not re-report
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (seq_gap)"]
    );

    // A chunk far larger than the wire cap.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    let messages = harness.chunk_of(0, cathedral_sim::STT_STREAM_MAX_CHUNK_SAMPLES + 1);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (oversized_chunk)"]
    );

    // An empty chunk is just as unusable.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    let messages = harness.chunk_of(0, 0);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (oversized_chunk)"]
    );

    // An endpoint that disagrees with the chunks that arrived.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    harness.chunk(0);
    let messages = harness.end(7);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (count_mismatch)"]
    );

    // An endpoint with nothing before it.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    let messages = harness.end(0);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (count_mismatch)"]
    );

    // A second endpoint for the same utterance.
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    harness.chunk(0);
    harness.end(1);
    let messages = harness.end(1);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (bad_end)"]
    );
}

#[test]
fn a_chunk_after_the_endpoint_degrades_the_stream() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    harness.chunk(0);
    harness.end(1); // → committed
    let messages = harness.chunk(1);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (chunk_after_end)"]
    );
}

#[test]
fn the_chunk_cap_matches_the_microphone_worker() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.begin(24_000);
    for seq in 0..cathedral_sim::STT_STREAM_MAX_CHUNKS {
        let messages = harness.chunk(seq);
        assert!(
            degrades(&messages).is_empty(),
            "chunk {seq} is within the cap"
        );
    }
    // 257th chunk: the worker would never send it, so it means something is
    // wrong with the utterance, not with the cap.
    let messages = harness.chunk(cathedral_sim::STT_STREAM_MAX_CHUNKS);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (too_many_chunks)"]
    );
}

#[test]
fn a_session_that_drops_a_chunk_degrades_with_backpressure_and_is_cleared() {
    let speech = SpeechProbe::cloud();
    let mut harness = Builder::default().speech(speech.clone()).build();
    harness.begin(24_000);
    // The session goes deaf mid-utterance: what it holds is now holed.
    speech.0.borrow_mut().realtime_begin_ok = false;
    let messages = harness.chunk(0);

    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (backpressure)"]
    );
    assert!(
        speech.calls().contains(&format!("clear:{WAV}")),
        "the holed session buffer is dropped: {:?}",
        speech.calls()
    );
}

/// 21. `test_recording_routes_to_selected_local_backend`
///
/// A local-only run has no realtime session at all — the session and the cloud
/// batch endpoint share one credential. The stream therefore degrades on `begin`
/// with `no_session`, which is an *expected* state and so says nothing to the
/// player; the recording then routes to the backend the player actually chose.
#[test]
fn a_local_only_run_streams_nowhere_quietly_and_records_locally() {
    let speech = SpeechProbe::default().with_local();
    let mut harness = Builder::default().speech(speech.clone()).build();

    let mut messages = harness.begin(24_000);
    messages.extend(harness.chunk(0));
    messages.extend(harness.end(1));
    assert!(
        degrades(&messages).is_empty(),
        "waiting for a session is not damage"
    );
    assert!(
        speech.calls().is_empty(),
        "a session that does not exist is never called"
    );
    assert!(command_results(&messages).is_empty());

    let messages = harness.recording(SttBackendKind::Local);
    assert_eq!(harness.speech.batch_jobs().len(), 1);
    assert_eq!(harness.speech.batch_jobs()[0].2, SttBackendKind::Local);
    assert!(
        harness.speech.batch_jobs()[0].1.ends_with(WAV),
        "the backend is handed the recording's own WAV"
    );
    assert!(command_results(&messages).is_empty(), "still in flight");

    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Local it is.");
    assert_eq!(speeches(&messages).len(), 1);
    assert!(timing_lines(&messages)[0].contains("path=batch"));
}

/// Row 55 (`test_commit_failure_degrades_quietly_to_batch`): the in-flight cap
/// and the reconnect backoff are *expected* states, and the session reports its
/// own transitions. No status spam per utterance.
#[test]
fn a_refused_commit_degrades_quietly() {
    let speech = SpeechProbe::cloud();
    speech.0.borrow_mut().realtime_commit_ok = false;
    let mut harness = Builder::default().speech(speech).build();

    harness.begin(24_000);
    harness.chunk(0);
    let messages = harness.end(1);
    assert!(degrades(&messages).is_empty(), "session waits are quiet");

    let messages = harness.recording(SttBackendKind::Cloud);
    assert!(command_results(&messages).is_empty());
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Batched.");
    let timing = timing_lines(&messages);
    assert!(
        timing[0].contains("path=batch(fallback:session_unavailable)"),
        "{}",
        timing[0]
    );
}

/// The recording overtook its own endpoint (a lost or reordered `player_audio_end`):
/// the stream is still mid-flight, so it is abandoned and the WAV uploaded.
#[test]
fn a_recording_that_arrives_mid_stream_falls_back_to_the_upload() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    harness.begin(24_000);
    harness.chunk(0);
    // No `end`: the utterance never reached its endpoint.
    harness.recording(SttBackendKind::Cloud);

    assert_eq!(harness.speech.batch_jobs().len(), 1);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Heard regardless.");
    assert_eq!(speeches(&messages).len(), 1);
    assert!(
        timing_lines(&messages)[0].contains("path=batch(fallback:incomplete_stream)"),
        "{}",
        timing_lines(&messages)[0]
    );
}

/// Offline, the batch backend plays the provider — and can refuse. The utterance
/// then takes the ordinary fallback road, and the player is still heard.
#[test]
fn a_refused_offline_stream_transcription_degrades_and_falls_back() {
    let speech = SpeechProbe::refusing(SttSubmitError::QueueFull);
    let mut harness = Builder::default().fake().speech(speech.clone()).build();

    harness.begin(24_000);
    harness.chunk(0);
    let messages = harness.end(1);
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (fake_transcription_failed)"]
    );

    // The queue drains; the recording's own upload goes through.
    speech.0.borrow_mut().submit_error = None;
    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Offline anyway.");
    assert_eq!(speeches(&messages).len(), 1);
    assert!(
        timing_lines(&messages)[0].contains("path=batch(fallback:fake_transcription_failed)"),
        "{}",
        timing_lines(&messages)[0]
    );
}

/// A refused `begin` is the same quiet story.
#[test]
fn a_refused_begin_degrades_quietly() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    let messages = harness.begin(24_000);
    assert!(degrades(&messages).is_empty());
}

/// 45. `test_chunk_after_completion_cannot_uncomplete_the_stream`
#[test]
fn late_noise_cannot_un_complete_a_finished_stream() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();
    harness.stream_utterance();
    harness.realtime(RealtimeResult::Transcript {
        key: WAV.to_string(),
        text: "Complete.".into(),
    });

    let messages = harness.chunk(9);
    assert!(degrades(&messages).is_empty(), "no status, no damage");

    // The transcript still resolves the recording — the late chunk changed nothing.
    let messages = harness.recording(SttBackendKind::Cloud);
    assert_eq!(
        transcription_results(&messages),
        vec![(Some("Complete.".to_string()), None)]
    );
    assert!(harness.speech.batch_jobs().is_empty());
}

/// 46. `test_silent_end_clears_stream_without_say` + 64.
#[test]
fn a_silent_endpoint_discards_the_utterance_and_releases_the_hold() {
    let speech = SpeechProbe::cloud();
    let mut harness = Builder::default().speech(speech.clone()).build();

    harness.begin(24_000);
    harness.chunk(0);
    assert!(
        harness.engine.floor_busy(harness.now),
        "the player is speaking"
    );

    let messages = harness.send(EngineCommand::PlayerAudioEnd {
        wav_basename: WAV.into(),
        chunk_count: 1,
        silent: true,
    });

    // Sub-minimum utterances are discarded by the worker: nothing may ever be
    // said for them, and the cast must not wait a further 3 s to find out.
    assert!(!harness.engine.floor_busy(harness.now));
    assert!(speeches(&messages).is_empty());
    assert!(degrades(&messages).is_empty());
    assert_eq!(harness.engine.speech_router().active_stream_count(), 0);
    assert!(speech.calls().contains(&format!("clear:{WAV}")));
}

/// 47. `test_abort_and_unknown_basenames_are_idempotent` + 65.
#[test]
fn aborts_and_unknown_basenames_are_idempotent() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    // Nothing exists yet: none of this may invent a stream or a hold.
    harness.chunk(0);
    harness.end(1);
    harness.send(EngineCommand::PlayerAudioAbort {
        wav_basename: WAV.into(),
    });
    assert!(!harness.engine.floor_busy(harness.now));
    assert_eq!(harness.engine.speech_router().active_stream_count(), 0);

    harness.begin(24_000);
    harness.chunk(0);
    assert!(harness.engine.floor_busy(harness.now));

    harness.send(EngineCommand::PlayerAudioAbort {
        wav_basename: WAV.into(),
    });
    assert!(
        !harness.engine.floor_busy(harness.now),
        "the hold is released"
    );

    // A trailing chunk for the aborted stream must not resurrect the hold.
    harness.chunk(1);
    assert!(!harness.engine.floor_busy(harness.now));

    // And a second abort is a no-op, not a panic.
    harness.send(EngineCommand::PlayerAudioAbort {
        wav_basename: WAV.into(),
    });
    assert!(!harness.engine.floor_busy(harness.now));
}

/// 48. `test_begin_replaces_a_live_stream`
#[test]
fn re_beginning_a_basename_replaces_the_live_stream() {
    let speech = SpeechProbe::cloud();
    let mut harness = Builder::default().fake().speech(speech.clone()).build();

    harness.begin(24_000);
    harness.chunk(0);
    harness.chunk(1);

    // The worker restarted the utterance: seq 0 is legal again.
    harness.begin(24_000);
    let messages = harness.chunk(0);
    assert!(degrades(&messages).is_empty(), "the sequence reset with it");

    harness.end(1);
    assert_eq!(harness.engine.speech_router().active_stream_count(), 1);
    let job = harness.speech.job(0);
    harness.transcribed(job, "Second take.");
    let messages = harness.recording(SttBackendKind::Cloud);
    assert_eq!(
        transcription_results(&messages),
        vec![(Some("Second take.".to_string()), None)]
    );
}

#[test]
fn the_oldest_stream_is_evicted_past_the_active_cap() {
    let speech = SpeechProbe::cloud();
    let mut harness = Builder::default().speech(speech.clone()).build();

    for index in 0..=cathedral_sim::MAX_ACTIVE_STREAMS {
        harness.send(EngineCommand::PlayerAudioBegin {
            wav_basename: format!("utterance-{index}.wav"),
            sample_rate: 24_000,
        });
    }
    assert_eq!(
        harness.engine.speech_router().active_stream_count(),
        cathedral_sim::MAX_ACTIVE_STREAMS
    );
    assert!(
        speech
            .calls()
            .contains(&"clear:utterance-0.wav".to_string()),
        "the evicted stream releases its session buffer: {:?}",
        speech.calls()
    );
}

/// 49. `test_completed_transcript_is_dropped_after_hold_window`
#[test]
fn a_held_transcript_expires_rather_than_becoming_a_late_say() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    harness.stream_utterance();
    harness.realtime(RealtimeResult::Transcript {
        key: WAV.to_string(),
        text: "Too late.".into(),
    });
    assert_eq!(harness.engine.speech_router().active_stream_count(), 1);

    // The owning `player_recording` never came (the game died, or an abort was
    // lost). A transcript nobody claimed must never turn into a late utterance.
    harness.advance(cathedral_sim::STT_STREAM_HELD_TRANSCRIPT_SECONDS + 0.01);
    harness.poll();
    assert_eq!(harness.engine.speech_router().active_stream_count(), 0);

    // A recording arriving now pays for the upload instead of resurrecting it.
    let messages = harness.recording(SttBackendKind::Cloud);
    assert!(speeches(&messages).is_empty());
    assert_eq!(harness.speech.batch_jobs().len(), 1);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Fresh.");
    let timing = timing_lines(&messages);
    assert!(timing[0].contains("path=batch"), "{}", timing[0]);
}

// ============================================================== the stream join

/// 51. `test_committed_recording_parks_and_resolves_on_completion`
#[test]
fn a_committed_recording_parks_until_the_provider_answers() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    harness.stream_utterance();
    let messages = harness.recording(SttBackendKind::Cloud);

    assert_eq!(harness.engine.speech_router().parked_count(), 1);
    assert!(
        harness.speech.batch_jobs().is_empty(),
        "the provider already holds the audio; do not pay for it twice"
    );
    let transcribing = statuses(&messages)
        .into_iter()
        .find(|status| status.state == "transcribing")
        .expect("the player is told his words are on their way");
    assert_eq!(transcribing.backend.as_deref(), Some("cloud"));
    assert!(command_results(&messages).is_empty(), "still in flight");

    harness.advance(0.3);
    let messages = harness.realtime(RealtimeResult::Transcript {
        key: WAV.to_string(),
        text: "  Parked and heard.  ".into(),
    });
    assert_eq!(harness.engine.speech_router().parked_count(), 0);
    assert_eq!(
        transcription_results(&messages),
        vec![(Some("Parked and heard.".to_string()), None)],
        "the transcript is trimmed"
    );
    assert_eq!(speeches(&messages).len(), 1);
    assert!(command_results(&messages)[0].0);
}

/// 52. `test_grace_expiry_batches_once_and_late_completion_is_discarded`
#[test]
fn the_grace_window_batches_once_and_a_late_completion_is_discarded() {
    let speech = SpeechProbe::cloud();
    let mut harness = Builder::default().speech(speech.clone()).grace(0.5).build();

    harness.stream_utterance();
    harness.recording(SttBackendKind::Cloud);

    // The provider is taking too long. Stop waiting; upload.
    harness.advance(0.51);
    let messages = harness.poll();
    assert!(command_results(&messages).is_empty());
    assert_eq!(harness.speech.batch_jobs().len(), 1);
    assert!(speech.calls().contains(&format!("clear:{WAV}")));

    // The realtime transcript turns up anyway — and is dropped. It must never
    // become a *second* say alongside the batch result.
    let messages = harness.realtime(RealtimeResult::Transcript {
        key: WAV.to_string(),
        text: "The late one.".into(),
    });
    assert!(speeches(&messages).is_empty());
    assert!(transcription_results(&messages).is_empty());

    // Only the batch result speaks.
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "The batched one.");
    assert_eq!(speeches(&messages).len(), 1);
    let timing = timing_lines(&messages);
    assert!(
        timing[0].contains("path=batch(fallback:grace)"),
        "{}",
        timing[0]
    );

    // And the grace timer does not fire twice.
    harness.advance(5.0);
    let messages = harness.poll();
    assert!(command_results(&messages).is_empty());
    assert_eq!(harness.speech.batch_jobs().len(), 1);
}

/// 53. `test_session_failure_batches_parked_requests_immediately`
#[test]
fn a_keyed_session_failure_batches_the_parked_recording() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    harness.stream_utterance();
    harness.recording(SttBackendKind::Cloud);

    harness.realtime(RealtimeResult::Failure {
        key: Some(WAV.to_string()),
        reason: "socket".into(),
    });
    assert_eq!(harness.engine.speech_router().parked_count(), 0);
    assert_eq!(harness.speech.batch_jobs().len(), 1);

    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "Rescued.");
    assert_eq!(speeches(&messages).len(), 1);
    let timing = timing_lines(&messages);
    assert!(
        timing[0].contains("path=batch(fallback:socket)"),
        "{}",
        timing[0]
    );
}

/// 54. `test_session_wide_failure_batches_every_parked_request`
#[test]
fn a_session_wide_failure_rescues_everything_in_flight() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    // One utterance parked, one still streaming.
    harness.stream_utterance();
    harness.recording(SttBackendKind::Cloud);
    harness.send(EngineCommand::PlayerAudioBegin {
        wav_basename: "utterance-2.wav".into(),
        sample_rate: 24_000,
    });

    let messages = harness.realtime(RealtimeResult::Failure {
        key: None,
        reason: "connect_failed".into(),
    });

    assert_eq!(harness.engine.speech_router().parked_count(), 0);
    assert_eq!(
        harness.speech.batch_jobs().len(),
        1,
        "the parked recording is uploaded"
    );
    assert_eq!(
        degrades(&messages),
        vec!["streamed audio fell back to batch (connect_failed)"],
        "the live stream is told too — once"
    );
}

/// 56. `test_local_backend_clears_the_session_and_never_streams`
#[test]
fn switching_to_local_transcription_mid_utterance_drops_the_streamed_copy() {
    let speech = SpeechProbe::cloud().with_local();
    let mut harness = Builder::default().speech(speech.clone()).build();

    harness.stream_utterance();
    // The player pressed Z between the endpoint and the recording.
    let messages = harness.recording(SttBackendKind::Local);

    assert_eq!(harness.engine.speech_router().parked_count(), 0);
    assert_eq!(harness.speech.batch_jobs().len(), 1);
    assert_eq!(harness.speech.batch_jobs()[0].2, SttBackendKind::Local);
    assert!(
        speech.calls().contains(&format!("clear:{WAV}")),
        "the streamed copy is irrelevant now"
    );
    let loading = statuses(&messages)
        .into_iter()
        .find(|status| status.state == "loading")
        .expect("the 5 GB first-use download is announced");
    assert_eq!(loading.backend.as_deref(), Some("local"));
}

// ========================================================= resolving the words

/// 23/69. A successful transcription is trimmed, said as a broadcast, heard by
/// every NPC, and hands the next turn to the nearest LLM listener.
#[test]
fn a_transcript_becomes_a_broadcast_say_and_wakes_the_nearest_listener() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();

    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "  Good day to you.  ");

    let speech = speeches(&messages);
    assert_eq!(speech.len(), 1);
    match speech[0] {
        EngineMessage::Speech {
            speaker_id,
            target_id,
            text,
            recipient_ids,
            ..
        } => {
            assert_eq!(speaker_id, &player());
            assert_eq!(target_id, &None, "microphone speech is never targeted");
            assert_eq!(text, "Good day to you.");
            for npc in ["sv3n1", "cb947", "k0fb1"] {
                assert!(
                    recipient_ids.contains(&ActorId::from_raw(npc)),
                    "{npc} is within earshot"
                );
            }
        }
        _ => unreachable!(),
    }

    // The nearest LLM hearer answers next, without waiting out the round-robin.
    assert_eq!(
        harness.engine.scheduler().priority_actor_id(),
        Some(&ActorId::from_raw("cb947"))
    );

    let (success, code, message) = command_results(&messages)[0].clone();
    assert!(success);
    assert_eq!(code, None);
    assert!(message.contains("Good day to you."), "{message}");
    // And the player's own line never holds the NPC floor (62).
    assert!(!harness.engine.floor_busy(harness.now));
}

/// 30. `test_recording_hearing_uses_utterance_position_while_player_moves`
#[test]
fn the_say_lands_where_the_utterance_was_spoken_not_where_the_player_now_is() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();

    // Recorded at the spawn, in earshot of all three.
    harness.recording_at(SttBackendKind::Cloud, PLAYER_SPAWN);
    let job = harness.speech.job(0);

    // He walks a hundred metres away while the provider thinks.
    harness.spatial_seq += 1;
    harness.send(EngineCommand::SpatialUpdate {
        spatial_seq: harness.spatial_seq,
        updates: vec![cathedral_sim::SpatialActorUpdate::new(
            player(),
            Vec3::new(0.0, 0.91, 211.0),
            None,
        )],
    });

    let messages = harness.transcribed(job, "Hear me.");
    match speeches(&messages)[0] {
        EngineMessage::Speech {
            speaker_position_m,
            recipient_ids,
            ..
        } => {
            assert_eq!(
                *speaker_position_m, PLAYER_SPAWN,
                "the utterance is frozen where it was spoken"
            );
            assert!(recipient_ids.contains(&ActorId::from_raw("cb947")));
        }
        _ => unreachable!(),
    }

    // …and the authoritative position was NOT rewound.
    assert_eq!(
        harness.engine.world().characters[&player()].position_m(),
        Vec3::new(0.0, 0.91, 211.0)
    );
}

/// 31. `test_stt_timeout_and_failure_degrade_without_crashing` + 67.
#[test]
fn a_failed_transcription_degrades_and_clears_the_player_hold() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();

    harness.recording(SttBackendKind::Cloud);
    // The 8 s transcribing hold is on: the cast waits for the player's words.
    assert!(harness.engine.floor_busy(harness.now));

    let job = harness.speech.job(0);
    let messages = harness.transcription_failed(job, "the speech provider timed out");

    assert_eq!(
        transcription_results(&messages),
        vec![(None, Some("the speech provider timed out".to_string()))]
    );
    let degraded = statuses(&messages)
        .into_iter()
        .find(|status| status.subsystem == Subsystem::Stt && status.state == "degraded")
        .expect("the failure is shown");
    assert_eq!(degraded.backend.as_deref(), Some("cloud"));
    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("transcription_failed")
    );
    assert!(speeches(&messages).is_empty());

    // Nothing will be said, so the cast may speak again at once.
    assert!(!harness.engine.floor_busy(harness.now));
    assert_eq!(timing_lines(&messages).len(), 1, "failures log a probe too");
}

#[test]
fn an_empty_transcript_is_reported_as_no_speech() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "   \n  ");

    assert_eq!(
        transcription_results(&messages),
        vec![(None, Some("no speech detected".to_string()))]
    );
    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("empty_transcription")
    );
    assert!(speeches(&messages).is_empty());
    assert!(!harness.engine.floor_busy(harness.now));
}

#[test]
fn an_overlong_transcript_is_rejected_by_the_same_rule_the_llm_obeys() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    // Unicode scalars, not bytes (D11): 501 two-byte characters.
    let messages =
        harness.transcribed(job, &"é".repeat(cathedral_sim::PLAYER_SPEECH_MAX_CHARS + 1));

    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("text_too_long")
    );
    assert_eq!(
        transcription_results(&messages)[0].1.as_deref(),
        Some("transcription exceeds the 500 character limit")
    );
    assert!(speeches(&messages).is_empty());
}

/// 32. `test_invalid_unicode_transcription_is_rejected_without_protocol_output`
#[test]
fn a_transcript_with_control_characters_is_rejected() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "hello\u{7}world");

    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("invalid_transcription")
    );
    assert_eq!(
        transcription_results(&messages),
        vec![(
            None,
            Some("transcription contains unsupported characters".to_string())
        )]
    );
    assert!(speeches(&messages).is_empty());
    // A newline is not a control character for this purpose — it is a pause.
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "one\ntwo");
    assert!(command_results(&messages)[0].0);
}

/// 33. `test_recording_path_traversal_is_rejected`
#[test]
fn a_recording_may_only_name_a_bare_wav_inside_the_runtime_directory() {
    for (basename, code) in [
        ("../secret.wav", "invalid_path"),
        ("sub/dir.wav", "invalid_path"),
        ("recording.mp3", "invalid_path"),
        ("", "invalid_request"),
    ] {
        let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
        let messages = harness.send(EngineCommand::PlayerRecording {
            request_id: "rec-1".into(),
            wav_basename: basename.into(),
            stt_backend: SttBackendKind::Cloud,
            position_m: PLAYER_SPAWN,
            spatial_seq: 1,
        });
        assert_eq!(
            command_results(&messages)[0].1.as_deref(),
            Some(code),
            "{basename}"
        );
        // The pending request is always answered — never left hanging.
        assert_eq!(transcription_results(&messages).len(), 1);
        assert!(harness.speech.batch_jobs().is_empty());
    }
}

/// 34. `test_rejected_recording_is_deleted_before_returning_error`
#[test]
fn a_recording_for_a_backend_that_does_not_exist_is_refused_up_front() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    // Cloud works; local was never configured.
    let messages = harness.recording(SttBackendKind::Local);

    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("stt_unavailable")
    );
    assert!(harness.speech.batch_jobs().is_empty(), "no charge, no work");
    assert!(!harness.engine.floor_busy(harness.now), "and no hold");
}

#[test]
fn a_full_transcription_queue_is_overloaded_not_silent() {
    let mut harness = Builder::default()
        .speech(SpeechProbe::refusing(SttSubmitError::QueueFull))
        .build();
    let messages = harness.recording(SttBackendKind::Cloud);

    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("overloaded")
    );
    assert_eq!(
        transcription_results(&messages)[0].1.as_deref(),
        Some("transcription queue is full")
    );
    assert!(!harness.engine.floor_busy(harness.now));
}

/// A parked recording whose fallback upload is also refused must still resolve —
/// otherwise the player's pending request, and his floor hold, hang forever.
#[test]
fn a_parked_recording_whose_fallback_is_refused_still_resolves() {
    let speech = SpeechProbe::cloud();
    let mut harness = Builder::default().speech(speech.clone()).grace(0.5).build();

    harness.stream_utterance();
    harness.recording(SttBackendKind::Cloud);
    speech.0.borrow_mut().submit_error = Some(SttSubmitError::QueueFull);

    harness.advance(0.51);
    let messages = harness.poll();

    assert_eq!(
        transcription_results(&messages)[0].1.as_deref(),
        Some("transcription queue is full")
    );
    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("transcription_failed")
    );
    assert!(!harness.engine.floor_busy(harness.now));
    assert_eq!(harness.engine.speech_router().parked_count(), 0);
}

/// A stale spatial sequence in the recording payload fails the command without
/// moving the player — the world's monotonicity guard is not negotiable.
#[test]
fn a_stale_recording_position_is_rejected() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();
    harness.spatial_seq = 5;
    harness.send(EngineCommand::SpatialUpdate {
        spatial_seq: 5,
        updates: vec![cathedral_sim::SpatialActorUpdate::new(
            player(),
            Vec3::new(0.0, 0.91, 120.0),
            None,
        )],
    });

    let messages = harness.send(EngineCommand::PlayerRecording {
        request_id: "rec-1".into(),
        wav_basename: WAV.into(),
        stt_backend: SttBackendKind::Cloud,
        position_m: PLAYER_SPAWN,
        spatial_seq: 2,
    });
    assert_eq!(
        command_results(&messages)[0].1.as_deref(),
        Some("stale_spatial_seq")
    );
    assert_eq!(
        harness.engine.world().characters[&player()].position_m(),
        Vec3::new(0.0, 0.91, 120.0)
    );
}

// ============================================================ the player's hold

/// 63. `test_streamed_audio_holds_the_floor_and_expires_on_its_own`
#[test]
fn the_microphone_holds_the_floor_on_a_rolling_deadline() {
    let mut harness = Builder::default().speech(SpeechProbe::cloud()).build();

    harness.begin(24_000);
    assert!(harness.engine.floor_busy(harness.now));

    // Each chunk re-arms the 1.7 s hold, so a talking player is never cut off.
    for seq in 0..5 {
        harness.advance(1.0);
        harness.chunk(seq);
        assert!(
            harness.engine.floor_busy(harness.now),
            "still speaking at chunk {seq}"
        );
    }

    // And a client that dies mid-utterance simply stops bumping it.
    harness.advance(cathedral_sim::FLOOR_PLAYER_CHUNK_HOLD_SECONDS + 0.01);
    assert!(!harness.engine.floor_busy(harness.now));
}

/// 66. `test_completed_transcription_clears_the_hold`
#[test]
fn the_endpoint_holds_for_three_seconds_and_a_resolution_clears_it() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();

    harness.begin(24_000);
    harness.chunk(0);
    harness.end(1);

    // Past the chunk hold, inside the endpoint hold: the transcript is coming.
    harness.advance(cathedral_sim::FLOOR_PLAYER_CHUNK_HOLD_SECONDS + 0.01);
    assert!(harness.engine.floor_busy(harness.now));

    harness.recording(SttBackendKind::Cloud);
    // The recording extends it to the 8 s transcribing hold.
    harness.advance(cathedral_sim::FLOOR_PLAYER_ENDPOINT_HOLD_SECONDS + 0.01);
    assert!(harness.engine.floor_busy(harness.now));

    let job = harness.speech.job(0);
    harness.transcribed(job, "Done.");
    assert!(
        !harness.engine.floor_busy(harness.now),
        "the say governs pacing from here, not the hold"
    );
}

/// Row 68, the flagship: an NPC line that lands while the player is talking is
/// *held* — the scheduler applies it only once the floor is free again.
#[test]
fn the_player_speaking_holds_the_floor_against_the_cast() {
    let mut harness = Builder::default().speech(SpeechProbe::batch_only()).build();

    harness.begin(24_000);
    harness.chunk(0);
    assert!(
        harness.engine.floor_busy(harness.now),
        "an NPC turn finishing now would be held, not applied"
    );

    harness.send(EngineCommand::PlayerAudioAbort {
        wav_basename: WAV.into(),
    });
    assert!(!harness.engine.floor_busy(harness.now));
}

// ==================================================================== NPC voices

/// The gating predicate: who gets a voice at all.
#[test]
fn only_an_audible_voiced_npc_line_is_ever_synthesized() {
    // Voices off: no work, no failure, and the line paces on its reading time.
    let tts = VoiceProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Off, tts.clone())
        .build();
    harness.npc("k0fb1", "say", json!({"text": "Voices are off."}));
    let messages = harness.poll();
    assert_eq!(speeches(&messages).len(), 1, "the text still ships");
    assert!(tts.submitted().is_empty());
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, EngineMessage::TtsFailed { .. })),
        "off is not a failure"
    );
    assert!(harness.engine.floor_busy(harness.now));
    harness.advance(speech_reading_seconds("Voices are off.") + 0.01);
    assert!(
        !harness.engine.floor_busy(harness.now),
        "a line with no audio may never be awaited (D26)"
    );

    // Out of earshot: no audio for a line the player cannot hear.
    let tts = VoiceProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, tts.clone())
        .build();
    harness.send(EngineCommand::SpatialUpdate {
        spatial_seq: 1,
        updates: vec![cathedral_sim::SpatialActorUpdate::new(
            player(),
            Vec3::new(0.0, 0.91, 1_000.0),
            None,
        )],
    });
    harness.npc("k0fb1", "say", json!({"text": "Nobody in earshot."}));
    let messages = harness.poll();
    assert_eq!(speeches(&messages).len(), 1);
    assert!(tts.submitted().is_empty());

    // The player's own voice never comes back at him.
    let tts = VoiceProbe::available();
    let mut harness = Builder::default()
        .speech(SpeechProbe::batch_only())
        .voices(TtsBackendKind::Local, tts.clone())
        .build();
    harness.recording(SttBackendKind::Cloud);
    let job = harness.speech.job(0);
    let messages = harness.transcribed(job, "My own words.");
    assert_eq!(speeches(&messages).len(), 1);
    assert!(tts.submitted().is_empty());
    assert!(!harness.engine.floor_busy(harness.now));
}

/// 19. `test_tts_mode_is_captured_when_each_utterance_is_queued`
#[test]
fn the_voice_backend_is_captured_when_the_utterance_is_queued() {
    let tts = VoiceProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, tts.clone())
        .build();

    harness.npc("k0fb1", "say", json!({"text": "Spoken under local."}));
    let messages = harness.poll();
    let first = speech_event_id(&messages);
    assert_eq!(tts.submitted()[0].kind, TtsBackendKind::Local);
    assert_eq!(tts.submitted()[0].voice_key, "ilse");

    // The player switches to the cloud voice while the first line is still
    // synthesizing. That must not re-route the line already in flight…
    harness.send(EngineCommand::SetTtsBackend {
        request_id: "tts-1".into(),
        backend: TtsBackendKind::Cloud,
    });
    let messages = harness.send(EngineCommand::Tts(TtsOutcome::StreamEnd {
        event_id: first.clone(),
        chunk_count: 2,
        first_chunk_ms: 173,
    }));
    let idle = statuses(&messages)
        .into_iter()
        .find(|status| status.subsystem == Subsystem::Tts && status.state == "idle")
        .expect("the local first-PCM latency is reported");
    assert_eq!(idle.message.as_deref(), Some("First local PCM in 173 ms"));
    assert_eq!(idle.backend.as_deref(), Some("local"));

    // …while the *next* line goes to the cloud.
    harness.npc("cb947", "say", json!({"text": "Spoken under cloud."}));
    harness.poll();
    assert_eq!(tts.submitted()[1].kind, TtsBackendKind::Cloud);
}

/// 18/36. The relay: chunks, stream end, whole-WAV success, and a failure that
/// releases the floor rather than stalling the cast for the failsafe window.
#[test]
fn synthesis_outcomes_reach_the_game_and_a_failure_frees_the_floor() {
    let tts = VoiceProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Cloud, tts)
        .build();

    let text = "You may have my copper coin.";
    harness.npc("k0fb1", "say", json!({"text": text}));
    let messages = harness.poll();
    let event_id = speech_event_id(&messages);

    let samples: Arc<[i16]> = Arc::from(vec![0i16, 1, 2].into_boxed_slice());
    let messages = harness.send(EngineCommand::Tts(TtsOutcome::Chunk {
        event_id: event_id.clone(),
        seq: 0,
        sample_rate: 24_000,
        samples,
    }));
    assert!(messages.iter().any(|m| matches!(
        m,
        EngineMessage::TtsChunk {
            chunk_seq: 0,
            sample_rate: 24_000,
            ..
        }
    )));

    // Awaited well past the reading estimate — audio is coming.
    harness.advance(speech_reading_seconds(text) + 1.0);
    assert!(harness.engine.floor_busy(harness.now));

    let messages = harness.send(EngineCommand::Tts(TtsOutcome::Done {
        event_id: event_id.clone(),
        result: Err(SpeechError::new("cloud speech provider timed out")),
    }));
    let degraded = statuses(&messages)
        .into_iter()
        .find(|status| status.subsystem == Subsystem::Tts && status.state == "degraded")
        .expect("the failure is shown");
    assert_eq!(
        degraded.message.as_deref(),
        Some("cloud speech provider timed out")
    );
    assert_eq!(
        degraded.backend.as_deref(),
        Some("cloud"),
        "queue-time capture"
    );
    assert!(messages.iter().any(|m| matches!(
        m,
        EngineMessage::TtsFailed { reason, .. } if reason == "cloud speech provider timed out"
    )));

    harness.advance(0.5); // past the post-utterance beat
    assert!(
        !harness.engine.floor_busy(harness.now),
        "a dead voice worker must not stall the cast for 45 s"
    );
}

#[test]
fn a_whole_wav_success_ships_the_bytes_and_reports_the_queue_time_backend() {
    let tts = VoiceProbe::available();
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Cloud, tts)
        .build();

    harness.npc("k0fb1", "say", json!({"text": "Here."}));
    let messages = harness.poll();
    let event_id = speech_event_id(&messages);

    let wav: Arc<[u8]> = Arc::from(vec![0u8; 8].into_boxed_slice());
    let messages = harness.send(EngineCommand::Tts(TtsOutcome::Done {
        event_id,
        result: Ok(wav),
    }));
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, EngineMessage::TtsReady { .. }))
    );
    let idle = statuses(&messages)
        .into_iter()
        .find(|status| status.subsystem == Subsystem::Tts && status.state == "idle")
        .expect("synthesis went quiet");
    assert_eq!(idle.backend.as_deref(), Some("cloud"));
}

/// R10. A submission the backend *refuses* must behave exactly like a line that
/// was never voiced — never like one whose audio is still coming.
#[test]
fn a_refused_submission_never_leaves_the_floor_awaiting() {
    for (error, reason) in [
        (TtsSubmitError::QueueFull, "speech queue is full"),
        (
            TtsSubmitError::PathInUse,
            "speech output path is already in use",
        ),
        (
            TtsSubmitError::Unavailable,
            "local NPC voice backend is unavailable",
        ),
    ] {
        let tts = VoiceProbe::refusing(error);
        let mut harness = Builder::default()
            .voices(TtsBackendKind::Local, tts)
            .build();

        harness.npc("k0fb1", "say", json!({"text": "Refused."}));
        let messages = harness.poll();

        assert!(
            messages
                .iter()
                .any(|m| matches!(m, EngineMessage::TtsFailed { reason: r, .. } if r == reason)),
            "{reason}"
        );
        // The line paces on its reading estimate, not on the 8+ s failsafe.
        assert!(harness.engine.floor_busy(harness.now));
        harness.advance(speech_reading_seconds("Refused.") + 0.01);
        assert!(!harness.engine.floor_busy(harness.now), "{reason}");
    }
}

#[test]
fn a_vanished_voice_backend_is_reported_and_the_line_stays_text() {
    // `available == false` while `local` is selected: the backend went away.
    let mut harness = Builder::default()
        .voices(TtsBackendKind::Local, VoiceProbe::default())
        .build();

    harness.npc("k0fb1", "say", json!({"text": "No voice."}));
    let messages = harness.poll();

    let unavailable = statuses(&messages)
        .into_iter()
        .find(|status| status.subsystem == Subsystem::Tts && status.state == "unavailable")
        .expect("the missing backend is reported");
    assert_eq!(
        unavailable.message.as_deref(),
        Some("local NPC voice backend is unavailable")
    );
    assert_eq!(speeches(&messages).len(), 1, "the text still ships");
    harness.advance(speech_reading_seconds("No voice.") + 0.01);
    assert!(!harness.engine.floor_busy(harness.now));
}
