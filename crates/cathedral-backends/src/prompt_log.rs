//! The per-session archive of every LLM exchange (`prompt_log.py`, prompt.md §5).
//!
//! A `.md` for reading and a `.json` for tooling, per turn, named
//! `<stamp>__<nn>__<actor id>__<actor name>_prompt.{md,json}`. That name is a
//! contract with the user's own tooling (AGENTS.md, risk R22): it is ported
//! byte-for-byte, sanitizer quirks included.
//!
//! Lives in cathedral-backends, not the sim, because it needs a filesystem and a
//! wall clock (D24). The scheduler emits `SchedulerEvent::PromptExchange`; the
//! host hands it here.

use cathedral_sim::prompt_archive::{PromptArchivePermit, PromptArchiveRetention};
use cathedral_sim::{Cognition, CognitionBusy, RequestId, SchedulerEvent, py_round};
use serde::Serialize;
use std::{
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

type SharedExchange = Arc<cathedral_sim::prompt_archive::PromptExchange>;
/// Eight outstanding exchanges, including reserved/held and active work.
pub const ARCHIVE_SLOTS: usize = 8;
pub const ARCHIVE_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
/// Payload plus explicit 2 MiB thread stack and 2 MiB fixed controls/IO/TLS.
pub const ARCHIVE_ALLOWANCE_BYTES: usize = ARCHIVE_PAYLOAD_BYTES + 4 * 1024 * 1024;
const COMPLETION_UPPER_BYTES: usize = 401_024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArchiveUsage {
    pub slots: usize,
    pub bytes: usize,
    pub active: usize,
    pub pending_writes: usize,
}
struct Core {
    usage: Mutex<ArchiveUsage>,
    slots: usize,
    bytes: usize,
    // Actual writer, queue and permits retain this through final destruction.
    _lease: Option<cathedral_sim::checkpoint::Reservation>,
}
impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveCore")
            .field("usage", &self.usage.lock().unwrap())
            .finish()
    }
}
#[derive(Debug)]
struct PayloadCharge {
    core: Arc<Core>,
    bytes: usize,
}
impl Drop for PayloadCharge {
    fn drop(&mut self) {
        let mut usage = self.core.usage.lock().unwrap();
        usage.slots -= 1;
        usage.bytes -= self.bytes;
    }
}
#[derive(Debug)]
struct Receipt {
    session: Arc<Session>,
    queued: Mutex<bool>,
    // Session metadata dies before its final payload charge is returned.
    charge: PayloadCharge,
}
impl PromptArchiveRetention for Receipt {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
#[derive(Debug, Default)]
struct Order {
    last_stamp: String,
    next_index: u64,
    accepted: u64,
    completed: u64,
}
#[derive(Debug)]
struct Progress {
    order: Mutex<Order>,
    done: Condvar,
}
#[derive(Debug)]
struct Session {
    directory: PathBuf,
    model: Option<String>,
    progress: Arc<Progress>,
}
struct WriteJob {
    session: Arc<Session>,
    base: String,
    timestamp: String,
    sequence: u64,
    // All charged job metadata drops before the exchange's final receipt.
    exchange: SharedExchange,
}

/// An isolated writer can be retained by a session and its forks. There is no
/// process-wide map of directories and no unbounded barrier/job queue.
/// Startup and final destruction are off-frame, like PromptLog::flush/Drop.
struct WriterOwner {
    worker: Option<thread::JoinHandle<()>>,
    _core: Arc<Core>,
}
impl Drop for WriterOwner {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[derive(Clone)]
pub struct ArchiveWriter {
    // Each clone drops its sender before its owner. The last owner therefore
    // joins after the channel closes, retaining the native stack allowance
    // through actual thread termination, including spawn/payload panic paths.
    sender: crossbeam_channel::Sender<WriteJob>,
    _owner: Arc<WriterOwner>,
    core: Arc<Core>,
}
impl ArchiveWriter {
    pub fn start_admitted(
        budget: &cathedral_sim::checkpoint::CheckpointBudget,
    ) -> cathedral_sim::checkpoint::Result<Self> {
        let lease = budget.reserve_running_overhead(ARCHIVE_ALLOWANCE_BYTES)?;
        Self::start(Some(lease), ARCHIVE_SLOTS, ARCHIVE_PAYLOAD_BYTES, None).map_err(|_| {
            cathedral_sim::checkpoint::CheckpointError {
                owner: "archive",
                reason: "writer startup failed".into(),
            }
        })
    }
    fn start(
        lease: Option<cathedral_sim::checkpoint::Reservation>,
        slots: usize,
        bytes: usize,
        before_write: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> std::io::Result<Self> {
        let core = Arc::new(Core {
            usage: Mutex::new(ArchiveUsage::default()),
            slots,
            bytes,
            _lease: lease,
        });
        let (sender, receiver) = crossbeam_channel::bounded::<WriteJob>(slots);
        let worker_core = Arc::clone(&core);
        let worker = thread::Builder::new()
            .name("cathedral-prompt-log".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                for job in receiver {
                    worker_core.usage.lock().unwrap().active += 1;
                    if let Some(hook) = &before_write {
                        hook();
                    }
                    render_and_write(
                        &job.session.directory,
                        &job.base,
                        &job.exchange,
                        job.session.model.as_deref(),
                        job.timestamp.clone(),
                    );
                    worker_core.usage.lock().unwrap().active -= 1;
                    // Completion controls are fixed and covered by Core's
                    // worker allowance. No directory/model string survives
                    // payload disposal through this observer.
                    let progress = Arc::clone(&job.session.progress);
                    let sequence = job.sequence;
                    // The strings and receipt die before flush observes completion.
                    drop(job);
                    worker_core.usage.lock().unwrap().pending_writes -= 1;
                    progress.order.lock().unwrap().completed = sequence;
                    progress.done.notify_all();
                }
            })?;
        Ok(Self {
            sender,
            _owner: Arc::new(WriterOwner {
                worker: Some(worker),
                _core: Arc::clone(&core),
            }),
            core,
        })
    }
    pub fn usage(&self) -> ArchiveUsage {
        *self.core.usage.lock().unwrap()
    }
    #[cfg(test)]
    pub(crate) fn isolated(
        slots: usize,
        bytes: usize,
        before_write: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self::start(None, slots, bytes, Some(before_write)).unwrap()
    }
}
fn writer() -> &'static ArchiveWriter {
    static WRITER: OnceLock<ArchiveWriter> = OnceLock::new();
    WRITER.get_or_init(|| {
        let writer = ArchiveWriter::start(None, ARCHIVE_SLOTS, ARCHIVE_PAYLOAD_BYTES, None)
            .expect("the prompt-log writer thread spawns");
        unsafe {
            libc::atexit(flush_at_exit);
        }
        writer
    })
}
extern "C" fn flush_at_exit() {
    // A bounded global writer has no unbounded flush message. At exit only,
    // wait for already accepted queue/active owners, not unsubmitted permits.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while writer().usage().pending_writes != 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// One archived exchange — successes and failures alike (`scheduler.py:205-213`).
#[derive(Debug, Clone, PartialEq)]
pub struct PromptExchange {
    pub actor_id: String,
    pub actor_name: String,
    pub prompt: String,
    /// `None` on a failed turn: the prompt is still archived.
    pub answer: Option<String>,
    pub duration_seconds: f64,
    pub error: Option<String>,
}

/// The `.json` twin: `{prompt, answer, meta}`, in that order.
///
/// A struct rather than a `serde_json::Map` because serde preserves *field*
/// order while the default `Map` is a `BTreeMap` and would sort the keys —
/// which would silently change the archive format.
#[derive(Debug, Serialize)]
struct Record<'a> {
    prompt: &'a str,
    answer: Option<&'a str>,
    meta: Meta<'a>,
}

#[derive(Debug, Serialize)]
struct Meta<'a> {
    actor_id: &'a str,
    actor_name: &'a str,
    model: Option<&'a str>,
    /// `round(x, 3)`, a JSON number.
    duration_seconds: f64,
    /// `isoformat(timespec="seconds")`: local, no timezone.
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

/// Writes the `.md`/`.json` pairs. Without a directory it is disabled and
/// `record` is a silent no-op (terminal prototype, tests, a sidecar launched
/// outside the game).
pub struct PromptLog {
    session: Option<Arc<Session>>,
    clock: Box<dyn FnMut() -> LocalTime + Send>,
    writer: Option<ArchiveWriter>,
}
impl std::fmt::Debug for PromptLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptLog")
            .field("session", &self.session)
            .finish()
    }
}
impl Drop for PromptLog {
    fn drop(&mut self) {
        self.flush();
    }
}
impl PromptLog {
    pub fn new(directory: Option<PathBuf>, model: Option<String>) -> Self {
        Self::with_clock(directory, model, Box::new(LocalTime::now))
    }
    pub fn with_clock(
        directory: Option<PathBuf>,
        model: Option<String>,
        clock: Box<dyn FnMut() -> LocalTime + Send>,
    ) -> Self {
        if directory.is_none() {
            Self {
                session: None,
                clock,
                writer: None,
            }
        } else {
            Self::with_writer(directory, model, clock, writer().clone())
        }
    }
    pub fn with_writer(
        directory: Option<PathBuf>,
        model: Option<String>,
        clock: Box<dyn FnMut() -> LocalTime + Send>,
        writer: ArchiveWriter,
    ) -> Self {
        Self {
            session: directory.map(|directory| {
                Arc::new(Session {
                    directory,
                    model,
                    progress: Arc::new(Progress {
                        order: Mutex::new(Order::default()),
                        done: Condvar::new(),
                    }),
                })
            }),
            writer: Some(writer),
            clock,
        }
    }
    /// Load successors retain this session's exact same-second filename order.
    pub fn fork(&self) -> Self {
        self.fork_with_clock(Box::new(LocalTime::now))
    }
    pub fn fork_with_clock(&self, clock: Box<dyn FnMut() -> LocalTime + Send>) -> Self {
        Self {
            session: self.session.clone(),
            writer: self.writer.clone(),
            clock,
        }
    }
    pub fn enabled(&self) -> bool {
        self.session.is_some()
    }
    pub fn directory(&self) -> Option<&Path> {
        self.session.as_ref().map(|s| s.directory.as_path())
    }
    pub fn usage(&self) -> ArchiveUsage {
        self.writer
            .as_ref()
            .map_or_else(ArchiveUsage::default, ArchiveWriter::usage)
    }
    /// Decorate both real and fake cognition. The scheduler requests its permit
    /// before provider acceptance and carries it through held result delivery.
    pub fn cognition(&self, inner: Box<dyn Cognition + Send>) -> Box<dyn Cognition + Send> {
        Box::new(ArchivedCognition {
            inner,
            session: self.session.clone(),
            writer: self.writer.clone(),
        })
    }
    fn reserve(
        &self,
        prompt: usize,
        labels: usize,
        completion: usize,
    ) -> Result<PromptArchivePermit, CognitionBusy> {
        reserve(
            self.writer.as_ref(),
            self.session.as_ref(),
            prompt,
            labels,
            completion,
        )
    }
    /// Standalone acceptance is fallible and returns the unchanged allocation.
    pub fn record(&mut self, exchange: PromptExchange) -> Result<(), PromptExchange> {
        if !self.enabled() {
            return Ok(());
        }
        let completion = exchange
            .answer
            .as_ref()
            .map_or(0, String::capacity)
            .saturating_add(exchange.error.as_ref().map_or(0, String::capacity));
        let permit = match self.reserve(
            exchange.prompt.capacity(),
            exchange
                .actor_id
                .capacity()
                .saturating_add(exchange.actor_name.capacity()),
            completion,
        ) {
            Ok(permit) => permit,
            Err(_) => return Err(exchange),
        };
        let exchange = cathedral_sim::prompt_archive::PromptExchange::new(
            cathedral_sim::prompt_archive::PromptExchangeData {
                actor_id: cathedral_sim::ActorId::from_raw(exchange.actor_id),
                actor_name: exchange.actor_name,
                prompt: exchange.prompt,
                answer: exchange.answer,
                duration_seconds: exchange.duration_seconds,
                error: exchange.error,
            },
            permit,
        );
        self.record_shared(exchange)
            .expect("fresh standalone archive admission");
        Ok(())
    }
    /// Shared events never copy their strings. A foreign/previously queued
    /// receipt is refused while returning that exact Arc to the caller.
    pub fn record_shared(&mut self, exchange: SharedExchange) -> Result<(), SharedExchange> {
        let Some(session) = self.session.as_ref() else {
            return if exchange.admission().enabled() {
                Err(exchange)
            } else {
                Ok(())
            };
        };
        // Shared values require their original unique admission. A new permit
        // only on a queue job would leave an external Arc clone uncharged.
        let permit = exchange.admission();
        let Some(receipt) = permit.owner::<Receipt>() else {
            return Err(exchange);
        };
        if !Arc::ptr_eq(&receipt.session, session)
            || !Arc::ptr_eq(&receipt.charge.core, &self.writer.as_ref().unwrap().core)
        {
            return Err(exchange);
        }
        let completion = exchange
            .answer
            .as_ref()
            .map_or(0, String::capacity)
            .saturating_add(exchange.error.as_ref().map_or(0, String::capacity));
        let required = allocation_bytes(
            session,
            exchange.prompt.capacity(),
            exchange
                .actor_name
                .capacity()
                .saturating_add(exchange.actor_id.allocated_bytes()),
            completion,
        );
        if required.is_none_or(|bytes| bytes > receipt.charge.bytes) {
            return Err(exchange);
        }
        let mut queued = receipt.queued.lock().unwrap();
        if *queued {
            drop(queued);
            return Err(exchange);
        }
        let moment = (self.clock)();
        let stamp = moment.file_stamp();
        let mut order = session.progress.order.lock().unwrap();
        let index = if stamp == order.last_stamp {
            order.next_index
        } else {
            0
        };
        let Some(next_index) = index.checked_add(1) else {
            drop(queued);
            return Err(exchange);
        };
        let Some(sequence) = order.accepted.checked_add(1) else {
            drop(queued);
            return Err(exchange);
        };
        let base = format!(
            "{stamp}__{index:02}__{}__{}_prompt",
            safe(exchange.actor_id.as_str()),
            safe(&exchange.actor_name)
        );
        // A reserved slot includes an active job: at most slots-1 other jobs
        // can occupy this slots-sized channel. No disk or capacity wait here.
        *queued = true;
        drop(queued);
        self.writer
            .as_ref()
            .unwrap()
            .core
            .usage
            .lock()
            .unwrap()
            .pending_writes += 1;
        self.writer
            .as_ref()
            .unwrap()
            .sender
            .try_send(WriteJob {
                exchange,
                session: Arc::clone(session),
                base,
                timestamp: moment.iso_seconds(),
                sequence,
            })
            .unwrap_or_else(|_| panic!("admitted prompt writer unavailable"));
        order.last_stamp = stamp;
        order.next_index = next_index;
        order.accepted = sequence;
        Ok(())
    }
    pub fn flush(&self) {
        let Some(session) = &self.session else {
            return;
        };
        let mut order = session.progress.order.lock().unwrap();
        let target = order.accepted;
        while order.completed < target {
            order = session.progress.done.wait(order).unwrap();
        }
    }
    pub fn record_scheduler_event(&mut self, event: &SchedulerEvent) -> Result<(), SharedExchange> {
        if !self.enabled() {
            return Ok(());
        }
        let SchedulerEvent::PromptExchange { exchange } = event else {
            return Ok(());
        };
        if exchange.admission().enabled() {
            return self.record_shared(Arc::clone(exchange));
        }
        // Legacy convenience ingress copies only after obtaining its own
        // admission; caller-owned unadmitted event clones remain caller-owned.
        let completion = exchange
            .answer
            .as_ref()
            .map_or(0, String::capacity)
            .saturating_add(exchange.error.as_ref().map_or(0, String::capacity));
        let permit = self
            .reserve(
                exchange.prompt.capacity(),
                exchange
                    .actor_name
                    .capacity()
                    .saturating_add(exchange.actor_id.allocated_bytes()),
                completion,
            )
            .map_err(|_| Arc::clone(exchange))?;
        let owned = cathedral_sim::prompt_archive::PromptExchange::new(
            cathedral_sim::prompt_archive::PromptExchangeData {
                actor_id: exchange.actor_id.clone(),
                actor_name: exchange.actor_name.clone(),
                prompt: exchange.prompt.clone(),
                answer: exchange.answer.clone(),
                duration_seconds: exchange.duration_seconds,
                error: exchange.error.clone(),
            },
            permit,
        );
        self.record_shared(owned)
            .expect("fresh convenience archive admission");
        Ok(())
    }
}
fn reserve(
    writer: Option<&ArchiveWriter>,
    session: Option<&Arc<Session>>,
    prompt: usize,
    labels: usize,
    completion: usize,
) -> Result<PromptArchivePermit, CognitionBusy> {
    let Some(session) = session else {
        return Ok(Default::default());
    };
    let writer = writer.expect("enabled archive retains its writer");
    let bytes = allocation_bytes(session, prompt, labels, completion).ok_or(CognitionBusy)?;
    {
        let order = session.progress.order.lock().unwrap();
        if order.accepted > u64::MAX - ARCHIVE_SLOTS as u64
            || order.next_index > u64::MAX - ARCHIVE_SLOTS as u64
        {
            return Err(CognitionBusy);
        }
    }
    let mut usage = writer.core.usage.lock().unwrap();
    if usage.slots == writer.core.slots || bytes > writer.core.bytes.saturating_sub(usage.bytes) {
        return Err(CognitionBusy);
    }
    usage.slots += 1;
    usage.bytes += bytes;
    drop(usage);
    Ok(PromptArchivePermit::new(Arc::new(Receipt {
        session: Arc::clone(session),
        queued: Mutex::new(false),
        charge: PayloadCharge {
            core: Arc::clone(&writer.core),
            bytes,
        },
    })))
}
fn allocation_bytes(
    session: &Session,
    prompt: usize,
    labels: usize,
    completion: usize,
) -> Option<usize> {
    // Prompt/answer/error capacities; cloned metadata, filename sanitizing and
    // joined paths (old/new growth), record roots and fixed formatting scratch.
    prompt
        .checked_add(completion)
        .and_then(|n| labels.checked_mul(8).and_then(|m| n.checked_add(m)))
        .and_then(|n| {
            session
                .directory
                .capacity()
                .checked_mul(4)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            session
                .model
                .as_ref()
                .map_or(0, String::capacity)
                .checked_mul(2)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(8192))
}
struct ArchivedCognition {
    inner: Box<dyn Cognition + Send>,
    session: Option<Arc<Session>>,
    writer: Option<ArchiveWriter>,
}
impl Cognition for ArchivedCognition {
    fn reserve_prompt_archive(
        &mut self,
        prompt: usize,
        labels: usize,
    ) -> Result<PromptArchivePermit, CognitionBusy> {
        reserve(
            self.writer.as_ref(),
            self.session.as_ref(),
            prompt,
            labels,
            COMPLETION_UPPER_BYTES,
        )
    }
    fn request(&mut self, p: String) -> Result<RequestId, CognitionBusy> {
        self.inner.request(p)
    }
    fn request_with_budget(
        &mut self,
        p: String,
        b: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.inner.request_with_budget(p, b)
    }
    fn request_night(&mut self, p: String, b: Option<u32>) -> Result<RequestId, CognitionBusy> {
        self.inner.request_night(p, b)
    }
}

/// Render the pair and put it on disk — all of it on the writer thread.
///
/// Stream JSON before Markdown. A filesystem or serialization error stops this
/// attempt and is reported as a diagnostic; already-written bytes may remain.
/// Disk failures retain the existing best-effort policy outside admission.
fn render_and_write(
    directory: &Path,
    base: &str,
    exchange: &cathedral_sim::prompt_archive::PromptExchange,
    model: Option<&str>,
    timestamp: String,
) {
    let meta = Meta {
        actor_id: exchange.actor_id.as_str(),
        actor_name: &exchange.actor_name,
        model,
        duration_seconds: py_round(exchange.duration_seconds, 3),
        timestamp,
        error: exchange.error.as_deref(),
    };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(directory)?;
        let mut json = BufWriter::with_capacity(
            16 * 1024,
            fs::File::create(directory.join(format!("{base}.json")))?,
        );
        serde_json::to_writer_pretty(
            &mut json,
            &Record {
                prompt: &exchange.prompt,
                answer: exchange.answer.as_deref(),
                meta: Meta {
                    timestamp: meta.timestamp.clone(),
                    ..meta
                },
            },
        )?;
        json.write_all(b"\n")?;
        json.flush()?;
        drop(json);
        let mut md = BufWriter::with_capacity(
            16 * 1024,
            fs::File::create(directory.join(format!("{base}.md")))?,
        );
        write!(
            md,
            "# Prompt\n\n{}\n\n# Answer\n\n{}\n\n# Meta\n\n- actor_id: {}\n- actor_name: {}\n- model: {}\n- duration_seconds: {:?}\n- timestamp: {}\n",
            exchange.prompt.trim_end_matches('\n'),
            exchange
                .answer
                .as_deref()
                .map(|s| s.trim_end_matches('\n'))
                .unwrap_or("*(no answer)*"),
            meta.actor_id,
            meta.actor_name,
            meta.model.unwrap_or("None"),
            meta.duration_seconds,
            meta.timestamp
        )?;
        if let Some(error) = meta.error {
            writeln!(md, "- error: {error}")?;
        }
        md.flush()
    };
    if let Err(error) = write() {
        eprintln!("[smart actors] prompt log write failed: {error}");
    }
}

