//! **The cadence band** (`features/knowledge_and_rumor/plan/02_numbers.md`).
//!
//! The whole knowledge layer's claim about *speed* is two sentences, and both are
//! only true of the **shipped city**: a `Bed` fact minted at the Wickmarket is
//! being said in another ward within about one office and has reached every ward
//! inside a game day; a `Craft` fact minted beside it, the same hour, by the same
//! mouth, may never leave its ward at all. Nothing in `cathedral-sim`'s own tests
//! can check either, because nothing there has 519 people, a nav graph or a daily
//! round. This crate owns the filesystem, so this is where the city is.
//!
//! Read the table it prints:
//!
//! ```sh
//! cargo test -p cathedral-backends --test pollen_cadence -- --nocapture
//! ```
//!
//! **Three rules bind every test here, and each one is a lesson already paid for.**
//!
//! 1. **The clock is the shipped clock** (`seconds_per_day: 3600`) and the engine
//!    steps at **0.4 s** = `MAX_MOVEMENT_CATCHUP_SLICES × MOVEMENT_TICK_SECONDS`,
//!    the most walking one poll can realise (D22). Rolls are clock-invariant;
//!    *walking* is not. At `seconds_per_day` 120 a game day is 120 sim-seconds and a
//!    270 m leg never completes inside one, so the band is unmeasurable there —
//!    `the_roll_count_is_invariant_under_the_time_scale` is the only test below that
//!    touches another clock, and it asserts the half that *is* invariant.
//! 2. **Every realised backstop reads `wards_reached`** — the number of wards whose
//!    *air* holds a row, i.e. deposits (D54). A cold holder walking home is not
//!    "the word being said" anywhere, so `holder_exits` is printed and never
//!    asserted.
//! 3. **Every expectation reads the run's own integrand**, `expected_crossings` =
//!    `warm_mint_ward_game_hours × X_W / 24` with `X_W` the **mint ward's** exit
//!    rate off that sample's snapshot — never a hardcoded 2.368 and never the city
//!    mean, because the standing wards are 7:1 lopsided.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Control, Engine, EngineCommand,
    EngineConfig, FactId, FactKey, FakeCognition, IdleCognitionMode, NavData, NullSight,
    NullTranscription, NullTts, Office, PlanningWard, PromptEnv, SoundCatalog, StageConfig,
    TtsBackendKind, Vec3, World, WorldClock,
    ids::RequestId,
    knowledge::{
        self, HOLDINGS_MAX, REHEAT_TO, Topic,
        pollen::{CrossingTally, PollenCensus, TopicRow},
    },
};

/// The measurement place. `02_numbers.md` §4 states both ends of the band from a
/// mint here, at the Dayspring bell.
const WICKMARKET: &str = "The Wickmarket";

/// The step the engine takes, in sim seconds: `MAX_MOVEMENT_CATCHUP_SLICES` (8) ×
/// `MOVEMENT_TICK_SECONDS` (0.05). Any coarser and a poll cannot realise the walking
/// it is charged for, so every commute starves and the band collapses (D22).
const STEP_SECONDS: f64 = 0.4;

/// Samples per game hour. Two — one per stir — so a ward transit (~1.2 gh at walking
/// pace across a 200–300 m ward) cannot fall between two samples, which is the one
/// way `CrossingTally` can silently undercount.
const SAMPLES_PER_GAME_HOUR: f64 = 2.0;

/// The shipped clock (`config.ron:50`). The band is *defined* here (D22).
const SECONDS_PER_DAY: f64 = 3600.0;

/// A corner of the map nobody lives in, so the cast takes essentially no turns and
/// the run measures the round's tide and the pollen roll rather than the fake
/// provider.
const FAR_CORNER: Vec3 = Vec3::new(-360.0, cathedral_sim::WALK_Y, -470.0);

#[derive(Clone, Default)]
struct SharedCognition(Rc<RefCell<FakeCognition>>);

impl Cognition for SharedCognition {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        self.0.borrow_mut().request(prompt)
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Which measurement levers a run is under. Two `bool`s rather than two functions,
/// so the identity test runs the *same* code path twice.
#[derive(Clone, Copy, Default)]
struct Levers {
    flat: bool,
    no_salience: bool,
}

/// A run: the engine, and the fake provider handle whose completions the loop feeds
/// back — exactly as `cathedral-headless`' own watch loop does, so a test run and a
/// traced run are the same run.
struct Run {
    engine: Engine,
    cognition: SharedCognition,
    now: f64,
}

/// The shipped city, through the real `Engine::new`.
fn city(levers: Levers, seconds_per_day: f64, packs: Vec<String>, crowd: usize) -> Run {
    let root = root();
    let assets = root.join("assets");
    let seed = load_world_seed(&assets, &root.join("lore")).expect("the shipped world data loads");
    let areas = AreaMap::from_json_str(&read(&assets.join("world/areas.json"))).expect("areas");
    let catalog =
        SoundCatalog::from_toml_str(&read(&assets.join("sounds/catalog.toml"))).expect("sounds");
    let prompts = PromptEnv::new(
        &read(&assets.join("prompts/turn.j2")),
        &read(&assets.join("prompts/night.j2")),
        &read(&assets.join("prompts/strings.toml")),
    )
    .expect("prompts");
    let nav = NavData::from_parts(
        &read(&assets.join("world/navigation.json")),
        &std::fs::read(assets.join("world/navigation.bin")).expect("the baked nav"),
    )
    .expect("the nav graph loads");
    let seed = match crowd {
        0 => seed,
        count => {
            let points = cathedral_sim::spread_over_walkable(&nav, count);
            let sheets = cathedral_sim::extra_ambient_sheets(&nav, &points, 0);
            seed.with_extra_ambient(sheets)
                .expect("the generated crowd is valid")
        }
    };

    let cognition = SharedCognition::default();
    let engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            tts_selected: TtsBackendKind::Off,
            // Gated and news-hungry, with the player parked in an empty corner:
            // between them the cast takes essentially no turns.
            idle_mode: IdleCognitionMode::Stage,
            stage: StageConfig::default(),
            idle_requires_news: true,
            // There is no `EngineConfig::start_office`: the office is the clock's.
            clock: WorldClock::new(
                seconds_per_day,
                Office::Dayspring,
                0,
                cathedral_sim::engine::DEFAULT_NIGHT_BRIGHTNESS,
            ),
            ring_the_offices: true,
            nav: Some(std::sync::Arc::new(nav)),
            fact_packs: packs,
            pollen_flat: levers.flat,
            pollen_no_salience: levers.no_salience,
            ..EngineConfig::default()
        },
        &seed,
        areas,
        catalog,
        prompts,
        Box::new(cognition.clone()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (FAR_CORNER, 0.0),
        0,
        0.0,
    )
    .expect("the shipped seed has a player");
    let mut run = Run {
        engine,
        cognition,
        now: 0.0,
    };
    // Publish the clock the first poll would publish, so a fact planted **before**
    // the first tick is stamped. An unstamped fact never cools, so every warm life
    // in `02_numbers.md` would be infinite and the slow end unmeasurable.
    // `--trace-knowledge` and `--pollen-seed` do the same thing for the same reason;
    // the first poll overwrites it unconditionally.
    let at = run.engine.config().clock.at(0.0);
    run.engine.world_mut().current_time = Some(at);
    run
}

impl Run {
    fn clock(&self) -> WorldClock {
        self.engine.config().clock
    }

    /// Absolute game-days at the instant the pack is planted — `now = 0`, the
    /// Dayspring bell.
    fn minted_at(&self) -> f64 {
        self.clock().game_days(0.0)
    }

    fn game_hours(&self) -> f64 {
        (self.clock().game_days(self.now) - self.minted_at()) * 24.0
    }

    /// One step of the loop, with the fake provider's completions fed back exactly
    /// as the headless watch loop feeds them.
    fn step(&mut self) {
        let commands: Vec<EngineCommand> = self
            .cognition
            .0
            .borrow_mut()
            .drain_completions()
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect();
        self.now += STEP_SECONDS * self.clock().seconds_per_day() / SECONDS_PER_DAY;
        self.engine.poll(self.now, commands);
    }

    /// Step to `until_game_hours` from the mint, sampling census + tally snapshot
    /// twice a game hour. Resumable: a second call carries on from where the first
    /// stopped, which is what lets the standing-fact test age a world, ask a
    /// question, and age it again.
    fn sample_to(
        &mut self,
        tally: &mut CrossingTally,
        until_game_hours: f64,
    ) -> Vec<(f64, PollenCensus)> {
        self.sample_to_at(tally, until_game_hours, SAMPLES_PER_GAME_HOUR)
    }

