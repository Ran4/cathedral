use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::{
    input::keyboard::Key,
    log::{error, info, warn},
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use serde::{Deserialize, Serialize};

const META_PATH: &str = "cathedral_meta.json";
const SCREENSHOT_DIRECTORY: &str = "screenshots";
const SCREENSHOT_EXTENSION: &str = "png";

pub struct CathedralScreenshotPlugin;

#[derive(Debug, Resource)]
struct ScreenshotSession {
    number: u64,
    output_directory: PathBuf,
    session_directory: PathBuf,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct CathedralMeta {
    session: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaptureTimestamp {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Plugin for CathedralScreenshotPlugin {
    fn build(&self, app: &mut App) {
        let session = begin_session(Path::new(META_PATH));
        let output_directory = PathBuf::from(SCREENSHOT_DIRECTORY);
        let session_directory = output_directory.join(format!("session_{session}"));

        if let Err(error) = fs::create_dir_all(&session_directory) {
            error!(
                "Could not create screenshot directory {}: {error}",
                session_directory.display()
            );
        }

        info!("Cathedral screenshot session {session}");
        app.insert_resource(ScreenshotSession {
            number: session,
            output_directory,
            session_directory,
        })
        .add_systems(Update, capture_screenshot_on_key);
    }
}

fn begin_session(path: &Path) -> u64 {
    let previous = match fs::read_to_string(path) {
        Ok(source) => match parse_session(&source) {
            Ok(session) => session,
            Err(error) => {
                warn!(
                    "Could not parse {}: {error}. Restarting the screenshot session counter at 1.",
                    path.display()
                );
                0
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => {
            warn!(
                "Could not read {}: {error}. Restarting the screenshot session counter at 1.",
                path.display()
            );
            0
        }
    };

    let session = match next_session(previous) {
        Some(session) => session,
        None => {
            error!(
                "Screenshot session counter in {} reached its maximum value; keeping session {previous}",
                path.display()
            );
            previous
        }
    };

    if let Err(error) = write_meta(path, session) {
        error!(
            "Could not update screenshot session in {}: {error}",
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

fn capture_screenshot_on_key(
    mut commands: Commands,
    physical_keys: Res<ButtonInput<KeyCode>>,
    logical_keys: Res<ButtonInput<Key>>,
    session: Res<ScreenshotSession>,
) {
    if !screenshot_key_just_pressed(&physical_keys, &logical_keys) {
        return;
    }

    if let Err(error) = fs::create_dir_all(&session.session_directory) {
        error!(
            "Could not create screenshot directory {}: {error}",
            session.session_directory.display()
        );
        return;
    }

    let timestamp = current_timestamp();
    let archival_path = archival_screenshot_path(
        &session.output_directory,
        session.number,
        timestamp,
        SCREENSHOT_EXTENSION,
    );
    let latest_path = latest_screenshot_path(&session.output_directory, SCREENSHOT_EXTENSION);

    info!("Capturing screenshot to {}", archival_path.display());
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(archival_path))
        .observe(save_to_disk(latest_path));
}

fn screenshot_key_just_pressed(
    physical_keys: &ButtonInput<KeyCode>,
    logical_keys: &ButtonInput<Key>,
) -> bool {
    physical_keys.just_pressed(KeyCode::F5)
        // The acute-accent key is physically `Equal` on a Swedish keyboard.
        || physical_keys.just_pressed(KeyCode::Equal)
        // Accept the US grave-key position too, for layouts that report it there.
        || physical_keys.just_pressed(KeyCode::Backquote)
        || logical_keys.just_pressed(Key::Dead(Some('\u{b4}')))
        || logical_keys.just_pressed(Key::Dead(Some('\u{301}')))
        || logical_keys.just_pressed(Key::Character("\u{b4}".into()))
}

fn current_timestamp() -> CaptureTimestamp {
    let unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    #[cfg(unix)]
    if let Some(timestamp) = unix_local_timestamp(unix_seconds) {
        return timestamp;
    }

    #[cfg(unix)]
    warn!("Could not determine local time for screenshot filename; using UTC");
    utc_timestamp(unix_seconds)
}

#[cfg(unix)]
fn unix_local_timestamp(unix_seconds: u64) -> Option<CaptureTimestamp> {
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
    Some(CaptureTimestamp {
        year: i64::from(local.tm_year) + 1900,
        month: u8::try_from(local.tm_mon + 1).ok()?,
        day: u8::try_from(local.tm_mday).ok()?,
        hour: u8::try_from(local.tm_hour).ok()?,
        minute: u8::try_from(local.tm_min).ok()?,
        second: u8::try_from(local.tm_sec).ok()?,
    })
}

fn utc_timestamp(unix_seconds: u64) -> CaptureTimestamp {
    const SECONDS_PER_DAY: u64 = 86_400;

    let days = i64::try_from(unix_seconds / SECONDS_PER_DAY).unwrap_or(i64::MAX);
    let seconds_today = unix_seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_date_from_unix_days(days);

    CaptureTimestamp {
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

fn archival_screenshot_path(
    output_directory: &Path,
    session: u64,
    timestamp: CaptureTimestamp,
    extension: &str,
) -> PathBuf {
    output_directory
        .join(format!("session_{session}"))
        .join(format!(
            "cathedral_screenshot_{:04}-{:02}-{:02}_{:02}_{:02}_{:02}.{extension}",
            timestamp.year,
            timestamp.month,
            timestamp.day,
            timestamp.hour,
            timestamp.minute,
            timestamp.second,
        ))
}

fn latest_screenshot_path(output_directory: &Path, extension: &str) -> PathBuf {
    output_directory.join(format!("cathedral_screenshot_latest.{extension}"))
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
    fn f5_and_swedish_acute_key_request_a_capture() {
        let mut f5_keys = ButtonInput::<KeyCode>::default();
        f5_keys.press(KeyCode::F5);
        assert!(screenshot_key_just_pressed(
            &f5_keys,
            &ButtonInput::<Key>::default()
        ));

        let mut acute_keys = ButtonInput::<Key>::default();
        acute_keys.press(Key::Dead(Some('\u{b4}')));
        assert!(screenshot_key_just_pressed(
            &ButtonInput::<KeyCode>::default(),
            &acute_keys
        ));

        assert!(!screenshot_key_just_pressed(
            &ButtonInput::<KeyCode>::default(),
            &ButtonInput::<Key>::default()
        ));
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
    fn archival_path_contains_session_directory_and_local_timestamp() {
        let timestamp = CaptureTimestamp {
            year: 2026,
            month: 7,
            day: 11,
            hour: 14,
            minute: 34,
            second: 50,
        };

        assert_eq!(
            archival_screenshot_path(Path::new("screenshots"), 34, timestamp, "png"),
            Path::new("screenshots/session_34/cathedral_screenshot_2026-07-11_14_34_50.png")
        );
        assert_eq!(
            latest_screenshot_path(Path::new("screenshots"), "png"),
            Path::new("screenshots/cathedral_screenshot_latest.png")
        );
    }

    #[test]
    fn utc_fallback_handles_epoch_and_leap_days() {
        assert_eq!(
            utc_timestamp(0),
            CaptureTimestamp {
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
            CaptureTimestamp {
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
