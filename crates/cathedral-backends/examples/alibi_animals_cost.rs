//! M2a8 existing-animals component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode,
    NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv, RequestId, ShelterMap,
    SoundCatalog, TtsBackendKind, Vec3, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    dogs::checkpoint::AnimalsCheckpointContext,
    engine::animals_checkpoint::EngineAnimalsDtoV1,
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
    let initial = engine.poll(0.0, Vec::new());
    let dog_messages = |out: &[cathedral_sim::EngineMessage]| {
        out.iter()
            .filter(|m| matches!(m, cathedral_sim::EngineMessage::Dogs { .. }))
            .count()
    };
    let initial_resting_publications = dog_messages(&initial);
    let initial_all_resting = engine.world().dogs.iter().all(|d| d.speed == 0.0);
    let quiet_publications = dog_messages(&engine.poll(0.011, Vec::new()));
    let mut movement_publications = 0;
    let mut turning_observed = false;
    let mut acceleration_observed = false;
    let mut selected = None;
    for frame in 1..=1200 {
        let before = engine.world().dogs.clone();
        let at = 0.011 + f64::from(frame) * 0.05;
        let out = engine.poll(at, Vec::new());
        movement_publications += dog_messages(&out);
        turning_observed |= before
            .iter()
            .zip(&engine.world().dogs)
            .any(|(a, b)| a.position_m == b.position_m && a.facing_yaw != b.facing_yaw);
        acceleration_observed |= before
            .iter()
            .zip(&engine.world().dogs)
            .any(|(a, b)| b.speed > a.speed && b.speed < cathedral_sim::dogs::DOG_TROT_MPS);
        if turning_observed && acceleration_observed {
            let boundary = LogicalTime::new(at).unwrap();
            let budget = CheckpointBudget::default();
            let dto = engine
                .export_animals_checkpoint(boundary, budget.reserve(Cohort::SavePayload, 4096)?)?;
            let counts = dto.value().counts(AnimalsCheckpointContext::from_world(
                engine.world(),
                boundary,
            ));
            if counts.animals.resting > 0 && counts.animals.moving > 0 && counts.animals.turning > 0
            {
                selected = Some(boundary);
                break;
            }
        }
    }
    let boundary = selected
        .ok_or("ordinary pack never produced concurrent resting/moving/turning witnesses")?;
    let w = engine.world();
    let context = AnimalsCheckpointContext::from_world(w, boundary);
    let budget = CheckpointBudget::default();
    let dto =
        engine.export_animals_checkpoint(boundary, budget.reserve(Cohort::SavePayload, 4096)?)?;
    let candidate = dto.into_candidate(context)?;
    let movement_now_seconds = candidate.value().movement_now().seconds();
    let cadence_residual_seconds = boundary.seconds() - movement_now_seconds;
    let witnesses = json!({"initial_resting_publications":initial_resting_publications,"initial_all_resting":initial_all_resting,"quiet_publications":quiet_publications,"movement_publications":movement_publications,"turning_observed":turning_observed,"acceleration_observed":acceleration_observed,"movement_now_seconds":movement_now_seconds,"boundary_seconds":boundary.seconds(),"cadence_residual_seconds":cadence_residual_seconds});
    assert!(
        initial_resting_publications == 1
            && initial_all_resting
            && quiet_publications == 0
            && movement_publications > 1
            && cadence_residual_seconds > 0.0
            && cadence_residual_seconds < cathedral_sim::MOVEMENT_TICK_SECONDS,
        "{witnesses}"
    );
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_animals_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_animals_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert!(
            counts.animals.dogs == 10
                && counts.animals.path_dogs > 0
                && counts.animals.resting > 0
                && counts.animals.moving > 0
                && counts.animals.turning > 0
                && counts.dogs_published
                && counts.engine_nav,
            "{counts:?}"
        );
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineAnimalsDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        let current = json!({"scenario":"animals-obligations-v1","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
