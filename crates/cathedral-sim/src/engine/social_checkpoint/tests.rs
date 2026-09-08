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

fn logical(n: f64) -> LogicalTime {
    LogicalTime::new(n).unwrap()
}
fn bytes(e: &Engine, t: f64) -> Vec<u8> {
    e.export_social_checkpoint(
        logical(t),
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
fn prefix(e: &mut Engine) {
    let at = e.world.characters[&e.config.player_id].position_m();
    e.poll(
        0.0,
        vec![
            EngineCommand::PlayerAttention {
                actor_id: Some(ActorId::from_raw("sv3n1")),
            },
            EngineCommand::PlayerSay {
                request_id: "social-prefix".into(),
                text: "Sven, can you help?".into(),
                position_m: at,
                spatial_seq: 1,
            },
        ],
    );
    e.poll(0.1,vec![EngineCommand::LlmCompletion(Completion{request_id:RequestId(1),result:Ok("say {\"target\":\"player\",\"text\":\"I will help.\"}\nsay {\"target\":\"cb947\",\"text\":\"Come join us.\"}".into()),duration_seconds:0.1})]);
    e.poll(
        0.4,
        vec![EngineCommand::PlayerAttention {
            actor_id: Some(ActorId::from_raw("sv3n1")),
        }],
    );
}
#[test]
fn checkpoint_social_engine_private_install_exact_then_ordinary_continuation() {
    let mut a = engine();
    let mut b = engine();
    prefix(&mut a);
    prefix(&mut b);
    let raw = bytes(&a, 0.4);
    let budget = CheckpointBudget::default();
    let c = b.social_checkpoint_context(logical(0.4));
    let candidate = EngineSocialDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    let count = candidate.value().counts(c);
    assert!(
        count.conversation.engaged
            && count.conversation.reciprocal
            && count.conversation.focus
            && count.warm_pairs > 0
            && count.novelty_memories > 0
            && count.novelty_told > 0,
        "{count:?}"
    );
    let order = b.scheduler.turn_order().to_vec();
    b.conversation = Conversation::default();
    b.npc_exchanges = WarmExchanges::default();
    b.novelty = Novelty::default();
    b.config.idle_mode = IdleCognitionMode::All;
    b.config.stage = StageConfig {
        radius_m: 1000.0,
        max_actors: 900,
    };
    b.config.idle_requires_news = false;
    b.config.idle_curiosity = CuriosityConfig {
        enabled: true,
        scale: 900.0,
    };
    assert_ne!(bytes(&b, 0.4), raw);
    let d = &candidate.value().data;
    b.conversation = d.conversation.clone();
    b.npc_exchanges = d.warm_exchanges.clone();
    b.novelty = d.novelty.clone();
    b.config.idle_mode = d.idle_mode;
    b.config.stage = d.stage;
    b.config.idle_requires_news = d.idle_requires_news;
    b.config.idle_curiosity = d.idle_curiosity;
    assert_eq!(bytes(&b, 0.4), raw, "before any continuation");
    assert_eq!(
        b.scheduler.turn_order(),
        order,
        "no scheduler reconstruction"
    );
    for (i, t) in [0.65, 1.0, 10.4, 30.4, 60.4, 61.0].into_iter().enumerate() {
        let cmds = if i == 0 {
            vec![
                EngineCommand::PlayerAttention {
                    actor_id: Some(ActorId::from_raw("sv3n1")),
                },
                EngineCommand::PlayerSay {
                    request_id: "social-after".into(),
                    text: "And then?".into(),
                    position_m: a.world.characters[&a.config.player_id].position_m(),
                    spatial_seq: 2,
                },
            ]
        } else {
            vec![]
        };
        assert_eq!(
            format!("{:?}", a.poll(t, cmds.clone())),
            format!("{:?}", b.poll(t, cmds))
        );
        assert_eq!(bytes(&a, t), bytes(&b, t));
    }
}
#[test]
fn checkpoint_social_engine_raw_config_and_scheduler_are_independent() {
    for bits in [
        0,
        (-0.0f64).to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8000000000042,
    ] {
        for max in [0, usize::MAX] {
            let mut e = engine();
            let order = e.scheduler.turn_order().to_vec();
            e.config.idle_mode = IdleCognitionMode::All;
            e.config.stage = StageConfig {
                radius_m: f64::from_bits(bits),
                max_actors: max,
            };
            e.config.idle_curiosity = CuriosityConfig {
                enabled: true,
                scale: f64::from_bits(bits),
            };
            let raw = bytes(&e, 0.0);
            let b = CheckpointBudget::default();
            let c = e.social_checkpoint_context(logical(0.0));
            let d = EngineSocialDtoV1::decode(
                &raw,
                b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
                c,
            )
            .unwrap()
            .into_candidate(c)
            .unwrap();
            assert_eq!(d.value().stage().radius_m.to_bits(), bits);
            assert_eq!(d.value().stage().max_actors, max);
            assert_eq!(d.value().idle_curiosity().scale.to_bits(), bits);
            assert_eq!(d.value().idle_mode(), IdleCognitionMode::All);
            assert_eq!(e.scheduler.turn_order(), order);
        }
    }
    println!(
        "social_layout engine_dto={} engine_candidate={} stage={} curiosity={} context={}",
        std::mem::size_of::<EngineSocialDtoV1>(),
        std::mem::size_of::<EngineSocialCandidate>(),
        std::mem::size_of::<StageConfig>(),
        std::mem::size_of::<CuriosityConfig>(),
        std::mem::size_of::<SocialCheckpointContext<'_>>()
    );
}

#[test]
fn checkpoint_social_engine_v1_fixture_and_strict_raw_float_records() {
    let raw = include_bytes!("../../../tests/fixtures/checkpoint_social/engine-v1.json");
    let b = CheckpointBudget::default();
    let mut e = engine();
    let c = e.social_checkpoint_context(logical(0.0));
    let d = EngineSocialDtoV1::decode(
        raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap();
    let d = &d.value().data;
    e.conversation = d.conversation.clone();
    e.npc_exchanges = d.warm_exchanges.clone();
    e.novelty = d.novelty.clone();
    e.config.idle_mode = d.idle_mode;
    e.config.stage = d.stage;
    e.config.idle_requires_news = d.idle_requires_news;
    e.config.idle_curiosity = d.idle_curiosity;
    assert_eq!(bytes(&e, 0.0), raw);
    let source = std::str::from_utf8(raw).unwrap();
    for bad in [
        source.replace(
            "\"bits\":9221120237041095220",
            "\"bits\":9221120237041095220,\"bits\":0",
        ),
        source.replace(
            "\"bits\":9221120237041095220",
            "\"bits\":9221120237041095220,\"unknown\":0",
        ),
        source.replace("\"max_actors\":0", "\"max_actors\":18446744073709551616"),
    ] {
        let b = CheckpointBudget::default();
        assert!(
            EngineSocialDtoV1::decode(
                bad.as_bytes(),
                b.reserve(Cohort::LoadCandidate, bad.len() + 4096).unwrap(),
                e.social_checkpoint_context(logical(0.0))
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
