use super::*;
use crate::{
    checkpoint::{CheckpointBudget, Cohort},
    receipts::{Admission, CommandLedgerDtoV1, HOST_PRODUCER, Outcome},
};
use serde_json::{Value, json};
fn at(t: f64) -> LogicalTime {
    LogicalTime::new(t).unwrap()
}
fn save() -> Reservation {
    CheckpointBudget::default()
        .reserve(Cohort::SavePayload, 4096)
        .unwrap()
}
fn context(w: &World) -> SpeechCheckpointContext<'_> {
    SpeechCheckpointContext::from_world(w, at(10.0))
}
fn bytes(r: &SpeechRouter, w: &World) -> Vec<u8> {
    r.export_checkpoint(context(w), save())
        .unwrap()
        .encode()
        .unwrap()
        .value()
        .clone()
}
fn decode(raw: &[u8], w: &World) -> Result<Admitted<SpeechRouterDtoV1>> {
    SpeechRouterDtoV1::decode(
        raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        context(w),
    )
}
fn task(w: &World, id: Option<CommandId>) -> TranscriptionTask {
    TranscriptionTask {
        semantic: id,
        request_id: "reused request".into(),
        basename: "same.wav".into(),
        position_m: Vec3::new(-0.0, 0.0, 1.0),
        backend: SttBackendKind::Cloud,
        attention: Conversation::default().capture(0.0, w),
    }
}
fn pending(w: &mut World, op: Option<crate::receipts::OperationId>, step: u16) -> CommandId {
    let op = op.unwrap_or_else(|| w.command_ledger.issue(HOST_PRODUCER).unwrap());
    w.command_ledger.protect(op).unwrap();
    let id = op.command(step);
    let Admission::New(ticket) = w
        .command_ledger
        .begin(id, &json!({"recording":"same.wav","step":step}))
    else {
        panic!("fresh receipt")
    };
    w.command_ledger.finish(
        ticket,
        -0.0,
        Outcome::new(ReceiptState::Accepted, "utterance_accepted", "awaiting"),
        vec![crate::receipts::AffectedRef::new("actor", "historical").unwrap()],
    );
    w.speech_actions.insert(id);
    id
}
fn fixture() -> (SpeechRouter, World) {
    let mut w = World::new();
    let a = pending(&mut w, None, 0);
    let b = pending(&mut w, Some(a.operation), 1);
    let mut r = SpeechRouter::new(f64::from_bits(0x7ff8000000000055));
    r.recording_jobs
        .push((TranscriptionJobId(77), task(&w, Some(a))));
    r.parked.push((
        "same.wav".into(),
        ParkedRecording {
            task: task(&w, Some(b)),
            deadline: -0.0,
        },
    ));
    let mut stream = StreamState::new();
    stream.phase = StreamPhase::Completed;
    stream.transcript = Some("  available \n draft — no automatic say  ".into());
    stream.completed_at = Some(9.0);
    r.streams.push(("draft.wav".into(), stream));
    r.captures
        .push(("draft.wav".into(), Conversation::default().capture(9.0, &w)));
    (r, w)
}

