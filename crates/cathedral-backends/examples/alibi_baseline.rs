//! M0's bounded physical-time baseline. No production clock policy is changed.
//! Run through the associated evidence/run_baseline.py for paired raw records.
use std::{cell::RefCell, fs, path::PathBuf, rc::Rc, sync::Arc, time::Instant};

use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineCommand, EngineConfig,
    EngineMessage, FakeCognition, IdleCognitionMode, NavData, NullSight, NullTranscription,
    NullTts, Office, PromptEnv, RequestId, ShelterMap, SoundCatalog, TtsBackendKind, Vec3,
    WorldClock,
};
use clap::Parser;
use serde_json::json;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 0)]
    extra: usize,
    #[arg(long, default_value_t = 600)]
    polls: usize,
    #[arg(long, default_value_t = 100)]
    warmup: usize,
    /// Empty corner, with no player speech. Otherwise use the Wickmarket.
    #[arg(long)]
    idle: bool,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Default)]
struct Brain {
    fake: FakeCognition,
    calls: usize,
    prompt_bytes_max: usize,
}

#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Brain>>);

impl Cognition for Shared {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        let mut brain = self.0.borrow_mut();
        brain.calls += 1;
        brain.prompt_bytes_max = brain.prompt_bytes_max.max(prompt.len());
        brain.fake.request(prompt)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.polls == 0 || args.polls > 72_000 || args.warmup > 1_200 || args.extra > 20_000 {
        return Err("require 1..=72000 polls, <=1200 warmup and <=20000 extra residents".into());
    }
    let started = Instant::now();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = root.join("assets");
    let read = |path: &str| fs::read_to_string(assets.join(path));
    let mut seed = load_world_seed(&assets, &root.join("lore"))?;
    let nav = NavData::from_parts(
        &read("world/navigation.json")?,
        &fs::read(assets.join("world/navigation.bin"))?,
    )?;
    let placement = if args.extra > 0 {
        let occupied: Vec<_> = seed
            .characters
            .iter()
            .map(|actor| actor.position_m)
            .collect();
        let crowd = cathedral_sim::generate_ambient(&nav, args.extra, 0, &occupied, &[])?;
        let placement = json!({
            "requested": crowd.placement.requested, "placed": crowd.placement.placed,
            "unplaced": crowd.placement.unplaced, "housed": crowd.placement.housed,
            "hardship": crowd.placement.hardship, "workers": crowd.placement.workers,
            "door_cap": crowd.placement.door_cap,
        });
        seed = seed.with_extra_ambient(crowd.sheets)?;
        placement
    } else {
        json!({"requested": 0, "placed": 0, "unplaced": 0})
    };
    let cognition = Shared::default();
    let position = if args.idle {
        Vec3::new(-360.0, cathedral_sim::WALK_Y, -470.0)
    } else {
        Vec3::new(-21.0, cathedral_sim::WALK_Y, 252.0)
    };
    let mut config = EngineConfig {
        fake_mode: true,
        idle_mode: IdleCognitionMode::Stage,
        idle_requires_news: true,
        tts_selected: TtsBackendKind::Off,
        clock: WorldClock::new(3600.0, Office::Dayspring, 2, 0.05),
        nav: Some(Arc::new(nav)),
        shelters: Arc::new(ShelterMap::from_json_str(&read("world/shelters.json")?)?),
        ..EngineConfig::default()
    };
    config.idle_curiosity.enabled = true;
    config.night_office.enabled = true;
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
        Box::new(cognition.clone()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (position, 0.0),
        0,
        0.0,
    )?;
    let startup_ms = started.elapsed().as_secs_f64() * 1000.0;
    let initial_positions: Vec<_> = engine
        .world()
        .characters
        .iter()
        .map(|(id, actor)| (id.clone(), actor.position_m()))
        .collect();
    let mut poll_us = Vec::with_capacity(args.polls);
    let mut snapshot_bytes_max = 0;
    let mut snapshot_publications = 0;
    let mut messages = 0;
    let mut knowledge_bytes_max = 0;
    let mut speech_events = 0;
    for step in 1..=args.warmup + args.polls {
        // Multiplication avoids accumulating floating-point summation drift.
        // Every supplied increment is nominally 0.05 seconds; there is never
        // coarse clock acceleration or a discarded physical span in this harness.
        let now = step as f64 * 0.05;
        let mut commands: Vec<_> = cognition
            .0
            .borrow_mut()
            .fake
            .drain_completions()
            .into_iter()
            .map(EngineCommand::LlmCompletion)
            .collect();
        if !args.idle && step % 200 == 1 {
            commands.push(EngineCommand::PlayerSay {
                request_id: format!("baseline-{step}"),
                text: "Does anyone know where the nearest well is?".into(),
                position_m: position,
                spatial_seq: step as i64,
            });
        }
        let before = Instant::now();
        let output = engine.poll(now, commands);
        let elapsed = before.elapsed().as_secs_f64() * 1_000_000.0;
        if step > args.warmup {
            poll_us.push(elapsed);
            messages += output.len();
            for message in &output {
                match message {
                    EngineMessage::Ready { snapshot, .. } | EngineMessage::Snapshot(snapshot) => {
                        snapshot_publications += 1;
                        snapshot_bytes_max =
                            snapshot_bytes_max.max(serde_json::to_vec(snapshot)?.len());
                    }
                    EngineMessage::Speech { .. } => speech_events += 1,
                    _ => {}
                }
            }
            if step % 100 == 0 {
                knowledge_bytes_max = knowledge_bytes_max.max(
                    engine.world().knowledge.footprint_bytes() + engine.knowledge_auxiliary_bytes(),
                );
            }
        }
    }
    // The read-only serialization/allocation probes are outside each timed poll.
    let snapshot_bytes_final = serde_json::to_vec(&engine.snapshot())?.len();
    let moved = initial_positions
        .iter()
        .filter(|(id, start)| {
            engine
                .world()
                .characters
                .get(id)
                .is_some_and(|actor| actor.position_m().distance(*start) > 0.1)
        })
        .count();
    let brain = cognition.0.borrow();
    let result = json!({
        "schema": 1, "extra_residents": args.extra, "placement": placement,
        "actors": engine.world().characters.len(),
        "workload": if args.idle { "idle" } else { "market_conversation" },
        "step_seconds": 0.05, "warmup_polls": args.warmup, "measured_polls": args.polls,
        "seconds_per_day": 3600, "start_day": 2, "start_office": "dayspring",
        "player_position": [position.x, position.y, position.z],
        "startup_ms": startup_ms, "poll_us": poll_us, "moved_actors_over_0_1m": moved,
        "message_count": messages, "measured_speech_events": speech_events,
        "snapshot_publications": snapshot_publications, "snapshot_bytes_max": snapshot_bytes_max,
        "snapshot_bytes_final": snapshot_bytes_final, "knowledge_bytes_max": knowledge_bytes_max,
        "cognition_calls_including_warmup": brain.calls, "prompt_bytes_max": brain.prompt_bytes_max,
        "checkpoint_bytes": null,
        "scope": "pure engine poll elapsed cost; no renderer/controller/filesystem/provider latency; startup and read-only byte probes excluded from poll timings"
    });
    fs::write(args.output, serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}
