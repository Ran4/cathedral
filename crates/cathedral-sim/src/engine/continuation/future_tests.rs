use super::future_harness::*;
use super::*;
use crate::round::continuation_fixture as round_fixture;
use crate::status::STATE_THINKING;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
struct Request {
    method: &'static str,
    prompt: String,
    budget: Option<u32>,
    id: crate::RequestId,
}
#[derive(Default)]
struct Calls {
    requests: Vec<Request>,
    next: u64,
    busy: bool,
}
struct Recorded(Rc<RefCell<Calls>>);
impl Recorded {
    fn submit(
        &mut self,
        method: &'static str,
        prompt: String,
        budget: Option<u32>,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        let mut c = self.0.borrow_mut();
        if c.busy {
            return Err(crate::CognitionBusy);
        }
        c.next += 1;
        let id = crate::RequestId(c.next);
        c.requests.push(Request {
            method,
            prompt,
            budget,
            id,
        });
        Ok(id)
    }
}
impl Cognition for Recorded {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        panic!("lost recorded method")
    }
    fn request_with_budget(
        &mut self,
        p: String,
        b: Option<u32>,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        self.submit("scheduler", p, b)
    }
    fn request_night(
        &mut self,
        p: String,
        b: Option<u32>,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        self.submit("night", p, b)
    }
}

fn at(i: u64) -> f64 {
    std::time::Duration::from_millis(i * 50).as_secs_f64()
}
fn installed_nav() -> std::sync::Arc<crate::NavData> {
    std::sync::Arc::new(
        crate::NavData::from_parts(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/world/navigation.json"
            )),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/world/navigation.bin"
            )),
        )
        .unwrap(),
    )
}
fn start_host(e: &mut Engine) -> Host {
    let p = e.world.characters[&e.config.player_id].position_m();
    e.poll(
        0.0,
        vec![EngineCommand::SpatialUpdate {
            spatial_seq: 1,
            updates: vec![crate::SpatialActorUpdate::new(
                e.config.player_id.clone(),
                Vec3::new(
                    f64::from(p.x as f32),
                    f64::from(p.y as f32),
                    f64::from(p.z as f32),
                ),
                Some(0.0),
            )],
        }],
    );
    let mut h = initial_host(e);
    h.after_poll(e, 0.0);
    h
}
fn quiet(config: EngineConfig) -> (Engine, Host) {
    let mut e = fixture(config, Box::new(Unavailable));
    e.scheduler.close();
    let h = start_host(&mut e);
    (e, h)
}
fn step(
    e: &mut Engine,
    h: &mut Host,
    r: &mut Restored,
    now: f64,
    commands: Vec<EngineCommand>,
    compare: bool,
) -> Vec<EngineMessage> {
    let restored_commands = commands
        .clone()
        .into_iter()
        .map(|command| EngineCommand::InGeneration {
            generation: r.engine().config.runtime_generation,
            command: Box::new(command),
        })
        .collect();
    let a = e.poll(now, commands);
    h.after_poll(e, now);
    let b = r.poll(now, restored_commands);
    assert_eq!(a, b, "events at {now}");
    if compare {
        r.assert_eq(e, h);
    }
    a
}

