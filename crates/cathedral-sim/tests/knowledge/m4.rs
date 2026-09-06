//! M4's production paths: the offered verb, the retained telling and its journal.
use super::*;
use cathedral_sim::error::{ActionErrorCode, CommandErrorCode};
use cathedral_sim::knowledge::{FactView, Learned, Topic};
use cathedral_sim::{EngineCommand, EngineMessage, FactKey, IdleGate, NpcScheduler};

fn word_world() -> World {
    let mut world = seed_world();
    world.current_time = Some(WorldTime {
        day: 0,
        fraction: 0.0,
        office: Office::Dayspring,
        weekday: Weekday::Bellday,
    });
    for id in ["player", "k0fb1", "sv3n1", "cb947"] {
        stand(&mut world, id, IN_FABRIC);
    }
    world.add_character(character(
        "fg2sh",
        "Grigor Ashe",
        Some(profile(Some("Salt trader"), PlanningWard::Weigh)),
        &[],
    ));
    stand(&mut world, "fg2sh", IN_FABRIC + Vec3::X * 3.0);
    world
}
fn arm(world: &mut World, id: &str) {
    world
        .knowledge
        .note_occasion(&actor(id), Some(actor("fg2sh")), Some(actor("player")), 0.0);
}
fn raise(
    world: &mut World,
    id: &str,
    args: serde_json::Value,
) -> Result<String, cathedral_sim::ActionError> {
    cathedral_sim::apply_action(world, &actor(id), "raise_word", &args)
}
fn raised(world: &World) -> (FactKey, &knowledge::Fact) {
    world
        .knowledge
        .facts()
        .find(|(_, fact)| fact.is_claimed())
        .expect("a claim was installed")
}
fn claim_guards(world: &World, speaker: &str) {
    let (_, fact) = raised(world);
    assert_eq!(fact.claimant(), Some(&actor(speaker)));
    assert_eq!(fact.seeded, BTreeSet::from([actor(speaker)]));
    assert!(fact.decays);
    assert!(fact.own.contains_key(&actor(speaker)));
    assert!(!fact.garble.place && !fact.garble.day);
}
fn pack_fact(world: &mut World, name: &str, topic: &str, decays: bool, witness: &str) -> FactKey {
    let row = json!({"id":name,"topic":topic,"said":format!("the {name} matter was settled"),"seeded":[witness],"decays":decays});
    assert!(seed_pack(world, &row.to_string()).is_empty());
    world.knowledge.key_of(&FactId::from_raw(name)).unwrap()
}
fn arrival(world: &mut World, key: FactKey, hops: u8, from: &str, heat: f32, day: f64) -> Learned {
    knowledge::learn(
        world,
        &actor("player"),
        key,
        Telling {
            hops,
            from: Some(actor(from)),
            heat,
            view: FactView::default(),
        },
        Some(day),
    )
}
/// Start a real scheduler exchange, then land controlled output into its request.
/// Rendering, draining, reheat, withdrawal and failure all run their real paths.
fn turn(world: &mut World, who: &str, since: &str, reply: Result<&str, &str>) -> String {
    world
        .characters
        .get_mut(&actor(who))
        .unwrap()
        .state
        .inbox
        .push(since.into());
    let mut scheduler = NpcScheduler::new(vec![actor(who)], 1.0, 60.0, 0.0);
    let mut fake = FakeCognition::new();
    let env = prompt_env();
    let mut transcript = Vec::new();
    scheduler.start(0.0);
    scheduler.poll(
        0.0,
        world,
        &mut transcript,
        &mut Vec::new(),
        false,
        IdleGate::All,
        &mut fake,
        &env,
    );
    let prompt = fake.prompts()[0].clone();
    let mut completions = fake.drain_completions();
    completions[0].result = reply
        .map(str::to_string)
        .map_err(cathedral_sim::CognitionError::new);
    scheduler.close();
    scheduler.poll(
        0.1,
        world,
        &mut transcript,
        &mut completions,
        false,
        IdleGate::All,
        &mut fake,
        &env,
    );
    prompt
}
const SPEAK: &str = r#"say {"target":"player","text":"Aye, I remember that business."}"#;