    /// [`Self::sample_to`] at a chosen cadence — for the one test that shows the
    /// tally's X does not depend on it.
    fn sample_to_at(
        &mut self,
        tally: &mut CrossingTally,
        until_game_hours: f64,
        samples_per_game_hour: f64,
    ) -> Vec<(f64, PollenCensus)> {
        let gap = self.clock().seconds_per_day() / (24.0 * samples_per_game_hour);
        let mut samples: Vec<(f64, PollenCensus)> = Vec::new();
        let mut next_sample = self.now;
        loop {
            if self.now >= next_sample {
                let game_days = self.clock().game_days(self.now);
                tally.sample(self.engine.world(), game_days);
                let mut census = self.engine.pollen_census(self.now);
                census.fill(&tally.snapshot());
                let age = self.game_hours();
                samples.push((age, census));
                next_sample = self.now + gap;
                if age >= until_game_hours {
                    break;
                }
            }
            self.step();
        }
        samples
    }
}

/// Plant the pack at a place at `now = 0` (the Dayspring bell), then step to
/// `until_game_hours`.
///
/// Returns every sample as `(game hours since the mint, census)`, already `fill`ed
/// from that sample's snapshot — so one run answers any bell.
///
/// Asserts inside that the final snapshot's city-mean X is at least 2.0, with a
/// message naming the walking/clock coupling: a future clock or step change fails
/// **here**, in one place, and not as a mystery in a band.
fn run_band(place: &str, until_game_hours: f64, levers: Levers) -> Vec<(f64, PollenCensus)> {
    let mut run = city(levers, SECONDS_PER_DAY, Vec::new(), 0);
    let (keys, line) = run
        .engine
        .seed_pollen_pack(place, 0.0)
        .expect("the pack plants at the named place");
    println!("{line}");
    let mut tally = CrossingTally::new(&keys);
    let samples = run.sample_to(&mut tally, until_game_hours);

    let city_x = samples
        .last()
        .map_or(0.0, |(_, census)| census.crossings_per_person_per_game_day);
    assert!(
        city_x >= 2.0,
        "the city crossed only {city_x:.2} ward boundaries per person per game day. \
         Rolls are clock-invariant but WALKING IS NOT (D22): check the step \
         ({STEP_SECONDS} s, the most one poll can realise) and the clock \
         ({SECONDS_PER_DAY} s/day) before touching any constant"
    );
    samples
}

/// The first sample at or after `game_hours` from the mint.
fn at(samples: &[(f64, PollenCensus)], game_hours: f64) -> &PollenCensus {
    samples
        .iter()
        .find(|(age, _)| *age >= game_hours)
        .map(|(_, census)| census)
        .unwrap_or_else(|| {
            panic!(
                "no sample at or after {game_hours} gh; the run stopped at {:.2} gh",
                samples.last().map_or(0.0, |(age, _)| *age)
            )
        })
}

/// One row by its **fact id**, never by topic alone: the two shipped authored rows
/// are `Coin` and `Craft` too, so a topic lookup would sometimes read `facts.json`.
fn row_of<'a>(census: &'a PollenCensus, id: &str) -> &'a TopicRow {
    let id = FactId::from_raw(id);
    census
        .topics
        .iter()
        .find(|row| row.fact == id)
        .unwrap_or_else(|| panic!("no census row for {id}"))
}

/// The pack's row for one topic.
fn row(census: &PollenCensus, topic: Topic) -> &TopicRow {
    row_of(census, &format!("pollen.{}", topic.as_str()))
}

/// The ward whose air holds a fact at hops 0 with no `via` — the ward it was minted
/// in, and the one every expectation divides by.
fn mint_ward(world: &World, key: FactKey) -> PlanningWard {
    PlanningWard::ALL
        .into_iter()
        .find(|ward| {
            world
                .knowledge
                .drift(*ward, key)
                .is_some_and(|drift| drift.hops == 0 && drift.via.is_none())
        })
        .expect("a freshly minted fact is in its own ward's air")
}

/// A named nav place's point — the mint point `seed_pollen_pack` used, for the
/// tests that re-plant the pack's rows with their own stamps.
fn place_point(world: &World, name: &str) -> Vec3 {
    let nav = world.nav.as_ref().expect("the harness loads the nav graph");
    let needle = name.to_lowercase();
    let place = nav
        .places()
        .iter()
        .find(|place| place.name.to_lowercase().contains(&needle))
        .unwrap_or_else(|| panic!("no nav place named {name}"));
    nav.node_point(place.node)
}

/// This process's peak resident set, off `/proc/self/status` — the RSS half of
/// the footprint record, read where the caps are actually full.
fn peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Print one topic's whole trace, so a failure is read off the run and not guessed.
fn trace(samples: &[(f64, PollenCensus)], topic: Topic) {
    println!(
        "\n  {:>6}  {:>5} {:>8} {:>5} {:>8} {:>6}   {topic:?}",
        "age gh", "wards", "carriers", "warm", "expect", "exits"
    );
    for (age, census) in samples {
        let row = row(census, topic);
        println!(
            "  {age:>6.2}  {:>5} {:>8} {:>5} {:>8.3} {:>6.0}",
            row.wards_reached,
            row.carriers,
            row.volunteering,
            row.expected_crossings,
            row.holder_exits,
        );
    }
}

// ---------------------------------------------------------------------------
// The fast end
// ---------------------------------------------------------------------------

/// **F1.** *"A `Bed` or `Blood` fact minted at the Wickmarket is being said in the
/// Weigh Ward within about one office"* — one office (Dayspring, 5 gh) plus a poll
/// gap, stated at the city's **own** bells: HighWick + 1.5 gh = 6.5 gh, Waning +
/// 1.5 gh = 9.5 gh (`clock.rs`).
///
/// The only lever if this misses is `STIRS_PER_GAME_HOUR` — the wave's *density*.
/// **The bells do not move: they are the city's.**
#[test]
fn the_fast_end_crosses_a_ward_within_one_office() {
    let samples = run_band(WICKMARKET, 9.5, Levers::default());
    trace(&samples, Topic::Bed);
    let first_bell = row(at(&samples, 6.5), Topic::Bed);
    assert!(
        first_bell.wards_reached >= 2,
        "at HighWick + 1.5 gh the Bed fact is in {} ward(s)' air, not 2 or more. \
         The one lever is STIRS_PER_GAME_HOUR (density), never the bells \
         (02_numbers.md §10.1)",
        first_bell.wards_reached
    );
    let second_bell = row(at(&samples, 9.5), Topic::Bed);
    assert!(
        second_bell.wards_reached >= 4,
        "at Waning + 1.5 gh the Bed fact is in {} ward(s)' air, not 4 or more",
        second_bell.wards_reached
    );
    // The fast *side* of the band, so a speed-up cannot pass silently: the spec's
    // "faster and nothing can ever be outrun". The player walks the Wickmarket to
    // the Shambles well (the nearest Weigh place, 224 m) in 1.15 gh; at one game
    // hour the wave may be in most wards' air — measured 6 of 8, because the
    // coin-0 pickups at the bell walk the Dayspring tide — but not yet in all of
    // them. STIRS_PER_GAME_HOUR cannot move the coin-0 pickups, so a failure here
    // is a transport change (the commute, the walk speed, the clock), not a
    // constant: see m2_measurements.md §1 before dividing anything.
    let first_hour = row(at(&samples, 1.0), Topic::Bed);
    assert!(
        first_hour.wards_reached < 8,
        "one game hour after the mint the Bed fact is already in every ward's air \
         ({}/8); nothing can be outrun any more. This is the band's ceiling, and it \
         is the city's commute rather than a constant — read m2_measurements.md §1 \
         before touching STIRS_PER_GAME_HOUR",
        first_hour.wards_reached
    );
}

/// **F2.** *"and has reached every ward inside a game day."* Asserted as a
/// **measurement**: saturation is certain by `02_numbers.md` §4's arithmetic, but it
/// is a branching process over the ward graph with no honest closed form.
#[test]
fn the_fast_end_reaches_every_ward_inside_a_game_day() {
    let samples = run_band(WICKMARKET, 24.0, Levers::default());
    let day = row(at(&samples, 24.0), Topic::Bed);
    println!(
        "[pollen] bed at one game day: wards {}/8, carriers {}, warm {}, mean hops {:.2}, \
         holder exits {:.0}",
        day.wards_reached, day.carriers, day.volunteering, day.mean_hops, day.holder_exits
    );
    assert_eq!(
        day.wards_reached, 8,
        "after a game day the Bed fact is in {}/8 wards' air",
        day.wards_reached
    );
}

