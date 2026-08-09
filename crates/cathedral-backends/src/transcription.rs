//! The player's voice: batch, local, and the realtime stream
//! (`server.py:1401-1583`, backend half).
//!
//! [`SttEngine`] is the [`Transcription`] the engine holds. It owns three
//! independent paths, and the router picks between them:
//!
//! * **batch cloud** — the recording is uploaded when the player lets go of the
//!   key. Always available with a key; always the fallback;
//! * **batch local** — the same recording, handed to the Canary-Qwen worker. No
//!   network, no key, no cloud;
//! * **realtime** — the audio was already streamed to the provider while he was
//!   speaking, so the transcript is usually there before the recording is. Every
//!   realtime method answers `bool`: `false` means *use batch*, and is not a
//!   failure.
//!
//! Batch work runs on **one** worker thread with a capacity-4 queue (Python's
//! `_DaemonWorker(..., capacity=4)`): the local worker can only transcribe one
//! utterance at a time, and a fifth queued recording means the player is talking
//! faster than the machine can listen — which is an `overloaded` refusal, not a
//! backlog.
//!
//! **WAV ownership lives here** (ARCHITECTURE §1.2): the router hands over a
//! path, and the recording is deleted once the job finishes, whatever the
//! outcome. A recording that a *realtime* transcript resolves is never submitted
//! here at all — so the router says so explicitly, through
//! [`Transcription::discard_recording`], and Python's "unlink on every
//! resolution path" (`server.py:1594-1602`) holds again.

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    thread::JoinHandle,
};

use cathedral_sim::{
    SpeechError, SttBackendKind, SttSubmitError, Transcription, TranscriptionJobId,
};
use crossbeam_channel::{Sender, TrySendError, bounded, unbounded};

use crate::{
    config::SpeechSettings,
    events::{BackendEvent, BackendSender},
    runtime::BackendRuntime,
    stt_cloud::CloudTranscriber,
    stt_local::CanaryTranscriber,
    stt_realtime::RealtimeSttHandle,
    wav::{safe_session_path, wav_duration_seconds},
};

/// `server.py:585-587` — the batch queue.
pub const STT_QUEUE_CAPACITY: usize = 4;

/// How much of a recording [`SttEngine::recording_seconds`] reads before it
/// gives up on the short road. The microphone's own header is 44 bytes; a page
/// leaves room for any `LIST`/`fact` chunk a future encoder might put in front
/// of `data`, and is four orders of magnitude smaller than the 2.9 MB a full
/// 15 s float32 utterance weighs.
const HEADER_PROBE_BYTES: usize = 4096;

struct Job {
    id: TranscriptionJobId,
    path: PathBuf,
    kind: SttBackendKind,
}

/// What the recording-disposal thread does next.
///
/// The unlink is queued rather than performed by the caller because
/// [`Transcription::discard_recording`] is called from inside `Engine::poll`,
/// which the game pumps on its main thread — on the very frame the player's
/// utterance resolves, which is the one frame he is already waiting on.
enum Discard {
    /// Delete this recording, whatever is left of it.
    File(PathBuf),
    /// Replies when every deletion queued before it has happened. Tests only —
    /// the game never waits for a recording to be gone, it only requires that
    /// it goes.
    #[cfg(test)]
    Barrier(Sender<()>),
}

/// The unlink of `_resolve_transcription` (`server.py:1594-1602`): a missing
/// file is the normal case (the batch worker usually got there first), and a
/// file that will not go away is logged rather than raised.
fn remove_recording(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        // `server.py:1599-1602` printed and carried on: a recording that will
        // not go away is not worth losing the utterance over.
        Err(error) => crate::worker::log(
            "stt",
            &format!("[smart actors] could not remove recording: {error}"),
        ),
    }
}

