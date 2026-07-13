//! Realtime transcription over one warm websocket
//! (`RealtimeTranscriptionSession`, `speech_client.py:966-1322`).
//!
//! The player's microphone streams 24 kHz PCM to the provider *while he is still
//! speaking*, so the transcript is usually there the moment he lets go of the
//! key. Everything about this file is written for the case where that does not
//! happen: **a realtime failure is never an error, it is a fallback to batch.**
//! Every method answers `bool`, every socket death fails the utterances it was
//! carrying (once, by name) and the recording — which is on disk regardless —
//! goes the slow way instead.
//!
//! The state machine is Python's, moved onto one tokio task:
//!
//! * `begin` → `input_audio_buffer.clear` (a fresh utterance never inherits a
//!   half-full provider buffer);
//! * `append` → `input_audio_buffer.append` with base64 PCM;
//! * `commit` → `input_audio_buffer.commit`, which the provider acknowledges
//!   with an `item_id` that **binds** to the utterance key. Completions are
//!   matched by that id and never by arrival order — they come back out of order
//!   and Python has a test for it.
//!
//! Connect failures back off exponentially (capped at 30 s) and reject `begin`
//! instantly while the window is open, so a dead endpoint costs the player
//! nothing but the batch latency he would have paid anyway. Error text is
//! scrubbed of the API key before it ever reaches a status row.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use cathedral_sim::{RealtimeResult, StatusEvent, Subsystem};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::{
    config::{RealtimeSettings, SpeechSettings},
    events::BackendSender,
    runtime::BackendRuntime,
    worker::truncate,
};

/// `speech_client.py:1000` — the writer's bounded action queue. A full queue is
/// backpressure, and backpressure is a degrade to batch.
const ACTION_QUEUE_CAPACITY: usize = 512;
/// `speech_client.py:1198` — the connect-backoff ceiling.
const MAX_BACKOFF_SECONDS: f64 = 30.0;
/// `speech_client.py:1266` — a provider transcript is not a novel.
const MAX_TRANSCRIPT_CHARS: usize = 2_000;
/// How often the task re-checks the idle deadline. Python polled this from the
/// protocol loop; a tick keeps the same semantics without the sim owning a clock.
const IDLE_TICK: Duration = Duration::from_millis(250);
/// Status/error text ceiling (`speech_client.py:1313`).
const MAX_MESSAGE_CHARS: usize = 160;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The duplex socket, abstracted so the tests can drive the whole state machine
/// in-process — no network, no TLS, no provider.
pub trait RealtimeTransport: Send {
    fn send(&mut self, text: String) -> BoxFuture<'_, Result<(), String>>;
    /// `Ok("")` means "nothing right now"; `Err` means the socket is dead.
    fn recv(&mut self) -> BoxFuture<'_, Result<String, String>>;
    /// Consuming, and awaited on a detached task: a provider close that wedges
    /// must never hold up shutdown (`speech_client.py:1295-1309`).
    fn close(self: Box<Self>) -> BoxFuture<'static, ()>;
}

/// Opens one socket. `Err` carries an already-scrubbed reason.
pub type TransportFactory =
    Arc<dyn Fn() -> BoxFuture<'static, Result<Box<dyn RealtimeTransport>, String>> + Send + Sync>;

/// Monotonic seconds. Injectable so the backoff and idle windows are testable
/// without sleeping through them.
pub type Clock = Arc<dyn Fn() -> f64 + Send + Sync>;

/// The default clock: seconds since the process's first call.
pub fn monotonic_clock() -> Clock {
    let origin = Instant::now();
    Arc::new(move || origin.elapsed().as_secs_f64())
}

// ---------------------------------------------------------------------- state

#[derive(Debug, Default)]
struct SessionState {
    /// The utterance currently appending audio.
    active_key: Option<String>,
    /// Committed, awaiting the provider's `item_id` acknowledgement (FIFO).
    pending_commits: VecDeque<String>,
    /// Acknowledged, awaiting a transcript: `item_id → key`.
    items: HashMap<String, String>,
    failures: u32,
    retry_at: f64,
    last_used: f64,
    connected: bool,
}

impl SessionState {
    fn in_flight(&self) -> usize {
        self.pending_commits.len() + self.items.len()
    }

    fn is_idle(&self) -> bool {
        self.active_key.is_none() && self.pending_commits.is_empty() && self.items.is_empty()
    }
}

#[derive(Debug)]
enum Action {
    Send(String),
    Close,
}

// --------------------------------------------------------------------- handle

/// The sync side. Every method is non-blocking and answers `bool`: `false`
/// means "not streaming — use batch", never an error.
pub struct RealtimeSttHandle {
    state: Arc<Mutex<SessionState>>,
    actions: mpsc::Sender<Action>,
    closing: Arc<AtomicBool>,
    clock: Clock,
    max_in_flight: usize,
}

