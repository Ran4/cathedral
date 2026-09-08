//! M2a5 carried-knowledge component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, FactId,
    IdleCognitionMode, NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv,
    RequestId, ShelterMap, SoundCatalog, TtsBackendKind, Vec3, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::knowledge_checkpoint::EngineKnowledgeDtoV1,
    knowledge::{
        self, FactSource, GarbleMask, Telling, Topic, checkpoint::KnowledgeCheckpointContext,
    },
    timeline::LogicalTime,
};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
struct Unavailable;
impl Cognition for Unavailable {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Mode {
    Authored,
    Populated,
}
#[derive(Parser)]
struct Args {
    #[arg(long, value_enum)]
    mode: Mode,
    #[arg(long, default_value_t = 100)]
    samples: usize,
    #[arg(long)]
    output: PathBuf,
}
#[derive(Default, Serialize)]
struct Phases {
    preflight_us: Vec<f64>,
    export_us: Vec<f64>,
    encode_us: Vec<f64>,
    decode_validate_us: Vec<f64>,
    candidate_validate_us: Vec<f64>,
    drop_us: Vec<f64>,
}
fn time<T>(out: &mut Vec<f64>, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    out.push(start.elapsed().as_secs_f64() * 1e6);
    result
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !(1..=1000).contains(&args.samples) {
        return Err("samples must be 1..=1000".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = root.join("assets");
    let read = |name: &str| fs::read_to_string(assets.join(name));
    let mut seed = load_world_seed(&assets, &root.join("lore"))?;
    let nav = NavData::from_parts(
        &read("world/navigation.json")?,
        &fs::read(assets.join("world/navigation.bin"))?,
    )?;
    let placement = if matches!(args.mode, Mode::Populated) {
        let occupied: Vec<_> = seed.characters.iter().map(|c| c.position_m).collect();
        let crowd = cathedral_sim::generate_ambient(&nav, 2000, 0, &occupied, &[])?;
        let p = json!({"requested":crowd.placement.requested,"placed":crowd.placement.placed,"unplaced":crowd.placement.unplaced});
        if crowd.placement.placed != 2000 {
            return Err("+2,000 placement incomplete".into());
        }
        seed = seed.with_extra_ambient(crowd.sheets)?;
        p
    } else {
        json!({"requested":0,"placed":0,"unplaced":0})
    };
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            tts_selected: TtsBackendKind::Off,
            clock: WorldClock::new(60.0, Office::Dayspring, 2, 0.05),
            nav: Some(Arc::new(nav)),
            shelters: Arc::new(ShelterMap::from_json_str(&read("world/shelters.json")?)?),
            ..Default::default()
        },
        &seed,
        AreaMap::from_json_str(&read("world/areas.json")?)?,
        SoundCatalog::from_toml_str(&read("sounds/catalog.toml")?)?,
        PromptEnv::new(
            &read("prompts/turn.j2")?,
            &read("prompts/night.j2")?,
            &read("prompts/strings.toml")?,
        )?,
        Box::new(Unavailable),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(-21.0, cathedral_sim::WALK_Y, 252.0), 0.0),
        0,
        0.0,
    )?;
    engine.poll(0.0, Vec::new());
    // Deliberate measurement setup through the real bounded knowledge writers.
    // The installed catalog/context and actual production population stay intact.
    let w = engine.world_mut();
    w.knowledge = knowledge::Knowledge::default();
    let days = w.current_time.unwrap().game_days();
    let player = ActorId::from_raw("player");
    let departed = ActorId::from_raw("diagnostic_departed");
    let mint = |w: &mut cathedral_sim::World, n: usize, source| {
        knowledge::mint::mint(
            w,
            FactId::from_raw(format!("checkpoint.probe.{n}")),
            Topic::Talk,
            format!("A quiet word {n} was heard at {{place}} {{day}}."),
            vec![],
            Vec3::new(100000.0, 0.0, 100000.0),
            GarbleMask {
                subject: false,
                place: true,
                day: true,
            },
            true,
            source,
            Some(days),
        )
        .unwrap()
    };
    let old = mint(w, 99, FactSource::authored());
    knowledge::learn(
        w,
        &player,
        old,
        Telling {
            hops: 2,
            from: Some(departed.clone()),
            heat: 1.0,
            view: Default::default(),
        },
        Some(days),
    );
    w.knowledge.note_seated(&departed, vec![old]);
    w.knowledge.note_occasion(
        &departed,
        Some(player.clone()),
        Some(departed.clone()),
        days,
    );
    w.knowledge.offer_occasion(&departed);
    w.knowledge.invalidate(old);
    let people: Vec<_> = w.characters.keys().cloned().collect();
    for n in 0..6 {
        let key = mint(w, n, FactSource::authored());
        for who in &people {
            knowledge::learn(
                w,
                who,
                key,
                Telling {
                    hops: 2,
                    from: Some(departed.clone()),
                    heat: 1.0,
                    view: Default::default(),
                },
                Some(days),
            );
        }
        for ward in cathedral_sim::lore::PlanningWard::ALL {
            w.knowledge.deposit(ward, key, 2, 0.8, &departed, days);
        }
    }
    engine.poll(0.01, Vec::new());
    let boundary = LogicalTime::new(0.01).unwrap();
    let w = engine.world();
    let context = KnowledgeCheckpointContext::from_world(w, boundary);
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_knowledge_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_knowledge_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        let witnesses = dto.value().history_counts();
        assert_eq!(counts.knowledge.holdings, 6 * counts.knowledge.characters);
        assert_eq!(counts.knowledge.holding_actors, counts.knowledge.characters);
        assert!(counts.journal_cached && counts.journal_entries > 0 && counts.ward_heat_rows == 8);
        assert!(
            witnesses.historical_receipts > 0
                && witnesses.historical_seated_keys > 0
                && witnesses.offered_occasions > 0
        );
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineKnowledgeDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        assert_eq!(candidate.value().history_counts(), witnesses);
        let current = json!({"scenario":"carried_news_with_historical_receipts_and_caches","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
        if let Some(prior) = &metadata {
            assert_eq!(prior, &current);
        } else {
            metadata = Some(current);
        }
        time(&mut phases.drop_us, || {
            drop(candidate);
            drop(bytes);
        });
        assert_eq!(budget.retained_bytes(), 0);
    }
    let mut output = metadata.unwrap();
    output["mode"] = json!(match args.mode {
        Mode::Authored => "authored",
        Mode::Populated => "populated",
    });
    output["samples"] = json!(args.samples);
    output["placement"] = placement;
    for (k, v) in serde_json::to_value(phases)?.as_object().unwrap() {
        output[k] = v.clone();
    }
    fs::write(args.output, serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}
