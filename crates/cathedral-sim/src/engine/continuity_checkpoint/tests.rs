use super::*;
use crate::{
    CognitionBusy, NullSight, NullTranscription, NullTts, RequestId,
    checkpoint::{CheckpointBudget, Cohort},
};
#[derive(Default)]
struct Recorded(u64);
impl Cognition for Recorded {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        self.0 += 1;
        Ok(RequestId(self.0))
    }
}
fn engine_with(mut config: EngineConfig) -> Engine {
    config.nav = Some(crate::dogs::checkpoint::tests::nav());
    config.clock = WorldClock::new(3600.0, Office::Waning, 0, 0.05);
    Engine::new(
        config,
        &crate::WorldSeed::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/demo_seed.json"
        )))
        .unwrap(),
        AreaMap::from_json_str(include_str!("../../../../../assets/world/areas.json")).unwrap(),
        SoundCatalog::from_toml_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sounds/catalog.toml"
        )))
        .unwrap(),
        PromptEnv::new(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/turn.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/night.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/strings.toml"
            )),
        )
        .unwrap(),
        Box::new(Recorded::default()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}
fn at(n: f64) -> LogicalTime {
    LogicalTime::new(n).unwrap()
}
fn engine() -> Engine {
    engine_with(EngineConfig {
        fake_mode: true,
        idle_mode: IdleCognitionMode::Stage,
        idle_requires_news: true,
        turn_delay_seconds: 0.0,
        ..Default::default()
    })
}
fn bytes(e: &Engine, t: f64) -> Vec<u8> {
    e.export_continuity_checkpoint(
        at(t),
        CheckpointBudget::default()
            .reserve(Cohort::SavePayload, 4096)
            .unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
    .value()
    .clone()
}
fn install(d: &EngineContinuityDtoV1, e: &mut Engine) {
    e.floor = owner::copy(&d.floor);
    e.last_snapshot_revision = d.last_snapshot_revision;
    e.last_player_sound_at = d.last_player_sound_at;
    e.next_round_tick_at = d.next_round_tick_at;
    e.lamp_revision_sent = d.lamp_revision_sent;
    e.startup_diagnostics = d.startup_diagnostics.clone();
    e.ready_emitted = d.ready_emitted;
    e.tts_selected = d.tts_selected;
    e.config.fake_mode = d.config.fake_mode;
    e.config.sounds_enabled = d.config.sounds_enabled;
    e.config.view_cone_degrees = d.config.view_cone_degrees;
    e.config.sound_cooldown_seconds = d.config.sound_cooldown_seconds;
    e.config.tts_selected = d.config.tts_selected;
    e.config.tts_startup_message = d.config.tts_startup_message.clone();
    e.config.stt_stream_grace_seconds = d.config.stt_stream_grace_seconds;
}
fn scramble_restore(a: &Engine, b: &mut Engine, t: f64) {
    let raw = bytes(a, t);
    let c = b.continuity_checkpoint_context(at(t));
    let d = EngineContinuityDtoV1::decode(
        &raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    b.floor = ConversationFloor::new();
    b.floor.bump_player_hold(100.0, 123.0);
    b.last_snapshot_revision = -55;
    b.last_player_sound_at = 444.0;
    b.next_round_tick_at = 555.0;
    b.lamp_revision_sent = 444;
    b.startup_diagnostics = vec!["scrambled".into()];
    b.ready_emitted = !a.ready_emitted;
    b.tts_selected = TtsBackendKind::Cloud;
    b.config.fake_mode = !a.config.fake_mode;
    b.config.sounds_enabled = !a.config.sounds_enabled;
    b.config.view_cone_degrees = -123.0;
    b.config.sound_cooldown_seconds = 3599.0;
    b.config.tts_selected = TtsBackendKind::Local;
    b.config.tts_startup_message = Some("scrambled".into());
    b.config.stt_stream_grace_seconds = 555.0;
    assert_ne!(bytes(b, t), raw);
    install(&d.value().data, b);
    assert_eq!(bytes(b, t), raw, "immediate equality before any poll");
}
fn same_poll(
    a: &mut Engine,
    b: &mut Engine,
    t: f64,
    commands: Vec<EngineCommand>,
) -> Vec<EngineMessage> {
    let x = a.poll(t, commands.clone());
    let y = b.poll(t, commands);
    assert_eq!(format!("{x:?}"), format!("{y:?}"));
    assert_eq!(a.world, b.world);
    assert_eq!(bytes(a, t), bytes(b, t));
    x
}
#[test]
fn checkpoint_continuity_engine_initial_publication_ready_and_diagnostics_then_cadence() {
    let mut a = engine();
    let mut b = engine();
    // Rare diagnostic text is private fixture authority, never a measured state.
    a.startup_diagnostics = vec![
        "first startup notice".into(),
        "second startup notice é".into(),
    ];
    a.config.tts_startup_message = Some("saved host explanation".into());
    b.startup_diagnostics = a.startup_diagnostics.clone();
    b.config.tts_startup_message = a.config.tts_startup_message.clone();
    scramble_restore(&a, &mut b, 0.0);
    let out = same_poll(&mut a, &mut b, 0.0, vec![]);
    assert_eq!(
        out.iter()
            .filter(|m| matches!(m, EngineMessage::Ready { .. }))
            .count(),
        1
    );
    let notices: Vec<_> = out
        .iter()
        .filter_map(|m| {
            if let EngineMessage::Diagnostic(s) = m {
                Some(s.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(
        notices
            .windows(2)
            .any(|w| w == ["first startup notice", "second startup notice é"])
    );
    assert!(a.startup_diagnostics.is_empty());
    assert!(a.ready_emitted);
    assert_eq!(a.next_round_tick_at, crate::MOVEMENT_TICK_SECONDS);
    let old = a.next_round_tick_at;
    same_poll(&mut a, &mut b, 0.02, vec![]);
    assert_eq!(a.next_round_tick_at, old);
    scramble_restore(&a, &mut b, 0.02);
    let out = same_poll(&mut a, &mut b, 0.05, vec![]);
    assert!(!out.iter().any(|m| matches!(m, EngineMessage::Ready { .. })));
    assert_eq!(a.next_round_tick_at, 0.05 + crate::MOVEMENT_TICK_SECONDS);
    assert_eq!(a.lamp_revision_sent, a.round.lamp_revision());
}
#[test]
fn checkpoint_continuity_engine_sound_cooldown_floor_and_cached_publication_continuation() {
    let mut a = engine();
    let mut b = engine();
    scramble_restore(&a, &mut b, 0.0);
    let sound = || EngineCommand::PlayerSound {
        sound_id: "fart".into(),
    };
    let out = same_poll(&mut a, &mut b, 0.0, vec![sound()]);
    assert!(format!("{out:?}").contains("player sound emitted"));
    assert_eq!(a.last_player_sound_at, 0.0);
    let say = EngineCommand::PlayerSay {
        request_id: "continuity".into(),
        text: "Sven, can you help?".into(),
        position_m: a.world.characters[&a.config.player_id].position_m(),
        spatial_seq: 1,
    };
    same_poll(&mut a, &mut b, 0.04, vec![say]);
    same_poll(
        &mut a,
        &mut b,
        0.08,
        vec![EngineCommand::LlmCompletion(Completion {
            request_id: RequestId(1),
            result: Ok("say {\"target\":\"player\",\"text\":\"I can help you.\"}".into()),
            duration_seconds: 0.04,
        })],
    );
    assert!(a.floor.floor_until() > 0.08);
    same_poll(
        &mut a,
        &mut b,
        0.12,
        vec![EngineCommand::PlayerAudioBegin {
            wav_basename: "continuity.wav".into(),
            sample_rate: 24_000,
        }],
    );
    assert!(a.floor.player_hold_until() > 0.12);
    scramble_restore(&a, &mut b, 0.12);
    let denied = same_poll(&mut a, &mut b, 0.16, vec![sound()]);
    assert!(format!("{denied:?}").contains("cooldown"));
    same_poll(
        &mut a,
        &mut b,
        0.20,
        vec![EngineCommand::PlayerAudioAbort {
            wav_basename: "continuity.wav".into(),
        }],
    );
    assert_eq!(a.floor.player_hold_until(), 0.0);
    for i in 6..=50 {
        same_poll(
            &mut a,
            &mut b,
            i as f64 * 0.04,
            if i == 50 { vec![sound()] } else { vec![] },
        );
    }
    assert_eq!(a.last_player_sound_at, 2.0);
}
#[test]
fn checkpoint_continuity_engine_stored_constructor_bits_and_reachable_edges() {
    for bits in [
        0,
        (-0.0f64).to_bits(),
        f64::MAX.to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8000000000042,
        0x7ff0000000000001,
    ] {
        let raw = f64::from_bits(bits);
        let a = engine_with(EngineConfig {
            view_cone_degrees: raw,
            sound_cooldown_seconds: raw,
            stt_stream_grace_seconds: raw,
            ..Default::default()
        });
        let wire = bytes(&a, 0.0);
        let c = a.continuity_checkpoint_context(at(0.0));
        let b = EngineContinuityDtoV1::decode(
            &wire,
            CheckpointBudget::default()
                .reserve(Cohort::LoadCandidate, wire.len() + 4096)
                .unwrap(),
            c,
        )
        .unwrap()
        .into_candidate(c)
        .unwrap();
        assert_eq!(b.value().view_cone_degrees().to_bits(), bits);
        assert_eq!(
            b.value().sound_cooldown_seconds().to_bits(),
            a.config.sound_cooldown_seconds.to_bits()
        );
        assert_eq!(
            b.value().stt_stream_grace_seconds().to_bits(),
            a.config.stt_stream_grace_seconds.to_bits()
        );
    }
    let mut a = engine();
    let mut b = engine();
    a.last_snapshot_revision = i64::MIN;
    a.lamp_revision_sent = u64::MAX;
    a.last_player_sound_at = f64::MAX;
    a.next_round_tick_at = f64::INFINITY;
    scramble_restore(&a, &mut b, 0.0);
    assert_eq!(a.next_round_tick_at, b.next_round_tick_at);
    assert_eq!(a.last_player_sound_at, b.last_player_sound_at);
}
#[test]
fn checkpoint_continuity_engine_maximum_diagnostics_and_layout() {
    let mut a = engine();
    a.startup_diagnostics = vec![String::new(); MAX_STARTUP_DIAGNOSTICS];
    a.startup_diagnostics[0] = "é".repeat(MAX_TEXT_BYTES / 2);
    a.config.tts_startup_message = Some("x".repeat(MAX_TEXT_BYTES));
    let raw = bytes(&a, 0.0);
    let c = a.continuity_checkpoint_context(at(0.0));
    let budget = CheckpointBudget::default();
    let d = EngineContinuityDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert_eq!(d.value().startup_diagnostics(), a.startup_diagnostics);
    drop(d);
    assert_eq!(budget.retained_bytes(), 0);
    a.startup_diagnostics.push(String::new());
    assert!(
        a.export_continuity_checkpoint(at(0.0), budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
    a.startup_diagnostics.clear();
    a.config.tts_startup_message.as_mut().unwrap().push('x');
    assert!(
        a.export_continuity_checkpoint(at(0.0), budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
    assert!(std::mem::size_of::<EngineContinuityDtoV1>() <= 512);
    assert!(std::mem::size_of::<View<'_>>() <= 512);
    println!(
        "continuity_layout engine_dto={} engine_candidate={} engine_view={} engine_context={} config={} config_view={} engine_wire={} option_string={}",
        std::mem::size_of::<EngineContinuityDtoV1>(),
        std::mem::size_of::<EngineContinuityCandidate>(),
        std::mem::size_of::<View<'_>>(),
        std::mem::size_of::<EngineContinuityCheckpointContext<'_>>(),
        std::mem::size_of::<Config>(),
        std::mem::size_of::<ConfigView<'_>>(),
        std::mem::size_of::<Wire>(),
        std::mem::size_of::<Option<String>>()
    );
}
fn fixture() -> Engine {
    let mut e = engine_with(EngineConfig {
        fake_mode: true,
        view_cone_degrees: f64::from_bits(0x7ff8000000000042),
        sound_cooldown_seconds: -0.0,
        stt_stream_grace_seconds: f64::INFINITY,
        tts_startup_message: Some("stored voice explanation".into()),
        ..Default::default()
    });
    // New supported private rare-state fixture: authored ordered diagnostics.
    e.startup_diagnostics = vec![
        "continuity initial diagnostic".into(),
        "second diagnostic é".into(),
    ];
    e
}
#[test]
fn checkpoint_continuity_engine_supported_fixture() {
    let raw = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/checkpoint_continuity/engine-initial-v1.json"),
    )
    .unwrap();
    let a = fixture();
    assert_eq!(raw, bytes(&a, 0.0));
    let mut b = fixture();
    scramble_restore(&a, &mut b, 0.0);
}
#[test]
#[ignore = "explicit new continuity Engine fixture"]
fn regenerate_checkpoint_continuity_engine_fixture() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_continuity");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("engine-initial-v1.json"), bytes(&fixture(), 0.0)).unwrap();
}
#[test]
fn checkpoint_continuity_engine_saved_backbone_binding_borrows_saved_membership() {
    use crate::world::checkpoint::WorldBackboneDtoV1;
    let mut e = engine();
    let raw = bytes(&e, 0.0);
    let budget = CheckpointBudget::default();
    let backbone = e
        .world
        .export_backbone_checkpoint(budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let saved = WorldBackboneDtoV1::decode(
        backbone.value(),
        budget
            .reserve(Cohort::LoadCandidate, backbone.value().len() + 4096)
            .unwrap(),
        &e.world.item_catalog,
        &e.world.command_ledger,
    )
    .unwrap()
    .into_candidate(&e.world.item_catalog, &e.world.command_ledger)
    .unwrap();
    e.world.characters.remove(&e.config.player_id);
    // Separate component budgets prove saved reference binding only; full
    // envelope cohort/lifetime composition remains deliberately unresolved.
    let continuity_budget = CheckpointBudget::default();
    let decode = |c| {
        EngineContinuityDtoV1::decode(
            &raw,
            continuity_budget
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            c,
        )
    };
    assert!(decode(e.continuity_checkpoint_context(at(0.0))).is_err());
    let c = EngineContinuityCheckpointContext::from_backbone(
        saved.value(),
        at(0.0),
        &e.config.player_id,
    );
    let candidate = decode(c).unwrap().into_candidate(c).unwrap();
    assert_eq!(candidate.value().player_id(), &e.config.player_id);
    drop(candidate);
    let wrong = ActorId::from_raw("sv3n1");
    assert!(
        decode(EngineContinuityCheckpointContext::from_backbone(
            saved.value(),
            at(0.0),
            &wrong
        ))
        .is_err()
    );
    drop(saved);
    drop(backbone);
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(continuity_budget.retained_bytes(), 0);
}
#[test]
fn checkpoint_continuity_sparse_live_capacity_is_not_copied() {
    let mut e = engine();
    e.startup_diagnostics = Vec::with_capacity(100_000);
    let mut text = String::with_capacity(1_000_000);
    text.push_str("small");
    e.startup_diagnostics.push(text);
    let d = e
        .export_continuity_checkpoint(
            at(0.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(d.value().startup_diagnostics.len(), 1);
    assert!(d.value().startup_diagnostics.capacity() <= 4);
    assert_eq!(d.value().startup_diagnostics[0].capacity(), 5);
}
