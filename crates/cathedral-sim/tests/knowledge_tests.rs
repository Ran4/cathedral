//! What the city knows (`features/knowledge_and_rumor/`), from outside the crate.
//!
//! The unit tests in `knowledge/` pin the constants, the merge rule, the seal and
//! the twenty-one hedge cells. These pin the parts that need a real `World`, a
//! real `PromptEnv` or a real `Engine`: that no authored row can reach a frozen
//! fixture, that the store never enters the public snapshot, that provenance
//! reaches no projection, and that `arm_actor` seeds a goal and nothing else.
//!
//! `Fact::source` is private, so a fact is built here the only way an outside
//! caller can: through the loader, which gets its validation exercised for free.

mod prompt_support;

use std::collections::BTreeSet;

use cathedral_sim::knowledge::{self, FactCatalog, Telling};
use cathedral_sim::{
    ActorId, Capabilities, Character, CharacterSheet, Control, Engine, EngineConfig, FactId,
    FakeCognition, LoreProfile, NullSight, NullTranscription, NullTts, Office, PlanningWard,
    PromptEnv, Significance, TtsBackendKind, Vec3, WALK_Y, Weekday, World, WorldTime,
    prompt::render_prompt,
};
use prompt_support::{actor, areas, catalog, demo_seed, known_bullets, prompt_env, seed_world};
use serde_json::json;

/// Seed one inline-JSON pack into a world and return the diagnostics.
fn seed_pack(world: &mut World, facts: &str) -> Vec<String> {
    let json = format!("{{\"schema_version\": 1, \"facts\": [{facts}]}}");
    let catalog = FactCatalog::from_json(&json).expect("the pack parses");
    catalog.seed(world)
}

/// The demo world through the real `Engine::new`, so the knowledge block of it —
/// the switch, the packs and the one seeding pass — is the thing under test.
fn engine_with_config(config: EngineConfig) -> Engine {
    Engine::new(
        config,
        &cathedral_sim::WorldSeed::from_json_str(&demo_seed()).expect("the demo seed loads"),
        areas(),
        catalog(),
        prompt_env(),
        Box::new(FakeCognition::new()),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::new(false, false, false, false, false, TtsBackendKind::Off),
        (Vec3::new(0.0, WALK_Y, 111.0), 0.0),
        0,
        0.0,
    )
    .expect("the seeded world has a player")
}

/// T21. The frozen fixtures stay unreachable from the shipped asset **by
/// construction**, not only by `seed()`'s roster filter: no authored row names a
/// demo id, and no template carries a placeholder the renderer does not fill.
///
/// `b4kr1` exists only in `manifest.json`'s own world dump and is the easy one to
/// forget.
#[test]
fn no_authored_fact_names_a_golden_demo_id() {
    let demo = ["sv3n1", "cb947", "k0fb1", "player", "b4kr1"];
    for spec in FactCatalog::default().specs() {
        for id in demo {
            let id = ActorId::from_raw(id);
            assert!(
                !spec.subject.contains(&id),
                "fact {} names the demo actor {id} as a subject",
                spec.id
            );
            assert!(
                !spec.seeded.contains(&id),
                "fact {} seeds the demo actor {id}",
                spec.id
            );
            assert!(
                !spec.own.contains_key(&id),
                "fact {} gives the demo actor {id} an own line",
                spec.id
            );
        }
        for template in std::iter::once(&spec.said).chain(spec.own.values()) {
            let mut rest = template.as_str();
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                let close = after.find('}').unwrap_or_else(|| {
                    panic!("fact {} has an unclosed placeholder", spec.id);
                });
                let name = &after[..close];
                assert!(
                    ["subject", "place", "day"].contains(&name),
                    "fact {} names the placeholder {{{name}}}, which nothing substitutes",
                    spec.id
                );
                rest = &after[close + 1..];
            }
        }
    }
}

/// T29. Dropping a fact removes it from the store, from every holding **and from
/// the next sheet**, with no actor cooperation at all — no `forget`, no LLM, no
/// drift, and no turn spent on it.
#[test]
fn dropping_a_fact_clears_it_off_the_next_sheet() {
    let mut world = seed_world();
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.custody.row", "topic": "law", "said": "they are held at the gate",
            "seeded": ["sv3n1"], "source": {"custody": "cb947"}}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = FactId::from_raw("test.custody.row");
    let key = world.knowledge.key_of(&id).expect("the row installed");

    // A second holder, at one remove, so the sweep is seen to clear a carried row
    // as well as a seeded one.
    let carrier = actor("k0fb1");
    knowledge::learn(
        &mut world,
        &carrier,
        key,
        Telling {
            hops: 1,
            from: Some(actor("sv3n1")),
            heat: 1.0,
            view: Default::default(),
        },
        Some(0.0),
    );
    assert!(knowledge::holds(&world, &actor("sv3n1"), &id).is_some());
    assert!(knowledge::holds(&world, &carrier, &id).is_some());
    let env = prompt_env();
    assert_eq!(
        bullets(&world, "sv3n1", &[], &env).expect("the witness carries it on their sheet"),
        vec!["You saw this yourself: they are held at the gate".to_string()]
    );

    // Nobody is in custody in the demo world, so the source never bore it out.
    let dead = knowledge::invalidate_stale(&mut world);
    assert_eq!(dead, vec![id.clone()]);
    assert!(knowledge::holds(&world, &actor("sv3n1"), &id).is_none());
    assert!(knowledge::holds(&world, &carrier, &id).is_none());
    assert_eq!(world.knowledge.holdings_len(&carrier), 0);
    // And it is off the sheet on the next render, with nobody having acted.
    assert_eq!(bullets(&world, "sv3n1", &[], &env), None);
    assert_eq!(bullets(&world, "k0fb1", &[], &env), None);
}

/// T31. Facts never enter the public snapshot, and seeding never bumps the
/// revision — the store is prompt state, exactly as the notices are.
#[test]
fn facts_do_not_enter_the_public_snapshot() {
    let bare = seed_world();
    let player = actor("player");
    let bare_snapshot =
        serde_json::to_vec(&bare.public_snapshot(&player)).expect("the snapshot serialises");

    let mut world = seed_world();
    let revision_before = world.world_revision;
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.snapshot.one", "topic": "bed", "said": "a thing happened",
            "seeded": ["sv3n1", "cb947", "k0fb1"]},
           {"id": "test.snapshot.two", "topic": "coin", "said": "another thing happened",
            "seeded": ["sv3n1", "cb947"]}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(world.knowledge.len(), 2);
    assert_eq!(
        world.world_revision, revision_before,
        "seeding facts must not bump the public revision"
    );

    // Five holdings across three bodies, and the wire is byte-identical.
    let held: usize = ["sv3n1", "cb947", "k0fb1"]
        .iter()
        .map(|id| knowledge::holdings_of(&world, &actor(id)).len())
        .sum();
    assert_eq!(held, 5);
    let with_facts =
        serde_json::to_vec(&world.public_snapshot(&player)).expect("the snapshot serialises");
    assert_eq!(
        with_facts, bare_snapshot,
        "a fact reached the public snapshot"
    );
}

/// T34. `arm_actor` seeds the goal and nothing else, and their own `set_goal`
/// still wins afterwards — a seed, not an override.
///
/// Deliberately no memories parameter: a seeded memory is erasable by `forget` on
/// the first turn, so a quest whose hinge is a memory would become unwinnable with
/// no error raised.
#[test]
fn arm_actor_seeds_the_goal_and_set_goal_still_wins() {
    let mut world = seed_world();
    let id = actor("sv3n1");
    let memories_before = world.characters[&id].state.memories.clone();

    world.arm_actor(&id, Some("find the bale".into()));
    assert_eq!(world.characters[&id].state.goal, "find the bale");
    assert_eq!(
        world.characters[&id].state.memories, memories_before,
        "arm_actor must not touch stored_memories"
    );

    // Their own verb wins.
    cathedral_sim::actions::apply_action(&mut world, &id, "set_goal", &json!({"goal": "Eat fish"}))
        .expect("set_goal applies");
    assert_eq!(world.characters[&id].state.goal, "Eat fish");

    // `None` is the crate's own "no goal" sentinel.
    world.arm_actor(&id, None);
    assert_eq!(world.characters[&id].state.goal, cathedral_sim::GOAL_NONE);

    // An unknown id changes nothing and does not panic.
    let before = world.clone();
    world.arm_actor(&ActorId::from_raw("nobody-at-all"), Some("x".into()));
    assert_eq!(world, before);
}