impl std::fmt::Debug for RealtimeSttHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RealtimeSttHandle")
            .field("closing", &self.closing.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl RealtimeSttHandle {
    /// The production session: a websocket to the configured provider.
    ///
    /// `None` without an `OPENAI_API_KEY` — the same non-probe as everywhere
    /// else (`speech_client.py:1017-1021`). Fake mode never calls this: an
    /// offline run must not open a socket.
    pub fn connect(
        runtime: &Arc<BackendRuntime>,
        settings: &SpeechSettings,
        events: BackendSender,
    ) -> Option<Self> {
        let api_key = settings.api_key.clone()?;
        let url = settings.realtime.url.clone();
        let key = api_key.clone();
        let factory: TransportFactory = Arc::new(move || {
            let url = url.clone();
            let key = key.clone();
            Box::pin(async move { WebsocketTransport::open(&url, &key).await })
        });
        Some(Self::with_transport(
            runtime,
            settings.realtime.clone(),
            Some(api_key),
            factory,
            monotonic_clock(),
            events,
        ))
    }

    /// The testable core.
    pub fn with_transport(
        runtime: &Arc<BackendRuntime>,
        settings: RealtimeSettings,
        api_key: Option<String>,
        factory: TransportFactory,
        clock: Clock,
        events: BackendSender,
    ) -> Self {
        let (actions, inbox) = mpsc::channel(ACTION_QUEUE_CAPACITY);
        let state = Arc::new(Mutex::new(SessionState::default()));
        let closing = Arc::new(AtomicBool::new(false));
        let max_in_flight = settings.max_in_flight;

        let task = SessionTask {
            state: Arc::clone(&state),
            closing: Arc::clone(&closing),
            clock: Arc::clone(&clock),
            settings,
            api_key,
            factory,
            events,
        };
        runtime.spawn(task.run(inbox));

        Self {
            state,
            actions,
            closing,
            clock,
            max_in_flight,
        }
    }

    /// Start streaming one utterance. `false` = fall straight back to batch.
    pub fn begin(&self, key: &str) -> bool {
        let now = (self.clock)();
        {
            let mut state = self.state.lock().expect("realtime state");
            if self.closing.load(Ordering::SeqCst) {
                return false;
            }
            // A recent connect failure is still backing off: reject instantly
            // rather than queue work that is going to fail anyway.
            if !state.connected && now < state.retry_at {
                return false;
            }
            state.active_key = Some(key.to_string());
            state.last_used = now;
        }
        self.enqueue(json!({"type": "input_audio_buffer.clear"}))
    }

    /// One 24 kHz mono chunk. Base64 lives on this wire and nowhere else.
    pub fn append(&self, key: &str, samples: &[i16]) -> bool {
        {
            let state = self.state.lock().expect("realtime state");
            if self.closing.load(Ordering::SeqCst) || state.active_key.as_deref() != Some(key) {
                return false;
            }
        }
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.enqueue(json!({
            "type": "input_audio_buffer.append",
            "audio": STANDARD.encode(&bytes),
        }))
    }

    /// End the utterance. Clears the active key **whether or not** the commit is
    /// accepted: the utterance is over either way (`speech_client.py:1065-1081`).
    pub fn commit(&self, key: &str) -> bool {
        {
            let mut state = self.state.lock().expect("realtime state");
            if self.closing.load(Ordering::SeqCst) || state.active_key.as_deref() != Some(key) {
                return false;
            }
            state.active_key = None;
            if state.in_flight() >= self.max_in_flight {
                return false;
            }
            state.pending_commits.push_back(key.to_string());
            state.last_used = (self.clock)();
        }
        if self.enqueue(json!({"type": "input_audio_buffer.commit"})) {
            return true;
        }
        // The queue was full: roll the pending entry back, or the utterance
        // would sit in flight forever.
        let mut state = self.state.lock().expect("realtime state");
        if let Some(position) = state.pending_commits.iter().position(|entry| entry == key) {
            state.pending_commits.remove(position);
        }
        false
    }

    /// Forget an utterance entirely (aborted, or gone to batch).
    pub fn clear(&self, key: &str) {
        let was_active = {
            let mut state = self.state.lock().expect("realtime state");
            let was_active = state.active_key.as_deref() == Some(key);
            if was_active {
                state.active_key = None;
            }
            if let Some(position) = state.pending_commits.iter().position(|entry| entry == key) {
                state.pending_commits.remove(position);
            }
            state.items.retain(|_, item_key| item_key != key);
            was_active
        };
        if was_active {
            self.enqueue(json!({"type": "input_audio_buffer.clear"}));
        }
    }

    /// Stop the session. Returns immediately — the socket is closed on a
    /// detached task, so a wedged provider cannot hold up the game's exit.
    pub fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        let _ = self.actions.try_send(Action::Close);
    }

    fn enqueue(&self, message: Value) -> bool {
        self.actions
            .try_send(Action::Send(message.to_string()))
            .is_ok()
    }
}

impl Drop for RealtimeSttHandle {
    fn drop(&mut self) {
        self.close();
    }
}

