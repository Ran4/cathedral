//! Agent drive mode: `CATHEDRAL_DRIVE` scripts the game from the inside so
//! Claude and CI can verify changes without synthetic X11 input.
//!
//! The env var holds a `;`-separated list of actions (`key Escape`,
//! `click Continue`, `shot menu_open`, `sleep 2`, `wait-online`, `quit`).
//! Actions inject real `ButtonInput<KeyCode>` presses and `Interaction`
//! transitions, so every existing keybinding and button handler works
//! unchanged. Each fired action prints a `[drive] 3.2s key Escape` line to
//! stdout as the evidence trail. Without the env var the plugin is never
//! added and there is zero behavior change.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::reflect::enums::{DynamicEnum, DynamicVariant};
use bevy::reflect::{TypeInfo, Typed};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::ui::UiSystems;

use crate::smart_actors::SmartActorRuntime;

pub const DRIVE_ENV: &str = "CATHEDRAL_DRIVE";
pub const SHOT_ENV: &str = "CATHEDRAL_SHOT";
pub const TIMEOUT_ENV: &str = "CATHEDRAL_DRIVE_TIMEOUT";

const DRIVE_DIRECTORY: &str = "screenshots/drive";
/// Default spacing between actions; every action is additionally at least one
/// frame apart because the scheduler fires at most one directive per tick.
const ACTION_SPACING: f64 = 0.5;
const WAIT_ONLINE_TIMEOUT: f64 = 30.0;
const AUTO_QUIT_DELAY: f64 = 2.0;
const DEFAULT_RUN_TIMEOUT: f64 = 60.0;

pub struct DrivePlugin {
    actions: Vec<Action>,
    timeout: Duration,
}

impl DrivePlugin {
    /// Builds the plugin from `CATHEDRAL_DRIVE` or the `CATHEDRAL_SHOT`
    /// shorthand. Returns `None` when neither is set. A malformed script or
    /// timeout terminates the process immediately: half a script must never
    /// run and pass for the whole one.
    pub fn from_env() -> Option<Self> {
        let script = match std::env::var(DRIVE_ENV) {
            Ok(script) => script,
            Err(_) => shot_shorthand(&std::env::var(SHOT_ENV).ok()?),
        };
        let actions = parse_script(&script).unwrap_or_else(|error| {
            eprintln!("[drive] could not parse {DRIVE_ENV}: {error}");
            std::process::exit(2);
        });
        let timeout = match std::env::var(TIMEOUT_ENV) {
            Ok(raw) => match raw.trim().parse::<f64>() {
                Ok(seconds) if seconds.is_finite() && seconds > 0.0 => seconds,
                _ => {
                    eprintln!(
                        "[drive] invalid {TIMEOUT_ENV} value `{raw}`: expected positive seconds"
                    );
                    std::process::exit(2);
                }
            },
            Err(_) => DEFAULT_RUN_TIMEOUT,
        };
        Some(Self {
            actions,
            timeout: Duration::from_secs_f64(timeout),
        })
    }
}

impl Plugin for DrivePlugin {
    fn build(&self, app: &mut App) {
        println!(
            "[drive] script has {} action(s); watchdog aborts after {}s",
            self.actions.len(),
            self.timeout.as_secs_f64()
        );
        spawn_watchdog(self.timeout);
        app.insert_resource(DriveState {
            scheduler: Scheduler::new(self.actions.clone()),
            shot_saved: None,
            pressed_key: None,
        })
        .add_systems(
            PreUpdate,
            // After InputSystems so the injected press survives the frame's
            // `ButtonInput` clear, and after UiSystems::Focus so an injected
            // `Interaction::Pressed` is not overwritten until the focus
            // system naturally resets it (to None) on the next frame.
            run_drive_script
                .after(InputSystems)
                .after(UiSystems::Focus),
        );
    }
}

/// A hung run (GPU stall, sidecar deadlock, window that never opens) must not
/// strand a background process; systems may not be ticking at all, so the
/// watchdog lives on a plain OS thread and hard-aborts.
fn spawn_watchdog(timeout: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        eprintln!(
            "[drive] watchdog: run exceeded {}s; aborting",
            timeout.as_secs_f64()
        );
        std::process::exit(124);
    });
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Key(KeyCode),
    Click(String),
    Shot(String),
    Sleep(f64),
    WaitOnline,
    Quit,
}