#[test]
fn raise_word_yields_claimed_and_nothing_else() {
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    raise(
        &mut world,
        "k0fb1",
        json!({"topic":"law","said":"Grigor Ashe was taken in charge"}),
    )
    .unwrap();
    claim_guards(&world, "k0fb1");
    let mut engine = engine_with_config(EngineConfig::default());
    engine.poll(
        0.0,
        vec![EngineCommand::DebugRaiseWord {
            who: "Ilse".into(),
            topic: "bed".into(),
            said: "a light burned in the upper room".into(),
        }],
    );
    claim_guards(engine.world(), "k0fb1");
    let mut world = word_world();
    let mut law = profile(Some("Watchman"), PlanningWard::Wick);
    law.occupation_id = Some("watchman_and_keeper".into());
    world
        .characters
        .get_mut(&actor("sv3n1"))
        .unwrap()
        .sheet
        .lore = Some(law);
    cathedral_sim::apply_action(
        &mut world,
        &actor("sv3n1"),
        "raise_notice",
        &json!({"about":"Grigor Ashe","deed":"short measure at the meal board","accused":"fg2sh"}),
    )
    .unwrap();
    let (_, fact) = world.knowledge.facts().next().expect("notice minted");
    assert!(!fact.is_claimed());
    assert_eq!(fact.claimant(), None);
    let row = knowledge::mint::MINT_KINDS
        .iter()
        .find(|r| r.kind == "raise_notice")
        .unwrap();
    assert_eq!(
        fact.said,
        row.said
            .replace("{deed}", "short measure at the meal board")
    );
    // Free-text claims enter only through mint_claim; coded notice prose asserts
    // that an accusation exists inside this fixed template (D61).
    for kind in knowledge::mint::STRANGER_DEED_KINDS {
        let mut event = cathedral_sim::DomainEvent::world_event(
            *kind,
            actor("player"),
            None,
            None,
            1,
            world.characters[&actor("player")].position_m(),
            vec![],
        );
        event.sequence = if *kind == "draw_mark" { 701 } else { 702 };
        event.text = Some("a model's assertion must never become deed prose".into());
        let key =
            knowledge::mint::mint_stranger_deed(&mut world, &event, &actor("player"), 0.0).unwrap();
        let fact = world.knowledge.fact(key).unwrap();
        assert!(!fact.is_claimed() && fact.claimant().is_none());
        assert_eq!(
            fact.said,
            knowledge::mint::MINT_KINDS
                .iter()
                .find(|row| row.kind == *kind)
                .unwrap()
                .said
        );
    }
}
#[test]
fn a_claim_names_its_subject_through_the_placeholder() {
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    let said = "GRIGOR ASHE gave short measure at the cellars";
    raise(&mut world, "k0fb1", json!({"said":said,"topic":"coin"})).unwrap();
    let (key, fact) = raised(&world);
    assert_eq!(fact.said, "{subject} gave short measure at the cellars");
    assert_eq!(fact.own[&actor("k0fb1")], said);
    assert!(fact.garble.subject);
    let held = knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap();
    let text = knowledge::render_plain(&world, &actor("player"), key, &held, Some(0.0)).unwrap();
    assert!(!text.contains("Grigor Ashe"));
    assert!(text.contains("salt trader of the Weigh Ward"), "{text}");
    world
        .characters
        .get_mut(&actor("player"))
        .unwrap()
        .state
        .knows
        .insert(actor("fg2sh"));
    assert!(
        knowledge::render_plain(&world, &actor("player"), key, &held, Some(0.0))
            .unwrap()
            .contains("Grigor Ashe")
    );
}
#[test]
fn a_claim_garbles_only_what_its_template_names() {
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    raise(
        &mut world,
        "k0fb1",
        json!({"said":"the salt weighed short by midday","topic":"coin"}),
    )
    .unwrap();
    assert_eq!(raised(&world).1.garble, knowledge::GarbleMask::NONE);
}
#[test]
fn an_unrecognised_topic_lands_on_talk() {
    for tag in [json!("scandalous"), json!(17), json!(null)] {
        let mut world = word_world();
        arm(&mut world, "k0fb1");
        let args = if tag.is_null() {
            json!({"said":"the salt weighed short"})
        } else {
            json!({"said":"the salt weighed short","topic":tag})
        };
        raise(&mut world, "k0fb1", args).unwrap();
        assert_eq!(raised(&world).1.topic, Topic::Talk);
    }
    assert_eq!(Topic::parse_or_talk("scandalous"), Topic::Talk);
}
#[test]
fn a_claim_cannot_invent_a_person_to_be_about() {
    for nearby in [false, true] {
        let mut world = word_world();
        world
            .knowledge
            .note_occasion(&actor("k0fb1"), None, None, 0.0);
        if !nearby {
            stand(&mut world, "fg2sh", IN_WEIGH);
        }
        raise(
            &mut world,
            "k0fb1",
            json!({"said":"Grigor Ashe went to the cellar"}),
        )
        .unwrap();
        assert_eq!(
            raised(&world).1.subject,
            if nearby { vec![actor("fg2sh")] } else { vec![] }
        );
    }
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    raise(
        &mut world,
        "k0fb1",
        json!({"said":"the man nobody can name was there"}),
    )
    .unwrap();
    assert_eq!(raised(&world).1.subject, vec![actor("fg2sh")]);
}
#[test]
fn raise_word_is_absent_without_an_occasion_and_present_with_one() {
    let mut world = word_world();
    let env = prompt_env();
    let who = actor("k0fb1");
    assert!(
        !render_prompt(&world, &who, None, &env)
            .unwrap()
            .contains("raise_word {")
    );
    assert!(!cathedral_sim::actions::may_raise_word(&world, &who));
    knowledge::mint::note_assertion(
        &mut world,
        &actor("player"),
        &who,
        "Grigor Ashe gave me short measure at the salt cellars",
        0.0,
    );
    assert!(
        render_prompt(&world, &who, None, &env)
            .unwrap()
            .contains("raise_word {")
    );
    assert!(cathedral_sim::actions::may_raise_word(&world, &who));
}
#[test]
fn raise_word_is_refused_past_the_cap_and_on_a_collision() {
    let mut world = word_world();
    let args = json!({"said":"Grigor Ashe weighed salt short","topic":"coin"});
    assert_eq!(
        raise(&mut world, "k0fb1", args.clone()).unwrap_err().code,
        ActionErrorCode::NoOccasion
    );
    arm(&mut world, "k0fb1");
    raise(&mut world, "k0fb1", args.clone()).unwrap();
    assert_eq!(
        raise(&mut world, "k0fb1", args.clone()).unwrap_err().code,
        ActionErrorCode::WordAlreadySaid
    );
    arm(&mut world, "sv3n1");
    assert_eq!(
        raise(&mut world, "sv3n1", args).unwrap_err().code,
        ActionErrorCode::WordAlreadyInTheAir
    );
    for code in [
        ActionErrorCode::NoOccasion,
        ActionErrorCode::WordAlreadySaid,
        ActionErrorCode::WordAlreadyInTheAir,
    ] {
        assert_eq!(
            CommandErrorCode::from(code),
            CommandErrorCode::InvalidAction
        );
    }
}
#[test]
fn a_refused_raise_spends_no_occasion() {
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    raise(
        &mut world,
        "k0fb1",
        json!({"said":"Grigor Ashe weighed salt short","topic":"coin"}),
    )
    .unwrap();
    arm(&mut world, "sv3n1");
    let before = world.knowledge.occasion(&actor("sv3n1")).cloned();
    assert_eq!(
        raise(
            &mut world,
            "sv3n1",
            json!({"said":"Grigor Ashe weighed salt short","topic":"coin"})
        )
        .unwrap_err()
        .code,
        ActionErrorCode::WordAlreadyInTheAir
    );
    assert_eq!(world.knowledge.occasion(&actor("sv3n1")), before.as_ref());
    assert_eq!(
        world
            .knowledge
            .raises_left(&actor("sv3n1"), 0, Office::Dayspring),
        1
    );
    raise(
        &mut world,
        "sv3n1",
        json!({"said":"Grigor Ashe weighed salt short","topic":"law"}),
    )
    .unwrap();
}
#[test]
fn raise_word_is_absent_when_knowledge_is_disabled() {
    let mut world = word_world();
    world.knowledge_enabled = false;
    knowledge::mint::note_assertion(
        &mut world,
        &actor("player"),
        &actor("k0fb1"),
        "Grigor Ashe weighed salt short this morning",
        0.0,
    );
    let event = cathedral_sim::DomainEvent::world_event(
        "uncovered",
        actor("player"),
        None,
        None,
        1,
        IN_FABRIC,
        vec![],
    );
    knowledge::mint::note_unminted_event(&mut world, &event, 0.0);
    assert!(world.knowledge.occasion(&actor("k0fb1")).is_none());
    arm(&mut world, "k0fb1");
    assert!(!cathedral_sim::actions::may_raise_word(
        &world,
        &actor("k0fb1")
    ));
    assert_eq!(
        raise(&mut world, "k0fb1", json!({"said":"the salt was short"}))
            .unwrap_err()
            .code,
        ActionErrorCode::NoOccasion
    );
    assert!(
        !render_prompt(&world, &actor("k0fb1"), None, &prompt_env())
            .unwrap()
            .contains("raise_word {")
    );
}
#[test]
fn telling_somebody_a_thing_they_do_not_hold_arms_the_verb() {
    let mut world = word_world();
    cathedral_sim::apply_action(
        &mut world,
        &actor("player"),
        "say",
        &json!({"target":"k0fb1","text":"Grigor Ashe gave short measure at the salt cellars"}),
    )
    .unwrap();
    let occasion = world.knowledge.occasion(&actor("k0fb1")).unwrap();
    assert_eq!(occasion.subject, Some(actor("fg2sh")));
    assert_eq!(occasion.from, Some(actor("player")));
    assert!(world.knowledge.occasion(&actor("sv3n1")).is_none());
}
#[test]
fn telling_somebody_a_thing_they_already_hold_arms_nothing() {
    let mut world = word_world();
    seed_pack(
        &mut world,
        r#"{"id":"ashe.known","topic":"coin","said":"{subject} used the wrong beam","subject":["fg2sh"],"seeded":["k0fb1"]}"#,
    );
    cathedral_sim::apply_action(
        &mut world,
        &actor("player"),
        "say",
        &json!({"target":"k0fb1","text":"Grigor Ashe gave short measure at the salt cellars"}),
    )
    .unwrap();
    assert!(world.knowledge.occasion(&actor("k0fb1")).is_none());
}
#[test]
fn an_untargeted_player_line_arms_the_nearest_llm_hearer() {
    let mut engine = engine_with_config(EngineConfig::default());
    for (id, x) in [
        ("player", 0.0),
        ("k0fb1", 2.0),
        ("sv3n1", 4.0),
        ("cb947", 6.0),
    ] {
        stand(engine.world_mut(), id, IN_FABRIC + Vec3::X * x);
    }
    engine.poll(
        0.0,
        vec![EngineCommand::PlayerSay {
            request_id: "m4".into(),
            text: "the upper granary was emptied this morning".into(),
            position_m: IN_FABRIC,
            spatial_seq: 1,
        }],
    );
    assert_eq!(
        engine
            .world()
            .knowledge
            .occasion(&actor("k0fb1"))
            .unwrap()
            .from,
        Some(actor("player"))
    );
    for id in ["sv3n1", "cb947"] {
        assert!(engine.world().knowledge.occasion(&actor(id)).is_none());
    }
}
#[test]
fn a_question_or_a_grunt_arms_nobody() {
    for text in [
        "What of it?",
        "Aye.",
        "Did Grigor Ashe weigh salt short at the cellars?",
    ] {
        let mut world = word_world();
        knowledge::mint::note_assertion(&mut world, &actor("player"), &actor("k0fb1"), text, 0.0);
        assert!(world.knowledge.occasion(&actor("k0fb1")).is_none());
    }
}
#[test]
fn a_witnessed_event_the_whitelist_misses_arms_the_verb() {
    for kind in ["uncovered", "commit", "raise_notice"] {
        let mut world = word_world();
        stand(&mut world, "cb947", IN_WEIGH);
        let event = cathedral_sim::DomainEvent::world_event(
            kind,
            actor("player"),
            None,
            None,
            1,
            IN_FABRIC,
            vec![],
        );
        knowledge::mint::note_unminted_event(&mut world, &event, 0.0);
        assert_eq!(
            world.knowledge.occasion(&actor("k0fb1")).is_some(),
            kind == "uncovered"
        );
        assert!(world.knowledge.occasion(&actor("cb947")).is_none());
        assert!(world.knowledge.occasion(&actor("player")).is_none());
    }
    let mut world = word_world();
    let event = cathedral_sim::DomainEvent::speech(
        actor("player"),
        None,
        "a long statement spoken aloud".into(),
        IN_FABRIC,
        vec![],
    );
    knowledge::mint::note_unminted_event(&mut world, &event, 0.0);
    assert!(world.knowledge.occasion(&actor("k0fb1")).is_none());
    let mut engine = engine_with_config(EngineConfig::default());
    *engine.world_mut() = word_world();
    engine
        .world_mut()
        .emit(cathedral_sim::DomainEvent::world_event(
            "uncovered",
            actor("player"),
            None,
            None,
            1,
            IN_FABRIC,
            vec![],
        ));
    engine.poll(0.0, vec![]);
    assert!(engine.world().knowledge.occasion(&actor("k0fb1")).is_some());
}
#[test]
fn an_occasion_does_not_survive_a_bell_unless_it_is_on_a_sheet() {
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    world.current_time.as_mut().unwrap().fraction = 0.1;
    assert!(!cathedral_sim::actions::may_raise_word(
        &world,
        &actor("k0fb1")
    ));
    world.knowledge.expire_occasions(0.1);
    assert!(world.knowledge.occasion(&actor("k0fb1")).is_none());
    world.current_time = None;
    arm(&mut world, "k0fb1");
    assert!(cathedral_sim::actions::may_raise_word(
        &world,
        &actor("k0fb1")
    ));
    cathedral_sim::prompt::render_prompt_and_drain(&mut world, &actor("k0fb1"), &prompt_env())
        .unwrap();
    world.current_time = Some(WorldTime {
        day: 1,
        fraction: 0.0,
        office: Office::Watch,
        weekday: Weekday::Bellday,
    });
    world.knowledge.expire_occasions(1.0);
    assert!(cathedral_sim::actions::may_raise_word(
        &world,
        &actor("k0fb1")
    ));
    world.knowledge.withdraw_offer(&actor("k0fb1"));
    world.knowledge.expire_occasions(1.0);
    assert!(!cathedral_sim::actions::may_raise_word(
        &world,
        &actor("k0fb1")
    ));
}
#[test]
fn a_claim_walks_back_to_the_player() {
    let mut world = word_world();
    cathedral_sim::apply_action(&mut world,&actor("player"),"say",&json!({"target":"k0fb1","text":"Grigor Ashe gave me short measure at the salt cellars yesterday"})).unwrap();
    let reply = cathedral_sim::fake::fake_reply(
        &render_prompt(&world, &actor("k0fb1"), None, &prompt_env()).unwrap(),
    );
    assert!(reply.contains("raise_word"));
    turn(&mut world, "k0fb1", "", Ok(&reply));
    let (key, _) = raised(&world);
    assert_eq!(
        knowledge::chain(&world, &actor("k0fb1"), key),
        vec![actor("player")]
    );
    assert_eq!(
        knowledge::holds_key(&world, &actor("k0fb1"), key)
            .unwrap()
            .from,
        Some(actor("player"))
    );
    // Actual air pickup: its chain starts with the claimant, not anonymous air.
    let ward = world.ward_at(IN_FABRIC).unwrap();
    assert_eq!(
        world.knowledge.drift(ward, key).unwrap().via,
        Some(actor("k0fb1"))
    );
    knowledge::pollen::poll_person(&mut world, &actor("sv3n1"), 0.0);
    // No-lore actors on coin do not have a certain roll; find a success by stir.
    for stir in 1..100 {
        if knowledge::holds_key(&world, &actor("sv3n1"), key).is_some() {
            break;
        }
        knowledge::pollen::sweep(&mut world, stir as f64 / 48.0);
        knowledge::pollen::poll_person(&mut world, &actor("sv3n1"), stir as f64 / 48.0);
    }
    assert_eq!(
        knowledge::chain(&world, &actor("sv3n1"), key),
        vec![actor("k0fb1"), actor("player")]
    );
}