// ---------------------------------------------------------------------------
// The slow end
// ---------------------------------------------------------------------------

/// **S1.** *"A `Craft` fact minted beside it, the same hour, the same mouth, may
/// never leave its ward at all"* — the expectation the spec words it as
/// (`expected_crossings` over a game day below one, computed 0.057), and the deposit
/// count that is what the city actually did (`wards_reached == 1`).
///
/// The pack's whole reason is here: none of the four seeded mouths holds the
/// subject's trade, so `s = 0.20 × 0.60 = 0.12` and `t_warm = 0.145 gh` — about one
/// poll. `volunteering` is 4 at the mint and 0 by the next sample.
#[test]
fn the_slow_end_may_never_leave_its_ward() {
    let samples = run_band(WICKMARKET, 24.0, Levers::default());
    trace(&samples, Topic::Craft);
    assert_eq!(
        row(&samples[0].1, Topic::Craft).volunteering,
        4,
        "at the mint all four seeded mouths must be warm, or the slow end measures nothing"
    );
    let day = row(at(&samples, 24.0), Topic::Craft);
    assert!(
        day.expected_crossings > 0.0,
        "the model's expectation reads exactly zero over a game day: the instrument saw \
         no warm window at all, so `< 1.0` below would pass for the wrong reason. The \
         closed-form integrand over four witnesses at t_warm = 0.145 gh must read \
         K × 0.145 × X_W / 24 at any sample cadence"
    );
    assert!(
        day.expected_crossings < 1.0,
        "the model expects {:.3} crossings of the Craft fact out of its ward over a game \
         day; 02_numbers.md §4 computes 0.057. Re-solve VOLUNTEER_HEAT from the boundary \
         walk (§10.2) — never a salience band",
        day.expected_crossings
    );
    assert_eq!(
        day.wards_reached, 1,
        "after a game day the Craft fact is in {}/8 wards' air, not its own alone \
         (carriers by ward {:?}, same-trade holder exits {:.0}). Before pointing at \
         VOLUNTEER_HEAT: a same-trade body (×2.0, warm for 21 gh) standing in Wick who \
         picked it up and walked out is the affinity table doing its job — check the \
         pack's subject trade against the cast (the_cadence_pack_seeds_four_off_affinity_mouths) \
         before re-solving anything",
        day.wards_reached, day.carriers_by_ward, day.holder_exits_same_trade
    );
}

/// **S2.** *"is still in its own ward at nightfall"* — Lamplight, 11 gh after a
/// Dayspring mint.
#[test]
fn the_slow_end_is_still_confined_at_nightfall() {
    let samples = run_band(WICKMARKET, 11.0, Levers::default());
    let night = row(at(&samples, 11.0), Topic::Craft);
    let confined = (-night.expected_crossings).exp();
    println!(
        "[pollen] craft at nightfall: P(still confined) = {confined:.3} \
         (expected crossings {:.3}), wards {}/8, carriers {}, holder exits {:.0}",
        night.expected_crossings, night.wards_reached, night.carriers, night.holder_exits
    );
    assert!(
        confined > 0.5,
        "P(the Craft fact is still confined at nightfall) = {confined:.3}; \
         02_numbers.md §4 computes 0.945"
    );
    assert_eq!(night.wards_reached, 1);
}

/// **A cold scandal out-travels a fresh squabble.** The single assertion that
/// salience is not heat, in wards rather than in arithmetic (the arithmetic half is
/// `knowledge/salience.rs`'s own test).
///
/// The `Bed` fact is **minted 20.85 gh before** the `Craft` one — its own stamp, so
/// its four witnesses stand at heat `λ^20.85 = 0.300` and re-deposit at 0.300 — and
/// its air row is cooled to the same by one backdated sweep. The `Craft` fact is
/// then minted fresh, at 1.0, by the same four mouths at the same place. That is
/// the two rows of `02_numbers.md` §4's table in one world: pickup ∝ `0.300 × 1.00`
/// against `1.00 × 0.12`, and warm life 16.0 gh against 0.145.
///
/// Backdating the **air alone** does not do it, and the run that did so measured a
/// fresh scandal: hot witnesses re-deposit at 1.0 on their first poll, `deposit`
/// takes the maximum, and the Bed row was back at 0.9998 within 0.4 s. So the
/// first sample after the first round tick asserts the row is still at ≤ 0.31 —
/// the guard that would have caught it.
#[test]
fn a_cold_scandal_out_travels_a_fresh_squabble_across_the_city() {
    let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
    // The pack chooses the four mouths and the subject by its own rule; the two
    // facts under test are then planted in its place with their own stamps.
    let (pack, _) = run
        .engine
        .seed_pollen_pack(WICKMARKET, 0.0)
        .expect("the pack plants");
    let (seeded, subject) = {
        let fact = run
            .engine
            .world()
            .knowledge
            .fact_by_id(&FactId::from_raw("pollen.bed"))
            .expect("the pack's bed row");
        (fact.seeded.clone(), fact.subject.clone())
    };
    let mint_point = place_point(run.engine.world(), WICKMARKET);
    for key in pack {
        run.engine.world_mut().knowledge.invalidate(key);
    }

    let minted_at = run.minted_at();
    let cold_since = minted_at - 20.85 / 24.0;
    let world = run.engine.world_mut();
    let bed = knowledge::mint::plant_for_measurement(
        world,
        FactId::from_raw("pollen.bed"),
        Topic::Bed,
        "{subject} — a bed matter at {place} {day}".into(),
        subject.clone(),
        seeded.clone(),
        mint_point,
        knowledge::GarbleMask::ALL,
        Some(cold_since),
    )
    .expect("the cold bed plants");
    let ward = mint_ward(world, bed);
    world.knowledge.rewind_sweep_clock(cold_since);
    assert!(
        knowledge::pollen::sweep(world, minted_at),
        "the backdating sweep changed nothing"
    );
    let craft = knowledge::mint::plant_for_measurement(
        world,
        FactId::from_raw("pollen.craft"),
        Topic::Craft,
        "{subject} — a craft matter at {place} {day}".into(),
        subject,
        seeded.clone(),
        mint_point,
        knowledge::GarbleMask::ALL,
        Some(minted_at),
    )
    .expect("the fresh craft plants");
    assert_eq!(ward, mint_ward(world, craft), "one mouth, one ward");

    let cooled = world
        .knowledge
        .drift(ward, bed)
        .expect("the bed row survived cooling")
        .heat;
    assert!(
        (0.28..0.32).contains(&cooled),
        "the bed row cooled to {cooled}, not the 0.300 the comparison is stated at"
    );
    for who in &seeded {
        let heat = knowledge::holds_key(world, who, bed)
            .expect("a seeded witness holds it")
            .heat(Some(minted_at));
        assert!(
            (0.28..0.32).contains(&heat),
            "witness {who} stands at {heat:.3}, not 0.300: the fact, and not only the air, \
             must be backdated"
        );
    }
    assert_eq!(
        world.knowledge.drift(ward, craft).map(|air| air.heat),
        Some(1.0),
        "the craft row is fresh"
    );

    // The guard: one round tick later — every mouth's first poll — the cold row is
    // still cold, because its witnesses are.
    run.step();
    let after = run
        .engine
        .world()
        .knowledge
        .drift(ward, bed)
        .expect("the bed row")
        .heat;
    assert!(
        after <= 0.31,
        "after the first round tick the Bed row is back at {after:.3}: its witnesses \
         re-deposited hot, so this run would compare a fresh scandal with a fresh squabble"
    );

    let mut tally = CrossingTally::new(&[bed, craft]);
    let samples = run.sample_to(&mut tally, 12.0);
    println!(
        "\n  {:>6}  {:>9} {:>9}   cold Bed (wards carriers) | fresh Craft (wards carriers)",
        "age gh", "bed", "craft"
    );
    for (age, census) in &samples {
        let bed_row = row(census, Topic::Bed);
        let craft_row = row(census, Topic::Craft);
        println!(
            "  {age:>6.2}  {:>4} {:>4} {:>4} {:>4}",
            bed_row.wards_reached, bed_row.carriers, craft_row.wards_reached, craft_row.carriers
        );
    }
    let last = at(&samples, 12.0);
    let cold_bed = row(last, Topic::Bed);
    let fresh_craft = row(last, Topic::Craft);
    println!(
        "[pollen] over 12 gh: a cold Bed at heat {cooled:.3} reached {}/8 wards and {} \
         carriers (warm {}); a fresh off-trade Craft at 1.000 reached {}/8 and {} (warm {})",
        cold_bed.wards_reached,
        cold_bed.carriers,
        cold_bed.volunteering,
        fresh_craft.wards_reached,
        fresh_craft.carriers,
        fresh_craft.volunteering,
    );
    assert!(
        cold_bed.wards_reached > fresh_craft.wards_reached,
        "the cold scandal reached {} wards and the fresh squabble {} — salience is \
         behaving as heat",
        cold_bed.wards_reached,
        fresh_craft.wards_reached
    );
}

