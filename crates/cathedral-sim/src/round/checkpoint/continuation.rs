//! Supported installed-content witnesses, reusing the ordinary Round test helpers.
use super::*;
use crate::{
    checkpoint::{Admitted, CheckpointBudget, Cohort},
    round::checkpoint::{RoundCandidate, RoundCheckpointContext, RoundDtoV1},
};
fn saved(
    round: &Round,
    world: &World,
    nav: Option<&NavData>,
) -> (Admitted<Vec<u8>>, Admitted<RoundCandidate>) {
    let budget = CheckpointBudget::default();
    let context = RoundCheckpointContext::from_world(world, nav);
    let dto = round
        .export_checkpoint(context, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap();
    let bytes = dto.encode().unwrap();
    let decoded = RoundDtoV1::decode(
        bytes.value(),
        budget
            .reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        context,
    )
    .unwrap();
    let candidate = decoded.into_candidate(context).unwrap();
    assert_eq!(candidate.value().round(), round);
    (bytes, candidate)
}
fn supported() -> (World, Round, NavData, WorldClock) {
    let navigation = nav();
    let mut world = road_party_world();
    for (id, occupation) in [
        ("e7mil", "miller"),
        ("baker", "baker"),
        ("draw1", "water_carrier"),
        ("draw2", "water_carrier"),
    ] {
        world.add_character(person(
            id,
            navigation.node_point(navigation.forecourt()),
            Some(occupation),
            Significance::Minor,
        ));
    }
    let generated = crate::generate_ambient(&navigation, 2, 0, &[], &[]).unwrap();
    for sheet in generated.sheets {
        world.add_character(Character::from_sheet(sheet));
    }
    let clock = clock_on(Office::Waning, 2);
    let mut round = Round::new();
    round.seed(&mut world, &navigation, 0.0, &clock);
    assert_eq!(round.resident_count(), 2);
    assert!(round.stalls.iter().any(|s| {
        s.preferred
            .as_ref()
            .is_some_and(|id| !world.characters.contains_key(id))
    }));
    (world, round, navigation, clock)
}
fn active() -> (World, Round, NavData, WorldClock) {
    let (mut w, mut r, n, c) = supported();
    let producer = ActorId::from_raw("e7mil");
    let spec = r
        .production_plans
        .iter()
        .find(|p| p.producer == producer)
        .unwrap()
        .transforms[0]
        .clone();
    w.characters.get_mut(&producer).unwrap().state.position_m = spec.point;
    w.characters.get_mut(&producer).unwrap().state.movement = None;
    w.add_stock(&producer, &stock("grain", 1), "checkpoint_grain")
        .unwrap();
    r.tick_production(&mut w, &c, 0.0, &BTreeSet::new());
    assert!(w.active_transform_job(&producer).is_some());
    let a = ActorId::from_raw("draw1");
    let b = ActorId::from_raw("draw2");
    r.sources[0].keeper = Some(producer.clone());
    for id in [&a, &b] {
        let p = r.people.get_mut(id).unwrap();
        p.source = Some(0);
        p.phase = Phase::Queued;
        p.is_household = id == &b;
        w.characters.get_mut(id).unwrap().state.position_m = r.sources[0].draw_point;
    }
    enqueue(&mut r, 0, a.clone());
    service_sources(&mut r, &mut w, &n, &c, 0.0, &player(), &BTreeSet::new());
    enqueue(&mut r, 0, b.clone());
    assert_eq!(r.sources[0].queue, [b, a]); // A later household entry precedes live service.
    r.last_household_watch_day = Some(1);
    r.last_household_settlement_day = Some(0);
    (w, r, n, c)
}
#[test]
fn checkpoint_services_production_and_household_guards_continue_exactly() {
    let (mut w, mut r, n, c) = active();
    let producer = ActorId::from_raw("e7mil");
    r.tick_production(&mut w, &c, 10.0, &warm(&producer));
    let (_bytes, candidate) = saved(&r, &w, Some(&n));
    let mut restored = candidate.value().round().clone();
    let mut other = w.clone();
    for now in [10.5, 20.0, 40.0, 160.0] {
        for (round, world) in [(&mut r, &mut w), (&mut restored, &mut other)] {
            service_sources(round, world, &n, &c, now, &player(), &BTreeSet::new());
            round.tick_production(world, &c, now, &BTreeSet::new());
        }
        assert_eq!(restored, r);
        assert_eq!(other, w);
    }
    assert!(w.active_transform_job(&producer).is_none());
    assert!(
        r.food_log
            .iter()
            .any(|s| s.starts_with("transform_finish:"))
    );
    for (round, world) in [(&mut r, &mut w), (&mut restored, &mut other)] {
        round.dispatch_household_settlement(world, 3);
        let once = world.clone();
        round.dispatch_household_settlement(world, 3);
        assert_eq!(*world, once);
    }
    assert_eq!(restored, r);
    assert_eq!(other, w);
    assert_eq!(r.last_household_watch_day, Some(3));
    assert_eq!(r.last_household_settlement_day, Some(3));
}
#[test]
fn checkpoint_stranded_and_manual_transform_obligations_survive() {
    let (mut w, mut r, n, c) = supported();
    let producer = ActorId::from_raw("e7mil");
    let input = w
        .add_stock(&producer, &stock("grain", 1), "manual_input")
        .unwrap();
    w.start_transform_job(TransformJob {
        job_id: "manual valid id".into(),
        spec_id: "mill_grain".into(),
        producer: producer.clone(),
        production_day: -5,
        start_slot: 99,
        inputs: vec![ReservedInput {
            item_id: input,
            quantity: 1,
        }],
        outputs: vec![stock("flour", 1)],
        progress_work_minutes: 0.0,
    })
    .unwrap();
    saved(&r, &w, Some(&n)); // Matching spec is a work lookup, not a generated-origin tag.
    r.production_plans.clear();
    r.sweep_stranded_transforms(&mut w, c.game_days(0.0));
    let (_bytes, candidate) = saved(&r, &w, Some(&n));
    let mut restored = candidate.value().round().clone();
    let mut other = w.clone();
    for elapsed in [0.5, 1.01] {
        r.sweep_stranded_transforms(&mut w, c.game_days(0.0) + elapsed);
        restored.sweep_stranded_transforms(&mut other, c.game_days(0.0) + elapsed);
        assert_eq!(r, restored);
        assert_eq!(w, other);
    }
    assert!(w.active_transform_job(&producer).is_none());
    assert!(
        r.food_log
            .iter()
            .any(|s| s.starts_with("transform_abandoned:"))
    );
}
#[test]
fn checkpoint_road_departure_notifications_and_next_trip_resume() {
    let navigation = nav();
    let mut w = road_party_world();
    let mut r = Round::new();
    let c = clock_on(Office::Dayspring, 2);
    r.seed(&mut w, &navigation, 0.0, &c);
    let id = PartyId::from_raw("brede_wool_gate");
    r.begin_road_return(&mut w, &id, 2, 0.0, &mut Vec::new());
    let party = r.road_parties[&id].clone();
    for member in &party.members {
        w.characters.get_mut(member).unwrap().state.position_m = party.gate_point;
    }
    r.tick_road_parties(&mut w, &navigation, c.at(0.0), 1.0, &BTreeSet::new());
    assert_eq!(
        r.road_parties[&id].state.phase,
        PartyPhase::DeparturePending
    );
    let (_bytes, candidate) = saved(&r, &w, Some(&navigation));
    let mut restored = candidate.value().round().clone();
    let mut other = w.clone();
    for (round, world) in [(&mut r, &mut w), (&mut restored, &mut other)] {
        round.tick_road_parties(world, &navigation, c.at(0.0), 2.0, &BTreeSet::new());
        assert!(!round.departed_this_tick.is_empty());
        round.trigger_road_stage(world, &id, 3);
    }
    assert_eq!(r, restored);
    assert_eq!(w, other);
    assert_eq!(r.road_parties[&id].state.trip_number, 2);
    assert!(
        party
            .members
            .iter()
            .all(|id| w.characters[id].state.presence_epoch > 0)
    );
    saved(&r, &w, Some(&navigation)); // Exact pending departure notifications retained.
}
#[test]
fn checkpoint_signed_pollen_order_rearm_and_stale_entries_are_bounded() {
    let (mut w, mut r, n, _) = supported();
    let id = ActorId::from_raw("draw1");
    let row = serde_json::json!({"schema_version":1,"facts":[{"id":"checkpoint.word","topic":"coin","said":"{subject} has a word","subject":[id.as_str()],"seeded":[id.as_str()],"day":-3}]});
    crate::knowledge::FactCatalog::from_json(&row.to_string())
        .unwrap()
        .seed_one(&mut w, &crate::FactId::from_raw("checkpoint.word"))
        .unwrap();
    w.knowledge_enabled = true;
    let clock = clock_on(Office::Dayspring, -3);
    tick_pollen(&mut r, &mut w, &clock, 0.0);
    let first = r.next_pollen[&id];
    assert!(first > clock.game_days(0.0) && first < 0.0);
    tick_pollen(&mut r, &mut w, &clock, 0.01);
    assert_eq!(first.to_bits(), r.next_pollen[&id].to_bits());
    r.arm_pollen(&id);
    tick_pollen(&mut r, &mut w, &clock, 1.0);
    assert!(r.pollen_due.contains(&(pollen_due_key(first), id.clone())));
    let (_bytes, candidate) = saved(&r, &w, Some(&n));
    let mut restored = candidate.value().round().clone();
    let mut other = w.clone();
    for now in [2.0, 50.0, 100.0] {
        tick_pollen(&mut r, &mut w, &clock, now);
        tick_pollen(&mut restored, &mut other, &clock, now);
        assert_eq!(r, restored);
        assert_eq!(w, other);
    }
    let ordered = [f64::NEG_INFINITY, -5.0, -0.1, -0.0, 0.1, 5.0].map(pollen_due_key);
    assert!(ordered.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(pollen_due_key(-0.0), pollen_due_key(0.0));
    let positive = clock_on(Office::Dayspring, 2);
    r.arm_pollen(&id);
    tick_pollen(&mut r, &mut w, &positive, 0.0);
    assert_eq!(
        r.next_pollen[&id],
        positive.game_days(0.0) + poll_gap_game_days(&id, r.people[&id].epoch)
    );
}
#[test]
fn checkpoint_round_corruption_rejects_static_and_dynamic_disagreement() {
    let (w, r, n, _) = active();
    let (bytes, _candidate) = saved(&r, &w, Some(&n));
    let original: serde_json::Value = serde_json::from_slice(bytes.value()).unwrap();
    let context = RoundCheckpointContext::from_world(&w, Some(&n));
    let bad = |v: serde_json::Value, reason: &str| {
        let b = serde_json::to_vec(&v).unwrap();
        let budget = CheckpointBudget::default();
        let e = RoundDtoV1::decode(
            &b,
            budget
                .reserve(Cohort::LoadCandidate, b.len() + 4096)
                .unwrap(),
            context,
        )
        .unwrap_err();
        assert!(e.reason.contains(reason), "{e}");
        assert_eq!(budget.retained_bytes(), 0);
    };
    let mut v = original.clone();
    v["round"]["production_plans"][0]["transforms"][0]["work_minutes"] = 1.into();
    bad(v, "static transform");
    let mut v = original.clone();
    v["round"]["sources"][0]["draw_point"]["x"] = 999.0.into();
    bad(v, "static water");
    let mut v = original.clone();
    v["round"]["sources"][0]["queue"]
        .as_array_mut()
        .unwrap()
        .push("draw1".into());
    bad(v, "duplicate queue");
    let mut v = original.clone();
    v["round"]["people"]["draw1"]
        .as_object_mut()
        .unwrap()
        .remove("evening_seed");
    bad(v, "missing field");
    let mut v = original.clone();
    v["round"]["production_plans"][0]["transforms"][0]["consumes"][0] =
        serde_json::json!({"kind":"stew","metadata":{},"quantity":2});
    bad(v, "stock quantity/catalog");
    let mut v = original.clone();
    let row = v["round"]["production_starts"][0].clone();
    v["round"]["production_starts"]
        .as_array_mut()
        .unwrap()
        .push(row);
    bad(v, "duplicate production start");
    let mut v = original.clone();
    v["round"]["institutional_payroll_sparks"] = u64::MAX.into();
    bad(v, "headroom");
    let resident = r.residents.people.keys().next().unwrap().as_str();
    let mut v = original.clone();
    v["round"]["residents"]["people"][resident]["preferred_spot"] = 999.into();
    bad(v, "preferred spot");
    let mut v = original.clone();
    v["round"]["residents"]["people"][resident]["target"] = 0.into();
    bad(v, "target/destination");
    let mut v = original.clone();
    v["round"]["residents"]["people"][resident]["projection"]["sheltered"] = true.into();
    bad(v, "shelter projection");
    let mut v = original;
    v["round"]["residents"]["cursor"] = 999.into();
    bad(v, "cursor");
    let mut wrong = w.clone();
    wrong
        .characters
        .get_mut(&ActorId::from_raw("draw1"))
        .unwrap()
        .state
        .you_sell = vec![VendorListing {
        name: "invented".into(),
        price_sparks: 1,
    }];
    let b = CheckpointBudget::default();
    let e = RoundDtoV1::decode(
        bytes.value(),
        b.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        RoundCheckpointContext::from_world(&wrong, Some(&n)),
    )
    .unwrap_err();
    assert!(e.reason.contains("orphan vendor"));
    wrong
        .characters
        .get_mut(&ActorId::from_raw("draw1"))
        .unwrap()
        .state
        .you_sell
        .clear();
    wrong
        .characters
        .get_mut(&ActorId::from_raw("draw1"))
        .unwrap()
        .state
        .resident = w.characters.values().find_map(|c| c.state.resident.clone());
    let e = RoundDtoV1::decode(
        bytes.value(),
        b.reserve(Cohort::LoadCandidate, bytes.value().len() + 4096)
            .unwrap(),
        RoundCheckpointContext::from_world(&wrong, Some(&n)),
    )
    .unwrap_err();
    assert!(e.reason.contains("orphan resident"));
}
#[test]
#[ignore = "explicit supported fixture writer"]
fn write_checkpoint_round_fixtures() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checkpoint_v1");
    let (w, r, n, _) = active();
    let (bytes, _candidate) = saved(&r, &w, Some(&n));
    std::fs::write(root.join("round_active.json"), bytes.value()).unwrap();
    let w = World::new();
    let (bytes, _candidate) = saved(&Round::new(), &w, None);
    std::fs::write(root.join("round_empty.json"), bytes.value()).unwrap();
}
#[test]
fn checkpoint_supported_round_fixture_bytes_are_exact() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checkpoint_v1");
    let (w, r, n, _) = active();
    let (bytes, _candidate) = saved(&r, &w, Some(&n));
    assert_eq!(
        *bytes.value(),
        std::fs::read(root.join("round_active.json")).unwrap()
    );
    let w = World::new();
    let (bytes, _candidate) = saved(&Round::new(), &w, None);
    assert_eq!(
        *bytes.value(),
        std::fs::read(root.join("round_empty.json")).unwrap()
    );
}