impl Action {
    fn describe(&self) -> String {
        match self {
            Self::Key(key) => format!("key {key:?}"),
            Self::Click(name) => format!("click {name}"),
            Self::Shot(name) => format!("shot {name}"),
            Self::Sleep(seconds) => format!("sleep {seconds}"),
            Self::WaitOnline => "wait-online".into(),
            Self::Quit => "quit".into(),
        }
    }
}

pub fn shot_shorthand(name: &str) -> String {
    format!("sleep 2; shot {name}; quit")
}

fn parse_script(source: &str) -> Result<Vec<Action>, String> {
    source
        .split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(parse_statement)
        .collect()
}

fn parse_statement(statement: &str) -> Result<Action, String> {
    let (verb, argument) = match statement.split_once(char::is_whitespace) {
        Some((verb, argument)) => (verb, argument.trim()),
        None => (statement, ""),
    };
    match verb {
        "key" => {
            let key = keycode_from_name(argument)
                .ok_or_else(|| format!("unknown key code `{argument}` in `{statement}`"))?;
            Ok(Action::Key(key))
        }
        "click" if !argument.is_empty() => Ok(Action::Click(argument.into())),
        "click" => Err("`click` needs a name substring, e.g. `click Continue`".into()),
        "shot" => {
            if argument.is_empty() {
                return Err("`shot` needs a file name, e.g. `shot menu_open`".into());
            }
            if argument.contains(['/', '\\']) || argument.contains("..") {
                return Err(format!(
                    "screenshot name `{argument}` must be a plain file name without path separators"
                ));
            }
            Ok(Action::Shot(argument.into()))
        }
        "sleep" => match argument.parse::<f64>() {
            Ok(seconds) if seconds.is_finite() && seconds >= 0.0 => Ok(Action::Sleep(seconds)),
            _ => Err(format!("bad sleep duration `{argument}` in `{statement}`")),
        },
        "wait-online" if argument.is_empty() => Ok(Action::WaitOnline),
        "quit" if argument.is_empty() => Ok(Action::Quit),
        "wait-online" | "quit" => Err(format!("`{verb}` takes no argument, got `{statement}`")),
        _ => Err(format!("unknown action `{verb}` in `{statement}`")),
    }
}

/// Resolves a `KeyCode` variant by name (`Escape`, `KeyZ`, `F5`) through
/// reflection, so the whole enum is accepted without a hand-written table.
fn keycode_from_name(name: &str) -> Option<KeyCode> {
    let TypeInfo::Enum(info) = KeyCode::type_info() else {
        return None;
    };
    if !info.contains_variant(name) {
        return None;
    }
    // `Unidentified(NativeKeyCode)` is the only non-unit variant; it fails
    // `from_reflect` here, which correctly rejects it as unscriptable.
    KeyCode::from_reflect(&DynamicEnum::new(name, DynamicVariant::Unit))
}

/// What the ECS layer must carry out on the current frame.
#[derive(Debug, Clone, PartialEq)]
enum Directive {
    PressKey(KeyCode),
    Click(String),
    Shot(String),
    Quit,
}

/// Pure action scheduler: no ECS access, fully unit-testable. Fires at most
/// one directive per tick, which guarantees actions are at least one frame
/// apart regardless of spacing.
struct Scheduler {
    actions: Vec<Action>,
    index: usize,
    next_at: f64,
    online_deadline: Option<f64>,
    awaiting_shot: bool,
    auto_quit_at: Option<f64>,
    finished: bool,
    log: Vec<String>,
}

impl Scheduler {
    fn new(actions: Vec<Action>) -> Self {
        Self {
            actions,
            index: 0,
            // Give the window a beat to open before the first action.
            next_at: ACTION_SPACING,
            online_deadline: None,
            awaiting_shot: false,
            auto_quit_at: None,
            finished: false,
            log: Vec::new(),
        }
    }

