//! `cathedral-headless` — the terminal tick loop (`main.py`), plus `kimi.py`.
//!
//! Same contract as the Python prototype: stdout carries the transcript, the
//! final world state and the run cost; stderr carries diagnostics (and, with
//! `-v`, the full prompts and raw replies). `make sim`-style piping keeps
//! working.
//!
//! Two deliberate divergences from `main.py`, both decided in ARCHITECTURE:
//!
//! * **D5 — scheduler semantics, not the prototype's turn loop.** The turns run
//!   through the real [`Engine`]: `wait` never reaches the transcript, presented
//!   percepts are absorbed only on success, a provider failure backs off and
//!   retries instead of crashing the process, and an NPC's speech holds the
//!   conversation floor. `main.py` did none of that; it had its own, simpler
//!   loop that drifted from the sidecar's.
//! * **A virtual clock.** The engine is clock-free, so `now` is a plain `f64`
//!   this loop advances itself: the inter-turn delay and the floor's reading
//!   pauses cost sim seconds, not wall-clock seconds. A six-tick offline run is
//!   instant.
//!
//! The clock is also what separates a turn's *submission* from its
//! *application*: applying a reply sets `next_turn_at = now + turn_delay`, so
//! with a non-zero delay the same poll cannot also submit the next turn. That is
//! what lets the loop below print `== tick n: Name ==` before the lines it
//! produced, one turn at a time.

use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use cathedral_backends::{
    BackendEvent, BackendRuntime, BackendsConfig, BackendsHandle, BackendsOptions, Environment,
    HttpCognition, LlmClient, LlmError, UsageLedger,
    config::{DEFAULT_DOTENV_PATH, DEFAULT_WORKERS_DIR},
    world_data::load_world_seed_with_knowledge,
};
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Completion, CuriosityConfig,
    DEFAULT_VIEW_CONE_DEGREES, Engine, EngineCommand, EngineConfig, EngineMessage, FakeCognition,
    IdleCognitionMode, NavData, NightOfficeConfig, NullSight, NullTranscription, NullTts, Office,
    PlayerKnowledge, PromptEnv, RequestId, ShelterMap, SoundCatalog, StageConfig, StatusKind,
    TtsBackendKind, Vec3, WeatherConfig, WeatherMode, World, WorldClock, WorldSeed,
    engine::{
        DEFAULT_MAXIMUM_BACKOFF_SECONDS, DEFAULT_NIGHT_BRIGHTNESS, DEFAULT_SECONDS_PER_DAY,
        DEFAULT_SOUND_COOLDOWN_SECONDS, DEFAULT_STT_STREAM_GRACE_SECONDS,
    },
    llm_turn_order,
};
use clap::Parser;

/// The player id every seed world must carry.
const PLAYER_ID: &str = "player";
/// Sim seconds one pump advances the virtual clock by. Small enough that the
/// floor's reading pause (3-10 s) resolves in a handful of polls, large enough
/// that no loop below spins.
const CLOCK_STEP_SECONDS: f64 = 0.5;
/// The loop separates submit-poll from apply-poll by the inter-turn delay, so
/// it must never be zero here (see the module docs). `NPC_TURN_DELAY_SECONDS=0`
/// is honored as "as fast as the sim allows", not as "collapse two turns into
/// one poll".
const MIN_TURN_DELAY_SECONDS: f64 = 0.001;
/// Sim-clock guard: a turn that neither submits nor applies within this many
/// virtual seconds is a bug in the pump, not a slow provider.
const MAX_SIM_SECONDS_PER_PHASE: f64 = 600.0;
/// Wall-clock guard on a real provider call (the per-attempt timeout is 45 s and
/// there is one retry, so this only fires if the backend never answers at all).
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Parser)]
#[command(
    name = "cathedral-headless",
    about = "Tick-loop demo: characters take turns acting on a town square."
)]
struct Args {
    /// number of turns to run (default 6)
    #[arg(short = 't', long, default_value_t = 6)]
    ticks: u32,

    /// dump prompts and raw replies to stderr
    #[arg(short = 'v', long)]
    verbose: bool,

    /// offline scripted cognition: no provider, no network, no API key
    #[arg(long)]
    fake: bool,

    /// developer mode: seed the player with all ambient names too
    #[arg(long)]
    know_everybody: bool,

    /// add N generated ambient citizens spread over the walkable city (0..=20000)
    ///
    /// The terminal twin of `config.ron: smart_actors.extra_ambient_npcs` — the
    /// way to see what a crowd costs the *simulation* (enrolment, the round,
    /// perception, the snapshot) with no renderer in the way. Needs a nav
    /// graph under `--assets`; without one there is nowhere walkable to stand
    /// anybody and the count is refused.
    #[arg(long, default_value_t = 0, value_name = "N")]
    extra_ambient: u32,

    /// gate idle turns on the player's neighborhood, as the game does
    ///
    /// Off by default: the terminal's player never moves, so an ungated
    /// rotation is what makes a six-tick run visit six different people. Turn it
    /// on when the gate itself is what you are testing.
    #[arg(long)]
    stage: bool,

    /// also require news for an idle turn, as the game does (implies --stage)
    ///
    /// This is the cost gate: with it, a tick spent on a cast with nothing to
    /// react to buys no prompt at all. A run that ends early with fewer turns
    /// than you asked for is the feature working — nobody had anything to say.
    #[arg(long)]
    news: bool,

    /// also let character decide who speaks first, as the game does (implies --news)
    ///
    /// Roughly four in five people you walk past keep their thoughts to
    /// themselves; the beggars and the hawkers do not. It gates *initiative*
    /// only — a `say` addressed to an aloof NPC is answered exactly as before.
    #[arg(long)]
    curiosity: bool,

    /// LLM provider override (moonshot | openai); beats LLM_PROVIDER
    #[arg(long)]
    provider: Option<String>,

    /// model override; beats LLM_MODEL
    #[arg(long)]
    model: Option<String>,

    /// send FILE as the whole prompt and print the reply (the old `kimi.py`)
    #[arg(long, value_name = "FILE")]
    one_shot: Option<PathBuf>,

    /// broadcast one player utterance from the spawn before the run starts
    ///
    /// The terminal has no microphone, so this is the way to poke the cast into
    /// reacting — pairs with `--fake`, whose scripted cast waves back when asked
    /// (`--say "please wave at me"`), so the gesture path prints its transcript
    /// line (`Conny waves at You`) in an offline run.
    #[arg(long, value_name = "TEXT")]
    say: Option<String>,