/// Filename-safe id/name components (`prompt_log.py:13-16`).
///
/// Every run of characters outside `[A-Za-z0-9-]` collapses to a single `-`,
/// then all leading/trailing `-` are stripped; an empty result becomes
/// `unknown`. `_` is deliberately *not* safe — it is the field separator.
/// `../evil` ⇒ `evil`, `Olof Skötkonung` ⇒ `Olof-Sk-tkonung`.
fn safe(value: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    let mut in_run = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '-' {
            cleaned.push(character);
            in_run = false;
        } else if !in_run {
            cleaned.push('-');
            in_run = true;
        }
    }
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

// -------------------------------------------------------------------- the clock

/// Local wall-clock time, to the second.
///
/// The same libc-based conversion the game's session log uses
/// (`src/session_log.rs`): std cannot turn a `SystemTime` into a local date, and
/// the archive's names are local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i64,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl LocalTime {
    pub fn now() -> Self {
        Self::from_unix_seconds(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        )
    }

    pub fn from_unix_seconds(unix_seconds: u64) -> Self {
        #[cfg(unix)]
        if let Some(local) = unix_local_time(unix_seconds) {
            return local;
        }
        utc_time(unix_seconds)
    }

    /// `strftime("%Y-%m-%d_%H_%M_%S")`.
    pub fn file_stamp(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}_{:02}_{:02}_{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// `isoformat(timespec="seconds")` — no timezone suffix, like Python's naive
    /// `datetime.now()`.
    pub fn iso_seconds(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

#[cfg(unix)]
fn unix_local_time(unix_seconds: u64) -> Option<LocalTime> {
    let seconds: libc::time_t = unix_seconds.try_into().ok()?;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();

    // SAFETY: `seconds` is valid for the call and the uninitialized `tm` is
    // written by `localtime_r` before it returns a non-null pointer to it.
    let result = unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) };
    if result.is_null() {
        return None;
    }
    // SAFETY: a non-null return means `local` is initialized.
    let local = unsafe { local.assume_init() };
    Some(LocalTime {
        year: i64::from(local.tm_year) + 1900,
        month: u8::try_from(local.tm_mon + 1).ok()?,
        day: u8::try_from(local.tm_mday).ok()?,
        hour: u8::try_from(local.tm_hour).ok()?,
        minute: u8::try_from(local.tm_min).ok()?,
        second: u8::try_from(local.tm_sec).ok()?,
    })
}

