//! M1d CPU-only active-duty probe. Identical declared city/config for 0/16
//! operations; sample bounded accepted Engine polls, never wall-clock acceleration.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::operations::{AdapterDeclaration, FixtureDeclaration, OperationConfig, Request};
use cathedral_sim::receipts::{OperationId, ReceiptState};
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Control, Engine, EngineCommand, EngineConfig,
    EngineMessage, IdleCognitionMode, NavData, NullSight, NullTranscription, NullTts, Office,
    Presence, PromptEnv, RequestId, ShelterMap, SoundCatalog, TtsBackendKind, Vec3, WorldClock,
};
use clap::Parser;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};

struct UnavailableCognition;
impl Cognition for UnavailableCognition {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 0)]
    extra: usize,
    #[arg(long, default_value_t = 16)]
    active: usize,
    #[arg(long, default_value_t = 400)]
    polls: usize,
    #[arg(long, default_value_t = 100)]
    warmup: usize,
    /// Use 1/60 to expose active needs work between the Round's 20 Hz ticks.
    #[arg(long, default_value_t = 0.05)]
    step_seconds: f64,
    /// Short work also emits a deterministic Accepted/Running/Completed trace.
    #[arg(long, default_value_t = 3600.0)]
    work_seconds: f64,
    #[arg(long)]
    output: PathBuf,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if ![0, 16].contains(&args.active)
        || args.extra > 2000
        || args.polls == 0
        || args.polls > 2000
        || args.warmup > 1200
        || !args.step_seconds.is_finite()
        || args.step_seconds <= 0.0
        || args.step_seconds > 0.05
        || !args.work_seconds.is_finite()
        || args.work_seconds <= 0.0
        || args.work_seconds > 3600.0
    {
        return Err(
            "require active 0/16, extra <=2000, polls 1..2000, warmup <=1200, step (0,0.05], work (0,3600]".into(),
        );
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = root.join("assets");
    let read = |name: &str| fs::read_to_string(assets.join(name));
    let nav = Arc::new(NavData::from_parts(
        &read("world/navigation.json")?,
        &fs::read(assets.join("world/navigation.bin"))?,
    )?);
    let mut seed = load_world_seed(&assets, &root.join("lore"))?;
    let mut placement = json!({"requested":0,"placed":0,"unplaced":0});
    if args.extra > 0 {
        let occupied: Vec<_> = seed.characters.iter().map(|c| c.position_m).collect();
        let crowd = cathedral_sim::generate_ambient(&nav, args.extra, 0, &occupied, &[])?;
        if crowd.placement.placed != args.extra {
            return Err("probe did not place the requested crowd".into());
        }
        placement = json!({"requested":crowd.placement.requested,"placed":crowd.placement.placed,
            "unplaced":crowd.placement.unplaced,"housed":crowd.placement.housed,
            "hardship":crowd.placement.hardship,"workers":crowd.placement.workers});
        seed = seed.with_extra_ambient(crowd.sheets)?;
    }
    let candidates: Vec<_> = seed
        .characters
        .iter()
        .filter(|c| c.control == Control::Llm && c.presence == Presence::InCity)
        .take(64)
        .enumerate()
        .map(|(n, c)| {
            (
                c.id.clone(),
                FixtureDeclaration {
                    id: format!("operation_probe_{n}"),
                    adapter: AdapterDeclaration::default(),
                    position: c.position_m.to_array(),
                },
            )
        })
        .collect();
    let config = EngineConfig {
        fake_mode: true,
        idle_mode: IdleCognitionMode::Stage,
        tts_selected: TtsBackendKind::Off,
        clock: WorldClock::new(3600.0, Office::Dayspring, 2, 0.05),
        nav: Some(nav),
        operations: OperationConfig {
            fixtures: candidates.iter().map(|(_, f)| f.clone()).collect(),
        },
        shelters: Arc::new(ShelterMap::from_json_str(&read("world/shelters.json")?)?),
        ..Default::default()
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
    engine.poll(0.0, Vec::new());
    let mut accepted = Vec::new();
    let mut refusals = Vec::new();
    let mut trace = Vec::new();
    for (n, (actor, fixture)) in candidates.iter().enumerate() {
        if accepted.len() == args.active {
            break;
        }
        let id = OperationId {
            producer: 0,
            sequence: n as u64 + 1,
        }
        .command(0);
        let messages = engine.poll(
            0.0,
            vec![
                EngineCommand::Operation(Request::Start {
                    actor: actor.clone(),
                    resource: fixture.id.clone(),
                    adapter: fixture.adapter.clone(),
                    work_seconds: args.work_seconds,
                    recovery_seconds: 7200.0,
                    retries: 2,
                })
                .identified(id),
            ],
        );
        let receipt = engine
            .world()
            .command_ledger
            .get(id)
            .ok_or("missing operation receipt")?;
        if receipt.outcome.state == ReceiptState::Accepted {
            accepted.push(id);
        } else {
            refusals.push(receipt.clone());
        }
        for message in messages {
            if let EngineMessage::ActionReceipt(receipt) = message {
                trace.push(receipt);
            }
        }
    }
    if accepted.len() != args.active {
        return Err(format!(
            "admitted {} of {}: {refusals:?}",
            accepted.len(),
            args.active
        )
        .into());
    }
    let initial_positions: Vec<_> = engine
        .world()
        .characters
        .iter()
        .map(|(id, c)| (id.clone(), c.position_m()))
        .collect();
    let mut poll_us = Vec::with_capacity(args.polls);
    let mut active_counts = Vec::with_capacity(args.polls);
    for step in 1..=args.warmup + args.polls {
        let now = step as f64 * args.step_seconds;
        let begin = Instant::now();
        let messages = engine.poll(now, Vec::new());
        let elapsed = begin.elapsed().as_secs_f64() * 1e6;
        if step > args.warmup {
            poll_us.push(elapsed);
            active_counts.push(engine.world().operations.active_count());
        }
        for message in messages {
            if let EngineMessage::ActionReceipt(receipt) = message {
                trace.push(receipt);
            }
        }
    }
    let moved = initial_positions
        .iter()
        .filter(|(id, p)| engine.world().characters[id].position_m().distance(*p) > 0.01)
        .count();
    let completed: u64 = candidates
        .iter()
        .map(|(_, f)| {
            engine
                .world()
                .operations
                .fixture(&f.id)
                .unwrap()
                .completed_units
        })
        .sum();
    let final_receipts: Vec<_> = accepted
        .iter()
        .filter_map(|id| engine.world().command_ledger.get(*id))
        .collect();
    fs::write(
        &args.output,
        serde_json::to_vec_pretty(&json!({"probe":"operation_fixture_v1","extra":args.extra,
        "total_actors":engine.world().characters.len(),"placement":placement,"requested_active":args.active,"admitted":accepted.len(),"polls":args.polls,"warmup":args.warmup,
        "step_seconds":args.step_seconds,"work_seconds":args.work_seconds,"poll_us":poll_us,"active_counts":active_counts,
        "completed_units":completed,"moved_actors":moved,"kernel_heap_upper_bound":engine.world().operations.allocated_upper_bound(),
        "admission_refusals":refusals,"receipts":final_receipts,"transition_trace":trace,
        "limits":"CPU Engine only; fixture adapter mix, no renderer, no appointments/orders/observations; active work refreshes shared need decay each boundary"}))?,
    )?;
    Ok(())
}
