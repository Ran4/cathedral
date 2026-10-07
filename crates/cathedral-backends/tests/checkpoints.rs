//! Simulation checkpoints (D06, Alibi R2): a restored city carries on exactly
//! as the saved one would have.
//!
//! Every scenario builds the shipped city — the real cast, nav, areas,
//! shelters and weather, with the Night Office and the knowledge layer on — on
//! the deterministic fake provider. Engine A runs uninterrupted. At the capture
//! point A is checkpointed (a read-only borrow, so A carries on undisturbed)
//! and the checkpoint is encoded to bytes; engine C is decoded and restored
//! from those bytes with fresh services, as a newly started game would be. A
//! and C then take identical steps and must say the same things poll for poll,
//! and every few polls re-encode to the same checkpoint — request ids aside,
//! since each belongs to its own provider.
//!
//! Where an LLM call was out at the capture, the claim is conservation: asked
//! once more, answered once.
//!
//! Under the developer's 40%-of-one-core cap, two test threads is plenty:
//!
//! ```sh
//! cargo test -p cathedral-backends --test checkpoints -- --test-threads=2
//! ```

use std::{
    cell::RefCell,
    collections::BTreeSet,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Instant,
};

use cathedral_backends::{
    save::{self, HostSave, SaveGame},
    world_data::load_world_seed,
};
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Checkpoint, Cognition, CognitionBusy, Engine, EngineCommand,
    EngineConfig, EngineMessage, FakeCognition, IdleCognitionMode, ItemId, NavData,
    NightOfficeConfig, NullSight, NullTranscription, NullTts, Office, PromptEnv, RequestId,
    RestoreError, ShelterMap, SoundCatalog, SpatialActorUpdate, StageConfig, TtsBackendKind,
    Vec3, WeatherConfig, WeatherKind, WeatherMode, WorldClock, WorldSeed, custody::Confinement,
};
use serde_json::Value;

/// One poll: the movement tick, so every poll steps the city.
const STEP: f64 = 0.05;
/// How often the twins' re-encoded checkpoints are compared.
const COMPARE_EVERY: usize = 50;
/// Off the edge of the cast, where nobody is on stage with the player.
const FAR_CORNER: Vec3 = Vec3::new(-360.0, cathedral_sim::WALK_Y, -470.0);
const ILSE: &str = "k0fb1";
const PLAYER: &str = "player";

#[derive(Clone, Default)]
struct SharedCognition(Rc<RefCell<FakeCognition>>);

impl Cognition for SharedCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request(prompt)
    }

    fn request_night(
        &mut self,
        prompt: String,
        max_output_tokens: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request_night(prompt, max_output_tokens)
    }
}

impl SharedCognition {
    fn prompts(&self) -> Vec<String> {
        self.0.borrow().prompts().to_vec()
    }
}

/// The shipped data, parsed once per test binary. Each engine gets its own
/// copies of the assets; the immutable nav and shelters are shared, as they
/// are behind an `Arc` in the game.
struct Assets {
    seed: WorldSeed,
    areas: AreaMap,
    catalog: SoundCatalog,
    prompts: [String; 3],
    nav: Arc<NavData>,
    shelters: Arc<ShelterMap>,
}

fn assets() -> &'static Assets {
    static ASSETS: OnceLock<Assets> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let assets = root.join("assets");
        let read = |relative: &str| {
            std::fs::read_to_string(assets.join(relative))
                .unwrap_or_else(|error| panic!("{relative}: {error}"))
        };
        Assets {
            seed: load_world_seed(&assets, &root.join("lore")).expect("the shipped world loads"),
            areas: AreaMap::from_json_str(&read("world/areas.json")).expect("areas"),
            catalog: SoundCatalog::from_toml_str(&read("sounds/catalog.toml")).expect("sounds"),
            prompts: [
                read("prompts/turn.j2"),
                read("prompts/night.j2"),
                read("prompts/strings.toml"),
            ],
            nav: Arc::new(
                NavData::from_parts(
                    &read("world/navigation.json"),
                    &std::fs::read(assets.join("world/navigation.bin")).expect("the baked nav"),
                )
                .expect("the nav graph loads"),
            ),
            shelters: Arc::new(
                ShelterMap::from_json_str(&read("world/shelters.json")).expect("shelters"),
            ),
        }
    })
}

fn prompts() -> PromptEnv {
    let [turn, night, strings] = &assets().prompts;
    PromptEnv::new(turn, night, strings).expect("prompts")
}