#[test]
fn complete_future_food_transform_and_resident_motion_without_cognition() {
    let n = installed_nav();
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            nav: Some(n.clone()),
            clock: WorldClock::new(60.0, Office::Waning, 2, 0.05),
            ..Default::default()
        },
        Box::new(Unavailable),
    );
    for sheet in crate::generate_ambient(&n, 3, 0, &[], &[]).unwrap().sheets {
        e.world.add_character(crate::Character::from_sheet(sheet));
    }
    round_fixture::production(&mut e.world, &mut e.round, &n, &e.clock);
    e.scheduler.close();
    let mut h = start_host(&mut e);
    assert!(
        e.world
            .active_transform_job(&ActorId::from_raw("e7mil"))
            .is_some()
    );
    assert_eq!(e.round.resident_count(), 3);
    assert!(e.world.characters.values().any(|a| {
        a.state.resident.as_ref().is_some_and(|r| {
            r.phase == crate::round::residents::ResidentPhase::MovingLocally
                && r.destination_spot.is_some()
        }) && a.is_walking()
    }));
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    let residents: Vec<_> = e
        .world
        .characters
        .iter()
        .filter(|(_, a)| a.state.resident.is_some())
        .map(|(id, a)| (id.clone(), a.position_m()))
        .collect();
    assert_eq!(residents.len(), 3);
    for i in 1..=600 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 10 == 0);
        if round_fixture::production_finished(&e.round)
            && residents
                .iter()
                .any(|(id, p)| e.world.characters[id].position_m() != *p)
        {
            break;
        }
    }
    assert!(
        round_fixture::production_finished(&e.round),
        "ordinary production must finish"
    );
    assert!(
        e.world.held_quantity(
            &ActorId::from_raw("e7mil"),
            &crate::ItemMatcher::new("flour")
        ) >= 3
    );
    assert!(
        residents
            .iter()
            .any(|(id, p)| e.world.characters[id].position_m() != *p),
        "at least one generated resident must actually move"
    );
    r.assert_eq(&e, &h);
}

#[test]
fn complete_future_market_queue_commits_purchases_without_cognition() {
    let n = installed_nav();
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            nav: Some(n.clone()),
            clock: WorldClock::new(3600.0, Office::HighWick, 2, 0.05),
            ..Default::default()
        },
        Box::new(Unavailable),
    );
    round_fixture::market(&mut e.world, &mut e.round, &n, &e.clock);
    e.scheduler.close();
    let mut h = start_host(&mut e);
    assert!(round_fixture::market_pending(&e.round));
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    for i in 1..=250 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 10 == 0);
        if round_fixture::market_finished(&e.world) {
            break;
        }
    }
    assert!(round_fixture::market_finished(&e.world));
    assert!(
        e.world.characters[&ActorId::from_raw("prov2")]
            .recent_history()
            .iter()
            .any(|s| s == "You sold a herring for 1 spark. (2 times now)")
    );
    r.assert_eq(&e, &h);
}

#[test]
fn complete_future_road_departure_releases_presence_once() {
    let n = installed_nav();
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            nav: Some(n.clone()),
            clock: WorldClock::new(3600.0, Office::Dayspring, 2, 0.05),
            ..Default::default()
        },
        Box::new(Unavailable),
    );
    round_fixture::road(&mut e.world, &mut e.round, &n, &e.clock);
    e.scheduler.close();
    let mut h = start_host(&mut e);
    assert!(round_fixture::road_pending(&e.round));
    let who = ActorId::from_raw("rbrde");
    let old_epoch = e.world.characters[&who].state.presence_epoch;
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    for i in 1..=4 {
        step(&mut e, &mut h, &mut r, at(i), vec![], true);
    }
    assert!(round_fixture::road_away(&e.round));
    assert!(!e.world.is_present(&who));
    assert_eq!(e.world.characters[&who].state.presence_epoch, old_epoch + 1);
}