    fn push_log(&mut self, now: f64, message: &str) {
        self.log.push(format!("[drive] {now:.1}s {message}"));
    }

    fn drain_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// `online` is `None` when the smart-actor runtime does not exist (actors
    /// disabled in config), `Some(ready)` otherwise. `shot_saved` reports
    /// whether the most recently requested screenshot has reached disk.
    fn tick(&mut self, now: f64, online: Option<bool>, shot_saved: bool) -> Option<Directive> {
        if self.finished {
            return None;
        }
        if self.awaiting_shot {
            if !shot_saved {
                return None;
            }
            self.awaiting_shot = false;
            self.next_at = now + ACTION_SPACING;
            return None;
        }
        if let Some(deadline) = self.online_deadline {
            match online {
                Some(true) => self.push_log(now, "online"),
                None => self.push_log(
                    now,
                    "warning: smart actors are disabled; wait-online skipped",
                ),
                Some(false) if now >= deadline => self.push_log(
                    now,
                    "error: wait-online timed out after 30s; continuing anyway",
                ),
                Some(false) => return None,
            }
            self.online_deadline = None;
            self.next_at = now + ACTION_SPACING;
            return None;
        }
        if now < self.next_at {
            return None;
        }
        let Some(action) = self.actions.get(self.index).cloned() else {
            return self.tick_auto_quit(now);
        };
        self.index += 1;
        self.next_at = now + ACTION_SPACING;
        self.push_log(now, &action.describe());
        match action {
            Action::Key(key) => Some(Directive::PressKey(key)),
            Action::Click(name) => Some(Directive::Click(name)),
            Action::Shot(name) => {
                self.awaiting_shot = true;
                Some(Directive::Shot(name))
            }
            Action::Sleep(seconds) => {
                self.next_at = now + seconds;
                None
            }
            Action::WaitOnline => {
                self.online_deadline = Some(now + WAIT_ONLINE_TIMEOUT);
                None
            }
            Action::Quit => {
                self.finished = true;
                Some(Directive::Quit)
            }
        }
    }

    fn tick_auto_quit(&mut self, now: f64) -> Option<Directive> {
        match self.auto_quit_at {
            None => {
                self.auto_quit_at = Some(now + AUTO_QUIT_DELAY);
                None
            }
            Some(at) if now >= at => {
                self.finished = true;
                self.push_log(now, "quit (automatic after script end)");
                Some(Directive::Quit)
            }
            Some(_) => None,
        }
    }
}

#[derive(Resource)]
struct DriveState {
    scheduler: Scheduler,
    /// Set by the screenshot-captured observer once `save_to_disk` can no
    /// longer be outrun: both observers fire in the same trigger flush, a
    /// frame before the scheduler reads this.
    shot_saved: Option<Arc<AtomicBool>>,
    /// Key injected last frame, released on the next so handlers see a full
    /// press/release cycle.
    pressed_key: Option<KeyCode>,
}

