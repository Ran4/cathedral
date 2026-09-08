//! M2a9 existing-Night component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode,
    NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv, RequestId, ShelterMap,
    SoundCatalog, TtsBackendKind, Vec3, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::night_checkpoint::EngineNightDtoV1,
    night::NightOfficeConfig,
    timeline::LogicalTime,
};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
#[derive(Default)]
struct Service {
    attempts: usize,
    prompts: Vec<String>,
    pending: Option<RequestId>,
}
struct Recorded(Arc<std::sync::Mutex<Service>>);
impl Cognition for Recorded {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
    fn request_night(
        &mut self,
        prompt: String,
        _: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        let mut s = self.0.lock().unwrap();
        s.attempts += 1;
        if s.attempts == 1 {
            return Err(CognitionBusy);
        }
        s.prompts.push(prompt);
        let id = RequestId(s.prompts.len() as u64);
        s.pending = Some(id);
        Ok(id)
    }
}
fn saturate(w: &mut cathedral_sim::World, now: f64) -> Result<(), Box<dyn std::error::Error>> {
    use cathedral_sim::receipts::*;
    for _ in 0..20 {
        let op = w
            .command_ledger
            .reserve_operation(HOST_PRODUCER)
            .map_err(|e| e.message)?;
        for step in 0..256 {
            if w.command_ledger.recent_len() == RECENT_CAPACITY
                && w.command_ledger.retained_len() == PROTECTED_CAPACITY
            {
                return Ok(());
            }
            let Admission::New(t) = w
                .command_ledger
                .begin(op.command(step), &json!({"fixture":"protected"}))
            else {
                return Err("receipt saturation setup failed".into());
            };
            w.command_ledger
                .finish(t, now, Outcome::completed("fixture"), vec![]);
            w.command_ledger.drain_updates();
        }
    }
    Err("receipt saturation missing".into())
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
    let service = Arc::new(std::sync::Mutex::new(Service::default()));
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            tts_selected: TtsBackendKind::Off,
            clock: WorldClock::new(60.0, Office::Waning, 0, 0.05),
            night_office: NightOfficeConfig {
                enabled: true,
                majors: false,
                wards: true,
                ambients: true,
            },
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
        Box::new(Recorded(service.clone())),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(10000.0, cathedral_sim::WALK_Y, 10000.0), 0.0),
        0,
        0.0,
    )?;
    let mut busy_admitted_observed = false;
    let mut submitted_observed = false;
    let mut ambient_reroll_observed = false;
    let mut first_applied = false;
    let mut saturated = false;
    let mut held_deferred_observed = false;
    let mut selected = None;
    for frame in 0..600 {
        let at = f64::from(frame) * 0.05;
        let pending = service.lock().unwrap().pending.take();
        let mut commands = vec![];
        if let Some(id) = pending {
            if first_applied && !saturated {
                saturate(engine.world_mut(), at)?;
                saturated = true;
            }
            commands.push(cathedral_sim::EngineCommand::LlmCompletion(
                cathedral_sim::Completion {
                    request_id: id,
                    result: Ok(
                        "ward_mood {\"mood\":\"The evening accounts await the morning.\"}".into(),
                    ),
                    duration_seconds: 0.125,
                },
            ));
        }
        let out = engine.poll(at, commands);
        ambient_reroll_observed |= out.iter().any(|m|matches!(m,cathedral_sim::EngineMessage::Diagnostic(t) if t.contains("ambient evenings")));
        first_applied |= !engine.world().ward_moods.is_empty();
        let boundary = LogicalTime::new(at).unwrap();
        let b = CheckpointBudget::default();
        let d = engine.export_night_checkpoint(boundary, b.reserve(Cohort::SavePayload, 4096)?)?;
        let counts = d.value().counts(engine.night_checkpoint_context(boundary));
        busy_admitted_observed |= counts.night.queued_admitted > 0;
        submitted_observed |= counts.night.in_flight;
        held_deferred_observed |= counts.night.held_success;
        if counts.night.held_success && counts.night.queued > 0 && counts.ward_moods > 0 {
            selected = Some(boundary);
            break;
        }
    }
    let boundary = selected.ok_or("ordinary Night never selected held/owed/mood boundary")?;
    let context = engine.night_checkpoint_context(boundary);
    let service = service.lock().unwrap();
    let witnesses = json!({"busy_admitted_observed":busy_admitted_observed,"submitted_observed":submitted_observed,"held_deferred_observed":held_deferred_observed,"ambient_reroll_observed":ambient_reroll_observed,"completed_reflections":engine.night().totals().0,"provider_attempts":service.attempts,"provider_submissions":service.prompts.len(),"maximum_submitted_prompt_bytes":service.prompts.iter().map(String::len).max().unwrap_or(0),"receipt_recent":engine.world().command_ledger.recent_len(),"receipt_retained":engine.world().command_ledger.retained_len(),"receipt_protected":engine.world().command_ledger.protected_len(),"boundary_seconds":boundary.seconds()});
    assert!(
        busy_admitted_observed && submitted_observed && held_deferred_observed && first_applied,
        "{witnesses}"
    );
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_night_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_night_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert!(
            counts.night.queued > 0
                && counts.night.in_flight
                && counts.night.held_success
                && counts.ward_moods > 0,
            "{counts:?}"
        );
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineNightDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        let current = json!({"scenario":"night-obligations-v1","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
