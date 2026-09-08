//! M2a1 component CPU/allocation probe. This is not a complete save benchmark.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::checkpoint::{Admitted, CheckpointBudget, Cohort, Reservation};
use cathedral_sim::operations::{
    FixtureDeclaration, OperationConfig, OperationKernelDtoV1, Request,
};
use cathedral_sim::receipts::{
    self, Admission, AffectedRef, CommandLedgerDtoV1, OperationId, Outcome, ReceiptState,
};
use cathedral_sim::timeline::LogicalTime;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Control, Engine, EngineCommand, EngineConfig,
    IdleCognitionMode, NullSight, NullTranscription, NullTts, Presence, PromptEnv, RequestId,
    SoundCatalog, TtsBackendKind, Vec3,
};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::PathBuf, time::Instant};

struct UnavailableCognition;
impl Cognition for UnavailableCognition {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Authored,
    Maximum,
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
struct Component {
    encoded_bytes: usize,
    conservative_heap_bytes: usize,
    working_allowance_bytes: usize,
    reserved_peak_bytes: usize,
    export_us: Vec<f64>,
    encode_us: Vec<f64>,
    decode_validate_us: Vec<f64>,
    index_validate_us: Vec<f64>,
    drop_us: Vec<f64>,
}
fn charge(budget: &CheckpointBudget, cohort: Cohort) -> Reservation {
    budget
        .reserve(cohort, 40 * 1024 * 1024)
        .expect("bounded component allowance")
}
fn time<T>(samples: &mut Vec<f64>, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = work();
    samples.push(start.elapsed().as_secs_f64() * 1e6);
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
    let seed = load_world_seed(&assets, &root.join("lore"))?;
    let count = match args.mode {
        Mode::Authored => 0,
        Mode::Maximum => 256,
    };
    let candidates: Vec<_> = seed
        .characters
        .iter()
        .filter(|c| c.control == Control::Llm && c.presence == Presence::InCity)
        .take(count)
        .enumerate()
        .map(|(n, c)| {
            (
                c.id.clone(),
                FixtureDeclaration {
                    id: format!("checkpoint_fixture_{n:045}"),
                    adapter: Default::default(),
                    position: c.position_m.to_array(),
                },
            )
        })
        .collect();
    assert_eq!(candidates.len(), count);
    let operations = OperationConfig {
        fixtures: candidates.iter().map(|(_, f)| f.clone()).collect(),
    };
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            idle_mode: IdleCognitionMode::Stage,
            operations: operations.clone(),
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
    for (n, (actor, f)) in candidates.iter().enumerate() {
        let id = OperationId {
            producer: 0,
            sequence: n as u64 + 1,
        }
        .command(0);
        engine.poll(
            0.0,
            vec![
                EngineCommand::Operation(Request::Start {
                    actor: actor.clone(),
                    resource: f.id.clone(),
                    adapter: f.adapter.clone(),
                    work_seconds: 100.0,
                    recovery_seconds: 200.0,
                    retries: 2,
                })
                .identified(id),
            ],
        );
        let receipt = engine
            .world()
            .command_ledger
            .get(id)
            .ok_or("missing active receipt")?;
        if receipt.outcome.state != ReceiptState::Accepted {
            return Err(format!("operation {n} refused: {receipt:?}").into());
        }
    }
    if count != 0 {
        // Fill through the production ledger, retaining all 256 active start
        // receipts behind the floor. Deliberately full legal metadata strings.
        let ledger = &mut engine.world_mut().command_ledger;
        for n in 257..=4352 {
            let id = OperationId {
                producer: 0,
                sequence: n,
            }
            .command(0);
            let Admission::New(ticket) = ledger.begin(id, &json!({"component_probe": n})) else {
                return Err("maximum ledger admission refused".into());
            };
            ledger.finish(
                ticket,
                0.0,
                Outcome::new(ReceiptState::Completed, &"c".repeat(48), &"m".repeat(192)),
                vec![
                    AffectedRef::new("actor", &"a".repeat(64)).unwrap(),
                    AffectedRef::new("item", &"i".repeat(64)).unwrap(),
                ],
            );
        }
        assert_eq!(ledger.recent_len(), receipts::RECENT_CAPACITY);
        assert_eq!(ledger.retained_len(), receipts::PROTECTED_CAPACITY);
        assert_eq!(ledger.protected_len(), receipts::PROTECTED_CAPACITY);
    }
    let now = LogicalTime::new(0.0).unwrap();
    let world = engine.world();
    let mut ledger_cost = Component {
        working_allowance_bytes: CommandLedgerDtoV1::WORKING_BYTES,
        ..Default::default()
    };
    let mut kernel_cost = Component {
        working_allowance_bytes: OperationKernelDtoV1::WORKING_BYTES,
        ..Default::default()
    };
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let save = charge(&budget, Cohort::SavePayload);
        let dto = time(&mut ledger_cost.export_us, || {
            world.command_ledger.checkpoint_v1(now, save)
        })?;
        ledger_cost.conservative_heap_bytes = dto.value().recent_heap_upper_bound();
        let encoded: Admitted<Vec<u8>> = time(&mut ledger_cost.encode_us, || {
            CommandLedgerDtoV1::encode_json(dto, now)
        })?;
        ledger_cost.encoded_bytes = encoded.value().len();
        let load = charge(&budget, Cohort::LoadCandidate);
        let loaded = time(&mut ledger_cost.decode_validate_us, || {
            CommandLedgerDtoV1::decode_json(encoded.value(), now, load)
        })?;
        time(&mut ledger_cost.index_validate_us, || {
            loaded.value().validate_index_build(now)
        })?;
        ledger_cost.reserved_peak_bytes = budget.retained_bytes();
        time(&mut ledger_cost.drop_us, || drop((loaded, encoded)));
        assert_eq!(budget.retained_bytes(), 0);

        let save = charge(&budget, Cohort::SavePayload);
        let dto = time(&mut kernel_cost.export_us, || {
            world
                .operations
                .checkpoint_v1(&operations, world, now, save)
        })?;
        kernel_cost.conservative_heap_bytes = world.operations.allocated_upper_bound();
        let encoded = time(&mut kernel_cost.encode_us, || {
            OperationKernelDtoV1::encode_json(dto, &operations, world, now)
        })?;
        kernel_cost.encoded_bytes = encoded.value().len();
        let load = charge(&budget, Cohort::LoadCandidate);
        let loaded = time(&mut kernel_cost.decode_validate_us, || {
            OperationKernelDtoV1::decode_json(encoded.value(), &operations, world, now, load)
        })?;
        time(&mut kernel_cost.index_validate_us, || {
            loaded.value().validate(&operations, world, now)
        })?;
        kernel_cost.reserved_peak_bytes = budget.retained_bytes();
        time(&mut kernel_cost.drop_us, || drop((loaded, encoded)));
        assert_eq!(budget.retained_bytes(), 0);
    }
    fs::write(
        &args.output,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1, "mode": format!("{:?}", args.mode).to_lowercase(), "samples": args.samples,
            "actor_count": world.characters.len(), "active_count": world.operations.active_count(),
            "recent_count": world.command_ledger.recent_len(), "retained_count": world.command_ledger.retained_len(),
            "protected_root_count": world.command_ledger.protected_len(),
            "ledger": ledger_cost, "kernel": kernel_cost,
            "scope": "nav-less authored Engine; component operations only; no complete world capture, host frame or filesystem timings",
            "phase_notes": {
                "export_us": "live-owner validation, DTO extraction and DTO validation; operation validation also constructs/disposes indexes",
                "encode_us": "validation, counted JSON sizing, exact-capacity encoding and consumed DTO destruction",
                "decode_validate_us": "bounded JSON decode and validation; operations also construct/dispose candidate indexes",
                "index_validate_us": "ledger validates and constructs/disposes recent/retained BTreeMaps; operations validate and construct/dispose all candidate indexes",
                "drop_us": "decoded DTO and encoded bytes destruction, including reservation release",
                "reserved_peak_bytes": "simultaneous per-component save/load allowances, excludes the running Engine and allocator RSS",
                "conservative_heap_bytes": "ledger recent BTree allocation upper bound; kernel all retained authoritative and derived allocations"
            }
        }))?,
    )?;
    Ok(())
}