/// T35. `FactSource` appears in no rendered string — the spec's backstop to the
/// structural seal, walked over every projection this milestone has.
///
/// The sentinel pack arrives through the real seam, `EngineConfig::fact_packs`,
/// so the authored spelling of provenance sits on `world.fact_catalog` for the
/// whole run exactly as a quest's would — and `World` derives `Debug`, so the
/// walk includes `format!("{world:?}")`, the good-faith diagnostic route the
/// structural seal exists for.
///
/// `claimant()`'s one bit is the spec's own walk-back affordance and is exempt: it
/// returns an `ActorId` the caller already has, never the payload. Later
/// milestones extend this same test with `JournalEntry`, `WardHeatRow` and
/// `PollenCensus::topic_lines()`.
#[test]
fn fact_source_reaches_no_projection() {
    const SENTINELS: [&str; 2] = ["zzsentinel", "zzcustodian"];
    const SENTINEL_PACK: &str = r#"{"schema_version": 1, "facts": [
        {"id": "test.quest.row", "topic": "omen", "said": "the bell was rung twice",
         "seeded": ["sv3n1"],
         "source": {"quest_phase": {"quest": "zzsentinel", "phase": 1}}},
        {"id": "test.held.row", "topic": "law", "said": "somebody is held",
         "seeded": ["cb947"], "source": {"custody": "zzcustodian"}}]}"#;

    let mut engine = engine_with_config(EngineConfig {
        fact_packs: vec![SENTINEL_PACK.to_string()],
        ..EngineConfig::default()
    });
    assert_eq!(
        engine.world().knowledge.len(),
        2,
        "both sentinel rows seeded"
    );

    // The world and the catalog it carries, through `Debug` — the authored
    // spelling lives on `World` for the whole run, so it is sealed like the
    // resolved one.
    let world_debug = format!("{:?}", engine.world());
    let catalog_debug = format!("{:?}", engine.world().fact_catalog);
    assert!(
        catalog_debug.contains("FactSourceSpec(<sealed>)"),
        "the catalog's rows print the placeholder: {catalog_debug}"
    );
    for sentinel in SENTINELS {
        assert!(
            !world_debug.contains(sentinel),
            "{sentinel} reached the world's Debug string"
        );
        assert!(
            !catalog_debug.contains(sentinel),
            "{sentinel} reached the catalog's Debug string"
        );
    }

    // Every roster actor's rendered prompt.
    let env = prompt_env();
    let roster = engine.world().roster.clone();
    for id in &roster {
        if engine.world().characters[id].control() == cathedral_sim::Control::Player {
            continue;
        }
        let prompt = render_prompt(engine.world(), id, None, &env).expect("a sheet renders");
        for sentinel in SENTINELS {
            assert!(
                !prompt.contains(sentinel),
                "{sentinel} reached {id}'s sheet"
            );
        }
    }

    // The cold public snapshot.
    let snapshot = serde_json::to_string(&engine.world().public_snapshot(&actor("player")))
        .expect("the snapshot serialises");
    for sentinel in SENTINELS {
        assert!(
            !snapshot.contains(sentinel),
            "{sentinel} reached the public snapshot"
        );
    }

    // The `--trace-knowledge` tracer, which renders the same sentence a sheet
    // does and is the one projection that reads the store deliberately.
    let traced = engine.knowledge_lines();
    assert!(
        traced.len() >= 3,
        "the tracer should carry a header and both holders: {traced:?}"
    );
    for line in &traced {
        for sentinel in SENTINELS {
            assert!(
                !line.contains(sentinel),
                "{sentinel} reached a knowledge_lines row: {line}"
            );
        }
    }

    // Ten polls' worth of every hot channel, through `Debug` — the route a
    // projection-walking test cannot see, and the reason the seal is structural.
    let mut seen = 0usize;
    for step in 0..10 {
        for message in engine.poll(f64::from(step) * 0.05, Vec::new()) {
            let rendered = format!("{message:?}");
            for sentinel in SENTINELS {
                assert!(
                    !rendered.contains(sentinel),
                    "{sentinel} reached an EngineMessage: {rendered}"
                );
            }
            seen += 1;
        }
    }
    assert!(seen > 0, "ten polls produced no messages at all");

    // And the one bit that *is* exempt still works, so the walk-back affordance is
    // not accidentally sealed away with the payload.
    let claimed = cathedral_sim::knowledge::FactSource::claimed(actor("sv3n1"));
    assert_eq!(claimed.claimant(), Some(&actor("sv3n1")));
    assert_eq!(format!("{claimed:?}"), "FactSource(<sealed>)");
}

/// An item-sourced fact binds to whoever is holding the item **now**: an item has
/// no position in this sim, so "the item moved" means "it changed hands".
#[test]
fn an_item_source_binds_to_the_current_holder() {
    let mut world = seed_world();
    let holder = world
        .characters
        .values()
        .find(|character| !character.holds().is_empty())
        .map(|character| (character.id().clone(), character.holds()[0].clone()));
    let Some((holder, item)) = holder else {
        // The demo seed always hands somebody something; if it stops, say so here
        // rather than passing vacuously.
        panic!("the demo world has no held item to bind a fact to");
    };

    let diagnostics = seed_pack(
        &mut world,
        &format!(
            r#"{{"id": "test.item.row", "topic": "coin", "said": "the thing is still about",
                 "seeded": ["{holder}"], "source": {{"item": "{item}"}}}}"#
        ),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = FactId::from_raw("test.item.row");
    assert!(knowledge::holds(&world, &holder, &id).is_some());
    // While it is in those hands the world bears it out.
    assert!(knowledge::invalidate_stale(&mut world).is_empty());

    // Take it out of their hands and the next sweep drops the fact.
    world
        .characters
        .get_mut(&holder)
        .expect("the holder")
        .state
        .holds
        .clear();
    assert_eq!(knowledge::invalidate_stale(&mut world), vec![id.clone()]);
    assert!(knowledge::holds(&world, &holder, &id).is_none());

    // An item nobody holds leaves the source unbound, with a diagnostic that
    // names the fact and not the item: the item's id is the binding a source
    // is made of, and this line rides into `logs.jsonl`.
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.item.loose", "topic": "coin", "said": "a loose thing",
            "seeded": ["sv3n1"], "source": {"item": "nosuchitem"}}"#,
    );
    assert_eq!(
        diagnostics,
        vec![
            "fact test.item.loose: its item is in nobody's hands; source left unbound".to_string()
        ]
    );
    assert!(
        !diagnostics.iter().any(|line| line.contains("nosuchitem")),
        "the item id is a provenance binding and must not reach a log line"
    );
    let spec_kinds: BTreeSet<bool> = FactCatalog::default()
        .specs()
        .iter()
        .map(cathedral_sim::knowledge::FactSpec::is_authored)
        .collect();
    assert_eq!(
        spec_kinds,
        BTreeSet::from([true]),
        "both shipped rows are authored"
    );
}

// ---------------------------------------------------------------------------
// The `what_you_know` block (M1 step 13). These render the real sheet through
// the shipped `turn.j2` and `strings.toml`, so they pin the layout M0b measured
// and the twenty-one frozen cells as the model actually receives them.
// ---------------------------------------------------------------------------

/// A profile is what makes `occupation_display` and `planning_ward` real, which
/// is the whole of the unknown-people branch. `curiosity_of` reads
/// `CURIOSITY_WITHOUT_LORE` for a profile-less body — irrelevant here, but a
/// profile is what M1 renders.
fn profile(display: Option<&str>, ward: PlanningWard) -> LoreProfile {
    LoreProfile {
        significance: Significance::Minor,
        planning_ward: ward,
        age: 30,
        gender: "f".into(),
        occupation_id: display.map(|_| "market_seller".to_string()),
        occupation_display: display.map(str::to_string),
        title: None,
        rank: None,
        faction_role: None,
        illegal_activity: None,
        district: "Wick".into(),
        father: None,
        mother: None,
        children: Vec::new(),
        circumstances: Vec::new(),
        conditions: Vec::new(),
        home: None,
        home_point_m: None,
        core_character_description: String::new(),
        extended_character_description: String::new(),
        curiosity: None,
        generated: false,
    }
}

fn character(id: &str, name: &str, lore: Option<LoreProfile>, knows: &[&str]) -> Character {
    Character::from_sheet(CharacterSheet {
        pockets: Vec::new(),
        frontbutt: None,
        id: actor(id),
        name: name.to_string(),
        control: Control::Llm,
        back_story: String::new(),
        location_description: String::new(),
        appearance: Default::default(),
        voice_key: None,
        // Far apart, so nobody lands in anybody's `you_see` and the sheets stay
        // about the block.
        position_m: Vec3::new(1000.0 * (id.len() as f64), WALK_Y, 0.0),
        facing_yaw: 0.0,
        holds: Vec::new(),
        goal: "None".into(),
        memories: Vec::new(),
        knows: knows.iter().map(|id| actor(id)).collect(),
        lore,
        presence: cathedral_sim::Presence::InCity,
        presence_epoch: 0,
        economic_class: cathedral_sim::EconomicClass::Resident,
    })
}

/// A world with the real area map and a clock at day 0.
///
/// Both halves are load-bearing: without the map `{place}` renders
/// `place_unknown`, and without the clock nothing ever cools and `{day}` renders
/// `a long while back` — correct behaviour, and the wrong fixture for a ladder.
fn block_world() -> World {
    let mut world = World::new();
    world.area_map = areas();
    world.current_time = Some(WorldTime {
        day: 0,
        fraction: 0.0,
        office: Office::Dayspring,
        weekday: Weekday::Bellday,
    });
    world
}

/// The `what_you_know` bullets on one actor's real sheet, with `since` standing
/// in for their inbox (`render_prompt`'s own override, which is what the engine
/// passes on a live turn).
fn bullets(world: &World, id: &str, since: &[&str], env: &PromptEnv) -> Option<Vec<String>> {
    let since: Vec<String> = since.iter().map(|line| (*line).to_string()).collect();
    let prompt = render_prompt(world, &actor(id), Some(&since), env).expect("the sheet renders");
    known_bullets(&prompt)
}

/// Hand one non-seeded reader a carried holding of `key`.
fn tell(world: &mut World, reader: &str, key: cathedral_sim::FactKey, hops: u8, heat: f32) {
    let learned = knowledge::learn(
        world,
        &actor(reader),
        key,
        Telling {
            hops,
            from: None,
            heat,
            view: Default::default(),
        },
        Some(0.0),
    );
    assert_eq!(
        learned,
        knowledge::Learned::Fresh,
        "{reader} was told something they already had"
    );
}