#[test]
fn continuation_interrupted_ordinals_remain_unique_after_eviction_and_across_live_families() {
    let (router, mut world) = fixture();
    let saved = router.export_checkpoint(context(&world), save()).unwrap();
    let state = saved.value().state.clone();
    drop(saved);
    let mut groups = Vec::new();
    for recording in &state.accepted_recordings {
        let id = recording.semantic.unwrap();
        let terminal = world
            .command_ledger
            .advance(
                id,
                10.0,
                Outcome::new(
                    ReceiptState::Interrupted,
                    INTERRUPTION_CODE,
                    INTERRUPTION_MESSAGE,
                ),
            )
            .unwrap();
        let mut one = state.clone();
        one.captures.clear();
        one.streams.clear();
        one.accepted_recordings = vec![recording.clone()];
        groups.push(InterruptedSpeech::new(at(10.0), one, vec![terminal]));
        world.speech_actions.remove(&id);
    }
    world
        .command_ledger
        .unprotect(groups[0].receipts[0].id.operation);
    world.command_ledger.drain_updates();
    for _ in 0..crate::receipts::RECENT_CAPACITY + 2 {
        let id = world
            .command_ledger
            .issue(HOST_PRODUCER)
            .unwrap()
            .command(0);
        let Admission::New(ticket) = world
            .command_ledger
            .begin(id, &json!({"ordinary":"completed"}))
        else {
            panic!("new command")
        };
        world.command_ledger.finish(
            ticket,
            10.0,
            Outcome::new(ReceiptState::Completed, "done", "done"),
            vec![],
        );
    }
    world.command_ledger.drain_updates();
    for g in &groups {
        assert!(world.command_ledger.get(g.receipts[0].id).is_none());
        assert!(world.command_ledger.valid_history_receipt(&g.receipts[0]));
    }
    validate_interrupted(&groups, context(&world)).unwrap();
    let mut contradictory = groups.clone();
    let ordinal = contradictory[0].receipts[0].ordinal;
    contradictory[1].receipts[0].ordinal = ordinal;
    contradictory[1].state.accepted_recordings[0]
        .receipt
        .as_mut()
        .unwrap()
        .ordinal = ordinal;
    // Each history is independently valid and both IDs have actually evicted.
    // Only their contradictory shared ordinal should reject the pair.
    for g in &contradictory {
        g.validate(context(&world)).unwrap();
    }
    assert!(
        validate_interrupted(&contradictory, context(&world))
            .unwrap_err()
            .reason
            .contains("ordinal")
    );
    let mut together = contradictory[0].clone();
    together
        .state
        .accepted_recordings
        .extend(contradictory[1].state.accepted_recordings.clone());
    together.receipts.extend(contradictory[1].receipts.clone());
    assert!(
        validate_interrupted(&[together], context(&world))
            .unwrap_err()
            .reason
            .contains("ordinal")
    );

    // A different command family owns a current retained ordinal. Archived
    // speech cannot take it, even though that speech command itself evicted.
    let live = world
        .command_ledger
        .issue(crate::receipts::TURN_PRODUCER)
        .unwrap()
        .command(0);
    let Admission::New(ticket) = world
        .command_ledger
        .begin(live, &json!({"other_family":true}))
    else {
        panic!("new command")
    };
    let live = world.command_ledger.finish(
        ticket,
        10.0,
        Outcome::new(ReceiptState::Completed, "done", "done"),
        vec![],
    );
    let mut collision = groups[0].clone();
    collision.receipts[0].ordinal = live.ordinal;
    collision.state.accepted_recordings[0]
        .receipt
        .as_mut()
        .unwrap()
        .ordinal = live.ordinal;
    assert!(
        !world
            .command_ledger
            .valid_history_receipt(&collision.receipts[0])
    );
    assert!(
        collision
            .validate(context(&world))
            .unwrap_err()
            .reason
            .contains("history")
    );
    let ledger = world
        .command_ledger
        .checkpoint_v1(
            at(10.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, CommandLedgerDtoV1::WORKING_BYTES)
                .unwrap(),
        )
        .unwrap();
    assert!(!ledger.value().valid_history_receipt(&collision.receipts[0]));
    for group in groups {
        assert!(ledger.value().valid_history_receipt(&group.receipts[0]));
    }
}
#[test]
fn checkpoint_speech_preserves_occurrences_siblings_and_exact_drafts() {
    let (r, w) = fixture();
    let raw = bytes(&r, &w);
    let c = decode(&raw, &w)
        .unwrap()
        .into_candidate(context(&w))
        .unwrap();
    let c = c.value();
    assert_eq!(c.accepted_recordings().len(), 2);
    assert_eq!(
        c.accepted_recordings()[0].basename(),
        c.accepted_recordings()[1].basename()
    );
    assert_eq!(c.counts(context(&w)).unique_roots, 1);
    assert_eq!(c.counts(context(&w)).semantic_receipts, 2);
    assert_eq!(
        c.accepted_recordings()[0].receipt().unwrap().at.to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(
        c.accepted_recordings()[1]
            .parked_deadline()
            .unwrap()
            .to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(
        c.streams()[0].available_text(),
        Some("  available \n draft — no automatic say  ")
    );
    assert_eq!(
        c.stt_stream_grace_seconds().to_bits(),
        r.stt_stream_grace_seconds.to_bits()
    );
    assert_eq!(
        serde_json::to_vec(decode(&raw, &w).unwrap().value()).unwrap(),
        raw
    );
}
#[test]
fn checkpoint_speech_live_owner_missing_extra_duplicate_and_unprotected_refuse() {
    let (mut r, mut w) = fixture();
    let good = bytes(&r, &w);
    let id = r.recording_jobs[0].1.semantic.unwrap();
    w.speech_actions.remove(&id);
    assert!(r.export_checkpoint(context(&w), save()).is_err());
    assert!(decode(&good, &w).is_err());
    w.speech_actions.insert(id);
    w.speech_actions.insert(id.operation.command(2));
    assert!(decode(&good, &w).is_err());
    w.speech_actions.remove(&id.operation.command(2));
    r.parked[0].1.task.semantic = Some(id);
    assert!(r.export_checkpoint(context(&w), save()).is_err());
    w.command_ledger.unprotect(id.operation);
    assert!(decode(&good, &w).is_err());
}
#[test]
fn checkpoint_speech_terminal_receipts_survive_and_revalidation_checks_exact_snapshot() {
    for state in [
        ReceiptState::Accepted,
        ReceiptState::InProgress,
        ReceiptState::Completed,
        ReceiptState::Interrupted,
        ReceiptState::Superseded,
        ReceiptState::Rejected,
    ] {
        let (r, mut w) = fixture();
        let id = r.recording_jobs[0].1.semantic.unwrap();
        w.command_ledger
            .advance(id, 1.0, Outcome::new(state, "state", "exact outcome"))
            .unwrap();
        w.command_ledger.drain_updates();
        let raw = bytes(&r, &w);
        let d = decode(&raw, &w).unwrap();
        assert_eq!(
            d.value().counts(context(&w)).terminal_receipts,
            usize::from(!matches!(
                state,
                ReceiptState::Accepted | ReceiptState::InProgress
            ))
        );
        w.command_ledger
            .recent
            .get_mut(&id)
            .unwrap()
            .receipt
            .outcome
            .message
            .push('x');
        assert!(d.into_candidate(context(&w)).is_err());
    }
}
#[test]
fn checkpoint_speech_saved_backbone_ledger_binding_without_adoption() {
    let (r, w) = fixture();
    let raw = bytes(&r, &w);
    let b = w
        .export_backbone_checkpoint(save())
        .unwrap()
        .into_candidate(&w.item_catalog, &w.command_ledger)
        .unwrap();
    let ledger = w
        .command_ledger
        .checkpoint_v1(
            at(10.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, CommandLedgerDtoV1::WORKING_BYTES)
                .unwrap(),
        )
        .unwrap();
    let c = SpeechCheckpointContext::from_backbone(b.value(), at(10.0), ledger.value());
    let d = SpeechRouterDtoV1::decode(
        &raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert_eq!(d.value().counts(c).semantic_receipts, 2);
    assert_eq!(
        r.export_checkpoint(c, save())
            .unwrap()
            .encode()
            .unwrap()
            .value(),
        &raw
    );
    let empty = World::new();
    let wrong = empty
        .command_ledger
        .checkpoint_v1(
            at(10.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, CommandLedgerDtoV1::WORKING_BYTES)
                .unwrap(),
        )
        .unwrap();
    assert!(
        SpeechRouterDtoV1::decode(
            &raw,
            CheckpointBudget::default()
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            SpeechCheckpointContext::from_backbone(b.value(), at(10.0), wrong.value())
        )
        .is_err()
    );
}
#[test]
fn checkpoint_speech_refuses_resolved_and_unflushed_boundaries_without_losing_staging() {
    let (mut r, mut w) = fixture();
    let t = task(&w, None);
    r.resolved.push((t, Ok("staged text".into())));
    let before = r.clone();
    assert!(r.export_checkpoint(context(&w), save()).is_err());
    assert!(r.checkpoint_cost(context(&w), save()).is_err());
    assert_eq!(r.resolved, before.resolved);
    r.resolved.clear();
    let id = r.recording_jobs[0].1.semantic.unwrap();
    w.command_ledger
        .advance(id, 1.0, Outcome::completed("done"))
        .unwrap();
    assert!(r.export_checkpoint(context(&w), save()).is_err());
}
#[test]
fn checkpoint_speech_transient_scrambling_cannot_change_semantic_projection() {
    let (mut r, w) = fixture();
    let raw = bytes(&r, &w);
    let candidate = decode(&raw, &w)
        .unwrap()
        .into_candidate(context(&w))
        .unwrap();
    r.next_job = u64::MAX;
    r.stream_jobs = (0..1000)
        .map(|i| (TranscriptionJobId(i), "old.wav".into()))
        .collect();
    r.tts_backends = (0..64)
        .map(|_| (SpeechEventId("duplicate".into()), TtsBackendKind::Cloud))
        .collect();
    r.timings = (0..64)
        .map(|_| {
            (
                "stale".into(),
                UtteranceTiming {
                    path: "discard".into(),
                    endpoint_at: f64::NAN,
                    audio_seconds: Some(f64::INFINITY),
                    commit_at: None,
                    completed_at: Some(-99.0),
                },
            )
        })
        .collect();
    r.streams[0].1.next_seq = u32::MAX;
    r.streams[0].1.decoded_bytes = u64::MAX;
    r.streams[0].1.status_sent = true;
    r.streams[0].1.degrade_reason = Some(DegradeReason::Session("stale".into()));
    r.captures[0].1 = Conversation::default().capture(999.0, &w);
    r.recording_jobs[0].1.attention = r.captures[0].1.clone();
    assert_eq!(bytes(&r, &w), raw);
    r.streams.clear();
    r.captures.clear();
    r.recording_jobs.clear();
    r.parked.clear();
    assert_eq!(candidate.value().accepted_recordings().len(), 2);
    assert_eq!(
        candidate.value().streams()[0].available_text(),
        Some("  available \n draft — no automatic say  ")
    );
}
#[test]
fn checkpoint_speech_optional_semantic_and_all_raw_provenance_bits() {
    let w = World::new();
    for bits in [
        0,
        1,
        (-0.0f64).to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8000000000055,
        u64::MAX,
    ] {
        let mut r = SpeechRouter::new(f64::from_bits(bits));
        r.parked.push((
            "same.wav".into(),
            ParkedRecording {
                task: task(&w, None),
                deadline: f64::from_bits(bits),
            },
        ));
        let raw = bytes(&r, &w);
        let c = decode(&raw, &w)
            .unwrap()
            .into_candidate(context(&w))
            .unwrap();
        assert_eq!(c.value().stt_stream_grace_seconds().to_bits(), bits);
        assert_eq!(
            c.value().accepted_recordings()[0]
                .parked_deadline()
                .unwrap()
                .to_bits(),
            bits
        );
        assert!(c.value().accepted_recordings()[0].receipt().is_none());
    }
}
#[test]
fn checkpoint_speech_strict_every_record_required_unknown_duplicate_nullable() {
    let (r, w) = fixture();
    let raw = bytes(&r, &w);
    let v: Value = serde_json::from_slice(&raw).unwrap();
    for pointer in [
        "",
        "/state",
        "/state/stt_stream_grace_seconds",
        "/state/streams/0",
        "/state/accepted_recordings/0",
        "/state/accepted_recordings/0/position_m",
        "/state/accepted_recordings/0/semantic",
        "/state/accepted_recordings/0/semantic/operation",
        "/state/accepted_recordings/0/receipt",
        "/state/accepted_recordings/0/receipt/id",
        "/state/accepted_recordings/0/receipt/id/operation",
        "/state/accepted_recordings/0/receipt/outcome",
        "/state/accepted_recordings/0/receipt/affected/0",
        "/state/accepted_recordings/1/parked_deadline",
    ] {
        for key in v.pointer(pointer).unwrap().as_object().unwrap().keys() {
            let mut bad = v.clone();
            bad.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                decode(&serde_json::to_vec(&bad).unwrap(), &w).is_err(),
                "missing {pointer}/{key}"
            );
        }
        let mut bad = v.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("alien".into(), json!(1));
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap(), &w).is_err(),
            "unknown {pointer}"
        );
    }
    let text = String::from_utf8(raw.clone()).unwrap();
    for key in [
        "version",
        "status",
        "bits",
        "available_text",
        "semantic",
        "receipt",
        "request_id",
        "source",
        "at",
        "ordinal",
        "code",
        "affected",
        "x",
        "producer",
        "step",
    ] {
        let needle = format!("\"{key}\":");
        let (before, after) = text.split_once(&needle).unwrap();
        let bad = format!("{before}{needle}null,{needle}{after}");
        assert!(decode(bad.as_bytes(), &w).is_err(), "duplicate {key}");
    }
    let mut bad = v.clone();
    bad["state"]["accepted_recordings"][0]["receipt"] = Value::Null;
    assert!(decode(&serde_json::to_vec(&bad).unwrap(), &w).is_err());
    let mut bad = v;
    bad["state"]["accepted_recordings"][0]["semantic"] = Value::Null;
    assert!(decode(&serde_json::to_vec(&bad).unwrap(), &w).is_err());
}
#[test]
fn checkpoint_speech_max_counts_text_and_sparse_capacity_proof() {
    let w = World::new();
    let mut r = SpeechRouter::default();
    r.streams = Vec::with_capacity(100_000);
    r.captures = Vec::with_capacity(100_000);
    r.recording_jobs = Vec::with_capacity(100_000);
    for i in 0..8 {
        let mut s = StreamState::new();
        s.transcript = Some("x".repeat(MAX_TEXT_BYTES));
        r.streams.push((format!("s{i}.wav"), s));
        r.captures.push((
            format!("c{i}.wav"),
            Conversation::default().capture(0.0, &w),
        ));
        let mut t = task(&w, None);
        t.request_id = "y".repeat(MAX_REQUEST_BYTES);
        r.recording_jobs.push((TranscriptionJobId(i), t));
    }
    let raw = bytes(&r, &w);
    let d = decode(&raw, &w).unwrap();
    assert_eq!(d.value().state.accepted_recordings.len(), 8);
    assert!(d.value().state.streams.capacity() <= 16);
    assert!(d.value().state.captures.capacity() <= 16);
    let cost = d.value().cost().unwrap();
    assert!(cost.peak_bytes < crate::checkpoint::MAX_RESIDENT_BYTES);
    r.streams[0].1.transcript.as_mut().unwrap().push('x');
    assert!(r.export_checkpoint(context(&w), save()).is_err());
    r.streams[0].1.transcript = Some(String::new());
    r.recording_jobs[0].1.request_id.push('x');
    assert!(r.export_checkpoint(context(&w), save()).is_err());
    r.recording_jobs[0].1.request_id.clear();
    r.parked.push((
        "same.wav".into(),
        ParkedRecording {
            task: task(&w, None),
            deadline: 0.0,
        },
    ));
    assert!(r.export_checkpoint(context(&w), save()).is_err());
}
#[test]
fn checkpoint_speech_raw_charge_and_failure_release() {
    let (r, w) = fixture();
    let mut raw = bytes(&r, &w);
    raw.extend(vec![b' '; 200_000]);
    let budget = CheckpointBudget::default();
    let d = SpeechRouterDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        context(&w),
    )
    .unwrap();
    let charge = d.reserved_bytes();
    let c = d.into_candidate(context(&w)).unwrap();
    assert_eq!(c.reserved_bytes(), charge);
    assert!(charge >= 3 * raw.len() + VALIDATION_WORKING_BYTES);
    drop(c);
    assert_eq!(budget.retained_bytes(), 0);
    let hold = budget
        .reserve(
            Cohort::Running,
            crate::checkpoint::MAX_RESIDENT_BYTES - raw.len() - 4096,
        )
        .unwrap();
    assert!(
        SpeechRouterDtoV1::decode(
            &raw,
            budget
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            context(&w)
        )
        .is_err()
    );
    drop(hold);
    assert_eq!(budget.retained_bytes(), 0);
}
#[test]
fn checkpoint_speech_layout_bound() {
    use std::mem::size_of;
    let layouts = json!({"accepted":size_of::<AcceptedRecording>(),"stream":size_of::<InterruptedStream>(),"receipt":size_of::<Receipt>(),"state":size_of::<StateV1>(),"dto":size_of::<SpeechRouterDtoV1>(),"wire":size_of::<Wire>(),"candidate":size_of::<SpeechCandidate>(),"context":size_of::<SpeechCheckpointContext>(),"view":size_of::<StateView>(),"recording_view":size_of::<RecordingView>(),"string":size_of::<String>(),"affected":size_of::<crate::receipts::AffectedRef>()});
    println!("speech_layout={layouts}");
    assert!(size_of::<AcceptedRecording>() <= 512);
    assert!(size_of::<StateV1>() <= 512);
    assert!(size_of::<SpeechRouterDtoV1>() <= 512);
    assert!(size_of::<InterruptedStream>() <= 64);
    assert!(size_of::<crate::receipts::AffectedRef>() <= 64);
}
#[test]
#[ignore = "new component fixture authoring only"]
fn checkpoint_speech_generate_component_fixtures() {
    let (r, w) = fixture();
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::write(
        base.join("checkpoint_speech_pending_v1.json"),
        bytes(&r, &w),
    )
    .unwrap();
    let w = World::new();
    std::fs::write(
        base.join("checkpoint_speech_empty_v1.json"),
        bytes(&SpeechRouter::default(), &w),
    )
    .unwrap();
}

