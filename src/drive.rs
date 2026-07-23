//! Agent drive mode: `CATHEDRAL_DRIVE` scripts the game from the inside so
//! Claude and CI can verify changes without synthetic X11 input.
//!
//! The env var holds a `;`-separated list of actions (`key Escape`,
//! `click Continue`, `shot menu_open`, `sleep 2`, `wait-online`, `sound
//! town_bell`, `quit`).
//! Actions inject real `ButtonInput<KeyCode>` presses and `Interaction`
//! transitions, so every existing keybinding and button handler works
//! unchanged. Each fired action prints a `[drive] 3.2s key Escape` line to
//! stdout as the evidence trail (mirrored into the session's `logs.jsonl`);
//! `shot` PNGs land in the session's `screenshots/` directory. Without the
//! env var the plugin is never added and there is zero behavior change.

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonState, InputSystems};
use bevy::prelude::*;
use bevy::reflect::enums::{DynamicEnum, DynamicVariant};
use bevy::reflect::{TypeInfo, Typed};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::ui::UiSystems;
use bevy::window::PrimaryWindow;
use cathedral_sim::{StatusKind, WeatherKind};

use crate::controller::{PlayerController, TeleportPlayer};
use crate::session_log;
use crate::smart_actors::SmartActorRuntime;
use crate::smart_actors::bridge::{BridgeCommand, BridgeHandle};
use crate::smart_actors::model::Position;
use crate::soundscape::{BellPattern, SoundscapeCue};

pub const DRIVE_ENV: &str = "CATHEDRAL_DRIVE";
pub const SHOT_ENV: &str = "CATHEDRAL_SHOT";
pub const TIMEOUT_ENV: &str = "CATHEDRAL_DRIVE_TIMEOUT";

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

/// Stdout is the documented evidence trail; the session `logs.jsonl` gets the
/// same line so a parsed session is complete on its own.
fn drive_log(line: &str) {
    println!("{line}");
    session_log::log_line("drive", "INFO", line);
}

impl Plugin for DrivePlugin {
    fn build(&self, app: &mut App) {
        drive_log(&format!(
            "[drive] script has {} action(s); watchdog aborts after {}s",
            self.actions.len(),
            self.timeout.as_secs_f64()
        ));
        spawn_watchdog(self.timeout);
        app.insert_resource(DriveState {
            scheduler: Scheduler::new(self.actions.clone()),
            shot_saved: None,
            pressed_key: None,
            held_key: None,
        })
        .add_systems(
            PreUpdate,
            // After InputSystems so the injected press survives the frame's
            // `ButtonInput` clear, and after UiSystems::Focus so an injected
            // `Interaction::Pressed` is not overwritten until the focus
            // system naturally resets it (to None) on the next frame. Before
            // the chat box, which reads (and then eats) the frame's keyboard.
            run_drive_script
                .after(InputSystems)
                .after(UiSystems::Focus)
                .before(crate::smart_actors::ChatInputSet),
        );
    }
}

/// A hung run (GPU stall, engine deadlock, window that never opens) must not
/// strand a background process; systems may not be ticking at all, so the
/// watchdog lives on a plain OS thread and hard-aborts.
fn spawn_watchdog(timeout: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        let message = format!(
            "[drive] watchdog: run exceeded {}s; aborting",
            timeout.as_secs_f64()
        );
        eprintln!("{message}");
        session_log::log_line("drive", "ERROR", &message);
        std::process::exit(124);
    });
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Key(KeyCode),
    /// Hold a key down for a duration (e.g. `hold KeyW 20` walks forward for
    /// 20 s). The scheduler waits out the hold like a `sleep`, so the next
    /// action fires after release.
    Hold { key: KeyCode, seconds: f64 },
    /// Inject text as a raw `KeyboardInput` message, the stream the chat box
    /// reads. `;` cannot appear in the text — it separates script actions.
    Type(String),
    Click(String),
    Shot(String),
    Sleep(f64),
    WaitOnline,
    /// Emit a catalog world sound at the player's position. The honest
    /// stand-in for world causes the sim does not model yet — nothing rings
    /// the town bell (no clock, no calendar), so drive scripts do.
    Sound(String),
    /// Ring one of the two civic bells from its own tower. The stand-in for
    /// the funeral and proclamation transactions the sim does not model yet:
    /// the Scold's curfew has a real clock trigger, but its summons and Maren
    /// Smallvoice's name-knell wait on events nothing yet raises.
    Bell(BellPattern),
    /// Set a carriage body status (`features/npc_bodies.md` §8) on the named
    /// character, so a drunk/weary walk can be eyeballed. The stand-in for the
    /// ale the sim does not model yet — like `Sound`, a developer poke.
    Status {
        name: String,
        kind: StatusKind,
        value: f64,
    },
    /// Force the sim-owned weather authority, or clear back to its timeline.
    Weather {
        kind: Option<WeatherKind>,
        intensity: Option<f64>,
    },
    /// Teleport the player to a world position and aim the view. Yaw 0 looks
    /// toward -Z, positive pitch looks up; flight engages so the pose holds.
    Tp {
        position: Vec3,
        yaw_degrees: f32,
        pitch_degrees: f32,
    },
    Quit,
}