/// T20. A world with no facts in it renders no block at all — the zero-byte
/// claim the golden bless rests on — and still carries the ignorance rule, which
/// is unconditional because the sheets it has to work on are exactly these.
#[test]
fn a_factless_world_renders_no_block_and_still_carries_the_rule() {
    let world = seed_world();
    let env = prompt_env();

    assert!(world.knowledge.is_empty());
    assert!(
        prompt_support::sheet(&world, "sv3n1", &env)
            .get("what_you_know")
            .is_none(),
        "an empty block must be omitted from the sheet, not serialized as []"
    );

    let prompt = render_prompt(&world, &actor("sv3n1"), None, &env).expect("the sheet renders");
    assert!(
        !prompt.contains("**what_you_know**"),
        "the block rendered on a factless sheet"
    );
    assert!(known_bullets(&prompt).is_none());
    // The rule itself, on the sheet that has no block — the go/no-go case.
    // Compacted, because the frozen paragraph is wrapped at turn.j2's own prose
    // band and the first clause straddles a line break.
    let rule = prompt_support::compact(&prompt);
    assert!(rule.contains("that empty place is itself an answer"));
    assert!(rule.contains("do one of these, not none of them"));
}

/// T22. Three holders of one shipped fact say three different things, and a
/// fourth — who has been told the subject's name — says the name.
///
/// Two of the three speak their own first-person line; the third has never been
/// told who the subject is and says the trade and the ward instead. `own` and
/// `said` are separate for exactly this, and the unknown-people rule is the same
/// assertion.
///
/// The subject's own line is seated by **relevance**, not heat: `may_carry` is
/// false for the subject, so `volunteers` is false, so nobody ever volunteers
/// news about themselves unasked. Being asked is what puts it on their sheet —
/// which is the same route a `decays: false` standing fact takes.
#[test]
fn three_holders_of_one_fact_say_three_different_things() {
    let mut world = block_world();
    world.add_character(character(
        "dv8ll",
        "Osanne Vell",
        Some(profile(Some("Chandler"), PlanningWard::Wick)),
        &[],
    ));
    world.add_character(character("dclsk", "Clemence Skep", None, &[]));
    world.add_character(character("p000x", "Petronel Clove", None, &[]));
    world.add_character(character("knwr1", "Jonet Kett", None, &["dv8ll"]));

    // The shipped catalog, not an inline row — this is the asset's own proof.
    // `ashe.salt.short`'s cast is absent, so that row reports itself and is
    // skipped; the row under test must report nothing at all.
    let diagnostics = FactCatalog::default().seed(&mut world);
    let id = FactId::from_raw("vell.stall.pitch");
    assert!(
        !diagnostics.iter().any(|line| line.contains("vell.stall")),
        "{diagnostics:?}"
    );
    let key = world
        .knowledge
        .key_of(&id)
        .expect("the shipped row installed");
    // A fourth holder at one remove, who has been told the subject's name.
    tell(&mut world, "knwr1", key, 1, 1.0);

    let env = prompt_env();
    // The subject is asked about it; the other three are not, and hold it warm.
    let subject = bullets(
        &world,
        "dv8ll",
        &["A stranger asked you about the corner pitch"],
        &env,
    )
    .expect("asked about it, the subject holds their own line");
    let witness = bullets(&world, "dclsk", &[], &env).expect("the witness holds it");
    let stranger = bullets(&world, "p000x", &[], &env).expect("the third holder holds it");
    let knower = bullets(&world, "knwr1", &[], &env).expect("the fourth holder holds it");
    assert_eq!(
        (subject.len(), witness.len(), stranger.len(), knower.len()),
        (1, 1, 1, 1)
    );

    assert_eq!(
        subject[0],
        "First hand, in your own words: The corner has been mine since the Great Rains \
         and I will not be moved off it"
    );
    assert_eq!(
        witness[0],
        "First hand, in your own words: I watched her set her trestles over another woman's \
         pitch, and nobody said a word"
    );
    assert_eq!(
        stranger[0],
        "You saw this yourself: a chandler of the Wick Ward (you don't know their name) took \
         the corner pitch at The Wickmarket that was not theirs to take, today"
    );
    assert!(
        !stranger[0].contains("Osanne Vell"),
        "a name the reader was never told leaked: {}",
        stranger[0]
    );
    assert_eq!(
        knower[0],
        "They say — and you have it from the one mouth only: Osanne Vell took the corner pitch \
         at The Wickmarket that was not theirs to take, today"
    );

    let said: BTreeSet<&String> = [&subject[0], &witness[0], &stranger[0], &knower[0]]
        .into_iter()
        .collect();
    assert_eq!(said.len(), 4, "four holders, four different sentences");
}

/// T23. A fact about you is never told back to you as news: `holds` is `Some`,
/// and the block is empty even when the ward is asking about it. With an `own`
/// line the block renders that line and nothing else — no narrator in front of a
/// witness's own first-person words.
#[test]
fn a_fact_about_you_is_never_told_to_you_as_news() {
    let env = prompt_env();
    let asked = ["A stranger said to you: \"what happened at the shambles?\""];
    let row = |own: &str| {
        format!(
            r#"{{"id": "test.about.you", "topic": "law",
                 "said": "{{subject}} was taken at {{place}}, {{day}}",
                 {own}
                 "subject": ["reader"], "seeded": ["reader"],
                 "place": "shambles", "day": 0}}"#
        )
    };

    let mut world = block_world();
    world.add_character(character("reader", "Ide Reader", None, &[]));
    let diagnostics = seed_pack(&mut world, &row(""));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        knowledge::holds(
            &world,
            &actor("reader"),
            &FactId::from_raw("test.about.you")
        )
        .is_some(),
        "they hold it — they were there"
    );
    assert_eq!(
        bullets(&world, "reader", &asked, &env),
        None,
        "a fact about the reader with no own line must not reach their sheet at all"
    );

    let mut world = block_world();
    world.add_character(character("reader", "Ide Reader", None, &[]));
    let diagnostics = seed_pack(
        &mut world,
        &row(r#""own": {"reader": "They took me up at the Shambles and I had done nothing"},"#),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        bullets(&world, "reader", &asked, &env).expect("their own line is theirs to hold"),
        vec![
            "First hand, in your own words: They took me up at the Shambles and I had done nothing"
                .to_string()
        ]
    );
}

/// The frozen cell for one `(band, rung)` pair, by field name — no band-shift can
/// creep back in behind a computed lookup.
fn cell(strings: &cathedral_sim::PromptStrings, band: &str, rung: &str) -> String {
    let value = match (band, rung) {
        ("default", "hops0_own") => &strings.know_hedge_default_hops0_own,
        ("default", "hops0") => &strings.know_hedge_default_hops0,
        ("default", "hops1") => &strings.know_hedge_default_hops1,
        ("default", "hops2") => &strings.know_hedge_default_hops2,
        ("default", "hops3") => &strings.know_hedge_default_hops3,
        ("default", "hops4") => &strings.know_hedge_default_hops4,
        ("default", "cold") => &strings.know_hedge_default_cold,
        ("top", "hops0_own") => &strings.know_hedge_top_hops0_own,
        ("top", "hops0") => &strings.know_hedge_top_hops0,
        ("top", "hops1") => &strings.know_hedge_top_hops1,
        ("top", "hops2") => &strings.know_hedge_top_hops2,
        ("top", "hops3") => &strings.know_hedge_top_hops3,
        ("top", "hops4") => &strings.know_hedge_top_hops4,
        ("top", "cold") => &strings.know_hedge_top_cold,
        ("low", "hops0_own") => &strings.know_hedge_low_hops0_own,
        ("low", "hops0") => &strings.know_hedge_low_hops0,
        ("low", "hops1") => &strings.know_hedge_low_hops1,
        ("low", "hops2") => &strings.know_hedge_low_hops2,
        ("low", "hops3") => &strings.know_hedge_low_hops3,
        ("low", "hops4") => &strings.know_hedge_low_hops4,
        ("low", "cold") => &strings.know_hedge_low_cold,
        other => panic!("no such cell {other:?}"),
    };
    value.clone()
}

/// The three bands, each with a topic that carries it and an id whose middle
/// segment is the word an asker would use.
const LADDER: [(&str, &str, &str); 3] = [
    ("top", "bed", "test.bedrow.told"),
    ("default", "law", "test.lawrow.told"),
    ("low", "craft", "test.craftrow.told"),
];

/// T24. Every one of the twenty-one cells renders, through the real sheet.
///
/// One reader per cell — a reader holds one fact at one rung, so the cap never
/// hides a cell — and every bullet is compared to its own `know_hedge_*` field by
/// name. The cold readers are seated by relevance (heat is what they lack), which
/// is also the only way a faded telling ever reaches a sheet.
#[test]
fn the_hedge_ladder_renders_all_twenty_one_cells() {
    const RUNGS: [&str; 7] = [
        "hops0_own",
        "hops0",
        "hops1",
        "hops2",
        "hops3",
        "hops4",
        "cold",
    ];

    let env = prompt_env();
    let mut world = block_world();
    // Every reader knows the subject, so `{subject}` is the same three words in
    // all twenty-one bullets and the only thing under test is the hedge.
    for (band, _, _) in LADDER {
        world.add_character(character(&format!("{band}subj"), "Osanne Vell", None, &[]));
        for rung in RUNGS {
            world.add_character(character(
                &format!("{band}{rung}"),
                "Ide Reader",
                None,
                &[&format!("{band}subj")],
            ));
        }
    }
    for (band, topic, id) in LADDER {
        let diagnostics = seed_pack(
            &mut world,
            &format!(
                r#"{{"id": "{id}", "topic": "{topic}",
                     "said": "{{subject}} was seen at {{place}}, {{day}}",
                     "own": {{"{band}hops0_own": "I was there, and it was not as they tell it"}},
                     "subject": ["{band}subj"], "seeded": ["{band}subj", "{band}hops0_own"],
                     "place": "wickmarket", "day": 0}}"#
            ),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let key = world
            .knowledge
            .key_of(&FactId::from_raw(id))
            .expect("the row installed");
        for hops in 0..=4u8 {
            tell(&mut world, &format!("{band}hops{hops}"), key, hops, 1.0);
        }
        // Cold at hops 0: the faded register outranks every hop count, own line
        // excepted. 0.05 × the highest base (1.00) is under VOLUNTEER_HEAT.
        tell(&mut world, &format!("{band}cold"), key, 0, 0.05);
    }

    let strings = env.strings();
    for (band, _, id) in LADDER {
        let sentence = "Osanne Vell was seen at The Wickmarket, today";
        for rung in RUNGS {
            let reader = format!("{band}{rung}");
            // The cold reader is asked about it; everybody else is warm enough to
            // say it unasked.
            let since = if rung == "cold" {
                vec![format!(
                    "A stranger asked you about the {}",
                    id.split('.').nth(1).unwrap()
                )]
            } else {
                Vec::new()
            };
            let since: Vec<&str> = since.iter().map(String::as_str).collect();
            let rendered = bullets(&world, &reader, &since, &env)
                .unwrap_or_else(|| panic!("{reader} holds nothing on their sheet"));
            let telling = if rung == "hops0_own" {
                "I was there, and it was not as they tell it"
            } else {
                sentence
            };
            assert_eq!(
                rendered,
                vec![cell(strings, band, rung).replacen("%s", telling, 1)],
                "the {band}/{rung} cell"
            );
        }
    }
}

