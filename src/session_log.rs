//! Per-run session logging: every game start creates
//! `logs/session_<n>_<start time>/` (symlinked as `logs/latest_session`)
//! holding that run's `screenshots/`, the LLM `prompts/` archive, and a
//! structured `logs.jsonl` that merges Bevy log events, the actor engine's
//! diagnostics, the speech workers' stderr, and drive-script evidence lines so
//! an agent can parse a whole session later.
//!
//! The session counter lives in `cathedral_meta.json` at the repository root
//! and increments once per game start. `init()` runs before the Bevy app is
//! built — the tracing layer, the screenshot systems, and the actor engine all
//! read the resulting process-wide state instead of threading it through
//! plugins that are constructed in different orders.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::app::App;
use bevy::log::{BoxedLayer, tracing, tracing_subscriber};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, bounded, unbounded};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const META_PATH: &str = "cathedral_meta.json";
const LOGS_DIRECTORY: &str = "logs";
const LATEST_LINK_NAME: &str = "latest_session";

static SESSION: OnceLock<SessionPaths> = OnceLock::new();
static WRITER: OnceLock<Sink> = OnceLock::new();

/// `logs.jsonl`, split so that no producer ever holds a lock across a write
/// syscall.
///
/// Records are appended to `staging` by whoever emitted them — the game's main
/// thread through the tracing layer, the engine pump, the perf recorder, the
/// speech workers' stderr forwarders — and appending to a `Vec<u8>` cannot
/// block on a disk. A flusher then takes `out`, swaps the staged bytes out from
/// under the *shorter* lock, and writes them while still holding `out`: that is
/// what keeps the file in the order the records were staged, which
/// `.claude/rules/LOGS_FOLDER.md` promises is chronological.
struct Sink {
    staging: Mutex<Vec<u8>>,
    out: Mutex<Out>,
    /// Wakes the flusher ahead of its interval. Capacity one: a nudge that
    /// finds one already pending has nothing to add.
    nudge: Sender<()>,
}

struct Out {
    file: File,
    /// The buffer the previous flush emptied, swapped back in so the staging
    /// area's allocation is not rebuilt every interval.
    scratch: Vec<u8>,
}

#[derive(Debug)]
pub struct SessionPaths {
    pub number: u64,
    /// Absolute: the prompt archive under `<root>/prompts` is written from
    /// several places and none of them can rely on the working directory.
    pub root: PathBuf,
    pub screenshots: PathBuf,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct CathedralMeta {
    session: u64,
}

/// Creates this run's session directory tree and opens `logs.jsonl`. Failures
/// are reported to stderr and leave `paths()` empty: the game must still run
/// on a read-only checkout, it just loses captures and file logs.
pub fn init() {
    let number = begin_session(Path::new(META_PATH));
    let directory_name = session_directory_name(number, current_timestamp());
    let logs_root = PathBuf::from(LOGS_DIRECTORY);
    let root = logs_root.join(&directory_name);

    for subdirectory in ["screenshots", "prompts"] {
        if let Err(error) = fs::create_dir_all(root.join(subdirectory)) {
            eprintln!(
                "[session] could not create {}: {error}",
                root.join(subdirectory).display()
            );
            return;
        }
    }
    let root = root.canonicalize().unwrap_or(root);

    if let Err(error) = update_latest_symlink(&logs_root, &directory_name) {
        eprintln!(
            "[session] could not update {}/{LATEST_LINK_NAME}: {error}",
            logs_root.display()
        );
    }

    match File::options()
        .create(true)
        .append(true)
        .open(root.join("logs.jsonl"))
    {
        Ok(file) => {
            let (nudge, wake) = bounded(1);
            let _ = WRITER.set(Sink {
                staging: Mutex::new(Vec::new()),
                out: Mutex::new(Out {
                    file,
                    scratch: Vec::new(),
                }),
                nudge,
            });
            spawn_flusher(wake);
        }
        Err(error) => eprintln!("[session] could not open logs.jsonl: {error}"),
    }

    let _ = SESSION.set(SessionPaths {
        number,
        screenshots: root.join("screenshots"),
        root,
    });
    log_line("session", "INFO", &format!("session {number} started"));
}

pub fn paths() -> Option<&'static SessionPaths> {
    SESSION.get()
}

/// `LogPlugin::custom_layer` hook: mirrors every (already filtered) tracing
/// event into `logs.jsonl` alongside the normal console output.
pub fn custom_layer(_app: &mut App) -> Option<BoxedLayer> {
    WRITER.get().map(|_| Box::new(JsonlLayer) as BoxedLayer)
}