#[test]
fn complete_future_weather_transition_and_old_slope_bell_sequence() {
    let (mut e, mut h) = quiet(EngineConfig {
        fake_mode: true,
        clock: WorldClock::new(60.0, Office::Dayspring, 0, 0.05),
        ..Default::default()
    });
    for i in 1..=260 {
        let now = at(i);
        let commands = if i == 260 {
            vec![
                EngineCommand::CycleTimeScale,
                EngineCommand::SetWeatherOverride {
                    kind: WeatherKind::Thunderstorm,
                    intensity: Some(0.8),
                },
            ]
        } else {
            vec![]
        };
        e.poll(now, commands);
        h.after_poll(&e, now);
    }
    assert!(!e.bell_strokes.is_empty());
    let expected_strokes = e.bell_strokes.len();
    let seq = e.bell_seq;
    let initial = e.last_weather_sample;
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    let mut bells = 0;
    for i in 261..=360 {
        let commands = if i == 262 {
            vec![EngineCommand::SetWeatherOverride {
                kind: WeatherKind::Clear,
                intensity: None,
            }]
        } else {
            vec![]
        };
        let events = step(&mut e, &mut h, &mut r, at(i), commands, i % 10 == 0);
        bells += events
            .iter()
            .filter(|m| matches!(m,EngineMessage::Sound{sound_id,..} if sound_id=="town_bell"))
            .count();
    }
    assert!(bells >= expected_strokes);
    assert!(e.bell_seq >= seq + expected_strokes as u64);
    assert_ne!(e.last_weather_sample, initial);
    r.assert_eq(&e, &h);
}

#[test]
fn complete_future_warm_conversation_keeps_its_partner_then_expires() {
    let (mut e, mut h) = quiet(EngineConfig {
        fake_mode: true,
        ..Default::default()
    });
    let actor = ActorId::from_raw("sv3n1");
    let player = e.config.player_id.clone();
    let p = e.world.characters[&player].position_m();
    e.world.characters.get_mut(&actor).unwrap().state.position_m = p + Vec3::new(1.0, 0.0, 0.0);
    let name = e.world.characters[&actor].name().to_string();
    e.poll(
        0.05,
        vec![EngineCommand::PlayerSay {
            request_id: "warm".into(),
            text: format!("{name}, will you wait with me?"),
            position_m: p,
            spatial_seq: 1,
        }],
    );
    h.after_poll(&e, 0.05);
    assert_eq!(e.conversation_partner(0.05), Some(&actor));
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    for i in 2..=610 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 100 == 0);
    }
    assert!(e.conversation_partner(30.5).is_none());
    r.assert_eq(&e, &h);
}

#[test]
fn complete_future_custody_escort_advances_with_cognition_unavailable() {
    let n = installed_nav();
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            nav: Some(n.clone()),
            ..Default::default()
        },
        Box::new(Unavailable),
    );
    e.scheduler.close();
    let officer = ActorId::from_raw("cb947");
    let prisoner = ActorId::from_raw("sv3n1");
    let station =
        crate::custody::nearest_station(&e.world.places, n.node_point(n.forecourt())).unwrap();
    let start = (0..n.node_count())
        .map(|i| n.node_point(i))
        .find(|p| {
            (20.0..=25.0).contains(&p.distance(station.point))
                && n.connected_route(*p, station.point, 0.0).is_some()
        })
        .unwrap();
    e.world
        .characters
        .get_mut(&officer)
        .unwrap()
        .state
        .position_m = start;
    e.world
        .characters
        .get_mut(&prisoner)
        .unwrap()
        .state
        .position_m = start;
    e.world
        .custody
        .seize(prisoner.clone(), officer.clone(), None, station, 0.0);
    e.world.custody.grab(&prisoner, officer.clone());
    let mut h = start_host(&mut e);
    assert_eq!(
        e.world.custody.get(&prisoner).unwrap().state,
        crate::custody::Confinement::InCharge
    );
    let initial = e.world.characters[&prisoner].position_m();
    let officer_initial = e.world.characters[&officer].position_m();
    let original_intent = e.world.characters[&officer].state.intent.clone().unwrap();
    assert!(e.world.characters[&officer].state.intent.is_some());
    assert!(e.world.characters[&officer].is_walking());
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    for i in 1..=40 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 20 == 0);
    }
    assert_eq!(
        e.world.custody.get(&prisoner).unwrap().state,
        crate::custody::Confinement::InCharge
    );
    assert_ne!(initial, e.world.characters[&prisoner].position_m());
    assert_eq!(
        e.world.custody.get(&prisoner).unwrap().holders,
        vec![officer.clone()]
    );
    assert_ne!(officer_initial, e.world.characters[&officer].position_m());
    assert_eq!(
        e.world.characters[&officer]
            .state
            .intent
            .as_ref()
            .unwrap()
            .target,
        original_intent.target
    );
    r.assert_eq(&e, &h);
    for i in 41..=500 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 50 == 0);
    }
    let record = e.world.custody.get(&prisoner).unwrap();
    println!(
        "M2d ordinary escort endpoint: officer_distance={} prisoner_distance={} custody={:?} officer_intent={:?}",
        e.world.characters[&officer]
            .position_m()
            .distance(record.station.point),
        e.world.characters[&prisoner]
            .position_m()
            .distance(record.station.point),
        record.state,
        e.world.characters[&officer].state.intent
    );
    r.assert_eq(&e, &h);
}