fn cold_world(topic: &str, witness: bool, decays: bool) -> (World, FactKey) {
    let mut world = word_world();
    let key = pack_fact(
        &mut world,
        "old.business",
        topic,
        decays,
        if witness { "k0fb1" } else { "sv3n1" },
    );
    if !witness {
        knowledge::learn(
            &mut world,
            &actor("k0fb1"),
            key,
            Telling {
                hops: 1,
                from: Some(actor("sv3n1")),
                heat: 0.02,
                view: FactView::default(),
            },
            Some(0.0),
        );
    }
    (world, key)
}
#[test]
fn the_relevance_reheat_is_bounded() {
    let (mut world, key) = cold_world("bed", false, true);
    let floor = std::hint::black_box(knowledge::REHEAT_TO);
    assert!(floor <= 0.15 && floor > knowledge::VOLUNTEER_HEAT);
    for _ in 0..10 {
        turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
        assert_eq!(
            knowledge::holds_key(&world, &actor("k0fb1"), key)
                .unwrap()
                .heat(Some(0.0)),
            knowledge::REHEAT_TO
        );
    }
    let bed = world.ward_at(IN_FABRIC).unwrap();
    knowledge::pollen::poll_person(&mut world, &actor("k0fb1"), 0.0);
    assert!(world.knowledge.drift(bed, key).is_some());
    let (mut world, key) = cold_world("craft", false, true);
    // Real off-trade profile: absent lore has a different, deliberate ear.
    let mut ear = profile(Some("Salt trader"), PlanningWard::Weigh);
    ear.occupation_id = Some("salt_trader".into());
    world
        .characters
        .get_mut(&actor("k0fb1"))
        .unwrap()
        .sheet
        .lore = Some(ear);
    turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
    assert_eq!(
        knowledge::holds_key(&world, &actor("k0fb1"), key)
            .unwrap()
            .heat(Some(0.0)),
        knowledge::REHEAT_TO
    );
    knowledge::pollen::poll_person(&mut world, &actor("k0fb1"), 0.0);
    assert!(world.knowledge.drift(bed, key).is_none());
}
#[test]
fn a_cold_witness_can_be_reheated() {
    let (mut world, key) = cold_world("law", true, true);
    world.current_time.as_mut().unwrap().day = 2;
    assert!(
        knowledge::holds_key(&world, &actor("k0fb1"), key)
            .unwrap()
            .heat(Some(2.0))
            < knowledge::REHEAT_TO
    );
    assert_eq!(world.knowledge.holdings_len(&actor("k0fb1")), 0);
    turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
    let held = knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap();
    assert_eq!(held.heat(Some(2.0)), knowledge::REHEAT_TO);
    assert_eq!(held.hops, 0);
    assert_eq!(held.from, None);
    assert!(held.view.is_pristine());
    assert_eq!(
        knowledge::learn(
            &mut world,
            &actor("k0fb1"),
            key,
            Telling {
                hops: 4,
                from: Some(actor("player")),
                heat: 1.0,
                view: FactView {
                    day_offset: 3,
                    ..Default::default()
                }
            },
            Some(2.0)
        ),
        Learned::Refused
    );
    assert_eq!(
        knowledge::holds_key(&world, &actor("k0fb1"), key).unwrap(),
        held
    );
    // The actual deposit must read the stored witness override too.
    knowledge::pollen::poll_person(&mut world, &actor("k0fb1"), 2.0);
    assert_eq!(
        world
            .knowledge
            .drift(world.ward_at(IN_FABRIC).unwrap(), key)
            .unwrap()
            .heat,
        knowledge::REHEAT_TO
    );
}
#[test]
fn a_standing_fact_reheats_into_the_air_and_nowhere_else() {
    let (mut world, key) = cold_world("talk", true, false);
    assert!(world.knowledge.air_is_empty());
    turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
    let ward = world.ward_at(IN_FABRIC).unwrap();
    let drift = world.knowledge.drift(ward, key).unwrap().clone();
    assert_eq!(drift.heat, knowledge::REHEAT_TO);
    assert_eq!(drift.stir, 0, "a new row starts on the current game stir");
    assert_eq!(
        knowledge::holds_key(&world, &actor("k0fb1"), key)
            .unwrap()
            .heat(Some(0.0)),
        1.0
    );
    assert_eq!(world.knowledge.air_entries(), 1);
    turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
    assert_eq!(world.knowledge.drift(ward, key), Some(&drift));
    // A previously cold row gets one fresh coin. Repeating the same ask cannot
    // keep buying coins once its heat has reached the absolute floor.
    knowledge::pollen::sweep(&mut world, 0.0);
    knowledge::pollen::sweep(&mut world, 0.05);
    let cold = world.knowledge.drift(ward, key).unwrap().clone();
    assert!(cold.heat < knowledge::REHEAT_TO);
    knowledge::reheat(&mut world, &actor("k0fb1"), key, Some(0.05));
    let reheated = world.knowledge.drift(ward, key).unwrap().clone();
    assert_eq!(reheated.stir, cold.stir.wrapping_add(1));
    assert_eq!(reheated.heat, knowledge::REHEAT_TO);
    knowledge::reheat(&mut world, &actor("k0fb1"), key, Some(0.05));
    assert_eq!(world.knowledge.drift(ward, key), Some(&reheated));
}
#[test]
fn the_reheat_never_cools_a_warm_fact() {
    let (mut world, key) = cold_world("bed", false, true);
    world
        .knowledge
        .set_heat_at(&actor("k0fb1"), key, 0.9, Some(0.0));
    turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
    assert_eq!(
        knowledge::holds_key(&world, &actor("k0fb1"), key)
            .unwrap()
            .heat(Some(0.0)),
        0.9
    );
}
#[test]
fn the_reheat_seats_exactly_what_the_sheet_seated() {
    for empty in [false, true] {
        let mut world = word_world();
        let names = if empty {
            ["nothing.first", "nothing.second", "absent.third"]
        } else {
            ["herring.first", "herring.second", "absent.third"]
        };
        let keys: Vec<_> = names
            .into_iter()
            .map(|id| pack_fact(&mut world, id, "talk", false, "k0fb1"))
            .collect();
        if !empty {
            world
                .characters
                .get_mut(&actor("k0fb1"))
                .unwrap()
                .state
                .inbox
                .push("What of the herring?".into());
        }
        let (prompt, _) = cathedral_sim::prompt::render_prompt_and_drain(
            &mut world,
            &actor("k0fb1"),
            &prompt_env(),
        )
        .unwrap();
        let seated = world.knowledge.take_seated(&actor("k0fb1"));
        assert_eq!(seated, keys[..2]);
        let bullets = known_bullets(&prompt).unwrap();
        assert_eq!(bullets.len(), 2);
        for key in seated {
            assert!(
                bullets
                    .iter()
                    .any(|line| line.contains(world.knowledge.fact(key).unwrap().said.as_str()))
            );
        }
    }
    // Six relevant rows still yield three visible rows and three warmed seats.
    let mut world = word_world();
    for n in 0..6 {
        pack_fact(&mut world, &format!("herring.{n}"), "talk", false, "k0fb1");
    }
    world
        .characters
        .get_mut(&actor("k0fb1"))
        .unwrap()
        .state
        .inbox
        .push("What of the herring?".into());
    let (prompt, _) =
        cathedral_sim::prompt::render_prompt_and_drain(&mut world, &actor("k0fb1"), &prompt_env())
            .unwrap();
    assert_eq!(
        known_bullets(&prompt).unwrap().len(),
        knowledge::KNOWN_SHEET_MAX
    );
    assert_eq!(
        world.knowledge.take_seated(&actor("k0fb1")).len(),
        knowledge::KNOWN_SHEET_MAX
    );
}
#[test]
fn a_silent_turn_reheats_nothing() {
    for reply in [Ok("wait {}"), Err("provider unavailable")] {
        let (mut world, key) = cold_world("bed", false, true);
        arm(&mut world, "k0fb1");
        turn(&mut world, "k0fb1", "What of the old business?", reply);
        assert_eq!(
            knowledge::holds_key(&world, &actor("k0fb1"), key)
                .unwrap()
                .heat(Some(0.0)),
            0.02
        );
        assert!(world.knowledge.take_seated(&actor("k0fb1")).is_empty());
        assert!(!world.knowledge.occasion(&actor("k0fb1")).unwrap().offered);
    }
}
#[test]
fn a_reheated_carrier_rewarms_their_ward_without_a_second_write() {
    let (mut world, key) = cold_world("bed", false, true);
    let ward = world.ward_at(IN_FABRIC).unwrap();
    world
        .knowledge
        .deposit(ward, key, 1, 0.02, &actor("k0fb1"), 0.0);
    let stir = world.knowledge.drift(ward, key).unwrap().stir;
    turn(&mut world, "k0fb1", "What of the old business?", Ok(SPEAK));
    assert_eq!(world.knowledge.drift(ward, key).unwrap().heat, 0.02);
    knowledge::pollen::poll_person(&mut world, &actor("k0fb1"), 0.0);
    let drift = world.knowledge.drift(ward, key).unwrap();
    assert_eq!(drift.heat, knowledge::REHEAT_TO);
    assert_eq!(drift.stir, stir);
}
#[test]
fn the_player_gets_a_receipt_for_every_telling() {
    let mut world = word_world();
    let key = pack_fact(&mut world, "heard.thrice", "bed", true, "k0fb1");
    arrival(&mut world, key, 3, "sv3n1", 0.4, 0.0);
    arrival(&mut world, key, 1, "k0fb1", 0.5, 0.01);
    stand(&mut world, "player", IN_WEIGH);
    arrival(&mut world, key, 4, "sv3n1", 0.5, 0.02);
    let receipt = &world.knowledge.player_learned[&FactId::from_raw("heard.thrice")];
    assert_eq!((receipt.tellings, receipt.wards, receipt.hops), (2, 2, 1));
    assert_eq!(receipt.from, Some(actor("k0fb1")));
    assert!(!receipt.word.is_empty());
}
#[test]
fn a_receipt_keeps_the_sentence_the_player_heard() {
    let mut world = word_world();
    let first = pack_fact(&mut world, "receipt.first", "bed", true, "k0fb1");
    arrival(&mut world, first, 1, "k0fb1", 0.1, 0.0);
    let word = world.knowledge.player_learned[&FactId::from_raw("receipt.first")]
        .word
        .clone();
    for n in 0..6 {
        let key = pack_fact(
            &mut world,
            &format!("receipt.filler{n}"),
            "bed",
            true,
            "k0fb1",
        );
        arrival(&mut world, key, 1, "k0fb1", 1.0, 0.0);
    }
    assert!(knowledge::holds_key(&world, &actor("player"), first).is_none());
    assert_eq!(
        world.knowledge.player_learned[&FactId::from_raw("receipt.first")].word,
        word
    );
    let key = pack_fact(&mut world, "receipt.selfeviction", "bed", true, "k0fb1");
    arrival(&mut world, key, 5, "k0fb1", 0.01, 0.0);
    assert!(knowledge::holds_key(&world, &actor("player"), key).is_none());
    assert!(
        !world.knowledge.player_learned[&FactId::from_raw("receipt.selfeviction")]
            .word
            .is_empty()
    );
}
#[test]
fn a_closer_telling_corrects_the_receipt_and_a_farther_one_does_not() {
    let mut world = word_world();
    let key = pack_fact(&mut world, "receipt.merged", "bed", true, "k0fb1");
    let id = FactId::from_raw("receipt.merged");
    for (hops, from, time) in [
        (3, "sv3n1", 0.0),
        (3, "cb947", 0.01),
        (4, "cb947", 0.02),
        (1, "k0fb1", 0.03),
        (0, "sv3n1", 0.04),
        (1, "cb947", 0.05),
    ] {
        arrival(&mut world, key, hops, from, 0.8, time);
        let r = &world.knowledge.player_learned[&id];
        let expected = if time < 0.03 {
            (3, "sv3n1", 0.0)
        } else if time < 0.04 {
            (1, "k0fb1", 0.03)
        } else {
            (0, "sv3n1", 0.04)
        };
        assert_eq!(
            (r.hops, r.from.clone(), r.at),
            (expected.0, Some(actor(expected.1)), Some(expected.2))
        );
    }
    assert_eq!(world.knowledge.player_learned[&id].tellings, 3);
}
#[test]
fn the_receipt_log_is_bounded_and_evicts_oldest_first() {
    fn run() -> Vec<FactId> {
        let mut world = word_world();
        for n in 0..80 {
            let key = pack_fact(&mut world, &format!("receipt.{n:02}"), "bed", true, "k0fb1");
            arrival(&mut world, key, 1, "k0fb1", 0.9, n as f64 / 100.0);
        }
        world.knowledge.player_learned.keys().cloned().collect()
    }
    let ids = run();
    assert_eq!(ids.len(), knowledge::PLAYER_RECEIPTS_MAX);
    assert_eq!(ids, run());
    assert_eq!(ids[0], FactId::from_raw("receipt.16"));
}
#[test]
fn a_receipt_is_not_a_percept() {
    let mut world = word_world();
    let key = pack_fact(&mut world, "receipt.silent", "bed", true, "k0fb1");
    let before: Vec<_> = world
        .characters
        .values()
        .map(|c| {
            (
                c.id().clone(),
                c.state.inbox.clone(),
                c.state.pending_history.clone(),
            )
        })
        .collect();
    arrival(&mut world, key, 1, "k0fb1", 1.0, 0.0);
    let after: Vec<_> = world
        .characters
        .values()
        .map(|c| {
            (
                c.id().clone(),
                c.state.inbox.clone(),
                c.state.pending_history.clone(),
            )
        })
        .collect();
    assert_eq!(before, after);
}
#[test]
fn the_player_rolls_on_player_curiosity_and_not_on_lore() {
    let mut world = word_world();
    let key = pack_fact(&mut world, "receipt.rolled", "bed", true, "k0fb1");
    let fact = world.knowledge.fact(key).unwrap();
    assert_eq!(
        knowledge::salience::salience(&world, fact, &actor("player")),
        1.0
    );
    let stir = (0..100)
        .find(|n| {
            !knowledge::pollen::picks_up(fact, &actor("player"), *n, knowledge::PLAYER_CURIOSITY)
        })
        .unwrap();
    assert!(knowledge::pollen::picks_up(
        fact,
        &actor("player"),
        stir,
        1.0
    ));
    let ward = world.ward_at(IN_FABRIC).unwrap();
    knowledge::pollen::debug_seed_air(&mut world, ward, key, Some(stir as f64 / 48.0));
    knowledge::pollen::poll_player(&mut world, &actor("player"), stir as f64 / 48.0);
    assert!(knowledge::holds_key(&world, &actor("player"), key).is_none());
    assert!(world.knowledge.player_learned.is_empty());
}
#[test]
fn the_standing_line_names_its_door() {
    let mut world = word_world();
    let key = knowledge::mint::mint_claim(
        &mut world,
        &actor("k0fb1"),
        Topic::Bed,
        "a light burned upstairs".into(),
        vec![],
        Some(actor("player")),
        Some(0.0),
    )
    .unwrap();
    let lines = knowledge::standing_lines(&world, &actor("player"));
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Fabric Ward")
            && lines[0].contains("1 mouth")
            && lines[0].contains("Say it straight")
    );
    let newer = knowledge::mint::mint_claim(
        &mut world,
        &actor("k0fb1"),
        Topic::Law,
        "the upper door was sealed".into(),
        vec![],
        Some(actor("player")),
        Some(0.0),
    )
    .unwrap();
    let lines = knowledge::standing_lines(&world, &actor("player"));
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("upper door"));
    world.knowledge.invalidate(newer);
    assert!(knowledge::standing_lines(&world, &actor("player"))[0].contains("light burned"));
    world.knowledge.invalidate(key);
    assert!(knowledge::standing_lines(&world, &actor("player")).is_empty());
}
#[test]
fn no_standing_line_when_nothing_is_live() {
    assert!(knowledge::standing_lines(&seed_world(), &actor("player")).is_empty());
}
#[test]
fn facts_never_enter_the_public_snapshot() {
    let mut world = word_world();
    let before = serde_json::to_vec(&world.public_snapshot(&actor("player"))).unwrap();
    let key = knowledge::mint::mint_claim(
        &mut world,
        &actor("k0fb1"),
        Topic::Bed,
        "the upper lamp burned".into(),
        vec![],
        Some(actor("player")),
        Some(0.0),
    )
    .unwrap();
    arrival(&mut world, key, 1, "k0fb1", 1.0, 0.0);
    assert!(!knowledge::standing_lines(&world, &actor("player")).is_empty());
    assert!(!world.knowledge.player_learned.is_empty());
    assert_eq!(
        before,
        serde_json::to_vec(&world.public_snapshot(&actor("player"))).unwrap()
    );
}