fn config(start: Office) -> EngineConfig {
    let assets = assets();
    EngineConfig {
        fake_mode: true,
        tts_selected: TtsBackendKind::Off,
        // The game's own gate: idle turns go to whoever is near the player.
        idle_mode: IdleCognitionMode::Stage,
        stage: StageConfig::default(),
        clock: WorldClock::new(
            3_600.0,
            start,
            0,
            cathedral_sim::engine::DEFAULT_NIGHT_BRIGHTNESS,
        ),
        weather: WeatherConfig {
            enabled: true,
            seed: 11,
            mode: WeatherMode::Timeline,
            frequency: 1.0,
        },
        shelters: Arc::clone(&assets.shelters),
        nav: Some(Arc::clone(&assets.nav)),
        night_office: NightOfficeConfig {
            enabled: true,
            ..NightOfficeConfig::default()
        },
        ..EngineConfig::default()
    }
}

fn capabilities() -> Capabilities {
    Capabilities::new(true, false, false, false, false, TtsBackendKind::Off)
}

/// One running engine and the fake provider whose answers the host loop feeds
/// back, exactly as the game's pump and the headless runner do.
struct City {
    engine: Engine,
    cognition: SharedCognition,
    now: f64,
    spatial_seq: i64,
}

impl City {
    fn new(start: Office, player_at: Vec3) -> Self {
        Self::with_crowd(start, player_at, 0)
    }

    /// The city with `extra` generated residents, placed as the game places
    /// `extra_ambient_npcs`.
    fn with_crowd(start: Office, player_at: Vec3, extra: usize) -> Self {
        let assets = assets();
        let crowded;
        let seed = if extra == 0 {
            &assets.seed
        } else {
            let occupied: Vec<Vec3> =
                assets.seed.characters.iter().map(|sheet| sheet.position_m).collect();
            let crowd = cathedral_sim::generate_ambient(&assets.nav, extra, 0, &occupied, &[])
                .expect("a small crowd places");
            crowded = assets
                .seed
                .clone()
                .with_extra_ambient(crowd.sheets)
                .expect("the generated crowd is valid");
            &crowded
        };
        let cognition = SharedCognition::default();
        let engine = Engine::new(
            config(start),
            seed,
            assets.areas.clone(),
            assets.catalog.clone(),
            prompts(),
            Box::new(cognition.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            capabilities(),
            (player_at, 0.0),
            0,
            0.0,
        )
        .expect("the shipped seed has a player");
        Self {
            engine,
            cognition,
            now: 0.0,
            spatial_seq: 0,
        }
    }

    /// Beside Ilse, where the stage is never empty.
    fn beside_ilse() -> Self {
        let ilse = assets()
            .seed
            .character(&ActorId::from_raw(ILSE))
            .expect("Ilse is in the shipped cast")
            .position_m;
        Self::new(Office::Dayspring, ilse + Vec3::new(1.2, 0.0, 0.0))
    }

    /// A new process loading a save: decode, then restore with fresh services.
    fn restored(bytes: &[u8], spatial_seq: i64) -> Result<Self, RestoreError> {
        let checkpoint: Checkpoint = serde_json::from_slice(bytes).expect("a checkpoint decodes");
        Self::from_checkpoint(checkpoint, spatial_seq)
    }

    fn from_checkpoint(checkpoint: Checkpoint, spatial_seq: i64) -> Result<Self, RestoreError> {
        Self::restore_under(checkpoint, spatial_seq, config(Office::Dayspring))
    }

    /// Restore under this run's config, which may differ from the saver's.
    fn restore_under(
        checkpoint: Checkpoint,
        spatial_seq: i64,
        config: EngineConfig,
    ) -> Result<Self, RestoreError> {
        let assets = assets();
        let now = checkpoint.now;
        let cognition = SharedCognition::default();
        let engine = Engine::restore(
            checkpoint,
            config,
            assets.areas.clone(),
            assets.catalog.clone(),
            prompts(),
            Box::new(cognition.clone()),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            capabilities(),
        )?;
        Ok(Self {
            engine,
            cognition,
            now,
            spatial_seq,
        })
    }

    fn step(&mut self, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        self.now += STEP;
        let mut all: Vec<EngineCommand> = self
            .cognition
            .0
            .borrow_mut()
            .drain_completions()
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect();
        all.extend(commands);
        self.engine.poll(self.now, all)
    }

    fn capture(&self) -> Vec<u8> {
        serde_json::to_vec(&self.engine.checkpoint(self.now)).expect("a checkpoint encodes")
    }

    fn position_of(&self, actor: &str) -> Vec3 {
        self.engine.world().characters[&ActorId::from_raw(actor)].position_m()
    }

    /// The player keeps at Ilse's shoulder, as a player walking with her
    /// would: a fresh spatial update every poll.
    fn follow_ilse(&mut self) -> (EngineCommand, i64) {
        self.spatial_seq += 1;
        let command = EngineCommand::SpatialUpdate {
            spatial_seq: self.spatial_seq,
            updates: vec![SpatialActorUpdate::new(
                ActorId::from_raw(PLAYER),
                self.position_of(ILSE) + Vec3::new(1.2, 0.0, 0.0),
                None,
            )],
        };
        (command, self.spatial_seq)
    }

    /// Step until `until` holds, failing after `limit` polls.
    fn step_until(
        &mut self,
        limit: usize,
        what: &str,
        mut commands: impl FnMut(&mut City, usize) -> Vec<EngineCommand>,
        mut until: impl FnMut(&City, &[EngineMessage]) -> bool,
    ) {
        let mut diagnostics: Vec<String> = Vec::new();
        for poll in 0..limit {
            let commands = commands(self, poll);
            let messages = self.step(commands);
            if until(self, &messages) {
                return;
            }
            diagnostics.extend(messages.into_iter().filter_map(|message| match message {
                EngineMessage::Diagnostic(line) => Some(line),
                _ => None,
            }));
        }
        let tail = &diagnostics[diagnostics.len().saturating_sub(12)..];
        panic!(
            "{what} did not happen within {limit} polls; the last diagnostics:\n{}",
            tail.join("\n")
        );
    }
}

/// A and C after a capture, stepped together.
struct Twins {
    a: City,
    c: City,
    polls: usize,
}

impl Twins {
    fn split(a: City) -> Self {
        let bytes = a.capture();
        let c = City::restored(&bytes, a.spatial_seq).expect("the capture restores");
        assert_same_world(&a, &c);
        Self { a, c, polls: 0 }
    }