/// T25. Cold is not forgotten: a fact cooled below the gate leaves the sheet,
/// stays in the store, and comes back in the faded register when somebody asks —
/// and the own line outranks even that.
#[test]
fn a_cold_fact_leaves_the_sheet_stays_in_the_store_and_relevance_seats_it_faded() {
    let env = prompt_env();
    let strings = env.strings();
    let mut world = block_world();
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    world.add_character(character("carrer", "Ide Reader", None, &["subjct"]));
    world.add_character(character("seedcl", "Jonet Kett", None, &["subjct"]));
    world.add_character(character("ownclr", "Clemence Skep", None, &["subjct"]));
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.craftrow.told", "topic": "craft",
            "said": "{subject} was seen at {place}, {day}",
            "own": {"ownclr": "I was there, and it was not as they tell it"},
            "subject": ["subjct"], "seeded": ["subjct", "seedcl", "ownclr"],
            "place": "wickmarket", "day": 0}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = FactId::from_raw("test.craftrow.told");
    let key = world.knowledge.key_of(&id).expect("the row installed");
    tell(&mut world, "carrer", key, 2, 0.05);

    // Still held — cold is a register, not a deletion.
    assert!(knowledge::holds(&world, &actor("carrer"), &id).is_some());
    assert_eq!(bullets(&world, "carrer", &[], &env), None);

    let asked = ["A stranger asked you about the craftrow"];
    assert_eq!(
        bullets(&world, "carrer", &asked, &env).expect("asked, it comes back"),
        vec![cell(strings, "low", "cold").replacen(
            "%s",
            "Osanne Vell was seen at The Wickmarket, today",
            1
        )]
    );

    // The precedence rule, both halves: cold beats hops 0 …
    assert_eq!(
        bullets(&world, "seedcl", &[], &env).expect("a seeded holder is warm at 1.0"),
        vec![cell(strings, "low", "hops0").replacen(
            "%s",
            "Osanne Vell was seen at The Wickmarket, today",
            1
        )]
    );
    // … and the own line beats cold.
    assert_eq!(
        bullets(&world, "ownclr", &[], &env).expect("an own holder is warm at 1.0"),
        vec![cell(strings, "low", "hops0_own").replacen(
            "%s",
            "I was there, and it was not as they tell it",
            1
        )]
    );
}

/// A holding cools on the clock and not on a pass: learned warm on day 0 it is
/// on the sheet, three game days later it is off it with nothing having run —
/// seventy-two game hours is six half-lives, 1.0 → 0.016, under the gate on
/// the highest band — and still held, and it comes back faded when asked. The
/// spec's "cooled below `VOLUNTEER_HEAT`" through the sheet itself.
#[test]
fn a_holding_cools_on_the_clock_and_leaves_the_sheet() {
    let env = prompt_env();
    let strings = env.strings();
    let mut world = block_world();
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    world.add_character(character("readr1", "Ide Reader", None, &["subjct"]));
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.bedrow.told", "topic": "bed",
            "said": "{subject} was seen at {place}, {day}",
            "subject": ["subjct"], "seeded": ["subjct"], "place": "wickmarket", "day": 0}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = FactId::from_raw("test.bedrow.told");
    let key = world.knowledge.key_of(&id).expect("the row installed");
    tell(&mut world, "readr1", key, 1, 1.0);
    assert_eq!(
        bullets(&world, "readr1", &[], &env).expect("warm, it is said unasked"),
        vec![cell(strings, "top", "hops1").replacen(
            "%s",
            "Osanne Vell was seen at The Wickmarket, today",
            1
        )]
    );

    world.current_time = Some(WorldTime {
        day: 3,
        fraction: 0.0,
        office: Office::Dayspring,
        weekday: Weekday::Bellday,
    });
    assert_eq!(
        bullets(&world, "readr1", &[], &env),
        None,
        "three game days on, nothing ran and it is off the sheet"
    );
    let held = knowledge::holds(&world, &actor("readr1"), &id).expect("still held");
    assert!(
        held.heat(Some(3.0)) < cathedral_sim::knowledge::VOLUNTEER_HEAT,
        "cooled below the gate: {}",
        held.heat(Some(3.0))
    );
    assert_eq!(
        bullets(
            &world,
            "readr1",
            &["A stranger asked you about the bedrow"],
            &env
        )
        .expect("asked, it comes back faded"),
        vec![cell(strings, "top", "cold").replacen(
            "%s",
            "Osanne Vell was seen at The Wickmarket, three days past",
            1
        )]
    );
}

/// T26. Asking about the thing standing beside it seats the fact — M0's finding
/// 1, the adjacent-ask dead end, moved out of prose and into relevance selection.
///
/// The question names a salt *contract*; the fact is about short measure and
/// names no contract at all. What connects them is the fact's own id, whose
/// segments are the words an asker will use.
#[test]
fn asking_about_the_thing_beside_it_seats_the_fact() {
    let env = prompt_env();
    let mut world = block_world();
    world.add_character(character("fg2sh", "Grigor Ashe", None, &[]));
    world.add_character(character("e9nan", "Nan", None, &["fg2sh"]));
    world.add_character(character("readr1", "Ide Reader", None, &["fg2sh"]));
    let diagnostics = FactCatalog::default().seed(&mut world);
    assert!(
        !diagnostics.iter().any(|line| line.contains("ashe.salt")),
        "{diagnostics:?}"
    );
    let key = world
        .knowledge
        .key_of(&FactId::from_raw("ashe.salt.short"))
        .expect("the shipped row installed");
    // Cooled below the gate first: `coin` at heat 1.0 would be seated by heat
    // alone and the test would pass without relevance existing at all.
    tell(&mut world, "readr1", key, 2, 0.05);

    assert_eq!(
        bullets(
            &world,
            "readr1",
            &["A stranger said to you: \"fine weather\""],
            &env
        ),
        None,
        "an unrelated question seats nothing"
    );
    let asked =
        ["A stranger (id player) said to you: \"Which of you holds the salt contract now?\""];
    let seated = bullets(&world, "readr1", &asked, &env).expect("the adjacent ask seats it");
    assert_eq!(seated.len(), 1);
    assert!(
        seated[0].contains("sold salt short of the measure"),
        "{}",
        seated[0]
    );
}

/// T27. Relevance outranks heat, the block is capped, and the order is total —
/// the same world rendered twice gives byte-identical bullets.
#[test]
fn relevance_outranks_heat_and_the_order_is_total() {
    let env = prompt_env();
    let mut world = block_world();
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    world.add_character(character("readr1", "Ide Reader", None, &["subjct"]));
    // One cold and relevant, three hot and irrelevant. All `bed`, so the band is
    // constant and only the seating is under test.
    for name in ["coldone", "hotalpha", "hotbeta", "hotgamma"] {
        let diagnostics = seed_pack(
            &mut world,
            &format!(
                r#"{{"id": "test.{name}.told", "topic": "bed",
                     "said": "{{subject}} was seen at {{place}}, {{day}} ({name})",
                     "subject": ["subjct"], "seeded": ["subjct"],
                     "place": "wickmarket", "day": 0}}"#
            ),
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let key = world
            .knowledge
            .key_of(&FactId::from_raw(format!("test.{name}.told")))
            .expect("the row installed");
        let heat = if name == "coldone" { 0.05 } else { 1.0 };
        tell(&mut world, "readr1", key, 1, heat);
    }

    let asked = ["A stranger asked you about the coldone"];
    let seated = bullets(&world, "readr1", &asked, &env).expect("four holdings, three seats");
    assert_eq!(
        seated.len(),
        cathedral_sim::knowledge::KNOWN_SHEET_MAX,
        "the block is capped"
    );
    assert!(
        seated[0].contains("(coldone)"),
        "the cold relevant holding must take the first seat: {seated:?}"
    );
    assert_eq!(
        seated,
        bullets(&world, "readr1", &asked, &env).expect("it renders twice"),
        "the order is total, so a replay is byte-identical"
    );
}