#[test]
fn a_stage_repeat_is_one_telling_per_mouth_and_game_stir() {
    let mut world = word_world();
    let key = pack_fact(&mut world, "receipt.repeated", "bed", true, "k0fb1");
    // Keep just the player and the first-hand mouth on stage, and the player
    // already holds it closer than the incoming pair. Old M3 code skipped it.
    for id in ["sv3n1", "cb947", "fg2sh"] {
        stand(&mut world, id, IN_WEIGH);
    }
    arrival(&mut world, key, 0, "k0fb1", 1.0, 0.0);
    let stir = (0..100)
        .find(|n| {
            knowledge::pollen::picks_up_from(
                world.knowledge.fact(key).unwrap(),
                &actor("player"),
                &actor("k0fb1"),
                *n,
                knowledge::PLAYER_CURIOSITY,
            )
        })
        .unwrap();
    let id = FactId::from_raw("receipt.repeated");
    let now = stir as f64 / 48.0;
    // The witness stays at full heat here so the test chooses precisely the
    // production chance, without silently selecting the wrong aged coin.
    world
        .knowledge
        .set_heat_at(&actor("k0fb1"), key, 1.0, Some(now));
    knowledge::pollen::hop_on_stage(&mut world, &actor("player"), 0.0, now);
    assert_eq!(world.knowledge.player_learned[&id].tellings, 1);
    for n in 1..30 {
        knowledge::pollen::hop_on_stage(&mut world, &actor("player"), n as f64 * 2.0, now);
    }
    assert_eq!(world.knowledge.player_learned[&id].tellings, 1);
    assert_eq!(world.knowledge.player_learned[&id].hops, 0);
}
#[test]
fn an_offered_occasion_keeps_the_source_its_sheet_was_given() {
    let mut world = word_world();
    arm(&mut world, "k0fb1");
    cathedral_sim::prompt::render_prompt_and_drain(&mut world, &actor("k0fb1"), &prompt_env())
        .unwrap();
    world.knowledge.note_occasion(
        &actor("k0fb1"),
        Some(actor("sv3n1")),
        Some(actor("cb947")),
        0.5,
    );
    let o = world.knowledge.occasion(&actor("k0fb1")).unwrap();
    assert_eq!(o.subject, Some(actor("fg2sh")));
    assert_eq!(o.from, Some(actor("player")));
    assert!(o.offered);
    world.current_time.as_mut().unwrap().day = 1;
    raise(
        &mut world,
        "k0fb1",
        json!({"said":"the salt weighed short"}),
    )
    .unwrap();
    let (key, fact) = raised(&world);
    assert_eq!(fact.subject, vec![actor("fg2sh")]);
    assert_eq!(
        knowledge::chain(&world, &actor("k0fb1"), key),
        vec![actor("player")]
    );
}

