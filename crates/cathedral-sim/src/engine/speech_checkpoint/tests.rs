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

fn bytes(e: &Engine) -> Vec<u8> {
    e.export_speech_checkpoint(
        at(0.0),
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
#[test]
fn checkpoint_speech_engine_player_and_config_are_independent() {
    let mut e = engine();
    e.poll(
        0.0,
        vec![EngineCommand::PlayerUtteranceStarted {
            wav_basename: "onset.wav".into(),
        }],
    );
    let raw = bytes(&e);
    let c = e.speech_checkpoint_context(at(0.0));
    let d = EngineSpeechDtoV1::decode(
        &raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert_eq!(d.value().captures(), &["onset.wav"]);
    assert_eq!(d.value().purpose(), InputPurpose::PublicPlayerSpeech);
    assert_eq!(d.value().status(), InterruptionStatus::InterruptedUnsent);
    let before = e.world.world_revision;
    let transcript = e.transcript.clone();
    e.config.stt_stream_grace_seconds = 999.0;
    assert_eq!(
        bytes(&e),
        raw,
        "prior Engine config owner is independent of actual router grace"
    );
    assert_eq!(e.world.world_revision, before);
    assert_eq!(e.transcript, transcript);
    e.speech_router = SpeechRouter::default();
    assert_ne!(bytes(&e), raw);
    assert_eq!(d.value().captures(), &["onset.wav"]);
    assert!(
        EngineSpeechDtoV1::decode(
            &raw,
            CheckpointBudget::default()
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            EngineSpeechCheckpointContext::from_world(
                &e.world,
                at(0.0),
                &ActorId::from_raw("other")
            )
        )
        .is_err()
    );
}
#[test]
fn checkpoint_speech_engine_saved_context_preserves_player_membership() {
    let mut e = engine();
    e.poll(
        0.0,
        vec![EngineCommand::PlayerUtteranceStarted {
            wav_basename: "onset.wav".into(),
        }],
    );
    let raw = bytes(&e);
    let b = e
        .world
        .export_backbone_checkpoint(
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, 4096)
                .unwrap(),
        )
        .unwrap()
        .into_candidate(&e.world.item_catalog, &e.world.command_ledger)
        .unwrap();
    let ledger = e
        .world
        .command_ledger
        .checkpoint_v1(
            at(0.0),
            CheckpointBudget::default()
                .reserve(Cohort::SavePayload, CommandLedgerDtoV1::WORKING_BYTES)
                .unwrap(),
        )
        .unwrap();
    let c = EngineSpeechCheckpointContext::from_backbone(
        b.value(),
        at(0.0),
        ledger.value(),
        &e.config.player_id,
    );
    let d = EngineSpeechDtoV1::decode(
        &raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert_eq!(d.value().player_id(), &e.config.player_id);
    assert_eq!(d.value().counts(c).captures, 1);
}
#[test]
fn checkpoint_speech_engine_layout_bound() {
    use std::mem::size_of;
    println!(
        "speech_engine_layout={}",
        serde_json::json!({"dto":size_of::<EngineSpeechDtoV1>(),"candidate":size_of::<EngineSpeechCandidate>(),"wire":size_of::<Wire>(),"view":size_of::<View>(),"context":size_of::<EngineSpeechCheckpointContext>()})
    );
    assert!(size_of::<EngineSpeechDtoV1>() <= 512);
    assert!(size_of::<View>() <= 512);
}
#[test]
#[ignore = "new component fixture authoring only"]
fn checkpoint_speech_generate_engine_fixtures() {
    let mut e = engine();
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::write(
        base.join("checkpoint_engine_speech_empty_v1.json"),
        bytes(&e),
    )
    .unwrap();
    e.poll(
        0.0,
        vec![EngineCommand::PlayerUtteranceStarted {
            wav_basename: "onset.wav".into(),
        }],
    );
    std::fs::write(
        base.join("checkpoint_engine_speech_onset_v1.json"),
        bytes(&e),
    )
    .unwrap();
}

#[test]
fn checkpoint_speech_engine_fixtures_pin_projection() {
    let mut e = engine();
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert_eq!(
        std::fs::read(base.join("checkpoint_engine_speech_empty_v1.json")).unwrap(),
        bytes(&e)
    );
    e.poll(
        0.0,
        vec![EngineCommand::PlayerUtteranceStarted {
            wav_basename: "onset.wav".into(),
        }],
    );
    assert_eq!(
        std::fs::read(base.join("checkpoint_engine_speech_onset_v1.json")).unwrap(),
        bytes(&e)
    );
}
