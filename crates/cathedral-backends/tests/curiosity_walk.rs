//! What it costs to walk past everybody
//! (`features/implemented/gate_idle_cognition_on_novelty.md` §2).
//!
//! The curiosity gate's whole claim is a number, and the number is only true of
//! the *shipped cast*: some 500 authored people, thinly spread over 1.2 × 1.0 km, each
//! with an age, a trade and a station that the derivation reads. Nothing in
//! `cathedral-sim`'s own tests can check it, because nothing there has a city.
//! This crate owns the filesystem, so this is where the city is.
//!
//! The walk is a serpentine across the whole map at a walking pace — literally
//! *"walking past everyone"*, which is how the target was stated. It counts two
//! things and divides them:
//!
//! - **passed**: an LLM actor who was inside the 32 m stage at some poll;
//! - **spoke first**: one who produced at least one `PromptExchange` over the
//!   whole walk, unprompted — nobody says a word to them, no bell rings, and the
//!   player never opens his mouth.
//!
//! Run it loud to see the table the feature doc records:
//!
//! ```sh
//! cargo test -p cathedral-backends --test curiosity_walk -- --nocapture
//! ```

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Control, CuriosityConfig, Engine,
    EngineCommand, EngineConfig, EngineMessage, FakeCognition, IdleCognitionMode, NullSight,
    NullTranscription, NullTts, PromptEnv, SoundCatalog, SpatialActorUpdate, StageConfig,
    TtsBackendKind, Vec3, ids::RequestId,
};

/// The stage radius (`attention::DEFAULT_STAGE_RADIUS_M`), and therefore the
/// widest circle in which the gate could possibly have spent money on somebody.
const STAGE_RADIUS_M: f64 = 32.0;

/// **Walked past** means *came within earshot* — `HEARING_RADIUS_M`, the radius a
/// word would have carried across.
///
/// This is the design's own definition and not a convenience: a person you walk
/// past gets exactly the two pieces of news the target is stated in terms of —
/// you enter their street (the 32 m stage), and then you enter their earshot (the
/// 20 m hash) — so their chance of thinking about you at all is `1 − (1 − c)²`.
/// Somebody who was only ever a figure across a 30 m square gets one of those two
/// and was not walked past; counting him would measure a different thing.
const PASSED_RADIUS_M: f64 = 20.0;

/// Lane spacing for the serpentine, and the one number here that had to be
/// *chosen* rather than read off the game. It is **wider than the stage is
/// across** (`2 × STAGE_RADIUS_M` = 64 m), and that is the whole point.
///
/// Narrower, and adjacent lanes both graze the same person. Each pass is a
/// separate *meeting* — the lanes are twelve minutes apart and the novelty memory
/// lapses in one — so the same man is walked past two or three times and rolls
/// two or three times as often. A 30 m serpentine measured 28.6%, which is
/// `1 − (1 − c)³`: an honest number about a walk nobody would ever take.
///
/// Wider than the stage, every person in the cast is met exactly once. The 43% of
/// the city that this leaves outside `PASSED_RADIUS_M` is not walked past at all
/// and is simply not counted — which the spread below survives, because a lane
/// that misses a tavern misses it in the numerator and the denominator alike.
const LANE_SPACING_M: f64 = 70.0;

/// A walking pace, and the poll step that samples it. 1.4 m per poll is far finer
/// than any distance threshold in the gate, so no arrival is missed between two
/// samples.
const WALK_SPEED_MPS: f64 = 1.4;
const POLL_STEP_SECONDS: f64 = 1.0;

/// The turn the cast actually gets in the running game: `npc_turn_delay_seconds`
/// (1.0 s) plus ~2.2 s of provider latency. The fake provider answers instantly,
/// so the delay is where the real spacing has to be put — otherwise the walk
/// measures a throughput the game does not have, and a crowd that could never be
/// served in time would read as a crowd that chose not to speak.
const TURN_SPACING_SECONDS: f64 = 3.2;

/// The city, from `lore/characters/**/*.json`.
const CITY_MIN: Vec3 = Vec3::new(-520.0, 0.91, -680.0);
const CITY_MAX: Vec3 = Vec3::new(505.0, 0.91, 495.0);

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

/// One person walked past, once. The unit of the measurement is the **encounter**
/// and not the person, because that is how the target is stated: *walking past
/// someone, a one-in-five chance they think about you*. Meeting the same
/// fishmonger on two different days is two chances, and averaging them into one
/// man would answer a question nobody asked.
struct Passer {
    occupation: String,
    curiosity: f64,
    spoke_first: bool,
}

