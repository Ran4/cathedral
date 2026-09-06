//! M5 consequences through the real sheet, command and law seams.
use super::*;
use cathedral_sim::knowledge::{FactView, Topic};
use cathedral_sim::{CivicRope, DomainEvent, EngineCommand, EngineMessage, FactKey};

fn world_with_officer() -> World {
    let mut world = seed_world();
    world.current_time = Some(WorldTime {
        day: 0,
        fraction: 0.0,
        office: Office::Dayspring,
        weekday: Weekday::Bellday,
    });
    let mut law = profile(Some("Civic officer"), PlanningWard::Fabric);
    law.occupation_id = Some("civic_officer".into());
    world.add_character(character(
        "officr",
        "Tib Officer",
        Some(law),
        &["sv3n1", "cb947", "player"],
    ));
    for id in ["player", "sv3n1", "cb947", "k0fb1", "officr"] {
        stand(&mut world, id, IN_FABRIC);
    }
    world
}

fn fact(world: &mut World, id: &str, topic: &str, subject: &str, seeded: &[&str]) -> FactKey {
    let row = json!({"id":id,"topic":topic,"said":"{subject} was seen at {place} {day}",
        "subject":[subject],"seeded":if seeded.is_empty() { vec![subject] } else { seeded.to_vec() },"place":"wickmarket","day":0,"garble":"subject,place,day"});
    assert!(seed_pack(world, &row.to_string()).is_empty());
    world.knowledge.key_of(&FactId::from_raw(id)).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn give(
    world: &mut World,
    to: &str,
    key: FactKey,
    hops: u8,
    heat: f32,
    subject: Option<&str>,
    from: &str,
    day: f64,
) {
    knowledge::learn(
        world,
        &actor(to),
        key,
        Telling {
            hops,
            heat,
            from: Some(actor(from)),
            view: FactView {
                subject: subject.map(actor),
                ..Default::default()
            },
        },
        Some(day),
    );
}

#[test]
fn a_knell_mints_blood_from_the_tower() {
    let mut engine = engine_with_config(EngineConfig {
        idle_mode: cathedral_sim::IdleCognitionMode::Stage,
        ..Default::default()
    });
    let at = Vec3::new(-140.5, 17.0, -275.6);
    for (id, offset) in [("player", 0.0), ("sv3n1", 20.0), ("cb947", 20.01)] {
        stand(engine.world_mut(), id, at + Vec3::X * offset);
    }
    let expected: BTreeSet<_> = engine
        .world()
        .characters_within(at, cathedral_sim::HEARING_RADIUS_M, None)
        .into_iter()
        .collect();
    let messages = engine.poll(0.0, vec![EngineCommand::Knell { years: 17, at }]);
    let knell = engine
        .world()
        .knowledge
        .facts()
        .find(|(_, f)| f.topic == Topic::Blood)
        .unwrap()
        .1;
    assert_eq!(knell.seeded, expected);
    assert!(knell.subject.is_empty() && knell.quiet_among.is_empty() && knell.craft_ear.is_none());
    assert_eq!(
        knell.garble,
        knowledge::GarbleMask {
            subject: false,
            place: false,
            day: true
        }
    );
    assert!(!knell.is_claimed());
    let held = knowledge::holds_key(engine.world(), &actor("sv3n1"), knell.key).unwrap();
    let line =
        knowledge::render_plain(engine.world(), &actor("sv3n1"), knell.key, &held, Some(0.0))
            .unwrap();
    assert!(line.contains("17") && !line.contains('{'));
    assert!(messages.iter().any(|m| matches!(m, EngineMessage::Journal { entries, .. } if entries.iter().any(|e| e.word.contains("17") && e.hops == 0 && e.from.is_none()))));
    let count = engine.world().knowledge.len();
    engine.poll(
        0.0,
        vec![
            EngineCommand::Knell { years: 17, at },
            EngineCommand::Knell { years: 18, at },
        ],
    );
    assert_eq!(engine.world().knowledge.len(), count + 1);
    // Bell knowledge itself changes no snapshot byte. Scheduler activity may
    // affect the world, so compare against a fresh direct mint for this claim.
    let mut direct = engine_with_config(EngineConfig::default());
    let before = serde_json::to_vec(&direct.world().public_snapshot(&actor("player"))).unwrap();
    knowledge::mint::mint_knell(direct.world_mut(), at, 17, 0.0).unwrap();
    assert_eq!(
        before,
        serde_json::to_vec(&direct.world().public_snapshot(&actor("player"))).unwrap()
    );
}

#[test]
fn peal_topic_covers_every_rope() {
    for rope in [CivicRope::Curfew, CivicRope::Summons] {
        let topic = match rope {
            CivicRope::Curfew | CivicRope::Summons => Topic::Law,
        };
        assert_eq!(knowledge::mint::peal_topic(rope), Some(topic));
    }
}

#[test]
fn a_peal_reheats_matching_air_and_never_mints() {
    let mut world = world_with_officer();
    let law = fact(&mut world, "bell.law", "law", "sv3n1", &[]);
    let bed = fact(&mut world, "bell.bed", "bed", "sv3n1", &[]);
    let home = world.ward_at(IN_FABRIC).unwrap();
    let away = world.ward_at(IN_WEIGH).unwrap();
    assert_ne!(home, away);
    for (ward, key) in [(home, law), (home, bed), (away, law)] {
        world
            .knowledge
            .deposit(ward, key, 2, 0.02, &actor("officr"), 0.0);
    }
    let before = world.knowledge.clone();
    knowledge::pollen::amplify(&mut world, IN_FABRIC, 0.0, Some(Topic::Law), 0.0);
    let air = world.knowledge.drift(home, law).unwrap();
    assert_eq!(air.heat, knowledge::REHEAT_TO);
    assert_eq!(air.stir, before.drift(home, law).unwrap().stir + 1);
    assert_eq!(world.knowledge.drift(home, bed), before.drift(home, bed));
    assert_eq!(world.knowledge.drift(away, law), before.drift(away, law));
    assert_eq!(world.knowledge.len(), before.len());
    let once = world.knowledge.clone();
    knowledge::pollen::amplify(&mut world, IN_FABRIC, 0.0, Some(Topic::Law), 0.0);
    assert_eq!(once, world.knowledge);
    world.knowledge_enabled = false;
    knowledge::pollen::amplify(&mut world, IN_WEIGH, 500.0, None, 0.0);
    assert_eq!(once, world.knowledge);
}

#[test]
fn a_peal_reads_cell_centres_and_changes_stir_only_with_whole_percent_heat() {
    let mut engine = engine_with_config(EngineConfig::default());
    let key = fact(engine.world_mut(), "bell.cells", "law", "sv3n1", &[]);
    let centroids = knowledge::pollen::ward_centroids();
    // Find an authored grid centre well away from its ward's centroid.
    // The map dot has no authority over what the bell can reach.
    let (at, ward) = (0..91)
        .flat_map(|x| {
            (0..104).map(move |z| {
                Vec3::new(
                    -365.0 + (x as f64 + 0.5) * 8.0,
                    0.0,
                    -480.5 + (z as f64 + 0.5) * 8.0,
                )
            })
        })
        .find_map(|point| {
            engine
                .world()
                .ward_at(point)
                .filter(|w| centroids[w].distance(point) > 100.0)
                .map(|w| (point, w))
        })
        .unwrap();
    for w in centroids.keys() {
        engine
            .world_mut()
            .knowledge
            .deposit(*w, key, 2, 0.02, &actor("sv3n1"), 0.0);
    }
    engine.poll(
        0.0,
        vec![EngineCommand::CivicPeal {
            rope: CivicRope::Summons,
            at,
            radius_m: 1.0,
        }],
    );
    for w in centroids.keys() {
        assert_eq!(
            engine.world().knowledge.drift(*w, key).unwrap().heat,
            if *w == ward {
                knowledge::REHEAT_TO
            } else {
                0.02
            }
        );
    }
    let equal_percent = fact(engine.world_mut(), "bell.samepercent", "law", "sv3n1", &[]);
    let warmer = fact(engine.world_mut(), "bell.warmer", "law", "sv3n1", &[]);
    let below = knowledge::REHEAT_TO - 0.0001;
    assert_eq!(
        knowledge::heat_pct(below),
        knowledge::heat_pct(knowledge::REHEAT_TO)
    );
    engine
        .world_mut()
        .knowledge
        .deposit(ward, equal_percent, 2, below, &actor("sv3n1"), 0.0);
    engine
        .world_mut()
        .knowledge
        .deposit(ward, warmer, 2, 1.0, &actor("sv3n1"), 0.0);
    let old = engine
        .world()
        .knowledge
        .drift(ward, equal_percent)
        .unwrap()
        .clone();
    let old_warm = engine
        .world()
        .knowledge
        .drift(ward, warmer)
        .unwrap()
        .clone();
    knowledge::pollen::amplify(engine.world_mut(), at, 1.0, Some(Topic::Law), 0.0);
    let changed = engine.world().knowledge.drift(ward, equal_percent).unwrap();
    assert_eq!(changed.heat, knowledge::REHEAT_TO);
    assert_eq!(changed.stir, old.stir);
    assert_eq!(
        engine.world().knowledge.drift(ward, warmer).unwrap(),
        &old_warm
    );
}

#[test]
fn invalidation_clears_a_fact_from_every_sheet() {
    let mut world = world_with_officer();
    let station = cathedral_sim::custody::Station {
        place_id: cathedral_sim::PlaceId::from_raw("hidden_station"),
        name: "a gate".into(),
        point: IN_FABRIC,
        stone_house: false,
    };
    world
        .custody
        .seize(actor("sv3n1"), actor("officr"), None, station, 0.0);
    let item = cathedral_sim::ItemId::from_raw("hidden_binding_item");
    world.add_item(cathedral_sim::Item::new(item.clone(), "loaf"));
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .holds
        .push(item.clone());
    let rows = json!([
        {"id":"stale.custody","topic":"law","said":"somebody is held","seeded":["k0fb1"],"source":{"custody":"sv3n1"}},
        {"id":"stale.item","topic":"coin","said":"a loaf is in those hands","seeded":["k0fb1"],"source":{"item":item.as_str()}},
        {"id":"stale.authored","topic":"bed","said":"a word remains","seeded":["k0fb1"]}
    ]);
    let doc = json!({"schema_version":1,"facts":rows});
    assert!(
        FactCatalog::from_json(&doc.to_string())
            .unwrap()
            .seed(&mut world)
            .is_empty()
    );
    let keys: Vec<_> = world.knowledge.facts().map(|(key, _)| key).collect();
    let ward = world.ward_at(IN_FABRIC).unwrap();
    for key in &keys {
        give(&mut world, "cb947", *key, 1, 1.0, None, "k0fb1", 0.0);
        world
            .knowledge
            .deposit(ward, *key, 1, 1.0, &actor("k0fb1"), 0.0);
    }
    knowledge::pollen::sweep(&mut world, 0.0);
    assert_eq!(world.knowledge.len(), 3);
    let before_prompt = render_prompt(&world, &actor("cb947"), None, &prompt_env()).unwrap();
    assert!(
        before_prompt.contains("somebody is held")
            && before_prompt.contains("a loaf is in those hands")
    );
    world.custody.release(&actor("sv3n1"));
    // Remove the actual object but deliberately leave the stale hand id.
    world.items.remove(&item);
    assert!(world.characters[&actor("sv3n1")].holds().contains(&item));
    knowledge::pollen::sweep(&mut world, (1.0 / knowledge::STIRS_PER_GAME_HOUR) / 24.0);
    assert_eq!(world.knowledge.len(), 1);
    assert_eq!(
        world.knowledge.facts().next().unwrap().1.id,
        FactId::from_raw("stale.authored")
    );
    assert_eq!(world.knowledge.holdings_len(&actor("cb947")), 1);
    assert_eq!(world.knowledge.ward_air(ward).count(), 1);
    let after_prompt = render_prompt(&world, &actor("cb947"), None, &prompt_env()).unwrap();
    assert!(
        !after_prompt.contains("somebody is held")
            && !after_prompt.contains("a loaf is in those hands")
    );
    assert!(after_prompt.contains("a word remains"));
    // A transfer invalidates in exactly the same beat, even when the item
    // survives in another person's hands.
    world.add_item(cathedral_sim::Item::new(item.clone(), "loaf"));
    FactCatalog::from_json(&doc.to_string())
        .unwrap()
        .seed(&mut world);
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .state
        .holds
        .retain(|i| i != &item);
    world
        .characters
        .get_mut(&actor("cb947"))
        .unwrap()
        .state
        .holds
        .push(item);
    knowledge::pollen::sweep(
        &mut world,
        2.0 * (1.0 / knowledge::STIRS_PER_GAME_HOUR) / 24.0,
    );
    assert_eq!(world.knowledge.len(), 1);
}

#[test]
fn a_stranger_deed_mints_about_the_player_and_they_never_pick_it_up() {
    for kind in knowledge::mint::STRANGER_DEED_KINDS {
        let mut world = world_with_officer();
        let mut event = DomainEvent::world_event(
            *kind,
            actor("player"),
            None,
            None,
            1,
            IN_FABRIC,
            vec![actor("fake_recipient")],
        );
        event.sequence = 42;
        event.text = Some("free text must never become the fact".into());
        let key =
            knowledge::mint::mint_stranger_deed(&mut world, &event, &actor("player"), 0.0).unwrap();
        let f = world.knowledge.fact(key).unwrap();
        assert_eq!(f.subject, vec![actor("player")]);
        assert_eq!(
            f.said,
            knowledge::mint::MINT_KINDS
                .iter()
                .find(|r| r.kind == *kind)
                .unwrap()
                .said
        );
        assert!(!f.is_claimed() && !f.garble.subject && f.garble.place && f.garble.day);
        let own = knowledge::holds_key(&world, &actor("player"), key).unwrap();
        assert_eq!((own.hops, own.from), (0, None));
        let receipt = &world.knowledge.player_learned[&f.id];
        assert!(receipt.word.starts_with("You ") && receipt.from.is_none() && receipt.hops == 0);
        assert!(
            knowledge::render_plain(
                &world,
                &actor("player"),
                key,
                &knowledge::holds_key(&world, &actor("player"), key).unwrap(),
                Some(0.0)
            )
            .is_none()
        );
        let listener = knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap();
        let rendered = knowledge::render_line(
            &world,
            &actor("k0fb1"),
            key,
            &listener,
            prompt_env().strings(),
            Some(0.0),
        )
        .unwrap();
        assert!(rendered.contains("a stranger") && !rendered.contains("Player"));
        let away = world.ward_at(IN_WEIGH).unwrap();
        stand(&mut world, "player", IN_WEIGH);
        knowledge::pollen::poll_player(&mut world, &actor("player"), 0.0);
        assert!(
            world.knowledge.drift(away, key).is_none(),
            "subject never deposits their own deed"
        );
        let before = world.knowledge.len();
        event.actor_id = Some(actor("officr"));
        assert!(
            knowledge::mint::mint_stranger_deed(&mut world, &event, &actor("player"), 0.0)
                .is_none()
        );
        assert!(
            knowledge::mint::mint_stranger_deed(&mut world, &event, &actor("officr"), 0.0)
                .is_none()
        );
        assert_eq!(world.knowledge.len(), before);
    }
}

#[test]
fn a_door_does_not_open_to_somebody_it_has_heard_of() {
    let mut world = world_with_officer();
    world
        .places
        .add_home(&actor("officr"), "Tib Officer", IN_FABRIC);
    let key = fact(&mut world, "door.news", "bed", "player", &["officr"]);
    assert_eq!(
        world.knowledge.holdings_len(&actor("officr")),
        0,
        "seeded witnesses also close doors"
    );
    assert!(knowledge::door_is_shut(
        &world,
        &actor("officr"),
        &actor("player")
    ));
    for who in ["player", "officr"] {
        stand(
            &mut world,
            who,
            IN_FABRIC + Vec3::X * knowledge::DOOR_SHUT_REACH_M,
        );
        assert!(knowledge::door_is_shut(
            &world,
            &actor("officr"),
            &actor("player")
        ));
        stand(
            &mut world,
            who,
            IN_FABRIC + Vec3::X * (knowledge::DOOR_SHUT_REACH_M + 0.01),
        );
        assert!(!knowledge::door_is_shut(
            &world,
            &actor("officr"),
            &actor("player")
        ));
        stand(&mut world, who, IN_FABRIC);
    }
    world
        .characters
        .get_mut(&actor("officr"))
        .unwrap()
        .notify_percept("A knock at the door.");
    assert!(!knowledge::door_is_shut(
        &world,
        &actor("officr"),
        &actor("player")
    ));
    world
        .characters
        .get_mut(&actor("officr"))
        .unwrap()
        .state
        .inbox
        .clear();
    world.knowledge_enabled = false;
    assert!(!knowledge::door_is_shut(
        &world,
        &actor("officr"),
        &actor("player")
    ));
    world.knowledge_enabled = true;
    world.knowledge.invalidate(key);
    assert!(!knowledge::door_is_shut(
        &world,
        &actor("officr"),
        &actor("player")
    ));
}

#[test]
fn the_stage_door_gate_is_independent_of_news_and_a_knock_still_gets_a_turn() {
    for requires_news in [false, true] {
        for mode in [
            cathedral_sim::IdleCognitionMode::Stage,
            cathedral_sim::IdleCognitionMode::All,
        ] {
            let mut engine = Engine::new(
                EngineConfig {
                    idle_mode: mode,
                    idle_requires_news: requires_news,
                    ..Default::default()
                },
                &cathedral_sim::WorldSeed::from_json_str(&demo_seed()).unwrap(),
                areas(),
                catalog(),
                prompt_env(),
                Box::new(FakeCognition::new()),
                Box::new(NullTranscription),
                Box::new(NullTts),
                Box::new(NullSight),
                Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
                (IN_FABRIC, 0.0),
                0,
                0.0,
            )
            .unwrap();
            stand(engine.world_mut(), "sv3n1", IN_FABRIC);
            for other in ["cb947", "k0fb1"] {
                stand(engine.world_mut(), other, IN_WEIGH + Vec3::X * 1000.0);
            }
            engine
                .world_mut()
                .places
                .add_home(&actor("sv3n1"), "Sven", IN_FABRIC);
            fact(
                engine.world_mut(),
                "door.stage",
                "bed",
                "player",
                &["sv3n1"],
            );
            let messages = engine.poll(0.0, vec![]);
            let closed = messages.iter().filter(|m| matches!(m, EngineMessage::Diagnostic(line) if line.contains("does not open"))).count();
            if mode == cathedral_sim::IdleCognitionMode::All {
                assert_eq!(closed, 0);
                assert_eq!(
                    engine.scheduler().in_flight_actor_id(),
                    Some(&actor("sv3n1"))
                );
                continue;
            }
            assert_eq!(closed, 1);
            assert!(engine.scheduler().in_flight_actor_id().is_none());
            let quiet = engine.poll(0.1, vec![]);
            assert!(quiet.iter().all(
                |m| !matches!(m, EngineMessage::Diagnostic(line) if line.contains("does not open"))
            ));
            assert!(
                engine.knowledge_auxiliary_bytes() + engine.world().knowledge.footprint_bytes()
                    < 32 * 1024 * 1024
            );
            engine
                .world_mut()
                .characters
                .get_mut(&actor("sv3n1"))
                .unwrap()
                .notify_percept("There is a knock at your door.");
            engine.poll(0.2, vec![]);
            assert_eq!(
                engine.scheduler().in_flight_actor_id(),
                Some(&actor("sv3n1")),
                "an inbox gets a real scheduled turn even with the door gate active"
            );
        }
    }
}

#[test]
fn the_door_reach_covers_the_idle_leash() {
    let doc: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/world/rounds.json")).unwrap();
    assert_eq!(
        doc["archetypes"]["day_worker"]["leash_m"].as_f64().unwrap(),
        knowledge::DOOR_SHUT_REACH_M
    );
    // Other authored trades already have wider leashes; their households
    // close only while both bodies remain within the same ten-metre door reach.
}

#[test]
fn a_garbled_subject_raises_a_wrongful_summons_and_it_settles() {
    let mut world = world_with_officer();
    let key = fact(&mut world, "arrest.garbled", "law", "sv3n1", &[]);
    give(
        &mut world,
        "officr",
        key,
        2,
        1.0,
        Some("cb947"),
        "k0fb1",
        0.0,
    );
    let before = world.knowledge.len();
    assert_eq!(knowledge::raise_hearsay_words(&mut world, 0.0).len(), 1);
    let notice = world.notices.live()[0].clone();
    assert_eq!(notice.accused, Some(actor("cb947")));
    assert_eq!(notice.rung(), cathedral_sim::notices::Rung::Hearsay);
    assert!(
        notice
            .line()
            .ends_with("had at second hand, and nobody saw it")
    );
    assert_no_mechanism_words(&[&notice.line()]);
    assert!(
        world
            .notices
            .fresh_own_notice(&actor("officr"), &actor("cb947"), Some(0.0))
            .is_none(),
        "a hearsay raiser did not witness a breach"
    );
    cathedral_sim::apply_action(
        &mut world,
        &actor("officr"),
        "say",
        &json!({"text":"I am taking you in."}),
    )
    .unwrap();
    assert!(
        cathedral_sim::apply_action(
            &mut world,
            &actor("officr"),
            "seize",
            &json!({"person":"cb947"})
        )
        .is_err()
    );
    let warrant = world
        .notices
        .raise(
            "another word".into(),
            "another wrong".into(),
            None,
            None,
            Some(0.0),
            actor("officr"),
            Some(actor("cb947")),
            None,
            None,
        )
        .unwrap();
    world
        .notices
        .summon(warrant, actor("officr"), Office::HighWick, Some(0.0));
    world.notices.issue_warrants(0.0);
    assert_eq!(world.notices.against(&actor("cb947"))[0].id, warrant);
    cathedral_sim::apply_action(
        &mut world,
        &actor("officr"),
        "summon",
        &json!({"notice_id":notice.id}),
    )
    .unwrap();
    assert_eq!(
        world.notices.get(notice.id).unwrap().rung(),
        cathedral_sim::notices::Rung::Summoned
    );
    cathedral_sim::apply_action(
        &mut world,
        &actor("officr"),
        "settle_notice",
        &json!({"notice_id":notice.id}),
    )
    .unwrap();
    assert!(world.notices.get(notice.id).is_none());
    assert!(knowledge::raise_hearsay_words(&mut world, 1.0).is_empty());
    assert_eq!(world.knowledge.len(), before, "no notice-to-fact feedback");
}

#[test]
fn a_hearsay_word_mints_no_second_fact() {
    let mut engine = engine_with_config(EngineConfig::default());
    let mut world = world_with_officer();
    let key = fact(&mut world, "arrest.noecho", "law", "sv3n1", &[]);
    give(
        &mut world,
        "officr",
        key,
        2,
        1.0,
        Some("cb947"),
        "k0fb1",
        0.0,
    );
    *engine.world_mut() = world;
    engine.poll(0.0, vec![]);
    assert_eq!(engine.world().notices.live().len(), 1);
    assert_eq!(engine.world().knowledge.len(), 1);
    assert!(!cathedral_sim::actions::may_raise_word(
        engine.world(),
        &actor("officr")
    ));
    for n in 1..20 {
        engine.poll(n as f64 * 0.05, vec![]);
    }
    assert_eq!(engine.world().knowledge.len(), 1);
}

#[test]
fn a_fact_about_the_person_in_front_of_you_is_seated() {
    let mut world = world_with_officer();
    let key = fact(&mut world, "greeting.hidden", "stranger", "player", &[]);
    give(&mut world, "k0fb1", key, 4, 0.01, None, "officr", 0.0);
    for n in 0..3 {
        let hot = fact(&mut world, &format!("other.hot{n}"), "bed", "sv3n1", &[]);
        give(&mut world, "k0fb1", hot, 1, 1.0, None, "officr", 0.0);
    }
    for id in ["sv3n1", "cb947", "officr"] {
        stand(&mut world, id, IN_WEIGH);
    }
    let line = knowledge::render_line(
        &world,
        &actor("k0fb1"),
        key,
        &knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap(),
        prompt_env().strings(),
        Some(0.0),
    )
    .unwrap();
    let at_door = bullets(&world, "k0fb1", &[], &prompt_env()).unwrap();
    assert!(at_door.contains(&line));
    assert!(
        line.contains("gone dim"),
        "cold still beats every hop count, including greetings"
    );
    let (rendered, _) =
        cathedral_sim::prompt::render_prompt_and_drain(&mut world, &actor("k0fb1"), &prompt_env())
            .unwrap();
    assert!(rendered.contains(&line));
    assert_eq!(world.knowledge.take_seated(&actor("k0fb1")), vec![key]);
    stand(&mut world, "player", IN_FABRIC + Vec3::X * 20.0);
    assert!(
        bullets(&world, "k0fb1", &[], &prompt_env())
            .unwrap()
            .contains(&line),
        "hearing radius inclusive"
    );
    stand(&mut world, "player", IN_FABRIC + Vec3::X * 20.01);
    assert!(
        !bullets(&world, "k0fb1", &[], &prompt_env())
            .unwrap()
            .contains(&line)
    );
    assert!(
        knowledge::render_plain(
            &world,
            &actor("player"),
            key,
            &knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap(),
            Some(0.0)
        )
        .is_none()
    );
}

#[test]
fn a_garble_naming_the_reader_is_neither_seated_nor_reheated() {
    let mut world = world_with_officer();
    let key = fact(&mut world, "greeting.selfgarble", "bed", "sv3n1", &[]);
    give(
        &mut world,
        "k0fb1",
        key,
        1,
        1.0,
        Some("k0fb1"),
        "officr",
        0.0,
    );
    let held = knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap();
    assert!(
        knowledge::render_line(
            &world,
            &actor("k0fb1"),
            key,
            &held,
            prompt_env().strings(),
            Some(0.0)
        )
        .is_none()
    );
    assert!(knowledge::render_plain(&world, &actor("k0fb1"), key, &held, Some(0.0)).is_none());
    world
        .characters
        .get_mut(&actor("k0fb1"))
        .unwrap()
        .notify_percept("Tell me about the greeting business.");
    let (prompt, _) =
        cathedral_sim::prompt::render_prompt_and_drain(&mut world, &actor("k0fb1"), &prompt_env())
            .unwrap();
    assert!(!prompt.contains("was seen at"));
    assert!(world.knowledge.take_seated(&actor("k0fb1")).is_empty());
}

#[test]
fn an_unrenderable_self_garble_does_not_block_another_wrongful_word() {
    let mut world = world_with_officer();
    let own = fact(&mut world, "hearsay.self", "law", "sv3n1", &[]);
    let other = fact(&mut world, "hearsay.other", "law", "sv3n1", &[]);
    give(
        &mut world,
        "officr",
        own,
        1,
        1.0,
        Some("officr"),
        "k0fb1",
        0.0,
    );
    give(
        &mut world,
        "officr",
        other,
        2,
        1.0,
        Some("cb947"),
        "k0fb1",
        0.0,
    );
    assert_eq!(knowledge::raise_hearsay_words(&mut world, 0.0).len(), 1);
    assert_eq!(world.notices.live()[0].accused, Some(actor("cb947")));
}

#[test]
fn locative_area_labels_compose_without_a_second_preposition() {
    let mut world = world_with_officer();
    let labels: Vec<_> = world
        .area_map
        .areas
        .iter()
        .map(|area| (area.id.clone(), area.label.clone()))
        .collect();
    for (n, (area, label)) in labels.into_iter().enumerate() {
        let row = json!({"id":format!("place.compose{n}"),"topic":"bed","said":"a bell rang at {place}","place":area,"seeded":["sv3n1"]});
        assert!(seed_pack(&mut world, &row.to_string()).is_empty());
        let key = world
            .knowledge
            .key_of(&FactId::from_raw(format!("place.compose{n}")))
            .unwrap();
        let line = knowledge::render_plain(
            &world,
            &actor("sv3n1"),
            key,
            &knowledge::holds_key(&world, &actor("sv3n1"), key).unwrap(),
            Some(0.0),
        )
        .unwrap();
        let lower = line.to_lowercase();
        for double in ["at in ", "at inside ", "at next to ", "at at "] {
            assert!(!lower.contains(double), "{label}: {line}");
        }
    }
}

#[test]
fn holds_about_uses_the_wrong_name_and_the_current_clock() {
    let mut world = world_with_officer();
    let key = fact(&mut world, "reading.age", "coin", "sv3n1", &[]);
    give(
        &mut world,
        "officr",
        key,
        2,
        1.0,
        Some("cb947"),
        "k0fb1",
        0.0,
    );
    assert!(
        knowledge::holds_about(
            &world,
            &actor("officr"),
            &actor("cb947"),
            Some(Topic::Coin),
            None
        )
        .is_some()
    );
    assert!(
        knowledge::holds_about(&world, &actor("officr"), &actor("sv3n1"), None, None).is_none()
    );
    world.current_time.as_mut().unwrap().day = 10;
    assert!(
        knowledge::holds_about(&world, &actor("officr"), &actor("cb947"), None, None).is_none()
    );
}

#[test]
fn distinct_mouths_survive_stirs_and_bounded_receipts() {
    let mut world = world_with_officer();
    let key = fact(&mut world, "mouths.distinct", "bed", "sv3n1", &[]);
    for (from, hops, day) in [("k0fb1", 3, 0.0), ("cb947", 1, 0.1), ("k0fb1", 4, 0.2)] {
        give(&mut world, "player", key, hops, 1.0, None, from, day);
    }
    let id = FactId::from_raw("mouths.distinct");
    assert_eq!(world.knowledge.player_learned[&id].tellings, 2);
    assert_eq!(
        world.knowledge.player_learned[&id].from,
        Some(actor("cb947"))
    );
    let revision = world.knowledge.receipts_revision();
    for n in 3..60 {
        give(
            &mut world,
            "player",
            key,
            4,
            1.0,
            None,
            "k0fb1",
            n as f64 / 10.0,
        );
    }
    assert_eq!(world.knowledge.player_learned[&id].tellings, 2);
    assert_eq!(
        world.knowledge.receipts_revision(),
        revision,
        "repeated mouths change no receipt"
    );
    let before = world.knowledge.footprint_bytes();
    for n in 0..knowledge::PLAYER_RECEIPTS_MAX * 2 {
        give(
            &mut world,
            "player",
            key,
            4,
            1.0,
            None,
            &format!("mouth{n}"),
            0.0,
        );
    }
    assert_eq!(
        usize::from(world.knowledge.player_learned[&id].tellings),
        knowledge::PLAYER_RECEIPTS_MAX
    );
    let capped = world.knowledge.footprint_bytes();
    assert!(capped > before);
    give(&mut world, "player", key, 4, 1.0, None, "one_more", 0.0);
    assert_eq!(world.knowledge.footprint_bytes(), capped);
}

#[test]
fn ward_heat_is_quantised_deduped_and_complete() {
    let mut engine = engine_with_config(EngineConfig {
        idle_mode: cathedral_sim::IdleCognitionMode::Stage,
        ..Default::default()
    });
    let first = engine.poll(0.0, vec![]);
    let rows = first
        .iter()
        .find_map(|m| {
            if let EngineMessage::WardHeat { wards } = m {
                Some(wards)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(rows.len(), 8);
    assert!(rows.iter().all(|r| r.heat_pct == 0 && r.words == 0));
    assert_eq!(
        rows.iter().map(|r| r.ward).collect::<BTreeSet<_>>().len(),
        8
    );
    assert!(
        engine
            .poll(0.01, vec![])
            .iter()
            .all(|m| !matches!(m, EngineMessage::WardHeat { .. }))
    );
    let key = fact(engine.world_mut(), "map.heat", "bed", "sv3n1", &[]);
    let ward = engine.world().ward_at(IN_FABRIC).unwrap();
    engine
        .world_mut()
        .knowledge
        .deposit(ward, key, 2, 0.121, &actor("sv3n1"), 0.0);
    let warm = engine.poll(0.02, vec![]);
    assert!(warm.iter().any(|m| matches!(m, EngineMessage::WardHeat { wards } if wards.iter().any(|r| r.ward == ward && r.heat_pct == 12 && r.words == 1))));
    engine
        .world_mut()
        .knowledge
        .deposit(ward, key, 2, 0.124, &actor("sv3n1"), 0.0);
    assert!(
        engine
            .poll(0.03, vec![])
            .iter()
            .all(|m| !matches!(m, EngineMessage::WardHeat { .. }))
    );
    engine.world_mut().knowledge_enabled = false;
    assert!(engine.poll(0.04, vec![]).iter().any(|m| matches!(m, EngineMessage::WardHeat { wards } if wards.iter().all(|r| r.heat_pct == 0 && r.words == 0))));
}

#[test]
fn m5_adds_no_new_probability() {
    for source in [
        include_str!("../../src/knowledge/mod.rs"),
        include_str!("../../src/knowledge/mint.rs"),
        include_str!("../../src/knowledge/pollen.rs"),
    ] {
        assert!(
            !source.contains("rand::")
                && !source.contains("thread_rng")
                && !source.contains("_GAIN")
        );
        for line in source
            .lines()
            .filter(|line| line.starts_with("pub const ") && line.contains("CHANCE"))
        {
            assert!(line.contains("GARBLE_CHANCE_PER_HOP"));
        }
    }
}