// ----------------------------------------------------------------------- task

struct SessionTask {
    state: Arc<Mutex<SessionState>>,
    closing: Arc<AtomicBool>,
    clock: Clock,
    settings: RealtimeSettings,
    api_key: Option<String>,
    factory: TransportFactory,
    events: BackendSender,
}

enum Next {
    Action(Option<Action>),
    Frame(Result<String, String>),
    Tick,
}

impl SessionTask {
    async fn run(self, mut inbox: mpsc::Receiver<Action>) {
        let mut transport: Option<Box<dyn RealtimeTransport>> = None;

        loop {
            let next = match transport.as_mut() {
                Some(socket) => tokio::select! {
                    action = inbox.recv() => Next::Action(action),
                    frame = socket.recv() => Next::Frame(frame),
                    () = tokio::time::sleep(IDLE_TICK) => Next::Tick,
                },
                None => Next::Action(inbox.recv().await),
            };

            match next {
                // The handle is gone, or asked us to stop.
                Next::Action(None) | Next::Action(Some(Action::Close)) => {
                    self.detach_close(transport.take());
                    return;
                }
                Next::Action(Some(Action::Send(payload))) => {
                    if transport.is_none() {
                        match self.connect().await {
                            Some(socket) => transport = Some(socket),
                            None => {
                                // Nothing to send it on, and nothing coming:
                                // every live utterance goes to batch.
                                self.fail_pending("connect_failed");
                                continue;
                            }
                        }
                    }
                    let socket = transport.as_mut().expect("just connected");
                    if socket.send(payload).await.is_err() {
                        self.detach_close(transport.take());
                        self.fail_pending("socket");
                    }
                }
                Next::Frame(Ok(raw)) => {
                    if !raw.is_empty() {
                        self.handle_provider_event(&raw);
                    }
                }
                Next::Frame(Err(_)) => {
                    // The reader died. Python distinguishes a *stale* reader
                    // (its transport was already replaced) from this one; here
                    // there is only ever one socket, so there is nothing stale.
                    self.detach_close(transport.take());
                    if !self.closing.load(Ordering::SeqCst) {
                        self.fail_pending("socket");
                    }
                }
                Next::Tick => {
                    if self.idle_expired() {
                        // A deliberate idle close surfaces no failures: there is
                        // nothing in flight, by definition.
                        self.detach_close(transport.take());
                    }
                }
            }
        }
    }

    /// `_ensure_connected` (`speech_client.py:1184-1216`).
    async fn connect(&self) -> Option<Box<dyn RealtimeTransport>> {
        {
            let state = self.state.lock().expect("realtime state");
            if (self.clock)() < state.retry_at {
                return None;
            }
        }
        self.status("loading", "Connecting realtime transcription session");

        let outcome = async {
            let mut transport = (self.factory)().await?;
            // The session config is the very first frame on every socket.
            transport
                .send(self.session_config().to_string())
                .await
                .map_err(|error| error.to_string())?;
            Ok::<_, String>(transport)
        }
        .await;

        match outcome {
            Ok(transport) => {
                let mut state = self.state.lock().expect("realtime state");
                state.failures = 0;
                state.connected = true;
                drop(state);
                self.status("ready", "Realtime transcription session connected");
                Some(transport)
            }
            Err(reason) => {
                let mut state = self.state.lock().expect("realtime state");
                state.failures += 1;
                state.connected = false;
                let backoff = 2f64.powi(state.failures as i32).min(MAX_BACKOFF_SECONDS);
                state.retry_at = (self.clock)() + backoff;
                drop(state);
                self.status("degraded", &self.scrub(&reason));
                None
            }
        }
    }