/// Appends one record for a non-tracing line (drive evidence, the actor
/// engine, a speech worker's stderr). A no-op before `init()` or when the
/// session could not be created.
pub fn log_line(source: &str, level: &str, message: &str) {
    write_record(source, level, None, message, Map::new());
}

fn write_record(
    source: &str,
    level: &str,
    target: Option<&str>,
    message: &str,
    extra: Map<String, Value>,
) {
    let Some(sink) = WRITER.get() else { return };
    let epoch_ms = now_epoch_milliseconds();
    let stamp = timestamp_from_unix_seconds(epoch_ms / 1_000);

    let mut record = Map::new();
    record.insert(
        "ts".into(),
        Value::String(format!("{}.{:03}", stamp.human(), epoch_ms % 1_000)),
    );
    record.insert("ts_ms".into(), Value::from(epoch_ms));
    record.insert("source".into(), source.into());
    record.insert("level".into(), level.into());
    if let Some(target) = target {
        record.insert("target".into(), target.into());
    }
    record.insert("message".into(), message.into());
    if !extra.is_empty() {
        record.insert("fields".into(), Value::Object(extra));
    }

    let staged = {
        let Ok(mut staging) = sink.staging.lock() else {
            return;
        };
        // Serializing into the staging buffer cannot touch the disk, so the
        // lock is held for a memcpy and nothing else. A half-serialized record
        // is rolled back rather than left in the file: `logs.jsonl` is parsed a
        // line at a time.
        let whole_records = staging.len();
        if serde_json::to_writer(&mut *staging, &Value::Object(record)).is_ok() {
            staging.push(b'\n');
        } else {
            staging.truncate(whole_records);
            return;
        }
        staging.len()
    };
    // The rare-but-load-bearing sources — drive evidence, the session marker —
    // keep the old line-durability contract (a parsed logs.jsonl is complete
    // on its own) and pay for it on the spot. Everything else, WARN and ERROR
    // included, wakes the flusher instead: it writes within microseconds, so an
    // abort has to land inside *that* window to lose anything, and no frame
    // ever waits on the disk to say something went wrong.
    if source == "drive" || source == "session" {
        flush_now();
    } else if staged >= STAGING_HIGH_WATER {
        // Never reached while the flusher is alive — it drains within an
        // interval, and a whole megabyte of records in one interval is not a
        // run anybody is playing. It is reached when the flusher's thread
        // failed to spawn, and it is what the `BufWriter` used to do for free:
        // a full buffer wrote itself out. Without a ceiling, staging would hold
        // the entire session's log in memory and `logs.jsonl` would stay empty
        // until exit.
        flush_now();
    } else if level != "INFO" {
        let _ = sink.nudge.try_send(());
    }
}

/// The staging buffer's ceiling: past this, whoever is logging writes it out
/// itself rather than let the backlog grow unbounded.
const STAGING_HIGH_WATER: usize = 1024 * 1024;

/// Flush cadence for staged INFO lines. Short enough that tailing
/// `logs.jsonl` still feels live; long enough that logging bursts cost the
/// main thread no syscalls.
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

fn flush_now() {
    if let Some(sink) = WRITER.get() {
        flush_sink(sink);
    }
}

fn flush_sink(sink: &Sink) {
    // `out` first, and held across the write: two flushers that each swapped
    // the staging buffer and then raced to the file would interleave their
    // halves. Producers never take this lock, so they never wait on the write.
    let Ok(mut out) = sink.out.lock() else { return };
    let Out { file, scratch } = &mut *out;
    scratch.clear();
    {
        let Ok(mut staging) = sink.staging.lock() else {
            return;
        };
        if staging.is_empty() {
            return;
        }
        std::mem::swap(&mut *staging, scratch);
    }
    let _ = file.write_all(scratch.as_slice());
}

extern "C" fn flush_at_exit() {
    flush_now();
}

fn spawn_flusher(wake: Receiver<()>) {
    let _ = std::thread::Builder::new()
        .name("cathedral-log-flush".into())
        .spawn(move || {
            loop {
                // A nudge (a WARN or an ERROR) writes it out at once; otherwise
                // the interval does. The sender lives in a static, so the
                // disconnected arm is unreachable — but it would busy-loop.
                if matches!(
                    wake.recv_timeout(FLUSH_INTERVAL),
                    Err(RecvTimeoutError::Disconnected)
                ) {
                    return;
                }
                flush_now();
            }
        });
    // A normal `main` return and `std::process::exit` both run atexit
    // handlers; only a hard abort can now lose staged lines.
    unsafe {
        libc::atexit(flush_at_exit);
    }
}

