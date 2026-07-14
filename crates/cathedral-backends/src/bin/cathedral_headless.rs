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
    time::Duration,
};

use cathedral_backends::{
    BackendEvent, BackendRuntime, BackendsConfig, BackendsHandle, BackendsOptions, Environment,
    HttpCognition, LlmClient, LlmError,
    config::{DEFAULT_DOTENV_PATH, DEFAULT_WORKERS_DIR},
    world_data::load_world_seed_with_knowledge,
};
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Completion,
    DEFAULT_VIEW_CONE_DEGREES, Engine, EngineCommand, EngineConfig, EngineMessage, FakeCognition,
    NullSight, NullTranscription, NullTts, PlayerKnowledge, PromptEnv, RequestId, SoundCatalog,
    TtsBackendKind, Vec3, World, WorldSeed,
    engine::{
        DEFAULT_MAXIMUM_BACKOFF_SECONDS, DEFAULT_SOUND_COOLDOWN_SECONDS,
        DEFAULT_STT_STREAM_GRACE_SECONDS,
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

    /// LLM provider override (moonshot | openai); beats LLM_PROVIDER
    #[arg(long)]
    provider: Option<String>,

    /// model override; beats LLM_MODEL
    #[arg(long)]
    model: Option<String>,

    /// send FILE as the whole prompt and print the reply (the old `kimi.py`)
    #[arg(long, value_name = "FILE")]
    one_shot: Option<PathBuf>,

    /// directory holding prompts/, sounds/ and world/
    #[arg(long, default_value = "assets", value_name = "DIR")]
    assets: PathBuf,

    /// directory holding characters/ and core_lore/; defaults beside assets/
    #[arg(long, value_name = "DIR")]
    lore: Option<PathBuf>,
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
    let assets = Assets::load(&args.assets, &lore, args.know_everybody)?;

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
    };
    runner.run_ticks(args.ticks)?;
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
}

impl Runner {
    fn run_ticks(&mut self, ticks: u32) -> Result<(), String> {
        for tick in 1..=ticks {
            let actor_name = self.submit_turn()?;
            println!("\n== tick {tick}: {actor_name} ==");
            self.apply_turn()?;
            self.print_new_transcript_lines();
        }
        Ok(())
    }

    /// Pump until the scheduler has a turn in flight, advancing the clock past
    /// the inter-turn delay, a provider backoff, or a floor pause as needed.
    /// Returns the name of the actor whose turn it is.
    fn submit_turn(&mut self) -> Result<String, String> {
        let deadline = self.now + MAX_SIM_SECONDS_PER_PHASE;
        while self.engine.scheduler().in_flight_actor_id().is_none() {
            let commands = self.collect_completions(false)?;
            self.pump(commands);
            if self.engine.scheduler().in_flight_actor_id().is_some() {
                break;
            }
            if self.now > deadline {
                return Err("the scheduler never started a turn".to_string());
            }
            self.now += CLOCK_STEP_SECONDS;
        }
        let actor_id = self
            .engine
            .scheduler()
            .in_flight_actor_id()
            .expect("the loop only exits with a turn in flight")
            .clone();
        Ok(self
            .engine
            .world()
            .characters
            .get(&actor_id)
            .map(|actor| actor.name().to_string())
            .unwrap_or_else(|| actor_id.to_string()))
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
                .map(|item_id| format!("{} ({item_id})", world.items[item_id].name))
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
        for (item_id, offer) in &world.offers {
            let target = offer
                .target_id
                .as_ref()
                .and_then(|id| world.characters.get(id))
                .map(|target| target.name())
                .unwrap_or("anyone");
            println!(
                "pending offer: {} offers {} ({item_id}) to {target}",
                world.characters[&offer.giver_id].name(),
                world.items[item_id].name,
            );
        }
    }

    fn print_cost(&self) {
        let cost = match &self.brain {
            // No provider, no bill — and Python's zero-call run printed exactly
            // this misleading line too (llm-headless.md risk 8).
            Brain::Fake(_) => None,
            Brain::Http { http, .. } => http.0.borrow().run_cost_usd(),
        };
        println!("\n{}", cost_line(cost));
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

// ------------------------------------------------------------------ the assets

/// Everything the engine is seeded from — the data files of ARCHITECTURE
/// §1.4. The sim reads no files itself (D22), so the host reads them for it.
struct Assets {
    seed: WorldSeed,
    areas: AreaMap,
    catalog: SoundCatalog,
    prompts: PromptEnv,
}

impl Assets {
    fn load(directory: &Path, lore_directory: &Path, know_everybody: bool) -> Result<Self, String> {
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
        let prompts = PromptEnv::new(&read("prompts/turn.j2")?, &read("prompts/strings.toml")?)
            .map_err(|error| format!("invalid prompt assets: {error}"))?;
        Ok(Self {
            seed,
            areas,
            catalog,
            prompts,
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
        let assets = Assets::load(Path::new("../../assets"), Path::new("../../lore"), false)
            .expect("the shipped assets load");
        assert_eq!(assets.player_spawn().0, Vec3::new(0.0, 0.91, 95.0));
        assert_eq!(assets.seed.characters.len(), 104);
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

        let assets = Assets::load(Path::new("../../assets"), Path::new("../../lore"), false)
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
