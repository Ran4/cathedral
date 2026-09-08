//! M2a13 interrupted speech input component diagnostic. No full-save or synchronous-host budget claim.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    AreaMap, Capabilities, Cognition, CognitionBusy, Engine, EngineConfig, IdleCognitionMode,
    NavData, NullSight, Office, PromptEnv, RequestId, ShelterMap, SoundCatalog, Tts,
    TtsBackendKind, TtsRequest, TtsSubmitError, WorldClock,
    checkpoint::{CheckpointBudget, Cohort},
    engine::speech_checkpoint::EngineSpeechDtoV1,
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
    let transcription = Arc::new(std::sync::Mutex::new(SttCalls::default()));
    let mut engine = Engine::new(
        EngineConfig {
            fake_mode: false,
            stt_stream_grace_seconds: 5.0,
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
        Box::new(Stt(transcription.clone())),
        Box::new(Voices(voices.clone())),
        Box::new(NullSight),
        Capabilities::new(true, true, true, true, true, TtsBackendKind::Cloud),
        (at, 0.0),
        0,
        0.0,
    )?;

    use cathedral_sim::receipts::{HOST_PRODUCER, OperationId};
    use cathedral_sim::{
        EngineCommand as Command, RealtimeResult, SttBackendKind, TranscriptionOutcome,
    };
    let mut driver = Driver::default();
    let utterance = format!("{name}, can you tell me about this street?");
    let first = driver.poll(
        &mut engine,
        0.0,
        vec![
            Command::PlayerAttention {
                actor_id: Some(partner.clone()),
            },
            Command::PlayerSay {
                request_id: "speech-first".into(),
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
    let reply = "say {\"target\":\"player\",\"text\":\"I can tell you about this street.\"}";
    let committed = driver.poll(
        &mut engine,
        0.05,
        vec![Command::LlmCompletion(cathedral_sim::Completion {
            request_id,
            result: Ok(reply.into()),
            duration_seconds: 0.05,
        })],
    );
    assert!(
        !voices.lock().unwrap().is_empty(),
        "committed NPC speech was not voiced"
    );
    let mut submitted_inputs = Vec::new();
    macro_rules! command {
        ($time:expr,$command:expr)=>{{let cmd=$command;submitted_inputs.push(json!({"at":$time,"command":format!("{cmd:?}")}));driver.poll(&mut engine,$time,vec![cmd])}};
    }
    command!(
        0.10,
        Command::PlayerUtteranceStarted {
            wav_basename: "onset.wav".into()
        }
    );
    command!(
        0.12,
        Command::PlayerAudioBegin {
            wav_basename: "available.wav".into(),
            sample_rate: 24000
        }
    );
    command!(
        0.14,
        Command::PlayerAudioChunk {
            wav_basename: "available.wav".into(),
            seq: 0,
            samples: Arc::from([1i16, 2, 3, 4])
        }
    );
    command!(
        0.16,
        Command::PlayerAudioEnd {
            wav_basename: "available.wav".into(),
            chunk_count: 1,
            silent: false
        }
    );
    let draft_text =
        "  Available words remain an unsent draft. A new intentional submission is required.  ";
    let draft_result = command!(
        0.18,
        Command::Transcription(TranscriptionOutcome::Realtime(RealtimeResult::Transcript {
            key: "available.wav".into(),
            text: draft_text.into()
        }))
    );
    assert!(
        !draft_result
            .iter()
            .any(|m| matches!(m, cathedral_sim::EngineMessage::Speech { .. }))
    );
    command!(
        0.20,
        Command::PlayerAudioBegin {
            wav_basename: "reused.wav".into(),
            sample_rate: 24000
        }
    );
    command!(
        0.22,
        Command::PlayerAudioChunk {
            wav_basename: "reused.wav".into(),
            seq: 0,
            samples: Arc::from([1i16, 2, 3, 4])
        }
    );
    command!(
        0.24,
        Command::PlayerAudioEnd {
            wav_basename: "reused.wav".into(),
            chunk_count: 1,
            silent: false
        }
    );
    let root = OperationId {
        producer: HOST_PRODUCER,
        sequence: 1,
    };
    let parked_output = command!(
        0.26,
        Command::PlayerRecording {
            request_id: "reused-host-request".into(),
            wav_basename: "reused.wav".into(),
            stt_backend: SttBackendKind::Cloud,
            position_m: at,
            spatial_seq: 2
        }
        .identified(root.command(0))
    );
    let batch_output = command!(
        0.28,
        Command::PlayerRecording {
            request_id: "reused-host-request".into(),
            wav_basename: "reused.wav".into(),
            stt_backend: SttBackendKind::Local,
            position_m: at,
            spatial_seq: 3
        }
        .identified(root.command(1))
    );
    let root2 = OperationId {
        producer: HOST_PRODUCER,
        sequence: 2,
    };
    let second_batch = command!(
        0.30,
        Command::PlayerRecording {
            request_id: "batch-second".into(),
            wav_basename: "batch.wav".into(),
            stt_backend: SttBackendKind::Cloud,
            position_m: at,
            spatial_seq: 4
        }
        .identified(root2.command(0))
    );
    command!(
        0.32,
        Command::PlayerAudioBegin {
            wav_basename: "active.wav".into(),
            sample_rate: 24000
        }
    );
    command!(
        0.34,
        Command::PlayerAudioChunk {
            wav_basename: "active.wav".into(),
            seq: 0,
            samples: Arc::from([1i16, 2, 3, 4])
        }
    );
    let boundary = LogicalTime::new(0.34).unwrap();
    let context = engine.speech_checkpoint_context(boundary);
    let service = service.lock().unwrap();
    let voices = voices.lock().unwrap();
    let transcription = transcription.lock().unwrap();
    assert_eq!(transcription.batch.len(), 2);
    assert_eq!(engine.speech_router().parked_count(), 1);
    assert_eq!(engine.speech_router().pending_transcription_count(), 2);
    let voice_inputs:Vec<_>=voices.iter().map(|(r,accepted)|json!({"event_id":r.event_id,"text":r.text,"voice_key":r.voice_key,"kind":r.kind,"accepted":accepted})).collect();
    let receipts: Vec<_> = [root.command(0), root.command(1), root2.command(0)]
        .iter()
        .map(|id| engine.world().command_ledger.get(*id).unwrap())
        .collect();
    assert!(
        receipts
            .iter()
            .all(|r| r.outcome.state == cathedral_sim::receipts::ReceiptState::Accepted)
    );
    let witnesses = json!({"boundary_seconds":boundary.seconds(),"partner":partner,"peer":peer,"player_position_m":at.to_array(),"utterance":utterance,"reply":reply,"draft_text":draft_text,"submitted_inputs":submitted_inputs,"recording_receipts":receipts,"tts_requests":voice_inputs,"stt_calls":*transcription,"submitted_prompts":service.prompts,"provider_submissions":service.prompts.len(),"first_messages":relevant(&first),"committed_messages":relevant(&committed),"parked_messages":relevant(&parked_output),"batch_messages":relevant(&batch_output),"second_batch_messages":relevant(&second_batch),"all_speech_messages":driver.all_speech_messages,"poll_count":driver.polls,"maximum_poll_step_seconds":driver.maximum_poll_step_seconds,"coarse_discard_diagnostics":driver.coarse_discard_diagnostics,"all_message_count":driver.messages,"all_message_debug_bytes":driver.digest.bytes,"all_message_digest_algorithm":"fnv1a64-debug-stream-v1","all_message_digest":format!("{:016x}",driver.digest.hash)});
    let mut phases = Phases::default();
    let mut metadata = None;
    for _ in 0..args.samples {
        let budget = CheckpointBudget::default();
        let initial = || budget.reserve(Cohort::SavePayload, 4096).unwrap();
        let preflight = time(&mut phases.preflight_us, || {
            engine.checkpoint_speech_cost(boundary, initial())
        })?;
        let cost = *preflight.value();
        drop(preflight);
        let dto = time(&mut phases.export_us, || {
            engine.export_speech_checkpoint(boundary, initial())
        })?;
        let counts = dto.value().counts(context);
        assert_eq!(
            counts.characters,
            match args.mode {
                Mode::Authored => 520,
                Mode::Populated => 2520,
            }
        );
        assert_eq!(counts.accepted_recordings, 3);
        assert_eq!(counts.parked, 1);
        assert_eq!(counts.batch_pending, 2);
        assert_eq!(counts.streams, 2);
        assert_eq!(counts.captures, 3);
        assert_eq!(counts.available_texts, 1);
        assert_eq!(counts.available_text_bytes, draft_text.len());
        assert_eq!(counts.semantic_receipts, 3);
        assert_eq!(counts.unique_roots, 2);
        let bytes = time(&mut phases.encode_us, || dto.encode())?;
        let wire: serde_json::Value = serde_json::from_slice(bytes.value())?;
        let input = budget.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)?;
        let decoded = time(&mut phases.decode_validate_us, || {
            EngineSpeechDtoV1::decode(bytes.value(), input, context)
        })?;
        let candidate = time(&mut phases.candidate_validate_us, || {
            decoded.into_candidate(context)
        })?;
        assert_eq!(candidate.value().counts(context), counts);
        assert_eq!(
            candidate.value().streams()[0].available_text(),
            Some(draft_text)
        );
        let current = json!({"scenario":"speech-ordinary-interrupted-inputs-v1","counts":counts,"witnesses":witnesses,"boundary_speech":wire,"cost":cost,"shared_reserved_peak_excluding_running_bytes":budget.retained_bytes()});
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
#[derive(Default, Serialize)]
struct SttCalls {
    available: usize,
    batch: Vec<(u64, PathBuf, cathedral_sim::SttBackendKind)>,
    realtime: Vec<String>,
    inspected: Vec<PathBuf>,
    discarded: Vec<PathBuf>,
}
struct Stt(Arc<std::sync::Mutex<SttCalls>>);
impl cathedral_sim::Transcription for Stt {
    fn available(&self, _: cathedral_sim::SttBackendKind) -> bool {
        self.0.lock().unwrap().available += 1;
        true
    }
    fn submit_batch(
        &mut self,
        job: cathedral_sim::TranscriptionJobId,
        path: PathBuf,
        kind: cathedral_sim::SttBackendKind,
    ) -> Result<(), cathedral_sim::SttSubmitError> {
        self.0.lock().unwrap().batch.push((job.0, path, kind));
        Ok(())
    }
    fn realtime_begin(&mut self, key: &str) -> bool {
        self.0.lock().unwrap().realtime.push(format!("begin:{key}"));
        true
    }
    fn realtime_append(&mut self, key: &str, samples: &[i16]) -> bool {
        self.0
            .lock()
            .unwrap()
            .realtime
            .push(format!("append:{key}:{}", samples.len()));
        true
    }
    fn realtime_commit(&mut self, key: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .realtime
            .push(format!("commit:{key}"));
        true
    }
    fn realtime_clear(&mut self, key: &str) {
        self.0.lock().unwrap().realtime.push(format!("clear:{key}"));
    }
    fn recording_seconds(&self, path: &std::path::Path) -> Option<f64> {
        self.0.lock().unwrap().inspected.push(path.to_owned());
        Some(0.01)
    }
    fn discard_recording(&mut self, path: &std::path::Path) {
        self.0.lock().unwrap().discarded.push(path.to_owned());
    }
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
