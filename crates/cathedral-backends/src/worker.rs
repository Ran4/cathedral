//! The shared uv-subprocess driver (`speech_client.py:238-898`, common half).
//!
//! Both ML workers — `canary_qwen_worker.py` and `pocket_tts_worker.py` — stay
//! Python: they are the only place the NeMo/Pocket dependency trees live, and
//! they are pinned by their own PEP-723 metadata. Rust drives them through
//! exactly the protocol they already speak:
//!
//! * spawn `uv run --python <version> [flags] --script <worker>`, inheriting the
//!   **whole** parent environment (the workers read `TTS_POCKET_VOICE_*`,
//!   `LOCAL_STT_MODEL`, `HF_HOME`, … themselves — R15);
//! * the first stdout line must be `{"type":"ready", …}`, or the child is
//!   forgotten and the backend degrades;
//! * one JSON object per line, **strictly sequential**: one request in flight,
//!   the caller holds the request lock across write-and-read;
//! * a reply carrying the wrong `request_id` poisons the stream — the child is
//!   killed rather than resynchronized (there is no way to know what the next
//!   line answers);
//! * stderr is forwarded line by line to the session log, and uv's install
//!   progress becomes `loading` status rows for the HUD;
//! * teardown is `SIGTERM → wait 1 s → SIGKILL`, and never takes the request
//!   lock: shutdown must be able to interrupt a worker that is 5 GB into a model
//!   download (`speech_client.py:778-789`).
//!
//! The driver is deliberately **blocking**. Its callers are the STT/TTS engines'
//! own worker threads (`transcription.rs`, `tts.rs`), which is precisely the
//! shape Python had, and it keeps the tokio runtime free for HTTP and the
//! realtime socket.

use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use cathedral_sim::{SpeechError, StatusEvent, Subsystem};
use serde_json::{Map, Value};

use crate::events::BackendSender;

/// `speech_client.py:365-367` — how long a worker gets to exit politely.
const TERMINATE_GRACE: Duration = Duration::from_secs(1);
/// How long a status/error message may be (`speech_client.py:454`).
const MAX_MESSAGE_CHARS: usize = 160;

// ------------------------------------------------------------------- log sink

/// Where a worker's stderr goes. The game installs a sink that writes into
/// `logs/latest_session/logs.jsonl`; everything else gets the process's stderr.
pub type LogSink = Arc<dyn Fn(&str, &str) + Send + Sync>;

static LOG_SINK: OnceLock<LogSink> = OnceLock::new();

/// Route worker stderr into the host's log. Only the first call wins (the game
/// installs it once, at startup).
pub fn set_log_sink(sink: LogSink) {
    let _ = LOG_SINK.set(sink);
}

pub(crate) fn log(source: &str, line: &str) {
    match LOG_SINK.get() {
        Some(sink) => sink(source, line),
        None => eprintln!("{line}"),
    }
}

// ---------------------------------------------------------------------- spec

/// The failure wording of one worker. Python spells these out per backend, and
/// the exact strings reach the player through the HUD's degraded rows — so they
/// are data, not a format string with a name substituted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerMessages {
    /// `uv` itself could not be executed.
    pub start_failed: &'static str,
    /// The worker script is not on disk (or no uv binary is configured).
    pub unavailable: &'static str,
    /// EOF on stdout: the child died.
    pub exited: &'static str,
    /// A line that is not JSON.
    pub invalid_json: &'static str,
    /// A line that is JSON but not an object — or answers the wrong request.
    pub invalid_response: &'static str,
    /// The request could not be written (broken pipe).
    pub write_failed: &'static str,
    /// The handshake line was not `{"type": "ready"}` and carried no error.
    pub load_failed: &'static str,
}