/// T28. With the layer off the block is absent and the store is untouched — the
/// gate is on the reader, not on what the city knows.
#[test]
fn the_block_is_omitted_when_the_layer_is_off() {
    let env = prompt_env();
    let mut world = block_world();
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    world.add_character(character("readr1", "Ide Reader", None, &["subjct"]));
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.bedrow.told", "topic": "bed", "said": "{subject} was seen at {place}, {day}",
            "subject": ["subjct"], "seeded": ["subjct", "readr1"], "place": "wickmarket", "day": 0},
           {"id": "test.lawrow.told", "topic": "law", "said": "somebody was taken up",
            "seeded": ["readr1"]}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(bullets(&world, "readr1", &[], &env).is_some());

    world.knowledge_enabled = false;
    assert_eq!(bullets(&world, "readr1", &[], &env), None);
    assert_eq!(
        knowledge::holdings_of(&world, &actor("readr1")).len(),
        2,
        "the store is unchanged; only the reader is gated"
    );
}

/// No string this feature adds may name its own machinery.
///
/// M3, M4 and M5 extend the caller, not this helper: M3 with `known_from`, M4
/// with the three `raise_word` error messages and the fence comment **minus its
/// `topics:` tail** (the one carve-out), M5 with the refusal percept and the
/// hearsay clause.
///
/// Deliberately scoped to what this feature adds: `turn.j2` already says "fact"
/// and "topic" in prose that predates it, and `strings.toml` has
/// `faction_role_label`. Those are English, not mechanism words; do not chase
/// them.
fn assert_no_mechanism_words(texts: &[&str]) {
    const FORBIDDEN: [&str; 11] = [
        "fact",
        "hop",
        "heat",
        "band",
        "salience",
        "topic",
        "rumour",
        "rumor",
        "store",
        "factkey",
        "factsource",
    ];
    for text in texts {
        let lowered = text.to_lowercase();
        for word in FORBIDDEN {
            assert!(
                !lowered.contains(word),
                "the word '{word}' reached a model: {text}"
            );
        }
    }
}

/// T30. The mechanism is invisible: no string this feature puts in front of a
/// model names a fact, a hop, heat, a band or salience, and no rendered bullet
/// carries an id, a hop count, or a digit at all — the day phrase is in words,
/// because a number in front of a model is a number it says back.
#[test]
fn no_string_this_feature_adds_says_fact_hop_heat_or_salience() {
    let env = prompt_env();
    let strings = env.strings();
    let mut authored: Vec<&str> = vec![
        strings.know_note.as_str(),
        strings.know_discipline.as_str(),
        strings.unknown_person_role.as_str(),
        strings.day_today.as_str(),
        strings.day_yesterday.as_str(),
        strings.day_days_past.as_str(),
        strings.day_long_ago.as_str(),
        strings.place_unknown.as_str(),
    ];
    for (band, _, _) in LADDER {
        for rung in [
            "hops0_own",
            "hops0",
            "hops1",
            "hops2",
            "hops3",
            "hops4",
            "cold",
        ] {
            // By value here, so the assertion reads the same bytes `hedge_of`
            // hands the renderer.
            let owned = cell(strings, band, rung);
            assert_no_mechanism_words(&[owned.as_str()]);
        }
    }

    // The unconditional `turn.j2` paragraph, exactly the bytes it ships as.
    let turn = prompt_support::asset("prompts/turn.j2");
    let start = turn
        .find("what_you_know is the whole")
        .expect("the ignorance rule is in turn.j2");
    let end = turn[start..]
        .find("speak of those as freely as ever.")
        .expect("the rule's last clause");
    let paragraph = &turn[start..start + end + "speak of those as freely as ever.".len()];
    assert_eq!(
        paragraph.lines().count(),
        22,
        "the frozen paragraph is 22 lines"
    );
    authored.push(paragraph);
    assert_no_mechanism_words(&authored);

    // And every rendered bullet, on the shipped asset's own holders.
    let mut world = block_world();
    world.add_character(character(
        "dv8ll",
        "Osanne Vell",
        Some(profile(Some("Chandler"), PlanningWard::Wick)),
        &[],
    ));
    world.add_character(character("dclsk", "Clemence Skep", None, &[]));
    world.add_character(character("p000x", "Petronel Clove", None, &[]));
    let _ = FactCatalog::default().seed(&mut world);
    let id = FactId::from_raw("vell.stall.pitch");
    let key = world
        .knowledge
        .key_of(&id)
        .expect("the shipped row installed");
    let held = knowledge::holds(&world, &actor("dclsk"), &id).expect("a holder");

    for reader in ["dclsk", "p000x"] {
        let rendered = bullets(&world, reader, &[], &env).expect("a holder's block");
        for bullet in &rendered {
            assert_no_mechanism_words(&[bullet.as_str()]);
            assert!(
                !bullet.contains(id.as_str()),
                "the fact's id reached a sheet: {bullet}"
            );
            assert!(
                !bullet.contains(&held.hops.to_string()),
                "a hop count reached a sheet: {bullet}"
            );
            assert!(
                !bullet.chars().any(|character| character.is_ascii_digit()),
                "a digit reached a sheet: {bullet}"
            );
        }
    }
    // The handle exists and is never rendered — the two halves of the same claim.
    assert!(world.knowledge.fact(key).is_some());
}

// ---------------------------------------------------------------------------
// The engine seam (M1 steps 15-16): the packs a quest plants, the ablation
// switch on the writer side, the tracer, and the offline mouth.
// ---------------------------------------------------------------------------

/// One fact pack, seeded to the demo cast, as the `--facts` flag hands it over.
const DEMO_PACK: &str = r#"{"schema_version": 1, "facts": [
    {"id": "test.packrow.told", "topic": "law",
     "said": "the bell was rung twice over at {place}, {day}",
     "own": {"sv3n1": "I heard it myself and counted"},
     "seeded": ["sv3n1", "cb947"], "day": 0, "garble": "place,day"}
]}"#;

/// A quest plants its own rows through `EngineConfig::fact_packs` — JSON text,
/// because the sim never touches the filesystem — and `Engine::new` merges them
/// over the embedded catalog before the one seeding pass. A duplicate id is a
/// startup diagnostic and the pack is dropped: never a panic, and never a silent
/// overwrite of a shipped row.
#[test]
fn a_fact_pack_reaches_the_store_through_the_engine() {
    let engine = engine_with_config(EngineConfig {
        fact_packs: vec![DEMO_PACK.to_string()],
        ..EngineConfig::default()
    });
    let id = FactId::from_raw("test.packrow.told");
    assert_eq!(engine.world().knowledge.len(), 1, "the pack's one row");
    assert!(
        knowledge::holds(engine.world(), &actor("sv3n1"), &id).is_some(),
        "the pack's seeded holder holds it"
    );
    // The shipped rows name a cast the demo world does not have, so they report
    // themselves and are skipped — which is what keeps the frozen fixtures
    // byte-identical whatever the catalog grows to.
    assert!(
        engine
            .world()
            .knowledge
            .fact_by_id(&FactId::from_raw("vell.stall.pitch"))
            .is_none()
    );

    // The tracer shows the pack's own words, rendered per reader: the `own` line
    // for the one who has it, `said` for the one who does not.
    let traced = engine.knowledge_lines();
    assert_eq!(traced.len(), 3, "a header and two holders: {traced:?}");
    assert_eq!(traced[0], "[knowledge] 1 facts live, 2 holdings");
    assert!(
        traced
            .iter()
            .any(|line| line.contains("I heard it myself and counted")),
        "{traced:?}"
    );
    assert!(
        traced
            .iter()
            .any(|line| line.contains("the bell was rung twice over at")),
        "{traced:?}"
    );

    // The same pack twice: the second is refused by id, with a diagnostic that
    // names the row, and the store still holds exactly one.
    let mut engine = engine_with_config(EngineConfig {
        fact_packs: vec![DEMO_PACK.to_string(), DEMO_PACK.to_string()],
        ..EngineConfig::default()
    });
    assert_eq!(
        engine.world().knowledge.len(),
        1,
        "the duplicate is dropped"
    );
    let refusal = engine
        .poll(0.0, Vec::new())
        .into_iter()
        .filter_map(|message| match message {
            cathedral_sim::EngineMessage::Diagnostic(line) => Some(line),
            _ => None,
        })
        .find(|line| line.contains("a fact pack was refused"))
        .expect("the refusal rides out with the first poll");
    assert!(refusal.contains("test.packrow.told"), "{refusal}");
}

/// The ablation switch gates the **writer** as well as the reader (D49): with the
/// layer off, `Engine::new` seeds nothing at all, so a run with
/// `CATHEDRAL_NO_KNOWLEDGE` is a city with no knowledge layer rather than one
/// accumulating state nobody reads.
#[test]
fn the_engine_seeds_nothing_when_the_layer_is_off() {
    let engine = engine_with_config(EngineConfig {
        knowledge_enabled: false,
        fact_packs: vec![DEMO_PACK.to_string()],
        ..EngineConfig::default()
    });
    assert!(!engine.world().knowledge_enabled);
    assert!(engine.world().knowledge.is_empty(), "nothing was seeded");
    assert_eq!(
        engine.knowledge_lines(),
        vec!["[knowledge] 0 facts live, 0 holdings".to_string()],
        "the tracer reports an empty store rather than nothing at all"
    );
}