#[test]
fn a_receipt_names_where_it_was_heard_and_never_the_hidden_event_place() {
    let mut world = word_world();
    let place_ids: Vec<_> = world
        .area_map
        .areas
        .iter()
        .take(3)
        .map(|a| a.id.clone())
        .collect();
    assert_eq!(place_ids.len(), 3);
    let hearing = world.area_map.areas[2].boxes[0].clone();
    stand(&mut world, "player", (hearing.min_m + hearing.max_m) * 0.5);
    let row = json!({"id":"receipt.places","topic":"bed","said":"a light burned at {place}","place":place_ids[0],"seeded":["k0fb1"],"garble":"place"});
    seed_pack(&mut world, &row.to_string());
    let key = world
        .knowledge
        .key_of(&FactId::from_raw("receipt.places"))
        .unwrap();
    let garbled = world.area_map.key_of_id(&place_ids[1]).unwrap();
    knowledge::learn(
        &mut world,
        &actor("player"),
        key,
        Telling {
            hops: 2,
            from: Some(actor("k0fb1")),
            heat: 1.0,
            view: FactView {
                place: Some(garbled),
                ..Default::default()
            },
        },
        Some(0.0),
    );
    let receipt = &world.knowledge.player_learned[&FactId::from_raw("receipt.places")];
    assert_eq!(receipt.place, world.area_map.key_of_id(&place_ids[2]));
    assert!(
        receipt
            .word
            .to_lowercase()
            .contains(&world.area_map.label_of_key(garbled).unwrap().to_lowercase())
    );
    assert!(!receipt.word.contains(&world.area_map.areas[0].label));
}
#[test]
fn every_raise_refusal_and_fence_comment_keeps_mechanism_words_out() {
    let mut world = word_world();
    let args = json!({"said":"Grigor Ashe weighed salt short","topic":"coin"});
    let no = raise(&mut world, "k0fb1", args.clone())
        .unwrap_err()
        .to_string();
    arm(&mut world, "k0fb1");
    raise(&mut world, "k0fb1", args.clone()).unwrap();
    let cap = raise(&mut world, "k0fb1", args.clone())
        .unwrap_err()
        .to_string();
    arm(&mut world, "sv3n1");
    let collision = raise(&mut world, "sv3n1", args).unwrap_err().to_string();
    let fence = include_str!("../../../../assets/prompts/turn.j2")
        .lines()
        .find(|l| l.contains("{% if has_raise_word %}raise_word"))
        .unwrap();
    let comment = fence
        .split_once("# ")
        .unwrap()
        .1
        .split_once("topics:")
        .unwrap()
        .0;
    assert_no_mechanism_words(&[&no, &cap, &collision, comment]);
}
#[test]
fn journal_publishing_preserves_words_caps_rows_and_deduplicates() {
    let mut engine = engine_with_config(EngineConfig::default());
    assert!(
        !engine
            .poll(0.0, vec![])
            .iter()
            .any(|m| matches!(m, EngineMessage::Journal { .. }))
    );
    for n in 0..30 {
        let key = pack_fact(
            engine.world_mut(),
            &format!("published.{n:02}"),
            "bed",
            true,
            "k0fb1",
        );
        arrival(engine.world_mut(), key, 2, "k0fb1", 0.8, n as f64 / 100.0);
    }
    let messages = engine.poll(0.1, vec![]);
    let entries = messages
        .iter()
        .find_map(|m| match m {
            EngineMessage::Journal { entries, .. } => Some(entries),
            _ => None,
        })
        .unwrap();
    assert_eq!(entries.len(), knowledge::JOURNAL_ENTRIES_MAX);
    assert!(entries[0].word.contains("published.29"));
    assert!(
        !engine
            .poll(0.2, vec![])
            .iter()
            .any(|m| matches!(m, EngineMessage::Journal { .. }))
    );
    engine.world_mut().knowledge_enabled = false;
    assert!(engine.poll(1.2,vec![]).iter().any(|m|matches!(m,EngineMessage::Journal{entries,standing} if entries.is_empty() && standing.is_empty())));
}

