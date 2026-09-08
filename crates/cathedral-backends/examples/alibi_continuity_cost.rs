//! M2a12 floor/Engine continuity component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode,
    NavData, NullSight, NullTranscription, Office, PromptEnv, RequestId, ShelterMap, SoundCatalog,
    Tts, TtsBackendKind, TtsRequest, TtsSubmitError, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::continuity_checkpoint::EngineContinuityDtoV1,
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
    all_speech_messages: Vec<String>,
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
            if matches!(message, cathedral_sim::EngineMessage::Speech { .. }) {
                self.all_speech_messages.push(format!("{message:?}"));
            }
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
    let voices = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: true,
            turn_delay_seconds: 0.0,
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            tts_selected: TtsBackendKind::Cloud,
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
        Box::new(Voices(voices.clone())),
        Box::new(NullSight),
        Capabilities::new(true, false, false, true, true, TtsBackendKind::Cloud),
        (at, 0.0),
        0,
        0.0,
    )?;
    let initial_budget = CheckpointBudget::default();
    let initial = engine
        .export_continuity_checkpoint(
            LogicalTime::new(0.0).unwrap(),
            initial_budget.reserve(Cohort::SavePayload, 4096)?,
        )?
        .encode()?;
    let initial_wire: serde_json::Value = serde_json::from_slice(initial.value())?;
    assert_eq!(initial_wire["ready_emitted"], false);
    assert_eq!(initial_wire["last_player_sound_at"], "never");
    drop(initial);
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
                request_id: "continuity-first".into(),
                text: utterance.clone(),
                position_m: at,
                spatial_seq: 1,
            },
        ],
    );
    assert_eq!(engine.scheduler().in_flight_actor_id(), Some(&partner));
    let request_id = service
        .lock()
        .unwrap()
        .pending
        .take()
        .ok_or("first submission missing")?;
    let first_reply = format!(
        "say {{\"target\":\"{peer}\",\"text\":\"Come and tell us about this street.\"}}\nsay {{\"target\":\"player\",\"text\":\"My neighbor knows the story.\"}}"
    );
    let first_done = driver.poll(
        &mut engine,
        0.05,
        vec![cathedral_sim::EngineCommand::LlmCompletion(
            cathedral_sim::Completion {
                request_id,
                result: Ok(first_reply.clone()),
                duration_seconds: 0.05,
            },
        )],
    );
    assert_eq!(engine.scheduler().in_flight_actor_id(), Some(&peer));
    let second_id = service
        .lock()
        .unwrap()
        .pending
        .take()
        .ok_or("handoff missing")?;
    let ack_ids: Vec<_> = voices
        .lock()
        .unwrap()
        .iter()
        .map(|(r, _)| r.event_id.clone())
        .collect();
    assert_eq!(ack_ids.len(), 2);
    let ack_one = driver.poll(
        &mut engine,
        0.10,
        vec![cathedral_sim::EngineCommand::SpeechPresented {
            event_id: ack_ids[0].clone(),
        }],
    );
    let ack_two = driver.poll(
        &mut engine,
        0.15,
        vec![cathedral_sim::EngineCommand::SpeechPresented {
            event_id: ack_ids[1].clone(),
        }],
    );
    let selected = driver.poll(
        &mut engine,
        0.20,
        vec![cathedral_sim::EngineCommand::SetTtsBackend {
            request_id: "continuity-local".into(),
            backend: TtsBackendKind::Local,
        }],
    );
    let second_reply = "say {\"target\":\"player\",\"text\":\"The street has been here a long time.\"}\nsay {\"target\":\"player\",\"text\":\"People still trade here every morning.\"}";
    let held = driver.poll(
        &mut engine,
        0.25,
        vec![cathedral_sim::EngineCommand::LlmCompletion(
            cathedral_sim::Completion {
                request_id: second_id,
                result: Ok(second_reply.into()),
                duration_seconds: 0.20,
            },
        )],
    );
    assert!(engine.scheduler().has_held_result());
    driver.poll(&mut engine, 0.60, vec![]);
    let sound = driver.poll(
        &mut engine,
        0.64,
        vec![cathedral_sim::EngineCommand::PlayerSound {
            sound_id: "fart".into(),
        }],
    );
    let cooled = driver.poll(
        &mut engine,
        0.68,
        vec![cathedral_sim::EngineCommand::PlayerSound {
            sound_id: "fart".into(),
        }],
    );
    let capture = driver.poll(
        &mut engine,
        0.72,
        vec![cathedral_sim::EngineCommand::PlayerAudioBegin {
            wav_basename: "continuity-probe.wav".into(),
            sample_rate: 24_000,
        }],
    );
    let boundary = LogicalTime::new(0.72).unwrap();
    let context = engine.continuity_checkpoint_context(boundary);
    let service = service.lock().unwrap();
    let voices = voices.lock().unwrap();
    assert_eq!(voices.len(), 4);
    assert!(voices[2].1);
    assert!(!voices[3].1);
    assert_eq!(
        service.prompts[0].1,
        Some(
            engine.world().characters[&partner]
                .significance()
                .output_token_budget()
        )
    );
    assert_eq!(
        service.prompts[1].1,
        Some(
            engine.world().characters[&peer]
                .significance()
                .output_token_budget()
        )
    );
    assert!(service.prompts.iter().all(|(p, _)| p.len() <= 65_536));
    let speeches: Vec<_> = first
        .iter()
        .chain(&first_done)
        .filter(|m| matches!(m, cathedral_sim::EngineMessage::Speech { .. }))
        .map(|m| format!("{m:?}"))
        .collect();
    let voice_inputs:Vec<_>=voices.iter().map(|(r,accepted)|json!({"event_id":r.event_id,"text":r.text,"voice_key":r.voice_key,"kind":r.kind,"accepted":accepted})).collect();
    let witnesses = json!({"boundary_seconds":boundary.seconds(),"partner":partner,"peer":peer,"player_position_m":at.to_array(),"utterance":utterance,"first_reply":first_reply,"second_reply":second_reply,"initial_continuity":initial_wire,"tts_requests":voice_inputs,"ack_ids":ack_ids,"ack_one_messages":relevant(&ack_one),"ack_two_messages":relevant(&ack_two),"selection_messages":relevant(&selected),"held_messages":relevant(&held),"sound_messages":relevant(&sound),"cooldown_messages":relevant(&cooled),"capture_messages":relevant(&capture),"provider_submissions":service.prompts.len(),"submitted_prompts":service.prompts,"maximum_submitted_prompt_bytes":service.prompts.iter().map(|p|p.0.len()).max().unwrap_or(0),"first_speech_messages":speeches,"all_speech_messages":driver.all_speech_messages,"poll_count":driver.polls,"maximum_poll_step_seconds":driver.maximum_poll_step_seconds,"coarse_discard_diagnostics":driver.coarse_discard_diagnostics,"all_message_count":driver.messages,"all_message_debug_bytes":driver.digest.bytes,"all_message_digest_algorithm":"fnv1a64-debug-stream-v1","all_message_digest":format!("{:016x}",driver.digest.hash)});
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_continuity_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_continuity_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert_eq!(
            counts.characters,
            match args.mode {
                Mode::Authored => 520,
                Mode::Populated => 2520,
            }
        );
        assert!(
            counts.floor.awaiting == 1
                && counts.floor.foreground_awaiting == 1
                && counts.floor.foreground_pacing
                && counts.floor.player_hold
                && counts.ready_emitted
                && counts.sound_ever_emitted,
            "{counts:?}"
        );
        assert_eq!(counts.tts_selected, TtsBackendKind::Local);
        assert_eq!(counts.configured_tts_selected, TtsBackendKind::Cloud);
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let wire: serde_json::Value = serde_json::from_slice(bytes.value())?;
        assert_eq!(
            wire["floor"]["awaiting"][0]["event_id"],
            serde_json::to_value(&voices[2].0.event_id)?
        );
        assert_eq!(wire["last_player_sound_at"], json!({"at":0.64}));
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineContinuityDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        let current = json!({"scenario":"continuity-ordinary-voiced-reading-cadence-v1","counts":counts,"witnesses":witnesses,"boundary_continuity":wire,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
struct Voices(Arc<std::sync::Mutex<Vec<(TtsRequest, bool)>>>);
impl Tts for Voices {
    fn available(&self, kind: TtsBackendKind) -> bool {
        kind != TtsBackendKind::Off
    }
    fn submit(&mut self, request: TtsRequest) -> Result<(), TtsSubmitError> {
        let mut rows = self.0.lock().unwrap();
        let accepted = rows.len() != 3;
        rows.push((request, accepted));
        if accepted {
            Ok(())
        } else {
            Err(TtsSubmitError::QueueFull)
        }
    }
    fn warm(&mut self, _: TtsBackendKind) {}
}