/// The offline mouth (step 16). A holder who is asked something says the thing
/// they hold, in the exact words the sheet gave them — read off the *rendered*
/// block, so a renderer that stops emitting it breaks the offline knowledge run
/// and this says so.
///
/// The deliberate omission is its mirror: a deterministic fake cannot show
/// directional ignorance (that is a prose property, measured 5/5 in
/// `m0_evidence/replies/round1/v2_structural/q3_nonholder_asked_*.txt`), and a
/// rule that fired on every question would change replies in existing
/// fake-driven tests. So a non-holder asked the same question still gets the
/// no-op, and T20 asserts the sheet they were asked it on carries the rule.
#[test]
fn the_offline_mouth_says_back_the_bullet_the_sheet_gave_it() {
    let mut world = block_world();
    world.add_character(character(
        "dv8ll",
        "Osanne Vell",
        Some(profile(Some("Chandler"), PlanningWard::Wick)),
        &[],
    ));
    world.add_character(character("p000x", "Petronel Clove", None, &[]));
    world.add_character(character("nohld", "Ide Ferrer", None, &[]));
    let _ = FactCatalog::default().seed(&mut world);

    let env = prompt_env();
    let asked = ["A stranger said to you: \"who took the corner pitch?\"".to_string()];
    let ask = |id: &str| {
        render_prompt(&world, &actor(id), Some(&asked), &env).expect("the sheet renders")
    };

    // The holder's own bullet comes back verbatim, JSON-escaped by the fake.
    let prompt = ask("p000x");
    let bullet = known_bullets(&prompt).expect("the holder has a block");
    assert_eq!(bullet.len(), 1);
    let reply = cathedral_sim::fake_reply(&prompt);
    assert!(
        reply.starts_with(r#"say {"target": "player", "text": "#),
        "{reply}"
    );
    assert!(reply.contains(&bullet[0]), "{reply}\nvs\n{}", bullet[0]);

    // The subject holds their own line and says that instead — the own/said
    // split reaching an actual mouth.
    let prompt = ask("dv8ll");
    let own = known_bullets(&prompt).expect("the subject was asked about it");
    assert!(
        cathedral_sim::fake_reply(&prompt).contains(&own[0]),
        "{}",
        own[0]
    );
    assert_ne!(own[0], bullet[0], "two holders, two sentences");

    // A non-holder asked the same question is untouched by the rule.
    let prompt = ask("nohld");
    assert!(
        known_bullets(&prompt).is_none(),
        "no block for a non-holder"
    );
    assert_eq!(
        cathedral_sim::fake_reply(&prompt),
        r#"set_goal {"goal": null}"#
    );
    assert!(
        prompt.contains("that empty"),
        "the ignorance rule is what answers for them instead"
    );
}

// ---------------------------------------------------------------------------
// M2 step 9 — the witness cools, and standing truth does not
// ---------------------------------------------------------------------------

/// Set the clock forward by `game_hours` from day 0, so a holding can be read at an
/// exact age.
fn at_game_hours(world: &mut World, game_hours: f64) {
    world.current_time = Some(WorldTime {
        day: 0,
        fraction: game_hours / 24.0,
        office: Office::Dayspring,
        weekday: Weekday::Bellday,
    });
}

/// The mechanism the whole slow end of the cadence band rests on: an off-affinity
/// `Craft` witness stops saying it inside ten game minutes.
///
/// `s = 0.20 × 0.60 = 0.12`, so their warm life is `12·log₂(0.12/0.119)` = 0.145
/// game hours — about one poll. Before M2 a seeded holder had no learning stamp at
/// all, sat at heat 1.0 for the whole run and deposited forever, and no choice of
/// constants could have fixed it; only the clock does. Observed through the sheet,
/// which is where the deposit gate is publicly visible: warm it is said unasked,
/// cold it is off the sheet and comes back only when somebody asks.
#[test]
fn a_craft_witness_stops_volunteering_inside_ten_game_minutes() {
    let env = prompt_env();
    let strings = env.strings();
    let mut world = block_world();
    // The subject is a cooper; the witness is not, so the ×0.6 other-trade row is
    // the one under test.
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    world.add_character(character(
        "witnss",
        "Jonet Kett",
        Some(profile(Some("Civic officer"), PlanningWard::Wick)),
        &["subjct"],
    ));
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "test.craftrow.batch", "topic": "craft",
            "said": "{subject} spoiled a batch at {place}, {day}",
            "subject": ["subjct"], "seeded": ["subjct", "witnss"],
            "place": "wickmarket", "day": 0}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = FactId::from_raw("test.craftrow.batch");
    let fact = world.knowledge.fact_by_id(&id).expect("installed");
    assert_eq!(
        fact.craft_ear.as_deref(),
        None,
        "the subject in this hermetic world has no lore, so every listener is the other trade"
    );
    assert_eq!(
        fact.minted_game_days,
        Some(0.0),
        "news is stamped at the mint"
    );

    let line = cell(strings, "low", "hops0").replacen(
        "%s",
        "Osanne Vell spoiled a batch at The Wickmarket, today",
        1,
    );
    at_game_hours(&mut world, 0.14);
    assert_eq!(
        bullets(&world, "witnss", &[], &env).expect("warm at 0.14 game hours"),
        vec![line]
    );
    at_game_hours(&mut world, 0.15);
    assert_eq!(
        bullets(&world, "witnss", &[], &env),
        None,
        "past 0.145 game hours a spoiled batch dies in its own lane"
    );
    // Cold is a register, not a deletion: asked, it comes back faded.
    assert!(
        bullets(
            &world,
            "witnss",
            &["A stranger asked you about the craftrow"],
            &env
        )
        .is_some_and(|lines| lines[0].starts_with(
            strings
                .know_hedge_low_cold
                .split("%s")
                .next()
                .expect("the cold cell has a prefix")
        )),
        "the fact is still held, in the cold register"
    );
    assert!(knowledge::holds(&world, &actor("witnss"), &id).is_some());
}

/// A standing fact is **answerable and never loud**: its seeded holders stay at heat
/// 1.0 forever, `1.00 × 0.15 > VOLUNTEER_HEAT` keeps them able to answer, nothing
/// puts it in a ward's air, and a carried row of it is stamped like news and cools.
#[test]
fn an_authored_standing_fact_never_cools_and_is_never_volunteered() {
    let mut world = block_world();
    world.add_character(character("subjct", "Renn Bale", None, &[]));
    world.add_character(character("holdr1", "Ide Holder", None, &["subjct"]));
    world.add_character(character("hearer", "Nan Hearer", None, &["subjct"]));
    let diagnostics = seed_pack(
        &mut world,
        r#"{"id": "bale.promise.standing", "topic": "talk",
            "said": "{subject} promised the ford would be mended",
            "subject": ["subjct"], "seeded": ["subjct", "holdr1"], "decays": false}"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let id = FactId::from_raw("bale.promise.standing");
    let key = world.knowledge.key_of(&id).expect("installed");
    assert_eq!(
        world.knowledge.fact(key).expect("live").minted_game_days,
        None,
        "standing truth carries no mint stamp, so nothing about it can age"
    );

    let holder = actor("holdr1");
    let held = knowledge::holds(&world, &holder, &id).expect("a seeded holder");
    assert_eq!(held.learned_on, None);
    for at in [Some(0.0), Some(1.0), Some(100.0), None] {
        assert_eq!(held.heat(at), 1.0, "a standing fact does not cool");
    }
    // The inequality that keeps the three holders answerable forever.
    assert!(1.00 * 0.15 > f64::from(cathedral_sim::knowledge::VOLUNTEER_HEAT));
    // And nothing put it in the air: it is asked for, never said unasked.
    assert_eq!(world.knowledge.air_entries(), 0);

    // A carried row of it is stamped like news — a standing fact reaches a
    // non-seeded head only through a re-heat, and from there it cools, so the word
    // stays in the ward it was asked in.
    let learned = knowledge::learn(
        &mut world,
        &actor("hearer"),
        key,
        Telling {
            hops: 1,
            from: Some(holder.clone()),
            heat: 0.1309,
            view: Default::default(),
        },
        Some(3.0),
    );
    assert_eq!(learned, knowledge::Learned::Fresh);
    let carried = knowledge::holds(&world, &actor("hearer"), &id).expect("a carried row");
    assert_eq!(carried.learned_on, Some(3.0));
    assert!(
        carried.heat(Some(4.0)) < carried.heat(Some(3.0)),
        "a carried row of standing truth cools like news"
    );
}

/// An authored `decays: true` row is news the world starts with, so it starts
/// cooling when the world starts — and a clock-less world stamps `None` on purpose,
/// which is the tolerance every golden fixture rests on.
#[test]
fn an_authored_decaying_fact_is_stamped_when_the_world_has_a_clock() {
    let row = r#"{"id": "test.bedrow.stamped", "topic": "bed",
                  "said": "{subject} was seen at {place}, {day}",
                  "subject": ["subjct"], "seeded": ["subjct", "witnss"], "day": 0}"#;

    let mut clocked = block_world();
    clocked.add_character(character("subjct", "Osanne Vell", None, &[]));
    clocked.add_character(character("witnss", "Jonet Kett", None, &["subjct"]));
    at_game_hours(&mut clocked, 6.0);
    assert!(seed_pack(&mut clocked, row).is_empty());
    let id = FactId::from_raw("test.bedrow.stamped");
    assert_eq!(
        clocked
            .knowledge
            .fact_by_id(&id)
            .expect("installed")
            .minted_game_days,
        Some(0.25),
        "the stamp is the clock the world was seeded on"
    );
    let held = knowledge::holds(&clocked, &actor("witnss"), &id).expect("a witness");
    assert_eq!(held.learned_on, Some(0.25));
    assert!(
        held.heat(Some(0.25 + 20.0 / 24.0)) < 1.0,
        "twenty game hours on, a witness of news has cooled"
    );

    let mut clockless = World::new();
    clockless.area_map = areas();
    clockless.add_character(character("subjct", "Osanne Vell", None, &[]));
    clockless.add_character(character("witnss", "Jonet Kett", None, &["subjct"]));
    assert!(clockless.current_time.is_none());
    assert!(seed_pack(&mut clockless, row).is_empty());
    assert_eq!(
        clockless
            .knowledge
            .fact_by_id(&id)
            .expect("installed")
            .minted_game_days,
        None
    );
    let held = knowledge::holds(&clockless, &actor("witnss"), &id).expect("a witness");
    assert_eq!(held.learned_on, None);
    assert_eq!(held.heat(Some(500.0)), 1.0, "no clock means nothing ages");
}

