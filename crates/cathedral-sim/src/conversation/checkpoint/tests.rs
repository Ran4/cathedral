use super::*;
use crate::checkpoint::{CheckpointBudget, Cohort};
fn id(s: &str) -> ActorId {
    ActorId::from_raw(s)
}
fn now(t: f64) -> LogicalTime {
    LogicalTime::new(t).unwrap()
}
fn world() -> World {
    crate::seed::build_world(
        &crate::WorldSeed::from_json_str(include_str!("../../../tests/fixtures/demo_seed.json"))
            .unwrap(),
        crate::WorldConfig::default(),
    )
}
fn prose() -> ConversationStrings {
    crate::PromptEnv::new(
        include_str!("../../../../../assets/prompts/turn.j2"),
        include_str!("../../../../../assets/prompts/night.j2"),
        include_str!("../../../../../assets/prompts/strings.toml"),
    )
    .unwrap()
    .strings()
    .conversation
    .clone()
}
fn bytes(c: &Conversation, t: f64) -> Vec<u8> {
    c.export_checkpoint(
        now(t),
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
fn restored(c: &Conversation, t: f64) -> Conversation {
    let raw = bytes(c, t);
    let budget = CheckpointBudget::default();
    let candidate = ConversationDtoV1::decode(
        &raw,
        budget
            .reserve(Cohort::LoadCandidate, raw.len() + 4096)
            .unwrap(),
        now(t),
    )
    .unwrap()
    .into_candidate(now(t))
    .unwrap();
    let mut other = Conversation::default();
    other.player_addressed(88.0, &id("scrambled"), &[id("wrong")]);
    other.observe_focus(77.0, Some(id("wrong")));
    assert_ne!(bytes(&other, t), raw);
    other = candidate.value().conversation().clone();
    assert_eq!(
        bytes(&other, t),
        raw,
        "immediate exact private installation"
    );
    other
}
#[test]
fn checkpoint_social_conversation_partial_backwards_and_future_anchors_are_exact() {
    let w = world();
    let actor = id("sv3n1");
    let mut a = Conversation::default();
    a.observe_focus(0.0, Some(actor.clone()));
    a.observe_focus(0.3, Some(actor.clone()));
    let mut b = restored(&a, 0.3);
    for t in [0.64, 0.65] {
        a.observe_focus(t, Some(actor.clone()));
        b.observe_focus(t, Some(actor.clone()));
        let x = a.capture(t, &w);
        let y = b.capture(t, &w);
        assert_eq!(x, y);
        assert_eq!(x.focus.is_some(), t >= 0.65);
    }
    a.observe_focus(9.0, Some(actor.clone()));
    a.observe_focus(8.0, Some(actor));
    assert_eq!(a.focus.as_ref().unwrap().1, 9.0);
    let mut b = restored(&a, 0.0);
    assert_eq!(a.capture(9.66, &w), b.capture(9.66, &w));
    a.focus = Some((id("historical"), -0.0, f64::from_bits(1)));
    assert_eq!(
        restored(&a, 0.0).focus.unwrap().1.to_bits(),
        (-0.0f64).to_bits()
    );
}
#[test]
fn checkpoint_social_conversation_independent_invitation_witnesses_and_expiry() {
    let w = world();
    let mut a = Conversation::default();
    let target = id("sv3n1");
    a.player_addressed(1.0, &target, &[id("cb947")]);
    assert!(!a.engagement.as_ref().unwrap().witnesses.contains(&target));
    a.npc_addressed_player(2.0, &target, &[id("cb947")]);
    a.npc_addressed_player(3.0, &id("k0fb1"), &[]);
    let mut b = restored(&a, 3.0);
    assert!(b.engagement.as_ref().unwrap().reciprocal);
    assert_eq!(b.invitation.as_ref().unwrap().0, id("k0fb1"));
    for t in [3.0, 12.999, 13.0, 31.999, 32.0, 200.0] {
        assert_eq!(a.partner(t, &w), b.partner(t, &w));
        let x = a.capture(t, &w);
        let y = b.capture(t, &w);
        assert_eq!(x, y);
        let at = w.characters[&id("player")].position_m();
        let x = x.select(t, &w, &id("player"), at, "And then?");
        let y = y.select(t, &w, &id("player"), at, "And then?");
        for who in ["sv3n1", "cb947", "k0fb1"] {
            assert_eq!(
                x.percept_suffix(&w, &id(who), &prose()),
                y.percept_suffix(&w, &id(who), &prose())
            );
        }
    }
    assert!(
        b.engagement.is_some(),
        "queries must not erase expired evidence"
    );
}
#[test]
fn checkpoint_social_conversation_old_capture_cannot_override_same_time_group_and_saturates() {
    let mut wa = world();
    let mut wb = world();
    let mut a = Conversation::default();
    a.player_addressed(0.0, &id("sv3n1"), &[]);
    a.observe_focus(0.0, Some(id("cb947")));
    a.observe_focus(0.65, Some(id("cb947")));
    let old = a.capture(0.65, &wa);
    let group = a.capture(0.65, &wa);
    let mut sa = NpcScheduler::new(vec![id("sv3n1"), id("cb947"), id("k0fb1")], 0.0, 10.0, 0.0);
    let mut sb = NpcScheduler::new(vec![id("sv3n1"), id("cb947"), id("k0fb1")], 0.0, 10.0, 0.0);
    let at = wa.characters[&id("player")].position_m();
    let mut pre = a.clone();
    super::super::speak(
        0.65,
        &mut wa,
        &mut sa,
        &mut a,
        &id("player"),
        &group,
        at,
        "Does anyone know?",
        &prose(),
    )
    .unwrap();
    super::super::speak(
        0.65,
        &mut wb,
        &mut sb,
        &mut pre,
        &id("player"),
        &group,
        at,
        "Does anyone know?",
        &prose(),
    )
    .unwrap();
    let mut b = restored(&a, 0.65);
    assert_eq!(a.latest_applied_utterance, group.sequence);
    let x = super::super::speak(
        0.65,
        &mut wa,
        &mut sa,
        &mut a,
        &id("player"),
        &old,
        at,
        "And then?",
        &prose(),
    )
    .unwrap();
    let y = super::super::speak(
        0.65,
        &mut wb,
        &mut sb,
        &mut b,
        &id("player"),
        &old,
        at,
        "And then?",
        &prose(),
    )
    .unwrap();
    assert_eq!(x, y);
    assert_eq!(a, b);
    assert_eq!(b.engagement.as_ref().unwrap().actor, id("sv3n1"));
    a.next_utterance = u64::MAX;
    a.latest_applied_utterance = u64::MAX;
    let mut b = restored(&a, 1.0);
    assert_eq!(a.capture(1.0, &wa), b.capture(1.0, &wb));
    b.player_addressed(1.0, &id("historical"), &[]);
    assert_eq!(b.next_utterance, u64::MAX);
}
#[test]
fn checkpoint_social_conversation_max_witnesses_and_layout() {
    let mut a = Conversation::default();
    let witnesses: Vec<_> = (0..130).map(|n| id(&format!("w{n:04}"))).collect();
    a.player_addressed(10.0, &id("zzzzz"), &witnesses);
    assert_eq!(a.engagement.as_ref().unwrap().witnesses.len(), 128);
    assert!(
        !a.engagement
            .as_ref()
            .unwrap()
            .witnesses
            .contains(&id("zzzzz"))
    );
    let b = restored(&a, 0.0);
    assert_eq!(a, b);
    for n in [0, 1, 2, 5, 6, 11, 12, 128] {
        a.engagement.as_mut().unwrap().witnesses = witnesses[..n].iter().cloned().collect();
        let d = a
            .export_checkpoint(
                now(0.0),
                CheckpointBudget::default()
                    .reserve(Cohort::SavePayload, 4096)
                    .unwrap(),
            )
            .unwrap();
        let e = d.value().cost().unwrap().expanded_upper_bytes;
        // Empty cloned BTreeSet has no root. Sparse first root has 11 slots;
        // subsequent nodes have >=5 entries, including room for 12 pointers.
        let nodes = if n == 0 { 0 } else { (n - 1) / 5 + 1 };
        let resident = std::mem::size_of::<ConversationDtoV1>()
            + nodes * (64 + 11 * std::mem::size_of::<ActorId>() + 12 * 8)
            + n * 6;
        assert!(resident <= e, "{n}: {resident} > {e}");
    }
    println!(
        "social_layout conversation={} engagement={} captured_attention_excluded={} conversation_dto={} conversation_candidate={}",
        std::mem::size_of::<Conversation>(),
        std::mem::size_of::<Engagement>(),
        std::mem::size_of::<CapturedAttention>(),
        std::mem::size_of::<ConversationDtoV1>(),
        std::mem::size_of::<ConversationCandidate>()
    );
}
#[test]
fn checkpoint_social_conversation_strict_and_raw_charge_retained() {
    let base = bytes(&Conversation::default(), 0.0);
    let raw = String::from_utf8(base.clone()).unwrap();
    let malformed = [
        raw.replace("\"focus\":null,", ""),
        raw.replace("\"invitation\":null,", ""),
        raw.replace("\"engagement\":null,", ""),
        raw.replace(
            "\"next_utterance\":0",
            "\"next_utterance\":0,\"next_utterance\":0",
        ),
        raw.replace(
            "\"focus\":null",
            "\"focus\":{\"actor\":\"actor\",\"since\":0,\"last\":0,\"extra\":1}",
        ),
        raw.replace(
            "\"latest_applied_utterance\":0",
            "\"latest_applied_utterance\":1",
        ),
    ];
    for raw in malformed {
        let b = CheckpointBudget::default();
        assert!(
            ConversationDtoV1::decode(
                raw.as_bytes(),
                b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
                now(0.0)
            )
            .is_err(),
            "{raw}"
        );
        assert_eq!(b.retained_bytes(), 0);
    }
    let b = CheckpointBudget::default();
    let mut padded = base.clone();
    padded.extend(std::iter::repeat_n(b' ', 50_000));
    let d = ConversationDtoV1::decode(
        &padded,
        b.reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        now(0.0),
    )
    .unwrap();
    let charge = b.retained_bytes();
    assert!(charge > d.value().cost().unwrap().peak_bytes);
    let c = d.into_candidate(now(0.0)).unwrap();
    assert_eq!(b.retained_bytes(), charge);
    drop(c);
    assert_eq!(b.retained_bytes(), 0);
    assert!(
        ConversationDtoV1::decode(
            &base,
            b.reserve(Cohort::SavePayload, base.len() + 4096).unwrap(),
            now(0.0)
        )
        .is_err()
    );
    let all = b
        .reserve(
            Cohort::Running,
            crate::checkpoint::MAX_RESIDENT_BYTES - 4096,
        )
        .unwrap();
    assert!(
        Conversation::default()
            .export_checkpoint(now(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
            .is_err()
    );
    drop(all);
    assert_eq!(b.retained_bytes(), 0);
}

#[test]
fn checkpoint_social_conversation_v1_fixture_and_malformed_witnesses() {
    let raw = include_bytes!("../../../tests/fixtures/checkpoint_social/conversation-v1.json");
    let b = CheckpointBudget::default();
    let d = ConversationDtoV1::decode(
        raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        now(0.0),
    )
    .unwrap()
    .into_candidate(now(0.0))
    .unwrap();
    assert_eq!(bytes(d.value().conversation(), 0.0), raw);
    let mut value: serde_json::Value = serde_json::from_slice(raw).unwrap();
    for witnesses in [
        vec!["dup".to_string(), "dup".to_string()],
        (0..129).map(|n| format!("w{n:04}")).collect(),
    ] {
        value["state"]["engagement"]["witnesses"] = serde_json::json!(witnesses);
        let malformed = serde_json::to_vec(&value).unwrap();
        let b = CheckpointBudget::default();
        assert!(
            ConversationDtoV1::decode(
                &malformed,
                b.reserve(Cohort::LoadCandidate, malformed.len() + 4096)
                    .unwrap(),
                now(0.0)
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