fn utc_time(unix_seconds: u64) -> LocalTime {
    const SECONDS_PER_DAY: u64 = 86_400;
    let days = i64::try_from(unix_seconds / SECONDS_PER_DAY).unwrap_or(i64::MAX);
    let seconds_today = unix_seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_date_from_unix_days(days);
    LocalTime {
        year,
        month,
        day,
        hour: (seconds_today / 3_600) as u8,
        minute: ((seconds_today % 3_600) / 60) as u8,
        second: (seconds_today % 60) as u8,
    }
}

/// Howard Hinnant's civil-calendar algorithm — the dependency-free non-Unix
/// fallback (same as the game's session log).
fn civil_date_from_unix_days(days: i64) -> (i64, u8, u8) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u8;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u8;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct Fixture {
        directory: PathBuf,
        moment: Arc<Mutex<LocalTime>>,
        log: PromptLog,
    }

    impl Fixture {
        /// `PromptLogTests.setUp`: 2026-07-13 09:52:30, model kimi-k2.5.
        fn new(tag: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let directory =
                std::env::temp_dir().join(format!("cathedral-prompt-log-{tag}-{unique}"));
            let moment = Arc::new(Mutex::new(LocalTime {
                year: 2026,
                month: 7,
                day: 13,
                hour: 9,
                minute: 52,
                second: 30,
            }));
            let clock = Arc::clone(&moment);
            let log = PromptLog::with_writer(
                Some(directory.clone()),
                Some("kimi-k2.5".to_string()),
                Box::new(move || *clock.lock().expect("clock")),
                ArchiveWriter::isolated(ARCHIVE_SLOTS, ARCHIVE_PAYLOAD_BYTES, Arc::new(|| {})),
            );
            Self {
                directory,
                moment,
                log,
            }
        }

        /// `PromptLogTests.record` with its default arguments.
        fn record(&mut self) {
            self.log.record(exchange()).unwrap();
            self.log.flush();
        }

        fn names(&self, extension: &str) -> Vec<String> {
            self.log.flush();
            let mut names: Vec<String> = fs::read_dir(&self.directory)
                .expect("archive directory")
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .filter(|name| name.ends_with(extension))
                .collect();
            names.sort();
            names
        }

        fn read(&self, name: &str) -> String {
            self.log.flush();
            fs::read_to_string(self.directory.join(name)).expect("archived file")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn exchange() -> PromptExchange {
        PromptExchange {
            actor_id: "k0fb1".to_string(),
            actor_name: "Ilse".to_string(),
            prompt: "the prompt".to_string(),
            answer: Some("wait {}".to_string()),
            duration_seconds: 1.234_567,
            error: None,
        }
    }

    /// prompt.md §8 test 15.
    #[test]
    fn the_md_and_json_pair_uses_the_schema_name() {
        let mut fixture = Fixture::new("schema-name");
        fixture.record();

        let base = "2026-07-13_09_52_30__00__k0fb1__Ilse_prompt";
        let mut names = fixture.names(".md");
        names.extend(fixture.names(".json"));
        names.sort();
        assert_eq!(names, [format!("{base}.json"), format!("{base}.md")]);

        let markdown = fixture.read(&format!("{base}.md"));
        assert!(markdown.contains("# Prompt\n\nthe prompt\n"), "{markdown}");
        assert!(markdown.contains("# Answer\n\nwait {}\n"), "{markdown}");
        assert!(
            markdown.contains("# Meta\n\n- actor_id: k0fb1"),
            "{markdown}"
        );
        assert!(markdown.contains("- model: kimi-k2.5"), "{markdown}");
        assert!(markdown.contains("- duration_seconds: 1.235"), "{markdown}");
        assert!(markdown.ends_with('\n'));

        let json: serde_json::Value =
            serde_json::from_str(&fixture.read(&format!("{base}.json"))).expect("json");
        assert_eq!(json["prompt"], "the prompt");
        assert_eq!(json["answer"], "wait {}");
        assert_eq!(json["meta"]["actor_name"], "Ilse");
        assert_eq!(json["meta"]["duration_seconds"], 1.235);
        assert!(
            json["meta"].get("error").is_none(),
            "no error key on success"
        );
    }

    /// The `{prompt, answer, meta}` key order is part of the format.
    #[test]
    fn the_json_keeps_python_key_order_and_raw_utf8() {
        let mut fixture = Fixture::new("key-order");
        fixture
            .log
            .record(PromptExchange {
                prompt: "Ilse sa: \"Hej då\"".to_string(),
                ..exchange()
            })
            .unwrap();
        fixture.log.flush();

        let raw = fixture.read("2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.json");
        let keys: Vec<&str> = raw
            .lines()
            .filter_map(|line| line.trim().strip_prefix('"'))
            .filter_map(|line| line.split('"').next())
            .collect();
        assert_eq!(&keys[..3], ["prompt", "answer", "meta"]);
        assert_eq!(
            &keys[3..8],
            [
                "actor_id",
                "actor_name",
                "model",
                "duration_seconds",
                "timestamp"
            ]
        );
        // ensure_ascii=False: the archive is raw UTF-8.
        assert!(raw.contains("Hej då"), "{raw}");
        assert!(
            raw.contains("\"timestamp\": \"2026-07-13T09:52:30\""),
            "{raw}"
        );
    }

    /// prompt.md §8 test 16.
    #[test]
    fn same_second_exchanges_get_increasing_suffixes() {
        let mut fixture = Fixture::new("suffixes");
        fixture.record();
        fixture.record();
        fixture.moment.lock().expect("clock").second = 31;
        fixture.record();

        assert_eq!(
            fixture.names(".md"),
            [
                "2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.md",
                "2026-07-13_09_52_30__01__k0fb1__Ilse_prompt.md",
                "2026-07-13_09_52_31__00__k0fb1__Ilse_prompt.md",
            ]
        );
    }

    /// prompt.md §8 test 17.
    #[test]
    fn a_failed_exchange_keeps_the_prompt_and_records_the_error() {
        let mut fixture = Fixture::new("failed");
        fixture
            .log
            .record(PromptExchange {
                answer: None,
                error: Some("TimeoutError('provider')".to_string()),
                ..exchange()
            })
            .unwrap();
        fixture.log.flush();

        let base = "2026-07-13_09_52_30__00__k0fb1__Ilse_prompt";
        let markdown = fixture.read(&format!("{base}.md"));
        assert!(
            markdown.contains("# Answer\n\n*(no answer)*\n"),
            "{markdown}"
        );
        assert!(
            markdown.contains("- error: TimeoutError('provider')"),
            "{markdown}"
        );

        let json: serde_json::Value =
            serde_json::from_str(&fixture.read(&format!("{base}.json"))).expect("json");
        assert!(json["answer"].is_null());
        assert_eq!(json["meta"]["error"], "TimeoutError('provider')");
    }

    /// prompt.md §8 test 18.
    #[test]
    fn hostile_name_components_are_sanitized() {
        let mut fixture = Fixture::new("hostile");
        fixture
            .log
            .record(PromptExchange {
                actor_id: "../evil".to_string(),
                actor_name: "Olof Skötkonung".to_string(),
                ..exchange()
            })
            .unwrap();
        fixture.log.flush();

        assert_eq!(
            fixture.names(".md"),
            ["2026-07-13_09_52_30__00__evil__Olof-Sk-tkonung_prompt.md"]
        );
    }

    /// prompt.md §8 test 19.
    #[test]
    fn without_a_directory_the_log_is_disabled() {
        let mut log = PromptLog::new(None, None);
        assert!(!log.enabled());
        log.record(exchange()).unwrap(); // must not panic, must not write anywhere
    }

    #[test]
    fn a_write_failure_never_reaches_the_turn_loop() {
        // A *file* where the archive directory should be: mkdir -p fails.
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let blocker = std::env::temp_dir().join(format!("cathedral-prompt-blocker-{unique}"));
        fs::write(&blocker, "not a directory").expect("blocker file");

        let mut log = PromptLog::new(Some(blocker.join("prompts")), Some("m".to_string()));
        log.record(exchange()).unwrap(); // swallowed

        fs::remove_file(&blocker).ok();
    }

    #[test]
    fn a_missing_model_prints_pythons_none() {
        let mut fixture = Fixture::new("no-model");
        Arc::get_mut(fixture.log.session.as_mut().unwrap())
            .unwrap()
            .model = None;
        fixture.record();

        let markdown = fixture.read("2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.md");
        assert!(markdown.contains("- model: None"), "{markdown}");
        let json: serde_json::Value =
            serde_json::from_str(&fixture.read("2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.json"))
                .expect("json");
        assert!(json["meta"]["model"].is_null());
    }

    #[test]
    fn a_whole_duration_keeps_its_decimal_point() {
        let mut fixture = Fixture::new("whole-duration");
        fixture
            .log
            .record(PromptExchange {
                duration_seconds: 2.0,
                ..exchange()
            })
            .unwrap();
        fixture.log.flush();
        let markdown = fixture.read("2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.md");
        assert!(
            markdown.contains("- duration_seconds: 2.0"),
            "python str(2.0) is '2.0', not '2': {markdown}"
        );
    }

    #[test]
    fn the_sanitizer_matches_pythons_regex() {
        assert_eq!(safe("k0fb1"), "k0fb1");
        assert_eq!(safe("../evil"), "evil");
        assert_eq!(safe("Olof Skötkonung"), "Olof-Sk-tkonung");
        assert_eq!(safe("under_score"), "under-score", "'_' is the separator");
        assert_eq!(safe("--a--b--"), "a--b");
        assert_eq!(safe("///"), "unknown");
        assert_eq!(safe(""), "unknown");
        assert_eq!(safe("åäö"), "unknown");
    }

    #[test]
    fn a_scheduler_event_is_archived_and_other_events_are_ignored() {
        let mut fixture = Fixture::new("scheduler-event");
        fixture
            .log
            .record_scheduler_event(&SchedulerEvent::Diagnostic("noise".to_string()))
            .unwrap();
        assert!(!fixture.directory.exists(), "no exchange, no directory");

        fixture
            .log
            .record_scheduler_event(&SchedulerEvent::PromptExchange {
                exchange: cathedral_sim::prompt_archive::PromptExchange::new(
                    cathedral_sim::prompt_archive::PromptExchangeData {
                        actor_id: cathedral_sim::ActorId::new("k0fb1").expect("id"),
                        actor_name: "Ilse".to_string(),
                        prompt: "the prompt".to_string(),
                        answer: Some("wait {}".to_string()),
                        duration_seconds: 0.5,
                        error: None,
                    },
                    Default::default(),
                ),
            })
            .unwrap();
        assert_eq!(
            fixture.names(".md"),
            ["2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.md"]
        );
    }

    /// The archive is a contract with the user's tooling (R22), so this pins the
    /// whole file, not a substring. Both expectations are the literal bytes
    /// `prompt_log.py` produced for the same two records at the same two
    /// moments.
    #[test]
    fn the_archive_is_byte_identical_to_python() {
        let mut fixture = Fixture::new("golden");
        fixture
            .log
            .record(PromptExchange {
                prompt: "Ilse sa: \"Hej då\"".to_string(),
                ..exchange()
            })
            .unwrap();
        fixture.log.flush();

        assert_eq!(
            fixture.read("2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.json"),
            concat!(
                "{\n",
                "  \"prompt\": \"Ilse sa: \\\"Hej då\\\"\",\n",
                "  \"answer\": \"wait {}\",\n",
                "  \"meta\": {\n",
                "    \"actor_id\": \"k0fb1\",\n",
                "    \"actor_name\": \"Ilse\",\n",
                "    \"model\": \"kimi-k2.5\",\n",
                "    \"duration_seconds\": 1.235,\n",
                "    \"timestamp\": \"2026-07-13T09:52:30\"\n",
                "  }\n",
                "}\n",
            )
        );
        assert_eq!(
            fixture.read("2026-07-13_09_52_30__00__k0fb1__Ilse_prompt.md"),
            concat!(
                "# Prompt\n\nIlse sa: \"Hej då\"\n\n",
                "# Answer\n\nwait {}\n\n",
                "# Meta\n\n",
                "- actor_id: k0fb1\n",
                "- actor_name: Ilse\n",
                "- model: kimi-k2.5\n",
                "- duration_seconds: 1.235\n",
                "- timestamp: 2026-07-13T09:52:30\n",
            )
        );

        // A failed exchange, an unset model, a whole-number duration, a prompt
        // with trailing newlines, and a hostile id/name — all in one record.
        let mut failed = PromptLog::with_clock(
            Some(fixture.directory.clone()),
            None,
            Box::new(|| LocalTime {
                year: 2026,
                month: 7,
                day: 13,
                hour: 9,
                minute: 52,
                second: 31,
            }),
        );
        failed
            .record(PromptExchange {
                actor_id: "../evil".to_string(),
                actor_name: "Olof Skötkonung".to_string(),
                prompt: "p\n\n".to_string(),
                answer: None,
                duration_seconds: 2.0,
                error: Some("TimeoutError('provider')".to_string()),
            })
            .unwrap();
        failed.flush();

        assert_eq!(
            fixture.read("2026-07-13_09_52_31__00__evil__Olof-Sk-tkonung_prompt.json"),
            concat!(
                "{\n",
                "  \"prompt\": \"p\\n\\n\",\n",
                "  \"answer\": null,\n",
                "  \"meta\": {\n",
                "    \"actor_id\": \"../evil\",\n",
                "    \"actor_name\": \"Olof Skötkonung\",\n",
                "    \"model\": null,\n",
                "    \"duration_seconds\": 2.0,\n",
                "    \"timestamp\": \"2026-07-13T09:52:31\",\n",
                "    \"error\": \"TimeoutError('provider')\"\n",
                "  }\n",
                "}\n",
            )
        );
        assert_eq!(
            fixture.read("2026-07-13_09_52_31__00__evil__Olof-Sk-tkonung_prompt.md"),
            concat!(
                "# Prompt\n\np\n\n",
                "# Answer\n\n*(no answer)*\n\n",
                "# Meta\n\n",
                "- actor_id: ../evil\n",
                "- actor_name: Olof Skötkonung\n",
                "- model: None\n",
                "- duration_seconds: 2.0\n",
                "- timestamp: 2026-07-13T09:52:31\n",
                "- error: TimeoutError('provider')\n",
            )
        );
    }

    #[test]
    fn the_stamp_and_iso_forms_agree_with_python() {
        let moment = LocalTime {
            year: 2026,
            month: 7,
            day: 13,
            hour: 9,
            minute: 52,
            second: 30,
        };
        assert_eq!(moment.file_stamp(), "2026-07-13_09_52_30");
        assert_eq!(moment.iso_seconds(), "2026-07-13T09:52:30");

        // The UTC fallback still produces a sane civil date.
        let epoch = utc_time(1_768_296_750);
        assert_eq!(epoch.year, 2026);
        assert!(epoch.month >= 1 && epoch.month <= 12);
    }
}