#[test]
fn complete_future_knowledge_propagates_with_exact_fact_and_receipt_identity() {
    let (mut e, mut h) = quiet(EngineConfig {
        fake_mode: true,
        knowledge_enabled: true,
        pollen_no_salience: true,
        clock: WorldClock::new(60.0, Office::Dayspring, 2, 0.05),
        ..Default::default()
    });
    let actor = ActorId::from_raw("sv3n1");
    let player = e.config.player_id.clone();
    let p = e.world.characters[&player].position_m();
    e.world.characters.get_mut(&actor).unwrap().state.position_m = p + Vec3::new(1.0, 0.0, 0.0);
    let day = e.clock.game_days(0.0);
    let key = crate::knowledge::mint::mint_claim(
        &mut e.world,
        &actor,
        crate::knowledge::Topic::Talk,
        "Conny has brought a fresh catch".into(),
        vec![ActorId::from_raw("cb947")],
        None,
        Some(day),
    )
    .unwrap();
    e.poll(0.05, vec![]);
    h.after_poll(&e, 0.05);
    assert!(crate::knowledge::holds_key(&e.world, &actor, key).is_some());
    assert!(crate::knowledge::holds_key(&e.world, &player, key).is_none());
    let mut r = Restored::new(&e, &h, Box::new(Unavailable));
    r.assert_eq(&e, &h);
    for i in 2..=400 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 20 == 0);
        if crate::knowledge::holds_key(&e.world, &player, key).is_some() {
            break;
        }
    }
    assert!(
        crate::knowledge::holds_key(&e.world, &player, key).is_some(),
        "future pollen must carry the captured fact"
    );
    r.assert_eq(&e, &h);
}

#[test]
fn complete_future_held_recorded_success_and_error_apply_once() {
    for failure in [false, true] {
        let calls = Rc::new(RefCell::new(Calls::default()));
        let mut e = fixture(
            EngineConfig {
                fake_mode: true,
                turn_delay_seconds: 0.0,
                ..Default::default()
            },
            Box::new(Recorded(calls.clone())),
        );
        let actor = ActorId::from_raw("sv3n1");
        e.scheduler = NpcScheduler::new(vec![actor.clone()], 0.0, 60.0, 0.0);
        e.scheduler.start(0.0);
        e.world
            .characters
            .get_mut(&actor)
            .unwrap()
            .state
            .inbox
            .push("before captured prompt".into());
        let mut h = start_host(&mut e);
        assert_eq!(calls.borrow().requests.len(), 1);
        e.floor.acquire(
            0.0,
            &SpeechEventId("reading-fixture".into()),
            "already committed reading",
            false,
        );
        let completion = Completion {
            request_id: calls.borrow().requests[0].id,
            result: if failure {
                Err(crate::CognitionError::detailed(
                    "RecordedFailure",
                    "original exact detail",
                ))
            } else {
                Ok("remember {\"memory\":\"M2d held memory\"}".into())
            },
            duration_seconds: 0.125,
        };
        e.poll(0.05, vec![EngineCommand::LlmCompletion(completion)]);
        h.after_poll(&e, 0.05);
        assert!(e.scheduler.has_held_result());
        e.scheduler.close();
        e.world
            .characters
            .get_mut(&actor)
            .unwrap()
            .state
            .inbox
            .push("newer unread arrival".into());
        let fresh = Rc::new(RefCell::new(Calls::default()));
        let mut r = Restored::new(&e, &h, Box::new(Recorded(fresh.clone())));
        r.assert_eq(&e, &h);
        for i in 2..=90 {
            step(&mut e, &mut h, &mut r, at(i), vec![], i % 10 == 0);
        }
        assert!(!e.scheduler.has_held_result());
        assert!(fresh.borrow().requests.is_empty());
        assert_eq!(
            e.world.characters[&actor]
                .state
                .memories
                .iter()
                .filter(|s| s.as_str() == "M2d held memory")
                .count(),
            usize::from(!failure)
        );
        assert!(
            e.world.characters[&actor]
                .state
                .inbox
                .iter()
                .any(|s| s == "newer unread arrival")
        );
        r.assert_eq(&e, &h);
    }
}