    /// set a body carriage status on a character after the world loads,
    /// repeatable: `--status Ilse=drunkenness:0.8` (`features/npc_bodies.md` §8)
    ///
    /// The stand-in for the ale the sim does not model yet — nothing makes
    /// anyone drunk — so a drunk or weary carriage can be poked in for a
    /// transcript or a `--trace-positions` walk. NAME resolves by display name
    /// first, then by actor id (`--status p006v=weariness:1`). Kinds:
    /// drunkenness, weariness, urgency; value is a 0..=1 float. (The sim writes
    /// `urgency` itself on the poop clock, `features/extra_pockets.md` M3 — this
    /// only forces it.) A handle matching nobody is a stderr diagnostic, not a
    /// fault. Sits beside `--say` as a pre-run poke.
    #[arg(long, value_name = "NAME=KIND:VALUE")]
    status: Vec<String>,

    /// real seconds per game day (24× default is one game day per real hour)
    #[arg(long, default_value_t = DEFAULT_SECONDS_PER_DAY, value_name = "SECONDS")]
    seconds_per_day: f64,

    /// which office the run opens on (watch|kindling|dayspring|high_wick|waning|lamplight|snuffing)
    #[arg(long, default_value = "dayspring", value_name = "OFFICE")]
    start_office: String,

    /// which day the run opens on; day 0 is a Bellday, 2 a Highmarket
    #[arg(long, default_value_t = 0, value_name = "DAY")]
    start_day: i64,

    /// force a weather kind, or use `timeline` (clear|broken|overcast|fog|drizzle|rain|downpour|storm)
    #[arg(long, default_value = "timeline", value_name = "KIND")]
    weather: String,

    /// instead of taking turns, watch the clock advance this many game days,
    /// printing each office as its bell rings
    #[arg(long, default_value_t = 0.0, value_name = "DAYS")]
    watch_clock: f64,

    /// directory holding prompts/, sounds/ and world/
    #[arg(long, default_value = "assets", value_name = "DIR")]
    assets: PathBuf,

    /// directory holding characters/ and core_lore/; defaults beside assets/
    #[arg(long, value_name = "DIR")]
    lore: Option<PathBuf>,

    /// print each mover's position on the transcript stream as it walks
    ///
    /// One `[pos]` line per moved actor per poll, so you can watch the daily
    /// round walk the cast about their errands. Needs a nav graph under
    /// `--assets`; without one nobody moves and nothing prints.
    #[arg(long)]
    trace_positions: bool,

    /// watch the M3 water round: a `[water]` census each tick, and a `[water]`
    /// line per draw as a keeper works a curb. Pairs well with `--watch-clock`
    /// (turn-free) or a long `--fake -t` run. Needs a nav graph under `--assets`.
    #[arg(long)]
    trace_water: bool,

    /// watch the M4 daily round: a `[census]` line each time an office rings,
    /// counting who is home, at their post, walking, or left in the street, and
    /// which posts are populated. Read a full game day with
    /// `--fake --seconds-per-day 120 -t 130 --census-by-area` (or `--watch-clock 1`).
    #[arg(long)]
    census_by_area: bool,

    /// watch the food & items M2 hunger census: a `[food]` line counting who is
    /// fed, hungry or famished, the mean gauge, and the coin held. Pairs with
    /// `--watch-clock 1` to see hunger climb through the morning and collapse at
    /// High Wick (the hearth). Needs a nav graph under `--assets`.
    #[arg(long)]
    trace_food: bool,

    /// run the Night Office: the second cognition lane (movement M6)
    ///
    /// Majors reflect individually at their own bedtimes, the Minors are
    /// batched one prompt per ward at the curfew, and the ambient cast's
    /// evenings are re-rolled in code for nothing. Roughly 39 provider calls a
    /// game day, so pair it with `--fake` unless you mean to spend them. The
    /// `[night]` lines ride stderr with the rest of the diagnostics; `--watch-clock 1`
    /// is the turn-free way to see a whole night pass.
    #[arg(long)]
    night_office: bool,

    /// mark somebody as owing, so the ward chalks their door, repeatable:
    /// `--owe "Ede Clove"` (`features/implemented/chalking_the_walls.md` M1)
    ///
    /// A cross is chalked off an aged, unsettled restitution notice, and
    /// raising one is an LLM's judgement — so an offline run cannot otherwise
    /// reach the door, the counter or the refusal. This raises the same notice
    /// `raise_notice` does, back-dated past the age gate; everything after it
    /// is the real code. NAME resolves by display name first, then by actor
    /// id. A handle matching nobody is a stderr diagnostic, not a fault.
    #[arg(long, value_name = "NAME")]
    owe: Vec<String>,

    /// Multiply the chalk decay (`features/implemented/chalking_the_walls.md`). A cross
    /// weathers over nine game days at `1.0`, which no `-t 12` run will ever
    /// see; `--marks-decay-scale 200` washes one off inside a short run so the
    /// re-chalk and the wash-off are both observable in one transcript.
    #[arg(long, default_value_t = 1.0)]
    marks_decay_scale: f64,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let mut environment = Environment::from_process(Some(Path::new(DEFAULT_DOTENV_PATH)));
    // A flag outranks the environment; absent, the environment outranks the
    // provider default (llm-headless.md §3.3).
    if let Some(provider) = &args.provider {
        environment.set("LLM_PROVIDER", provider.clone());
    }
    if let Some(model) = &args.model {
        environment.set("LLM_MODEL", model.clone());
    }
    let options = BackendsOptions {
        dotenv_path: Some(PathBuf::from(DEFAULT_DOTENV_PATH)),
        workers_dir: PathBuf::from(DEFAULT_WORKERS_DIR),
        uv_binary: "uv".to_string(),
        fake_mode: args.fake,
    };
    let config = BackendsConfig::resolve(&environment, &options);