/// Everything needed to start and talk to one worker.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    /// `uv`, or a configured absolute path.
    pub program: String,
    /// Everything after the program: `run --python 3.12 … --script <worker>`.
    pub args: Vec<String>,
    /// Extra variables **on top of** the inherited environment.
    pub env: Vec<(String, String)>,
    /// The worker script, for the availability probe.
    pub script: PathBuf,
    /// Log prefix, e.g. `stt` / `tts`.
    pub log_source: &'static str,
    pub messages: WorkerMessages,
    pub subsystem: Subsystem,
    /// The `backend` field of every status row this worker publishes.
    pub backend: &'static str,
    pub loading_message: &'static str,
    pub ready_message: &'static str,
    /// Turn uv's `Downloading …` / `Building …` / `Installed …` stderr lines
    /// into `loading` statuses. Canary does this (its first run pulls ~5 GB);
    /// Pocket does not (`speech-python.md` §3).
    pub install_progress_statuses: bool,
}

/// What the caller wants done with the message it was just handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerStep {
    /// A streamed intermediate message (a Pocket PCM chunk); read the next one.
    Continue,
    /// The exchange is complete.
    Done,
    /// The exchange failed. `forget` kills the child: use it when the *stream*
    /// is untrustworthy (a malformed chunk), not when only this one reply was
    /// bad (`speech_client.py:341-350` keeps the process on a bad completion).
    Fail { message: String, forget: bool },
}

// -------------------------------------------------------------------- worker

struct WorkerIo {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// One lazily-started, reused ML worker subprocess.
pub struct Worker {
    spec: WorkerSpec,
    events: BackendSender,
    /// The request lock: held across write-then-read, which *is* the protocol.
    io: Mutex<Option<WorkerIo>>,
    /// Separate on purpose — [`Worker::close`] must be able to kill a child that
    /// a request thread is currently blocked on.
    child: Mutex<Option<Child>>,
    next_request_id: AtomicU64,
    spawns: AtomicU64,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Worker")
            .field("program", &self.spec.program)
            .field("script", &self.spec.script)
            .finish_non_exhaustive()
    }
}

impl Worker {
    pub fn new(spec: WorkerSpec, events: BackendSender) -> Self {
        Self {
            spec,
            events,
            io: Mutex::new(None),
            child: Mutex::new(None),
            next_request_id: AtomicU64::new(0),
            spawns: AtomicU64::new(0),
        }
    }

    /// The script exists and a uv binary is configured. No GPU probe, no model
    /// check: load failures surface on first use (`speech_client.py:731-733`).
    pub fn available(&self) -> bool {
        !self.spec.program.trim().is_empty() && self.spec.script.is_file()
    }

    /// How many child processes this worker has started — one, if it is being
    /// reused correctly.
    pub fn spawn_count(&self) -> u64 {
        self.spawns.load(Ordering::SeqCst)
    }

    /// Pay the model-load cost now (`PocketTtsBackend.warm`).
    pub fn warm(&self) -> Result<(), SpeechError> {
        let mut io = self.io.lock().expect("worker io lock");
        self.ensure(&mut io)?;
        Ok(())
    }

    /// One request, its (possibly streamed) reply.
    ///
    /// `body` must be the exact key set the worker expects — both Python workers
    /// reject a request whose key set differs at all (R15). `request_id` is
    /// added here.
    pub fn request(
        &self,
        mut body: Map<String, Value>,
        mut on_message: impl FnMut(&Map<String, Value>) -> WorkerStep,
    ) -> Result<(), SpeechError> {
        let mut io = self.io.lock().expect("worker io lock");
        self.ensure(&mut io)?;

        let request_id = self.next_request_id.fetch_add(1, Ordering::SeqCst) + 1;
        body.insert("request_id".to_string(), Value::from(request_id));
        let line = format!("{}\n", Value::Object(body));

        let write = io
            .as_mut()
            .expect("ensure left a live worker")
            .write_line(&line);
        if write.is_err() {
            self.forget(&mut io);
            return Err(self.error(self.spec.messages.write_failed));
        }

        loop {
            let message = match self.read_message(&mut io) {
                Ok(message) => message,
                Err(error) => {
                    self.forget(&mut io);
                    return Err(error);
                }
            };
            // Replies are strictly sequential: a mismatched id means we can no
            // longer tell which request the *next* line answers.
            if message.get("request_id").and_then(Value::as_u64) != Some(request_id) {
                self.forget(&mut io);
                return Err(self.error(self.spec.messages.invalid_response));
            }
            match on_message(&message) {
                WorkerStep::Continue => continue,
                WorkerStep::Done => return Ok(()),
                WorkerStep::Fail { message, forget } => {
                    if forget {
                        self.forget(&mut io);
                    }
                    return Err(SpeechError::new(truncate(&message, MAX_MESSAGE_CHARS)));
                }
            }
        }
    }