    /// One identical step for both; asserts they said the same things and,
    /// every [`COMPARE_EVERY`] polls, that they are the same city. Returns
    /// what C said.
    fn step(&mut self, commands: Vec<EngineCommand>) -> Vec<EngineMessage> {
        let mut from_a = self.a.step(commands.clone());
        let mut from_c = self.c.step(commands);
        // C's first poll republishes `Ready` and every hot channel, as at
        // startup; the journal is a once-a-second presentation cache whose
        // phase a restore resets.
        let first = self.polls == 0;
        for messages in [&mut from_a, &mut from_c] {
            messages.retain(|message| !(first && republished(message) || is_journal(message)));
        }
        assert_same_messages(self.polls, &from_a, &from_c);
        self.polls += 1;
        if self.polls % COMPARE_EVERY == 0 {
            self.assert_same_city();
        }
        from_c
    }

    fn steps(
        &mut self,
        count: usize,
        mut commands: impl FnMut(&mut City, usize) -> Vec<EngineCommand>,
    ) -> Vec<EngineMessage> {
        let mut said = Vec::new();
        for poll in 0..count {
            let commands = commands(&mut self.a, poll);
            self.c.spatial_seq = self.a.spatial_seq;
            said.extend(self.step(commands));
        }
        self.assert_same_city();
        said
    }

    fn assert_same_city(&self) {
        let (a, c) = (masked(&self.a), masked(&self.c));
        if a != c {
            panic!(
                "poll {}: the restored city diverged at {}",
                self.polls,
                first_difference(&a, &c, String::new()).unwrap_or_default()
            );
        }
        assert_same_world(&self.a, &self.c);
    }
}

/// The two worlds as values. The encoded comparison cannot see a field the
/// checkpoint does not write; this one sees the assets, switches and place
/// indexes a restore rebinds or rebuilds. The host's spatial sequence is the
/// one thing a restore resets on purpose.
fn assert_same_world(a: &City, c: &City) {
    let saved = a.engine.world();
    let mut world = c.engine.world().clone();
    world.spatial_sequence = saved.spatial_sequence;
    if world == *saved {
        return;
    }
    macro_rules! differing {
        ($($field:ident),*) => {
            [$((stringify!($field), world.$field == saved.$field)),*]
                .into_iter()
                .filter_map(|(name, same)| (!same).then_some(name))
                .collect::<Vec<_>>()
        };
    }
    let fields = differing!(
        area_map, characters, roster, items, item_catalog, offers, world_revision,
        event_sequence, sounds_enabled, view_cone_degrees, sound_catalog, current_time,
        current_weather, shelters, nav, places, needle_claim, notices, custody, dogs, marks,
        mark_catalog, marks_enabled, mark_kinds, ward_moods, knowledge, fact_catalog, salience,
        knowledge_enabled, area_adjacency, household_doors, pollen_no_salience
    );
    panic!("the restored world differs in {fields:?} (empty: a crate-private field)");
}

fn republished(message: &EngineMessage) -> bool {
    matches!(
        message,
        EngineMessage::Ready { .. }
            | EngineMessage::Lamps { .. }
            | EngineMessage::Dogs { .. }
            | EngineMessage::LawStanding { .. }
            | EngineMessage::ChalkStanding { .. }
            | EngineMessage::WardHeat { .. }
            | EngineMessage::ResidentStates { .. }
    )
}

fn is_journal(message: &EngineMessage) -> bool {
    matches!(message, EngineMessage::Journal { .. })
}

fn assert_same_messages(poll: usize, a: &[EngineMessage], c: &[EngineMessage]) {
    if a == c {
        return;
    }
    let index = a
        .iter()
        .zip(c)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(c.len()));
    let excerpt = |message: Option<&EngineMessage>| {
        format!("{message:?}").chars().take(800).collect::<String>()
    };
    panic!(
        "poll {poll}: message {index} differs (A said {}, C said {})\nA: {}\nC: {}",
        a.len(),
        c.len(),
        excerpt(a.get(index)),
        excerpt(c.get(index))
    );
}