fn run_drive_script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut state: ResMut<DriveState>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    runtime: Option<Res<SmartActorRuntime>>,
    mut interactions: Query<(&Name, &mut Interaction)>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(key) = state.pressed_key.take() {
        keys.release(key);
    }

    let now = time.elapsed_secs_f64();
    let online = runtime.map(|runtime| runtime.interactions_enabled());
    let shot_saved = state
        .shot_saved
        .as_ref()
        .is_none_or(|saved| saved.load(Ordering::Acquire));

    let directive = state.scheduler.tick(now, online, shot_saved);
    if !state.scheduler.awaiting_shot {
        state.shot_saved = None;
    }
    for line in state.scheduler.drain_log() {
        println!("{line}");
    }

    match directive {
        None => {}
        Some(Directive::PressKey(key)) => {
            keys.press(key);
            state.pressed_key = Some(key);
        }
        Some(Directive::Click(name)) => {
            let needle = name.to_lowercase();
            let mut target = interactions
                .iter_mut()
                .find(|(entity_name, _)| entity_name.as_str().to_lowercase().contains(&needle));
            match target.as_mut() {
                // The UI focus system resets this to `None` next frame, which
                // produces the `Changed<Interaction>` press the handlers expect.
                Some((_, interaction)) => **interaction = Interaction::Pressed,
                None => println!("[drive] {now:.1}s warning: no UI entity named like `{name}`"),
            }
        }
        Some(Directive::Shot(name)) => {
            if let Err(error) = fs::create_dir_all(DRIVE_DIRECTORY) {
                println!("[drive] {now:.1}s warning: could not create {DRIVE_DIRECTORY}: {error}");
            }
            let path = PathBuf::from(DRIVE_DIRECTORY).join(format!("{name}.png"));
            let saved = Arc::new(AtomicBool::new(false));
            state.shot_saved = Some(saved.clone());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path))
                .observe(move |_: On<ScreenshotCaptured>| saved.store(true, Ordering::Release));
        }
        Some(Directive::Quit) => {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_script_parses() {
        let actions = parse_script(
            "wait-online; key Escape; shot menu_open; click Local Canary-Qwen; sleep 1.5; quit",
        )
        .expect("example script should parse");

        assert_eq!(
            actions,
            vec![
                Action::WaitOnline,
                Action::Key(KeyCode::Escape),
                Action::Shot("menu_open".into()),
                Action::Click("Local Canary-Qwen".into()),
                Action::Sleep(1.5),
                Action::Quit,
            ]
        );
    }

    #[test]
    fn empty_statements_are_skipped() {
        assert_eq!(parse_script(""), Ok(vec![]));
        assert_eq!(parse_script(" ; ;; quit "), Ok(vec![Action::Quit]));
    }

    #[test]
    fn parse_errors_name_the_bad_token() {
        let error = parse_script("key Escape; dance").expect_err("unknown verb should fail");
        assert!(error.contains("dance"), "was: {error}");

        let error = parse_script("key Escapee").expect_err("unknown key should fail");
        assert!(error.contains("Escapee"), "was: {error}");

        let error = parse_script("sleep soon").expect_err("bad duration should fail");
        assert!(error.contains("soon"), "was: {error}");

        let error = parse_script("quit now").expect_err("quit takes no argument");
        assert!(error.contains("quit now"), "was: {error}");

        assert!(parse_script("click").is_err());
        assert!(parse_script("shot").is_err());
    }

    #[test]
    fn screenshot_names_must_stay_inside_the_drive_directory() {
        assert!(parse_script("shot ../escape").is_err());
        assert!(parse_script("shot sub/dir").is_err());
        assert!(parse_script("shot back\\slash").is_err());
        assert!(parse_script("shot menu_open").is_ok());
    }

    #[test]
    fn keycodes_resolve_by_variant_name() {
        assert_eq!(keycode_from_name("Escape"), Some(KeyCode::Escape));
        assert_eq!(keycode_from_name("KeyZ"), Some(KeyCode::KeyZ));
        assert_eq!(keycode_from_name("F5"), Some(KeyCode::F5));
        assert_eq!(keycode_from_name("NotAKey"), None);
        // The one tuple variant cannot be scripted.
        assert_eq!(keycode_from_name("Unidentified"), None);
    }

    #[test]
    fn shot_shorthand_expands_to_a_valid_script() {
        assert_eq!(
            parse_script(&shot_shorthand("smoke")),
            Ok(vec![
                Action::Sleep(2.0),
                Action::Shot("smoke".into()),
                Action::Quit,
            ])
        );
    }

    #[test]
    fn actions_fire_in_order_with_default_spacing() {
        let mut scheduler = Scheduler::new(vec![
            Action::Key(KeyCode::Escape),
            Action::Key(KeyCode::KeyZ),
        ]);

        assert_eq!(scheduler.tick(0.0, None, true), None);
        assert_eq!(
            scheduler.tick(0.5, None, true),
            Some(Directive::PressKey(KeyCode::Escape))
        );
        // Same instant again: gated, so actions are never batched into one frame.
        assert_eq!(scheduler.tick(0.5, None, true), None);
        assert_eq!(scheduler.tick(0.9, None, true), None);
        assert_eq!(
            scheduler.tick(1.0, None, true),
            Some(Directive::PressKey(KeyCode::KeyZ))
        );
        assert_eq!(
            scheduler.drain_log(),
            vec!["[drive] 0.5s key Escape", "[drive] 1.0s key KeyZ"]
        );
    }

    #[test]
    fn sleep_extends_the_gap_to_the_next_action() {
        let mut scheduler = Scheduler::new(vec![Action::Sleep(3.0), Action::Quit]);

        assert_eq!(scheduler.tick(0.5, None, true), None);
        assert_eq!(scheduler.tick(3.0, None, true), None);
        assert_eq!(scheduler.tick(3.5, None, true), Some(Directive::Quit));
    }

    #[test]
    fn wait_online_holds_until_ready() {
        let mut scheduler = Scheduler::new(vec![Action::WaitOnline, Action::Quit]);

        assert_eq!(scheduler.tick(0.5, Some(false), true), None);
        assert_eq!(scheduler.tick(5.0, Some(false), true), None);
        assert_eq!(scheduler.tick(6.0, Some(true), true), None);
        assert_eq!(scheduler.tick(6.5, Some(true), true), Some(Directive::Quit));
        let log = scheduler.drain_log();
        assert!(log.iter().any(|line| line == "[drive] 6.0s online"), "was: {log:?}");
    }

    #[test]
    fn wait_online_times_out_and_continues() {
        let mut scheduler = Scheduler::new(vec![Action::WaitOnline, Action::Quit]);

        assert_eq!(scheduler.tick(0.5, Some(false), true), None);
        assert_eq!(scheduler.tick(30.0, Some(false), true), None);
        // Past the 30 s deadline (armed at 0.5 s): logs an error, then continues.
        assert_eq!(scheduler.tick(30.6, Some(false), true), None);
        assert_eq!(
            scheduler.tick(31.1, Some(false), true),
            Some(Directive::Quit)
        );
        let log = scheduler.drain_log();
        assert!(
            log.iter().any(|line| line.contains("error: wait-online timed out")),
            "was: {log:?}"
        );
    }

    #[test]
    fn wait_online_skips_when_actors_are_disabled() {
        let mut scheduler = Scheduler::new(vec![Action::WaitOnline, Action::Quit]);

        assert_eq!(scheduler.tick(0.5, None, true), None);
        // The skip is noticed on this tick; the next action follows normally.
        assert_eq!(scheduler.tick(1.0, None, true), None);
        assert_eq!(scheduler.tick(1.5, None, true), Some(Directive::Quit));
        let log = scheduler.drain_log();
        assert!(
            log.iter().any(|line| line.contains("wait-online skipped")),
            "was: {log:?}"
        );
    }

    #[test]
    fn shot_blocks_until_the_file_is_saved() {
        let mut scheduler = Scheduler::new(vec![Action::Shot("proof".into()), Action::Quit]);

        assert_eq!(
            scheduler.tick(0.5, None, true),
            Some(Directive::Shot("proof".into()))
        );
        assert_eq!(scheduler.tick(1.0, None, false), None);
        assert_eq!(scheduler.tick(4.0, None, false), None);
        // Readback landed: pacing resumes from that moment.
        assert_eq!(scheduler.tick(4.2, None, true), None);
        assert_eq!(scheduler.tick(4.6, None, true), None);
        assert_eq!(scheduler.tick(4.7, None, true), Some(Directive::Quit));
    }

    #[test]
    fn script_without_quit_auto_quits_after_a_grace_period() {
        let mut scheduler = Scheduler::new(vec![Action::Key(KeyCode::Escape)]);

        assert_eq!(
            scheduler.tick(0.5, None, true),
            Some(Directive::PressKey(KeyCode::Escape))
        );
        assert_eq!(scheduler.tick(1.0, None, true), None); // arms the auto-quit
        assert_eq!(scheduler.tick(2.9, None, true), None);
        assert_eq!(scheduler.tick(3.0, None, true), Some(Directive::Quit));
        assert_eq!(scheduler.tick(3.5, None, true), None); // finished stays finished
        let log = scheduler.drain_log();
        assert!(
            log.iter().any(|line| line.contains("quit (automatic")),
            "was: {log:?}"
        );
    }
}