    /// The `session.update` frame (`speech_client.py:1023-1041`).
    ///
    /// Documentation-volatile: model, delay and the shape itself are pinned by a
    /// unit test *and* by config, so drift degrades to batch instead of breaking
    /// (R16). `turn_detection: null` is explicit — endpointing is the game's job.
    fn session_config(&self) -> Value {
        let mut transcription = serde_json::Map::new();
        transcription.insert(
            "model".to_string(),
            Value::from(self.settings.model.clone()),
        );
        transcription.insert(
            "delay".to_string(),
            Value::from(self.settings.delay.clone()),
        );
        if let Some(language) = &self.settings.language {
            transcription.insert("language".to_string(), Value::from(language.clone()));
        }
        json!({
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": {"type": "audio/pcm", "rate": 24_000},
                        "transcription": transcription,
                        "turn_detection": Value::Null,
                    }
                },
            },
        })
    }

    /// `_handle_provider_event` (`speech_client.py:1237-1274`).
    fn handle_provider_event(&self, raw: &str) {
        let Ok(Value::Object(event)) = serde_json::from_str::<Value>(raw) else {
            return;
        };
        match event.get("type").and_then(Value::as_str) {
            Some("input_audio_buffer.committed") => {
                let item_id = event.get("item_id").and_then(Value::as_str);
                let orphan = {
                    let mut state = self.state.lock().expect("realtime state");
                    let Some(key) = state.pending_commits.pop_front() else {
                        // No commit is outstanding: not ours.
                        return;
                    };
                    match item_id.filter(|id| !id.is_empty()) {
                        Some(item_id) => {
                            state.items.insert(item_id.to_string(), key);
                            None
                        }
                        // An acknowledgement with no id can never be matched to
                        // a transcript: fail the utterance now, not in silence.
                        None => Some(key),
                    }
                };
                if let Some(key) = orphan {
                    self.result(RealtimeResult::Failure {
                        key: Some(key),
                        reason: "protocol".to_string(),
                    });
                }
            }
            Some("conversation.item.input_audio_transcription.completed") => {
                let Some(item_id) = event.get("item_id").and_then(Value::as_str) else {
                    return;
                };
                let key = {
                    let mut state = self.state.lock().expect("realtime state");
                    state.items.remove(item_id)
                };
                // Unknown or already-abandoned item: a late completion after a
                // batch fallback is discarded here, never a second `say`.
                let Some(key) = key else { return };

                match event.get("transcript").and_then(Value::as_str) {
                    Some(transcript) => self.result(RealtimeResult::Transcript {
                        key,
                        text: truncate(transcript, MAX_TRANSCRIPT_CHARS),
                    }),
                    None => self.result(RealtimeResult::Failure {
                        key: Some(key),
                        reason: "protocol".to_string(),
                    }),
                }
            }
            Some("error") => {
                let message = event
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("realtime transcription provider error")
                    .to_string();
                // A provider error row degrades the pill; it does not fail keys
                // (the session may well recover on the next utterance).
                self.status("degraded", &self.scrub(&message));
            }
            _ => {}
        }
    }

    /// `_fail_pending` (`speech_client.py:1276-1293`): every live utterance goes
    /// to batch, each named exactly once, in the order they were spoken.
    fn fail_pending(&self, reason: &str) {
        let keys = {
            let mut state = self.state.lock().expect("realtime state");
            let mut keys: Vec<String> = Vec::new();
            if let Some(active) = state.active_key.take() {
                keys.push(active);
            }
            keys.extend(state.pending_commits.drain(..));
            keys.extend(state.items.values().cloned());
            state.items.clear();
            state.connected = false;
            keys
        };

        let mut seen = HashSet::new();
        let unique: Vec<String> = keys
            .into_iter()
            .filter(|key| seen.insert(key.clone()))
            .collect();
        for key in &unique {
            self.result(RealtimeResult::Failure {
                key: Some(key.clone()),
                reason: reason.to_string(),
            });
        }
        if !unique.is_empty() {
            self.status(
                "degraded",
                &format!("realtime transcription dropped ({reason}); using batch fallback"),
            );
        }
    }

    fn idle_expired(&self) -> bool {
        let state = self.state.lock().expect("realtime state");
        state.connected
            && state.is_idle()
            && (self.clock)() - state.last_used > self.settings.idle_close_seconds
    }

    fn detach_close(&self, transport: Option<Box<dyn RealtimeTransport>>) {
        {
            let mut state = self.state.lock().expect("realtime state");
            state.connected = false;
        }
        if let Some(transport) = transport {
            tokio::spawn(transport.close());
        }
    }

    /// `_scrubbed_reason` (`speech_client.py:958-963`) — an error message that
    /// echoes the request must never carry the key into a log or the HUD.
    fn scrub(&self, message: &str) -> String {
        let mut message = truncate(message, MAX_MESSAGE_CHARS);
        if let Some(key) = self.api_key.as_deref().filter(|key| !key.is_empty()) {
            message = message.replace(key, "***");
        }
        message
    }

    fn result(&self, result: RealtimeResult) {
        self.events
            .send(crate::events::BackendEvent::RealtimeResult(result));
    }

    fn status(&self, state: &str, message: &str) {
        self.events.send(StatusEvent {
            subsystem: Subsystem::Stt,
            state: state.to_string(),
            actor_id: None,
            message: Some(truncate(message, MAX_MESSAGE_CHARS)),
            backend: Some("cloud".to_string()),
        });
    }
}

// ------------------------------------------------------------ real websocket

struct WebsocketTransport {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
}

impl WebsocketTransport {
    async fn open(url: &str, api_key: &str) -> Result<Box<dyn RealtimeTransport>, String> {
        use tokio_tungstenite::tungstenite::{
            client::IntoClientRequest,
            http::{HeaderValue, header::AUTHORIZATION},
        };

        let mut request = url
            .into_client_request()
            .map_err(|error| format!("{}: {error}", error_kind(&error)))?;
        let bearer = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| "InvalidHeader: the API key is not a valid header value".to_string())?;
        request.headers_mut().insert(AUTHORIZATION, bearer);

        let (socket, _response) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|error| format!("{}: {error}", error_kind(&error)))?;
        Ok(Box::new(Self { socket }))
    }
}

