//! Per-run session paths and bounded diagnostic JSONL/console output.
//! Required cognition prompt archives remain a separate admitted service.
use bevy::app::App;
use bevy::log::{BoxedFmtLayer, BoxedLayer, tracing, tracing_subscriber};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod bounded;
mod format;
#[cfg(test)]
mod tests_bounded;
#[cfg(test)]
mod tests_public;
use bounded::{BoundedSink, Refusal, SinkSender};

const META_PATH: &str = "cathedral_meta.json";
const LOGS_DIRECTORY: &str = "logs";
const LATEST_LINK_NAME: &str = "latest_session";
const EVIDENCE_TIMEOUT: Duration = Duration::from_secs(1);
static SESSION: OnceLock<SessionPaths> = OnceLock::new();
static SENDERS: OnceLock<Mutex<Option<Senders>>> = OnceLock::new();
static EVIDENCE_FAILED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct Senders {
    jsonl: Option<SinkSender>,
    stderr: Option<SinkSender>,
}
/// Normal destruction is off-frame and joins actual native workers. The global
/// producers cannot own this join guard. Atexit only performs a bounded fence.
pub(crate) struct SessionLogGuard {
    jsonl: Option<BoundedSink>,
    stderr: Option<BoundedSink>,
}
impl Drop for SessionLogGuard {
    fn drop(&mut self) {
        if let Some(global) = SENDERS.get() {
            global.lock().unwrap().take();
        }
        // Close both before joining either; no producer can keep accepting.
        if let Some(sink) = &self.jsonl {
            sink.sender().close();
        }
        if let Some(sink) = &self.stderr {
            sink.sender().close();
        }
    }
}
#[derive(Debug)]
pub struct SessionPaths {
    pub number: u64,
    pub root: PathBuf,
    pub screenshots: PathBuf,
}
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct CathedralMeta {
    session: u64,
}

/// Startup filesystem work and warnings happen before the app. Both finite
/// sinks retain disjoint leases in the installed startup recipe's shared budget.
pub(crate) fn init(budget: &cathedral_sim::checkpoint::CheckpointBudget) -> SessionLogGuard {
    let stderr = BoundedSink::start_admitted(io::stderr(), budget).ok();
    let number = begin_session(Path::new(META_PATH));
    let directory_name = session_directory_name(number, current_timestamp());
    let logs_root = PathBuf::from(LOGS_DIRECTORY);
    let root = logs_root.join(&directory_name);
    let mut jsonl = None;
    let directory = ["screenshots", "prompts"]
        .into_iter()
        .try_for_each(|subdirectory| fs::create_dir_all(root.join(subdirectory)));
    if let Err(error) = directory {
        eprintln!("[session] could not create {}: {error}", root.display());
    } else {
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
            Ok(file) => match BoundedSink::start_admitted(file, budget) {
                Ok(sink) => jsonl = Some(sink),
                Err(error) => eprintln!("[session] could not start log worker: {error}"),
            },
            Err(error) => eprintln!("[session] could not open logs.jsonl: {error}"),
        }
        let _ = SESSION.set(SessionPaths {
            number,
            screenshots: root.join("screenshots"),
            root,
        });
    }
    *SENDERS.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(Senders {
        jsonl: jsonl.as_ref().map(BoundedSink::sender),
        stderr: stderr.as_ref().map(BoundedSink::sender),
    });
    unsafe {
        libc::atexit(flush_at_exit);
    }
    log_line("session", "INFO", &format!("session {number} started"));
    SessionLogGuard { jsonl, stderr }
}
pub fn paths() -> Option<&'static SessionPaths> {
    SESSION.get()
}
fn senders() -> Option<Senders> {
    SENDERS.get()?.lock().unwrap().clone()
}

/// Both sinks consume bounded event fields. The ordinary Bevy formatter is
/// replaced because its internal String and synchronous stderr writer bypass
/// any queue added only to the JSONL custom layer.
pub fn custom_layer(_app: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(DiagnosticLayer))
}
pub fn fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    Some(Box::new(tracing_subscriber::layer::Identity::new()))
}
struct DiagnosticLayer;
impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for DiagnosticLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(senders) = senders() else { return };
        if let Some(sink) = &senders.jsonl {
            let _ = format::event(sink, event, true);
            report_losses(sink, true);
        }
        if let Some(sink) = &senders.stderr {
            let _ = format::event(sink, event, false);
            report_losses(sink, false);
        }
    }
}