// ------------------------------------------------------------------- stderr

/// Diagnostic lines that reach stderr without their author paying for the
/// syscall.
///
/// `std::io::Stderr` is unbuffered, and the actor engine's diagnostics are
/// emitted from the engine pump — on the game's main thread, in the middle of a
/// frame — where a terminal that is slow to drain (a tmux pane with a long
/// scrollback, a pipe whose reader has stalled) can block the write for as long
/// as it likes. The bytes and their order are unchanged; only the thread that
/// writes them is, and it does nothing else, so a line still lands within
/// microseconds of being handed over.
enum StderrJob {
    Line(String),
    /// Replies when every line queued before it has been written.
    Barrier(Sender<()>),
}

fn stderr_writer() -> &'static Sender<StderrJob> {
    static STDERR: OnceLock<Sender<StderrJob>> = OnceLock::new();
    STDERR.get_or_init(|| {
        let (sender, lines) = unbounded::<StderrJob>();
        let spawned = std::thread::Builder::new()
            .name("cathedral-stderr".into())
            .spawn(move || {
                let mut stderr = io::stderr();
                for job in lines {
                    match job {
                        StderrJob::Line(line) => {
                            // One `write_all` for the whole line: `eprintln!`
                            // splits the newline into a second syscall.
                            let mut bytes = line.into_bytes();
                            bytes.push(b'\n');
                            let _ = stderr.write_all(&bytes);
                        }
                        StderrJob::Barrier(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            });
        if spawned.is_ok() {
            // A `OnceLock` static never drops, and the drive watchdog leaves
            // via `std::process::exit`; atexit covers both, so only a hard
            // abort can lose a queued line.
            unsafe {
                libc::atexit(drain_stderr_at_exit);
            }
        }
        sender
    })
}

/// Print one line to stderr, from a thread that can afford to wait for it.
pub fn print_line(line: String) {
    if let Err(undelivered) = stderr_writer().send(StderrJob::Line(line))
        && let StderrJob::Line(line) = undelivered.into_inner()
    {
        // No writer thread to hand it to: a blocked frame beats a lost
        // diagnostic.
        eprintln!("{line}");
    }
}

extern "C" fn drain_stderr_at_exit() {
    let (done, wait) = bounded(1);
    if stderr_writer().send(StderrJob::Barrier(done)).is_ok() {
        let _ = wait.recv_timeout(std::time::Duration::from_secs(5));
    }
}

struct JsonlLayer;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for JsonlLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = FieldCollector::default();
        event.record(&mut fields);
        let metadata = event.metadata();
        write_record(
            "rust",
            &metadata.level().to_string(),
            Some(metadata.target()),
            &fields.message,
            fields.extra,
        );
    }
}

#[derive(Default)]
struct FieldCollector {
    message: String,
    extra: Map<String, Value>,
}

impl FieldCollector {
    fn insert(&mut self, name: &str, value: Value) {
        if name == "message" {
            self.message = match value {
                Value::String(text) => text,
                other => other.to_string(),
            };
        } else {
            self.extra.insert(name.into(), value);
        }
    }
}