    /// `SIGTERM → wait 1 s → SIGKILL`, without the request lock: shutdown must
    /// interrupt a worker that is mid-download (`speech_client.py:778-789`).
    pub fn close(&self) {
        let child = self.child.lock().expect("worker child lock").take();
        if let Some(child) = child {
            terminate_and_reap(child);
        }
    }

    // ------------------------------------------------------------- internals

    fn ensure(&self, io: &mut Option<WorkerIo>) -> Result<(), SpeechError> {
        if io.is_some() && self.child_is_running() {
            return Ok(());
        }
        *io = None;
        if !self.available() {
            return Err(self.error(self.spec.messages.unavailable));
        }
        self.status("loading", self.spec.loading_message);

        let mut command = Command::new(&self.spec.program);
        command
            .args(&self.spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // The parent environment is inherited wholesale (Python's
        // `os.environ.copy()`); the extras are the per-worker additions.
        for (key, value) in &self.spec.env {
            command.env(key, value);
        }

        let mut child =
            spawn_program(&mut command).map_err(|_| self.error(self.spec.messages.start_failed))?;
        self.spawns.fetch_add(1, Ordering::SeqCst);

        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let stderr = child.stderr.take().expect("piped stderr");
        self.forward_stderr(stderr);

        {
            let mut slot = self.child.lock().expect("worker child lock");
            if let Some(mut previous) = slot.replace(child) {
                let _ = previous.kill();
                let _ = previous.wait();
            }
        }
        *io = Some(WorkerIo { stdin, stdout });

        // The handshake. Anything but `ready` means the model did not load.
        let ready = match self.read_message(io) {
            Ok(message) => message,
            Err(error) => {
                self.forget(io);
                return Err(error);
            }
        };
        if ready.get("type").and_then(Value::as_str) != Some("ready") {
            self.forget(io);
            let reported = ready
                .get("error")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|error| !error.is_empty())
                .map(|error| truncate(error, MAX_MESSAGE_CHARS))
                .unwrap_or_else(|| self.spec.messages.load_failed.to_string());
            return Err(SpeechError::new(reported));
        }
        self.status("ready", self.spec.ready_message);
        Ok(())
    }

    fn read_message(&self, io: &mut Option<WorkerIo>) -> Result<Map<String, Value>, SpeechError> {
        let Some(worker) = io.as_mut() else {
            return Err(self.error(self.spec.messages.exited));
        };
        let mut line = String::new();
        match worker.stdout.read_line(&mut line) {
            Ok(0) | Err(_) => return Err(self.error(self.spec.messages.exited)),
            Ok(_) => {}
        }
        let value: Value = serde_json::from_str(line.trim_end_matches(['\r', '\n']))
            .map_err(|_| self.error(self.spec.messages.invalid_json))?;
        match value {
            Value::Object(message) => Ok(message),
            _ => Err(self.error(self.spec.messages.invalid_response)),
        }
    }

    /// Drop the pipes and kill the child: the next request starts a new one
    /// (`_forget_process`, `speech_client.py:446-450`).
    fn forget(&self, io: &mut Option<WorkerIo>) {
        *io = None;
        let child = self.child.lock().expect("worker child lock").take();
        let Some(child) = child else { return };
        // A dead child is reaped by its *parent*, not by the OS, and `Child` has
        // no `Drop` that waits — a signalled worker we drop here stays a zombie
        // for the rest of the session, one pid per poisoned stream. But we
        // cannot wait for it either: forgetting happens on the request path and
        // a wedged child must not hold a turn. So the corpse goes to a
        // short-lived thread that runs the same `SIGTERM → 1 s → SIGKILL → wait`
        // shutdown does, out of the caller's way.
        let reaper = std::thread::Builder::new()
            .name(format!("{}-worker-reaper", self.spec.log_source))
            .spawn(move || terminate_and_reap(child));
        if reaper.is_err() {
            log(
                self.spec.log_source,
                "could not start a reaper thread; a forgotten worker may linger",
            );
        }
    }