/// Diagnostics may be refused whole; counters and recovery summaries expose
/// loss. Drive/session use reserved storage and a bounded prefix durability wait.
/// Their failure is sticky and makes a drive run's normal exit unsuccessful.
pub fn log_line(source: &str, level: &str, message: &str) {
    let Some(senders) = senders() else { return };
    let evidence = matches!(source, "drive" | "session");
    let Some(sink) = &senders.jsonl else {
        if evidence {
            EVIDENCE_FAILED.store(true, Ordering::Release);
        }
        return;
    };
    let deadline = evidence.then(|| Instant::now() + EVIDENCE_TIMEOUT);
    let result =
        format::line(sink, source, level, message, evidence, deadline).and_then(|ticket| {
            if let Some(deadline) = deadline {
                sink.fence(Some(ticket), deadline)
            } else {
                Ok(())
            }
        });
    if evidence && result.is_err() {
        EVIDENCE_FAILED.store(true, Ordering::Release);
    }
    report_losses(sink, true);
}
pub(crate) fn evidence_failed() -> bool {
    EVIDENCE_FAILED.load(Ordering::Acquire)
}

/// The input is already caller-owned. Admission refuses excessive capacity and
/// returns it intact at the isolated seam; this diagnostic convenience drops a
/// refused input after recording its bounded loss counter.
pub fn print_line(line: String) {
    if let Some(sink) = senders().and_then(|s| s.stderr) {
        let _ = sink.line_owned(line);
        report_losses(&sink, false);
    }
}
pub(crate) fn print_args(args: std::fmt::Arguments<'_>) {
    if let Some(sink) = senders().and_then(|s| s.stderr) {
        let _ = print_to(&sink, args);
        report_losses(&sink, false);
    }
}
fn print_to(sink: &SinkSender, args: std::fmt::Arguments<'_>) -> Result<bounded::Ticket, Refusal> {
    let mut draft = sink.begin(false, None)?;
    let (_, output) = draft.parts();
    let mut out = bounded::SliceWriter::new(output);
    if out.write_fmt(args).is_err() {
        sink.refuse(Refusal::Oversized);
        return Err(Refusal::Oversized);
    }
    let len = out.len;
    draft.set_body_len(len);
    draft.console_body();
    sink.commit(draft, false)
}
fn report_losses(sink: &SinkSender, json: bool) {
    let Some(losses) = sink.unreported_losses() else {
        return;
    };
    let mut storage = [0; 256];
    let mut out = bounded::SliceWriter::new(&mut storage);
    let _ = write!(
        out,
        "diagnostic records refused: full={} oversized={} closed={}",
        losses.0, losses.1, losses.2
    );
    let len = out.len;
    let message = std::str::from_utf8(&storage[..len]).unwrap();
    let result = if json {
        format::line(sink, "diagnostics", "WARN", message, false, None)
    } else {
        print_to(sink, format_args!("[diagnostics] {message}"))
    };
    if result.is_ok() {
        sink.mark_reported(losses);
    }
}
extern "C" fn flush_at_exit() {
    let deadline = Instant::now() + Duration::from_secs(5);
    if let Some(senders) = senders() {
        if let Some(sink) = &senders.jsonl {
            report_losses(sink, true);
            if sink.fence(None, deadline).is_err() {
                EVIDENCE_FAILED.store(true, Ordering::Release);
            }
        }
        if let Some(sink) = &senders.stderr {
            report_losses(sink, false);
            let _ = sink.fence(None, deadline);
        }
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

    #[test]
    fn flushing_writes_each_staged_record_exactly_once_and_in_order() {
        let path = temporary_meta_path("staging").with_extension("jsonl");
        let sink = BoundedSink::start_default(File::create(&path).unwrap()).unwrap();
        let sender = sink.sender();
        sender.line_owned("one".into()).unwrap();
        let prefix = sender.line_owned("two".into()).unwrap();
        sender
            .fence(Some(prefix), Instant::now() + Duration::from_secs(2))
            .unwrap();
        sender
            .fence(Some(prefix), Instant::now() + Duration::from_secs(2))
            .unwrap();
        let last = sender.line_owned("three".into()).unwrap();
        sender
            .fence(Some(last), Instant::now() + Duration::from_secs(2))
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\nthree\n");
        drop(sink);
        drop(sender);
        fs::remove_file(path).unwrap();
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
