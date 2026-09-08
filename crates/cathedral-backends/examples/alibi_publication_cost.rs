//! CPU cost of the production host's publication allocation accounting.
//! Run separately from builds/other benchmarks. This does not measure Bevy or rendering.

use std::{fs, hint::black_box, path::PathBuf, sync::Arc, time::Instant};

use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, EngineMessage,
    IdleCognitionMode, NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv,
    RequestId, ShelterMap, SoundCatalog, TtsBackendKind, Vec3, WorldClock,
};
use clap::Parser;
use serde_json::json;

// Include the actual pure accounting implementation used by LocalEngine. A
// copied estimator could silently stop measuring the production traversal.
#[path = "../../../src/smart_actors/publication_bytes.rs"]
mod publication_bytes;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 0)]
    extra: usize,
    #[arg(long, default_value_t = 5000)]
    samples: usize,
    #[arg(long)]
    output: PathBuf,
}

struct UnavailableCognition;

impl Cognition for UnavailableCognition {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.extra > 2000 || !(1..=50_000).contains(&args.samples) {
        return Err("require <=2000 extra residents and 1..=50000 samples".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = root.join("assets");
    let read = |name: &str| fs::read_to_string(assets.join(name));
    let mut seed = load_world_seed(&assets, &root.join("lore"))?;
    let nav = NavData::from_parts(
        &read("world/navigation.json")?,
        &fs::read(assets.join("world/navigation.bin"))?,
    )?;
    if args.extra > 0 {
        let occupied: Vec<_> = seed.characters.iter().map(|a| a.position_m).collect();
        let generated = cathedral_sim::generate_ambient(&nav, args.extra, 0, &occupied, &[])?;
        assert_eq!(generated.placement.placed, args.extra);
        seed = seed.with_extra_ambient(generated.sheets)?;
    }
    let config = EngineConfig {
        idle_mode: IdleCognitionMode::Stage,
        clock: WorldClock::new(3600.0, Office::Dayspring, 2, 0.05),
        nav: Some(Arc::new(nav)),
        shelters: Arc::new(ShelterMap::from_json_str(&read("world/shelters.json")?)?),
        ..EngineConfig::default()
    };
    let mut engine = Engine::new(
        config,
        &seed,
        AreaMap::from_json_str(&read("world/areas.json")?)?,
        SoundCatalog::from_toml_str(&read("sounds/catalog.toml")?)?,
        PromptEnv::new(
            &read("prompts/turn.j2")?,
            &read("prompts/night.j2")?,
            &read("prompts/strings.toml")?,
        )?,
        Box::new(UnavailableCognition),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(-21.0, cathedral_sim::WALK_Y, 252.0), 0.0),
        0,
        0.0,
    )?;
    for step in 1..=100 {
        let _ = engine.poll(step as f64 * 0.05, Vec::new());
    }
    let snapshot = engine.snapshot();
    let actor_count = snapshot.actors.len();
    let encoded_bytes = serde_json::to_vec(&snapshot)?.len();
    let message = EngineMessage::Snapshot(snapshot);
    let allocation_bytes = publication_bytes::message(&message);
    for _ in 0..1000 {
        black_box(publication_bytes::message(black_box(&message)));
    }
    let mut samples_us = Vec::with_capacity(args.samples);
    for _ in 0..args.samples {
        let started = Instant::now();
        let accounted = black_box(publication_bytes::message(black_box(&message)));
        samples_us.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        assert_eq!(accounted, allocation_bytes);
    }
    let result = json!({
        "schema": 1,
        "scope": "Hot repeated production allocation-accounting traversal of one real snapshot; excludes snapshot construction, encoding, queue atomics, consumer work and renderer.",
        "extra_residents": args.extra,
        "actor_count": actor_count,
        "logical_elapsed_seconds": 5.0,
        "encoded_snapshot_bytes": encoded_bytes,
        "charged_message_bytes": allocation_bytes,
        "warmup_traversals": 1000,
        "samples_us": samples_us,
    });
    fs::write(args.output, serde_json::to_vec(&result)?)?;
    Ok(())
}