/// The whole cast walked past, once each per pass.
fn walk(curiosity: CuriosityConfig) -> Vec<Passer> {
    const {
        assert!(
            LANE_SPACING_M > 2.0 * STAGE_RADIUS_M,
            "adjacent lanes would graze the same person twice; see LANE_SPACING_M"
        );
    }
    let root = root();
    let assets = root.join("assets");
    let seed = load_world_seed(&assets, &root.join("lore")).expect("the shipped world data loads");
    let areas = AreaMap::from_json_str(&read(&assets.join("world/areas.json"))).expect("areas");
    let catalog =
        SoundCatalog::from_toml_str(&read(&assets.join("sounds/catalog.toml"))).expect("sounds");
    let prompts = PromptEnv::new(
        &read(&assets.join("prompts/turn.j2")),
        &read(&assets.join("prompts/strings.toml")),
    )
    .expect("prompts");

    // The authored trade, read off the seed rather than guessed from a spawn
    // position.
    let mut trade: BTreeMap<ActorId, String> = BTreeMap::new();
    for character in &seed.characters {
        if character.control != Control::Llm {
            continue;
        }
        let Some(lore) = character.lore.as_ref() else {
            continue;
        };
        trade.insert(
            character.id.clone(),
            lore.occupation_id
                .clone()
                .unwrap_or_else(|| "no_fixed_trade".to_string()),
        );
    }

    let cognition = SharedCognition::default();
    let start = Vec3::new(CITY_MIN.x, CITY_MIN.y, CITY_MIN.z);
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            // The provider is instant, so this is the entire turn spacing.
            turn_delay_seconds: TURN_SPACING_SECONDS,
            tts_selected: TtsBackendKind::Off,
            idle_mode: IdleCognitionMode::Stage,
            stage: StageConfig::default(),
            idle_requires_news: true,
            idle_curiosity: curiosity,
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
        (start, 0.0),
        0,
        0.0,
    )
    .expect("the shipped seed has a player");

    let player = ActorId::from_raw("player");
    let mut encounters: Vec<Passer> = Vec::new();
    let mut now = 0.0;
    let mut spatial_seq = 0;

    // Two serpentines, the second at right angles to the first. One pass alone
    // meets only the 57% of the city that its lanes happen to run within earshot
    // of, and that sample is not random: the cast is clustered by trade, so a
    // lane that misses the tavern misses every tavern worker. Crossing the city
    // the other way gives a second, independent set of ~280 encounters and a
    // sample that is not a map of one set of lanes.
    for pass in [serpentine(Axis::EastWest), serpentine(Axis::NorthSouth)] {
        let mut passed: BTreeSet<ActorId> = BTreeSet::new();
        let mut spoke: BTreeSet<ActorId> = BTreeSet::new();

        for position in pass {
            spatial_seq += 1;
            let mut commands: Vec<EngineCommand> = cognition
                .0
                .borrow_mut()
                .drain_completions()
                .into_iter()
                .map(EngineCommand::LlmCompletion)
                .collect();
            commands.push(EngineCommand::SpatialUpdate {
                spatial_seq,
                updates: vec![SpatialActorUpdate::new(player.clone(), position, None)],
            });

            for message in engine.poll(now, commands) {
                if let EngineMessage::PromptExchange { actor_id, .. } = message {
                    spoke.insert(actor_id);
                }
            }

            // Whom did we walk past? Near enough that a word would have carried
            // — which is also near enough that both of the design's two news
            // events have happened to them. See `PASSED_RADIUS_M`.
            for actor_id in
                engine
                    .world()
                    .characters_within(position, PASSED_RADIUS_M, Some(&player))
            {
                passed.insert(actor_id);
            }
            now += POLL_STEP_SECONDS;
        }

        for actor_id in &passed {
            let Some(occupation) = trade.get(actor_id) else {
                continue;
            };
            encounters.push(Passer {
                occupation: occupation.clone(),
                curiosity: cathedral_sim::curiosity_of(engine.world(), actor_id),
                spoke_first: spoke.contains(actor_id),
            });
        }
    }
    encounters
}

#[derive(Clone, Copy)]
enum Axis {
    EastWest,
    NorthSouth,
}

/// A boustrophedon across the city: down one lane, up the next.
fn serpentine(axis: Axis) -> Vec<Vec3> {
    let step_m = WALK_SPEED_MPS * POLL_STEP_SECONDS;
    let (along_min, along_max, across_min, across_max) = match axis {
        Axis::EastWest => (CITY_MIN.x, CITY_MAX.x, CITY_MIN.z, CITY_MAX.z),
        Axis::NorthSouth => (CITY_MIN.z, CITY_MAX.z, CITY_MIN.x, CITY_MAX.x),
    };
    let mut path = Vec::new();
    let mut across = across_min;
    let mut forward = true;
    while across <= across_max {
        let mut lane = Vec::new();
        let mut along = along_min;
        while along <= along_max {
            lane.push(along);
            along += step_m;
        }
        if !forward {
            lane.reverse();
        }
        path.extend(lane.into_iter().map(|along| match axis {
            Axis::EastWest => Vec3::new(along, CITY_MIN.y, across),
            Axis::NorthSouth => Vec3::new(across, CITY_MIN.y, along),
        }));
        across += LANE_SPACING_M;
        forward = !forward;
    }
    path
}

/// The people whose living *is* speaking first.
const FORWARD: &[&str] = &[
    "no_fixed_trade",
    "market_seller",
    "food_provisioner",
    "fish_trader",
    "entertainer",
    "tavern_worker",
    "pilgrim",
    "scavenger",
];

