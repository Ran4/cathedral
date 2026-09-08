//! Independent public boundaries for floor pacing and Engine continuity.
//! Floor already has Clone; Engine candidates expose no partial installation.
mod prompt_support;

use cathedral_sim::{
    ActorId, Capabilities, Engine, EngineCommand, EngineConfig, EngineMessage, FakeCognition,
    NullSight, NullTranscription, NullTts, TtsBackendKind, Vec3, WorldSeed,
    checkpoint::{CheckpointBudget, Cohort, MAX_RESIDENT_BYTES},
    engine::continuity_checkpoint::EngineContinuityDtoV1,
    floor::{ConversationFloor, checkpoint::FloorDtoV1},
    ids::SpeechEventId,
    timeline::LogicalTime,
};
use serde_json::{Value, json};

fn at(seconds: f64) -> LogicalTime {
    LogicalTime::new(seconds).unwrap()
}
fn event(name: &str) -> SpeechEventId {
    SpeechEventId(name.to_owned())
}
fn saved_floor(floor: &ConversationFloor, now: f64) -> Vec<u8> {
    let budget = CheckpointBudget::default();
    let bytes = floor
        .export_checkpoint(at(now), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let result = bytes.value().clone();
    drop(bytes);
    assert_eq!(budget.retained_bytes(), 0);
    result
}
fn floor_round_trip(floor: &ConversationFloor, now: f64) -> (ConversationFloor, Value) {
    let bytes = saved_floor(floor, now);
    let budget = CheckpointBudget::default();
    let candidate = FloorDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        at(now),
    )
    .unwrap()
    .into_candidate(at(now))
    .unwrap();
    assert_eq!(saved_floor(candidate.value().floor(), now), bytes);
    assert_eq!(candidate.value().floor(), floor);
    // The existing bounded Floor Clone permits ordinary public continuation;
    // this does not create a production Engine adoption path.
    let restored = candidate.value().floor().clone();
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    (restored, serde_json::from_slice(&bytes).unwrap())
}
fn engine(config: EngineConfig) -> Engine {
    Engine::new(
        config,
        &WorldSeed::from_json_str(&prompt_support::demo_seed()).unwrap(),
        prompt_support::areas(),
        prompt_support::catalog(),
        prompt_support::prompt_env(),
        Box::new(FakeCognition::new()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::ZERO, 0.0),
        0,
        0.0,
    )
    .unwrap()
}
fn engine_wire(engine: &Engine, now: f64) -> Value {
    let budget = CheckpointBudget::default();
    let bytes = engine
        .export_continuity_checkpoint(at(now), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let context = engine.continuity_checkpoint_context(at(now));
    let candidate = EngineContinuityDtoV1::decode(
        bytes.value(),
        budget
            .reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap()
    .into_candidate(context)
    .unwrap();
    assert_eq!(candidate.value().player_id(), &ActorId::from_raw("player"));
    let floor: Value =
        serde_json::from_slice(&saved_floor(candidate.value().floor(), now)).unwrap();
    let wire: Value = serde_json::from_slice(bytes.value()).unwrap();
    assert_eq!(floor["state"], wire["floor"]);
    drop((bytes, candidate));
    assert_eq!(budget.retained_bytes(), 0);
    wire
}

#[test]
fn refreshed_awaits_keep_insertion_order_and_trim_before_refresh_at_capacity() {
    let mut floor = ConversationFloor::new();
    let first = event("an arbitrary event / å / not an actor");
    let second = event("");
    floor.acquire_scoped(0.0, &first, "", true, false);
    floor.acquire_scoped(0.04, &second, "", true, true);
    floor.acquire_scoped(2.0, &first, "", true, true);
    let (mut restored, wire) = floor_round_trip(&floor, 2.0);
    let rows = wire["state"]["awaiting"].as_array().unwrap();
    assert_eq!(rows[0]["event_id"], first.0);
    assert_eq!(rows[1]["event_id"], "");
    assert_eq!(rows[0]["deadline"]["at"], 10.0);
    assert_eq!(rows[1]["deadline"]["at"], 8.04);
    assert_eq!(rows[0]["blocks_player_reaction"], true);
    assert!(restored.busy(8.04));
    assert!(!restored.is_awaiting(&second));
    assert!(restored.is_awaiting(&first));
    assert!(!restored.busy(10.0));
    assert_eq!(
        restored.floor_until(),
        0.0,
        "expiry adds no acknowledgement beat"
    );

    let mut full = ConversationFloor::new();
    for index in 0..32 {
        full.acquire(0.0, &event(&format!("line {index}")), "", true);
    }
    full.acquire(0.04, &event("line 31"), "", true);
    let (_, wire) = floor_round_trip(&full, 0.04);
    let rows = wire["state"]["awaiting"].as_array().unwrap();
    assert_eq!(rows.len(), 31);
    assert_eq!(rows[0]["event_id"], "line 1");
    assert_eq!(rows[30]["event_id"], "line 31");
    assert_eq!(rows[30]["deadline"]["at"], 8.04);
}

#[test]
fn background_reading_microphone_and_acknowledgement_pacing_continue_independently() {
    let mut floor = ConversationFloor::new();
    let background = event("background");
    floor.acquire_scoped(0.0, &background, "", true, false);
    floor.acquire_scoped(0.04, &event("read only"), &"é".repeat(30), false, true);
    floor.bump_player_hold(0.08, 1.0);
    let (mut restored, wire) = floor_round_trip(&floor, 0.08);
    assert_eq!(wire["state"]["awaiting"].as_array().unwrap().len(), 1);
    assert_eq!(restored.player_hold_until(), 1.08);
    assert_eq!(restored.floor_until(), 4.04);
    restored.release(0.12, &event("read only"));
    assert_eq!(
        restored.floor_until(),
        4.04,
        "unvoiced text has no acknowledgement"
    );
    assert!(restored.busy_for_player_reaction(4.03));
    assert!(!restored.busy_for_player_reaction(4.04));
    assert!(restored.busy(4.04));
    assert!(!restored.busy(8.0));

    let mut floor = ConversationFloor::new();
    floor.acquire(0.0, &event("one"), "", true);
    floor.acquire(0.0, &event("two"), "", true);
    floor.acquire_scoped(0.0, &background, "", true, false);
    let (mut restored, _) = floor_round_trip(&floor, 0.0);
    for state in [&mut floor, &mut restored] {
        state.release(0.04, &event("one"));
        assert_eq!(state.floor_until(), 0.0);
        state.release(0.08, &event("two"));
        let beat = 0.08 + cathedral_sim::FLOOR_POST_UTTERANCE_BEAT_SECONDS;
        assert_eq!(state.floor_until(), beat);
        state.release(0.12, &event("two"));
        state.release(0.16, &event("unknown"));
        assert_eq!(state.floor_until(), beat);
        assert!(!state.busy_for_player_reaction(beat));
        assert!(state.busy(beat));
        state.bump_player_hold(beat, 1.0);
        state.bump_player_hold(beat, 0.1);
        assert_eq!(state.player_hold_until(), beat + 1.0);
        state.clear_player_hold();
        assert!(!state.busy_for_player_reaction(beat));
    }
    assert_eq!(restored, floor);
}

#[test]
fn historical_future_and_infinite_holds_do_not_normalize_the_saved_boundary() {
    let mut floor = ConversationFloor::new();
    floor.acquire_scoped(f64::MAX, &event("future"), "", true, false);
    floor.acquire(f64::MAX, &event("reading"), "", false);
    floor.bump_player_hold(f64::MAX, f64::MAX);
    let (mut restored, wire) = floor_round_trip(&floor, -0.0);
    assert_eq!(
        wire["boundary"].as_f64().unwrap().to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(
        wire["state"]["awaiting"][0]["deadline"]["at"].as_f64(),
        Some(f64::MAX)
    );
    assert_eq!(
        wire["state"]["foreground_floor_until"]["at"].as_f64(),
        Some(f64::MAX)
    );
    assert_eq!(wire["state"]["player_hold_until"], "never");
    assert!(restored.busy_for_player_reaction(f64::MAX));
    restored.clear_player_hold();
    assert!(!restored.busy(f64::MAX));
    let bytes = saved_floor(&floor, -0.0);
    let budget = CheckpointBudget::default();
    assert!(
        FloorDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            at(0.0),
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn strict_floor_records_retain_raw_charges_and_release_failed_admissions() {
    let mut floor = ConversationFloor::new();
    floor.acquire(0.0, &event("retained"), "", true);
    let original: Value = serde_json::from_slice(&saved_floor(&floor, 0.0)).unwrap();
    let mut invalid = Vec::new();
    let mut changed = original.clone();
    changed["version"] = json!(2);
    invalid.push(changed);
    let mut changed = original.clone();
    changed["state"]
        .as_object_mut()
        .unwrap()
        .remove("player_hold_until");
    invalid.push(changed);
    let mut changed = original.clone();
    changed["state"]["awaiting"][0]["extra"] = json!(true);
    invalid.push(changed);
    let mut changed = original.clone();
    changed["state"]["awaiting"][0]["deadline"] = json!({"at":-1.0});
    invalid.push(changed);
    let mut changed = original.clone();
    changed["state"]["awaiting"][0]["event_id"] = json!("é".repeat(32_769));
    invalid.push(changed);
    let mut changed = original.clone();
    let row = changed["state"]["awaiting"][0].clone();
    changed["state"]["awaiting"]
        .as_array_mut()
        .unwrap()
        .push(row);
    invalid.push(changed);
    let mut changed = original.clone();
    changed["state"]["awaiting"] = json!(
        (0..33)
            .map(|i| json!({
                "event_id":format!("{i}"),"deadline":{"at":8.0},"blocks_player_reaction":false
            }))
            .collect::<Vec<_>>()
    );
    invalid.push(changed);
    let mut changed = original.clone();
    changed["state"]["foreground_floor_until"] = Value::Null;
    invalid.push(changed);
    let budget = CheckpointBudget::default();
    for changed in invalid {
        assert_ne!(changed, original);
        let bytes = serde_json::to_vec(&changed).unwrap();
        assert!(
            FloorDtoV1::decode(
                &bytes,
                budget
                    .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                    .unwrap(),
                at(0.0),
            )
            .is_err(),
            "accepted malformed floor: {changed}"
        );
        assert_eq!(budget.retained_bytes(), 0);
    }
    let canonical = serde_json::to_string(&original).unwrap();
    let duplicate = canonical.replace("\"version\":1", "\"version\":1,\"version\":1");
    assert_ne!(duplicate, canonical);
    assert!(
        FloorDtoV1::decode(
            duplicate.as_bytes(),
            budget
                .reserve(Cohort::LoadCandidate, duplicate.len() + 4096)
                .unwrap(),
            at(0.0),
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);

    let padding = 1024 * 1024;
    let mut raw = vec![b' '; padding];
    raw.extend_from_slice(canonical.as_bytes());
    let decoded = FloorDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        at(0.0),
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained >= canonical_peak + 3 * padding);
    let candidate = decoded.into_candidate(at(0.0)).unwrap();
    assert_eq!(budget.retained_bytes(), retained);
    assert!(candidate.value().floor().is_awaiting(&event("retained")));
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    let lexical = raw.len() + 4096;
    let running = budget
        .reserve(Cohort::Running, MAX_RESIDENT_BYTES - lexical)
        .unwrap();
    assert!(
        FloorDtoV1::decode(
            &raw,
            budget.reserve(Cohort::LoadCandidate, lexical).unwrap(),
            at(0.0),
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), MAX_RESIDENT_BYTES - lexical);
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn stored_configuration_and_current_voice_selection_remain_distinct_after_ready() {
    let view_bits = 0x7ff8_0000_0000_0169;
    let cooldown_bits = 0x7ff8_0000_0000_0731;
    let reason = "The original voice fallback explanation.";
    let mut engine = engine(EngineConfig {
        fake_mode: false,
        sounds_enabled: true,
        view_cone_degrees: f64::from_bits(view_bits),
        sound_cooldown_seconds: f64::from_bits(cooldown_bits),
        stt_stream_grace_seconds: f64::NAN,
        tts_selected: TtsBackendKind::Cloud,
        tts_startup_message: Some(reason.into()),
        ..EngineConfig::default()
    });
    let initial = engine_wire(&engine, 0.0);
    assert_eq!(initial["ready_emitted"], false);
    assert_eq!(initial["last_player_sound_at"], "never");
    assert_eq!(initial["tts_selected"], "cloud");
    assert_eq!(initial["config"]["view_cone_degrees"]["bits"], view_bits);
    assert_eq!(
        initial["config"]["sound_cooldown_seconds"]["bits"],
        cooldown_bits
    );
    assert_eq!(
        initial["config"]["stt_stream_grace_seconds"]["bits"],
        0.2f64.to_bits()
    );
    let output = engine.poll(
        0.0,
        vec![EngineCommand::SetTtsBackend {
            request_id: "turn voices off".into(),
            backend: TtsBackendKind::Off,
        }],
    );
    assert_eq!(
        output
            .iter()
            .filter(|m| matches!(m, EngineMessage::Ready { .. }))
            .count(),
        1
    );
    assert_eq!(
        output
            .iter()
            .filter(|m| matches!(m, EngineMessage::Status(s) if s.message.as_deref()==Some(reason)))
            .count(),
        1
    );
    let after = engine_wire(&engine, 0.0);
    assert_eq!(after["ready_emitted"], true);
    assert_eq!(after["startup_diagnostics"], json!([]));
    assert_eq!(after["tts_selected"], "off");
    assert_eq!(after["config"], initial["config"]);
    assert_eq!(
        after["last_snapshot_revision"],
        engine.world().world_revision
    );
    let output = engine.poll(
        0.04,
        vec![EngineCommand::PlayerSound {
            sound_id: "fart".into(),
        }],
    );
    assert!(
        !output
            .iter()
            .any(|m| matches!(m, EngineMessage::Ready { .. }))
    );
    assert!(
        !output
            .iter()
            .any(|m| matches!(m, EngineMessage::Status(s) if s.message.as_deref()==Some(reason)))
    );
    assert_eq!(
        engine_wire(&engine, 0.04)["last_player_sound_at"]["at"],
        0.04
    );
}

#[test]
fn engine_nullable_binding_and_stored_config_limits_are_not_silently_repaired() {
    let engine = engine(EngineConfig {
        sound_cooldown_seconds: f64::INFINITY,
        stt_stream_grace_seconds: f64::INFINITY,
        view_cone_degrees: f64::NEG_INFINITY,
        tts_startup_message: None,
        ..EngineConfig::default()
    });
    let original = engine_wire(&engine, -0.0);
    assert_eq!(
        original["config"]["sound_cooldown_seconds"]["bits"],
        3600.0f64.to_bits()
    );
    assert_eq!(
        original["config"]["stt_stream_grace_seconds"]["bits"],
        f64::INFINITY.to_bits()
    );
    assert_eq!(
        original["config"]["view_cone_degrees"]["bits"],
        f64::NEG_INFINITY.to_bits()
    );
    assert!(original["config"]["tts_startup_message"].is_null());
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed["config"]
        .as_object_mut()
        .unwrap()
        .remove("tts_startup_message");
    cases.push(changed);
    let mut changed = original.clone();
    changed["player_id"] = json!("sv3n1");
    cases.push(changed);
    let mut changed = original.clone();
    changed["config"]["sound_cooldown_seconds"]["bits"] = json!(4000.0f64.to_bits());
    cases.push(changed);
    let mut changed = original.clone();
    changed["config"]["stt_stream_grace_seconds"]["bits"] = json!(0.0f64.to_bits());
    cases.push(changed);
    let mut changed = original.clone();
    changed["boundary"] = json!(0.0);
    cases.push(changed);
    let budget = CheckpointBudget::default();
    let context = engine.continuity_checkpoint_context(at(-0.0));
    for changed in cases {
        let bytes = serde_json::to_vec(&changed).unwrap();
        assert!(
            EngineContinuityDtoV1::decode(
                &bytes,
                budget
                    .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                    .unwrap(),
                context,
            )
            .is_err(),
            "accepted malformed Engine continuity"
        );
        assert_eq!(budget.retained_bytes(), 0);
    }
}