/// Read as much of `buffer` as the file holds, restarting on a short read.
///
/// `Read::read` may return fewer bytes than asked for even when more are there,
/// and a RIFF chunk table split across two reads would look truncated — which
/// [`crate::wav::wav_duration_seconds`] would report as "malformed" rather than
/// as "read more".
fn read_prefix(file: &mut std::fs::File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// The [`Transcription`] the engine listens through.
pub struct SttEngine {
    cloud: Option<Arc<CloudTranscriber>>,
    local: Option<Arc<CanaryTranscriber>>,
    realtime: Option<RealtimeSttHandle>,
    jobs: Sender<Job>,
    /// Recordings the router has finished with, on their way to a thread that
    /// can afford to block on the unlink. Deliberately *not* the batch queue:
    /// that one is four deep on purpose, and a deletion taking one of its slots
    /// would refuse a real utterance as `overloaded`.
    discards: Sender<Discard>,
    /// Where a bare basename resolves to (D28). `None` in the headless runner,
    /// which has no microphone.
    session_dir: Option<PathBuf>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SttEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SttEngine")
            .field("cloud", &self.cloud.is_some())
            .field("local", &self.local.is_some())
            .field("realtime", &self.realtime.is_some())
            .finish_non_exhaustive()
    }
}

impl SttEngine {
    /// The production engine. `realtime` is `None` without a key — and must stay
    /// `None` in fake mode, which never opens a socket (`server.py:577-584`).
    pub fn new(
        runtime: Arc<BackendRuntime>,
        settings: &SpeechSettings,
        session_dir: Option<PathBuf>,
        events: BackendSender,
    ) -> Self {
        let cloud = Arc::new(CloudTranscriber::new(settings));
        let local = Arc::new(CanaryTranscriber::new(settings, events.clone()));
        let realtime = RealtimeSttHandle::connect(&runtime, settings, events.clone());

        let (jobs, inbox) = bounded::<Job>(STT_QUEUE_CAPACITY);
        let worker = {
            let cloud = Arc::clone(&cloud);
            let local = Arc::clone(&local);
            let events = events.clone();
            std::thread::Builder::new()
                .name("cathedral-stt".to_string())
                .spawn(move || {
                    for job in inbox {
                        let result = match job.kind {
                            SttBackendKind::Cloud => runtime.block_on(cloud.transcribe(&job.path)),
                            SttBackendKind::Local => local.transcribe(&job.path),
                        };
                        // The recording has been heard (or failed to be): it is a
                        // file with the player's voice in it, and it goes now.
                        let _ = std::fs::remove_file(&job.path);
                        events.send(BackendEvent::TranscriptionDone {
                            job: job.id,
                            result,
                        });
                    }
                })
                .expect("a transcription thread")
        };

        let (discards, disposals) = unbounded::<Discard>();
        std::thread::Builder::new()
            .name("cathedral-stt-discard".to_string())
            .spawn(move || {
                for job in disposals {
                    match job {
                        Discard::File(path) => remove_recording(&path),
                        #[cfg(test)]
                        Discard::Barrier(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("a recording-disposal thread");

        Self {
            cloud: Some(cloud),
            local: Some(local),
            realtime,
            jobs,
            discards,
            session_dir,
            worker: Some(worker),
        }
    }

    /// A bare basename means "in the session's runtime directory", and may not
    /// mean anything else (`_safe_basename`, `server.py:152-160`). An absolute
    /// path is the caller's own business — the headless runner and the tests
    /// hand over temp files.
    fn resolve(&self, path: PathBuf) -> Result<PathBuf, SpeechError> {
        if path.is_absolute() {
            return Ok(path);
        }
        let basename = path
            .to_str()
            .ok_or_else(|| SpeechError::new("recording path is not valid UTF-8"))?;
        let directory = self
            .session_dir
            .as_deref()
            .ok_or_else(|| SpeechError::new("there is no runtime directory for recordings"))?;
        Ok(safe_session_path(directory, basename)?)
    }

    /// Block until every recording handed to
    /// [`Transcription::discard_recording`] so far is gone.
    ///
    /// Not on the trait: the game has nothing to wait for — the file goes when
    /// it goes, and `SessionDir` takes whatever is left with it — but a test
    /// that looks at the directory afterwards needs a point to synchronize on
    /// that is not a sleep.
    #[cfg(test)]
    fn wait_for_discards(&self) {
        let (done, wait) = bounded(1);
        if self.discards.send(Discard::Barrier(done)).is_ok() {
            let _ = wait.recv();
        }
    }
}

impl Transcription for SttEngine {
    fn available(&self, kind: SttBackendKind) -> bool {
        match kind {
            SttBackendKind::Cloud => self.cloud.as_ref().is_some_and(|cloud| cloud.available()),
            SttBackendKind::Local => self.local.as_ref().is_some_and(|local| local.available()),
        }
    }

    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        wav_path: PathBuf,
        kind: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        if !self.available(kind) {
            return Err(SttSubmitError::Unavailable);
        }
        let Ok(path) = self.resolve(wav_path) else {
            return Err(SttSubmitError::Unavailable);
        };
        match self.jobs.try_send(Job {
            id: job,
            path,
            kind,
        }) {
            Ok(()) => Ok(()),
            // Four recordings are already waiting: the player is speaking faster
            // than the machine can listen.
            Err(TrySendError::Full(_)) => Err(SttSubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SttSubmitError::Unavailable),
        }
    }

    fn realtime_begin(&mut self, key: &str) -> bool {
        self.realtime
            .as_ref()
            .is_some_and(|realtime| realtime.begin(key))
    }

    fn realtime_append(&mut self, key: &str, samples: &[i16]) -> bool {
        self.realtime
            .as_ref()
            .is_some_and(|realtime| realtime.append(key, samples))
    }

    fn realtime_commit(&mut self, key: &str) -> bool {
        self.realtime
            .as_ref()
            .is_some_and(|realtime| realtime.commit(key))
    }

    fn realtime_clear(&mut self, key: &str) {
        if let Some(realtime) = &self.realtime {
            realtime.clear(key);
        }
    }

    /// The header walk of `_wav_duration_seconds` (`server.py:181-215`), on the
    /// bytes the microphone wrote. Unreadable, missing, malformed — all `None`,
    /// which the probe prints as `audio=?`.
    ///
    /// It reads the *header*, not the recording. This is called from inside
    /// `Engine::poll`, which the game pumps on its main thread on the frame the
    /// player's utterance resolves, and a capped 15 s float32 recording is
    /// nearly 3 MB — a visible hitch to copy in full for the sake of a chunk
    /// table in the first few dozen bytes. A prefix that turns out not to reach
    /// the `data` header still falls back to the whole file, so an exotic chunk
    /// layout answers exactly what it always did.
    fn recording_seconds(&self, wav_path: &Path) -> Option<f64> {
        let path = self.resolve(wav_path.to_path_buf()).ok()?;
        let mut file = std::fs::File::open(&path).ok()?;
        let mut header = [0u8; HEADER_PROBE_BYTES];
        let read = read_prefix(&mut file, &mut header).ok()?;
        if let Some(seconds) = wav_duration_seconds(&header[..read]) {
            return Some(seconds);
        }
        if read < header.len() {
            // The prefix *was* the whole file: there is nothing more to read,
            // and `None` is the honest answer.
            return None;
        }
        wav_duration_seconds(&std::fs::read(&path).ok()?)
    }

    /// The recording goes when the utterance resolves, on every road it may have
    /// taken (`server.py:1594-1602`). The batch worker deletes the WAV as it
    /// finishes with it, so this is usually a no-op — but a realtime transcript
    /// resolves an utterance the batch pipeline never saw, and that file is the
    /// player's recorded voice sitting in a RAM-backed `/tmp`.
    ///
    /// The unlink itself is queued: this runs on the game's main thread on the
    /// frame the player is waiting for his reply, and `unlink(2)` on a busy
    /// journalled filesystem can block behind a journal commit. The file still
    /// goes, and still goes in order — the queue is FIFO and nothing else
    /// deletes recordings — and `SessionDir` removes whatever a crash leaves
    /// behind when the process ends.
    fn discard_recording(&mut self, wav_path: &Path) {
        let Ok(path) = self.resolve(wav_path.to_path_buf()) else {
            return;
        };
        if let Err(undelivered) = self.discards.send(Discard::File(path)) {
            // No disposal thread left to hand it to: better a blocked frame
            // than the player's voice staying on disk.
            match undelivered.into_inner() {
                Discard::File(path) => remove_recording(&path),
                #[cfg(test)]
                Discard::Barrier(_) => {}
            }
        }
    }
}

impl Drop for SttEngine {
    fn drop(&mut self) {
        if let Some(realtime) = &self.realtime {
            realtime.close();
        }
        // The child dies first — it is what unblocks a worker thread parked on a
        // model download — and the queue closes behind it. Not joined: a cloud
        // request still inside its timeout must not hold the game's exit open.
        if let Some(local) = &self.local {
            local.close();
        }
        let (dead, _) = bounded(0);
        drop(std::mem::replace(&mut self.jobs, dead));
        // The disposal thread ends with its queue too. Not joined either: what
        // it still owes is a handful of unlinks inside a directory `SessionDir`
        // is about to remove wholesale.
        let (dead, _) = bounded(0);
        drop(std::mem::replace(&mut self.discards, dead));
        self.worker.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BackendsConfig, BackendsOptions, Environment},
        events::backend_channel,
        testing::MockServer,
        worker::tests::StubWorker,
    };
    use crossbeam_channel::Receiver;
    use std::{collections::BTreeMap, path::Path, time::Duration};

    fn settings(pairs: &[(&str, &str)], workers_dir: PathBuf, uv: &str) -> SpeechSettings {
        let vars: BTreeMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        BackendsConfig::resolve(
            &Environment::from_map(vars),
            &BackendsOptions {
                dotenv_path: None,
                workers_dir,
                uv_binary: uv.to_string(),
                fake_mode: false,
            },
        )
        .speech
    }

    fn recording(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, b"RIFF____WAVEfmt ").expect("a recording");
        path
    }

    fn next(events: &Receiver<BackendEvent>) -> BackendEvent {
        loop {
            match events
                .recv_timeout(Duration::from_secs(10))
                .expect("a backend event")
            {
                // Worker lifecycle rows are not what these tests are about.
                BackendEvent::Status(_) => continue,
                event => return event,
            }
        }
    }

    /// The cloud path end to end: accept, upload, answer on the channel — and the
    /// player's voice does not stay on disk afterwards.
    #[test]
    fn a_cloud_recording_is_transcribed_and_then_deleted() {
        let server = MockServer::start(vec![MockServer::ok(r#"{"text": "What's your name?"}"#)]);
        let stub_dir = std::env::temp_dir().join(format!(
            "cathedral-stt-engine-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&stub_dir).expect("session dir");
        let path = recording(&stub_dir, "player-recording-1.wav");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut stt = SttEngine::new(runtime, &speech, Some(stub_dir.clone()), sender);
        assert!(stt.available(SttBackendKind::Cloud));
        assert!(!stt.available(SttBackendKind::Local), "no worker script");

        stt.submit_batch(TranscriptionJobId(1), path.clone(), SttBackendKind::Cloud)
            .expect("accepted");

        assert_eq!(
            next(&events),
            BackendEvent::TranscriptionDone {
                job: TranscriptionJobId(1),
                result: Ok("What's your name?".to_string()),
            }
        );
        assert!(
            !path.exists(),
            "the recording is gone once it has been heard"
        );
        std::fs::remove_dir_all(&stub_dir).ok();
    }

    /// The realtime road never hands the WAV to any backend, so the router has to
    /// say when it is done with it — otherwise every utterance of a normal cloud
    /// session (where realtime *is* the default road) stays on disk. And the
    /// probe wants to know how long the recording was: the sim cannot open it.
    ///
    /// The unlink happens on the disposal thread now, so the assertions wait on
    /// the queue rather than on the call — the contract is still "gone", it is
    /// simply not gone *by the time the caller's frame ends*.
    #[test]
    fn the_router_can_measure_and_discard_a_recording_it_never_submitted() {
        let session = std::env::temp_dir().join(format!(
            "cathedral-stt-discard-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&session).expect("session dir");

        // Half a second of the float32 the microphone actually writes.
        let mut wav = Vec::new();
        let data_bytes: u32 = 12_000 * 4;
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&3u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&24_000u32.to_le_bytes());
        wav.extend_from_slice(&96_000u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&32u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, data_bytes as usize));

        let path = session.join("player-recording-9.wav");
        std::fs::write(&path, &wav).expect("a recording");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _events) = backend_channel();
        let speech = settings(&[], PathBuf::from("/nonexistent"), "uv");
        let mut stt = SttEngine::new(runtime, &speech, Some(session.clone()), sender);

        let seconds = stt
            .recording_seconds(Path::new("player-recording-9.wav"))
            .expect("the float32 header the strict decoders will not open");
        assert!((seconds - 0.5).abs() < 1e-9, "{seconds}");

        stt.discard_recording(Path::new("player-recording-9.wav"));
        stt.wait_for_discards();
        assert!(!path.exists(), "the player's voice does not stay in /tmp");
        // Idempotent: the batch worker usually deleted it first.
        stt.discard_recording(Path::new("player-recording-9.wav"));
        stt.wait_for_discards();
        assert_eq!(stt.recording_seconds(&path), None, "gone is not an error");

        std::fs::remove_dir_all(&session).ok();
    }

    /// The probe reads a header, not a recording — but a `data` chunk that a
    /// fat leading chunk has pushed past that prefix must still measure, or an
    /// encoder that writes a `LIST` block first would silently turn every
    /// utterance into `audio=?`.
    #[test]
    fn a_header_beyond_the_probe_window_still_measures() {
        let session = std::env::temp_dir().join(format!(
            "cathedral-stt-header-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&session).expect("session dir");

        // A `LIST` chunk exactly as long as the prefix: the `fmt ` and `data`
        // headers both land past it.
        let filler = HEADER_PROBE_BYTES;
        let data_bytes: u32 = 12_000 * 4;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(4 + 8 + filler as u32 + 24 + 8 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"LIST");
        wav.extend_from_slice(&(filler as u32).to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, filler));
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&3u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&24_000u32.to_le_bytes());
        wav.extend_from_slice(&96_000u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&32u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, data_bytes as usize));
        std::fs::write(session.join("player-recording-8.wav"), &wav).expect("a recording");

        // …and a file too short to hold a chunk table at all is still the
        // `None` the probe prints as `audio=?`.
        recording(&session, "player-recording-7.wav");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _events) = backend_channel();
        let speech = settings(&[], PathBuf::from("/nonexistent"), "uv");
        let stt = SttEngine::new(runtime, &speech, Some(session.clone()), sender);

        let seconds = stt
            .recording_seconds(Path::new("player-recording-8.wav"))
            .expect("the walk continues past the prefix it started with");
        assert!((seconds - 0.5).abs() < 1e-9, "{seconds}");
        assert_eq!(stt.recording_seconds(Path::new("player-recording-7.wav")), None);

        std::fs::remove_dir_all(&session).ok();
    }

    /// A bare basename resolves inside the runtime directory — and only there.
    #[test]
    fn a_basename_resolves_into_the_session_directory_and_cannot_escape_it() {
        let server = MockServer::start(vec![MockServer::ok(r#"{"text": "heard"}"#)]);
        let session = std::env::temp_dir().join(format!(
            "cathedral-stt-basename-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&session).expect("session dir");
        recording(&session, "player-recording-2.wav");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut stt = SttEngine::new(runtime, &speech, Some(session.clone()), sender);

        stt.submit_batch(
            TranscriptionJobId(2),
            PathBuf::from("player-recording-2.wav"),
            SttBackendKind::Cloud,
        )
        .expect("accepted");
        assert_eq!(
            next(&events),
            BackendEvent::TranscriptionDone {
                job: TranscriptionJobId(2),
                result: Ok("heard".to_string()),
            }
        );

        assert_eq!(
            stt.submit_batch(
                TranscriptionJobId(3),
                PathBuf::from("../../etc/passwd.wav"),
                SttBackendKind::Cloud,
            ),
            Err(SttSubmitError::Unavailable),
            "a recording may only ever live in the session directory"
        );
        std::fs::remove_dir_all(&session).ok();
    }

    /// The local path drives the real worker protocol (against a stub).
    #[test]
    fn a_local_recording_goes_to_the_canary_worker() {
        let stub = StubWorker::new(
            "stt-engine-local",
            &[
                r#"{"type":"ready","model":"nvidia/canary-qwen-2.5b","precision":"fp16"}"#,
                r#"{"type":"result","request_id":1,"text":"two coppers"}"#,
            ],
        );
        let speech = settings(
            &[],
            stub.directory.clone(),
            &stub.program.display().to_string(),
        );
        std::fs::rename(&stub.script, speech.canary_script()).expect("worker script");
        let path = recording(&stub.directory, "player-recording-3.wav");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let mut stt = SttEngine::new(runtime, &speech, Some(stub.directory.clone()), sender);
        assert!(stt.available(SttBackendKind::Local));
        assert!(!stt.available(SttBackendKind::Cloud), "no key");

        stt.submit_batch(TranscriptionJobId(4), path.clone(), SttBackendKind::Local)
            .expect("accepted");
        assert_eq!(
            next(&events),
            BackendEvent::TranscriptionDone {
                job: TranscriptionJobId(4),
                result: Ok("two coppers".to_string()),
            }
        );
        assert!(!path.exists());
    }

    /// An unavailable backend refuses at submit time — the router needs the
    /// answer now, to fail the command and tell the player.
    #[test]
    fn an_unavailable_backend_is_refused_synchronously() {
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _events) = backend_channel();
        let speech = settings(&[], PathBuf::from("/nonexistent"), "uv");
        let mut stt = SttEngine::new(runtime, &speech, None, sender);

        assert!(!stt.available(SttBackendKind::Cloud));
        assert!(!stt.available(SttBackendKind::Local));
        assert_eq!(
            stt.submit_batch(
                TranscriptionJobId(1),
                PathBuf::from("/tmp/x.wav"),
                SttBackendKind::Cloud
            ),
            Err(SttSubmitError::Unavailable)
        );
        // And with no key there is no realtime session: every utterance is batch.
        assert!(!stt.realtime_begin("player-recording-1.wav"));
        assert!(!stt.realtime_append("player-recording-1.wav", &[0, 1]));
        assert!(!stt.realtime_commit("player-recording-1.wav"));
        stt.realtime_clear("player-recording-1.wav");
    }

    /// The queue is four deep, and the fifth recording is refused rather than
    /// queued behind a model download.
    #[test]
    fn the_batch_queue_refuses_a_fifth_recording() {
        let server = MockServer::start(vec![MockServer::hang()]);
        let session = std::env::temp_dir().join(format!(
            "cathedral-stt-full-{}",
            crate::session_dir::SessionDir::new_session_id()
        ));
        std::fs::create_dir_all(&session).expect("session dir");

        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, _events) = backend_channel();
        let speech = settings(
            &[
                ("OPENAI_API_KEY", "sk"),
                ("OPENAI_BASE_URL", &server.base_url()),
            ],
            PathBuf::from("/nonexistent"),
            "uv",
        );
        let mut stt = SttEngine::new(runtime, &speech, Some(session.clone()), sender);

        // The provider never answers, so the worker never comes back for more:
        // at most one utterance in its hands and four in the queue can be taken.
        let mut accepted = 0;
        let mut refusals = Vec::new();
        for index in 0..8u64 {
            let path = recording(&session, &format!("player-recording-{index}.wav"));
            match stt.submit_batch(TranscriptionJobId(index), path, SttBackendKind::Cloud) {
                Ok(()) => accepted += 1,
                Err(error) => refusals.push(error),
            }
        }
        assert!(
            accepted <= STT_QUEUE_CAPACITY + 1,
            "four queued plus the one being transcribed: {accepted}"
        );
        assert!(
            refusals
                .iter()
                .all(|error| *error == SttSubmitError::QueueFull),
            "{refusals:?}"
        );
        assert!(!refusals.is_empty(), "the queue has a bottom");

        std::fs::remove_dir_all(&session).ok();
    }
}