/// The office, the cloister and the ledger.
const RESERVED: &[&str] = &[
    "watchman_and_keeper",
    "militia_and_soldier",
    "bailiff_and_gaoler",
    "civic_officer",
    "candor_cleric",
    "church_attendant",
    "scribe_and_clerk",
    "merchant",
];

/// The acceptance number: **about one person in five** that you walk past thinks
/// about you at all — and it is not a flat fifth painted over the cast.
///
/// The other four in five are not a rounding error; they are the feature. Before
/// curiosity, every single one of these 500 people spent a prompt on the stranger
/// in their street — affordably, since the novelty gate, and still absurdly.
///
/// Who the fifth *is* is the other half. It is supposed to be a fact about the
/// person: the beggar and the hawker live by accosting strangers, and the
/// watchman, the clerk and the canon do not.
///
/// Note what the walk does *not* do: it never speaks, never rings a bell, never
/// offers anything, and it never stands still. Every turn counted here was
/// somebody's own idea about a stranger walking by.
#[test]
fn one_person_in_five_speaks_first_when_you_walk_past_the_whole_city() {
    let cast = walk(CuriosityConfig {
        enabled: true,
        scale: 1.0,
    });
    let people: Vec<&Passer> = cast.iter().collect();
    let passed = people.len();
    let spoke = people.iter().filter(|entry| entry.spoke_first).count();
    let measured = spoke as f64 / passed as f64 * 100.0;
    let mean_curiosity: f64 =
        people.iter().map(|entry| entry.curiosity).sum::<f64>() / passed as f64;

    let rate_of = |trades: &[&str]| -> f64 {
        let group: Vec<&&Passer> = people
            .iter()
            .filter(|entry| trades.contains(&entry.occupation.as_str()))
            .collect();
        assert!(!group.is_empty(), "nobody in {trades:?} was walked past");
        group.iter().filter(|entry| entry.spoke_first).count() as f64 / group.len() as f64 * 100.0
    };
    let forward = rate_of(FORWARD);
    let reserved = rate_of(RESERVED);

    // Grouped by trade, which is where the texture is meant to show.
    let mut by_trade: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for entry in &people {
        let slot = by_trade.entry(entry.occupation.as_str()).or_default();
        slot.0 += 1;
        slot.1 += usize::from(entry.spoke_first);
    }
    let mut ranked: Vec<(&str, usize, usize)> = by_trade
        .into_iter()
        .filter(|(_, (total, _))| *total >= 6)
        .map(|(trade, (total, spoke))| (trade, total, spoke))
        .collect();
    ranked.sort_by(|left, right| {
        let left_rate = left.2 as f64 / left.1 as f64;
        let right_rate = right.2 as f64 / right.1 as f64;
        right_rate
            .partial_cmp(&left_rate)
            .unwrap()
            .then(left.0.cmp(right.0))
    });

    eprintln!("\nwalked past {passed} of the cast; {spoke} spoke first — {measured:.1}%");
    eprintln!("mean derived curiosity {mean_curiosity:.3}");
    eprintln!(
        "forward trades {forward:.0}% · city {measured:.0}% · reserved trades {reserved:.0}%\n"
    );
    eprintln!("  {:>5}  {:>4}  trade (n ≥ 6)", "rate", "n");
    for (trade, total, spoke) in &ranked {
        eprintln!(
            "  {:>4.0}%  {total:>4}  {trade}",
            *spoke as f64 / *total as f64 * 100.0
        );
    }

    assert!(
        (17.0..=23.0).contains(&measured),
        "the city speaks first {measured:.1}% of the time; the design asks for ~20% \
         (retune attention::CURIOSITY_BASE)"
    );
    assert!(
        forward > measured + 8.0,
        "the hawkers and the beggars are no more forward than anybody else \
         ({forward:.1}% vs {measured:.1}%)"
    );
    assert!(
        reserved < measured - 5.0,
        "the watch and the chapter are no more reserved than anybody else \
         ({reserved:.1}% vs {measured:.1}%)"
    );
}

/// The cost the whole thing is for. The same walk, past the same people, with
/// the gate off: everybody thinks about you.
#[test]
fn without_curiosity_the_whole_city_thinks_about_you() {
    let cast = walk(CuriosityConfig::default());
    let people: Vec<&Passer> = cast.iter().collect();
    let spoke = people.iter().filter(|entry| entry.spoke_first).count();
    let measured = spoke as f64 / people.len() as f64 * 100.0;
    eprintln!(
        "\nwithout curiosity: {spoke} of {} spoke first — {measured:.1}%",
        people.len()
    );
    // Not 100%: the walk is finite, one turn is in flight at a time, and a
    // stranger who leaves the stage before his turn comes round never gets one.
    // That backlog is the *un*-gated bill — and it is the reason the number below
    // is a floor and not a ceiling.
    assert!(
        measured > 70.0,
        "the ungated city spent only {measured:.1}% of its prompts on the player's neighbours; \
         the comparison this test exists to make is broken"
    );
}