#[test]
fn checkpoint_market_session_hold_rebind_and_purchase_continue() {
    let navigation = nav();
    let mut world = base_world();
    for (id, occupation) in [("p008s", "provisioner"), ("e7mil", "miller")] {
        world.add_character(person(
            id,
            navigation.node_point(navigation.forecourt()),
            Some(occupation),
            Significance::Minor,
        ));
    }
    let clock = clock_on(Office::HighWick, 2);
    let mut round = Round::new();
    round.seed(&mut world, &navigation, 0.0, &clock);
    let buyer = ActorId::from_raw("e7mil");
    let seller = ActorId::from_raw("p008s");
    let counter = round.counters["betriss_grain_seven_lofts"].clone();
    world
        .add_stock(&seller, &stock("grain", 3), "checkpoint_market_stock")
        .unwrap();
    world
        .credit_sparks(&buyer, 20, "checkpoint_market_purse")
        .unwrap();
    for id in [&buyer, &seller] {
        world.characters.get_mut(id).unwrap().state.needs = Default::default();
    }
    world.characters.get_mut(&seller).unwrap().state.position_m = counter.pitch;
    let away = (0..navigation.node_count())
        .map(|i| navigation.node_point(i))
        .find(|p| {
            (20.0..=35.0).contains(&p.distance(counter.pitch))
                && navigation.connected_route(*p, counter.pitch, 0.0).is_some()
        })
        .unwrap();
    world.characters.get_mut(&buyer).unwrap().state.position_m = away;
    round.tick_stock_plans(
        &mut world,
        &navigation,
        clock.at(0.0),
        0.0,
        &BTreeSet::new(),
    );
    let original = round.market_errands[&buyer].clone();
    assert!(original.travel_deadline_real.is_some());
    round.tick_stock_plans(&mut world, &navigation, clock.at(5.0), 5.0, &warm(&buyer));
    assert_eq!(
        round.market_errands[&buyer].deadline_hold_began_real,
        Some(5.0)
    );
    let (_bytes, candidate) = saved(&round, &world, Some(&navigation));
    let mut restored = candidate.value().round().clone();
    let mut other = world.clone();
    for (r, w) in [(&mut round, &mut world), (&mut restored, &mut other)] {
        r.tick_stock_plans(w, &navigation, clock.at(15.0), 15.0, &BTreeSet::new());
        assert_eq!(
            r.market_errands[&buyer].travel_deadline_real,
            original.travel_deadline_real.map(|t| t + 10.0)
        );
        // A seller stepping off the pitch suspends this exact session and budget.
        w.characters.get_mut(&seller).unwrap().state.position_m = away;
        r.tick_stock_plans(w, &navigation, clock.at(16.0), 16.0, &BTreeSet::new());
        assert_eq!(
            r.market_errands[&buyer].phase,
            MarketErrandPhase::WaitingForOpen
        );
    }
    assert_eq!(round, restored);
    assert_eq!(world, other);
    saved(&round, &world, Some(&navigation));
    for (r, w) in [(&mut round, &mut world), (&mut restored, &mut other)] {
        w.characters.get_mut(&seller).unwrap().state.position_m = counter.pitch;
        w.characters.get_mut(&buyer).unwrap().state.position_m = counter.pitch;
        w.characters.get_mut(&buyer).unwrap().state.movement = None;
        for now in [20.0, 21.0] {
            r.tick_stock_plans(w, &navigation, clock.at(now), now, &BTreeSet::new());
        }
        assert!(r.closed_market_visits.contains_key("bertran_grain"));
        assert_eq!(w.held_quantity(&buyer, &ItemMatcher::new("grain")), 3);
        assert_eq!(
            r.closed_market_visits["bertran_grain"].bindings_seen,
            original.bindings_seen
        );
    }
    assert_eq!(round, restored);
    assert_eq!(world, other);
    saved(&round, &world, Some(&navigation));
}