impl Action {
    fn describe(&self) -> String {
        match self {
            Self::Key(key) => format!("key {key:?}"),
            Self::Hold { key, seconds } => format!("hold {key:?} {seconds}"),
            Self::Type(text) => format!("type {text}"),
            Self::Click(name) => format!("click {name}"),
            Self::Shot(name) => format!("shot {name}"),
            Self::Sleep(seconds) => format!("sleep {seconds}"),
            Self::WaitOnline => "wait-online".into(),
            Self::Sound(sound_id) => format!("sound {sound_id}"),
            Self::Bell(BellPattern::ScoldCurfew) => "bell curfew".into(),
            Self::Bell(BellPattern::ScoldSummons) => "bell summons".into(),
            Self::Bell(BellPattern::NameKnell { years }) => format!("bell knell {years}"),
            Self::Status { name, kind, value } => {
                format!("status {name} {}:{value}", kind.as_str())
            }
            Self::Weather { kind: None, .. } => "weather timeline".into(),
            Self::Weather {
                kind: Some(kind),
                intensity,
            } => intensity.map_or_else(
                || format!("weather {kind}"),
                |intensity| format!("weather {kind} {intensity}"),
            ),
            Self::Tp {
                position,
                yaw_degrees,
                pitch_degrees,
            } => format!(
                "tp {} {} {} {yaw_degrees} {pitch_degrees}",
                position.x, position.y, position.z
            ),
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
        "hold" => {
            let (key_name, duration) = argument
                .split_once(char::is_whitespace)
                .ok_or_else(|| format!("`hold` needs a key and seconds in `{statement}`"))?;
            let key = keycode_from_name(key_name.trim())
                .ok_or_else(|| format!("unknown key code `{key_name}` in `{statement}`"))?;
            match duration.trim().parse::<f64>() {
                Ok(seconds) if seconds.is_finite() && seconds > 0.0 => {
                    Ok(Action::Hold { key, seconds })
                }
                _ => Err(format!("bad hold duration `{duration}` in `{statement}`")),
            }
        }
        "click" if !argument.is_empty() => Ok(Action::Click(argument.into())),
        "click" => Err("`click` needs a name substring, e.g. `click Continue`".into()),
        "type" if !argument.is_empty() => Ok(Action::Type(argument.into())),
        "type" => Err("`type` needs text, e.g. `type Hello there`".into()),
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
        "sound" => {
            if !argument.is_empty()
                && argument
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            {
                Ok(Action::Sound(argument.into()))
            } else {
                Err(format!(
                    "`sound` needs a catalog sound id like `town_bell`, got `{statement}`"
                ))
            }
        }
        "bell" => parse_bell(argument, statement),
        "status" => parse_status(argument, statement),
        "weather" => parse_weather(argument, statement),
        "tp" => {
            let numbers = argument
                .split_whitespace()
                .map(|token| token.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("bad number in `{statement}`: {error}"))?;
            match numbers[..] {
                [x, y, z] => Ok(Action::Tp {
                    position: Vec3::new(x, y, z),
                    yaw_degrees: 0.0,
                    pitch_degrees: 0.0,
                }),
                [x, y, z, yaw] => Ok(Action::Tp {
                    position: Vec3::new(x, y, z),
                    yaw_degrees: yaw,
                    pitch_degrees: 0.0,
                }),
                [x, y, z, yaw, pitch] => Ok(Action::Tp {
                    position: Vec3::new(x, y, z),
                    yaw_degrees: yaw,
                    pitch_degrees: pitch,
                }),
                _ => Err(format!(
                    "`tp` needs `x y z [yaw_deg [pitch_deg]]`, got `{statement}`"
                )),
            }
        }
        "wait-online" if argument.is_empty() => Ok(Action::WaitOnline),
        "quit" if argument.is_empty() => Ok(Action::Quit),
        "wait-online" | "quit" => Err(format!("`{verb}` takes no argument, got `{statement}`")),
        _ => Err(format!("unknown action `{verb}` in `{statement}`")),
    }
}

/// `bell curfew` / `bell summons` / `bell knell <years>`. The knell's count is
/// the dead's age, so it is required and validated here: an unparseable or
/// absurd count must fail before the run rather than ring a lie.
fn parse_bell(argument: &str, statement: &str) -> Result<Action, String> {
    let mut tokens = argument.split_whitespace();
    let pattern = match (tokens.next(), tokens.next()) {
        (Some("curfew"), None) => BellPattern::ScoldCurfew,
        (Some("summons"), None) => BellPattern::ScoldSummons,
        (Some("knell"), Some(years)) => match years.parse::<u16>() {
            Ok(years) if (1..=BellPattern::MAX_KNELL_YEARS).contains(&years) => {
                BellPattern::NameKnell { years }
            }
            _ => {
                return Err(format!(
                    "`bell knell` needs an age in 1..={} in `{statement}`",
                    BellPattern::MAX_KNELL_YEARS
                ));
            }
        },
        _ => {
            return Err(format!(
                "`bell` needs `curfew`, `summons`, or `knell <years>`, got `{statement}`"
            ));
        }
    };
    if tokens.next().is_some() {
        return Err(format!("too many arguments in `{statement}`"));
    }
    Ok(Action::Bell(pattern))
}

/// `status <name-or-id> <kind> <value>` (`features/npc_bodies.md` §8). The
/// handle may be a display name (spaces allowed — everything before the last
/// two tokens) or the actor id the HUD shows for strangers (`id p006v`); the
/// sim resolves name first, then id. The kind is a `StatusKind` wire word
/// (`drunkenness`, `weariness`) and the value a `0..=1` float. Validated here
/// so a malformed script fails before the run; a handle that matches nobody is
/// logged by the engine (not caught here — the target list lives in the sim).
fn parse_status(argument: &str, statement: &str) -> Result<Action, String> {
    let tokens: Vec<&str> = argument.split_whitespace().collect();
    if tokens.len() < 3 {
        return Err(format!(
            "`status` needs `<name> <kind> <value>`, e.g. `status Ilse drunkenness 0.8`, got `{statement}`"
        ));
    }
    let value = tokens[tokens.len() - 1];
    let kind_word = tokens[tokens.len() - 2];
    let name = tokens[..tokens.len() - 2].join(" ");
    let kind = StatusKind::from_wire(kind_word).ok_or_else(|| {
        format!("unknown status kind `{kind_word}` in `{statement}` (try drunkenness, weariness)")
    })?;
    let value = match value.parse::<f64>() {
        Ok(value) if value.is_finite() && (0.0..=1.0).contains(&value) => value,
        _ => {
            return Err(format!(
                "status value `{value}` must be a number in 0..=1 in `{statement}`"
            ));
        }
    };
    Ok(Action::Status { name, kind, value })
}

fn parse_weather(argument: &str, statement: &str) -> Result<Action, String> {
    let mut tokens = argument.split_whitespace();
    let Some(kind_name) = tokens.next() else {
        return Err(format!(
            "`weather` needs a kind or `timeline`, e.g. `weather rain 0.5`, got `{statement}`"
        ));
    };
    if kind_name.eq_ignore_ascii_case("timeline") {
        if tokens.next().is_some() {
            return Err(format!(
                "`weather timeline` takes no intensity in `{statement}`"
            ));
        }
        return Ok(Action::Weather {
            kind: None,
            intensity: None,
        });
    }
    let kind = WeatherKind::from_config_name(kind_name).ok_or_else(|| {
        format!(
            "unknown weather kind `{kind_name}` in `{statement}` (try clear, broken, overcast, fog, drizzle, rain, downpour, storm)"
        )
    })?;
    let intensity = match tokens.next() {
        None => None,
        Some(raw) => match raw.parse::<f64>() {
            Ok(value) if value.is_finite() && (0.0..=1.0).contains(&value) => Some(value),
            _ => {
                return Err(format!(
                    "weather intensity `{raw}` must be in 0..=1 in `{statement}`"
                ));
            }
        },
    };
    if tokens.next().is_some() {
        return Err(format!("too many arguments in `{statement}`"));
    }
    Ok(Action::Weather {
        kind: Some(kind),
        intensity,
    })
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
    Hold { key: KeyCode, until: f64 },
    Type(String),
    Click(String),
    Shot(String),
    Sound(String),
    Bell(BellPattern),
    Status {
        name: String,
        kind: StatusKind,
        value: f64,
    },
    Weather {
        kind: Option<WeatherKind>,
        intensity: Option<f64>,
    },
    Tp {
        position: Vec3,
        yaw_degrees: f32,
        pitch_degrees: f32,
    },
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
            Action::Hold { key, seconds } => {
                self.next_at = now + seconds;
                Some(Directive::Hold {
                    key,
                    until: now + seconds,
                })
            }
            Action::Type(text) => Some(Directive::Type(text)),
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
            Action::Sound(sound_id) => Some(Directive::Sound(sound_id)),
            Action::Bell(pattern) => Some(Directive::Bell(pattern)),
            Action::Status { name, kind, value } => Some(Directive::Status { name, kind, value }),
            Action::Weather { kind, intensity } => Some(Directive::Weather { kind, intensity }),
            Action::Tp {
                position,
                yaw_degrees,
                pitch_degrees,
            } => Some(Directive::Tp {
                position,
                yaw_degrees,
                pitch_degrees,
            }),
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
    /// A `hold` in progress: no device backs the key, so `ButtonInput` keeps
    /// it pressed on its own until this releases it at the deadline.
    held_key: Option<(KeyCode, f64)>,
}

#[allow(clippy::too_many_arguments)]
fn run_drive_script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut state: ResMut<DriveState>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut keyboard_events: MessageWriter<KeyboardInput>,
    windows: Query<Entity, With<PrimaryWindow>>,
    runtime: Option<Res<SmartActorRuntime>>,
    bridge: Option<Res<BridgeHandle>>,
    players: Query<&GlobalTransform, With<PlayerController>>,
    mut interactions: Query<(&Name, &mut Interaction)>,
    mut teleports: MessageWriter<TeleportPlayer>,
    mut cues: MessageWriter<SoundscapeCue>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(key) = state.pressed_key.take() {
        keys.release(key);
    }