// ---------------------------------------------------------------------------
// The two identities
// ---------------------------------------------------------------------------

/// **Reported, never asserted: the same-trade ear** (`02_numbers.md` §4 and §7's
/// `craft_ear` pack). A `Craft` witness of the subject's own trade has
/// `s = 0.20 × 2.00 = 0.40` and a warm life of 21.0 gh, so *one* such witness
/// contributes ≈ 2.1 crossings a game day; bounding that would be bounding the
/// affinity table — "nothing to anyone but a cooper, everything to a cooper". The
/// figures are printed for `m2_measurements.md`. What is asserted is only that the
/// pack is what it says (three mouths of the ear's trade, none of the subject's
/// household, each hearing the fact at 0.40) and that the ear does *something*: a
/// same-trade witness is still warm at nightfall, 11 gh in, where the off-trade
/// one went cold at 0.145.
///
/// The subject is the cadence pack's own (`revenue_worker`, four in the cast), so
/// this scenario differs from the pack's slow end in exactly one thing — who the
/// mouths are.
#[test]
fn the_same_trade_ear_is_reported() {
    let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
    let (pack, _) = run
        .engine
        .seed_pollen_pack(WICKMARKET, 0.0)
        .expect("the pack plants");
    let pack_subject = run
        .engine
        .world()
        .knowledge
        .fact_by_id(&FactId::from_raw("pollen.craft"))
        .expect("the pack's craft row")
        .subject[0]
        .clone();
    for key in pack {
        run.engine.world_mut().knowledge.invalidate(key);
    }
    let mint_point = place_point(run.engine.world(), WICKMARKET);

    let (trade, subject, mouths) = {
        let world = run.engine.world();
        let trade_of = |id: &ActorId| -> Option<String> {
            world
                .characters
                .get(id)
                .and_then(|body| body.lore())
                .and_then(|lore| lore.occupation_id.clone())
        };
        let kin_of = |id: &ActorId| -> BTreeSet<ActorId> {
            world
                .characters
                .get(id)
                .and_then(|body| body.lore())
                .map(|lore| {
                    lore.father
                        .iter()
                        .chain(lore.mother.iter())
                        .chain(lore.children.iter())
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        // Every LLM body, nearest the mint first — `characters_within`'s own order.
        let bodies: Vec<ActorId> = world
            .characters_within(mint_point, 1_200.0, None)
            .into_iter()
            .filter(|id| {
                world
                    .characters
                    .get(id)
                    .is_some_and(|body| body.control() == Control::Llm)
            })
            .collect();
        let mouths_of = |subject: &ActorId, trade: &str| -> Vec<ActorId> {
            let kin = kin_of(subject);
            bodies
                .iter()
                .filter(|id| {
                    *id != subject
                        && trade_of(id).as_deref() == Some(trade)
                        && !kin.contains(*id)
                        && !kin_of(id).contains(subject)
                })
                .take(3)
                .cloned()
                .collect()
        };
        let own = trade_of(&pack_subject).expect("the pack's subject has a trade");
        let mouths = mouths_of(&pack_subject, &own);
        if mouths.len() == 3 {
            (own, pack_subject.clone(), mouths)
        } else {
            // The cast lost a revenue worker: the most populous trade instead,
            // nearest member as subject, in BTreeMap order for a tie.
            let mut by_trade: BTreeMap<String, Vec<ActorId>> = BTreeMap::new();
            for id in &bodies {
                if let Some(trade) = trade_of(id) {
                    by_trade.entry(trade).or_default().push(id.clone());
                }
            }
            let (trade, members) = by_trade
                .iter()
                .max_by_key(|(_, members)| members.len())
                .expect("a trade");
            let subject = members[0].clone();
            let mouths = mouths_of(&subject, trade);
            assert_eq!(mouths.len(), 3, "no trade has four present members");
            (trade.clone(), subject, mouths)
        }
    };
    // Minted where the nearest of the three stands, so the mint ward holds at
    // least one warm mouth and the mint-ward columns (`expect`, `same`) mean
    // something; the trade's other two stand where they stand.
    let (mint_point, stand_in) = {
        let world = run.engine.world();
        let at = world.characters[&mouths[0]].position_m();
        let wards: Vec<String> = mouths
            .iter()
            .map(|who| {
                world
                    .ward_at(world.characters[who].position_m())
                    .map_or("-", PlanningWard::as_str)
                    .to_string()
            })
            .collect();
        (at, wards)
    };
    println!(
        "[pollen] same-trade pack: trade {trade}, subject {subject}, mouths {mouths:?} standing \
         in {stand_in:?}; minted where {} stands",
        mouths[0]
    );

    let minted_at = run.minted_at();
    let key = knowledge::mint::plant_for_measurement(
        run.engine.world_mut(),
        FactId::from_raw("measure.craft.same_trade"),
        Topic::Craft,
        "{subject} — a craft matter at {place} {day}".into(),
        vec![subject.clone()],
        mouths.iter().cloned().collect(),
        mint_point,
        knowledge::GarbleMask::ALL,
        Some(minted_at),
    )
    .expect("the same-trade craft fact plants");
    {
        let world = run.engine.world();
        let fact = world.knowledge.fact(key).expect("the row");
        assert_eq!(fact.craft_ear.as_deref(), Some(trade.as_str()));
        for who in &mouths {
            let s = knowledge::salience::salience(world, fact, who);
            assert!(
                (s - 0.40).abs() < 1e-9,
                "{who} hears the same-trade Craft fact at {s:.3}, not 0.20 × 2.00 = 0.40"
            );
        }
    }

    let mut tally = CrossingTally::new(&[key]);
    let samples = run.sample_to(&mut tally, 24.0);
    println!(
        "\n  {:>6}  {:>5} {:>8} {:>5} {:>8} {:>6} {:>5} {:>9}   same-trade Craft",
        "age gh", "wards", "carriers", "warm", "expect", "exits", "same", "warm-any"
    );
    for (age, census) in &samples {
        let r = row_of(census, "measure.craft.same_trade");
        println!(
            "  {age:>6.2}  {:>5} {:>8} {:>5} {:>8.3} {:>6.0} {:>5.0} {:>9.0}",
            r.wards_reached,
            r.carriers,
            r.volunteering,
            r.expected_crossings,
            r.holder_exits,
            r.holder_exits_same_trade,
            r.warm_same_trade_exits,
        );
    }
    let night = row_of(at(&samples, 11.0), "measure.craft.same_trade");
    let day = row_of(at(&samples, 24.0), "measure.craft.same_trade");
    println!(
        "[pollen] same-trade Craft at nightfall: wards {}/8, carriers {}, warm {}, expect \
         {:.3}, holder exits {:.0} (same trade {:.0}, warm same-trade exits of any ward \
         {:.0}); after a game day: wards {}/8, carriers {}, warm {}, expect {:.3}, holder \
         exits {:.0} (same trade {:.0}, warm same-trade exits of any ward {:.0}) — \
         02_numbers.md §4 predicts ≈ 2.1 warm crossings a day per witness",
        night.wards_reached,
        night.carriers,
        night.volunteering,
        night.expected_crossings,
        night.holder_exits,
        night.holder_exits_same_trade,
        night.warm_same_trade_exits,
        day.wards_reached,
        day.carriers,
        day.volunteering,
        day.expected_crossings,
        day.holder_exits,
        day.holder_exits_same_trade,
        day.warm_same_trade_exits,
    );
    assert!(
        night.volunteering >= 1,
        "no same-trade mouth is still warm at nightfall; the ×2.0 ear is not doing anything \
         (a witness at 0.40 has 21 gh of warm life)"
    );
}

/// **The tally's X is a count of walks, not of the sample rate's own flicker.**
/// A raw boundary count read 3.79 / 8.21 / 12.21 crossings per person per game day
/// at 12 / 48 / 192 samples a day; the debounced count (`CrossingTally`) may only
/// *undercount* at a coarse cadence — a transit shorter than about three sample
/// gaps is not resolved — so it must rise **monotonically** with the cadence and
/// always sit at or under the raw flip count. That is the signature of an
/// instrument that misses short walks, and not of one that counts the mill.
///
/// What it does **not** assert is independence of the cadence: the standing wards
/// are a patchwork (`the_standing_wards_are_a_patchwork`, `cathedral-sim`), so a
/// finer cadence resolves shorter transits between real pieces of real wards. The
/// figure every expectation divides by is therefore stated **at the harness
/// cadence** (two samples a game hour), and `m2_measurements.md` §4 records the
/// whole curve.
#[test]
fn the_crossing_tally_does_not_count_the_mill() {
    let x_at = |samples_per_game_hour: f64| -> (f64, f64, f64) {
        let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
        let (keys, _) = run
            .engine
            .seed_pollen_pack(WICKMARKET, 0.0)
            .expect("the pack plants");
        let mut tally = CrossingTally::new(&keys);
        let samples = run.sample_to_at(&mut tally, 12.0, samples_per_game_hour);
        let last = &samples.last().expect("samples").1;
        let wick = last
            .ward_exit_rate_per_game_day
            .get(&PlanningWard::Wick)
            .copied()
            .unwrap_or(0.0);
        (
            last.crossings_per_person_per_game_day,
            last.crossings_per_person_per_game_day_raw,
            wick,
        )
    };
    let (coarse, coarse_raw, coarse_wick) = x_at(0.5);
    let (shipped, shipped_raw, shipped_wick) = x_at(SAMPLES_PER_GAME_HOUR);
    let (fine, fine_raw, fine_wick) = x_at(8.0);
    println!(
        "[pollen] X over 12 gh from the Dayspring bell, debounced (raw): {coarse:.2} \
         ({coarse_raw:.2}) at 0.5 samples/gh, {shipped:.2} ({shipped_raw:.2}) at \
         {SAMPLES_PER_GAME_HOUR}, {fine:.2} ({fine_raw:.2}) at 8; Wick {coarse_wick:.2} / \
         {shipped_wick:.2} / {fine_wick:.2}"
    );
    assert!(
        coarse <= shipped && shipped <= fine,
        "the debounced X is not monotone in the cadence ({coarse:.2}, {shipped:.2}, \
         {fine:.2}): a count that can only undercount cannot fall as the cadence rises"
    );
    assert!(
        coarse <= coarse_raw && shipped <= shipped_raw && fine <= fine_raw,
        "the debounced X exceeds the raw flip count somewhere"
    );
    assert!(
        fine_raw - fine > 0.5,
        "at eight samples a game hour the debounce removed only {:.2} crossings per person \
         per day; a body milling on its leash across a ward line flips wards at every \
         sample and must be filtered",
        fine_raw - fine
    );
}

/// **The flat-table identity, in its strongest form** (`02_numbers.md` §5).
///
/// `--pollen-flat` runs `SalienceTable::flat()` — every band, ear and multiplier at
/// 1.0. `--pollen-no-salience` **deletes the factor from the expression**:
/// `salience_for` returns a bare 1.0. A multiplication by one that changes a number
/// is a bug in the roll, so the two `PollenCensus` must be equal **field for
/// field** — not close.
///
/// It holds because the self-subject rule lives in `may_carry`, outside the product
/// both levers touch (D51). With the rule *inside* `salience()` this failed by
/// construction: under the lever a subject picked up their own fact and under
/// `flat()` never did.
#[test]
fn pollen_flat_reproduces_the_no_salience_run() {
    let flat = run_band(
        WICKMARKET,
        6.0,
        Levers {
            flat: true,
            no_salience: false,
        },
    );
    let deleted = run_band(
        WICKMARKET,
        6.0,
        Levers {
            flat: false,
            no_salience: true,
        },
    );
    assert_eq!(
        flat.len(),
        deleted.len(),
        "the two runs took a different number of samples"
    );
    for (index, ((flat_age, flat_census), (base_age, base_census))) in
        flat.iter().zip(deleted.iter()).enumerate()
    {
        assert_eq!(flat_age, base_age, "sample {index} is at a different age");
        assert_eq!(
            flat_census, base_census,
            "sample {index} ({flat_age:.2} gh) differs between the flat table and the \
             deleted factor. A multiplication by one that changes a number is a bug in \
             the roll (02_numbers.md §5) — do not widen a tolerance"
        );
    }
    let last = &flat.last().expect("samples").1;
    println!(
        "[pollen] the identity holds field for field over {} samples; the last carries \
         {} holdings, {} air rows and {} store bytes",
        flat.len(),
        last.holdings,
        last.air_entries,
        last.store_bytes,
    );
}

/// **The roll count is invariant under the time scale** (D22's graft). The same six
/// game hours at `seconds_per_day` 120 and 3600 take the same number of samples
/// and leave the same `Drift::stir` on every row both runs share — the coins
/// everybody rolled against are the same coins.
///
/// **Deviation from the plan's table, recorded:** `rolls_per_game_hour` is *not*
/// compared. It is `S × Σ over present bodies |air(their ward)|`, and where bodies
/// stand is walking, which runs in sim-seconds (D22) — so it cannot be
/// clock-invariant and the plan's row asked for something unsatisfiable. Carriers
/// need not agree for the same reason: at 120 s/day nobody finishes a leg and the
/// wave stays in one ward. What must agree is everything the *deadlines* decide,
/// which is what makes a played run's roll count reproducible under the `T` key's
/// 60× and under `--watch-clock`. One more non-walking channel is deliberately not
/// asserted either: the poll gap is salted on the ladder epoch, which advances in
/// sim-seconds, so a person's *phase* on the grid differs by clock too — coins are
/// never skipped by it (a gap is always under a stir window), but which coin a
/// deposit lands before is not clock-invariant.
#[test]
fn the_roll_count_is_invariant_under_the_time_scale() {
    let stirs_of = |seconds_per_day: f64| -> (usize, BTreeMap<(&'static str, u32), u32>) {
        let mut run = city(Levers::default(), seconds_per_day, Vec::new(), 0);
        let (keys, _) = run
            .engine
            .seed_pollen_pack(WICKMARKET, 0.0)
            .expect("the pack plants");
        let mut tally = CrossingTally::new(&keys);
        let samples = run.sample_to(&mut tally, 6.0);
        let mut stirs = BTreeMap::new();
        for ward in PlanningWard::ALL {
            for key in &keys {
                if let Some(drift) = run.engine.world().knowledge.drift(ward, *key) {
                    stirs.insert((ward.as_str(), key.0), drift.stir);
                }
            }
        }
        (samples.len(), stirs)
    };
    let (fast_samples, fast_stirs) = stirs_of(120.0);
    let (shipped_samples, shipped_stirs) = stirs_of(SECONDS_PER_DAY);
    println!(
        "[pollen] six game hours: {fast_samples} samples at 120 s/day, {shipped_samples} at \
         3600; {} rows in the air against {}",
        fast_stirs.len(),
        shipped_stirs.len()
    );
    assert_eq!(
        fast_samples, shipped_samples,
        "the two clocks took a different number of samples over the same game hours, so \
         a deadline is in real seconds somewhere (D22)"
    );
    let mut compared = 0usize;
    for ((ward, key), stir) in &shipped_stirs {
        if let Some(fast) = fast_stirs.get(&(*ward, *key)) {
            assert_eq!(
                fast, stir,
                "row ({ward}, {key}) is on stir {fast} at 120 s/day and {stir} at 3600 — \
                 the stir grid is not in game time (D22, D28)"
            );
            compared += 1;
        }
    }
    assert!(
        compared >= Topic::ALL.len(),
        "only {compared} rows were in both runs' air; the mint ward's nine must be"
    );
    assert!(
        fast_stirs.values().any(|stir| *stir > 0),
        "no row was ever stirred, so this test compared nothing"
    );
}

// ---------------------------------------------------------------------------
// A standing fact, and a household
// ---------------------------------------------------------------------------

/// **A standing fact is answerable and never loud** (D52). `bale.promise` is the
/// hinge of a quest: its three holders must be able to answer forever and the ward
/// must not start saying it.
///
/// With the M2 model as first written they sat at heat 1.0 and deposited on every
/// poll, and 58% of a ward held the hinge inside a day. So `volunteers` is false for
/// `!decays`, and the only way in is a genuine re-heat at `REHEAT_TO`.
///
/// The re-heat here goes through `Knowledge::stir_up`, which is exactly what M4's
/// `knowledge::reheat` calls — M4 owns `reheat`'s body (D62), so this is the
/// mechanism one layer down and not a stand-in for it.
#[test]
fn a_standing_fact_is_answerable_and_never_loud() {
    const ID: &str = "test.standing.promise";
    // Three of the Wickmarket's own mouths, so the holders stand in one ward.
    let pack = format!(
        r#"{{"schema_version": 1, "facts": [
            {{"id": "{ID}", "topic": "talk", "decays": false,
              "said": "{{subject}} promised the bale would be there",
              "subject": ["p0043"], "seeded": ["dv8ll", "p003t", "bn5jk"]}}]}}"#
    );
    let mut run = city(Levers::default(), SECONDS_PER_DAY, vec![pack], 0);
    let key = run
        .engine
        .world()
        .knowledge
        .key_of(&FactId::from_raw(ID))
        .expect("the standing pack seeded");
    let mut tally = CrossingTally::new(&[key]);
    let quiet = run.sample_to(&mut tally, 24.0);
    let standing = row_of(at(&quiet, 24.0), ID);
    println!(
        "[pollen] the standing fact after a game day: wards {}/8, carriers {}, warm {}",
        standing.wards_reached, standing.carriers, standing.volunteering
    );
    assert_eq!(
        standing.wards_reached, 0,
        "a standing fact put a row in {} ward(s)' air unasked",
        standing.wards_reached
    );
    assert_eq!(
        standing.carriers, 3,
        "a standing fact grew from 3 seeded holders to {}; nobody may catch it unasked",
        standing.carriers
    );

    // Now the ask: the re-heat puts it in the ward the holder is standing in, at
    // `REHEAT_TO` and no higher.
    let holder = ActorId::from_raw("dv8ll");
    let asked_in = {
        let world = run.engine.world();
        world
            .ward_at(world.characters[&holder].position_m())
            .expect("a Wickmarket mouth stands in a real ward")
    };
    let game_days = run.clock().game_days(run.now);
    assert!(
        run.engine
            .world_mut()
            .knowledge
            .stir_up(asked_in, key, REHEAT_TO, game_days),
        "the re-heat put no row in the air"
    );
    // The first stir after the ask: `02_numbers.md` §4 computes ≈ 0.12 expected new
    // carriers in a 53-person ward, and §9 asserts **≤ 1**.
    let stir = run.sample_to(&mut tally, 24.0 + 0.5);
    let first_stir = row_of(at(&stir, 24.0 + 0.5), ID);
    println!(
        "[pollen] one stir after the ask: wards {}/8, carriers {} (3 seeded), warm {}",
        first_stir.wards_reached, first_stir.carriers, first_stir.volunteering
    );
    assert!(
        first_stir.carriers <= 3 + 1,
        "one ask gave the word to {} new holders on its first stir; 02_numbers.md §4 \
         computes ≈ 0.12 expected and §9 asserts ≤ 1",
        first_stir.carriers - 3
    );

    // And over the whole life of the row it entered: the word stays in the ward it
    // was asked in — `wards_reached == 1` at every sample, never 2 — and no holder of
    // it may ever pass it on.
    let after = run.sample_to(&mut tally, 48.0);
    let asked = row_of(at(&after, 48.0), ID);
    println!(
        "[pollen] and 24 gh after one ask: wards {}/8, carriers {}, warm {}, by {:?}",
        asked.wards_reached, asked.carriers, asked.volunteering, asked.carriers_by_ward
    );
    for (age, census) in stir.iter().chain(after.iter()) {
        let row = row_of(census, ID);
        assert_eq!(
            row.wards_reached, 1,
            "at {age:.2} gh the asked-for word is in {} wards' air; it belongs in the one \
             it was asked in",
            row.wards_reached
        );
        assert_eq!(
            row.volunteering, 0,
            "at {age:.2} gh {} holder(s) of a standing fact are volunteering it; none may",
            row.volunteering
        );
    }
    // Carriers *walk*, so a holder standing in another ward is the fiction working —
    // what must not happen is the air following them, which the loop above pins.
    println!(
        "[pollen] the ask's whole yield: {} new carriers over the row's life, spread \
         {:?} by where they now stand",
        asked.carriers - 3,
        asked.carriers_by_ward
    );
}

/// **The household hears it after the city.** *"The subject's own housemates hold a
/// fact about them later than the city mean, and the subject never holds it as news
/// at all."*
///
/// **Not** asserted pointwise, and the reason is arithmetic rather than taste.
/// `02_numbers.md` §9 words it as a fraction at every sample; the shipped cast's
/// largest household present in the city is **four** people, so that fraction moves
/// in steps of 0.25 while the city's moves in steps of 0.002, and one kin who picks
/// it up in the first game hour puts 0.25 against the city's 0.06 and fails a sound
/// model. With `HOUSEHOLD_DAMPING = 0.15` that early pickup is a ~4% event per kin
/// per game hour, so a deterministic run meets it about as often as not.
///
/// So the claim is asserted the way the spec words it — **later** — as the household's
/// holding fraction *integrated over the run* against the city's, plus the fraction
/// at the end. That is strictly the area under the two curves, which is what "later"
/// means, and it does not turn on one lucky stir. The pointwise violations are
/// counted and printed rather than hidden — and as it stands they are **0 of 49**
/// with the largest household, whose first pickup is 7.5 gh against the city's 0.5,
/// so nothing here is being papered over: the pointwise form passes today and is not
/// asserted only because a sound model can fail it.
///
/// A **mean first-pickup time** would be the wrong statistic in the other direction:
/// a housemate's warm life at hops 1 is 1.19 gh, so most of them never hold it at all
/// and a mean over the ones who do is a survivor statistic.
///
/// At `--extra-ambient 0` only the **kin** limb of `quiet_among` can fire — all 413
/// baked doors are distinct and `HOUSEHOLD_EPSILON_M` is below their 1.2748 m minimum
/// separation — which is why the subject is chosen for kin, and for the **most** kin.
#[test]
fn the_household_hears_it_after_the_city() {
    let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
    // The subject with the *most* kin present, found in the shipped lore rather than
    // named, so a cast edit moves the test and does not break it — and so the
    // household fraction is as fine-grained as this city can make it.
    let (subject, kin) = {
        let world = run.engine.world();
        let mut best: Option<(ActorId, BTreeSet<ActorId>)> = None;
        for id in &world.roster {
            let Some(lore) = world.characters.get(id).and_then(|body| body.lore()) else {
                continue;
            };
            let kin: BTreeSet<ActorId> = lore
                .father
                .iter()
                .chain(lore.mother.iter())
                .chain(lore.children.iter())
                .filter(|who| world.is_present(who))
                .cloned()
                .collect();
            if best.as_ref().is_none_or(|(_, most)| kin.len() > most.len()) {
                best = Some((id.clone(), kin));
            }
        }
        let found = best.expect("the shipped cast has somebody with kin in the city");
        assert!(
            found.1.len() >= 2,
            "the largest household present is {} — the pack needs two kin",
            found.1.len()
        );
        found
    };

    // Four mouths who are neither the subject nor kin, nearest the subject, so the
    // mint has an earshot that is not the household.
    let (seeded, at_point) = {
        let world = run.engine.world();
        let at = world.characters[&subject].position_m();
        let seeded: BTreeSet<ActorId> = world
            .characters_within(at, 1_200.0, None)
            .into_iter()
            .filter(|id| {
                *id != subject
                    && !kin.contains(id)
                    && world
                        .characters
                        .get(id)
                        .is_some_and(|body| body.control() == Control::Llm)
            })
            .take(4)
            .collect();
        (seeded, at)
    };
    assert_eq!(seeded.len(), 4);
    println!(
        "[pollen] household subject {subject} with {} kin present; seeded {:?}",
        kin.len(),
        seeded
    );
    let minted_at = run.minted_at();
    let key = knowledge::mint::plant_for_measurement(
        run.engine.world_mut(),
        FactId::from_raw("test.household.bed"),
        Topic::Bed,
        "{subject} was seen leaving {place} {day}".into(),
        vec![subject.clone()],
        seeded.clone(),
        at_point,
        knowledge::GarbleMask::ALL,
        Some(minted_at),
    )
    .expect("the household fact plants");
    // Both limbs of the freeze, checked before anything is measured off it.
    let quiet = &run
        .engine
        .world()
        .knowledge
        .fact(key)
        .expect("the row")
        .quiet_among;
    for who in kin.iter().chain(std::iter::once(&subject)) {
        assert!(quiet.contains(who), "{who} is not in quiet_among");
    }

    let mut tally = CrossingTally::new(&[key]);
    let mut household_first: Option<f64> = None;
    let mut city_first: Option<f64> = None;
    let mut shares: Vec<(f64, f64, f64)> = Vec::new();
    // The deterministic complement, with no draw in it: the mean pickup chance
    // (at heat 1.0) of the kin against that of everybody else standing in the
    // same wards as the kin, summed over the samples. The damping enters the
    // chance directly, so this cannot pass or fail by luck.
    let mut kin_chance = 0.0_f64;
    let mut peer_chance = 0.0_f64;
    let mut chance_samples = 0usize;
    let mut next_sample = run.now;
    loop {
        if run.now >= next_sample {
            tally.sample(run.engine.world(), run.clock().game_days(run.now));
            let age = run.game_hours();
            let world = run.engine.world();
            let held = |who: &ActorId| knowledge::holds_key(world, who, key).is_some();
            let household: Vec<&ActorId> = kin.iter().collect();
            let held_household = household.iter().filter(|who| held(who)).count();
            let rest: Vec<&ActorId> = world
                .roster
                .iter()
                .filter(|id| **id != subject && !kin.contains(*id))
                .collect();
            let held_rest = rest.iter().filter(|who| held(who)).count();
            {
                let fact = world.knowledge.fact(key).expect("the row");
                let ward_of = |who: &ActorId| {
                    world
                        .is_present(who)
                        .then(|| world.ward_at(world.characters[who].position_m()))
                        .flatten()
                };
                let kin_wards: BTreeSet<PlanningWard> =
                    household.iter().filter_map(|who| ward_of(who)).collect();
                let kin_present: Vec<&ActorId> = household
                    .iter()
                    .copied()
                    .filter(|who| ward_of(who).is_some())
                    .collect();
                let peers: Vec<&ActorId> = rest
                    .iter()
                    .copied()
                    .filter(|who| {
                        !seeded.contains(*who)
                            && ward_of(who).is_some_and(|ward| kin_wards.contains(&ward))
                    })
                    .collect();
                if !kin_present.is_empty() && !peers.is_empty() {
                    let mean = |people: &[&ActorId]| {
                        people
                            .iter()
                            .map(|who| knowledge::pollen::pickup_chance(world, fact, who, 1.0))
                            .sum::<f64>()
                            / people.len() as f64
                    };
                    kin_chance += mean(&kin_present);
                    peer_chance += mean(&peers);
                    chance_samples += 1;
                }
            }
            let household_share = held_household as f64 / household.len() as f64;
            let city_share = held_rest as f64 / rest.len() as f64;
            assert!(
                !held(&subject),
                "at {age:.2} gh the subject holds a fact about themselves as news"
            );
            if held_household > 0 && household_first.is_none() {
                household_first = Some(age);
            }
            if held_rest > seeded.len() && city_first.is_none() {
                city_first = Some(age);
            }
            shares.push((age, household_share, city_share));
            next_sample = run.now + SECONDS_PER_DAY / (24.0 * SAMPLES_PER_GAME_HOUR);
            if age >= 24.0 {
                break;
            }
        }
        run.step();
    }
    let &(age, household_share, city_share) = shares.last().expect("at least one sample");
    let samples = shares.len() as f64;
    let household_area = shares.iter().map(|(_, share, _)| share).sum::<f64>() / samples;
    let city_area = shares.iter().map(|(_, _, share)| share).sum::<f64>() / samples;
    let pointwise = shares
        .iter()
        .filter(|(_, household, city)| household > city)
        .count();
    println!(
        "[pollen] household {household_share:.3} vs city {city_share:.3} at {age:.2} gh; \
         integrated {household_area:.3} vs {city_area:.3} over {} samples; first household \
         pickup {household_first:?} gh, first non-seeded city pickup {city_first:?} gh \
         (None = never, inside the run); {pointwise} sample(s) where the household led \
         (a 4-body fraction moves in steps of 0.25)",
        shares.len()
    );
    assert!(
        household_area <= city_area,
        "the household held it {household_area:.3} of the time against the city's \
         {city_area:.3}: integrated over the run they heard it EARLIER, not later, so \
         HOUSEHOLD_DAMPING is not damping"
    );
    assert!(
        household_share <= city_share,
        "after a game day {household_share:.3} of the subject's household hold it against \
         {city_share:.3} of the rest of the city"
    );
    println!(
        "[pollen] mean pickup chance at heat 1.0 over {chance_samples} samples: kin {:.4} \
         against the same wards' other standers {:.4}",
        kin_chance / chance_samples.max(1) as f64,
        peer_chance / chance_samples.max(1) as f64
    );
    assert!(
        chance_samples > 0 && kin_chance < peer_chance,
        "the kin's summed pickup chance ({kin_chance:.3}) is not below their ward-mates' \
         ({peer_chance:.3}): the household damping is not in the roll"
    );
}

// ---------------------------------------------------------------------------
// The inputs the band is solved from
// ---------------------------------------------------------------------------

/// `02_numbers.md` §1's **c̄ = 0.1193**, walked over all 519 authored sheets — and
/// the plan's `the_pickup_chance_never_clamps`, as the fold it specifies: the roll's
/// own `pickup_chance` over every authored body × all nine topics at heat 1.0, on
/// the pack's rows.
///
/// A band and not a point, so one new citizen is not a red test — and the whole-cast
/// walk lives here because only this crate can read `lore/characters/**`
/// (`02_numbers.md` §8 files it under `cathedral-sim`, which cannot).
#[test]
fn the_measured_curiosity_mean() {
    let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
    let (keys, _) = run
        .engine
        .seed_pollen_pack(WICKMARKET, 0.0)
        .expect("the pack plants");
    let world = run.engine.world();
    let authored: Vec<&ActorId> = world
        .roster
        .iter()
        .filter(|id| {
            world
                .characters
                .get(*id)
                .is_some_and(|body| body.control() == Control::Llm && body.lore().is_some())
        })
        .collect();
    let mut curiosities: Vec<f64> = authored
        .iter()
        .map(|id| cathedral_sim::curiosity_of(world, id))
        .collect();
    curiosities.sort_by(|left, right| left.partial_cmp(right).expect("no NaN curiosity"));
    let count = curiosities.len();
    assert!(count >= 500, "only {count} authored sheets carry lore");
    let mean = curiosities.iter().sum::<f64>() / count as f64;
    let rank = |fraction: f64| curiosities[((count as f64 - 1.0) * fraction).round() as usize];
    println!(
        "[pollen] c̄ over {count} authored sheets: mean {mean:.4}, median {:.3}, p10 {:.3}, \
         p90 {:.3}, min {:.3}, max {:.3}",
        rank(0.5),
        rank(0.1),
        rank(0.9),
        curiosities[0],
        curiosities[count - 1],
    );
    assert!(
        (0.10..=0.14).contains(&mean),
        "c̄ is {mean:.4}; 02_numbers.md §1 measured 0.1193 and every closed form in §4 is \
         quoted at it"
    );

    // The clamp never binds on the real cast: every body × every topic at heat
    // 1.0, through the roll's own function — no literal stands in for a row.
    let mut worst: (f64, String, Topic) = (0.0, String::new(), Topic::Bed);
    for id in &authored {
        for key in &keys {
            let fact = world.knowledge.fact(*key).expect("the pack's row");
            let chance = knowledge::pollen::pickup_chance(world, fact, id, 1.0);
            assert!(
                chance < 1.0,
                "{id} × {:?} clamps at {chance}; every closed form in 02_numbers.md assumes \
                 the roll stays linear",
                fact.topic
            );
            if chance > worst.0 {
                worst = (chance, id.to_string(), fact.topic);
            }
        }
    }
    println!(
        "[pollen] worst per-roll chance over the whole cast: {:.3} ({} on {:?})",
        worst.0, worst.1, worst.2
    );
    // And for a crowd nobody authored: no generated sheet carries a `curiosity`,
    // so its roll is the derived ceiling times the widest multiplier in the table.
    let table = &*world.salience;
    let widest = Topic::ALL
        .iter()
        .map(|topic| {
            let affinity =
                table
                    .ear_of(*topic)
                    .1
                    .max(table.no_trade())
                    .max(if *topic == Topic::Craft {
                        table.craft_own()
                    } else {
                        0.0
                    });
            table.base(*topic) * affinity
        })
        .fold(0.0, f64::max);
    let crowd_bound = cathedral_sim::attention::CURIOSITY_CEILING * widest;
    println!(
        "[pollen] the generated crowd's bound: CURIOSITY_CEILING {} × the widest \
         base × affinity {widest} = {crowd_bound:.3}",
        cathedral_sim::attention::CURIOSITY_CEILING
    );
    assert!(
        crowd_bound < 1.0,
        "a generated citizen at the derived-curiosity ceiling would clamp on the widest ear"
    );
}

/// Why every cadence figure is stated per ward and never as a city mean: the
/// **standing** wards are 7:1 lopsided.
#[test]
fn the_standing_wards_are_lopsided() {
    let run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
    let census = run.engine.pollen_census(0.0);
    let total: usize = census.ward_population.values().sum();
    let present = run
        .engine
        .world()
        .roster
        .iter()
        .filter(|id| run.engine.world().is_present(id))
        .count();
    let mut counts: Vec<(PlanningWard, usize)> = PlanningWard::ALL
        .into_iter()
        .map(|ward| {
            (
                ward,
                census.ward_population.get(&ward).copied().unwrap_or(0),
            )
        })
        .collect();
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (ward, count) in &counts {
        println!(
            "[pollen] {:>15} {count:>4}  c̄{:.3}",
            ward.as_str(),
            census.ward_mean_curiosity.get(ward).copied().unwrap_or(0.0)
        );
    }
    assert_eq!(
        total, present,
        "the eight standing wards hold {total} bodies and the roster has {present} present"
    );
    let most = counts.first().expect("eight wards").1 as f64;
    let least = counts.last().expect("eight wards").1.max(1) as f64;
    assert!(
        most / least >= 5.0,
        "the standing wards are only {:.1}:1 lopsided; every per-ward figure in \
         02_numbers.md exists because they are ~7:1",
        most / least
    );
}

/// The pack's whole reason: without it neither end's arithmetic is the arithmetic
/// that was solved.
///
/// Four mouths (K = 4, `02_numbers.md` §4), **none** of them holding the subject's
/// trade — so the `Craft` fact is ×0.6 for every one of them — and none of them in
/// the `Bed` ear, so the `Bed` fact is ×1.0 for every one of them.
#[test]
fn the_cadence_pack_seeds_four_off_affinity_mouths() {
    let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), 0);
    let (keys, line) = run
        .engine
        .seed_pollen_pack(WICKMARKET, 0.0)
        .expect("the pack plants");
    println!("{line}");
    assert_eq!(keys.len(), Topic::ALL.len(), "one fact per topic");
    let world = run.engine.world();
    let craft = world
        .knowledge
        .fact_by_id(&FactId::from_raw("pollen.craft"))
        .expect("the craft row");
    assert_eq!(craft.seeded.len(), 4, "K = 4");
    let ear = craft.craft_ear.clone().expect("the subject has a trade");
    let (bed_ear, bed_multiplier) = world.salience.ear_of(Topic::Bed);
    for who in &craft.seeded {
        let trade = world.characters[who]
            .lore()
            .and_then(|lore| lore.occupation_id.clone());
        assert_ne!(
            trade.as_deref(),
            Some(ear.as_str()),
            "{who} holds the Craft fact's own ear ({ear}), so s would be 0.40 and not the \
             0.12 the slow end is solved from"
        );
        assert!(
            !trade
                .as_deref()
                .is_some_and(|trade| bed_ear.iter().any(|in_ear| in_ear == trade)),
            "{who} is in the Bed ear (×{bed_multiplier}), so the fast end's s would not be \
             1.00"
        );
    }
    let subject = craft.subject.first().expect("a subject");
    let subject_ward = world
        .ward_at(world.characters[subject].position_m())
        .expect("the subject stands somewhere real");
    assert_eq!(
        subject_ward,
        mint_ward(world, craft.key),
        "the subject stands in {subject_ward:?} and the fact was minted elsewhere; \
         02_numbers.md §5 needs them together, because that is what makes the flat-table \
         identity exercise `may_carry` on real content"
    );
    // And the subject never holds their own fact.
    assert!(knowledge::holds_key(world, subject, craft.key).is_none());
}