#[test]
fn complete_future_unfinished_scheduler_retries_original_input_and_settles_exactly() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            turn_delay_seconds: 0.0,
            knowledge_enabled: true,
            ..Default::default()
        },
        Box::new(Recorded(calls.clone())),
    );
    let actor = ActorId::from_raw("sv3n1");
    e.scheduler = NpcScheduler::new(vec![actor.clone()], 0.0, 60.0, 0.0);
    e.scheduler.start(0.0);
    e.world
        .characters
        .get_mut(&actor)
        .unwrap()
        .state
        .inbox
        .push("only in original drained input".into());
    let mut h = start_host(&mut e);
    let original = calls.borrow().requests[0].clone();
    assert_eq!(calls.borrow().requests.len(), 1);
    assert!(e.world.characters[&actor].state.inbox.is_empty());
    e.world
        .characters
        .get_mut(&actor)
        .unwrap()
        .state
        .inbox
        .push("newer unread event".into());
    let fresh = Rc::new(RefCell::new(Calls {
        next: 100,
        ..Default::default()
    }));
    let mut r = Restored::new(&e, &h, Box::new(Recorded(fresh.clone())));
    assert_eq!(r.engine().scheduler.load_retry_count(), 1);
    let captured = r.wire();
    let flight = &captured["scheduler"]["continuation"]["load_retries"][0]["flight"];
    let source = wire(&e, &h, &r.budget);
    let source_flight = source.pointer("/scheduler/scheduler/in_flight").unwrap();
    assert!(source_flight["semantic"].is_object());
    assert!(!original.prompt.is_empty());
    // V2 adds the resolved output budget. Every original V1 flight field must
    // remain verbatim, independently of preparing either comparison timeline.
    for (key, value) in source_flight.as_object().unwrap() {
        assert_eq!(flight[key], *value, "original scheduler flight field {key}");
    }
    assert_eq!(flight["prompt"], serde_json::json!(original.prompt));
    assert_eq!(flight["actor_id"], serde_json::json!(actor.as_str()));
    assert_eq!(
        flight["drained_events"],
        serde_json::json!(["only in original drained input"])
    );
    assert_eq!(
        flight["presence_epoch"],
        serde_json::json!(e.world.characters[&actor].state.presence_epoch)
    );
    assert!(
        captured["ledger"]["protected"]
            .as_array()
            .unwrap()
            .contains(&flight["semantic"])
    );
    assert!(fresh.borrow().requests.is_empty());
    // Compare every owner in the same prepared semantic representation. The
    // actual control remains uninterrupted. Only the one external request ID
    // differs; no owner, durable root, context or allocation counter is omitted.
    let equivalent = |e: &Engine, h: &Host, r: &Restored| {
        let control_observation = Restored::new(e, h, Box::new(Unavailable));
        let mut a = control_observation.wire();
        let restored_observation = Restored::new(r.engine(), &r.host, Box::new(Unavailable));
        let mut b = restored_observation.wire();
        for v in [&mut a, &mut b] {
            assert_eq!(
                v["scheduler"]["continuation"]["load_retries"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            *v.pointer_mut("/scheduler/continuation/load_retries/0/flight/request_id")
                .unwrap() = serde_json::json!(1);
        }
        assert_wire_eq(a, b);
        // Both actual observation Engines retain all leases through comparison.
        drop(restored_observation);
        drop(control_observation);
    };
    equivalent(&e, &h, &r);
    let a = e.poll(0.05, vec![]);
    h.after_poll(&e, 0.05);
    let b = r.poll(0.05, vec![]);
    let statuses: Vec<_> = b
        .iter()
        .filter(|m| matches!(m, EngineMessage::Status(_)))
        .collect();
    assert_eq!(
        statuses,
        vec![&EngineMessage::Status(StatusEvent::llm(
            STATE_THINKING,
            Some(actor.clone()),
            None
        ))]
    );
    assert_eq!(
        a,
        b.into_iter()
            .filter(|m| !matches!(m, EngineMessage::Status(_)))
            .collect::<Vec<_>>()
    );
    let retry = fresh.borrow().requests[0].clone();
    assert_eq!(fresh.borrow().requests.len(), 1);
    assert_eq!(
        (&retry.method, &retry.prompt, retry.budget),
        (&original.method, &original.prompt, original.budget)
    );
    assert_ne!(retry.id, original.id);
    assert_eq!(
        r.engine().world.characters[&actor].state.inbox,
        vec!["newer unread event"]
    );
    equivalent(&e, &h, &r);
    calls.borrow_mut().busy = true;
    fresh.borrow_mut().busy = true;
    let completion = |id| {
        EngineCommand::LlmCompletion(Completion {
            request_id: id,
            result: Ok("remember {\"memory\":\"M2d exact retry memory\"}".into()),
            duration_seconds: 0.2,
        })
    };
    let a = e.poll(0.1, vec![completion(original.id)]);
    h.after_poll(&e, 0.1);
    let b = r.poll(
        0.1,
        vec![EngineCommand::InGeneration {
            generation: r.engine().config.runtime_generation,
            command: Box::new(completion(retry.id)),
        }],
    );
    assert_eq!(a, b);
    r.assert_eq(&e, &h);
    for i in 3..=5 {
        step(&mut e, &mut h, &mut r, at(i), vec![], true);
    }
    assert_eq!(
        e.world.characters[&actor]
            .state
            .memories
            .iter()
            .filter(|s| s.as_str() == "M2d exact retry memory")
            .count(),
        1
    );
    assert_eq!(fresh.borrow().requests.len(), 1);
}

#[test]
fn complete_future_queued_night_crossing_reflects_once_after_unavailable_service() {
    let calls = Rc::new(RefCell::new(Calls {
        busy: true,
        ..Default::default()
    }));
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            clock: WorldClock::new(60.0, Office::Lamplight, 2, 0.05),
            night_office: NightOfficeConfig {
                enabled: true,
                majors: true,
                wards: false,
                ambients: true,
            },
            ..Default::default()
        },
        Box::new(Recorded(calls.clone())),
    );
    e.scheduler.close();
    let mut h = start_host(&mut e);
    for i in 1..=150 {
        e.poll(at(i), vec![]);
        h.after_poll(&e, at(i));
    }
    assert!(e.night.owed() > 0);
    assert!(e.night.in_flight_subject().is_none());
    assert_eq!(e.night.totals(), (0, 0));
    let fresh = Rc::new(RefCell::new(Calls {
        busy: true,
        ..Default::default()
    }));
    let mut r = Restored::new(&e, &h, Box::new(Recorded(fresh.clone())));
    r.assert_eq(&e, &h);
    for i in 151..=249 {
        step(&mut e, &mut h, &mut r, at(i), vec![], i % 20 == 0);
    }
    calls.borrow_mut().busy = false;
    fresh.borrow_mut().busy = false;
    step(&mut e, &mut h, &mut r, 12.5, vec![], true);
    assert_eq!(calls.borrow().requests.len(), 1);
    assert_eq!(calls.borrow().requests, fresh.borrow().requests);
    assert_eq!(calls.borrow().requests[0].method, "night");
    let actor = ActorId::from_raw(e.night.in_flight_subject().unwrap());
    calls.borrow_mut().busy = true;
    fresh.borrow_mut().busy = true;
    let done = EngineCommand::LlmCompletion(Completion {
        request_id: calls.borrow().requests[0].id,
        result: Ok("remember {\"memory\":\"M2d nightly memory\"}".into()),
        duration_seconds: 0.125,
    });
    step(&mut e, &mut h, &mut r, 12.55, vec![done], true);
    for i in 252..=260 {
        step(&mut e, &mut h, &mut r, at(i), vec![], true);
    }
    assert_eq!(e.night.totals(), (1, 0));
    assert_eq!(
        e.world.characters[&actor]
            .state
            .memories
            .iter()
            .filter(|s| s.as_str() == "M2d nightly memory")
            .count(),
        1
    );
}