// ---------------------------------------------------------------------------
// The ward's air, from outside: the pass, the pickup, the deposit, and the two
// things that must stay true of both (M2 steps 13–16)
// ---------------------------------------------------------------------------

/// Two patches of real city paving in two different wards.
///
/// The ward grid is baked from the embedded `homes.json`, not from the world under
/// test, so a hermetic world's bodies stand in real wards wherever a test sets them
/// down — which is what lets these tests carry a word over a boundary with no nav
/// graph and no round.
const IN_FABRIC: Vec3 = Vec3::new(0.0, WALK_Y, 0.0);
const IN_WEIGH: Vec3 = Vec3::new(-300.0, WALK_Y, 100.0);

/// A world with a clock, one `bed` fact seeded to `witnss`, and `hearer` stood in
/// the Fabric ward beside the origin. Neither body has lore, so
/// `attention::curiosity_of` gives them `CURIOSITY_WITHOUT_LORE` and the pickup
/// chance clamps to certainty — which is exactly what a test of the *plumbing*
/// wants: the roll is pinned by `pollen.rs`'s own tests, and here it must not be
/// what decides whether the test passes.
fn air_world() -> (World, cathedral_sim::FactKey) {
    let mut world = block_world();
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    world.add_character(character("witnss", "Jonet Kett", None, &[]));
    world.add_character(character("hearer", "Tib Stott", None, &[]));
    let key = seed_pack(
        &mut world,
        r#"{"id": "test.air.row", "topic": "bed", "said": "{subject} was seen leaving",
            "subject": ["subjct"], "seeded": ["witnss"]}"#,
    );
    assert!(key.is_empty(), "{key:?}");
    let key = world
        .knowledge
        .key_of(&FactId::from_raw("test.air.row"))
        .expect("the row installed");
    for id in ["subjct", "witnss", "hearer"] {
        stand(&mut world, id, IN_FABRIC);
    }
    (world, key)
}

/// Put a body on a named patch of ground — `poll_person` reads the position and
/// nothing else to decide which ward's air it is standing in.
fn stand(world: &mut World, id: &str, at: Vec3) {
    world
        .characters
        .get_mut(&actor(id))
        .expect("the body is in the world")
        .state
        .position_m = at;
}

/// A pickup lands one hop past the air, takes the air's `via` as its chain link,
/// and is charged the hop loss **once** — in the stored heat, never in the chance.
///
/// Getting those two the wrong way round makes every warm life and every closed
/// form in `02_numbers.md` wrong, which is why the arithmetic is asserted here and
/// not inferred from a cadence run.
#[test]
fn a_pickup_lands_one_hop_past_the_air_and_charges_the_loss_once() {
    let (mut world, key) = air_world();
    let ward = world.ward_at(IN_FABRIC).expect("the origin is in a ward");
    assert_eq!(ward, PlanningWard::Fabric);
    assert!(
        world
            .knowledge
            .deposit(ward, key, 0, 1.0, &actor("witnss"), 0.0)
    );

    assert!(
        knowledge::holds(&world, &actor("hearer"), &FactId::from_raw("test.air.row")).is_none()
    );
    knowledge::pollen::poll_person(&mut world, &actor("hearer"), 0.0);

    let held = knowledge::holds(&world, &actor("hearer"), &FactId::from_raw("test.air.row"))
        .expect("the air was warm and the roll was certain");
    assert_eq!(held.hops, 1, "a pickup lands one hop past the air");
    assert_eq!(
        held.from,
        Some(actor("witnss")),
        "the air's own mouth is the chain link"
    );
    assert_eq!(
        held.heat(Some(0.0)),
        1.0 * cathedral_sim::knowledge::HOP_LOSS,
        "the loss is charged once, at the hop"
    );
    assert_eq!(held.learned_on, Some(0.0));
    assert!(held.view.is_pristine(), "M3 is what makes a view drift");
    // The air is where it was: a pickup reads it and does not touch it.
    let air = world.knowledge.drift(ward, key).expect("the row").clone();
    assert_eq!((air.hops, air.via), (0, Some(actor("witnss"))));
}

/// **A pickup is not a percept.** `Novelty::admits_idle` short-circuits to `true`
/// on a non-empty inbox, so one `notify_percept` per pickup would turn the pollen
/// field into a city-wide prompt firehose at `extra_ambient_npcs: 1000` — a
/// thousand people each holding an idle turn open because the ward is talking.
#[test]
fn a_pickup_is_not_a_percept() {
    let (mut world, key) = air_world();
    let ward = world.ward_at(IN_FABRIC).expect("a ward");
    world
        .knowledge
        .deposit(ward, key, 0, 1.0, &actor("witnss"), 0.0);
    let before = {
        let hearer = &world.characters[&actor("hearer")];
        (
            hearer.inbox().len(),
            hearer.pending_history().len(),
            hearer.recent_history().len(),
        )
    };
    knowledge::pollen::poll_person(&mut world, &actor("hearer"), 0.0);
    assert!(
        knowledge::holds(&world, &actor("hearer"), &FactId::from_raw("test.air.row")).is_some(),
        "the test is only worth anything if something was picked up"
    );
    let hearer = &world.characters[&actor("hearer")];
    assert_eq!(
        (
            hearer.inbox().len(),
            hearer.pending_history().len(),
            hearer.recent_history().len()
        ),
        before,
        "a pickup wrote to an inbox or a history — that is a prompt, not a rumour"
    );
}

/// Layer 1's deposit rule, through the pass that performs it: a carrier who is warm
/// enough to be saying a thing lowers the ward's hop count to their own and becomes
/// the mouth a later pickup records.
#[test]
fn a_deposit_lowers_the_hops_and_takes_the_via() {
    let (mut world, key) = air_world();
    let ward = world.ward_at(IN_FABRIC).expect("a ward");
    // Somebody four removes out got there first…
    world
        .knowledge
        .deposit(ward, key, 4, 0.20, &actor("subjct"), 0.0);
    // …and a hops-1 carrier, warm enough to volunteer, is standing in it.
    tell(&mut world, "hearer", key, 1, 0.85);
    knowledge::pollen::poll_person(&mut world, &actor("hearer"), 0.0);

    let air = world.knowledge.drift(ward, key).expect("the row").clone();
    assert_eq!(air.hops, 1, "the closest mouth sets the ward's hop count");
    assert_eq!(
        air.via,
        Some(actor("hearer")),
        "and becomes the chain link a pickup records"
    );
    assert_eq!(
        cathedral_sim::knowledge::heat_pct(air.heat),
        85,
        "heat takes the maximum of the air and the mouth"
    );
}

/// The fiction, asserted: **the word crosses the city in mouths.** One holder walks
/// from one ward to another and the second ward starts saying it; nobody there held
/// it before, and nobody there had to be told by the first ward's air.
#[test]
fn the_air_carries_a_word_across_a_ward_boundary_in_a_mouth() {
    let (mut world, key) = air_world();
    let home = world.ward_at(IN_FABRIC).expect("a ward");
    let abroad = world.ward_at(IN_WEIGH).expect("a ward");
    assert_ne!(home, abroad, "the two patches must be in different wards");
    assert_eq!(abroad, PlanningWard::Weigh);
    stand(&mut world, "hearer", IN_WEIGH);

    // Before the walk: the witness's own ward is saying it, and the far ward is not.
    knowledge::pollen::poll_person(&mut world, &actor("witnss"), 0.0);
    assert!(world.knowledge.drift(home, key).is_some());
    assert!(world.knowledge.drift(abroad, key).is_none());
    knowledge::pollen::poll_person(&mut world, &actor("hearer"), 0.0);
    assert!(
        knowledge::holds(&world, &actor("hearer"), &FactId::from_raw("test.air.row")).is_none(),
        "a ward away is a ward away: the air does not seep"
    );

    // The witness walks there, still warm, and says it.
    stand(&mut world, "witnss", IN_WEIGH);
    knowledge::pollen::poll_person(&mut world, &actor("witnss"), 0.0);
    let carried = world.knowledge.drift(abroad, key).expect("the far ward");
    assert_eq!(
        (carried.hops, carried.via.clone()),
        (0, Some(actor("witnss")))
    );

    // And now the body standing there catches it.
    knowledge::pollen::poll_person(&mut world, &actor("hearer"), 0.0);
    let held = knowledge::holds(&world, &actor("hearer"), &FactId::from_raw("test.air.row"))
        .expect("a mouth brought it");
    assert_eq!(held.hops, 1);
    assert_eq!(held.from, Some(actor("witnss")));
}

/// A world with no facts in it grows no air, however long it is polled and swept —
/// which is the `extra_ambient_npcs: 0` byte-for-byte claim, and the reason the
/// frozen fixtures could not have moved.
#[test]
fn a_factless_world_grows_no_air() {
    let mut world = block_world();
    world.add_character(character("hearer", "Tib Stott", None, &[]));
    stand(&mut world, "hearer", IN_FABRIC);
    assert!(world.knowledge.is_empty());
    let revision = world.world_revision;

    for step in 0..1_000 {
        let game_days = step as f64 / 48.0;
        knowledge::pollen::poll_person(&mut world, &actor("hearer"), game_days);
        knowledge::pollen::sweep(&mut world, game_days);
    }
    assert_eq!(world.knowledge.air_entries(), 0);
    assert!(world.knowledge.is_empty());
    assert_eq!(knowledge::holdings_of(&world, &actor("hearer")).len(), 0);
    assert_eq!(
        world.world_revision, revision,
        "the air is prompt state and must never touch the public revision"
    );
}