/// The checkpoint as JSON, with the provider-assigned request ids blanked.
fn masked(city: &City) -> Value {
    let mut value =
        serde_json::to_value(city.engine.checkpoint(city.now)).expect("a checkpoint encodes");
    for pointer in [
        "/state/scheduler/in_flight/request_id",
        "/state/scheduler/held_result/request_id",
        "/state/night/in_flight/request_id",
    ] {
        if let Some(slot) = value.pointer_mut(pointer) {
            *slot = Value::Null;
        }
    }
    value
}

/// The JSON pointer of the first place two values differ, with both sides.
fn first_difference(a: &Value, b: &Value, path: String) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|key| {
                let (p, q) = (
                    x.get(key).unwrap_or(&Value::Null),
                    y.get(key).unwrap_or(&Value::Null),
                );
                first_difference(p, q, format!("{path}/{key}"))
            })
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(index, (p, q))| first_difference(p, q, format!("{path}/{index}"))),
        _ if a == b => None,
        _ => {
            let show = |value: &Value| value.to_string().chars().take(300).collect::<String>();
            Some(format!("{path}: A {} / C {}", show(a), show(b)))
        }
    }
}

/// The prompts A sent that C was answered on: exchanges archived by C whose
/// prompt is `prompt`.
fn answered(messages: &[EngineMessage], prompt: &str) -> usize {
    messages
        .iter()
        .filter(|message| {
            matches!(message, EngineMessage::PromptExchange { prompt: sent, answer: Some(_), .. } if sent == prompt)
        })
        .count()
}

fn sounded(messages: &[EngineMessage], sound: &str) -> usize {
    messages
        .iter()
        .filter(|message| matches!(message, EngineMessage::Sound { sound_id, .. } if sound_id == sound))
        .count()
}

/// The night's answers, by whom: the fake answers a reflection with a
/// memory of the day or a ward's mood, which no daytime turn does.
fn reflections(messages: &[EngineMessage]) -> Vec<ActorId> {
    messages
        .iter()
        .filter_map(|message| match message {
            EngineMessage::PromptExchange {
                actor_id,
                answer: Some(answer),
                ..
            } if answer.starts_with("remember {\"memory\": \"I, ")
                || answer.starts_with("ward_mood") =>
            {
                Some(actor_id.clone())
            }
            _ => None,
        })
        .collect()
}

fn none(_: &mut City, _: usize) -> Vec<EngineCommand> {
    Vec::new()
}

fn follow(city: &mut City, _: usize) -> Vec<EngineCommand> {
    vec![city.follow_ilse().0]
}

fn sixty_times(_: &mut City, poll: usize) -> Vec<EngineCommand> {
    if poll == 0 {
        vec![EngineCommand::CycleTimeScale, EngineCommand::CycleTimeScale]
    } else {
        Vec::new()
    }
}

fn player_say(city: &mut City, text: &str) -> Vec<EngineCommand> {
    let (follow, spatial_seq) = city.follow_ilse();
    vec![
        follow,
        EngineCommand::PlayerSay {
            request_id: format!("say-{spatial_seq}"),
            text: text.to_string(),
            position_m: city.position_of(PLAYER),
            spatial_seq,
        },
    ]
}

// ------------------------------------------------------------------ round trip

