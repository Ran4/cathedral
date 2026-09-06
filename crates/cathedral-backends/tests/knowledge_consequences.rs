//! M5 affinity claims against the shipped cast, without changing the cadence harness.
use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    WorldConfig, build_world,
    knowledge::{self, FactCatalog, Topic},
};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn the_shipped_ears_are_the_sizes_the_table_claims() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
    let world = build_world(&seed, WorldConfig::default());
    let mut counts = BTreeMap::<String, usize>::new();
    let mut no_trade = 0;
    for character in world
        .characters
        .values()
        .filter(|body| body.control() != cathedral_sim::Control::Player)
    {
        if let Some(occupation) = character.lore().and_then(|l| l.occupation_id.as_ref()) {
            *counts.entry(occupation.clone()).or_default() += 1;
        } else {
            no_trade += 1;
        }
    }
    assert_eq!(no_trade, 10);
    for (topic, expected) in [
        (Topic::Bed, 77),
        (Topic::Law, 63),
        (Topic::Coin, 42),
        (Topic::Bread, 42),
    ] {
        let (ears, _) = world.salience.ear_of(topic);
        assert_eq!(
            ears.iter()
                .map(|ear| counts.get(ear).copied().unwrap_or(0))
                .sum::<usize>(),
            expected,
            "{topic:?}"
        );
    }
    for (occupation, expected) in [
        ("domestic_servant", 45),
        ("tavern_worker", 9),
        ("sex_worker", 8),
        ("water_and_bath_worker", 8),
        ("laundress", 7),
    ] {
        assert_eq!(counts[occupation], expected, "{occupation}");
    }
    let occupations: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("lore/core_lore/occupations.json")).unwrap(),
    )
    .unwrap();
    let catalog = occupations.as_array().unwrap();
    assert_eq!(catalog.len(), 65);
    for topic in Topic::ALL {
        for ear in world.salience.ear_of(topic).0 {
            assert_ne!(ear, "no_fixed_trade");
            assert!(
                catalog
                    .iter()
                    .any(|row| row["occupation_id"].as_str() == Some(ear)),
                "missing ear {ear}"
            );
        }
    }
}

#[test]
fn shipped_affinity_player_and_household_precedence_match_the_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
    let mut world = build_world(&seed, WorldConfig::default());
    let subject = world
        .characters
        .iter()
        .find(|(_, c)| c.lore().and_then(|l| l.occupation_id.as_deref()) == Some("baker"))
        .unwrap()
        .0
        .clone();
    for topic in Topic::ALL {
        let doc = serde_json::json!({"schema_version":1,"facts":[{"id":"affinity.probe","topic":topic,"said":"a word about {subject}","subject":[subject.as_str()],"seeded":[subject.as_str()]}]});
        let key = FactCatalog::from_json(&doc.to_string())
            .unwrap()
            .seed_one(
                &mut world,
                &cathedral_sim::FactId::from_raw("affinity.probe"),
            )
            .unwrap();
        let fact = world.knowledge.fact(key).unwrap();
        for (id, body) in &world.characters {
            let occupation = body.lore().and_then(|l| l.occupation_id.as_deref());
            let multiplier = if body.control() == cathedral_sim::Control::Player {
                1.0
            } else if occupation.is_none() {
                world.salience.no_trade()
            } else if topic == Topic::Craft {
                if fact.craft_ear.as_deref() == occupation {
                    world.salience.craft_own()
                } else {
                    world.salience.craft_other()
                }
            } else {
                let (ears, multiplier) = world.salience.ear_of(topic);
                if ears.iter().any(|ear| Some(ear.as_str()) == occupation) {
                    multiplier
                } else {
                    1.0
                }
            };
            let damping = if fact.quiet_among.contains(id) {
                world.salience.household()
            } else {
                1.0
            };
            let expected = (world.salience.base(topic) * multiplier * damping).clamp(0.0, 4.0);
            assert_eq!(
                knowledge::salience::salience(&world, fact, id),
                expected,
                "{topic:?} {id}"
            );
        }
        world.knowledge.invalidate(key);
    }
}