impl tracing::field::Visit for FieldCollector {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.insert(field.name(), Value::String(format!("{value:?}")));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.insert(field.name(), Value::String(value.into()));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.insert(field.name(), Value::from(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.insert(field.name(), Value::from(value));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.insert(field.name(), Value::from(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.insert(field.name(), Value::from(value));
    }
}

fn session_directory_name(number: u64, timestamp: Timestamp) -> String {
    format!("session_{number}_{}", timestamp.file_stamp())
}

/// Repoints `logs/latest_session` at the new session directory. The relative
/// target keeps the link valid if the repository is moved; the temp-then-
/// rename replaces an existing link atomically.
#[cfg(unix)]
fn update_latest_symlink(logs_root: &Path, directory_name: &str) -> io::Result<()> {
    let temporary = logs_root.join(format!(".{LATEST_LINK_NAME}.tmp"));
    let _ = fs::remove_file(&temporary);
    std::os::unix::fs::symlink(directory_name, &temporary)?;
    fs::rename(&temporary, logs_root.join(LATEST_LINK_NAME))
}

#[cfg(not(unix))]
fn update_latest_symlink(_logs_root: &Path, _directory_name: &str) -> io::Result<()> {
    Ok(())
}

fn begin_session(path: &Path) -> u64 {
    let previous = match fs::read_to_string(path) {
        Ok(source) => match parse_session(&source) {
            Ok(session) => session,
            Err(error) => {
                eprintln!(
                    "[session] could not parse {}: {error}. Restarting the session counter at 1.",
                    path.display()
                );
                0
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => {
            eprintln!(
                "[session] could not read {}: {error}. Restarting the session counter at 1.",
                path.display()
            );
            0
        }
    };

    let session = match next_session(previous) {
        Some(session) => session,
        None => {
            eprintln!(
                "[session] counter in {} reached its maximum value; keeping session {previous}",
                path.display()
            );
            previous
        }
    };

    if let Err(error) = write_meta(path, session) {
        eprintln!(
            "[session] could not update the counter in {}: {error}",
            path.display()
        );
    }

    session
}

fn parse_session(source: &str) -> Result<u64, serde_json::Error> {
    serde_json::from_str::<CathedralMeta>(source).map(|meta| meta.session)
}

fn next_session(previous: u64) -> Option<u64> {
    previous.checked_add(1)
}

fn write_meta(path: &Path, session: u64) -> io::Result<()> {
    let source = serde_json::to_string(&CathedralMeta { session })
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let temporary_path = path.with_extension("json.tmp");

    fs::write(&temporary_path, &source)?;
    match fs::rename(&temporary_path, path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // Some platforms do not replace an existing file during rename. A direct
            // write still leaves a valid counter and is preferable to losing the update.
            let result = fs::write(path, source);
            let _ = fs::remove_file(&temporary_path);
            result.map_err(|_| rename_error)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Timestamp {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Timestamp {
    /// The `2026-07-13_09_52_30` form used in session and file names.
    pub(crate) fn file_stamp(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}_{:02}_{:02}_{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second,
        )
    }

    fn human(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second,
        )
    }
}

pub(crate) fn current_timestamp() -> Timestamp {
    timestamp_from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

fn now_epoch_milliseconds() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn timestamp_from_unix_seconds(unix_seconds: u64) -> Timestamp {
    #[cfg(unix)]
    if let Some(timestamp) = unix_local_timestamp(unix_seconds) {
        return timestamp;
    }

    utc_timestamp(unix_seconds)
}

#[cfg(unix)]
fn unix_local_timestamp(unix_seconds: u64) -> Option<Timestamp> {
    let seconds: libc::time_t = unix_seconds.try_into().ok()?;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();

    // SAFETY: `seconds` and the uninitialized `tm` allocation are both valid for the
    // duration of this call. `localtime_r` initializes the latter before returning it.
    let result = unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) };
    if result.is_null() {
        return None;
    }

    // SAFETY: a non-null return from `localtime_r` means it initialized `local`.
    let local = unsafe { local.assume_init() };
    Some(Timestamp {
        year: i64::from(local.tm_year) + 1900,
        month: u8::try_from(local.tm_mon + 1).ok()?,
        day: u8::try_from(local.tm_mday).ok()?,
        hour: u8::try_from(local.tm_hour).ok()?,
        minute: u8::try_from(local.tm_min).ok()?,
        second: u8::try_from(local.tm_sec).ok()?,
    })
}

fn utc_timestamp(unix_seconds: u64) -> Timestamp {
    const SECONDS_PER_DAY: u64 = 86_400;

    let days = i64::try_from(unix_seconds / SECONDS_PER_DAY).unwrap_or(i64::MAX);
    let seconds_today = unix_seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_date_from_unix_days(days);

    Timestamp {
        year,
        month,
        day,
        hour: u8::try_from(seconds_today / 3_600).expect("hour is in range"),
        minute: u8::try_from((seconds_today % 3_600) / 60).expect("minute is in range"),
        second: u8::try_from(seconds_today % 60).expect("second is in range"),
    }
}

// Converts days since 1970-01-01 to a Gregorian date. This is the civil-calendar
// algorithm by Howard Hinnant, used here to keep the non-Unix fallback dependency-free.
fn civil_date_from_unix_days(days: i64) -> (i64, u8, u8) {
    let adjusted_days = days + 719_468;
    let era = if adjusted_days >= 0 {
        adjusted_days
    } else {
        adjusted_days - 146_096
    } / 146_097;
    let day_of_era = adjusted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    (
        year,
        u8::try_from(month).expect("calendar month is in range"),
        u8::try_from(day).expect("calendar day is in range"),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_meta_path(test_name: &str) -> PathBuf {
        let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cathedralbevy-{test_name}-{}-{nanos}-{unique}.json",
            std::process::id()
        ))
    }

    #[test]
    fn initial_meta_parses_and_increments() {
        let previous = parse_session(r#"{"session": 0}"#).expect("initial meta should parse");

        assert_eq!(previous, 0);
        assert_eq!(next_session(previous), Some(1));
        assert_eq!(next_session(34), Some(35));
    }

    #[test]
    fn invalid_or_exhausted_session_is_detected() {
        assert!(parse_session(r#"{"session": "many"}"#).is_err());
        assert!(parse_session("not JSON").is_err());
        assert_eq!(next_session(u64::MAX), None);
    }

    #[test]
    fn session_file_is_created_then_increased_once_per_start() {
        let path = temporary_meta_path("increment");

        assert_eq!(begin_session(&path), 1);
        assert_eq!(begin_session(&path), 2);
        assert_eq!(
            parse_session(&fs::read_to_string(&path).expect("meta should be readable"))
                .expect("written meta should parse"),
            2
        );

        fs::remove_file(path).expect("temporary meta should be removable");
    }

    #[test]
    fn malformed_session_file_recovers_to_session_one() {
        let path = temporary_meta_path("recovery");
        fs::write(&path, "broken").expect("malformed fixture should be writable");

        assert_eq!(begin_session(&path), 1);
        assert_eq!(
            parse_session(&fs::read_to_string(&path).expect("meta should be readable"))
                .expect("recovered meta should parse"),
            1
        );

        fs::remove_file(path).expect("temporary meta should be removable");
    }

    #[test]
    fn session_directory_name_embeds_number_and_start_time() {
        let timestamp = Timestamp {
            year: 2026,
            month: 7,
            day: 13,
            hour: 9,
            minute: 52,
            second: 30,
        };

        assert_eq!(
            session_directory_name(34, timestamp),
            "session_34_2026-07-13_09_52_30"
        );
        assert_eq!(timestamp.human(), "2026-07-13 09:52:30");
    }

    #[cfg(unix)]
    #[test]
    fn latest_symlink_is_created_and_repointed() {
        let logs_root = std::env::temp_dir().join(format!(
            "cathedralbevy-symlink-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&logs_root).expect("temporary logs root should be creatable");

        update_latest_symlink(&logs_root, "session_1_a").expect("first link should be created");
        update_latest_symlink(&logs_root, "session_2_b").expect("link should be replaced");
        assert_eq!(
            fs::read_link(logs_root.join(LATEST_LINK_NAME)).expect("link should be readable"),
            PathBuf::from("session_2_b")
        );

        fs::remove_dir_all(logs_root).expect("temporary logs root should be removable");
    }

    /// The staged bytes reach the file once, in order, and the recycled scratch
    /// buffer does not re-emit the previous flush — `logs.jsonl` is documented
    /// as one record per line, chronological.
    #[test]
    fn flushing_writes_each_staged_record_exactly_once_and_in_order() {
        let path = temporary_meta_path("staging").with_extension("jsonl");
        let (nudge, _wake) = bounded(1);
        let sink = Sink {
            staging: Mutex::new(Vec::new()),
            out: Mutex::new(Out {
                file: File::options()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .expect("a log file"),
                scratch: Vec::new(),
            }),
            nudge,
        };
        let stage = |line: &str| {
            sink.staging
                .lock()
                .expect("staging")
                .extend_from_slice(line.as_bytes());
        };

        stage("one\n");
        stage("two\n");
        flush_sink(&sink);
        // Nothing staged since: a flush is a no-op, not a repeat of the last.
        flush_sink(&sink);
        stage("three\n");
        flush_sink(&sink);

        assert_eq!(
            fs::read_to_string(&path).expect("the log is readable"),
            "one\ntwo\nthree\n"
        );
        fs::remove_file(path).expect("temporary log should be removable");
    }

    #[test]
    fn utc_fallback_handles_epoch_and_leap_days() {
        assert_eq!(
            utc_timestamp(0),
            Timestamp {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            }
        );
        assert_eq!(
            utc_timestamp(951_827_696),
            Timestamp {
                year: 2000,
                month: 2,
                day: 29,
                hour: 12,
                minute: 34,
                second: 56,
            }
        );
    }
}