    let now = time.elapsed_secs_f64();
    if let Some((key, until)) = state.held_key {
        if now >= until {
            keys.release(key);
            state.held_key = None;
        } else {
            // Re-assert every frame: a window focus loss makes Bevy
            // release_all() the keyboard, which would silently end the hold
            // while the evidence log still claims a full walk. `press` only
            // sets `just_pressed` on the released→pressed transition, so
            // re-asserting an already-held key is free.
            keys.press(key);
        }
    }
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
        drive_log(&line);
    }

    match directive {
        None => {}
        Some(Directive::PressKey(key)) => {
            keys.press(key);
            state.pressed_key = Some(key);
        }
        Some(Directive::Hold { key, until }) => {
            keys.press(key);
            state.held_key = Some((key, until));
        }
        Some(Directive::Type(text)) => match windows.single() {
            // One raw keyboard message carrying the whole string: the chat box
            // inserts `text` verbatim, exactly as a real keypress would arrive.
            // F35 exists on no keybinding, so the ButtonInput echo Bevy's input
            // system produces from this message next frame stays inert.
            Ok(window) => {
                for state in [ButtonState::Pressed, ButtonState::Released] {
                    keyboard_events.write(KeyboardInput {
                        key_code: KeyCode::F35,
                        logical_key: Key::Character(text.as_str().into()),
                        state,
                        text: (state == ButtonState::Pressed).then(|| text.as_str().into()),
                        repeat: false,
                        window,
                    });
                }
            }
            Err(_) => drive_log(&format!(
                "[drive] {now:.1}s warning: no primary window; `type` skipped"
            )),
        },
        Some(Directive::Click(name)) => {
            let needle = name.to_lowercase();
            let mut target = interactions
                .iter_mut()
                .find(|(entity_name, _)| entity_name.as_str().to_lowercase().contains(&needle));
            match target.as_mut() {
                // The UI focus system resets this to `None` next frame, which
                // produces the `Changed<Interaction>` press the handlers expect.
                Some((_, interaction)) => **interaction = Interaction::Pressed,
                None => drive_log(&format!(
                    "[drive] {now:.1}s warning: no UI entity named like `{name}`"
                )),
            }
        }
        Some(Directive::Shot(name)) => match session_log::paths() {
            None => drive_log(&format!(
                "[drive] {now:.1}s warning: no session directory; screenshot `{name}` skipped"
            )),
            Some(session) => {
                if let Err(error) = fs::create_dir_all(&session.screenshots) {
                    drive_log(&format!(
                        "[drive] {now:.1}s warning: could not create {}: {error}",
                        session.screenshots.display()
                    ));
                }
                let path = session.screenshots.join(format!("{name}.png"));
                let saved = Arc::new(AtomicBool::new(false));
                state.shot_saved = Some(saved.clone());
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path))
                    .observe(move |_: On<ScreenshotCaptured>| saved.store(true, Ordering::Release));
            }
        },
        Some(Directive::Sound(sound_id)) => {
            // Emitted at the player's position: the drive trigger stands in
            // for world causes the sim lacks, not for a modeled bell tower.
            let position = players
                .single()
                .ok()
                .and_then(|player| Position::try_from(player.translation()).ok());
            match (bridge.as_deref(), position) {
                (Some(bridge), Some(position_m)) => {
                    if let Err(error) = bridge.try_send(BridgeCommand::DebugSound {
                        sound_id,
                        position_m,
                    }) {
                        drive_log(&format!(
                            "[drive] {now:.1}s warning: sound not sent: {error}"
                        ));
                    }
                }
                _ => drive_log(&format!(
                    "[drive] {now:.1}s warning: `sound` needs smart actors and a player"
                )),
            }
        }
        Some(Directive::Bell(pattern)) => {
            // A civic bell is render-side texture with a diegetic meaning, not
            // a catalog event: it goes straight to the soundscape's own cue
            // stream and never reaches an actor inbox.
            cues.write(SoundscapeCue::CivicBell(pattern));
        }
        Some(Directive::Status { name, kind, value }) => match bridge.as_deref() {
            // Travels the same host→engine path as `sound`: a bridge command the
            // engine applies to the sim (`EngineCommand::DebugSetStatus`).
            Some(bridge) => {
                if let Err(error) = bridge.try_send(BridgeCommand::DebugStatus {
                    name: name.clone(),
                    kind,
                    value,
                }) {
                    drive_log(&format!(
                        "[drive] {now:.1}s warning: status not sent: {error}"
                    ));
                }
            }
            None => drive_log(&format!(
                "[drive] {now:.1}s warning: `status` needs smart actors"
            )),
        },
        Some(Directive::Weather { kind, intensity }) => match bridge.as_deref() {
            Some(bridge) => {
                let command = kind.map_or(BridgeCommand::ClearWeatherOverride, |kind| {
                    BridgeCommand::SetWeatherOverride { kind, intensity }
                });
                if let Err(error) = bridge.try_send(command) {
                    drive_log(&format!(
                        "[drive] {now:.1}s warning: weather override not sent: {error}"
                    ));
                }
            }
            None => drive_log(&format!(
                "[drive] {now:.1}s warning: `weather` needs the actor engine"
            )),
        },
        Some(Directive::Tp {
            position,
            yaw_degrees,
            pitch_degrees,
        }) => {
            teleports.write(TeleportPlayer {
                position,
                yaw_degrees,
                pitch_degrees,
                // The drive `tp` holds an elevated vantage for screenshots.
                fly: true,
            });
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
    fn type_keeps_inner_spaces_and_requires_text() {
        assert_eq!(
            parse_script("key Enter; type Hello there, Ilse!; key Enter"),
            Ok(vec![
                Action::Key(KeyCode::Enter),
                Action::Type("Hello there, Ilse!".into()),
                Action::Key(KeyCode::Enter),
            ])
        );
        assert!(parse_script("type").is_err());
    }

    #[test]
    fn tp_parses_position_and_optional_view() {
        assert_eq!(
            parse_script("tp 10 2 -30"),
            Ok(vec![Action::Tp {
                position: Vec3::new(10.0, 2.0, -30.0),
                yaw_degrees: 0.0,
                pitch_degrees: 0.0,
            }])
        );
        assert_eq!(
            parse_script("tp 64 28 -270 90 -15.5"),
            Ok(vec![Action::Tp {
                position: Vec3::new(64.0, 28.0, -270.0),
                yaw_degrees: 90.0,
                pitch_degrees: -15.5,
            }])
        );
        assert!(parse_script("tp 1 2").is_err());
        assert!(parse_script("tp 1 2 3 4 5 6").is_err());
        assert!(parse_script("tp here").is_err());
    }

    #[test]
    fn bell_parses_both_ropes_and_validates_the_count() {
        assert_eq!(
            parse_script("bell curfew; bell summons; bell knell 17"),
            Ok(vec![
                Action::Bell(BellPattern::ScoldCurfew),
                Action::Bell(BellPattern::ScoldSummons),
                Action::Bell(BellPattern::NameKnell { years: 17 }),
            ])
        );
        assert_eq!(
            parse_script("bell knell 17").map(|actions| actions[0].describe()),
            Ok("bell knell 17".into())
        );
        // The knell's count is a life; it may not be missing, zero or absurd.
        assert!(parse_script("bell knell").is_err());
        assert!(parse_script("bell knell 0").is_err());
        assert!(parse_script("bell knell 900").is_err());
        assert!(parse_script("bell knell twelve").is_err());
        assert!(parse_script("bell curfew 3").is_err());
        assert!(parse_script("bell gravemouth").is_err());
        assert!(parse_script("bell").is_err());
    }

    #[test]
    fn empty_statements_are_skipped() {
        assert_eq!(parse_script(""), Ok(vec![]));
        assert_eq!(parse_script(" ; ;; quit "), Ok(vec![Action::Quit]));
    }

    #[test]
    fn status_parses_name_kind_and_bounded_value() {
        assert_eq!(
            parse_script("status Ilse drunkenness 0.8"),
            Ok(vec![Action::Status {
                name: "Ilse".into(),
                kind: StatusKind::Drunkenness,
                value: 0.8,
            }])
        );
        // A name may carry spaces; the last two tokens are kind and value.
        assert_eq!(
            parse_script("status Old Nan weariness 1"),
            Ok(vec![Action::Status {
                name: "Old Nan".into(),
                kind: StatusKind::Weariness,
                value: 1.0,
            }])
        );
        assert!(
            parse_script("status Ilse sobriety 0.5").is_err(),
            "unknown kind"
        );
        assert!(
            parse_script("status Ilse drunkenness 2").is_err(),
            "out of range"
        );
        assert!(
            parse_script("status Ilse drunkenness").is_err(),
            "missing value"
        );
    }

    #[test]
    fn weather_parses_forced_kinds_intensity_and_timeline() {
        assert_eq!(
            parse_script("weather rain 0.5; weather storm; weather timeline"),
            Ok(vec![
                Action::Weather {
                    kind: Some(WeatherKind::Rain),
                    intensity: Some(0.5),
                },
                Action::Weather {
                    kind: Some(WeatherKind::Thunderstorm),
                    intensity: None,
                },
                Action::Weather {
                    kind: None,
                    intensity: None,
                },
            ])
        );
        assert!(parse_script("weather sleet").is_err());
        assert!(parse_script("weather rain 2").is_err());
        assert!(parse_script("weather timeline 0.5").is_err());
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
    fn screenshot_names_must_stay_inside_the_session_directory() {
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
        assert!(
            log.iter().any(|line| line == "[drive] 6.0s online"),
            "was: {log:?}"
        );
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
            log.iter()
                .any(|line| line.contains("error: wait-online timed out")),
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
