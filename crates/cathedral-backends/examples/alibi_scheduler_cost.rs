//! M2a11 existing-scheduler component diagnostic. No full-save or synchronous-host budget claim.
#[path = "support/cognition_inputs_cost.rs"]
mod cognition_inputs_cost;
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode,
    NavData, NullSight, NullTranscription, NullTts, Office, PromptEnv, RequestId, ShelterMap,
    SoundCatalog, TtsBackendKind, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::scheduler_checkpoint::EngineSchedulerDtoV1,
    night::NightOfficeConfig,
    timeline::LogicalTime,
};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
#[derive(Default)]
struct Service {
    prompts: Vec<(String, Option<u32>)>,
    pending: Option<RequestId>,
}
struct Recorded(Arc<std::sync::Mutex<Service>>);
impl Cognition for Recorded {
    fn request(&mut self, prompt: String) -> Result<RequestId, CognitionBusy> {
        let mut s = self.0.lock().unwrap();
        s.prompts.push((prompt, None));
        let id = RequestId(s.prompts.len() as u64);
        s.pending = Some(id);
        Ok(id)
    }
    fn request_with_budget(
        &mut self,
        prompt: String,
        budget: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        let mut s = self.0.lock().unwrap();
        s.prompts.push((prompt, budget));
        let id = RequestId(s.prompts.len() as u64);
        s.pending = Some(id);
        Ok(id)
    }
}
// Diagnostic stream fingerprint only, not semantic identity or security.
struct MessageDigest {
    hash: u64,
    bytes: usize,
}
impl Default for MessageDigest {
    fn default() -> Self {
        Self {
            hash: 0xcbf29ce484222325,
            bytes: 0,
        }
    }
}
impl std::fmt::Write for MessageDigest {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.bytes += text.len();
        for byte in text.bytes() {
            self.hash ^= u64::from(byte);
            self.hash = self.hash.wrapping_mul(0x100000001b3);
        }
        Ok(())
    }
}
#[derive(Default)]
struct Driver {
    now: f64,
    polls: usize,
    maximum_poll_step_seconds: f64,
    coarse_discard_diagnostics: usize,
    messages: usize,
    digest: MessageDigest,
}
impl Driver {
    fn call(
        &mut self,
        e: &mut Engine,
        now: f64,
        commands: Vec<cathedral_sim::EngineCommand>,
    ) -> Vec<cathedral_sim::EngineMessage> {
        use std::fmt::Write;
        let delta = now - self.now;
        assert!(
            delta >= 0.0 && delta <= 0.05 + 1e-12,
            "unbounded probe poll {delta}"
        );
        self.maximum_poll_step_seconds = self.maximum_poll_step_seconds.max(delta);
        self.now = now;
        self.polls += 1;
        let out = e.poll(now, commands);
        for message in &out {
            self.messages += 1;
            write!(&mut self.digest, "{message:?}\n").unwrap();
            if matches!(message,cathedral_sim::EngineMessage::Diagnostic(text) if text.contains("coarse poll discarded"))
            {
                self.coarse_discard_diagnostics += 1;
            }
        }
        assert_eq!(
            self.coarse_discard_diagnostics, 0,
            "probe discarded physical work"
        );
        out
    }
    fn poll(
        &mut self,
        e: &mut Engine,
        now: f64,
        commands: Vec<cathedral_sim::EngineCommand>,
    ) -> Vec<cathedral_sim::EngineMessage> {
        // 40ms leaves room for ordinary floating-point representation while
        // honoring the same <=50ms accepted stepping contract for every call.
        while now - self.now > 0.04 {
            self.call(e, self.now + 0.04, vec![]);
        }
        self.call(e, now, commands)
    }
}
fn relevant(messages: &[cathedral_sim::EngineMessage]) -> Vec<String> {
    messages
        .iter()
        .filter(|m| match m {
            cathedral_sim::EngineMessage::Status(_)
            | cathedral_sim::EngineMessage::ActionReceipt(_)
            | cathedral_sim::EngineMessage::CommandAdmissionRefused { .. } => true,
            cathedral_sim::EngineMessage::Diagnostic(text) => {
                text.starts_with("[smart actors]") || text.starts_with("[time]")
            }
            _ => false,
        })
        .map(|m| format!("{m:?}"))
        .collect()
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Mode {
    Authored,
    Populated,
}
#[derive(Parser)]
struct Args {
    #[arg(long)]
    cognition_inputs: bool,
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
            turn_delay_seconds: 0.0,
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
    let mut driver = Driver::default();
    let utterance = format!("{name}, can you tell me about this street?");
    let first = driver.poll(
        &mut engine,
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
    let second = driver.poll(
        &mut engine,
        0.05,
        vec![cathedral_sim::EngineCommand::LlmCompletion(
            cathedral_sim::Completion {
                request_id,
                result: Ok(reply.clone()),
                duration_seconds: 0.05,
            },
        )],
    );
    // The ordinary handoff is submitted despite the reading floor. Its
    // terminal arrives after that floor expires, creating real retry courtesy.
    let second_id = service
        .lock()
        .unwrap()
        .pending
        .take()
        .ok_or("handoff missing")?;
    assert_eq!(engine.scheduler().in_flight_actor_id(), Some(&peer));
    let failure = driver.poll(
        &mut engine,
        5.0,
        vec![cathedral_sim::EngineCommand::LlmCompletion(
            cathedral_sim::Completion {
                request_id: second_id,
                result: Err(cathedral_sim::CognitionError::detailed(
                    "TimeoutError",
                    "scripted ordinary retry witness",
                )),
                duration_seconds: 4.95,
            },
        )],
    );
    let followup = format!("{name}, can you explain another detail?");
    driver.poll(
        &mut engine,
        5.1,
        vec![cathedral_sim::EngineCommand::PlayerSay {
            request_id: "scheduler-followup".into(),
            text: followup.clone(),
            position_m: at,
            spatial_seq: 2,
        }],
    );
    assert_eq!(engine.scheduler().in_flight_actor_id(), Some(&partner));
    let held_id = service
        .lock()
        .unwrap()
        .pending
        .take()
        .ok_or("protected followup missing")?;
    let late = format!("{name}, can you answer this too?");
    driver.poll(
        &mut engine,
        5.2,
        vec![cathedral_sim::EngineCommand::PlayerSay {
            request_id: "scheduler-late".into(),
            text: late.clone(),
            position_m: at,
            spatial_seq: 3,
        }],
    );
    let held_reply = "remember {\"memory\":\"The player asked for a second detail.\"}";
    let held_messages = driver.poll(
        &mut engine,
        5.3,
        vec![
            cathedral_sim::EngineCommand::PlayerAudioBegin {
                wav_basename: "scheduler-probe.wav".into(),
                sample_rate: 24_000,
            },
            cathedral_sim::EngineCommand::LlmCompletion(cathedral_sim::Completion {
                request_id: held_id,
                result: Ok(held_reply.into()),
                duration_seconds: 0.2,
            }),
        ],
    );
    assert!(engine.scheduler().has_held_result());
    let boundary = LogicalTime::new(5.3).unwrap();
    let context = engine.scheduler_checkpoint_context(boundary);
    let service = service.lock().unwrap();
    let speeches: Vec<_> = first
        .iter()
        .chain(&second)
        .filter(|m| matches!(m, cathedral_sim::EngineMessage::Speech { .. }))
        .map(|m| format!("{m:?}"))
        .collect();
    let witnesses = json!({"boundary_seconds":boundary.seconds(),"partner":partner,"peer":peer,"player_position_m":at.to_array(),"utterance":utterance,"reply":reply,"followup":followup,"late_utterance":late,"held_reply":held_reply,"submitted_actor":selected,"provider_submissions":service.prompts.len(),"submitted_prompts":service.prompts,"maximum_submitted_prompt_bytes":service.prompts.iter().map(|p|p.0.len()).max().unwrap_or(0),"speech_messages":speeches,"failure_messages":relevant(&failure),"held_messages":relevant(&held_messages),"floor_busy_hold":true,"poll_count":driver.polls,"maximum_poll_step_seconds":driver.maximum_poll_step_seconds,"coarse_discard_diagnostics":driver.coarse_discard_diagnostics,"all_message_count":driver.messages,"all_message_debug_bytes":driver.digest.bytes,"all_message_digest_algorithm":"fnv1a64-debug-stream-v1","all_message_digest":format!("{:016x}",driver.digest.hash)});
    assert_eq!(service.prompts.len(), 3);
    if args.cognition_inputs {
        let b = CheckpointBudget::default();
        let legacy =
            engine.export_scheduler_checkpoint(boundary, b.reserve(Cohort::SavePayload, 4096)?)?;
        let counts = serde_json::to_value(legacy.value().counts(context))?;
        drop(legacy);
        let submitted_requests=json!(service.prompts.iter().enumerate().map(|(i,(prompt,budget))|json!({"request_id":i+1,"method":"request_with_budget","prompt":prompt,"output_token_budget":budget})).collect::<Vec<_>>());
        let mut output = cognition_inputs_cost::measure(
            &engine,
            boundary,
            args.samples,
            "scheduler",
            counts,
            witnesses,
            submitted_requests,
        )?;
        output["mode"] = json!(match args.mode {
            Mode::Authored => "authored",
            Mode::Populated => "populated",
        });
        output["samples"] = json!(args.samples);
        output["placement"] = placement;
        fs::write(args.output, serde_json::to_vec_pretty(&output)?)?;
        return Ok(());
    }
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_scheduler_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_scheduler_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert!(
            counts.held_success
                && counts.in_flight
                && counts.flight_player_reaction
                && counts.priority_handoffs > 0
                && counts.player_reactions > 0
                && counts.retry_work > 0
                && counts.provider_failures == 1
                && counts.drained_rows > 0
                && counts.presented_rows > 0
                && !counts.submitted,
            "{counts:?}"
        );
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let wire: serde_json::Value = serde_json::from_slice(bytes.value())?;
        assert_eq!(
            wire["scheduler"]["in_flight"]["prompt"],
            service.prompts.last().unwrap().0
        );
        assert!(
            !wire["scheduler"]["in_flight"]["prompt"]
                .as_str()
                .unwrap()
                .contains(&late)
        );
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineSchedulerDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        let current = json!({"scenario":"scheduler-held-and-retry-v1","counts":counts,"witnesses":witnesses,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
