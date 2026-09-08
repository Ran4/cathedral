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
fn warm_bytes(w: &WarmExchanges) -> Vec<u8> {
    w.export_checkpoint(
        now(0.0),
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
fn novelty_bytes(n: &Novelty) -> Vec<u8> {
    n.export_checkpoint(
        now(0.0),
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
fn warm_restore(w: &WarmExchanges) -> WarmExchanges {
    let raw = warm_bytes(w);
    let b = CheckpointBudget::default();
    let c = WarmExchangesDtoV1::decode(
        &raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        now(0.0),
    )
    .unwrap()
    .into_candidate(now(0.0))
    .unwrap();
    let mut other = WarmExchanges::default();
    other.note(&id("wrong"), &id("scrambled"), 999.0);
    assert_ne!(warm_bytes(&other), raw);
    other = c.value().warm_exchanges().clone();
    assert_eq!(warm_bytes(&other), raw);
    other
}
fn novelty_restore(n: &Novelty) -> Novelty {
    let raw = novelty_bytes(n);
    let b = CheckpointBudget::default();
    let c = NoveltyDtoV1::decode(
        &raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        now(0.0),
    )
    .unwrap()
    .into_candidate(now(0.0))
    .unwrap();
    let mut other = Novelty::default();
    other.observe(999.0, &[id("wrong")].into_iter().collect());
    assert_ne!(novelty_bytes(&other), raw);
    other = c.value().novelty().clone();
    assert_eq!(novelty_bytes(&other), raw);
    other
}
#[test]
fn checkpoint_social_warm_aliases_historical_expiry_and_forget() {
    let mut a = WarmExchanges::default();
    a.note(&id("lost-a"), &id("lost-b"), 0.0);
    a.note(&id("lost-b"), &id("lost-a"), 1.0);
    a.note(&id("self"), &id("self"), 2.0);
    assert_eq!(a.pairs.len(), 1);
    let mut b = warm_restore(&a);
    assert_eq!(a.warm_actors(30.999), b.warm_actors(30.999));
    assert_eq!(b.pairs.len(), 1);
    assert_eq!(a.warm_actors(31.0), b.warm_actors(31.0));
    assert!(b.pairs.is_empty());
    a.note(
        &id("future"),
        &id("past"),
        crate::checkpoint::MAX_LOGICAL_SECONDS,
    );
    b = warm_restore(&a);
    assert_eq!(a.warm_actors(0.0), b.warm_actors(0.0));
    a.forget(&[id("future")]);
    b.forget(&[id("future")]);
    assert_eq!(warm_bytes(&a), warm_bytes(&b));
}
#[test]
fn checkpoint_social_novelty_opaque_bits_submission_late_inbox_and_inclusive_expiry() {
    let mut w = world();
    let actor = id("sv3n1");
    let stage = [actor.clone()].into_iter().collect();
    for bits in [0, (-0.0f64).to_bits(), 0x7ff8000000001234, u64::MAX] {
        let mut a = Novelty::default();
        a.told(f64::from_bits(bits), &w, &actor);
        // A finite later touch keeps the original opaque salt, even if its
        // initiating public told call supplied a NaN timestamp.
        a.observe(5.0, &stage);
        assert_eq!(a.last_told[&actor].visit, bits);
        let mut b = novelty_restore(&a);
        assert_eq!(b.last_told[&actor].visit, bits);
        assert_eq!(
            a.admits_idle(&w, &actor, &CuriosityConfig::default()),
            b.admits_idle(&w, &actor, &CuriosityConfig::default())
        );
        assert!(!b.has_news(&w, &actor));
        w.characters
            .get_mut(&actor)
            .unwrap()
            .notify_percept("arrived after submission");
        assert!(a.has_news(&w, &actor) && b.has_news(&w, &actor));
        w.characters.get_mut(&actor).unwrap().state.inbox.clear();
        for t in [65.0, 65.0000001] {
            a.observe(t, &BTreeSet::new());
            b.observe(t, &BTreeSet::new());
            assert_eq!(novelty_bytes(&a), novelty_bytes(&b));
            assert_eq!(b.last_told.contains_key(&actor), t == 65.0);
        }
    }
    let mut a = Novelty::default();
    a.observe(0.0, &stage);
    assert_eq!(a.last_told[&actor].context, None);
    a.last_told.insert(
        id("historical"),
        Memory {
            context: Some(u64::MAX),
            visit: 0x7ff8000000001234,
            touched_at: 900.0,
        },
    );
    let mut b = novelty_restore(&a);
    assert_eq!(b.last_told[&id("historical")].context, Some(u64::MAX));
    a.forget(&[actor.clone()]);
    b.forget(&[actor]);
    assert_eq!(novelty_bytes(&a), novelty_bytes(&b));
}
#[test]
fn checkpoint_social_attention_strict_duplicate_unknown_and_required_null() {
    let mut w = WarmExchanges::default();
    w.note(&id("a"), &id("b"), 1.0);
    let raw = String::from_utf8(warm_bytes(&w)).unwrap();
    let row = r#"{"a":"a","b":"b","at":1.0}"#;
    for bad in [
        raw.replace(row, &format!("{row},{row}")),
        raw.replace("\"a\":\"a\"", "\"a\":\"b\""),
        raw.replace("\"b\":\"b\"", "\"b\":\"0\""),
        raw.replace("\"at\":1.0", "\"at\":1.0,\"unknown\":0"),
        raw.replace("\"at\":1.0", "\"at\":1.0,\"at\":1.0"),
    ] {
        assert_ne!(bad, raw);
        let b = CheckpointBudget::default();
        assert!(
            WarmExchangesDtoV1::decode(
                bad.as_bytes(),
                b.reserve(Cohort::LoadCandidate, bad.len() + 4096).unwrap(),
                now(0.0)
            )
            .is_err(),
            "{bad}"
        );
        assert_eq!(b.retained_bytes(), 0);
    }
    let mut n = Novelty::default();
    n.observe(0.0, &[id("a")].into_iter().collect());
    let raw = String::from_utf8(novelty_bytes(&n)).unwrap();
    let row = r#""a":{"context":null,"visit":0,"touched_at":0.0}"#;
    for bad in [
        raw.replace(row, &format!("{row},{row}")),
        raw.replace("\"context\":null,", ""),
        raw.replace("\"visit\":0", "\"visit\":0,\"unknown\":false"),
        raw.replace("\"context\":null", "\"context\":null,\"context\":null"),
    ] {
        assert_ne!(bad, raw);
        let b = CheckpointBudget::default();
        assert!(
            NoveltyDtoV1::decode(
                bad.as_bytes(),
                b.reserve(Cohort::LoadCandidate, bad.len() + 4096).unwrap(),
                now(0.0)
            )
            .is_err(),
            "{bad}"
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn checkpoint_social_attention_sparse_and_max_shapes_fit_layout_and_refuse_overflow() {
    for n in [0, 1, 2, 5, 6, 11, 12, 25_000] {
        let mut w = WarmExchanges::default();
        let mut a = Novelty::default();
        for i in 0..n {
            let left = id(&format!("a{i:05}"));
            let right = id(&format!("b{i:05}"));
            w.note(&left, &right, 1.0);
            a.last_told.insert(
                left,
                Memory {
                    context: (i % 2 == 0).then_some(i as u64),
                    visit: u64::MAX - i as u64,
                    touched_at: -0.0,
                },
            );
        }
        let wb = warm_restore(&w);
        let ab = novelty_restore(&a);
        assert_eq!(wb.pairs.len(), n);
        assert_eq!(ab.last_told.len(), n);
        let nodes = if n == 0 { 0 } else { (n - 1) / 5 + 1 };
        let warm_resident = std::mem::size_of::<WarmExchangesDtoV1>()
            + nodes * (64 + 11 * std::mem::size_of::<((ActorId, ActorId), f64)>() + 12 * 8)
            + n * 12;
        let novelty_resident = std::mem::size_of::<NoveltyDtoV1>()
            + nodes * (64 + 11 * std::mem::size_of::<(ActorId, Memory)>() + 12 * 8)
            + n * 6;
        let b = CheckpointBudget::default();
        let d = w
            .export_checkpoint(now(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
            .unwrap();
        assert!(warm_resident <= d.value().cost().unwrap().expanded_upper_bytes);
        drop(d);
        let d = a
            .export_checkpoint(now(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
            .unwrap();
        assert!(novelty_resident <= d.value().cost().unwrap().expanded_upper_bytes);
        drop(d);
        if n == 25_000 {
            w.note(&id("over-a"), &id("over-b"), 0.0);
            a.observe(0.0, &[id("overflow")].into_iter().collect());
            assert!(
                w.export_checkpoint(now(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
                    .is_err()
            );
            assert!(
                a.export_checkpoint(now(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
                    .is_err()
            );
        }
    }
    println!(
        "social_layout warm={} novelty={} memory={} warm_pair_slot={} novelty_pair_slot={} warm_dto={} novelty_dto={}",
        std::mem::size_of::<WarmExchanges>(),
        std::mem::size_of::<Novelty>(),
        std::mem::size_of::<Memory>(),
        std::mem::size_of::<((ActorId, ActorId), f64)>(),
        std::mem::size_of::<(ActorId, Memory)>(),
        std::mem::size_of::<WarmExchangesDtoV1>(),
        std::mem::size_of::<NoveltyDtoV1>()
    );
}

#[test]
fn checkpoint_social_attention_v1_fixtures_malformed_ids_anchors_and_raw_caps() {
    let raw = include_bytes!("../../../tests/fixtures/checkpoint_social/warm-v1.json");
    let b = CheckpointBudget::default();
    let d = WarmExchangesDtoV1::decode(
        raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        now(0.0),
    )
    .unwrap()
    .into_candidate(now(0.0))
    .unwrap();
    assert_eq!(warm_bytes(d.value().warm_exchanges()), raw);
    drop(d);
    let raw = include_bytes!("../../../tests/fixtures/checkpoint_social/novelty-v1.json");
    let d = NoveltyDtoV1::decode(
        raw,
        b.reserve(Cohort::LoadCandidate, raw.len() + 4096).unwrap(),
        now(0.0),
    )
    .unwrap()
    .into_candidate(now(0.0))
    .unwrap();
    assert_eq!(novelty_bytes(d.value().novelty()), raw);
    drop(d);
    for key in [
        "".to_string(),
        "bad\nidentity".to_string(),
        "a".repeat(2_000_000),
    ] {
        let malformed=serde_json::to_vec(&serde_json::json!({"version":1,"boundary":0.0,"state":{"last_told":{key:{"context":null,"visit":0,"touched_at":0.0}}}})).unwrap();
        assert!(
            NoveltyDtoV1::decode(
                &malformed,
                b.reserve(Cohort::LoadCandidate, malformed.len() + 4096)
                    .unwrap(),
                now(0.0)
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
    for at in [-1.0, crate::checkpoint::MAX_LOGICAL_SECONDS + 1.0] {
        let mut w = WarmExchanges::default();
        w.note(&id("a"), &id("b"), at);
        assert!(
            w.export_checkpoint(now(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
                .is_err()
        );
    }
    let rows: serde_json::Map<String, serde_json::Value> = (0..25_001)
        .map(|i| {
            (
                format!("a{i:05}"),
                serde_json::json!({"context":null,"visit":0,"touched_at":0.0}),
            )
        })
        .collect();
    let malformed = serde_json::to_vec(
        &serde_json::json!({"version":1,"boundary":0.0,"state":{"last_told":rows}}),
    )
    .unwrap();
    assert!(
        NoveltyDtoV1::decode(
            &malformed,
            b.reserve(Cohort::LoadCandidate, malformed.len() + 4096)
                .unwrap(),
            now(0.0)
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
    let mut deep = vec![b'['; 65];
    deep.extend(std::iter::repeat_n(b']', 65));
    assert!(
        NoveltyDtoV1::decode(
            &deep,
            b.reserve(Cohort::LoadCandidate, deep.len() + 4096).unwrap(),
            now(0.0)
        )
        .is_err()
    );
    assert_eq!(b.retained_bytes(), 0);
}
