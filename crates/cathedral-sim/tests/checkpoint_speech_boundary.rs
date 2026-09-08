//! Independent public boundaries for the interrupted-input checkpoint.
//! This value is read-only; complete load interruption/adoption is a later cut.
mod prompt_support;

use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

use cathedral_sim::{
    Capabilities, Engine, EngineCommand, EngineConfig, EngineMessage, FakeCognition, NullSight,
    NullTts, RealtimeResult, SpeechRouter, SttBackendKind, SttSubmitError, Transcription,
    TranscriptionJobId, TranscriptionOutcome, TtsBackendKind, Vec3, WorldSeed,
    checkpoint::{Admitted, CheckpointBudget, Cohort, MAX_RESIDENT_BYTES},
    engine::speech_checkpoint::{EngineSpeechCandidate, EngineSpeechDtoV1},
    receipts::{CommandId, HOST_PRODUCER, OperationId, Outcome, ReceiptState},
    speech_router::checkpoint::{
        InputPurpose, InterruptionStatus, RecordingSource, SpeechCheckpointContext,
        SpeechRouterDtoV1,
    },
    timeline::LogicalTime,
};
use serde_json::{Value, json};

fn at(now: f64) -> LogicalTime {
    LogicalTime::new(now).unwrap()
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Calls {
    available: usize,
    batch: Vec<(TranscriptionJobId, PathBuf, SttBackendKind)>,
    realtime: Vec<String>,
    inspected: Vec<PathBuf>,
    discarded: Vec<PathBuf>,
}

#[derive(Clone, Default)]
struct Probe(Rc<RefCell<Calls>>);
impl Transcription for Probe {
    fn available(&self, _: SttBackendKind) -> bool {
        self.0.borrow_mut().available += 1;
        true
    }
    fn submit_batch(
        &mut self,
        job: TranscriptionJobId,
        path: PathBuf,
        kind: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        self.0.borrow_mut().batch.push((job, path, kind));
        Ok(())
    }
    fn realtime_begin(&mut self, key: &str) -> bool {
        self.0.borrow_mut().realtime.push(format!("begin:{key}"));
        true
    }
    fn realtime_append(&mut self, key: &str, _: &[i16]) -> bool {
        self.0.borrow_mut().realtime.push(format!("append:{key}"));
        true
    }
    fn realtime_commit(&mut self, key: &str) -> bool {
        self.0.borrow_mut().realtime.push(format!("commit:{key}"));
        true
    }
    fn realtime_clear(&mut self, key: &str) {
        self.0.borrow_mut().realtime.push(format!("clear:{key}"));
    }
    fn recording_seconds(&self, path: &std::path::Path) -> Option<f64> {
        self.0.borrow_mut().inspected.push(path.to_owned());
        Some(0.01)
    }
    fn discard_recording(&mut self, path: &std::path::Path) {
        self.0.borrow_mut().discarded.push(path.to_owned());
    }
}

struct Harness {
    engine: Engine,
    probe: Probe,
    sequence: i64,
}
impl Harness {
    fn new(fake_mode: bool) -> Self {
        let probe = Probe::default();
        let engine = Engine::new(
            EngineConfig {
                fake_mode,
                stt_stream_grace_seconds: 2.0,
                ..EngineConfig::default()
            },
            &WorldSeed::from_json_str(&prompt_support::demo_seed()).unwrap(),
            prompt_support::areas(),
            prompt_support::catalog(),
            prompt_support::prompt_env(),
            Box::new(FakeCognition::default()),
            Box::new(probe.clone()),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(false, true, true, false, false, TtsBackendKind::Off),
            (Vec3::ZERO, 0.0),
            0,
            0.0,
        )
        .unwrap();
        let mut h = Self {
            engine,
            probe,
            sequence: 0,
        };
        h.engine.poll(0.0, vec![]);
        h
    }
    fn send(&mut self, command: EngineCommand) -> Vec<EngineMessage> {
        self.engine.poll(0.0, vec![command])
    }
    fn stream(&mut self, name: &str) {
        self.send(EngineCommand::PlayerAudioBegin {
            wav_basename: name.into(),
            sample_rate: 24_000,
        });
        self.send(EngineCommand::PlayerAudioChunk {
            wav_basename: name.into(),
            seq: 0,
            samples: Arc::from([1i16, 2, 3]),
        });
        self.send(EngineCommand::PlayerAudioEnd {
            wav_basename: name.into(),
            chunk_count: 1,
            silent: false,
        });
    }
    fn draft(&mut self, name: &str, text: &str) {
        self.stream(name);
        let output = self.send(EngineCommand::Transcription(
            TranscriptionOutcome::Realtime(RealtimeResult::Transcript {
                key: name.into(),
                text: text.into(),
            }),
        ));
        assert!(
            !output
                .iter()
                .any(|m| matches!(m, EngineMessage::Speech { .. }))
        );
    }
    fn recording(&mut self, name: &str, request: &str, id: CommandId) {
        self.sequence += 1;
        self.send(
            EngineCommand::PlayerRecording {
                request_id: request.into(),
                wav_basename: name.into(),
                stt_backend: SttBackendKind::Cloud,
                position_m: Vec3::ZERO,
                spatial_seq: self.sequence,
            }
            .identified(id),
        );
        assert_eq!(
            self.engine
                .world()
                .command_ledger
                .get(id)
                .unwrap()
                .outcome
                .state,
            ReceiptState::Accepted
        );
    }
}

fn save(engine: &Engine, budget: &CheckpointBudget) -> Admitted<Vec<u8>> {
    engine
        .export_speech_checkpoint(at(0.0), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn candidate(
    engine: &Engine,
    raw: &[u8],
    budget: &CheckpointBudget,
) -> Admitted<EngineSpeechCandidate> {
    let context = engine.speech_checkpoint_context(at(0.0));
    EngineSpeechDtoV1::decode(
        raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap()
}
fn wire(engine: &Engine) -> Value {
    serde_json::from_slice(save(engine, &CheckpointBudget::default()).value()).unwrap()
}
fn refused(engine: &Engine, bytes: &[u8], now: f64) {
    let budget = CheckpointBudget::default();
    assert!(
        EngineSpeechDtoV1::decode(
            bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            engine.speech_checkpoint_context(at(now)),
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn unsubmitted_stream_text_keeps_exact_bytes_without_replaying_committed_speech() {
    let mut h = Harness::new(false);
    let output = h.send(EngineCommand::PlayerSay {
        request_id: "committed".into(),
        text: "An earlier intentional line.".into(),
        position_m: Vec3::ZERO,
        spatial_seq: 1,
    });
    assert!(
        output
            .iter()
            .any(|m| matches!(m, EngineMessage::Speech { .. }))
    );
    let basename = format!("{}.WaV", "é".repeat(cathedral_sim::MAX_ID_CHARS - 4));
    let text = format!(" \tDraft\u{0085}{}\n ", "🙂".repeat(600));
    h.draft(&basename, &text);
    let calls = h.probe.0.borrow().clone();
    let transcript = h.engine.transcript().to_vec();
    let revision = h.engine.world().world_revision;
    let before = wire(&h.engine);
    let budget = CheckpointBudget::default();
    let bytes = save(&h.engine, &budget);
    let restored = candidate(&h.engine, bytes.value(), &budget);
    let value = restored.value();
    assert_eq!(value.purpose(), InputPurpose::PublicPlayerSpeech);
    assert_eq!(value.status(), InterruptionStatus::InterruptedUnsent);
    assert_eq!(value.captures(), &[basename.clone()]);
    assert_eq!(value.streams().len(), 1);
    assert_eq!(value.streams()[0].basename(), basename);
    assert_eq!(value.streams()[0].available_text(), Some(text.as_str()));
    assert!(value.accepted_recordings().is_empty());
    assert_eq!(value.semantic_ids().count(), 0);
    let counts = value.counts(h.engine.speech_checkpoint_context(at(0.0)));
    assert_eq!(counts.available_text_bytes, text.len());
    assert_eq!(counts.available_texts, 1);
    assert_eq!(h.engine.transcript(), transcript);
    assert_eq!(h.engine.world().world_revision, revision);
    assert_eq!(*h.probe.0.borrow(), calls);
    assert_eq!(wire(&h.engine), before);
    drop((restored, bytes));
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn reused_filenames_keep_sibling_command_steps_and_terminal_receipts_separate() {
    let mut h = Harness::new(false);
    let root = OperationId {
        producer: HOST_PRODUCER,
        sequence: 1,
    };
    let first = root.command(0);
    let second = root.command(1);
    h.recording("shared.wav", "same correlation", first);
    h.stream("shared.wav");
    h.recording("shared.wav", "same correlation", second);
    let terminal = h
        .engine
        .world_mut()
        .command_ledger
        .advance(
            second,
            0.0,
            Outcome::new(ReceiptState::Interrupted, "stopped", "Already stopped"),
        )
        .unwrap();
    // Complete this direct test edit's notification boundary without an extra
    // Engine poll. Ordinary command processing performs this flush itself.
    let updates = h.engine.world_mut().command_ledger.drain_updates();
    assert_eq!(updates, [terminal.clone()]);
    let budget = CheckpointBudget::default();
    let raw = save(&h.engine, &budget);
    let restored = candidate(&h.engine, raw.value(), &budget);
    let rows = restored.value().accepted_recordings();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source(), RecordingSource::BatchPending);
    assert_eq!(rows[0].semantic(), Some(first));
    assert_eq!(rows[0].parked_deadline(), None);
    assert_eq!(rows[1].source(), RecordingSource::Parked);
    assert_eq!(rows[1].semantic(), Some(second));
    assert_eq!(rows[1].parked_deadline(), Some(2.0));
    assert_eq!(rows[1].receipt(), Some(&terminal));
    for row in rows {
        assert_eq!(row.basename(), "shared.wav");
        assert_eq!(row.request_id(), "same correlation");
        assert_eq!(row.position_m(), Vec3::ZERO);
        assert_eq!(row.backend(), SttBackendKind::Cloud);
    }
    assert_eq!(
        restored.value().semantic_ids().collect::<Vec<_>>(),
        [first, second]
    );
    let counts = restored
        .value()
        .counts(h.engine.speech_checkpoint_context(at(0.0)));
    assert_eq!(counts.unique_roots, 1);
    assert_eq!(counts.semantic_receipts, 2);
    assert_eq!(counts.terminal_receipts, 1);
    assert_eq!(h.probe.0.borrow().batch.len(), 1);
    assert!(h.engine.transcript().is_empty());
    drop(restored);

    let original: Value = serde_json::from_slice(raw.value()).unwrap();
    let mut changed = original.clone();
    changed["state"]["accepted_recordings"][1]["receipt"]["outcome"]["message"] = json!("invented");
    refused(&h.engine, &serde_json::to_vec(&changed).unwrap(), 0.0);
    let mut changed = original;
    changed["state"]["accepted_recordings"][1]["semantic"] =
        changed["state"]["accepted_recordings"][0]["semantic"].clone();
    refused(&h.engine, &serde_json::to_vec(&changed).unwrap(), 0.0);
    h.engine.world_mut().command_ledger.unprotect(root);
    refused(&h.engine, raw.value(), 0.0);
    assert!(
        h.engine
            .export_speech_checkpoint(
                at(0.0),
                CheckpointBudget::default()
                    .reserve(Cohort::SavePayload, 4096)
                    .unwrap(),
            )
            .is_err()
    );
}

#[test]
fn abandoned_fake_stream_jobs_do_not_become_eight_extra_draft_obligations() {
    let mut h = Harness::new(true);
    for _ in 0..12 {
        h.stream("reused.wav");
        h.send(EngineCommand::PlayerAudioAbort {
            wav_basename: "reused.wav".into(),
        });
    }
    assert_eq!(h.probe.0.borrow().batch.len(), 12);
    assert_eq!(h.engine.speech_router().active_stream_count(), 0);
    let empty = wire(&h.engine);
    assert_eq!(empty["state"]["streams"], json!([]));
    assert_eq!(empty["state"]["captures"], json!([]));
    assert_eq!(empty["state"]["accepted_recordings"], json!([]));
    h.stream("remaining.wav");
    let job = h.probe.0.borrow().batch.last().unwrap().0;
    h.send(EngineCommand::Transcription(TranscriptionOutcome::Done {
        job,
        result: Ok("Still an unsent draft".into()),
    }));
    let budget = CheckpointBudget::default();
    let raw = save(&h.engine, &budget);
    let restored = candidate(&h.engine, raw.value(), &budget);
    assert_eq!(restored.value().streams().len(), 1);
    assert_eq!(
        restored.value().streams()[0].available_text(),
        Some("Still an unsent draft")
    );
    assert!(restored.value().accepted_recordings().is_empty());
    assert!(h.engine.transcript().is_empty());
}

#[test]
fn closed_records_reject_malformed_purpose_text_deadlines_and_player_binding() {
    let mut h = Harness::new(false);
    h.draft("draft.wav", "text");
    h.recording(
        "batch.wav",
        "",
        OperationId {
            producer: HOST_PRODUCER,
            sequence: 1,
        }
        .command(0),
    );
    let original = wire(&h.engine);
    let mut variants = Vec::new();
    let mut v = original.clone();
    v["state"].as_object_mut().unwrap().remove("purpose");
    variants.push(v);
    let mut v = original.clone();
    v["state"]["purpose"] = json!("submit_automatically");
    variants.push(v);
    let mut v = original.clone();
    v["state"]["status"] = json!("ready_to_speak");
    variants.push(v);
    let mut v = original.clone();
    v["state"]["old_job"] = json!(123);
    variants.push(v);
    let mut v = original.clone();
    v["state"]["streams"][0]
        .as_object_mut()
        .unwrap()
        .remove("available_text");
    variants.push(v);
    let mut v = original.clone();
    v["state"]["accepted_recordings"][0]
        .as_object_mut()
        .unwrap()
        .remove("parked_deadline");
    variants.push(v);
    let mut v = original.clone();
    v["state"]["accepted_recordings"][0]["parked_deadline"] = json!({"bits":0});
    variants.push(v);
    let mut v = original.clone();
    v["state"]["accepted_recordings"][0]["receipt"] = Value::Null;
    variants.push(v);
    let mut v = original.clone();
    v["state"]["accepted_recordings"][0]["position_m"]["x"] = json!(1_000_001.0);
    variants.push(v);
    let mut v = original.clone();
    v["state"]["accepted_recordings"][0]["request_id"] = json!("é".repeat(32_769));
    variants.push(v);
    let mut v = original.clone();
    v["state"]["streams"][0]["available_text"] = json!("🙂".repeat(100_001));
    variants.push(v);
    let mut v = original.clone();
    v["state"]["streams"][0]["basename"] = json!(format!(
        "{}.wav",
        "é".repeat(cathedral_sim::MAX_ID_CHARS - 3)
    ));
    variants.push(v);
    let mut v = original.clone();
    v["state"]["captures"] = json!((0..9).map(|i| format!("{i}.wav")).collect::<Vec<_>>());
    variants.push(v);
    let mut v = original.clone();
    v["state"]["streams"] = json!(
        (0..9)
            .map(|i| json!({"basename":format!("{i}.wav"),"available_text":null}))
            .collect::<Vec<_>>()
    );
    variants.push(v);
    let mut v = original.clone();
    v["player_id"] = json!("missing-player");
    variants.push(v);
    for variant in variants {
        assert_ne!(variant, original);
        refused(&h.engine, &serde_json::to_vec(&variant).unwrap(), 0.0);
    }
    let canonical = serde_json::to_string(&original).unwrap();
    let duplicate = canonical.replace("\"version\":1", "\"version\":1,\"version\":1");
    assert_ne!(canonical, duplicate);
    refused(&h.engine, duplicate.as_bytes(), 0.0);
    refused(&h.engine, canonical.as_bytes(), -0.0);
}

#[test]
fn raw_padding_and_maximum_available_text_remain_charged_through_candidate_lifetime() {
    let mut h = Harness::new(false);
    let text = "🙂".repeat(100_000);
    h.draft("large.wav", &text);
    let canonical = serde_json::to_vec(&wire(&h.engine)).unwrap();
    let padding = 1024 * 1024;
    let mut raw = vec![b' '; padding];
    raw.extend_from_slice(&canonical);
    let budget = CheckpointBudget::default();
    let context = h.engine.speech_checkpoint_context(at(0.0));
    let decoded = EngineSpeechDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained >= canonical_peak + 3 * padding);
    let restored = decoded.into_candidate(context).unwrap();
    assert_eq!(budget.retained_bytes(), retained);
    assert_eq!(
        restored.value().streams()[0].available_text(),
        Some(text.as_str())
    );
    drop(restored);
    assert_eq!(budget.retained_bytes(), 0);
    let lexical = raw.len() + 4096;
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - lexical)
        .unwrap();
    assert!(
        EngineSpeechDtoV1::decode(
            &raw,
            budget.reserve(Cohort::LoadCandidate, lexical).unwrap(),
            context,
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES - lexical);
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn standalone_router_keeps_raw_grace_bits_and_required_nullable_draft_text() {
    let world = prompt_support::seed_world();
    for bits in [
        0,
        (-0.0f64).to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8_0000_0000_0849,
    ] {
        let router = SpeechRouter::new(f64::from_bits(bits));
        let budget = CheckpointBudget::default();
        let context = SpeechCheckpointContext::from_world(&world, at(-0.0));
        let raw = router
            .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .unwrap()
            .encode()
            .unwrap();
        let restored = SpeechRouterDtoV1::decode(
            raw.value(),
            budget
                .reserve(Cohort::LoadCandidate, raw.value().len() + 4096)
                .unwrap(),
            context,
        )
        .unwrap()
        .into_candidate(context)
        .unwrap();
        assert_eq!(restored.value().stt_stream_grace_seconds().to_bits(), bits);
        assert!(restored.value().streams().is_empty());
        assert!(restored.value().captures().is_empty());
        drop((restored, raw));
        assert_eq!(budget.retained_bytes(), 0);
    }
    let mut h = Harness::new(false);
    h.send(EngineCommand::PlayerAudioBegin {
        wav_basename: "onset.wav".into(),
        sample_rate: 24_000,
    });
    let budget = CheckpointBudget::default();
    let raw = save(&h.engine, &budget);
    let restored = candidate(&h.engine, raw.value(), &budget);
    assert_eq!(restored.value().streams()[0].available_text(), None);
    assert_eq!(restored.value().captures(), &["onset.wav".to_owned()]);
}