#[test]
fn complete_future_unfinished_night_preserves_input_and_declares_retry_pacing() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            clock: WorldClock::new(60.0, Office::Lamplight, 2, 0.05),
            night_office: NightOfficeConfig {
                enabled: true,
                majors: true,
                wards: false,
                ambients: false,
            },
            ..Default::default()
        },
        Box::new(Recorded(calls.clone())),
    );
    let actor = ActorId::from_raw("sv3n1");
    let absent: Vec<_> = e
        .world
        .characters
        .keys()
        .filter(|id| **id != actor && **id != e.config.player_id)
        .cloned()
        .collect();
    e.world
        .transition_presence(&absent, crate::Presence::BeyondTheWalls, &BTreeMap::new())
        .unwrap();
    e.scheduler.close();
    let mut h = start_host(&mut e);
    for i in 1..=150 {
        e.poll(at(i), vec![]);
        h.after_poll(&e, at(i));
    }
    assert_eq!(e.night.owed(), 0);
    assert_eq!(e.night.in_flight_subject(), Some(actor.to_string()));
    assert_eq!(calls.borrow().requests.len(), 1);
    let original = calls.borrow().requests[0].clone();
    let fresh = Rc::new(RefCell::new(Calls {
        next: 100,
        ..Default::default()
    }));
    let mut r = Restored::new(&e, &h, Box::new(Recorded(fresh.clone())));
    assert!(r.engine().night.load_retry_pending());
    assert!(fresh.borrow().requests.is_empty());
    let source = wire(&e, &h, &r.budget);
    let captured = r.wire();
    let source_flight = source.pointer("/night/night/night/in_flight").unwrap();
    assert!(source_flight["semantic"].is_object());
    assert_eq!(source_flight["owed_day"], serde_json::json!(2));
    assert_eq!(
        source_flight["subject"],
        serde_json::json!({ "person": actor.as_str() })
    );
    assert_eq!(
        source_flight["presence_epoch"],
        serde_json::json!(e.world.characters[&actor].state.presence_epoch)
    );
    assert!(!original.prompt.is_empty());
    assert_eq!(source_flight["prompt"], serde_json::json!(original.prompt));
    assert_eq!(
        serde_json::to_vec(source_flight).unwrap(),
        serde_json::to_vec(
            captured
                .pointer("/night/base/night/night/in_flight")
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&source["cognition_inputs"]["night"]).unwrap(),
        serde_json::to_vec(&captured["cognition_inputs"]["night"]).unwrap()
    );
    let observed = Restored::new(&e, &h, Box::new(Unavailable));
    assert_wire_eq(observed.wire(), r.wire());
    drop(observed);
    for i in 151..=153 {
        let now = at(i);
        let a = e.poll(now, vec![]);
        h.after_poll(&e, now);
        let b = r.poll(now, vec![]);
        assert_eq!(
            a, b,
            "Night replacement submission has no committed world event"
        );
    }
    assert_eq!(fresh.borrow().requests.len(), 1);
    let retry = fresh.borrow().requests[0].clone();
    assert_eq!(
        (&original.method, &original.prompt, original.budget),
        (&retry.method, &retry.prompt, retry.budget)
    );
    assert_eq!(retry.method, "night");
    assert_ne!(original.id, retry.id);
    // A retry is another actual Night submission: the production policy starts
    // its pacing at retry time. This is explicitly conserved work, not exact
    // future-state equality with an execution that never needed replacement.
    let original_attempt = 7.5 + 60.0 / 400.0;
    let retry_attempt = 7.65 + 60.0 / 400.0;
    let compare_pacing = |e: &Engine, h: &Host, r: &Restored, active: bool| {
        let mut a = wire(e, h, &r.budget);
        let mut b = r.wire();
        let p = "/night/night/night/next_attempt_at/at";
        assert_eq!(a.pointer(p).unwrap().as_f64().unwrap(), original_attempt);
        assert_eq!(b.pointer(p).unwrap().as_f64().unwrap(), retry_attempt);
        *a.pointer_mut(p).unwrap() = serde_json::json!(retry_attempt);
        if active {
            // Both active requests refer to the same conserved duty/input. The
            // two exact external request identity paths are the only further
            // differences permitted before the recorded result settles.
            for p in [
                "/night/night/night/in_flight/request_id",
                "/cognition_inputs/night/request_id",
            ] {
                assert_eq!(a.pointer(p).unwrap(), &serde_json::json!(original.id.0));
                assert_eq!(b.pointer(p).unwrap(), &serde_json::json!(retry.id.0));
                *a.pointer_mut(p).unwrap() = serde_json::json!(1);
                *b.pointer_mut(p).unwrap() = serde_json::json!(1);
            }
        }
        assert_wire_eq(a, b);
    };
    compare_pacing(&e, &h, &r, true);
    let completion = |id| {
        EngineCommand::LlmCompletion(Completion {
            request_id: id,
            result: Ok("remember {\"memory\":\"M2d retried night\"}".into()),
            duration_seconds: 0.125,
        })
    };
    let a = e.poll(7.7, vec![completion(original.id)]);
    h.after_poll(&e, 7.7);
    let b = r.poll(
        7.7,
        vec![EngineCommand::InGeneration {
            generation: r.engine().config.runtime_generation,
            command: Box::new(completion(retry.id)),
        }],
    );
    assert_eq!(a, b);
    compare_pacing(&e, &h, &r, false);
    for i in 155..=160 {
        let now = at(i);
        let a = e.poll(now, vec![]);
        h.after_poll(&e, now);
        assert_eq!(a, r.poll(now, vec![]));
        compare_pacing(&e, &h, &r, false);
    }
    assert_eq!(e.night.totals(), (1, 0));
    assert_eq!(r.engine().night.totals(), (1, 0));
    assert_eq!(
        e.world.characters[&actor]
            .state
            .memories
            .iter()
            .filter(|s| s.as_str() == "M2d retried night")
            .count(),
        1
    );
    assert_eq!(fresh.borrow().requests.len(), 1);
}

#[test]
fn complete_future_initial_unavailable_city_is_equal() {
    let mut e = fixture(
        EngineConfig {
            fake_mode: true,
            ..Default::default()
        },
        Box::new(Unavailable),
    );
    e.scheduler.close();
    let mut h = start_host(&mut e);
    let mut loaded = Restored::new(&e, &h, Box::new(Unavailable));
    loaded.assert_eq(&e, &h);
    for i in 1..=4 {
        let now = at(i);
        let a = e.poll(now, vec![]);
        h.after_poll(&e, now);
        let b = loaded.poll(now, vec![]);
        assert_eq!(a, b);
        loaded.assert_eq(&e, &h);
    }
}
