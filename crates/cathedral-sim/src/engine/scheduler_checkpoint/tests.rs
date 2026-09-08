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
fn engine() -> Engine {
    Engine::new(
        EngineConfig {
            fake_mode: true,
            nav: Some(crate::dogs::checkpoint::tests::nav()),
            clock: WorldClock::new(3600.0, Office::Waning, 0, 0.05),
            idle_mode: IdleCognitionMode::Stage,
            idle_requires_news: true,
            turn_delay_seconds: 0.0,
            tts_selected: TtsBackendKind::Off,
            ..Default::default()
        },
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
fn bytes(e: &Engine, t: f64) -> Vec<u8> {
    e.export_scheduler_checkpoint(
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
fn say(e: &Engine, seq: i64, text: &str) -> EngineCommand {
    EngineCommand::PlayerSay {
        request_id: format!("scheduler-{seq}"),
        text: text.into(),
        position_m: e.world.characters[&e.config.player_id].position_m(),
        spatial_seq: seq,
    }
}
fn prefix(e: &mut Engine) {
    e.poll(0.0, vec![say(e, 1, "Sven, can you help?")]);
    assert!(e.scheduler.in_flight_actor_id().is_some());
    e.poll(0.1, vec![say(e, 2, "Sven, can you also look here?")]);
    e.poll(
        0.2,
        vec![
            EngineCommand::PlayerAudioBegin {
                wav_basename: "scheduler-hold.wav".into(),
                sample_rate: 24_000,
            },
            EngineCommand::LlmCompletion(Completion {
                request_id: RequestId(1),
                result: Ok("remember {\"memory\":\"Engine held input\"}".into()),
                duration_seconds: 0.123,
            }),
        ],
    );
    assert!(e.scheduler.has_held_result());
}
#[test]
fn checkpoint_scheduler_engine_private_install_exact_then_ordinary_continuation() {
    let mut a = engine();
    let mut b = engine();
    prefix(&mut a);
    prefix(&mut b);
    let raw = bytes(&a, 0.2);
    let budget = CheckpointBudget::default();
    let c = b.scheduler_checkpoint_context(at(0.2));
    let saved = EngineSchedulerDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    b.scheduler = NpcScheduler::new(vec![ActorId::from_raw("scrambled")], 88.0, 99.0, 55.0);
    b.config.turn_delay_seconds = 123.0;
    b.config.maximum_backoff_seconds = 124.0;
    assert_ne!(bytes(&b, 0.2), raw);
    let d = &saved.value().data;
    b.scheduler = owner::copy(&d.scheduler);
    b.config.turn_delay_seconds = d.turn_delay_seconds;
    b.config.maximum_backoff_seconds = d.maximum_backoff_seconds;
    assert_eq!(
        bytes(&b, 0.2),
        raw,
        "immediate canonical equality before continuation"
    );
    for t in [0.3, 1.0, 2.0, 3.0, 10.0] {
        let cmds = if t == 0.3 {
            vec![EngineCommand::PlayerAudioAbort {
                wav_basename: "scheduler-hold.wav".into(),
            }]
        } else {
            vec![]
        };
        assert_eq!(
            format!("{:?}", a.poll(t, cmds.clone())),
            format!("{:?}", b.poll(t, cmds))
        );
        assert_eq!(bytes(&a, t), bytes(&b, t));
        assert_eq!(a.world, b.world);
    }
    assert!(
        a.world.characters[&ActorId::from_raw("sv3n1")]
            .memories()
            .iter()
            .any(|m| m == "Engine held input")
    );
}
#[test]
fn checkpoint_scheduler_engine_original_config_raw_bits_independent_of_actual_owner() {
    for bits in [
        0,
        (-0.0f64).to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8000000000042,
        0x7ff0000000000001,
    ] {
        let mut e = engine();
        e.config.turn_delay_seconds = f64::from_bits(bits);
        e.config.maximum_backoff_seconds = f64::from_bits(bits);
        let raw = bytes(&e, 0.0);
        let c = e.scheduler_checkpoint_context(at(0.0));
        let d = EngineSchedulerDtoV1::decode(
            &raw,
            CheckpointBudget::default()
                .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                .unwrap(),
            c,
        )
        .unwrap()
        .into_candidate(c)
        .unwrap();
        assert_eq!(d.value().turn_delay_seconds().to_bits(), bits);
        assert_eq!(d.value().maximum_backoff_seconds().to_bits(), bits);
        let w: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(
            w["scheduler"]["minimum_delay_seconds"],
            serde_json::json!({"at":0.0})
        );
    }
    println!(
        "scheduler_layout engine_dto={} engine_candidate={} engine_context={}",
        std::mem::size_of::<EngineSchedulerDtoV1>(),
        std::mem::size_of::<EngineSchedulerCandidate>(),
        std::mem::size_of::<EngineSchedulerCheckpointContext<'_>>()
    );
}
fn fixture_engine() -> Engine {
    let mut e = engine();
    e.config.turn_delay_seconds = f64::from_bits(0x7ff8000000000042);
    e.config.maximum_backoff_seconds = -0.0;
    e
}
#[test]
fn checkpoint_scheduler_engine_supported_fixture_and_strict_original_bits() {
    let e = fixture_engine();
    let raw = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/checkpoint_scheduler/engine-v1.json"),
    )
    .unwrap();
    assert_eq!(bytes(&e, 0.0), raw);
    let c = e.scheduler_checkpoint_context(at(0.0));
    let d = EngineSchedulerDtoV1::decode(
        &raw,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    assert_eq!(d.value().turn_delay_seconds().to_bits(), 0x7ff8000000000042);
    let original = String::from_utf8(raw).unwrap();
    for mutation in [
        "\"bits\":0,\"bits\":0",
        "\"bits\":0,\"unknown\":0",
        "\"unknown\":0",
    ] {
        let raw = original.replacen("\"bits\":9221120237041090626", mutation, 1);
        assert_ne!(original, raw);
        assert!(
            EngineSchedulerDtoV1::decode(
                raw.as_bytes(),
                CheckpointBudget::default()
                    .reserve(Cohort::LoadCandidate, raw.len() + 4096)
                    .unwrap(),
                c
            )
            .is_err()
        );
    }
}
#[test]
#[ignore = "explicit new Engine scheduler fixture generation"]
fn regenerate_checkpoint_scheduler_engine_fixture() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/checkpoint_scheduler");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("engine-v1.json"), bytes(&fixture_engine(), 0.0)).unwrap();
}