#[test]
fn the_shipped_city_round_trips_through_a_save_slot_unchanged() {
    let mut a = City::beside_ilse();
    a.step_until(400, "a busy afternoon", sixty_times, |city, _| {
        city.now > 15.0
    });

    let started = Instant::now();
    let checkpoint = a.engine.checkpoint(a.now);
    let captured = started.elapsed();
    let dir = std::env::temp_dir().join(format!("cathedral-saves-{}", std::process::id()));
    let game = SaveGame {
        checkpoint,
        host: HostSave {
            pitch: 0.25,
            flying: true,
        },
    };
    save::write_slot(&dir, "roundtrip", &game).expect("writes");
    // Again, so the first is kept as the backup.
    let written = save::write_slot(&dir, "roundtrip", &game).expect("writes again");
    assert!(dir.join("roundtrip.bak").exists() && !dir.join("roundtrip.tmp").exists());
    let listed = save::list_slots(&dir);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1.label, game.checkpoint.label);

    let started = Instant::now();
    let read = save::read_slot(&dir, "roundtrip").expect("reads");
    let decoded = started.elapsed();
    assert_eq!(read.host, game.host);
    let started = Instant::now();
    let c = City::from_checkpoint(read.checkpoint, a.spatial_seq).expect("restores");
    let restored = started.elapsed();
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!(
        "[checkpoint] authored city ({}): capture {:.1} ms, encode+check+gzip {:.1} ms, \
         write {:.1} ms, read+decode {:.1} ms, restore {:.1} ms, {} bytes JSON, {} bytes on disk",
        game.checkpoint.label,
        captured.as_secs_f64() * 1e3,
        written.encode_ms,
        written.write_ms,
        decoded.as_secs_f64() * 1e3,
        restored.as_secs_f64() * 1e3,
        written.json_bytes,
        written.file_bytes
    );

    // Restored and re-encoded with no poll between: the same city.
    let (first, again) = (masked(&a), masked(&c));
    assert!(
        first == again,
        "{}",
        first_difference(&first, &again, String::new()).unwrap_or_default()
    );
    assert_same_world(&a, &c);
    assert_eq!(c.engine.world().spatial_sequence, -1, "the host counts its own updates");
}

#[test]
fn a_run_without_weather_loads_a_rainy_save_dry() {
    let mut a = City::new(Office::Dayspring, FAR_CORNER);
    let said = a.step(vec![EngineCommand::SetWeatherOverride {
        kind: WeatherKind::Rain,
        intensity: Some(0.8),
    }]);
    assert!(said.iter().any(
        |message| matches!(message, EngineMessage::Weather(sample) if sample.kind == WeatherKind::Rain)
    ));
    let checkpoint: Checkpoint = serde_json::from_slice(&a.capture()).expect("decodes");
    let mut dry = config(Office::Dayspring);
    dry.weather.enabled = false;
    let mut c = City::restore_under(checkpoint, a.spatial_seq, dry).expect("restores");
    let skies: Vec<WeatherKind> = c
        .step(Vec::new())
        .into_iter()
        .filter_map(|message| match message {
            EngineMessage::Weather(sample) => Some(sample.kind),
            _ => None,
        })
        .collect();
    assert_eq!(skies, vec![WeatherKind::Clear], "this run's switch, not the save's");
}

/// Who thinks next is the developer's cost knob (`idle_cognition.mode` in
/// `config.ron`), so a save made under the stage gate and loaded by a run set
/// to `"all"` takes turns in the city-wide order, as a fresh `"all"` run would.
#[test]
fn a_restored_city_takes_turns_in_this_runs_order() {
    let a = City::beside_ilse();
    let saved: Vec<ActorId> = a.engine.scheduler().turn_order().to_vec();
    assert_eq!(saved, cathedral_sim::stage_turn_order(a.engine.world()));
    let checkpoint: Checkpoint = serde_json::from_slice(&a.capture()).expect("decodes");
    let mut everyone = config(Office::Dayspring);
    everyone.idle_mode = IdleCognitionMode::All;
    let c = City::restore_under(checkpoint, a.spatial_seq, everyone).expect("restores");
    let order = c.engine.scheduler().turn_order();
    assert_eq!(order, cathedral_sim::background_turn_order(c.engine.world()));
    assert_ne!(order, saved, "the two modes weight the cast differently");
}

#[test]
fn two_captures_with_no_poll_between_them_are_the_same_bytes() {
    let mut a = City::beside_ilse();
    for _ in 0..20 {
        let commands = follow(&mut a, 0);
        a.step(commands);
    }
    assert_eq!(a.capture(), a.capture());
}

// ------------------------------------------------------------------ lockstep

#[test]
fn an_idle_city_at_sixty_times_rings_in_the_night_identically() {
    // From Waning to past the Snuffing: the Lamplight and Snuffing bells, the
    // lamps, the ward's curfew and the first of the night's reflections — with
    // a small generated crowd going home to rest, and chalk, a notice and a
    // raised word already abroad at the save.
    let mut a = City::with_crowd(Office::Waning, FAR_CORNER, 60);
    a.step_until(
        200,
        "five o'clock",
        |city, poll| {
            let mut commands = sixty_times(city, poll);
            if poll == 1 {
                commands.extend([
                    EngineCommand::DebugChalk {
                        kind: "ward_sign".to_string(),
                        anchor: "The Wickmarket".to_string(),
                    },
                    EngineCommand::DebugOwe {
                        who: ILSE.to_string(),
                    },
                    EngineCommand::DebugRaiseWord {
                        who: ILSE.to_string(),
                        topic: "bed".to_string(),
                        said: "the reeve's wife was at the Bellstand after curfew".to_string(),
                    },
                ]);
            }
            commands
        },
        |city, _| city.now >= 5.0,
    );
    let world = a.engine.world();
    assert!(
        !world.marks.is_empty() && !world.notices.live().is_empty(),
        "the chalk and the notice are up"
    );
    let mut twins = Twins::split(a);
    let said = twins.steps(260, none);
    assert!(sounded(&said, "town_bell") > 0, "no office rang");
    assert!(!reflections(&said).is_empty(), "nobody reflected");
}

