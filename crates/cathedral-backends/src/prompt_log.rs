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

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cathedral_sim::{SchedulerEvent, py_round};
use serde::Serialize;

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

impl PromptExchange {
    /// The scheduler's event, if it is one — anything else is not an exchange.
    pub fn from_scheduler_event(event: &SchedulerEvent) -> Option<Self> {
        match event {
            SchedulerEvent::PromptExchange {
                actor_id,
                actor_name,
                prompt,
                answer,
                duration_seconds,
                error,
            } => Some(Self {
                actor_id: actor_id.as_str().to_string(),
                actor_name: actor_name.clone(),
                prompt: prompt.clone(),
                answer: answer.clone(),
                duration_seconds: *duration_seconds,
                error: error.clone(),
            }),
            _ => None,
        }
    }
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
    directory: Option<PathBuf>,
    model: Option<String>,
    clock: Box<dyn FnMut() -> LocalTime + Send>,
    last_stamp: String,
    next_index: u32,
}

impl std::fmt::Debug for PromptLog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PromptLog")
            .field("directory", &self.directory)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl PromptLog {
    /// `model` is the prompt log's `meta.model`: the provider's model name, or
    /// `"fake"` / `"injected"` (`server.py:534-543`).
    pub fn new(directory: Option<PathBuf>, model: Option<String>) -> Self {
        Self::with_clock(directory, model, Box::new(LocalTime::now))
    }

    /// The injectable-clock constructor the tests use.
    pub fn with_clock(
        directory: Option<PathBuf>,
        model: Option<String>,
        clock: Box<dyn FnMut() -> LocalTime + Send>,
    ) -> Self {
        Self {
            directory,
            model,
            clock,
            last_stamp: String::new(),
            next_index: 0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.directory.is_some()
    }

    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    /// Archive one exchange. Write failures are swallowed (after a stderr line):
    /// a logging problem must never break the turn loop (`prompt_log.py:96-100`).
    pub fn record(&mut self, exchange: &PromptExchange) {
        let Some(directory) = self.directory.clone() else {
            return;
        };
        let moment = (self.clock)();
        let stamp = moment.file_stamp();
        if stamp != self.last_stamp {
            self.last_stamp = stamp.clone();
            self.next_index = 0;
        }
        let index = self.next_index;
        self.next_index += 1;

        let base = format!(
            "{stamp}__{index:02}__{}__{}_prompt",
            safe(&exchange.actor_id),
            safe(&exchange.actor_name),
        );

        let meta = Meta {
            actor_id: &exchange.actor_id,
            actor_name: &exchange.actor_name,
            model: self.model.as_deref(),
            duration_seconds: py_round(exchange.duration_seconds, 3),
            timestamp: moment.iso_seconds(),
            error: exchange.error.as_deref(),
        };

        if let Err(error) = write_pair(&directory, &base, exchange, meta) {
            eprintln!("[smart actors] prompt log write failed: {error}");
        }
    }

    /// Convenience for the host's `SchedulerEvent`/`EngineMessage` drain loop:
    /// non-exchange events are ignored.
    pub fn record_scheduler_event(&mut self, event: &SchedulerEvent) {
        if let Some(exchange) = PromptExchange::from_scheduler_event(event) {
            self.record(&exchange);
        }
    }
}

fn write_pair(
    directory: &Path,
    base: &str,
    exchange: &PromptExchange,
    meta: Meta<'_>,
) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;

    let markdown = markdown(exchange, &meta);

    // `ensure_ascii=False, indent=2` + a trailing newline: raw UTF-8, unlike the
    // ASCII-escaped character sheet inside the prompt itself.
    let record = Record {
        prompt: &exchange.prompt,
        answer: exchange.answer.as_deref(),
        meta,
    };
    let mut json = serde_json::to_string_pretty(&record)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    json.push('\n');
    fs::write(directory.join(format!("{base}.json")), json)?;
    fs::write(directory.join(format!("{base}.md")), markdown)?;
    Ok(())
}

