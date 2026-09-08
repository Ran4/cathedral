use super::*;
use crate::{
    Vec3, WorldConfig, WorldSeed,
    checkpoint::{CheckpointBudget, Cohort},
};
use serde_json::{Value, json};

pub(crate) fn actor(s: &str) -> ActorId {
    ActorId::from_raw(s)
}
pub(crate) fn world() -> World {
    let seed =
        WorldSeed::from_json_str(include_str!("../../../tests/fixtures/demo_seed.json")).unwrap();
    let mut w = crate::build_world(
        &seed,
        WorldConfig {
            area_map: crate::AreaMap::from_json_str(include_str!(
                "../../../../../assets/world/areas.json"
            ))
            .unwrap(),
            ..Default::default()
        },
    );
    w.current_time = Some(WorldTime::from_game_days(2.0));
    w.area_adjacency = Arc::new(AreaAdjacency::build(&w.area_map));
    w
}
pub(crate) fn mint(w: &mut World, n: usize, source: FactSource) -> FactKey {
    super::super::mint::mint(
        w,
        FactId::from_raw(format!("checkpoint.{n}")),
        Topic::Talk,
        format!("A quiet word {n} passed through the city."),
        vec![],
        Vec3::new(100000.0, 0.0, 100000.0),
        GarbleMask::ALL,
        true,
        source,
        Some(2.0),
    )
    .unwrap()
}
pub(crate) fn active(w: &mut World) {
    w.knowledge = Knowledge::default();
    let historical = mint(w, 99, FactSource::custody(actor("departed")));
    learn(
        w,
        &actor("player"),
        historical,
        Telling {
            hops: 2,
            from: Some(actor("departed")),
            heat: 1.0,
            view: Default::default(),
        },
        Some(2.0),
    );
    w.knowledge.note_seated(&actor("sv3n1"), vec![historical]);
    w.knowledge.note_occasion(
        &actor("sv3n1"),
        Some(actor("departed")),
        Some(actor("departed")),
        2.0,
    );
    w.knowledge.offer_occasion(&actor("sv3n1"));
    w.knowledge.invalidate(historical);
    let sources = [
        FactSource::authored(),
        FactSource::claimed(actor("departed")),
        FactSource::custody(actor("departed")),
        FactSource::item_with(crate::ItemId::from_raw("old_item"), actor("departed")),
        FactSource::quest_phase("sealed-test-phase", 3),
        FactSource::event("sealed-test-event", -7),
    ];
    let people: Vec<_> = w.characters.keys().cloned().collect();
    for (n, source) in sources.into_iter().enumerate() {
        let key = mint(w, n, source);
        for who in &people {
            learn(
                w,
                who,
                key,
                Telling {
                    hops: 2,
                    from: Some(actor("departed")),
                    heat: 1.0,
                    view: Default::default(),
                },
                Some(2.0),
            );
        }
        for ward in PlanningWard::ALL {
            w.knowledge
                .deposit(ward, key, 2, 0.8, &actor("departed"), 2.0);
        }
    }
    // Public Fact fields may be authored or runtime-mutated independently of
    // their frozen prompt context; retain that context, not a fresh catalog seed.
    let place = (0..w.area_map.areas.len())
        .map(|i| AreaKey(i as u16))
        .find(|k| !w.area_adjacency.neighbours(*k).is_empty())
        .unwrap();
    let fact = w.knowledge.live.get_mut(&FactKey(1)).unwrap();
    fact.subject = vec![actor("cb947")];
    fact.place = Some(place);
    fact.day = Some(1);
    fact.said = "{subject} was seen at {place} {day}.".into();
    fact.own
        .insert(actor("departed"), "I remember this differently.".into());
    fact.quiet_among.insert(actor("departed"));
    fact.craft_ear = Some("a retired craft".into());
    let view = super::super::garble::view_for(
        w,
        w.knowledge.fact(FactKey(1)).unwrap(),
        &actor("player"),
        16,
    );
    let row = Arc::make_mut(&mut w.knowledge.holdings)
        .get_mut(&actor("player"))
        .unwrap()
        .iter_mut()
        .find(|h| h.key == FactKey(1))
        .unwrap();
    row.hops = 16;
    row.view = view;
    w.knowledge
        .note_raise(&actor("departed"), -4, Office::Watch);
    w.knowledge.last_sweep_game_days = 2.0;
    w.knowledge.last_hearsay_beat_game_days = 1.5;
    w.knowledge.player_stage_stir = Some(u32::MAX);
    w.knowledge
        .player_stage_tellings
        .insert((FactKey(1), actor("departed")));
}
fn now() -> LogicalTime {
    LogicalTime::new(10.0).unwrap()
}
fn bytes(w: &World) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    w.export_knowledge_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn decode(w: &World, bytes: &[u8]) -> Result<Admitted<WorldKnowledgeCandidate>> {
    let b = CheckpointBudget::default();
    let c = KnowledgeCheckpointContext::from_world(w, now());
    WorldKnowledgeDtoV1::decode(
        bytes,
        b.reserve(Cohort::LoadCandidate, bytes.len() + 4096)?,
        c,
    )?
    .into_candidate(c)
}
fn wire(w: &World) -> Value {
    serde_json::from_slice(bytes(w).value()).unwrap()
}
fn refusal(w: &World, v: Value, reason: &str) {
    let b = CheckpointBudget::default();
    let bytes = serde_json::to_vec(&v).unwrap();
    let c = KnowledgeCheckpointContext::from_world(w, now());
    let e = WorldKnowledgeDtoV1::decode(
        &bytes,
        b.reserve(Cohort::LoadCandidate, bytes.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap_err();
    assert!(e.reason.contains(reason), "{} expected {reason}", e.reason);
    assert_eq!(b.retained_bytes(), 0);
}

#[test]
fn every_source_sparse_handles_and_wrapping_stirs_round_trip() {
    let mut w = world();
    active(&mut w);
    w.knowledge.next_key = 901;
    w.knowledge.next_sequence = 1203;
    w.knowledge.receipts_revision = u64::MAX;
    Arc::make_mut(&mut w.knowledge.air)
        .values_mut()
        .for_each(|d| d.stir = u32::MAX);
    let saved = bytes(&w);
    let candidate = decode(&w, saved.value()).unwrap();
    assert_eq!(candidate.value().knowledge(), &w.knowledge);
    assert!(!format!("{:?}", candidate.value()).contains("sealed-test"));
    let mut resumed = w.clone();
    resumed.knowledge = candidate.value().data.knowledge.clone();
    for world in [&mut w, &mut resumed] {
        assert_eq!(
            world.knowledge.take_seated(&actor("sv3n1")),
            vec![FactKey(0)]
        );
        assert!(world.knowledge.take_seated(&actor("sv3n1")).is_empty());
        assert!(world.knowledge.spend_occasion(&actor("sv3n1")));
        super::super::pollen::sweep(world, 2.5);
        let key = mint(world, 20, FactSource::authored());
        assert_eq!(key, FactKey(901));
    }
    assert_eq!(w.knowledge, resumed.knowledge);
    assert_eq!(bytes(&w).value(), bytes(&resumed).value());
}

#[test]
fn invalid_sampled_time_refuses_export_and_releases() {
    for fraction in [f64::NAN, f64::INFINITY, -0.1, 1.0] {
        let mut w = world();
        w.current_time.as_mut().unwrap().fraction = fraction;
        let b = CheckpointBudget::default();
        let c = KnowledgeCheckpointContext::from_world(&w, now());
        assert!(
            w.knowledge
                .export_checkpoint(c, b.reserve(Cohort::SavePayload, 4096).unwrap())
                .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
        assert!(
            w.export_knowledge_checkpoint(now(), b.reserve(Cohort::SavePayload, 4096).unwrap())
                .is_err()
        );
        assert_eq!(b.retained_bytes(), 0);
    }
}

#[test]
fn strict_indices_numbers_nullable_fields_and_aggregate_admission() {
    let mut w = world();
    active(&mut w);
    let original = wire(&w);
    for (path, value, reason) in [
        ("/knowledge/next_key", json!(u32::MAX), "headroom"),
        ("/knowledge/next_sequence", json!(-1), "headroom"),
        ("/knowledge/live/1/sequence", json!(1), "sequence"),
        ("/knowledge/by_id/0/1", json!(200), "index"),
        ("/knowledge/holdings/0/rows/0/key", json!(777), "ordered"),
        (
            "/knowledge/holdings/0/rows/0/heat_at_learn",
            json!(1.01),
            "heat",
        ),
        (
            "/knowledge/holdings/0/rows/0/view/day_offset",
            json!(4),
            "day offset",
        ),
        (
            "/knowledge/last_sweep_game_days",
            json!({"at":null}),
            "invalid type",
        ),
        ("/area_adjacency/0", json!([0]), "adjacency"),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        refusal(&w, v, reason);
    }
    for path in [
        "/knowledge/live/0",
        "/knowledge/holdings/0",
        "/knowledge/air/0",
        "/knowledge/by_id/0",
    ] {
        let mut v = original.clone();
        let row = v.pointer(path).unwrap().clone();
        let parent = path.rsplit_once('/').unwrap().0;
        v.pointer_mut(parent)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(row);
        refusal(&w, v, "duplicate");
    }
    for (path, field) in [
        ("/knowledge/live/0", "minted_game_days"),
        ("/knowledge/live/0", "craft_ear"),
        ("/knowledge/holdings/0/rows/0", "from"),
        ("/knowledge/air/0/drift", "via"),
        ("/knowledge", "seated"),
    ] {
        let mut v = original.clone();
        v.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        refusal(&w, v, "missing field");
    }
    let mut v = original.clone();
    v["unexpected"] = json!(true);
    refusal(&w, v, "unknown field");
    let b = CheckpointBudget::default();
    let c = KnowledgeCheckpointContext::from_world(&w, now());
    for (s, reason) in [
        (
            format!("{{\"x\":{}0{}}}", "[".repeat(65), "]".repeat(65)),
            "nesting",
        ),
        (
            format!("{{\"x\":[{}]}}", vec!["[]"; 270000].join(",")),
            "aggregate expanded",
        ),
    ] {
        let e = WorldKnowledgeDtoV1::decode(
            s.as_bytes(),
            b.reserve(Cohort::LoadCandidate, s.len() + 4096).unwrap(),
            c,
        )
        .unwrap_err();
        assert!(e.reason.contains(reason), "{e}");
        assert_eq!(b.retained_bytes(), 0);
    }
}

#[test]
fn receipts_allow_65_tellings_and_reject_inconsistent_ward_bits() {
    let mut w = world();
    active(&mut w);
    let r = w.knowledge.player_learned.values_mut().next().unwrap();
    r.mouths_seen = (0..64).map(|i| actor(&format!("old_{i}"))).collect();
    r.unattributed_seen = true;
    r.tellings = 65;
    r.wards_seen = 255;
    r.wards = 8;
    let saved = bytes(&w);
    assert_eq!(
        decode(&w, saved.value()).unwrap().value().knowledge(),
        &w.knowledge
    );
    let mut v = wire(&w);
    v["knowledge"]["player_learned"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["wards"] = json!(7);
    refusal(&w, v, "ward count");
}

#[test]
fn default_adjacency_and_catalog_larger_than_live_cap_are_supported() {
    let mut w = world();
    w.area_adjacency = Arc::new(AreaAdjacency::default());
    let specs: Vec<_> = (0..300)
        .map(|n| json!({"id":format!("pack.{n}"),"said":"a quiet word","topic":"talk"}))
        .collect();
    w.fact_catalog = Arc::new(
        FactCatalog::from_json(&json!({"schema_version":1,"facts":specs}).to_string()).unwrap(),
    );
    let saved = bytes(&w);
    let c = decode(&w, saved.value()).unwrap();
    assert_eq!(
        super::super::pollen::checkpoint::rows(&c.value().data.area_adjacency),
        0
    );
    assert!(c.value().knowledge().is_empty());
    w.area_adjacency = Arc::new(AreaAdjacency::build(&w.area_map));
    assert!(decode(&w, bytes(&w).value()).is_ok());
}

#[test]
fn maximum_live_air_holdings_receipts_and_eviction_continue() {
    let mut w = world();
    w.knowledge = Knowledge::default();
    for i in 0..FACTS_MAX_LIVE {
        let key = mint(&mut w, i, FactSource::authored());
        learn(
            &mut w,
            &actor("player"),
            key,
            Telling {
                hops: 3,
                from: Some(actor("departed")),
                heat: 0.8,
                view: Default::default(),
            },
            Some(2.0),
        );
        for ward in PlanningWard::ALL {
            w.knowledge
                .deposit(ward, key, 3, 0.8, &actor("departed"), 2.0);
        }
    }
    assert_eq!(w.knowledge.len(), 256);
    assert_eq!(w.knowledge.holdings_len(&actor("player")), 6);
    assert_eq!(w.knowledge.player_learned.len(), 64);
    assert_eq!(w.knowledge.air_entries(), 192);
    let candidate = decode(&w, bytes(&w).value()).unwrap();
    let mut resumed = w.clone();
    resumed.knowledge = candidate.value().data.knowledge.clone();
    for w in [&mut w, &mut resumed] {
        assert_eq!(mint(w, 999, FactSource::authored()), FactKey(256));
    }
    assert_eq!(w.knowledge, resumed.knowledge);
}

#[test]
fn frozen_context_and_garbled_view_continue_through_closer_telling() {
    let mut w = world();
    active(&mut w);
    let key = FactKey(1);
    let before = holds_key(&w, &actor("player"), key).unwrap();
    assert!(!before.view.is_pristine());
    let decoded = decode(&w, bytes(&w).value()).unwrap();
    let mut resumed = w.clone();
    resumed.knowledge = decoded.value().data.knowledge.clone();
    for hops in [7, 3, 1] {
        for world in [&mut w, &mut resumed] {
            let view = super::super::garble::view_for(
                world,
                world.knowledge.fact(key).unwrap(),
                &actor("player"),
                hops,
            );
            learn(
                world,
                &actor("player"),
                key,
                Telling {
                    hops,
                    view,
                    from: Some(actor("old_mouth")),
                    heat: 0.9,
                },
                Some(2.1),
            );
        }
        assert_eq!(w.knowledge, resumed.knowledge);
        assert_eq!(
            render_plain(
                &w,
                &actor("player"),
                key,
                &holds_key(&w, &actor("player"), key).unwrap(),
                Some(2.1)
            ),
            render_plain(
                &resumed,
                &actor("player"),
                key,
                &holds_key(&resumed, &actor("player"), key).unwrap(),
                Some(2.1)
            )
        );
    }
}

#[test]
fn actual_bounded_text_field_enforces_utf8_bytes_and_decode_scratch() {
    let mut w = world();
    active(&mut w);
    let max = crate::checkpoint::records::MAX_TEXT_BYTES;
    w.knowledge.live.get_mut(&FactKey(1)).unwrap().said = "é".repeat(max / 2);
    assert!(decode(&w, bytes(&w).value()).is_ok());
    let mut v = wire(&w);
    v["knowledge"]["live"][0]["said"] = json!("é".repeat(max / 2 + 1));
    refusal(&w, v, "string byte limit");
    let v = wire(&w);
    let raw = serde_json::to_string(&v).unwrap();
    let escaped = raw.replace(&"é".repeat(max / 2), &"\\u00e9".repeat(max / 2 + 1));
    let b = CheckpointBudget::default();
    let c = KnowledgeCheckpointContext::from_world(&w, now());
    let error = WorldKnowledgeDtoV1::decode(
        escaped.as_bytes(),
        b.reserve(Cohort::LoadCandidate, escaped.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap_err();
    assert!(error.reason.contains("string byte limit"));
    assert_eq!(b.retained_bytes(), 0);
}

#[test]
#[ignore = "diagnostic fixture writer; updates only this owner fixture"]
fn write_knowledge_fixture() {
    let mut w = world();
    active(&mut w);
    let v = wire(&w);
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/knowledge.json"
        ),
        serde_json::to_string_pretty(&v).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
#[ignore = "target-specific closed-layout evidence"]
fn knowledge_layout() {
    macro_rules! row {($($t:ty),*)=>{$(println!("{}={}",stringify!($t),std::mem::size_of::<$t>());)*}}
    row!(
        Fact,
        Holding,
        Drift,
        LearnedHow,
        Knowledge,
        KnowledgeDtoV1,
        WorldKnowledgeDtoV1,
        crate::areas::NearestArea<'static>,
        (&crate::areas::Area, usize, &crate::areas::AreaBox),
        crate::homes::HomesDoc,
        crate::homes::HomeEntry,
        crate::homes::BedlessEntry
    );
    records::layout();
    super::super::pollen::checkpoint::layout_maximum_adjacency();
    let homes = aggregate::inspect(crate::homes::HOMES_JSON.as_bytes()).unwrap();
    println!(
        "homes={homes:?}; definition_scratch={}",
        128 * 1024 + 2 * homes.expanded_upper_bytes + 3 * homes.encoded_bytes
    );
    assert!(std::mem::size_of::<Fact>() <= 512);
    assert!(std::mem::size_of::<Holding>() <= 128);
    assert!(std::mem::size_of::<LearnedHow>() <= 192);
}

#[test]
fn supported_knowledge_fixture_exact_decode_and_reexport() {
    let mut w = world();
    active(&mut w);
    let fixture = include_bytes!("../../../tests/fixtures/checkpoint_v1/knowledge.json");
    let expected: Value = serde_json::from_slice(fixture).unwrap();
    assert!(wire(&w) == expected, "knowledge fixture differs");
    let c = decode(&w, fixture).unwrap();
    assert_eq!(&c.value().data.knowledge, &w.knowledge);
    let encoded: Value =
        serde_json::from_slice(&serde_json::to_vec(&c.value().data).unwrap()).unwrap();
    assert!(encoded == expected, "knowledge fixture re-export differs");
}

#[test]
fn concrete_sparse_record_and_buffer_allocation_charges() {
    let mut w = world();
    let key = mint(&mut w, 0, FactSource::authored());
    // Smallest useful records exercise sparse BTree roots and many short values.
    w.knowledge.live.get_mut(&key).unwrap().said.clear();
    let v = wire(&w);
    let charge = aggregate::measure(&v["knowledge"]["live"][0], OWNER)
        .unwrap()
        .expanded_upper_bytes;
    let fact_root = 11 * (std::mem::size_of::<FactKey>() + std::mem::size_of::<Fact>()) + 168;
    println!("sparse_fact_charge={charge}; eleven_slot_fact_root={fact_root}");
    assert!(charge >= fact_root);
    // A holdings map's key+Vec sparse root fits its object, actor and empty
    // rows container charges, before each nonempty Holding contributes more.
    let holding_actor = aggregate::measure(&json!({"actor":"x","rows":[]}), OWNER)
        .unwrap()
        .expanded_upper_bytes;
    let holder_root =
        11 * (std::mem::size_of::<ActorId>() + std::mem::size_of::<Vec<Holding>>()) + 168;
    assert!(holding_actor >= holder_root);
    let occasion = aggregate::measure(
        &json!({"subject":null,"from":null,"at_game_days":0,"offered":false}),
        OWNER,
    )
    .unwrap()
    .expanded_upper_bytes;
    let occasion_root =
        11 * (std::mem::size_of::<ActorId>() + std::mem::size_of::<Occasion>()) + 168;
    // Sparse maps additionally receive the map container + first key charge.
    assert!(occasion + 512 + 64 >= occasion_root);
}

#[test]
fn seeded_history_hearsay_and_stage_dedupe_keep_exact_obligations() {
    let mut w = world();
    active(&mut w);
    let key = FactKey(2);
    let fact = w.knowledge.live.get_mut(&key).unwrap();
    fact.topic = Topic::Law;
    fact.subject = vec![actor("cb947")];
    fact.seeded.insert(actor("old_witness"));
    w.knowledge.hearsay_raised.insert((key, actor("departed")));
    let c = decode(&w, bytes(&w).value()).unwrap();
    let mut restored = c.value().data.knowledge.clone();
    assert_eq!(&restored, &w.knowledge);
    assert!(!Arc::ptr_eq(&restored.holdings, &w.knowledge.holdings));
    assert!(!Arc::ptr_eq(&restored.air, &w.knowledge.air));
    assert!(restored.hearsay_raised.contains(&(key, actor("departed"))));
    assert!(restored.player_stage_telling_seen(FactKey(1), &actor("departed"), u32::MAX));
    assert!(!restored.note_player_stage_telling(FactKey(1), actor("departed"), u32::MAX));
    assert!(restored.note_player_stage_telling(FactKey(1), actor("departed"), 0));
    restored.invalidate(key);
    assert!(restored.hearsay_raised.is_empty());
    let mut v = wire(&w);
    let pair = v["knowledge"]["hearsay_raised"][0].clone();
    v["knowledge"]["hearsay_raised"]
        .as_array_mut()
        .unwrap()
        .push(pair);
    refusal(&w, v, "duplicate");
}