/// The ablation is a real ablation: with the layer off nothing mints, no ward says
/// anything, and a poll that would certainly have picked something up does nothing.
#[test]
fn knowledge_disabled_is_a_real_ablation() {
    let (mut world, key) = air_world();
    let ward = world.ward_at(IN_FABRIC).expect("a ward");
    world
        .knowledge
        .deposit(ward, key, 0, 1.0, &actor("witnss"), 0.0);
    world.knowledge_enabled = false;

    assert_eq!(
        knowledge::mint::mint(
            &mut world,
            FactId::from_raw("test.air.off"),
            knowledge::Topic::Law,
            "{subject} was taken in charge".into(),
            vec![actor("subjct")],
            IN_FABRIC,
            knowledge::GarbleMask::ALL,
            true,
            knowledge::FactSource::authored(),
            Some(0.0),
        ),
        None,
        "a mint with the layer off installs nothing"
    );
    knowledge::pollen::poll_person(&mut world, &actor("hearer"), 0.0);
    assert!(
        knowledge::holds(&world, &actor("hearer"), &FactId::from_raw("test.air.row")).is_none(),
        "poll_person must be a no-op with the layer off"
    );
    assert!(
        !knowledge::pollen::sweep(&mut world, 1.0),
        "and so must the sweep"
    );
}

/// Nothing knowledge-shaped reaches the carriage state: nine mints and five hundred
/// polls leave the public snapshot byte-identical and the world revision where it
/// was.
///
/// Facts are prompt state, like notices. The snapshot is already at 137 KB of its
/// 160 KiB bound, and a fact that reached it would also be a fact on the wire.
#[test]
fn nothing_knowledge_shaped_reaches_the_snapshot() {
    // The demo world, so the snapshot under test is the real thing — a live player,
    // the whole cast, positions, holdings — and not a three-body stub whose bytes
    // could be identical for want of anything in them.
    let mut world = seed_world();
    let player = actor("player");
    for id in ["sv3n1", "cb947"] {
        stand(&mut world, id, IN_FABRIC);
    }
    assert!(
        seed_pack(
            &mut world,
            r#"{"id": "test.snapshot.row", "topic": "bed", "said": "{subject} was seen leaving",
                "subject": ["k0fb1"], "seeded": ["sv3n1"]}"#,
        )
        .is_empty()
    );
    let key = world
        .knowledge
        .key_of(&FactId::from_raw("test.snapshot.row"))
        .expect("the row installed");
    let before = serde_json::to_vec(&world.public_snapshot(&player)).expect("it serialises");
    assert!(
        before.len() > 1_000,
        "a stub snapshot would make this test vacuous: {} bytes",
        before.len()
    );
    let revision = world.world_revision;

    for index in 0..9 {
        assert!(
            knowledge::mint::mint(
                &mut world,
                FactId::from_raw(format!("test.snapshot.mint.{index}")),
                knowledge::Topic::ALL[index],
                "{subject} did a thing at {place} {day}".into(),
                vec![actor("k0fb1")],
                IN_FABRIC,
                knowledge::GarbleMask::ALL,
                true,
                knowledge::FactSource::authored(),
                Some(0.0),
            )
            .is_some(),
            "mint {index} refused"
        );
    }
    let ward = world.ward_at(IN_FABRIC).expect("a ward");
    world
        .knowledge
        .deposit(ward, key, 0, 1.0, &actor("sv3n1"), 0.0);
    for step in 0..500 {
        let game_days = step as f64 / 48.0;
        knowledge::pollen::poll_person(&mut world, &actor("cb947"), game_days);
        knowledge::pollen::poll_person(&mut world, &actor("sv3n1"), game_days);
        knowledge::pollen::sweep(&mut world, game_days);
    }
    assert_eq!(world.knowledge.len(), 10);
    assert!(
        !knowledge::holdings_of(&world, &actor("cb947")).is_empty(),
        "five hundred polls must actually have moved something"
    );
    assert_eq!(
        serde_json::to_vec(&world.public_snapshot(&player)).expect("it serialises"),
        before,
        "something knowledge-shaped reached the public snapshot"
    );
    assert_eq!(
        world.world_revision, revision,
        "a mint or a pickup bumped the public revision"
    );
}

/// **The store is bounded at the crowd knob's ceiling.** 20,519 synthetic bodies
/// (the cast plus `--extra-ambient 20000`), every one of them at `HOLDINGS_MAX`,
/// against `02_numbers.md` §6's 32 MB.
///
/// A **test**, not a number in a markdown file — but `footprint_bytes()` is
/// `size_of` arithmetic and therefore asserts the formula against itself, which is
/// why M2 step 20e also records the peak-RSS delta of a saturated
/// `--extra-ambient 20000` run and checks the same bound against *that*.
/// `size_of::<Holding>() <= 88`, the row this arithmetic rests on, is pinned by
/// name in `knowledge/tests.rs` (`a_holding_stores_no_sentence`), which is the only
/// place that type is in scope.
///
/// It also times one `World::clone()` at that size, which is what
/// `World::market_sale` pays on **every** catalog sale: `holdings` and `air` are
/// behind `Arc` and `live` is not, so the number here is what settles whether
/// `live` needs one too (M1's note 14).
#[test]
fn the_store_footprint_is_bounded() {
    const BODIES: usize = 20_519;
    let mut world = World::new();
    world.add_character(character("subjct", "Osanne Vell", None, &[]));
    // Six facts, so every body can be filled to the cap and no arrival is ever its
    // own eviction victim.
    let mut keys = Vec::new();
    for index in 0..cathedral_sim::knowledge::HOLDINGS_MAX {
        assert!(
            seed_pack(
                &mut world,
                &format!(
                    r#"{{"id": "test.footprint.{index}", "topic": "bed",
                        "said": "{{subject}} was seen leaving",
                        "subject": ["subjct"], "seeded": ["subjct"]}}"#
                ),
            )
            .is_empty()
        );
        keys.push(
            world
                .knowledge
                .key_of(&FactId::from_raw(format!("test.footprint.{index}")))
                .expect("the row installed"),
        );
    }

    // Six-character ids, the generated crowd's own shape (`x00000`…), so the heap
    // an `ActorId` costs is the heap the real crowd costs.
    for index in 0..BODIES {
        let id = format!("x{index:05}");
        world.add_character(character(&id, &id, None, &[]));
        let who = actor(&id);
        for (hop, key) in keys.iter().enumerate() {
            knowledge::learn(
                &mut world,
                &who,
                *key,
                Telling {
                    hops: hop as u8 + 1,
                    from: Some(actor("subjct")),
                    heat: 0.9,
                    view: Default::default(),
                },
                Some(0.0),
            );
        }
    }

    for index in 0..BODIES {
        assert_eq!(
            world
                .knowledge
                .holdings_len(&actor(&format!("x{index:05}"))),
            cathedral_sim::knowledge::HOLDINGS_MAX,
            "body {index} is not at the cap, so this measures the wrong store"
        );
    }
    let bytes = world.knowledge.footprint_bytes();
    println!(
        "[pollen] store footprint at {BODIES} bodies × {} holdings: {bytes} bytes ({:.1} MB)",
        cathedral_sim::knowledge::HOLDINGS_MAX,
        bytes as f64 / (1024.0 * 1024.0),
    );
    assert!(
        bytes <= 32 * 1024 * 1024,
        "the store is {bytes} bytes at {BODIES} bodies, over 02_numbers.md §6's 32 MB bound"
    );

    // What the store itself adds to `World::clone()` — the only part of that clone
    // this feature is answerable for. `holdings` and `air` are behind `Arc`, so all
    // that is deep-copied is `live` and `by_id`.
    let started = std::time::Instant::now();
    let store = world.knowledge.clone();
    let store_clone = started.elapsed();
    let started = std::time::Instant::now();
    let whole = world.clone();
    let world_clone = started.elapsed();
    println!(
        "[pollen] Knowledge::clone() at {} live facts: {:.4} ms; the whole World::clone() \
         (20,520 characters): {:.3} ms — the store is {:.2}% of what a catalog sale pays",
        store.len(),
        store_clone.as_secs_f64() * 1000.0,
        world_clone.as_secs_f64() * 1000.0,
        store_clone.as_secs_f64() / world_clone.as_secs_f64() * 100.0,
    );
    assert_eq!(whole.knowledge.len(), world.knowledge.len());

    // And at the cap, which is where the number matters: `live` is deep-copied
    // per clone, linearly in the facts, and 6 rows say nothing about 256.
    for index in 0..(cathedral_sim::knowledge::FACTS_MAX_LIVE - world.knowledge.len()) {
        assert!(
            seed_pack(
                &mut world,
                &format!(
                    r#"{{"id": "test.footprint.fill.{index}", "topic": "talk",
                        "said": "{{subject}} said a filler thing",
                        "subject": ["subjct"], "seeded": ["subjct"]}}"#
                ),
            )
            .is_empty()
        );
    }
    assert_eq!(
        world.knowledge.len(),
        cathedral_sim::knowledge::FACTS_MAX_LIVE,
        "the store is at FACTS_MAX_LIVE"
    );
    let started = std::time::Instant::now();
    let store = world.knowledge.clone();
    let store_clone_full = started.elapsed();
    let started = std::time::Instant::now();
    let whole = world.clone();
    let world_clone_full = started.elapsed();
    println!(
        "[pollen] Knowledge::clone() at {} live facts: {:.4} ms; the whole World::clone() \
         (20,520 characters): {:.3} ms — the store is {:.2}% of what a catalog sale pays",
        store.len(),
        store_clone_full.as_secs_f64() * 1000.0,
        world_clone_full.as_secs_f64() * 1000.0,
        store_clone_full.as_secs_f64() / world_clone_full.as_secs_f64() * 100.0,
    );
    assert_eq!(whole.knowledge.len(), world.knowledge.len());
}