#[test]
fn an_npc_turn_in_flight_is_asked_once_more_and_answered_once() {
    let mut a = City::beside_ilse();
    a.step_until(600, "an idle turn", follow, |city, _| {
        let scheduler = city.engine.scheduler();
        scheduler.in_flight_actor_id().is_some()
            && !scheduler.has_held_result()
            && !scheduler.in_flight_is_player_reaction()
    });
    let prompt = a.cognition.prompts().last().cloned().expect("a prompt went out");
    let mut twins = Twins::split(a);
    assert_eq!(
        twins.c.cognition.prompts(),
        vec![prompt.clone()],
        "the restored engine re-asks exactly the saved prompt"
    );
    let said = twins.steps(120, follow);
    assert_eq!(answered(&said, &prompt), 1, "answered exactly once");
}

#[test]
fn a_reply_held_for_the_floor_is_applied_once_and_never_re_asked() {
    let mut a = City::beside_ilse();
    a.step_until(
        800,
        "a reply held behind a line",
        |city, poll| {
            if poll == 10 {
                player_say(city, "Please wave at me")
            } else {
                follow(city, poll)
            }
        },
        |city, _| city.engine.scheduler().has_held_result(),
    );
    let prompt = a.cognition.prompts().last().cloned().expect("a prompt went out");
    let mut twins = Twins::split(a);
    assert!(twins.c.engine.scheduler().has_held_result());
    assert!(twins.c.cognition.prompts().is_empty(), "a received reply is not re-asked");
    let said = twins.steps(200, follow);
    assert_eq!(answered(&said, &prompt), 1, "applied exactly once");
}

#[test]
fn a_reaction_to_the_player_in_flight_survives_the_restore() {
    let mut a = City::beside_ilse();
    a.step_until(
        400,
        "a reaction to the player",
        |city, poll| {
            if poll == 10 {
                player_say(city, "What's your name?")
            } else {
                follow(city, poll)
            }
        },
        |city, _| city.engine.scheduler().in_flight_is_player_reaction(),
    );
    let prompt = a.cognition.prompts().last().cloned().expect("a prompt went out");
    let mut twins = Twins::split(a);
    assert!(
        twins.c.engine.scheduler().in_flight_is_player_reaction(),
        "still the player's lane"
    );
    let said = twins.steps(120, follow);
    assert_eq!(answered(&said, &prompt), 1, "answered exactly once");
}

#[test]
fn an_offer_made_before_the_save_is_accepted_after_it() {
    let coin = ItemId::from_raw("c0prs");
    let mut a = City::beside_ilse();
    a.step_until(
        800,
        "Ilse holding out her coin",
        |city, poll| {
            if poll == 10 {
                let (follow, spatial_seq) = city.follow_ilse();
                vec![
                    follow,
                    EngineCommand::DebugPlayerSay {
                        request_id: "ask".to_string(),
                        text: "Please offer me your coin".to_string(),
                        target_id: Some(ActorId::from_raw(ILSE)),
                        position_m: city.position_of(PLAYER),
                        spatial_seq,
                    },
                ]
            } else {
                follow(city, poll)
            }
        },
        |city, _| city.engine.world().offers.contains_key(&coin),
    );
    let mut twins = Twins::split(a);
    let said = twins.steps(80, |city, poll| {
        let (follow, spatial_seq) = city.follow_ilse();
        if poll == 5 {
            vec![
                follow,
                EngineCommand::PlayerAccept {
                    request_id: "accept".to_string(),
                    item_id: coin.clone(),
                    position_m: city.position_of(ILSE) + Vec3::new(1.2, 0.0, 0.0),
                    spatial_seq,
                },
            ]
        } else {
            vec![follow]
        }
    });
    assert!(said.iter().any(|message| matches!(
        message,
        EngineMessage::CommandResult { request_id, success: true, .. } if request_id == "accept"
    )));
    let player = &twins.c.engine.world().characters[&ActorId::from_raw(PLAYER)];
    assert!(player.holds().contains(&coin), "the coin changed hands");
}