/// `# Prompt` / `# Answer` / `# Meta`, with Python's `str()` stringification of
/// the meta values (`prompt_log.py:103-109`).
fn markdown(exchange: &PromptExchange, meta: &Meta<'_>) -> String {
    let mut lines: Vec<String> = vec![
        "# Prompt".to_string(),
        String::new(),
        // `rstrip("\n")` strips newlines only, not spaces.
        exchange.prompt.trim_end_matches('\n').to_string(),
        String::new(),
        "# Answer".to_string(),
        String::new(),
    ];
    lines.push(match exchange.answer.as_deref() {
        Some(answer) => answer.trim_end_matches('\n').to_string(),
        None => "*(no answer)*".to_string(),
    });
    lines.extend(["".to_string(), "# Meta".to_string(), String::new()]);

    lines.push(format!("- actor_id: {}", meta.actor_id));
    lines.push(format!("- actor_name: {}", meta.actor_name));
    // Python's `str(None)`.
    lines.push(format!("- model: {}", meta.model.unwrap_or("None")));
    // Python's `str(float)` keeps the `.0` on whole numbers; Rust's `{}` would
    // print `2`, so this must be the `{:?}` (shortest round-trip) form.
    lines.push(format!("- duration_seconds: {:?}", meta.duration_seconds));
    lines.push(format!("- timestamp: {}", meta.timestamp));
    if let Some(error) = meta.error {
        lines.push(format!("- error: {error}"));
    }
    lines.push(String::new());
    lines.join("\n")
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
            let log = PromptLog::with_clock(
                Some(directory.clone()),
                Some("kimi-k2.5".to_string()),
                Box::new(move || *clock.lock().expect("clock")),
            );
            Self {
                directory,
                moment,
                log,
            }
        }

        /// `PromptLogTests.record` with its default arguments.
        fn record(&mut self) {
            self.log.record(&exchange());
        }

        fn names(&self, extension: &str) -> Vec<String> {
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
        fixture.log.record(&PromptExchange {
            prompt: "Ilse sa: \"Hej då\"".to_string(),
            ..exchange()
        });

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
        // ensure_ascii=False: the archive is raw UTF-8 even though the sheet
        // inside the prompt is \uXXXX-escaped.
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
        fixture.log.record(&PromptExchange {
            answer: None,
            error: Some("TimeoutError('provider')".to_string()),
            ..exchange()
        });

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
        fixture.log.record(&PromptExchange {
            actor_id: "../evil".to_string(),
            actor_name: "Olof Skötkonung".to_string(),
            ..exchange()
        });

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
        log.record(&exchange()); // must not panic, must not write anywhere
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
        log.record(&exchange()); // swallowed

        fs::remove_file(&blocker).ok();
    }

    #[test]
    fn a_missing_model_prints_pythons_none() {
        let mut fixture = Fixture::new("no-model");
        fixture.log.model = None;
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
        fixture.log.record(&PromptExchange {
            duration_seconds: 2.0,
            ..exchange()
        });
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
            .record_scheduler_event(&SchedulerEvent::Diagnostic("noise".to_string()));
        assert!(!fixture.directory.exists(), "no exchange, no directory");

        fixture
            .log
            .record_scheduler_event(&SchedulerEvent::PromptExchange {
                actor_id: cathedral_sim::ActorId::new("k0fb1").expect("id"),
                actor_name: "Ilse".to_string(),
                prompt: "the prompt".to_string(),
                answer: Some("wait {}".to_string()),
                duration_seconds: 0.5,
                error: None,
            });
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
        fixture.log.record(&PromptExchange {
            prompt: "Ilse sa: \"Hej då\"".to_string(),
            ..exchange()
        });

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
        failed.record(&PromptExchange {
            actor_id: "../evil".to_string(),
            actor_name: "Olof Skötkonung".to_string(),
            prompt: "p\n\n".to_string(),
            answer: None,
            duration_seconds: 2.0,
            error: Some("TimeoutError('provider')".to_string()),
        });

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
