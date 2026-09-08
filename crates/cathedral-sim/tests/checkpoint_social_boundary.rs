//! Independent social boundaries through public APIs; no whole-city adoption.
mod prompt_support;

use cathedral_sim::{
    ActorId, Capabilities, CuriosityConfig, Engine, EngineConfig, FakeCognition, IdleCognitionMode,
    Novelty, NullSight, NullTranscription, NullTts, StageConfig, TtsBackendKind, Vec3,
    WarmExchanges, World, WorldSeed,
    attention::checkpoint::{NoveltyDtoV1, WarmExchangesDtoV1},
    checkpoint::{CheckpointBudget, Cohort},
    conversation::{AddressEvidence, Conversation, checkpoint::ConversationDtoV1},
    engine::social_checkpoint::EngineSocialDtoV1,
    prompt::render_prompt_and_drain,
    timeline::LogicalTime,
};
use std::collections::BTreeSet;

fn at(seconds: f64) -> LogicalTime {
    LogicalTime::new(seconds).unwrap()
}

fn actor(id: &str) -> ActorId {
    ActorId::from_raw(id)
}

fn conversation_bytes(value: &Conversation, now: f64) -> Vec<u8> {
    value
        .export_checkpoint(
            at(now),
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

fn warm_bytes(value: &WarmExchanges, now: f64) -> Vec<u8> {
    value
        .export_checkpoint(
            at(now),
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

fn novelty_bytes(value: &Novelty, now: f64) -> Vec<u8> {
    value
        .export_checkpoint(
            at(now),
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

fn restore_conversation(bytes: &[u8], now: f64) -> Conversation {
    let budget = CheckpointBudget::default();
    let candidate = ConversationDtoV1::decode(
        bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        at(now),
    )
    .unwrap()
    .into_candidate(at(now))
    .unwrap();
    let restored = candidate.value().conversation().clone();
    drop(candidate);
    assert_eq!(budget.retained_bytes(), 0);
    restored
}

fn position(world: &World) -> Vec3 {
    world.characters[&actor("player")].position_m()
}

#[test]
fn partial_focus_finishes_its_original_dwell_and_expires_at_its_original_sample() {
    let world = prompt_support::seed_world();
    let player = actor("player");
    let sven = actor("sv3n1");
    let conny = actor("cb947");
    let mut control = Conversation::default();
    control.observe_focus(0.0, Some(sven.clone()));
    control.observe_focus(0.3, Some(sven.clone()));
    let bytes = conversation_bytes(&control, 0.3);
    let mut restored = restore_conversation(&bytes, 0.3);
    assert_eq!(restored, control);
    assert_eq!(conversation_bytes(&restored, 0.3), bytes);

    for (now, evidence, target) in [
        (0.64, AddressEvidence::Nearest, &conny),
        (0.65, AddressEvidence::Focus, &sven),
        (1.31, AddressEvidence::Nearest, &conny),
    ] {
        let a = control.capture(now, &world);
        let b = restored.capture(now, &world);
        assert_eq!(a, b);
        let choice = a.select(now, &world, &player, position(&world), "Hello there.");
        assert_eq!(
            b.select(now, &world, &player, position(&world), "Hello there."),
            choice
        );
        assert_eq!(choice.evidence, evidence);
        assert_eq!(choice.addressee.as_ref(), Some(target));
    }
    assert_eq!(restored, control);
}

#[test]
fn reciprocal_choice_retains_its_witnesses_and_an_independent_invitation() {
    let world = prompt_support::seed_world();
    let player = actor("player");
    let conny = actor("cb947");
    let sven = actor("sv3n1");
    let ilse = actor("k0fb1");
    let mut control = Conversation::default();
    control.player_addressed(2.0, &conny, &[player.clone(), sven.clone()]);
    control.npc_addressed_player(2.5, &conny, &[player.clone(), sven.clone()]);
    control.npc_addressed_player(3.0, &ilse, &[player.clone(), ilse.clone()]);
    let bytes = conversation_bytes(&control, 3.0);
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["state"]["next_utterance"], 1);
    assert_eq!(wire["state"]["latest_applied_utterance"], 1);
    assert_eq!(wire["state"]["engagement"]["actor"], "cb947");
    assert_eq!(wire["state"]["engagement"]["reciprocal"], true);
    assert_eq!(wire["state"]["invitation"]["actor"], "k0fb1");
    let mut restored = restore_conversation(&bytes, 3.0);
    assert_eq!(restored, control);
    assert_eq!(conversation_bytes(&restored, 3.0), bytes);

    let a = control.capture(3.0, &world);
    let b = restored.capture(3.0, &world);
    let selection = a.select(3.0, &world, &player, position(&world), "Please continue.");
    assert_eq!(
        b.select(3.0, &world, &player, position(&world), "Please continue."),
        selection
    );
    assert_eq!(selection.addressee.as_ref(), Some(&conny));
    assert_eq!(selection.evidence, AddressEvidence::Reciprocal);
    let env = prompt_support::prompt_env();
    let prose = &env.strings().conversation;
    assert!(
        selection
            .percept_suffix(&world, &sven, prose)
            .contains(&prose.continuing)
    );
    assert!(
        selection
            .percept_suffix(&world, &ilse, prose)
            .contains(&prose.apparent_attention)
    );
    for value in [&control, &restored] {
        assert_eq!(value.partner(13.0, &world), Some(&conny));
        assert_eq!(value.partner(32.499, &world), Some(&conny));
        assert_eq!(value.partner(32.5, &world), None);
    }
    assert_eq!(
        conversation_bytes(&control, 32.5),
        conversation_bytes(&restored, 32.5)
    );
}

#[test]
fn historical_warm_pairs_keep_alias_identity_until_ordinary_expiry() {
    let a = actor("historicala");
    let b = actor("historicalb");
    let mut control = WarmExchanges::default();
    control.note(&a, &b, 10.0);
    control.note(&b, &a, 12.0);
    control.note(&a, &a, 15.0);
    // A backwards caller does not rewrite a historical last-line sample.
    let bytes = warm_bytes(&control, 11.0);
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["state"]["pairs"].as_array().unwrap().len(), 1);
    assert_eq!(wire["state"]["pairs"][0]["a"], "historicala");
    assert_eq!(wire["state"]["pairs"][0]["b"], "historicalb");
    assert_eq!(wire["state"]["pairs"][0]["at"], 12.0);
    let candidate = WarmExchangesDtoV1::decode(
        &bytes,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        at(11.0),
    )
    .unwrap()
    .into_candidate(at(11.0))
    .unwrap();
    let mut restored = candidate.value().warm_exchanges().clone();
    assert_eq!(warm_bytes(&restored, 11.0), bytes);
    // Exporting an already stale pair must also leave pruning to its consumer.
    let stale: serde_json::Value = serde_json::from_slice(&warm_bytes(&restored, 50.0)).unwrap();
    assert_eq!(stale["state"]["pairs"].as_array().unwrap().len(), 1);
    let expected = BTreeSet::from([a, b]);
    assert_eq!(control.warm_actors(41.999), expected);
    assert_eq!(restored.warm_actors(41.999), expected);
    assert!(control.warm_actors(42.0).is_empty());
    assert!(restored.warm_actors(42.0).is_empty());
    assert_eq!(warm_bytes(&restored, 42.0), warm_bytes(&control, 42.0));
}

#[test]
fn novelty_preserves_opaque_visit_bits_submission_context_and_later_news() {
    let mut world = prompt_support::seed_world();
    let sven = actor("sv3n1");
    let ilse = actor("k0fb1");
    let nan_bits = 0x7ff8_0000_0000_0143;
    let mut control = Novelty::default();
    control.told(f64::from_bits(nan_bits), &world, &sven);
    control.observe(0.0, &BTreeSet::from([sven.clone()]));
    control.observe(-0.0, &BTreeSet::from([ilse.clone()]));
    control.observe(6.0, &BTreeSet::from([sven.clone(), ilse.clone()]));
    assert!(!control.has_news(&world, &sven));
    assert!(control.has_news(&world, &ilse));
    let bytes = novelty_bytes(&control, 6.0);
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let memories = &wire["state"]["last_told"];
    assert_eq!(memories["sv3n1"]["visit"], nan_bits);
    assert_eq!(memories["k0fb1"]["visit"], (-0.0_f64).to_bits());
    assert!(memories["sv3n1"]["context"].is_u64());
    assert!(memories["k0fb1"]["context"].is_null());
    let candidate = NoveltyDtoV1::decode(
        &bytes,
        CheckpointBudget::default()
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        at(6.0),
    )
    .unwrap()
    .into_candidate(at(6.0))
    .unwrap();
    let mut restored = candidate.value().novelty().clone();
    assert_eq!(novelty_bytes(&restored, 6.0), bytes);
    assert!(!restored.has_news(&world, &sven));
    assert!(restored.has_news(&world, &ilse));

    world
        .characters
        .get_mut(&sven)
        .unwrap()
        .notify_percept("A late arrival after submission.");
    let no_initiative = CuriosityConfig {
        enabled: true,
        scale: 0.0,
    };
    assert!(control.admits_idle(&world, &sven, &no_initiative));
    assert!(restored.admits_idle(&world, &sven, &no_initiative));
    let (prompt, _) =
        render_prompt_and_drain(&mut world, &sven, &prompt_support::prompt_env()).unwrap();
    assert!(prompt.contains("A late arrival after submission."));
    assert!(world.characters[&sven].inbox().is_empty());
    for value in [&mut control, &mut restored] {
        value.told(7.0, &world, &sven);
        assert!(!value.has_news(&world, &sven));
        value.observe(67.0, &BTreeSet::new());
        assert!(
            !value.has_news(&world, &sven),
            "the 60-second edge is inclusive"
        );
        value.observe(67.000001, &BTreeSet::new());
        assert!(value.has_news(&world, &sven));
    }
    assert_eq!(
        novelty_bytes(&restored, 67.000001),
        novelty_bytes(&control, 67.000001)
    );
}

#[test]
fn backward_focus_history_is_exact_and_signed_zero_boundaries_do_not_alias() {
    let mut control = Conversation::default();
    control.observe_focus(10.0, Some(actor("historicalfocus")));
    control.observe_focus(9.0, Some(actor("historicalfocus")));
    let bytes = conversation_bytes(&control, 8.0);
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["state"]["focus"]["since"], 10.0);
    assert_eq!(wire["state"]["focus"]["last"], 9.0);
    assert_eq!(restore_conversation(&bytes, 8.0), control);

    let bytes = conversation_bytes(&Conversation::default(), -0.0);
    let budget = CheckpointBudget::default();
    assert!(
        ConversationDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            at(0.0),
        )
        .is_err()
    );
    assert_eq!(budget.retained_bytes(), 0);
    assert_eq!(restore_conversation(&bytes, -0.0), Conversation::default());
}

#[test]
fn engine_social_preserves_raw_config_and_rejects_malformed_or_wrong_context() {
    let radius_bits = 0x7ff8_0000_0000_0057;
    let engine = Engine::new(
        EngineConfig {
            idle_mode: IdleCognitionMode::Stage,
            stage: StageConfig {
                radius_m: f64::from_bits(radius_bits),
                max_actors: usize::MAX,
            },
            idle_requires_news: true,
            idle_curiosity: CuriosityConfig {
                enabled: true,
                scale: f64::NEG_INFINITY,
            },
            ..EngineConfig::default()
        },
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
    .unwrap();
    let budget = CheckpointBudget::default();
    let saved = engine
        .export_social_checkpoint(at(0.0), budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let charge = budget.retained_bytes();
    let context = engine.social_checkpoint_context(at(0.0));
    let original: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    assert_eq!(original["stage"]["radius_m"]["bits"], radius_bits);
    assert_eq!(original["stage"]["max_actors"], usize::MAX as u64);
    for case in 0..4 {
        let mut value = original.clone();
        match case {
            0 => value["unrecognized_authority"] = true.into(),
            1 => {
                assert!(
                    value["conversation"]
                        .as_object_mut()
                        .unwrap()
                        .remove("invitation")
                        .is_some()
                );
            }
            2 => {
                value["player_id"] = "sv3n1".into();
            }
            _ => {
                value["warm_exchanges"]["pairs"] = serde_json::json!([
                    {"a":"historicala","b":"historicalb","at":0.0},
                    {"a":"historicala","b":"historicalb","at":0.0},
                ]);
            }
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            EngineSocialDtoV1::decode(
                &bytes,
                budget
                    .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                    .unwrap(),
                context,
            )
            .is_err()
        );
        assert_eq!(budget.retained_bytes(), charge);
    }
    let padding = 1024 * 1024;
    let mut bytes = vec![b' '; padding];
    bytes.extend_from_slice(saved.value());
    let decoded = EngineSocialDtoV1::decode(
        &bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let canonical_peak = decoded.value().cost().unwrap().peak_bytes;
    let retained = budget.retained_bytes();
    assert!(retained - charge >= canonical_peak + 3 * padding);
    let candidate = decoded.into_candidate(context).unwrap();
    assert_eq!(candidate.value().player_id(), &actor("player"));
    assert_eq!(candidate.value().idle_mode(), IdleCognitionMode::Stage);
    assert_eq!(candidate.value().stage().radius_m.to_bits(), radius_bits);
    assert_eq!(candidate.value().stage().max_actors, usize::MAX);
    assert!(candidate.value().idle_requires_news());
    assert!(candidate.value().idle_curiosity().enabled);
    assert_eq!(
        candidate.value().idle_curiosity().scale.to_bits(),
        f64::NEG_INFINITY.to_bits()
    );
    assert_eq!(budget.retained_bytes(), retained);
    drop((candidate, saved));
    assert_eq!(budget.retained_bytes(), 0);
}