#[test]
fn a_seizure_mid_escort_still_commits_after_the_restore() {
    let ilse = ActorId::from_raw(ILSE);
    let mut a = City::beside_ilse();
    a.step_until(
        200,
        "Ilse led away",
        |_, poll| {
            if poll == 2 {
                vec![EngineCommand::DebugSeize {
                    officer: "Ede Clove".to_string(),
                    target: Some(ILSE.to_string()),
                }]
            } else {
                Vec::new()
            }
        },
        |city, _| {
            city.now >= 2.0
                && city
                    .engine
                    .world()
                    .custody
                    .get(&ilse)
                    .is_some_and(|record| record.state == Confinement::InCharge)
        },
    );
    let mut twins = Twins::split(a);
    assert_eq!(
        twins.c.engine.world().custody.get(&ilse).map(|record| record.state),
        Some(Confinement::InCharge)
    );
    twins.steps(80, |_, poll| {
        if poll == 10 {
            vec![EngineCommand::DebugCommit {
                target: Some(ILSE.to_string()),
            }]
        } else {
            Vec::new()
        }
    });
    assert_eq!(
        twins.c.engine.world().custody.get(&ilse).map(|record| record.state),
        Some(Confinement::Committed)
    );
}

#[test]
fn a_fact_seeded_into_the_air_spreads_identically_after_the_restore() {
    let mut a = City::new(Office::Dayspring, FAR_CORNER);
    a.step_until(
        40,
        "a word in the Wick's air",
        |city, poll| {
            let mut commands = sixty_times(city, poll);
            if poll == 1 {
                commands.push(EngineCommand::DebugSeedFact {
                    fact: "ashe.salt.short".to_string(),
                    ward: Some("wick".to_string()),
                });
            }
            commands
        },
        |city, _| city.engine.world().knowledge.air_entries() > 0,
    );
    let held = |city: &City| {
        let world = city.engine.world();
        world
            .characters
            .keys()
            .map(|actor| world.knowledge.holdings_len(actor))
            .sum::<usize>()
    };
    let before = held(&a);
    let mut twins = Twins::split(a);
    twins.steps(300, none);
    assert!(held(&twins.c) > before, "nobody picked the word up");
}

#[test]
fn the_weather_turns_while_bell_strokes_are_still_owed() {
    let mut a = City::new(Office::Waning, FAR_CORNER);
    // The Lamplight bell: six strokes three seconds apart, the first on the
    // crossing and the rest owed. It is also some people's bedtime, so wait
    // out any reflection already in the air — that one is the night test's.
    a.step_until(300, "the Lamplight bell", sixty_times, |_, said| {
        sounded(said, "town_bell") > 0
    });
    a.step_until(40, "a quiet moment in the night lane", none, |city, _| {
        city.engine.night().in_flight_subject().is_none()
    });
    let mut twins = Twins::split(a);
    let said = twins.steps(360, |_, poll| match poll {
        3 => vec![EngineCommand::SetWeatherOverride {
            kind: WeatherKind::Rain,
            intensity: Some(0.8),
        }],
        200 => vec![EngineCommand::ClearWeatherOverride],
        _ => Vec::new(),
    });
    assert!(sounded(&said, "town_bell") >= 4, "the owed strokes rang");
    assert!(said.iter().any(
        |message| matches!(message, EngineMessage::Weather(sample) if sample.kind == WeatherKind::Rain)
    ));
}

// ------------------------------------------------------------ conservation

#[test]
fn a_reflection_out_at_the_save_happens_once_and_nobody_reflects_twice() {
    let mut a = City::new(Office::Lamplight, FAR_CORNER);
    a.step_until(400, "a reflection out", sixty_times, |city, _| {
        city.engine.night().in_flight_subject().is_some()
    });
    let subject = a.engine.night().in_flight_subject().expect("checked");
    let bytes = a.capture();
    let mut c = City::restored(&bytes, a.spatial_seq).expect("restores");
    assert_eq!(c.engine.night().in_flight_subject(), None);
    assert!(c.engine.night().owed() > 0, "back in the queue");

    // The reflection that was out lands later in C than in A — it waits its
    // turn again — so this compares who reflected, not when.
    let (mut from_a, mut from_c) = (Vec::new(), Vec::new());
    for _ in 0..240 {
        from_a.extend(reflections(&a.step(Vec::new())));
        from_c.extend(reflections(&c.step(Vec::new())));
    }
    let once = |reflected: &[ActorId]| {
        let unique: BTreeSet<&ActorId> = reflected.iter().collect();
        assert_eq!(unique.len(), reflected.len(), "somebody reflected twice: {reflected:?}");
        unique.into_iter().cloned().collect::<BTreeSet<ActorId>>()
    };
    let (in_a, in_c) = (once(&from_a), once(&from_c));
    assert_eq!(in_a, in_c, "the same people reflected");
    assert!(
        in_c.iter().any(|actor| actor.as_str() == subject || actor.as_str() == format!("ward:{subject}")),
        "{subject}'s reflection happened in the restored city"
    );
    assert_eq!(a.engine.night().totals().0, c.engine.night().totals().0);
}