#[test]
fn witnessing_a_coded_mint_leaves_a_first_hand_receipt_without_percepts() {
    let mut world = word_world();
    let before: Vec<_> = world
        .characters
        .values()
        .map(|c| (c.state.inbox.clone(), c.state.pending_history.clone()))
        .collect();
    let key = knowledge::mint::mint_commitment(
        &mut world,
        &actor("fg2sh"),
        Some(&actor("sv3n1")),
        "the gate arch",
        IN_FABRIC,
        Some(0.0),
    )
    .unwrap();
    let id = world.knowledge.fact(key).unwrap().id.clone();
    let receipt = &world.knowledge.player_learned[&id];
    assert_eq!(receipt.hops, 0);
    assert_eq!(receipt.from, None);
    assert_eq!(receipt.tellings, 1);
    assert!(!receipt.word.is_empty());
    assert_eq!(
        before,
        world
            .characters
            .values()
            .map(|c| (c.state.inbox.clone(), c.state.pending_history.clone()))
            .collect::<Vec<_>>()
    );
    let held = knowledge::holds_key(&world, &actor("player"), key).unwrap();
    assert_eq!(held.hops, 0);
    assert!(held.view.is_pristine());
    let self_key = knowledge::mint::mint_commitment(
        &mut world,
        &actor("player"),
        Some(&actor("sv3n1")),
        "the gate arch",
        IN_FABRIC,
        Some(0.0),
    )
    .unwrap();
    assert!(
        !world
            .knowledge
            .player_learned
            .contains_key(&world.knowledge.fact(self_key).unwrap().id)
    );
}