fn error_kind<E>(_error: &E) -> &'static str {
    // Python published `type(error).__name__`; tungstenite's variants are not
    // worth enumerating — what matters is that the text is scrubbed and short.
    "RealtimeError"
}

impl RealtimeTransport for WebsocketTransport {
    fn send(&mut self, text: String) -> BoxFuture<'_, Result<(), String>> {
        use futures_util::SinkExt;
        Box::pin(async move {
            self.socket
                .send(tokio_tungstenite::tungstenite::Message::Text(text.into()))
                .await
                .map_err(|error| error.to_string())
        })
    }

    fn recv(&mut self) -> BoxFuture<'_, Result<String, String>> {
        use futures_util::StreamExt;
        use tokio_tungstenite::tungstenite::Message;
        Box::pin(async move {
            match self.socket.next().await {
                Some(Ok(Message::Text(text))) => Ok(text.to_string()),
                // Pings/pongs/binary frames are not protocol events; tungstenite
                // answers pings itself.
                Some(Ok(_)) => Ok(String::new()),
                Some(Err(error)) => Err(error.to_string()),
                None => Err("the realtime socket closed".to_string()),
            }
        })
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            let mut socket = self.socket;
            // A courteous close frame; a wedged provider just gets dropped.
            let _ = socket.close(None).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{BackendEvent, backend_channel};
    use crossbeam_channel::Receiver;

    /// A scripted duplex transport: sends are recorded, `recv` blocks on a
    /// queue. The whole realtime state machine runs against it — in-process, no
    /// socket anywhere.
    struct FakeTransport {
        sent: Arc<Mutex<Vec<Value>>>,
        /// A tokio channel, not a blocking one: `recv` is dropped and recreated
        /// on every loop of the session task, so it has to be cancel-safe or a
        /// pushed frame would vanish with the future that was reading it.
        incoming: mpsc::UnboundedReceiver<Option<String>>,
        closed: Arc<AtomicBool>,
        /// Sleep this long inside `close` — a wedged provider.
        close_delay: Duration,
    }

    #[derive(Clone)]
    struct FakeSocket {
        sent: Arc<Mutex<Vec<Value>>>,
        incoming: mpsc::UnboundedSender<Option<String>>,
        closed: Arc<AtomicBool>,
    }

    impl FakeSocket {
        fn sent_types(&self) -> Vec<String> {
            self.sent
                .lock()
                .expect("sent")
                .iter()
                .map(|message| message["type"].as_str().unwrap_or_default().to_string())
                .collect()
        }

        fn sent(&self) -> Vec<Value> {
            self.sent.lock().expect("sent").clone()
        }

        fn push(&self, event: Value) {
            let _ = self.incoming.send(Some(event.to_string()));
        }

        /// Kill the socket under the session.
        fn kill(&self) {
            let _ = self.incoming.send(None);
        }
    }

    impl RealtimeTransport for FakeTransport {
        fn send(&mut self, text: String) -> BoxFuture<'_, Result<(), String>> {
            Box::pin(async move {
                let value: Value = serde_json::from_str(&text).expect("we only send JSON");
                self.sent.lock().expect("sent").push(value);
                Ok(())
            })
        }

        fn recv(&mut self) -> BoxFuture<'_, Result<String, String>> {
            Box::pin(async move {
                match self.incoming.recv().await {
                    Some(Some(frame)) => Ok(frame),
                    _ => Err("socket closed".to_string()),
                }
            })
        }

        fn close(self: Box<Self>) -> BoxFuture<'static, ()> {
            Box::pin(async move {
                tokio::time::sleep(self.close_delay).await;
                self.closed.store(true, Ordering::SeqCst);
            })
        }
    }

    /// Hands out transports, and can be told to refuse the first `n` connects.
    struct Factory {
        sockets: Arc<Mutex<Vec<FakeSocket>>>,
        calls: Arc<Mutex<usize>>,
        fail_connects: usize,
        close_delay: Duration,
        leak_key: Option<String>,
    }

    impl Factory {
        fn new() -> Self {
            Self {
                sockets: Arc::new(Mutex::new(Vec::new())),
                calls: Arc::new(Mutex::new(0)),
                fail_connects: 0,
                close_delay: Duration::ZERO,
                leak_key: None,
            }
        }

        fn failing(count: usize) -> Self {
            Self {
                fail_connects: count,
                ..Self::new()
            }
        }

        fn build(self) -> (TransportFactory, Probe) {
            let sockets = Arc::clone(&self.sockets);
            let calls = Arc::clone(&self.calls);
            let probe = Probe {
                sockets: Arc::clone(&sockets),
                calls: Arc::clone(&calls),
            };
            let fail_connects = self.fail_connects;
            let close_delay = self.close_delay;
            let leak_key = self.leak_key.clone();

            let factory: TransportFactory = Arc::new(move || {
                let sockets = Arc::clone(&sockets);
                let calls = Arc::clone(&calls);
                let leak_key = leak_key.clone();
                Box::pin(async move {
                    let attempt = {
                        let mut calls = calls.lock().expect("calls");
                        *calls += 1;
                        *calls
                    };
                    if attempt <= fail_connects {
                        let reason = match &leak_key {
                            // A provider that echoes the bearer token back in its
                            // error text — the exact leak the scrubber exists for.
                            Some(key) => {
                                format!("ConnectionError: handshake rejected for bearer {key}")
                            }
                            None => "ConnectionError: connect refused".to_string(),
                        };
                        return Err(reason);
                    }
                    let (sender, receiver) = mpsc::unbounded_channel();
                    let socket = FakeSocket {
                        sent: Arc::new(Mutex::new(Vec::new())),
                        incoming: sender,
                        closed: Arc::new(AtomicBool::new(false)),
                    };
                    sockets.lock().expect("sockets").push(socket.clone());
                    Ok(Box::new(FakeTransport {
                        sent: socket.sent,
                        incoming: receiver,
                        closed: socket.closed,
                        close_delay,
                    }) as Box<dyn RealtimeTransport>)
                })
            });
            (factory, probe)
        }
    }

    struct Probe {
        sockets: Arc<Mutex<Vec<FakeSocket>>>,
        calls: Arc<Mutex<usize>>,
    }

    impl Probe {
        fn calls(&self) -> usize {
            *self.calls.lock().expect("calls")
        }

        fn socket(&self, index: usize) -> FakeSocket {
            self.sockets.lock().expect("sockets")[index].clone()
        }

        fn socket_count(&self) -> usize {
            self.sockets.lock().expect("sockets").len()
        }
    }

    /// A session with a fake clock the test moves by hand.
    struct Session {
        handle: RealtimeSttHandle,
        events: Receiver<BackendEvent>,
        now: Arc<Mutex<f64>>,
        _runtime: Arc<BackendRuntime>,
    }

    impl Session {
        fn advance(&self, seconds: f64) {
            *self.now.lock().expect("clock") += seconds;
        }

        fn results(&self) -> Vec<RealtimeResult> {
            self.events
                .try_iter()
                .filter_map(|event| match event {
                    BackendEvent::RealtimeResult(result) => Some(result),
                    _ => None,
                })
                .collect()
        }

        fn statuses(&self) -> Vec<(String, String)> {
            self.events
                .try_iter()
                .filter_map(|event| match event {
                    BackendEvent::Status(status) => {
                        Some((status.state, status.message.unwrap_or_default()))
                    }
                    _ => None,
                })
                .collect()
        }
    }

    fn session(factory: TransportFactory, settings: RealtimeSettings, api_key: &str) -> Session {
        let runtime = BackendRuntime::new().expect("runtime");
        let (sender, events) = backend_channel();
        let now = Arc::new(Mutex::new(1_000.0));
        let clock_now = Arc::clone(&now);
        let clock: Clock = Arc::new(move || *clock_now.lock().expect("clock"));

        let handle = RealtimeSttHandle::with_transport(
            &runtime,
            settings,
            Some(api_key.to_string()),
            factory,
            clock,
            sender,
        );
        Session {
            handle,
            events,
            now,
            _runtime: runtime,
        }
    }

    /// Spin until `condition` holds, or give up. Everything here is a background
    /// task, so every assertion about it is eventually-consistent.
    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the realtime session never reached the expected state");
    }

    /// speech-python.md test 15: the first frame is the documented
    /// `session.update`; the second is the buffer clear that starts an utterance.
    #[test]
    fn connecting_sends_the_documented_transcription_session_config() {
        let (factory, probe) = Factory::new().build();
        let session = session(factory, RealtimeSettings::default(), "sk-test");

        assert!(session.handle.begin("player-recording-1.wav"));
        wait_until(|| probe.socket_count() == 1 && probe.socket(0).sent().len() >= 2);

        let sent = probe.socket(0).sent();
        assert_eq!(sent[0]["type"], "session.update");
        assert_eq!(sent[0]["session"]["type"], "transcription");
        let input = &sent[0]["session"]["audio"]["input"];
        assert_eq!(
            input["format"],
            json!({"type": "audio/pcm", "rate": 24_000})
        );
        assert_eq!(input["transcription"]["model"], "gpt-realtime-whisper");
        assert_eq!(input["transcription"]["delay"], "low");
        assert!(
            input["transcription"].get("language").is_none(),
            "no language unless one is configured"
        );
        assert!(
            input["turn_detection"].is_null(),
            "endpointing is the game's job, not the provider's"
        );
        // A fresh utterance always starts from an empty provider buffer.
        assert_eq!(sent[1], json!({"type": "input_audio_buffer.clear"}));
    }

    /// speech-python.md test 16: an error that quotes the request must not carry
    /// the key into the HUD or the log.
    #[test]
    fn the_api_key_never_appears_in_a_status() {
        let secret = "sk-test-very-secret-value";
        let mut factory = Factory::failing(100);
        factory.leak_key = Some(secret.to_string());
        let (factory, _probe) = factory.build();
        let session = session(factory, RealtimeSettings::default(), secret);

        assert!(session.handle.begin("a.wav"));
        let mut seen = Vec::new();
        wait_until(|| {
            seen.extend(session.statuses());
            seen.iter().any(|(state, _)| state == "degraded")
        });

        assert!(
            seen.iter().any(|(_, message)| message.contains("***")),
            "the key must be scrubbed, not merely absent: {seen:?}"
        );
        for (_, message) in &seen {
            assert!(!message.contains(secret), "{message}");
        }
    }

    /// speech-python.md test 17: commits are acknowledged with item ids, and
    /// completions that arrive out of order still resolve to their own keys.
    #[test]
    fn commit_acknowledgements_bind_item_ids_and_late_completions_still_resolve() {
        let (factory, probe) = Factory::new().build();
        let session = session(factory, RealtimeSettings::default(), "sk");

        assert!(session.handle.begin("a.wav"));
        assert!(session.handle.append("a.wav", &[0, 1, 2]));
        assert!(session.handle.commit("a.wav"));
        assert!(session.handle.begin("b.wav"));
        assert!(session.handle.commit("b.wav"));

        wait_until(|| {
            probe.socket_count() == 1
                && probe
                    .socket(0)
                    .sent_types()
                    .iter()
                    .filter(|kind| *kind == "input_audio_buffer.commit")
                    .count()
                    == 2
        });
        let socket = probe.socket(0);
        assert!(
            socket
                .sent()
                .iter()
                .any(|message| message["type"] == "input_audio_buffer.append"
                    && message["audio"]
                        .as_str()
                        .is_some_and(|audio| !audio.is_empty())),
            "the PCM is base64 on the wire and nowhere else"
        );

        socket.push(json!({"type": "input_audio_buffer.committed", "item_id": "item-a"}));
        socket.push(json!({"type": "input_audio_buffer.committed", "item_id": "item-b"}));
        // The provider answers the *second* utterance first.
        socket.push(json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "item_id": "item-b",
            "transcript": "second utterance",
        }));
        socket.push(json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "item_id": "item-a",
            "transcript": "first utterance",
        }));

        let mut results = Vec::new();
        wait_until(|| {
            results.extend(session.results());
            results.len() == 2
        });
        assert_eq!(
            results,
            vec![
                RealtimeResult::Transcript {
                    key: "b.wav".to_string(),
                    text: "second utterance".to_string()
                },
                RealtimeResult::Transcript {
                    key: "a.wav".to_string(),
                    text: "first utterance".to_string()
                },
            ],
            "matched by item id, never by arrival order"
        );

        // A completion for an item nobody is waiting for is discarded: a late
        // transcript must never become a second `say`.
        socket.push(json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "item_id": "item-a",
            "transcript": "again",
        }));
        std::thread::sleep(Duration::from_millis(100));
        assert!(session.results().is_empty());
    }

    /// speech-python.md test 18: a dead socket fails its bound key exactly once,
    /// and the next utterance reconnects.
    #[test]
    fn a_dead_socket_fails_its_keys_once_and_the_next_utterance_reconnects() {
        let (factory, probe) = Factory::new().build();
        let session = session(factory, RealtimeSettings::default(), "sk");

        assert!(session.handle.begin("a.wav"));
        assert!(session.handle.commit("a.wav"));
        wait_until(|| {
            probe.socket_count() == 1
                && probe
                    .socket(0)
                    .sent_types()
                    .contains(&"input_audio_buffer.commit".to_string())
        });
        probe
            .socket(0)
            .push(json!({"type": "input_audio_buffer.committed", "item_id": "item-a"}));
        probe.socket(0).kill();

        let mut results = Vec::new();
        wait_until(|| {
            results.extend(session.results());
            !results.is_empty()
        });
        assert_eq!(
            results,
            vec![RealtimeResult::Failure {
                key: Some("a.wav".to_string()),
                reason: "socket".to_string()
            }],
            "exactly once, by name"
        );

        assert!(session.handle.begin("b.wav"), "a new utterance reconnects");
        wait_until(|| probe.socket_count() >= 2 && probe.socket(1).sent().len() >= 2);
    }

    /// speech-python.md test 19: one connect attempt, then a backoff window that
    /// rejects instantly — a dead endpoint must not become a retry storm.
    #[test]
    fn a_connect_failure_backs_off_instead_of_retrying_in_a_storm() {
        let (factory, probe) = Factory::failing(1).build();
        let session = session(factory, RealtimeSettings::default(), "sk");

        assert!(session.handle.begin("a.wav"));
        let mut results = Vec::new();
        wait_until(|| {
            results.extend(session.results());
            !results.is_empty()
        });
        assert_eq!(
            results,
            vec![RealtimeResult::Failure {
                key: Some("a.wav".to_string()),
                reason: "connect_failed".to_string()
            }]
        );
        assert_eq!(probe.calls(), 1, "exactly one attempt");

        // Inside the window: refused without touching the network.
        assert!(!session.handle.begin("b.wav"));
        assert_eq!(probe.calls(), 1);

        session.advance(60.0);
        assert!(session.handle.begin("c.wav"));
        wait_until(|| probe.calls() == 2);
    }

    /// speech-python.md test 20: the provider may only owe us so many
    /// transcripts; the newest commit is the one that is refused.
    #[test]
    fn the_in_flight_cap_refuses_the_newest_commit() {
        let (factory, _probe) = Factory::new().build();
        let settings = RealtimeSettings {
            max_in_flight: 2,
            ..RealtimeSettings::default()
        };
        let session = session(factory, settings, "sk");

        for key in ["a.wav", "b.wav"] {
            assert!(session.handle.begin(key));
            assert!(session.handle.commit(key));
        }
        assert!(session.handle.begin("c.wav"));
        assert!(
            !session.handle.commit("c.wav"),
            "two are already in flight: this one goes to batch"
        );
    }

    /// speech-python.md test 21: a cleared utterance is gone — no commit frame
    /// for it ever goes out, and the provider's buffer is emptied.
    #[test]
    fn clearing_an_utterance_forgets_it_and_blocks_its_commit() {
        let (factory, probe) = Factory::new().build();
        let session = session(factory, RealtimeSettings::default(), "sk");

        assert!(session.handle.begin("a.wav"));
        assert!(session.handle.append("a.wav", &[1, 2, 3]));
        session.handle.clear("a.wav");

        assert!(!session.handle.commit("a.wav"));
        assert!(!session.handle.append("a.wav", &[4]));

        wait_until(|| {
            probe.socket_count() == 1
                && probe
                    .socket(0)
                    .sent_types()
                    .iter()
                    .filter(|kind| *kind == "input_audio_buffer.clear")
                    .count()
                    >= 2
        });
        assert!(
            !probe
                .socket(0)
                .sent_types()
                .contains(&"input_audio_buffer.commit".to_string()),
            "a cleared utterance is never committed"
        );
    }

    /// speech-python.md test 22: an idle session closes its socket quietly and
    /// reconnects on the next utterance.
    #[test]
    fn an_idle_session_closes_quietly_and_reconnects_on_the_next_utterance() {
        let (factory, probe) = Factory::new().build();
        let settings = RealtimeSettings {
            idle_close_seconds: 5.0,
            ..RealtimeSettings::default()
        };
        let session = session(factory, settings, "sk");

        assert!(session.handle.begin("a.wav"));
        assert!(session.handle.commit("a.wav"));
        wait_until(|| {
            probe.socket_count() == 1
                && probe
                    .socket(0)
                    .sent_types()
                    .contains(&"input_audio_buffer.commit".to_string())
        });
        let socket = probe.socket(0);
        socket.push(json!({"type": "input_audio_buffer.committed", "item_id": "item-a"}));
        socket.push(json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "item_id": "item-a",
            "transcript": "done",
        }));
        let mut results = Vec::new();
        wait_until(|| {
            results.extend(session.results());
            !results.is_empty()
        });

        session.advance(6.0);
        wait_until(|| socket.closed.load(Ordering::SeqCst));
        assert!(
            session.results().is_empty(),
            "a deliberate idle close surfaces no failures"
        );

        assert!(session.handle.begin("b.wav"));
        wait_until(|| probe.calls() == 2);
    }

    /// speech-python.md test 23: shutdown is bounded even when the provider's
    /// close wedges — the close is detached, and nobody waits for it.
    #[test]
    fn closing_is_bounded_even_with_a_wedged_provider() {
        let mut factory = Factory::new();
        factory.close_delay = Duration::from_secs(5);
        let (factory, probe) = factory.build();
        let session = session(factory, RealtimeSettings::default(), "sk");

        assert!(session.handle.begin("a.wav"));
        wait_until(|| probe.socket_count() == 1 && probe.socket(0).sent().len() >= 2);

        let started = Instant::now();
        session.handle.close();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "close took {:?}",
            started.elapsed()
        );
        // And the session refuses new work afterwards.
        assert!(!session.handle.begin("b.wav"));
    }

    #[test]
    fn a_configured_language_reaches_the_session_config() {
        let (factory, probe) = Factory::new().build();
        let settings = RealtimeSettings {
            language: Some("sv".to_string()),
            model: "gpt-realtime-whisper-next".to_string(),
            ..RealtimeSettings::default()
        };
        let session = session(factory, settings, "sk");

        assert!(session.handle.begin("a.wav"));
        wait_until(|| probe.socket_count() == 1 && !probe.socket(0).sent().is_empty());
        let config = probe.socket(0).sent()[0].clone();
        let transcription = &config["session"]["audio"]["input"]["transcription"];
        assert_eq!(transcription["language"], "sv");
        assert_eq!(transcription["model"], "gpt-realtime-whisper-next");
    }
}