// ------------------------------------------------------------------ refusals

#[test]
fn a_save_of_another_version_or_city_or_with_a_body_nowhere_is_refused() {
    let mut a = City::beside_ilse();
    for _ in 0..10 {
        let commands = follow(&mut a, 0);
        a.step(commands);
    }
    let before = a.capture();
    let encoded: Value = serde_json::from_slice(&before).expect("json");

    let mut version = encoded.clone();
    version["version"] = Value::from(999);
    let refused = City::from_checkpoint(serde_json::from_value(version).expect("decodes"), 0);
    assert!(matches!(refused, Err(RestoreError::Version { found: 999 })));

    let mut geometry = encoded.clone();
    let saved = geometry["geometry"].as_u64().expect("a fingerprint");
    geometry["geometry"] = Value::from(saved ^ 1);
    let refused = City::from_checkpoint(serde_json::from_value(geometry).expect("decodes"), 0);
    assert!(matches!(refused, Err(RestoreError::Geometry { .. })));

    // A NaN cannot even be written as JSON (it becomes `null`, which no
    // position decodes from), and a checkpoint that carries one in memory is
    // refused by the restore itself.
    let player = ActorId::from_raw(PLAYER);
    let standing = a.position_of(PLAYER);
    a.engine
        .world_mut()
        .characters
        .get_mut(&player)
        .expect("the player")
        .state
        .position_m = Vec3::new(f64::NAN, standing.y, standing.z);
    let nowhere = a.engine.checkpoint(a.now);
    a.engine
        .world_mut()
        .characters
        .get_mut(&player)
        .expect("the player")
        .state
        .position_m = standing;
    let bytes = serde_json::to_vec(&nowhere).expect("encodes, as null");
    assert!(serde_json::from_slice::<Checkpoint>(&bytes).is_err());
    assert!(matches!(
        City::from_checkpoint(nowhere, 0),
        Err(RestoreError::Invalid(_))
    ));

    // The running engine never noticed.
    assert_eq!(a.capture(), before);
    let commands = follow(&mut a, 0);
    a.step(commands);
}

// ------------------------------------------------------------------ cost

/// `cargo test -p cathedral-backends --test checkpoints -- --ignored --nocapture`
#[test]
#[ignore = "measurement: a 2,000-resident city"]
fn a_crowded_city_checkpoint_costs() {
    let assets = assets();
    let points = cathedral_sim::spread_over_walkable(&assets.nav, 2_000);
    let sheets = cathedral_sim::extra_ambient_sheets(&assets.nav, &points, 0);
    let seed = assets
        .seed
        .clone()
        .with_extra_ambient(sheets)
        .expect("the generated crowd is valid");
    let cognition = SharedCognition::default();
    let engine = Engine::new(
        config(Office::Dayspring),
        &seed,
        assets.areas.clone(),
        assets.catalog.clone(),
        prompts(),
        Box::new(cognition.clone()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        capabilities(),
        (FAR_CORNER, 0.0),
        0,
        0.0,
    )
    .expect("the crowded seed has a player");
    let mut a = City {
        engine,
        cognition,
        now: 0.0,
        spatial_seq: 0,
    };
    a.step_until(200, "a minute of the crowd", sixty_times, |city, _| {
        city.now >= 5.0
    });
    for round in 0..3 {
        let started = Instant::now();
        let checkpoint = a.engine.checkpoint(a.now);
        let captured = started.elapsed();
        let started = Instant::now();
        let bytes = serde_json::to_vec(&checkpoint).expect("encodes");
        let encoded = started.elapsed();
        let started = Instant::now();
        let decoded: Checkpoint = serde_json::from_slice(&bytes).expect("decodes");
        let c = City::from_checkpoint(decoded, 0).expect("restores");
        let restored = started.elapsed();
        if round == 0 {
            assert!(masked(&a) == masked(&c), "the crowded city round-trips");
            assert_same_world(&a, &c);
        }
        eprintln!(
            "[checkpoint] +2,000 residents, run {round}: {} characters, capture {:.1} ms, \
             encode {:.1} ms, decode+restore {:.1} ms, {} bytes",
            c.engine.world().characters.len(),
            captured.as_secs_f64() * 1e3,
            encoded.as_secs_f64() * 1e3,
            restored.as_secs_f64() * 1e3,
            bytes.len()
        );
    }
}