// ---------------------------------------------------------------------------
// The cost guard
// ---------------------------------------------------------------------------

/// **The store is bounded at `--extra-ambient 20000`**, on the real crowd and
/// **saturated** — nine rows in every ward's air, because a run with two authored
/// facts and ≤ 2 rows per ward rolls almost nothing and guards nothing
/// (`02_numbers.md` §6).
///
/// `#[ignore]`d: startup alone is ~5.5 s at that count, and even at the coarse step
/// the pump costs about **3.0 s a poll** at 20,520 bodies with the air saturated
/// (`m2_measurements.md` §3: 300 polls in 905 s) — the plan's own 24 game hours
/// would be an hour. Run it by hand, as `m2_measurements.md` records:
///
/// ```sh
/// cargo test -p cathedral-backends --release --test pollen_cadence \
///     the_store_stays_bounded_at_twenty_thousand -- --ignored --nocapture
/// ```
///
/// **The window is six game hours, not the plan's twenty-four**, and nothing this
/// test asserts turns on the difference: the air is saturated from the first instant
/// by `debug_seed_air`, so six game hours is 36 polls per body and ~2.4 million rolls
/// — every holdings cap is filled long before the end, and the footprint bound is a
/// bound on the *store* rather than on the length of the run. Twenty-four game hours
/// costs four times as long to say the same thing.
#[test]
#[ignore = "20,000 generated citizens; run it by hand with --ignored"]
fn the_store_stays_bounded_at_twenty_thousand() {
    const CROWD: usize = 20_000;
    const WINDOW_GAME_HOURS: f64 = 6.0;
    let started = std::time::Instant::now();
    let mut run = city(Levers::default(), SECONDS_PER_DAY, Vec::new(), CROWD);
    println!(
        "[pollen] {} bodies seeded in {:.1} s",
        run.engine.world().roster.len(),
        started.elapsed().as_secs_f64()
    );
    let (keys, line) = run
        .engine
        .seed_pollen_pack(WICKMARKET, 0.0)
        .expect("the pack plants");
    println!("{line}");
    let game_days = run.minted_at();
    for key in &keys {
        for ward in PlanningWard::ALL {
            knowledge::pollen::debug_seed_air(run.engine.world_mut(), ward, *key, Some(game_days));
        }
    }
    // And `live` to its cap, so the O(live) deposit walk runs at the length it is
    // bounded at (`02_numbers.md` §6's `seeded_by_actor` decision reads this).
    let mint_point = place_point(run.engine.world(), WICKMARKET);
    let filled = knowledge::mint::fill_live_for_measurement(
        run.engine.world_mut(),
        mint_point,
        Some(game_days),
    );
    println!(
        "[pollen] saturated: {} rows in every ward's air, {filled} filler facts to a live store of {}",
        keys.len(),
        run.engine.world().knowledge.len()
    );
    // The coarse step: crossings are not what this measures, and 9,000 polls at the
    // measured ~3.0 s each would be seven and a half hours.
    let mut polls = 0usize;
    while run.game_hours() < WINDOW_GAME_HOURS {
        run.now += 3.0;
        let now = run.now;
        run.engine.poll(now, Vec::new());
        polls += 1;
    }
    println!(
        "[pollen] {polls} polls over {WINDOW_GAME_HOURS} game hours in {:.1} s wall",
        started.elapsed().as_secs_f64()
    );
    let mut census = run.engine.pollen_census(run.now);
    census.fill(&CrossingTally::new(&keys).snapshot());
    for line in census.topic_lines() {
        println!("{line}");
    }
    let world = run.engine.world();
    let bytes = world.knowledge.footprint_bytes();
    println!(
        "[pollen] footprint at {CROWD} extra ambient ({} bodies): {bytes} bytes ({:.1} MB); \
         peak RSS of this process {}",
        world.roster.len(),
        bytes as f64 / (1024.0 * 1024.0),
        peak_rss_kb().map_or("unknown".to_string(), |kb| format!(
            "{:.1} MB",
            kb as f64 / 1024.0
        ))
    );
    assert!(
        bytes <= 32 * 1024 * 1024,
        "the store is {bytes} bytes at {CROWD} extra ambient, over the 32 MB bound"
    );
    for id in &world.roster {
        assert!(
            world.knowledge.holdings_len(id) <= HOLDINGS_MAX,
            "{id} holds {} facts, over HOLDINGS_MAX",
            world.knowledge.holdings_len(id)
        );
    }
    assert!(
        census.rolls_per_game_hour > 0.0,
        "a guard that rolled nothing guards nothing: the saturation is what makes this \
         measure a real load"
    );
}