    /// The pid of the child currently held, so a test can follow it into
    /// `/proc` after the worker has let go of it.
    #[cfg(test)]
    fn child_pid(&self) -> Option<u32> {
        self.child
            .lock()
            .expect("worker child lock")
            .as_ref()
            .map(Child::id)
    }

    fn child_is_running(&self) -> bool {
        let mut slot = self.child.lock().expect("worker child lock");
        match slot.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn forward_stderr(&self, stderr: std::process::ChildStderr) {
        let events = self.events.clone();
        let source = self.spec.log_source;
        let subsystem = self.spec.subsystem;
        let backend = self.spec.backend;
        let progress = self.spec.install_progress_statuses;
        std::thread::Builder::new()
            .name(format!("{source}-worker-log"))
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    log(source, &line);
                    if !progress {
                        continue;
                    }
                    // uv's install chatter is the only feedback a first run has;
                    // it is 5 GB of it (`speech_client.py:892-898`).
                    let detail = line.split_whitespace().collect::<Vec<_>>().join(" ");
                    if detail.starts_with("Downloading ")
                        || detail.starts_with("Building ")
                        || detail.starts_with("Installed ")
                    {
                        events.send(StatusEvent {
                            subsystem,
                            state: "loading".to_string(),
                            actor_id: None,
                            message: Some(truncate(&detail, MAX_MESSAGE_CHARS)),
                            backend: Some(backend.to_string()),
                        });
                    }
                }
            })
            .expect("a log-forwarding thread");
    }

    fn status(&self, state: &str, message: &str) {
        self.events.send(StatusEvent {
            subsystem: self.spec.subsystem,
            state: state.to_string(),
            actor_id: None,
            message: Some(truncate(message, MAX_MESSAGE_CHARS)),
            backend: Some(self.spec.backend.to_string()),
        });
    }

    /// Publish a per-request status row (`transcribing` / `synthesizing`).
    pub fn publish_status(&self, state: &str, message: &str) {
        self.status(state, message);
    }

    fn error(&self, message: &str) -> SpeechError {
        SpeechError::new(truncate(message, MAX_MESSAGE_CHARS))
    }
}

impl WorkerIo {
    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.close();
    }
}

/// Spawn, tolerating a transient `ETXTBSY`.
///
/// A program written moments ago (a uv shim, a stub) can refuse to exec while
/// *any* process still holds a write descriptor for it — including a child that
/// another thread has forked but not yet exec'd, which inherited ours. The
/// window is microseconds; a few short retries turn a spurious "could not start
/// the worker" into a normal spawn.
fn spawn_program(command: &mut Command) -> std::io::Result<Child> {
    const ATTEMPTS: u32 = 20;
    for attempt in 1..=ATTEMPTS {
        match command.spawn() {
            Err(error) if is_text_busy(&error) && attempt < ATTEMPTS => {
                std::thread::sleep(Duration::from_millis(10));
            }
            outcome => return outcome,
        }
    }
    command.spawn()
}

fn is_text_busy(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::ETXTBSY)
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

/// A polite request to exit, so the worker can release the GPU. `Child::kill`
/// is SIGKILL, which is the *second* half of the sequence, not the first.
fn terminate(child: &Child) {
    #[cfg(unix)]
    // SAFETY: `kill(2)` with a pid we own; the worst case for a reaped pid is
    // ESRCH, which we ignore.
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    {
        let _ = child;
    }
}

