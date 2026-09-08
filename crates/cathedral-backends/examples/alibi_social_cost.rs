//! M2a10 existing-social component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode,
    NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv, RequestId, ShelterMap,
    SoundCatalog, TtsBackendKind, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::social_checkpoint::EngineSocialDtoV1,
    night::NightOfficeConfig,
    timeline::LogicalTime,
};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
#[derive(Default)]
struct Service {
    prompts: Vec<String>,
    pending: Option<RequestId>,
}
struct Recorded(Arc<std::sync::Mutex<Service>>);
impl Cognition for Recorded {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        let mut s = self.0.lock().unwrap();
        s.prompts.push(prompt);
        let id = RequestId(s.prompts.len() as u64);
        s.pending = Some(id);
        Ok(id)
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
    // Choose two real, settled bodies already close enough to hear.
    // Only the ordinary player spawn is placed beside them; no social state or
    // private Engine owner is edited to manufacture the measured boundary.
    let pair = seed
        .characters
        .iter()
        .filter(|a| {
            a.control == cathedral_sim::Control::Llm
                && a.presence == cathedral_sim::Presence::InCity
        })
        .find_map(|a| {
            seed.characters
                .iter()
                .find(|b| {
                    b.id != a.id
                        && b.control == cathedral_sim::Control::Llm
                        && b.presence == cathedral_sim::Presence::InCity
                        && a.position_m.distance(b.position_m) < 8.0
                })
                .map(|b| (a.id.clone(), a.name.clone(), b.id.clone(), a.position_m))
        })
        .ok_or("no hearing pair")?;
    let (partner, name, peer, at) = pair;
    let service = Arc::new(std::sync::Mutex::new(Service::default()));
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            tts_selected: TtsBackendKind::Off,
            clock: WorldClock::new(3600.0, Office::Dayspring, 0, 0.05),
            night_office: NightOfficeConfig {
                enabled: false,
                majors: false,
                wards: false,
                ambients: false,
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
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (at, 0.0),
        0,
        0.0,
    )?;
    let utterance = format!("{name}, can you tell me about this street?");
    let first = engine.poll(
        0.0,
        vec![
            cathedral_sim::EngineCommand::PlayerAttention {
                actor_id: Some(partner.clone()),
            },
            cathedral_sim::EngineCommand::PlayerSay {
                request_id: "social-probe-first".into(),
                text: utterance.clone(),
                position_m: at,
                spatial_seq: 1,
            },
        ],
    );
    let selected = engine.scheduler().in_flight_actor_id().cloned();
    if selected.as_ref() != Some(&partner) {
        return Err(format!("named reaction did not select partner: {selected:?}").into());
    }
    let request_id = service
        .lock()
        .unwrap()
        .pending
        .take()
        .ok_or("ordinary reaction not submitted")?;
    let reply = format!(
        "say {{\"target\":\"player\",\"text\":\"This street is familiar to me.\"}}\nsay {{\"target\":\"{peer}\",\"text\":\"Can you join our conversation?\"}}"
    );
    let second = engine.poll(
        0.05,
        vec![cathedral_sim::EngineCommand::LlmCompletion(
            cathedral_sim::Completion {
                request_id,
                result: Ok(reply.clone()),
                duration_seconds: 0.05,
            },
        )],
    );
    engine.poll(
        0.3,
        vec![cathedral_sim::EngineCommand::PlayerAttention {
            actor_id: Some(peer.clone()),
        }],
    );
    let boundary = LogicalTime::new(0.3).unwrap();
    let context = engine.social_checkpoint_context(boundary);
    let service = service.lock().unwrap();
    let speeches: Vec<_> = first
        .iter()
        .chain(&second)
        .filter(|m| matches!(m, cathedral_sim::EngineMessage::Speech { .. }))
        .map(|m| format!("{m:?}"))
        .collect();
    let witnesses = json!({"boundary_seconds":boundary.seconds(),"partner":partner,"peer":peer,"player_position_m":at.to_array(),"utterance":utterance,"reply":reply,"submitted_actor":selected,"provider_submissions":service.prompts.len(),"maximum_submitted_prompt_bytes":service.prompts.iter().map(String::len).max().unwrap_or(0),"speech_messages":speeches,"partner_retained":engine.conversation_partner(boundary.seconds())==Some(&partner)});
    assert!(witnesses["partner_retained"] == true && !speeches.is_empty());
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_social_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_social_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert!(
            counts.conversation.engaged
                && counts.conversation.reciprocal
                && counts.conversation.focus
                && counts.warm_pairs > 0
                && counts.novelty_memories > 0
                && counts.novelty_told > 0,
            "{counts:?}"
        );
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineSocialDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        let current = json!({"scenario":"social-continuity-v1","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