    if let Some(path) = &args.one_shot {
        return one_shot(path, &config);
    }
    match run(&args, config) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// The catalog ids the M3 water round emits, so `--trace-water` can pick a draw
/// out of the general sound stream.
fn is_water_sound(sound_id: &str) -> bool {
    matches!(
        sound_id,
        "draw_water" | "chain_windlass" | "pour_trough" | "pail_clatter"
    )
}

/// Parse one `--status NAME=KIND:VALUE` spec (`features/npc_bodies.md` §8). The
/// name may contain spaces or `:` (only the last `:` splits kind from value);
/// the kind is a `StatusKind` wire word, the value a `0..=1` float.
fn parse_status_flag(spec: &str) -> Result<(String, StatusKind, f64), String> {
    let (name, rest) = spec.split_once('=').ok_or_else(|| {
        format!("--status `{spec}` must be NAME=KIND:VALUE, e.g. Ilse=drunkenness:0.8")
    })?;
    let (kind_word, value_word) = rest.rsplit_once(':').ok_or_else(|| {
        format!("--status `{spec}` must be NAME=KIND:VALUE, e.g. Ilse=drunkenness:0.8")
    })?;
    if name.is_empty() {
        return Err(format!("--status `{spec}` names nobody"));
    }
    let kind = StatusKind::from_wire(kind_word).ok_or_else(|| {
        format!(
            "--status `{spec}`: unknown kind `{kind_word}` (try drunkenness, weariness, urgency)"
        )
    })?;
    let value = match value_word.parse::<f64>() {
        Ok(value) if value.is_finite() && (0.0..=1.0).contains(&value) => value,
        _ => {
            return Err(format!(
                "--status `{spec}`: value must be a number in 0..=1"
            ));
        }
    };
    Ok((name.to_string(), kind, value))
}

// --------------------------------------------------------------- one-shot mode

/// `kimi.py`: one file in, one raw reply out (llm-headless.md §4).
fn one_shot(path: &Path, config: &BackendsConfig) -> ExitCode {
    let prompt = match fs::read_to_string(path) {
        Ok(prompt) => prompt,
        Err(error) => {
            eprintln!("error: cannot read {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };

    // Python raised the configuration error *inside* `complete()`, so a missing
    // key reports as a failed request — same wording, same exit code.
    let client = config
        .llm
        .clone()
        .map_err(LlmError::from)
        .and_then(LlmClient::new);
    let reply = client.and_then(|client| {
        let runtime = BackendRuntime::new()
            .map_err(|error| LlmError::Transport(format!("no runtime: {error}")))?;
        runtime.block_on(client.complete(prompt))
    });

    match reply {
        Ok(reply) => {
            println!("{reply}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: API request failed: {error}");
            ExitCode::FAILURE
        }
    }
}

// ------------------------------------------------------------------- tick loop

fn run(args: &Args, config: BackendsConfig) -> Result<ExitCode, String> {
    let lore = args.lore.clone().unwrap_or_else(|| {
        args.assets
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("lore")
    });
    let assets = Assets::load(&args.assets, &lore, args.know_everybody, args.extra_ambient)?;

    // Fake cognition needs no provider and no key; a real run needs both, and a
    // scheduler with nothing to call would simply never take a turn.
    let (cognition, brain): (Box<dyn Cognition>, Brain) = if args.fake {
        let fake = Shared::new(FakeCognition::new());
        (Box::new(fake.clone()), Brain::Fake(fake))
    } else {
        if let Err(error) = &config.llm {
            return Err(error.to_string());
        }
        let handle = BackendsHandle::start(config.clone(), None)
            .map_err(|error| format!("could not start the backends: {error}"))?;
        let http = Shared::new(handle.cognition());
        (
            Box::new(http.clone()),
            Brain::Http {
                handle: Box::new(handle),
                http,
            },
        )
    };

    let turn_delay_seconds = config.npc_turn_delay_seconds.max(MIN_TURN_DELAY_SECONDS);
    let (player_spawn, player_yaw) = assets.player_spawn();
    let start_office = Office::from_config_name(&args.start_office).ok_or_else(|| {
        format!(
            "unknown --start-office `{}` (try watch, kindling, dayspring, high_wick, waning, lamplight, snuffing)",
            args.start_office
        )
    })?;
    let clock = WorldClock::new(
        args.seconds_per_day,
        start_office,
        args.start_day,
        DEFAULT_NIGHT_BRIGHTNESS,
    );
    let weather_mode = WeatherMode::from_config_name(&args.weather).ok_or_else(|| {
        format!(
            "unknown --weather `{}` (try timeline, clear, broken, overcast, fog, drizzle, rain, downpour, storm)",
            args.weather
        )
    })?;
    let engine = Engine::new(
        EngineConfig {
            player_id: ActorId::from_raw(PLAYER_ID),
            fake_mode: args.fake,
            sounds_enabled: true,
            view_cone_degrees: DEFAULT_VIEW_CONE_DEGREES,
            sound_cooldown_seconds: DEFAULT_SOUND_COOLDOWN_SECONDS,
            turn_delay_seconds,
            maximum_backoff_seconds: DEFAULT_MAXIMUM_BACKOFF_SECONDS,
            // The terminal has no speakers: `Off` also means the floor paces the
            // cast by the reading estimate rather than by audio (floor.rs).
            tts_selected: TtsBackendKind::Off,
            // Silence was the plan, so there is nothing to apologize for.
            tts_startup_message: None,
            stt_stream_grace_seconds: DEFAULT_STT_STREAM_GRACE_SECONDS,
            // The terminal has no microphone either: no recording will ever be
            // named, so there is no directory to name it in.
            runtime_dir: PathBuf::new(),
            // `--news` is meaningless ungated, so it implies `--stage` rather
            // than silently doing nothing — and `--curiosity` is meaningless
            // without news for the same reason, so it implies both.
            idle_mode: match args.stage || args.news || args.curiosity {
                true => IdleCognitionMode::Stage,
                false => IdleCognitionMode::All,
            },
            stage: StageConfig::default(),
            idle_requires_news: args.news || args.curiosity,
            idle_curiosity: CuriosityConfig {
                enabled: args.curiosity,
                ..CuriosityConfig::default()
            },
            clock,
            ring_the_offices: true,
            weather: WeatherConfig {
                mode: weather_mode,
                ..WeatherConfig::default()
            },
            shelters: assets.shelters.clone(),
            // Movement and the daily round only exist when a nav graph is present.
            nav: assets.nav.clone(),
            // The second lane, off unless asked for: 39 provider calls a game
            // day is not something a `-t 6` run should buy by accident.
            night_office: NightOfficeConfig {
                enabled: args.night_office,
                ..NightOfficeConfig::default()
            },
            // The pack costs nothing here — a transcript line only when a dog
            // drifts through somebody's you_see — so the default stands.
            dogs_enabled: true,
            // The chalk costs nothing either — the walls start bare, and a
            // mark reaches a sheet only when a hand has drawn one — so the
            // default stands here too. `--marks-decay-scale` weathers a wall
            // in a short run instead of over nine game days.
            marks_enabled: true,
            mark_kinds: cathedral_sim::marks::MarkKindSwitches::default(),
            marks_decay_scale: args.marks_decay_scale,
        },
        &assets.seed,
        assets.areas,
        assets.catalog,
        assets.prompts,
        cognition,
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (player_spawn, player_yaw),
        0,
        0.0,
    )
    .map_err(|error| error.to_string())?;

    let mut runner = Runner {
        engine,
        brain,
        now: 0.0,
        verbose: args.verbose,
        printed_lines: 0,
        provider_failed: false,
        requires_news: args.news || args.curiosity,
        last_office: None,
        trace_positions: args.trace_positions,
        trace_water: args.trace_water,
        census_by_area: args.census_by_area,
        trace_food: args.trace_food,
    };
    // One player utterance before the run, so an offline cast can be poked into
    // reacting (a wave, a reply) with no microphone in the loop.
    if let Some(text) = args.say.clone() {
        runner.pump(vec![EngineCommand::PlayerSay {
            request_id: "headless-say".to_string(),
            text,
            position_m: player_spawn,
            spatial_seq: 1,
        }]);
    }
    // Body-carriage pokes before the run (npc_bodies M5): each `--status` sets a
    // status on a named character through the same command path the drive-mode
    // `status` action uses. A missing name is a `Diagnostic` on stderr, not a
    // fault, so a typo does not abort the whole run.
    for spec in &args.status {
        let (name, kind, value) = parse_status_flag(spec)?;
        runner.pump(vec![EngineCommand::DebugSetStatus { name, kind, value }]);
    }
    // The debt stand-in, through the same command path the rest of the pokes
    // use, and before the ticks so the first beat of the run finds it.
    for who in &args.owe {
        runner.pump(vec![EngineCommand::DebugOwe { who: who.clone() }]);
    }
    if args.watch_clock > 0.0 {
        runner.watch_clock(args.watch_clock, clock.seconds_per_day())?;
    } else {
        runner.run_ticks(args.ticks)?;
    }
    runner.print_final_state();
    runner.print_cost();

    Ok(match runner.provider_failed {
        // A provider that failed at least once did not produce the run that was
        // asked for, even though the scheduler retried around it.
        true => ExitCode::FAILURE,
        false => ExitCode::SUCCESS,
    })
}

/// Where the completions come from.
enum Brain {
    /// Staged synchronously at submit time; drained by the loop.
    Fake(Shared<FakeCognition>),
    /// Answered by a tokio task on the backend channel; the loop blocks on it.
    /// Boxed: the handle owns the runtime and the channel, and dwarfs the fake.
    Http {
        handle: Box<BackendsHandle>,
        http: Shared<HttpCognition>,
    },
}

struct Runner {
    engine: Engine,
    brain: Brain,
    /// The virtual clock. Only this loop moves it.
    now: f64,
    verbose: bool,
    /// How much of `engine.transcript()` has already been echoed.
    printed_lines: usize,
    provider_failed: bool,
    /// Whether a tick that buys no turn is quiescence rather than a stall
    /// (`--news`). Nothing external ever happens in a terminal run — no player
    /// walks past, no bell rings — so a cast that has run out of news has run
    /// out for good, and the run is simply over.
    requires_news: bool,
    /// The last office the clock reported, so the runner prints each bell once
    /// as it rings rather than on every poll.
    last_office: Option<Office>,
    /// `--trace-positions`: echo every mover's pose to the transcript stream.
    trace_positions: bool,
    /// `--trace-water`: echo the water round's census and every draw.
    trace_water: bool,
    /// `--census-by-area`: echo a behavioural census as each office rings.
    census_by_area: bool,
    /// `--trace-food`: echo the M2 hunger census.
    trace_food: bool,
}

impl Runner {
    fn run_ticks(&mut self, ticks: u32) -> Result<(), String> {
        for tick in 1..=ticks {
            let Some(actor_name) = self.submit_turn()? else {
                println!(
                    "\n== nobody has anything to react to; stopped after {} of {ticks} ticks ==",
                    tick - 1
                );
                break;
            };
            println!("\n== tick {tick}: {actor_name} ==");
            self.apply_turn()?;
            self.print_new_transcript_lines();
            if self.trace_food {
                for line in self.engine.drain_food_log() {
                    println!("[food] {line}");
                }
            }
            if self.trace_water {
                println!("[water] {}", self.engine.water_summary());
            }
            if self.census_by_area {
                println!("[census] {}", self.engine.round_census(self.now).summary());
            }
            if self.trace_food {
                println!("[food] {}", self.engine.food_summary());
            }
        }
        Ok(())
    }

    /// Watch the clock advance `game_days` days with no turns, so every office
    /// prints as its bell rings — the pure-clock counterpart to `run_ticks`, and
    /// the fastest way to see a whole game day pass.
    fn watch_clock(&mut self, game_days: f64, seconds_per_day: f64) -> Result<(), String> {
        let real_seconds = game_days * seconds_per_day;
        let end = self.now + real_seconds;
        // Small enough that even the closest two offices (the Kindling and
        // Dayspring, two game hours apart) never share a step, so no bell's line
        // is skipped. When tracing the water round the step must also stay under
        // the mover accumulator's catch-up budget (3.2 s), or a coarse step snaps
        // walkers forward and drops the walk — so cap it at a stride there.
        let mut step = (seconds_per_day / 200.0).max(0.05);
        // Tracing positions (water or the round census) needs the step under the
        // mover accumulator's catch-up budget (3.2 s), or a coarse step snaps
        // walkers forward and drops the walk — nobody would ever be seen to arrive.
        if self.trace_water || self.census_by_area || self.trace_food {
            step = step.min(3.0);
        }
        println!(
            "== watching {game_days} game day(s): {real_seconds:.0} s at {seconds_per_day:.0} s/day =="
        );
        // The census samples *within* each office, not at the bell — right when a
        // bell rings the whole cast has just re-routed and everyone is walking.
        let census_interval = (real_seconds / (16.0 * game_days).max(1.0)).max(step);
        let mut next_water = self.now;
        let mut next_census = self.now + census_interval;
        let mut next_food = self.now;
        while self.now < end {
            self.now += step;
            // Non-blocking, and normally empty: `--watch-clock` takes no turns.
            // But the Night Office does — its lane is driven by the bell, not by
            // the tick loop — so without this a watched night submits one
            // reflection and then waits forever for an answer nobody handed
            // back (M6).
            let commands = self.collect_completions(false)?;
            self.pump(commands);
            if self.trace_food {
                // Drain every step so the restock, the sales and the ledger print
                // in the order they happened, not clumped at the census interval.
                for line in self.engine.drain_food_log() {
                    println!("[food] {line}");
                }
            }
            if self.trace_water && self.now >= next_water {
                println!("[water] {}", self.engine.water_summary());
                next_water = self.now + 3.0;
            }
            if self.census_by_area && self.now >= next_census {
                println!("[census] {}", self.engine.round_census(self.now).summary());
                next_census = self.now + census_interval;
            }
            if self.trace_food && self.now >= next_food {
                println!("[food] {}", self.engine.food_summary());
                next_food = self.now + census_interval;
            }
        }
        Ok(())
    }

    /// Pump until the scheduler has a turn in flight, advancing the clock past
    /// the inter-turn delay, a provider backoff, or a floor pause as needed.
    /// Returns the name of the actor whose turn it is, or `None` when the cast
    /// has nothing to react to and `--news` says that is an ending rather than a
    /// fault.
    fn submit_turn(&mut self) -> Result<Option<String>, String> {
        let deadline = self.now + MAX_SIM_SECONDS_PER_PHASE;
        while self.engine.scheduler().in_flight_actor_id().is_none() {
            let commands = self.collect_completions(false)?;
            self.pump(commands);
            if self.engine.scheduler().in_flight_actor_id().is_some() {
                break;
            }
            if self.now > deadline {
                return match self.requires_news {
                    true => Ok(None),
                    false => Err("the scheduler never started a turn".to_string()),
                };
            }
            self.now += CLOCK_STEP_SECONDS;
        }
        let actor_id = self
            .engine
            .scheduler()
            .in_flight_actor_id()
            .expect("the loop only exits with a turn in flight")
            .clone();
        Ok(Some(
            self.engine
                .world()
                .characters
                .get(&actor_id)
                .map(|actor| actor.name().to_string())
                .unwrap_or_else(|| actor_id.to_string()),
        ))
    }

    /// Feed the completion back and pump until the turn has been applied — which
    /// the conversation floor may hold off until the previous line has been
    /// "read" (server-core.md §8).
    fn apply_turn(&mut self) -> Result<(), String> {
        let deadline = self.now + MAX_SIM_SECONDS_PER_PHASE;
        loop {
            // Wait on the provider only while it still owes us an answer. Once
            // the completion has arrived and the floor has *parked* it, the turn
            // is still "in flight" but the backend channel is empty and will
            // stay empty — no speech backends are wired up here — so blocking
            // again would burn the whole `COMPLETION_TIMEOUT` and then abort a
            // perfectly healthy run with "the provider never answered". The
            // held turn is released by the clock below, not by the channel.
            let blocking = !self.engine.scheduler().has_held_result();
            let commands = self.collect_completions(blocking)?;
            self.pump(commands);
            if self.engine.scheduler().in_flight_actor_id().is_none() {
                debug_assert!(
                    !self.engine.scheduler().has_held_result(),
                    "a held result keeps the turn in flight"
                );
                return Ok(());
            }
            if self.now > deadline {
                return Err("a turn never finished".to_string());
            }
            self.now += CLOCK_STEP_SECONDS;
        }
    }

    fn pump(&mut self, commands: Vec<EngineCommand>) {
        for message in self.engine.poll(self.now, commands) {
            self.report(message);
        }
    }

    /// Everything a turn wants to say goes to stderr; only the transcript, the
    /// final state and the cost belong on stdout (llm-headless.md §3.3).
    fn report(&mut self, message: EngineMessage) {
        match message {
            EngineMessage::Diagnostic(line) => eprintln!("{line}"),
            EngineMessage::PromptExchange {
                actor_name,
                prompt,
                answer,
                error,
                ..
            } => {
                if error.is_some() {
                    self.provider_failed = true;
                }
                if self.verbose {
                    // `main.py` printed the prompt before the call and the reply
                    // after it; the engine archives both together, once the turn
                    // is over, so they print as one block.
                    eprintln!("--- prompt for {actor_name} ---\n{prompt}");
                    if let Some(answer) = answer {
                        eprintln!("--- reply from {actor_name} ---\n{answer}");
                    }
                }
            }
            // One line per bell, not per poll: the clock is republished every
            // poll, but the office only occasionally changes.
            EngineMessage::Clock {
                day,
                day_fraction,
                office,
                weekday,
                ..
            } if self.last_office != Some(office) => {
                let first = self.last_office.is_none();
                self.last_office = Some(office);
                let minutes = (day_fraction * 24.0 * 60.0).round() as i64;
                let (hour, minute) = (
                    minutes.div_euclid(60).rem_euclid(24),
                    minutes.rem_euclid(60),
                );
                let event = if first {
                    format!("clock opens at {}", office.label())
                } else {
                    format!("{} rings", office.label())
                };
                println!(
                    "== {hour:02}:{minute:02}  {event}  —  day {day}, {} ==",
                    weekday.label()
                );
            }
            // The mover trace: one line per moved actor per poll, on stdout so it
            // rides the transcript stream with `2>/dev/null` still clean.
            EngineMessage::Movement { moved } if self.trace_positions => {
                for motion in &moved {
                    println!(
                        "[pos] {} x={:.2} z={:.2} speed={:.2}",
                        motion.actor_id, motion.position_m.x, motion.position_m.z, motion.speed
                    );
                }
            }
            // The lamp set (M7): one line per change — the dark seed, each
            // lighting on the dusk round, the dawn snuff.
            EngineMessage::Lamps { lamps } => {
                let lit = lamps.iter().filter(|lamp| lamp.lit).count();
                println!("[lamps] {lit}/{} lit", lamps.len());
            }
            // A keeper working a curb: one line per draw, so a queue reads as a
            // rhythm on the transcript stream.
            EngineMessage::Sound {
                sound_id,
                actor_id,
                position_m,
                recipient_ids,
                ..
            } if self.trace_water && is_water_sound(&sound_id) => {
                let keeper = actor_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "?".to_string());
                println!(
                    "[water] {keeper} :: {sound_id} at x={:.1} z={:.1} ({} heard)",
                    position_m.x,
                    position_m.z,
                    recipient_ids.len()
                );
            }
            _ => {}
        }
    }

    /// The host's half of the backend contract: collect what the cognition has
    /// finished and hand it back as commands.
    ///
    /// `blocking` is true only while a turn is in flight — that is the one moment
    /// a real provider call is worth waiting on.
    fn collect_completions(&self, blocking: bool) -> Result<Vec<EngineCommand>, String> {
        let completions: Vec<Completion> = match &self.brain {
            Brain::Fake(fake) => fake.0.borrow_mut().drain_completions(),
            Brain::Http { handle, .. } => {
                let mut events: Vec<BackendEvent> = handle.drain_events();
                if events.is_empty() && blocking {
                    match handle.events().recv_timeout(COMPLETION_TIMEOUT) {
                        Ok(event) => events.push(event),
                        Err(_) => return Err("the provider never answered".to_string()),
                    }
                }
                events
                    .into_iter()
                    .filter_map(|event| match event {
                        BackendEvent::LlmCompletion(completion) => Some(completion),
                        // No speech backends are wired up here, so nothing else
                        // can arrive on this channel.
                        _ => None,
                    })
                    .collect()
            }
        };
        Ok(completions
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect())
    }

    fn print_new_transcript_lines(&mut self) {
        for line in &self.engine.transcript()[self.printed_lines..] {
            println!("  {line}");
        }
        self.printed_lines = self.engine.transcript().len();
    }

    /// `main.py:162-176`, verbatim in shape: Python `repr` of the goal, Python
    /// list literals for `knows`/`holds`.
    fn print_final_state(&self) {
        println!("\n== final state ==");
        let world: &World = self.engine.world();
        // The night's ledger (M6). Dropped reflections are not failures: the
        // night ended before the lane reached them, which is the design.
        if self.engine.night().enabled() {
            let (reflected, dropped) = self.engine.night().totals();
            println!(
                "[night] {reflected} reflected, {dropped} dropped unspent, {} still owed",
                self.engine.night().owed()
            );
            for (ward, mood) in &world.ward_moods {
                println!("[night] {}: {mood}", ward.as_str());
            }
        }
        if let Some(weather) = world.current_weather {
            println!(
                "{} (wetness: {})",
                weather.prompt_phrase(None),
                weather.wetness_band()
            );
        }
        for actor_id in llm_turn_order(world) {
            let actor = &world.characters[&actor_id];
            // `knows` is a BTreeSet, so it is already `sorted(c.knows)`; a
            // character who has left the world is dropped, not rendered.
            let known: Vec<String> = actor
                .knows()
                .iter()
                .filter_map(|id| world.characters.get(id))
                .map(|known| known.name().to_string())
                .collect();
            let holds: Vec<String> = actor
                .holds()
                .iter()
                .map(|item_id| world.item_dump_label(&world.items[item_id]))
                .collect();
            println!(
                "{}: goal={}, knows={}, holds={}",
                actor.name(),
                py_repr(actor.goal()),
                py_list(&known),
                py_list(&holds),
            );
            for memory in actor.memories() {
                println!("  - {memory}");
            }
        }
        // The player never takes an LLM turn, but their held stacks show the
        // counted-stack rendering (`spark (spr03) ×3`) the item catalog adds.
        if let Some(player) = world.characters.get(&ActorId::from_raw(PLAYER_ID))
            && !player.holds().is_empty()
        {
            let holds: Vec<String> = player
                .holds()
                .iter()
                .map(|item_id| world.item_dump_label(&world.items[item_id]))
                .collect();
            println!("{}: holds={}", player.name(), py_list(&holds));
        }
        for (item_id, offer) in &world.offers {
            let target = offer
                .target_id
                .as_ref()
                .and_then(|id| world.characters.get(id))
                .map(|target| target.name())
                .unwrap_or("anyone");
            let offered = if offer.quantity > 1 {
                format!(
                    "{} {}",
                    offer.quantity,
                    world.item_catalog.display_plural(&world.items[item_id])
                )
            } else {
                world.item_catalog.display_name(&world.items[item_id])
            };
            println!(
                "pending offer: {} offers {offered} ({item_id}) to {target}",
                world.characters[&offer.giver_id].name(),
            );
        }
    }

    fn print_cost(&self) {
        let (cost, usage) = match &self.brain {
            // No provider, no bill — and Python's zero-call run printed exactly
            // this misleading line too (llm-headless.md risk 8).
            Brain::Fake(_) => (None, UsageLedger::new()),
            Brain::Http { http, .. } => {
                let http = http.0.borrow();
                (http.run_cost_usd(), http.usage())
            }
        };
        println!("\n{}", cost_line(cost));
        if let Some(line) = cache_line(&usage) {
            println!("{line}");
        }
    }
}

/// `main.py:178-186`.
fn cost_line(cost: Option<f64>) -> String {
    match cost {
        None => "Run cost: unknown (no pricing entry for this model)".to_string(),
        Some(cost) if cost >= 0.005 => format!("Run cost: {cost:.2} USD"),
        Some(cost) => format!("Run cost: {cost:.4} USD"),
    }
}

/// What the provider's prompt cache actually did with `turn.j2`'s static prefix.
///
/// A run whose hit rate stays at 0% is the signal that the prefix is not being
/// reused — either the template's static block moved back behind the sheet, or
/// the provider does not do prefix caching at all (the openai endpoint does
/// not; moonshot does). The run cost above bills every input token at full
/// price, so a hit here means the true bill is lower than the line above says.
fn cache_line(usage: &UsageLedger) -> Option<String> {
    if usage.is_empty() {
        return None;
    }
    let (prompt_tokens, cached) = usage.prompt_totals();
    if prompt_tokens == 0 {
        return None;
    }
    let percent = 100.0 * cached as f64 / prompt_tokens as f64;
    Some(format!(
        "Input tokens: {prompt_tokens} ({cached} served from the provider's \
         prompt cache, {percent:.0}%)"
    ))
}

// ------------------------------------------------------------------ the assets

/// Everything the engine is seeded from — the data files of ARCHITECTURE
/// §1.4. The sim reads no files itself (D22), so the host reads them for it.
struct Assets {
    seed: WorldSeed,
    areas: AreaMap,
    catalog: SoundCatalog,
    prompts: PromptEnv,
    /// The street graph, when both baked files are present under `--assets`.
    /// Missing files are a warning, not an error: the run simply has no movers.
    nav: Option<Arc<NavData>>,
    shelters: Arc<ShelterMap>,
}

impl Assets {
    fn load(
        directory: &Path,
        lore_directory: &Path,
        know_everybody: bool,
        extra_ambient: u32,
    ) -> Result<Self, String> {
        let read = |relative: &str| -> Result<String, String> {
            let path = directory.join(relative);
            fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))
        };
        let knowledge = if know_everybody {
            PlayerKnowledge::Everyone
        } else {
            PlayerKnowledge::PublicFigures
        };
        let seed = load_world_seed_with_knowledge(directory, lore_directory, knowledge)?;
        let areas = AreaMap::from_json_str(&read("world/areas.json")?)
            .map_err(|error| format!("invalid world areas: {error}"))?;
        let catalog = SoundCatalog::from_toml_str(&read("sounds/catalog.toml")?)
            .map_err(|error| format!("invalid sound catalog: {error}"))?;
        let prompts = PromptEnv::new(
            &read("prompts/turn.j2")?,
            &read("prompts/night.j2")?,
            &read("prompts/strings.toml")?,
        )
        .map_err(|error| format!("invalid prompt assets: {error}"))?;
        let nav = load_nav(directory);
        // The generated crowd (`--extra-ambient`), placed on the same walkable
        // ground the game places it on. No graph, nowhere to stand: refused
        // out loud rather than heaped on the origin.
        let seed = match (extra_ambient, nav.as_deref()) {
            (0, _) => seed,
            (count, Some(nav)) => {
                let count = count.min(cathedral_sim::MAX_EXTRA_AMBIENT_NPCS);
                let points = cathedral_sim::spread_over_walkable(nav, count as usize);
                let sheets = cathedral_sim::extra_ambient_sheets(&points, 0);
                // The no-trade cohort, counted out loud
                // (`features/give_the_crowd_somewhere_to_be.md` M2): roughly a
                // quarter of any crowd has no occupation at all, and every one
                // of them must carry a circumstance saying how they eat — the
                // same pairing the lore loader demands of an authored
                // `no_fixed_trade/` sheet. The unsupported count is a zero that
                // deserves to be printed rather than assumed.
                let no_trade: Vec<&cathedral_sim::CharacterSheet> = sheets
                    .iter()
                    .filter(|sheet| {
                        sheet
                            .lore
                            .as_ref()
                            .is_some_and(|lore| lore.occupation_id.is_none())
                    })
                    .collect();
                let unsupported = no_trade
                    .iter()
                    .filter(|sheet| {
                        !sheet.lore.as_ref().is_some_and(|lore| {
                            lore.circumstances.iter().any(|circumstance| {
                                cathedral_sim::SUPPORT_CIRCUMSTANCES
                                    .contains(&circumstance.as_str())
                            })
                        })
                    })
                    .count();
                eprintln!(
                    "[crowd] {} generated ambient citizens; {} with no trade at all ({:.1}%), \
                     {unsupported} of those with no support circumstance",
                    sheets.len(),
                    no_trade.len(),
                    100.0 * no_trade.len() as f64 / sheets.len().max(1) as f64,
                );
                seed.with_extra_ambient(sheets)
                    .map_err(|error| format!("invalid generated crowd: {error}"))?
            }
            (count, None) => {
                eprintln!("warning: --extra-ambient {count} needs a navigation graph; no crowd");
                seed
            }
        };
        let shelters = Arc::new(
            ShelterMap::from_json_str(&read("world/shelters.json")?)
                .map_err(|error| format!("invalid world shelters: {error}"))?,
        );
        Ok(Self {
            seed,
            areas,
            catalog,
            prompts,
            nav,
            shelters,
        })
    }

    /// The seeded player stays where the seed put him: the headless runner has
    /// no camera to spawn him from.
    fn player_spawn(&self) -> (Vec3, f64) {
        self.seed
            .characters
            .iter()
            .find(|character| character.id.as_str() == PLAYER_ID)
            .map(|player| (player.position_m, player.facing_yaw))
            .unwrap_or((Vec3::ZERO, 0.0))
    }
}

/// Load the baked street graph the same way the host does — `navigation.json`
/// plus its `navigation.bin` companion, relative to `--assets`. Missing files
/// warn and disable movement rather than failing the run.
fn load_nav(directory: &Path) -> Option<Arc<NavData>> {
    let json_path = directory.join("world/navigation.json");
    let bin_path = directory.join("world/navigation.bin");
    let (json, bin) = match (fs::read_to_string(&json_path), fs::read(&bin_path)) {
        (Ok(json), Ok(bin)) => (json, bin),
        _ => {
            eprintln!(
                "warning: no navigation graph under {}; movement is off",
                directory.display()
            );
            return None;
        }
    };
    match NavData::from_parts(&json, &bin) {
        Ok(nav) => Some(Arc::new(nav)),
        Err(error) => {
            eprintln!("warning: navigation graph did not load: {error}; movement is off");
            None
        }
    }
}

// ------------------------------------------------------------------- utilities

/// A [`Cognition`] the loop and the engine both hold.
///
/// The engine owns its `Box<dyn Cognition>`, but the loop still has to drain the
/// fake's staged completions and read the real client's usage ledger — the same
/// shared handle the game's `LocalEngine` will need.
struct Shared<T>(Rc<RefCell<T>>);

impl<T> Shared<T> {
    fn new(inner: T) -> Self {
        Self(Rc::new(RefCell::new(inner)))
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Cognition> Cognition for Shared<T> {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request(prompt)
    }

    fn request_with_budget(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0
            .borrow_mut()
            .request_with_budget(prompt, max_output_tokens)
    }

    /// Forwarded, not defaulted: the default refuses, and a wrapper that
    /// silently swallowed the second lane would make `--night-office` a no-op.
    fn request_night(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request_night(prompt, max_output_tokens)
    }
}

/// CPython's `repr()` of a `str`: single quotes unless the text contains one and
/// no double quote. The goal is a plain sentence in practice, but it is model
/// output, so the escaping is real.
fn py_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// CPython's `repr()` of a `list[str]`.
fn py_list(items: &[String]) -> String {
    let rendered: Vec<String> = items.iter().map(|item| py_repr(item)).collect();
    format!("[{}]", rendered.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cost_footer_switches_precision_at_half_a_cent() {
        assert_eq!(
            cost_line(None),
            "Run cost: unknown (no pricing entry for this model)"
        );
        assert_eq!(cost_line(Some(0.0)), "Run cost: 0.0000 USD");
        assert_eq!(cost_line(Some(0.0018)), "Run cost: 0.0018 USD");
        assert_eq!(cost_line(Some(0.005)), "Run cost: 0.01 USD");
        assert_eq!(cost_line(Some(1.239)), "Run cost: 1.24 USD");
    }

    /// `--status NAME=KIND:VALUE` (npc_bodies M5): a multi-word name, an
    /// out-of-range value and an unknown kind are all handled at parse time.
    #[test]
    fn status_flag_parses_name_kind_and_bounded_value() {
        assert_eq!(
            parse_status_flag("Ilse=drunkenness:0.8"),
            Ok(("Ilse".to_string(), StatusKind::Drunkenness, 0.8))
        );
        // A name may carry spaces; only the last `:` splits kind from value.
        assert_eq!(
            parse_status_flag("Old Nan=weariness:1"),
            Ok(("Old Nan".to_string(), StatusKind::Weariness, 1.0))
        );
        assert!(
            parse_status_flag("Ilse=sobriety:0.5").is_err(),
            "unknown kind"
        );
        assert!(
            parse_status_flag("Ilse=drunkenness:2").is_err(),
            "out of range"
        );
        assert!(
            parse_status_flag("Ilse=drunkenness:-0.1").is_err(),
            "out of range"
        );
        assert!(parse_status_flag("Ilse=drunkenness").is_err(), "no value");
        assert!(
            parse_status_flag("drunkenness:0.5").is_err(),
            "no name split"
        );
        assert!(parse_status_flag("=drunkenness:0.5").is_err(), "empty name");
    }

    #[test]
    fn the_final_state_block_prints_python_literals() {
        // `goal='None'` — the sentinel string, quoted, not a null (D15).
        assert_eq!(py_repr("None"), "'None'");
        assert_eq!(py_list(&[]), "[]");
        assert_eq!(
            py_list(&["Conny".to_string(), "Ilse".to_string()]),
            "['Conny', 'Ilse']"
        );
        assert_eq!(
            py_list(&["copper coin (c0prs)".to_string()]),
            "['copper coin (c0prs)']"
        );
    }

    #[test]
    fn repr_escapes_the_way_cpython_does() {
        assert_eq!(py_repr("it's"), "\"it's\"");
        assert_eq!(py_repr("it's \"quoted\""), "'it\\'s \"quoted\"'");
        assert_eq!(py_repr("a\nb\\c"), "'a\\nb\\\\c'");
    }

    #[test]
    fn the_shipped_assets_load() {
        let assets = Assets::load(Path::new("../../assets"), Path::new("../../lore"), false, 0)
            .expect("the shipped assets load");
        assert_eq!(assets.player_spawn().0, Vec3::new(0.0, 0.91, 95.0));

        // The cast is seed.json's own characters plus every file below
        // lore/characters. Deriving that total rather than copying it keeps the
        // test from going stale each time the city gains a citizen — and, unlike
        // a hardcoded number, it fails when a character file silently does not
        // compose into the seed, which is the thing actually worth catching.
        let base: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../assets/world/seed.json").expect("the seed is readable"),
        )
        .expect("the seed parses");
        let in_seed = base["characters"]
            .as_array()
            .expect("the seed lists characters")
            .len();
        let in_lore = cathedral_backends::world_data::character_sources(Path::new("../../lore"))
            .expect("the lore cast is readable")
            .len();

        assert!(
            in_lore > 100,
            "the lore cast lost its characters: {in_lore}"
        );
        assert_eq!(assets.seed.characters.len(), in_seed + in_lore);
    }

    /// A canned chat-completions endpoint: enough HTTP to answer reqwest, and
    /// nothing more. The lib's `MockServer` is `#[cfg(test)]`-private to the
    /// library, and a bin is its own crate.
    fn stub_provider(replies: Vec<&'static str>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port is free");
        let base_url = format!("http://{}/v1", listener.local_addr().expect("bound"));
        std::thread::spawn(move || {
            for (stream, reply) in listener.incoming().zip(replies) {
                let Ok(mut stream) = stream else { return };
                // Read whatever the client sends; we answer the same way
                // regardless. One read is enough for a Content-Length body.
                let mut scratch = [0_u8; 8192];
                let _ = std::io::Read::read(&mut stream, &mut scratch);
                let body = format!(
                    r#"{{"choices": [{{"message": {{"content": "say {{\"text\": \"{reply}\"}}"}}}}]}}"#
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            }
        });
        base_url
    }

    /// The regression: with a real provider, the *second* speaking turn is
    /// harvested while the conversation floor is still presenting the first, so
    /// the scheduler parks it and leaves the turn in flight. The loop must not
    /// then block on a backend channel that has nothing left to send — nothing
    /// else can arrive on it, so it would wait out the whole `COMPLETION_TIMEOUT`
    /// and abort a healthy run with "the provider never answered". Only the clock
    /// releases a held turn.
    #[test]
    fn a_turn_parked_by_the_floor_does_not_wait_on_the_provider() {
        use cathedral_backends::config::{LlmSettings, Provider};

        let base_url = stub_provider(vec![
            "Good evening to you, traveller.",
            "And a good evening to you.",
        ]);
        let mut config = BackendsConfig::load(&BackendsOptions::default());
        config.llm = Ok(LlmSettings {
            provider: Provider::Moonshot,
            model: "kimi-k2.5".to_string(),
            base_url,
            api_key: "sk-test".to_string(),
            timeout_seconds: 5.0,
            content_parts: true,
        });

        let assets = Assets::load(Path::new("../../assets"), Path::new("../../lore"), false, 0)
            .expect("the shipped assets");
        let (player_spawn, player_yaw) = assets.player_spawn();
        let handle = BackendsHandle::start(config, None).expect("the backends start");
        let http = Shared::new(handle.cognition());
        let engine = Engine::new(
            EngineConfig {
                player_id: ActorId::from_raw(PLAYER_ID),
                turn_delay_seconds: 1.0,
                // No speakers: the floor paces the cast by the reading estimate,
                // which is exactly the window that parks the second turn.
                tts_selected: TtsBackendKind::Off,
                ..EngineConfig::default()
            },
            &assets.seed,
            assets.areas,
            assets.catalog,
            assets.prompts,
            Box::new(http.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (player_spawn, player_yaw),
            0,
            0.0,
        )
        .expect("the seed carries a player");

        let mut runner = Runner {
            engine,
            brain: Brain::Http {
                handle: Box::new(handle),
                http,
            },
            now: 0.0,
            verbose: false,
            printed_lines: 0,
            provider_failed: false,
            requires_news: false,
            last_office: None,
            trace_positions: false,
            trace_water: false,
            census_by_area: false,
            trace_food: false,
        };

        let started = std::time::Instant::now();
        runner.run_ticks(2).expect("both turns finish");
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the loop blocked on an empty channel instead of on the clock"
        );
        assert!(!runner.provider_failed, "the stub answered both turns");
        // The floor really was held: both lines made the transcript.
        assert_eq!(runner.engine.transcript().len(), 2);
    }
}