/// The full teardown of one child: `SIGTERM → wait 1 s → SIGKILL`, and then the
/// `wait` that actually collects it. Blocking, so a caller on the request path
/// hands it to a thread ([`Worker::forget`]) rather than running it inline.
fn terminate_and_reap(mut child: Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    terminate(&child);
    let deadline = Instant::now() + TERMINATE_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub(crate) fn truncate(text: &str, characters: usize) -> String {
    text.chars().take(characters).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::events::{BackendEvent, backend_channel};
    use crossbeam_channel::Receiver;
    use std::fs;
    use std::path::Path;

    /// A scripted stand-in for a uv worker: it speaks the same JSON-lines
    /// protocol from a shell script, so the driver's spawn/handshake/poison
    /// paths are exercised for real — with no ML, no GPU and no network.
    ///
    /// `blocks` are separated by `@@`: the first is the handshake, each
    /// following one answers exactly one request. Running out of blocks ends the
    /// process, which is how a dead worker is simulated.
    pub struct StubWorker {
        pub directory: PathBuf,
        pub program: PathBuf,
        pub script: PathBuf,
    }

    impl StubWorker {
        pub fn new(tag: &str, blocks: &[&str]) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "cathedral-worker-{tag}-{}",
                crate::session_dir::SessionDir::new_session_id()
            ));
            fs::create_dir_all(&directory).expect("stub directory");

            let responses = directory.join("responses.txt");
            fs::write(&responses, format!("{}\n@@\n", blocks.join("\n@@\n"))).expect("responses");

            let program = directory.join("stub-uv");
            let script = directory.join("worker.py");
            fs::write(&script, "# the driver only ever checks that this exists\n")
                .expect("worker script");

            // POSIX sh flushes stdout before each read, so the protocol streams.
            let source = format!(
                r#"#!/bin/sh
printf '%s\n' "$@" > "{argv}"
[ -n "$STUB_STDERR" ] && printf '%s\n' "$STUB_STDERR" >&2
[ -n "$STUB_ENV_DUMP" ] && printf 'LOCAL_STT_MODEL=%s\nTTS_POCKET_VOICE_SVEN=%s\n' \
    "$LOCAL_STT_MODEL" "$TTS_POCKET_VOICE_SVEN" > "$STUB_ENV_DUMP"
[ -n "$STUB_IGNORE_TERM" ] && trap '' TERM
exec 3< "{responses}"
emit() {{
    while IFS= read -r out <&3; do
        [ "$out" = "@@" ] && return 0
        printf '%s\n' "$out"
    done
    return 1
}}
emit || exit 0
while IFS= read -r line; do
    printf '%s\n' "$line" >> "{requests}"
    emit || exit 0
done
"#,
                argv = directory.join("argv.txt").display(),
                responses = responses.display(),
                requests = directory.join("requests.jsonl").display(),
            );
            fs::write(&program, source).expect("stub program");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&program, fs::Permissions::from_mode(0o700))
                    .expect("executable stub");
            }

            Self {
                directory,
                program,
                script,
            }
        }

        /// The argv the stub was spawned with (minus the program itself).
        pub fn argv(&self) -> Vec<String> {
            read_lines(&self.directory.join("argv.txt"))
        }

        /// Every request line the driver wrote, in order.
        pub fn requests(&self) -> Vec<Map<String, Value>> {
            read_lines(&self.directory.join("requests.jsonl"))
                .iter()
                .map(|line| serde_json::from_str(line).expect("a JSON request"))
                .collect()
        }

        pub fn spec(&self, messages: WorkerMessages) -> WorkerSpec {
            WorkerSpec {
                program: self.program.display().to_string(),
                args: vec![
                    "run".to_string(),
                    "--python".to_string(),
                    "3.12".to_string(),
                    "--script".to_string(),
                    self.script.display().to_string(),
                ],
                env: Vec::new(),
                script: self.script.clone(),
                log_source: "test",
                messages,
                subsystem: Subsystem::Tts,
                backend: "local",
                loading_message: "loading the stub",
                ready_message: "the stub is ready",
                install_progress_statuses: true,
            }
        }
    }

    impl Drop for StubWorker {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.directory).ok();
        }
    }

    fn read_lines(path: &Path) -> Vec<String> {
        // The stub writes these from another process: a missing file just means
        // nothing was recorded yet.
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    pub const MESSAGES: WorkerMessages = WorkerMessages {
        start_failed: "could not start the stub; make sure uv is available",
        unavailable: "stub worker script is unavailable",
        exited: "stub worker exited; check the actor log",
        invalid_json: "stub returned invalid JSON",
        invalid_response: "stub returned an invalid response",
        write_failed: "stub worker stopped",
        load_failed: "stub failed to load",
    };

    /// Answer with the message's own text, or say what went wrong.
    fn echo(worker: &Worker, body: Map<String, Value>) -> Result<String, SpeechError> {
        let mut text = String::new();
        worker.request(body, |message| {
            match message.get("type").and_then(Value::as_str) {
                Some("result") => {
                    text = message
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    WorkerStep::Done
                }
                _ => WorkerStep::Fail {
                    message: message
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("stub failed")
                        .to_string(),
                    forget: false,
                },
            }
        })?;
        Ok(text)
    }

    fn statuses(events: &Receiver<BackendEvent>) -> Vec<(String, String)> {
        events
            .try_iter()
            .filter_map(|event| match event {
                BackendEvent::Status(status) => {
                    Some((status.state, status.message.unwrap_or_default()))
                }
                _ => None,
            })
            .collect()
    }

    fn body(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    /// speech-python.md test 4: one spawn serves two requests, the ids count
    /// from 1, and the command line is the documented uv invocation.
    #[test]
    fn one_lazily_started_child_serves_every_request() {
        let stub = StubWorker::new(
            "reuse",
            &[
                r#"{"type":"ready","model":"stub","precision":"fp16"}"#,
                r#"{"type":"result","request_id":1,"text":"first local"}"#,
                r#"{"type":"result","request_id":2,"text":"second local"}"#,
            ],
        );
        let (sender, events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);
        assert!(worker.available());
        assert_eq!(
            worker.spawn_count(),
            0,
            "lazy: nothing spawns until first use"
        );

        assert_eq!(
            echo(&worker, body(&[("wav_path", Value::from("/tmp/a.wav"))])).expect("a transcript"),
            "first local"
        );
        assert_eq!(
            echo(&worker, body(&[("wav_path", Value::from("/tmp/a.wav"))])).expect("a transcript"),
            "second local"
        );
        assert_eq!(worker.spawn_count(), 1, "the child is reused");

        assert_eq!(
            stub.argv(),
            vec![
                "run".to_string(),
                "--python".to_string(),
                "3.12".to_string(),
                "--script".to_string(),
                stub.script.display().to_string(),
            ]
        );
        let requests = stub.requests();
        assert_eq!(
            requests
                .iter()
                .map(|request| request["request_id"].as_u64().expect("an id"))
                .collect::<Vec<_>>(),
            vec![1, 2],
            "request ids count from 1"
        );
        assert_eq!(requests[0]["wav_path"], "/tmp/a.wav");

        let states: Vec<String> = statuses(&events)
            .into_iter()
            .map(|(state, _)| state)
            .collect();
        assert_eq!(states, vec!["loading", "ready"], "one handshake, one pill");

        worker.close();
        assert!(!worker.child_is_running());
    }

    /// A worker that never says `ready` is a degraded backend, not a crash, and
    /// its own error text is what the player sees.
    #[test]
    fn a_worker_that_fails_to_load_reports_its_own_error() {
        let stub = StubWorker::new(
            "load-failure",
            &[r#"{"type":"fatal","error":"local Canary-Qwen failed to load; check CUDA"}"#],
        );
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);

        let error = echo(&worker, body(&[])).expect_err("no ready line");
        assert_eq!(
            error.presentable,
            "local Canary-Qwen failed to load; check CUDA"
        );
        assert!(!worker.child_is_running(), "the child is forgotten");

        // A handshake with no error at all falls back to the generic wording.
        let stub = StubWorker::new("load-silent", &[r#"{"type":"bogus"}"#]);
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);
        assert_eq!(
            echo(&worker, body(&[]))
                .expect_err("no ready line")
                .presentable,
            "stub failed to load"
        );
    }

    /// speech-python.md test 7: a worker that dies after `ready` is an
    /// `Unavailable`, never a panic — and the next request restarts it.
    #[test]
    fn a_dead_worker_is_a_safe_failure_and_the_next_request_restarts_it() {
        let stub = StubWorker::new("dead", &[r#"{"type":"ready"}"#]);
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);

        assert_eq!(
            echo(&worker, body(&[]))
                .expect_err("the worker exits")
                .presentable,
            "stub worker exited; check the actor log"
        );
        assert_eq!(worker.spawn_count(), 1);
        // Forgotten, so the next call spawns again (and fails again, identically).
        assert!(echo(&worker, body(&[])).is_err());
        assert_eq!(worker.spawn_count(), 2, "restarted, not wedged");
    }

    /// A reply carrying the wrong id poisons the stream: there is no way to know
    /// what the next line answers, so the child dies.
    #[test]
    fn a_mismatched_request_id_kills_the_child() {
        let stub = StubWorker::new(
            "poison",
            &[
                r#"{"type":"ready"}"#,
                r#"{"type":"result","request_id":99,"text":"for someone else"}"#,
            ],
        );
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);

        assert_eq!(
            echo(&worker, body(&[]))
                .expect_err("a poisoned stream")
                .presentable,
            "stub returned an invalid response"
        );
        assert!(!worker.child_is_running());
    }

    /// …and it is *reaped*, not merely signalled. On unix a dead child stays a
    /// zombie until its parent collects it, so a session that poisons the stream
    /// once per spoken line would otherwise leak a pid apiece.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_forgotten_child_leaves_no_zombie() {
        let stub = StubWorker::new(
            "zombie",
            &[
                r#"{"type":"ready"}"#,
                r#"{"type":"result","request_id":99,"text":"for someone else"}"#,
            ],
        );
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);
        worker.warm().expect("ready");
        let pid = worker.child_pid().expect("a spawned child");

        // The poison path: the child is certainly still alive when it is let go.
        echo(&worker, body(&[])).expect_err("a poisoned stream");
        assert!(worker.child_pid().is_none(), "the child is let go");

        // The pid disappears from /proc only once somebody has waited on it; an
        // unreaped one sits at `Z` forever.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if process_state(pid).is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "pid {pid} is still around as {:?} — nobody reaped it",
            process_state(pid)
        );
    }

    /// The one-letter run state of a pid (`R`, `S`, `Z`, …), or `None` once the
    /// process is gone. The `comm` field can hold spaces and parentheses, so the
    /// state is read from after the *last* `)`.
    #[cfg(target_os = "linux")]
    fn process_state(pid: u32) -> Option<char> {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (_, rest) = stat.rsplit_once(')')?;
        rest.split_whitespace().next()?.chars().next()
    }

    #[test]
    fn a_line_that_is_not_a_json_object_is_an_invalid_response() {
        let stub = StubWorker::new("garbage", &[r#"{"type":"ready"}"#, "not json at all"]);
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);
        assert_eq!(
            echo(&worker, body(&[])).expect_err("garbage").presentable,
            "stub returned invalid JSON"
        );

        let stub = StubWorker::new("array", &[r#"{"type":"ready"}"#, "[1, 2, 3]"]);
        let (sender, _events) = backend_channel();
        let worker = Worker::new(stub.spec(MESSAGES), sender);
        assert_eq!(
            echo(&worker, body(&[])).expect_err("an array").presentable,
            "stub returned an invalid response"
        );
    }

    #[test]
    fn a_missing_script_or_uv_binary_is_unavailable_without_a_spawn() {
        let stub = StubWorker::new("missing", &[r#"{"type":"ready"}"#]);
        let mut spec = stub.spec(MESSAGES);
        spec.script = stub.directory.join("not-there.py");
        let (sender, _events) = backend_channel();
        let worker = Worker::new(spec, sender);
        assert!(!worker.available());
        assert_eq!(
            echo(&worker, body(&[])).expect_err("no script").presentable,
            "stub worker script is unavailable"
        );
        assert_eq!(worker.spawn_count(), 0);

        let mut spec = stub.spec(MESSAGES);
        spec.program = "   ".to_string();
        let (sender, _events) = backend_channel();
        let worker = Worker::new(spec, sender);
        assert!(!worker.available());
        assert!(echo(&worker, body(&[])).is_err());
        assert_eq!(worker.spawn_count(), 0);
    }

    #[test]
    fn an_unstartable_uv_binary_is_a_presentable_failure() {
        let stub = StubWorker::new("no-uv", &[r#"{"type":"ready"}"#]);
        let mut spec = stub.spec(MESSAGES);
        // Exists as a path (the script does), but is not executable as a program.
        spec.program = stub.script.display().to_string();
        let (sender, _events) = backend_channel();
        let worker = Worker::new(spec, sender);
        assert_eq!(
            echo(&worker, body(&[]))
                .expect_err("uv is not there")
                .presentable,
            "could not start the stub; make sure uv is available"
        );
    }

    /// The extra environment reaches the child, and so does the parent's
    /// (R15: `TTS_POCKET_VOICE_*` is read *by the worker*, not by us).
    #[test]
    fn the_child_inherits_the_parent_environment_plus_the_extras() {
        let stub = StubWorker::new("env", &[r#"{"type":"ready"}"#]);
        let dump = stub.directory.join("env.txt");
        let mut spec = stub.spec(MESSAGES);
        spec.env = vec![
            (
                "LOCAL_STT_MODEL".to_string(),
                "nvidia/canary-qwen-2.5b".to_string(),
            ),
            ("STUB_ENV_DUMP".to_string(), dump.display().to_string()),
            ("TTS_POCKET_VOICE_SVEN".to_string(), "michael".to_string()),
        ];
        let (sender, _events) = backend_channel();
        let worker = Worker::new(spec, sender);
        worker.warm().expect("the stub is ready");

        let dumped = fs::read_to_string(&dump).expect("the child wrote its environment");
        assert!(
            dumped.contains("LOCAL_STT_MODEL=nvidia/canary-qwen-2.5b"),
            "{dumped}"
        );
        assert!(dumped.contains("TTS_POCKET_VOICE_SVEN=michael"), "{dumped}");
    }

    /// uv's install chatter is the only progress a 5 GB first run has.
    #[test]
    fn install_progress_on_stderr_becomes_a_loading_status() {
        let stub = StubWorker::new("progress", &[r#"{"type":"ready"}"#]);
        let mut spec = stub.spec(MESSAGES);
        spec.env = vec![(
            "STUB_STDERR".to_string(),
            "Downloading torch (766.6MiB)".to_string(),
        )];
        let (sender, events) = backend_channel();
        let worker = Worker::new(spec, sender);
        worker.warm().expect("ready");

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            seen.extend(statuses(&events));
            if seen
                .iter()
                .any(|(_, message)| message.starts_with("Downloading torch"))
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no download progress reached the HUD: {seen:?}");
    }

    /// Shutdown must be able to interrupt a worker that ignores a polite ask —
    /// a model download can hold SIGTERM for minutes.
    #[test]
    fn close_kills_a_worker_that_ignores_sigterm() {
        let stub = StubWorker::new("stubborn", &[r#"{"type":"ready"}"#]);
        let mut spec = stub.spec(MESSAGES);
        spec.env = vec![("STUB_IGNORE_TERM".to_string(), "1".to_string())];
        let (sender, _events) = backend_channel();
        let worker = Worker::new(spec, sender);
        worker.warm().expect("ready");
        assert!(worker.child_is_running());

        let started = Instant::now();
        worker.close();
        let elapsed = started.elapsed();
        assert!(
            elapsed >= TERMINATE_GRACE && elapsed < Duration::from_secs(3),
            "terminate, wait one second, then kill: {elapsed:?}"
        );
        assert!(!worker.child_is_running(), "the child is gone");
    }
}