#[test]
fn checkpoint_speech_enum_type_receipt_principal_and_preflight_domain() {
    let (r, mut w) = fixture();
    let v: Value = serde_json::from_slice(&bytes(&r, &w)).unwrap();
    for (pointer, key) in [
        ("/state/purpose", "public_player_speech"),
        ("/state/status", "interrupted_unsent"),
        ("/state/accepted_recordings/0/source", "batch_pending"),
        ("/state/accepted_recordings/0/backend", "cloud"),
        (
            "/state/accepted_recordings/0/receipt/outcome/state",
            "accepted",
        ),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(pointer).unwrap() = json!({key:null});
        assert!(decode(&serde_json::to_vec(&bad).unwrap(), &w).is_err());
    }
    let id = r.recording_jobs[0].1.semantic.unwrap();
    for (kind, key) in [("mark", "not-a-number"), ("ward", "not-a-ward")] {
        w.command_ledger
            .recent
            .get_mut(&id)
            .unwrap()
            .receipt
            .affected = vec![crate::receipts::AffectedRef {
            kind: kind.into(),
            id: key.into(),
        }];
        assert!(r.checkpoint_cost(context(&w), save()).is_err());
        assert!(r.export_checkpoint(context(&w), save()).is_err());
    }
}
#[test]
fn checkpoint_speech_new_fixtures_pin_projection() {
    let (r, w) = fixture();
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert_eq!(
        std::fs::read(base.join("checkpoint_speech_pending_v1.json")).unwrap(),
        bytes(&r, &w)
    );
    let w = World::new();
    assert_eq!(
        std::fs::read(base.join("checkpoint_speech_empty_v1.json")).unwrap(),
        bytes(&SpeechRouter::default(), &w)
    );
}
