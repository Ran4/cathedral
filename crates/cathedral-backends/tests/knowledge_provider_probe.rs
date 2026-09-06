//! Reproducible, offline sheet preparation for the recorded live knowledge probes.
//! Run explicitly with `--ignored --nocapture`; this never calls a provider.

use std::{fs, path::PathBuf};

use cathedral_backends::world_data::load_world_seed;
use cathedral_sim::{
    ActorId, AreaMap, FactId, Office, PromptEnv, Weekday, World, WorldTime,
    knowledge::{self, FactCatalog, pollen::AreaAdjacency},
};
use serde_json::{Value, json};

#[test]
#[ignore = "writes the explicitly requested live-provider evidence sheets"]
fn prepare_m3_provider_sheets() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let feature = [
        "features/knowledge_and_rumor",
        "features/implemented/knowledge_and_rumor",
    ]
    .into_iter()
    .map(|path| root.join(path))
    .find(|path| path.is_dir())
    .expect("knowledge feature evidence");
    let evidence = feature.join("m0_evidence");
    let output = std::env::var_os("KNOWLEDGE_PROBE_OUT")
        .map(PathBuf::from)
        .expect("set KNOWLEDGE_PROBE_OUT to an output directory");
    fs::create_dir_all(&output).unwrap();
    let read = |path: &str| fs::read_to_string(root.join(path)).unwrap();
    let env = PromptEnv::new(
        &read("assets/prompts/turn.j2"),
        &read("assets/prompts/night.j2"),
        &read("assets/prompts/strings.toml"),
    )
    .unwrap();
    let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
    let actor = |name: &str| {
        seed.characters
            .iter()
            .find(|person| person.name == name)
            .unwrap_or_else(|| panic!("no authored character {name}"))
            .id
            .clone()
    };
    let mut scenarios: Vec<_> = fs::read_dir(evidence.join("scenarios"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            name.starts_with("q4_wick_")
                || name.starts_with("q2_holder_hops3_")
                || name.starts_with("q2_holder_hops4_")
        })
        .collect();
    scenarios.sort();
    let mut records = Vec::new();
    for path in scenarios {
        let scenario: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        let name = scenario["name"].as_str().unwrap();
        let row = &scenario["what_you_know"][0];
        let reader_name = scenario["sheet"]["you"]
            .as_str()
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        let reader = actor(reader_name);
        let subject = actor(row["subject"]["name"].as_str().unwrap());
        let hops = row["hops"].as_u64().unwrap() as u8;
        let q4 = name.starts_with("q4_");
        let mut world = World::new();
        world.area_map = AreaMap::from_json_str(&read("assets/world/areas.json")).unwrap();
        world.area_adjacency = AreaAdjacency::build(&world.area_map).into();
        world.current_time = Some(WorldTime {
            day: 2,
            fraction: 0.0,
            office: Office::Dayspring,
            weekday: Weekday::Bellday,
        });
        // Same authored roster and stable ids for every sheet. The scenario
        // explicitly says its holder knows the true subject; preserve that.
        for mut sheet in seed.characters.clone() {
            sheet.holds.clear();
            sheet.pockets.clear();
            if sheet.id == reader {
                sheet.knows.insert(subject.clone());
            }
            world.add_character(cathedral_sim::Character::from_sheet(sheet));
        }
        let said = if q4 {
            "{subject} was taken at {place}, {day}, and lies in the Stone House".to_string()
        } else {
            row["said"]
                .as_str()
                .unwrap()
                .replace("{subject}'s mill", "the mill of {subject}")
        };
        let witnesses: Vec<ActorId> = if q4 {
            vec![actor("Osanne Vell"), actor("Ede Kett")]
        } else {
            vec![subject.clone()]
        };
        let own = if q4 {
            json!({actor("Osanne Vell").as_str(): "I was standing in my own doorway on the Wickmarket when they took {subject} — he did not say one word, and the sergeant had his arm up behind his back"})
        } else {
            json!({})
        };
        let id = FactId::from_raw(row["fact_id"].as_str().unwrap());
        let pack = json!({"schema_version": 1, "facts": [{
            "id": id.as_str(), "topic": row["topic"].as_str().unwrap().to_lowercase(),
            "said": said, "subject": [subject], "seeded": witnesses,
            "own": own, "place": if q4 { json!("wickmarket") } else { Value::Null },
            "day": if q4 { json!(0) } else { Value::Null },
            "garble": if q4 { "subject,place,day" } else { "subject" }
        }]});
        let diagnostics = FactCatalog::from_json(&pack.to_string())
            .unwrap()
            .seed(&mut world);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let key = world.knowledge.key_of(&id).unwrap();
        let fact = world.knowledge.fact(key).unwrap();
        let view = knowledge::garble::view_for(&world, fact, &reader, hops);
        let from = if q4 && hops > 0 {
            Some(actor(match hops {
                1 => "Osanne Vell",
                2 => "Ansel Quern",
                3 => "Gile Skell",
                _ => "Havise Bram",
            }))
        } else {
            None
        };
        let mut held =
            knowledge::holds_key(&world, fact.seeded.iter().next().unwrap(), key).unwrap();
        held.hops = hops;
        held.from = from.clone();
        held.view = view.clone();
        let line =
            knowledge::render_line(&world, &reader, key, &held, env.strings(), Some(2.0)).unwrap();
        // Baseline is the actual M0b shipping-position sheet, not a newly
        // templated persona. Only its knowledge bullet changes.
        let original = fs::read_to_string(
            evidence
                .join("sheets/v6_both_turnj2")
                .join(format!("{name}.txt")),
        )
        .unwrap();
        let start = original.find("**what_you_know**").unwrap();
        let bullet = start + original[start..].find("\n- ").unwrap() + 1;
        let end = bullet + original[bullet..].find('\n').unwrap();
        let mut sheet = original.clone();
        sheet.replace_range(bullet..end, &format!("- {line}"));
        assert_eq!(&sheet[..bullet], &original[..bullet]);
        assert_eq!(&sheet[bullet + line.len() + 2..], &original[end..]);
        fs::write(output.join(format!("{name}.txt")), sheet).unwrap();
        records.push(json!({
            "sheet": name, "carrier": reader, "sequence": fact.sequence, "hops": hops,
            "from": from, "view": {"subject": view.subject, "place": view.place.map(|key| world.area_map.label_of_key(key)), "day_offset": view.day_offset},
            "rendered_line": line, "pack": pack
        }));
    }
    assert_eq!(records.len(), 12);
    fs::write(
        output.join("SEEDS.json"),
        serde_json::to_string_pretty(&records).unwrap() + "\n",
    )
    .unwrap();
    println!("Prepared {} sheets in {}", records.len(), output.display());
}

#[test]
#[ignore = "writes the explicitly requested live-provider evidence sheets"]
fn prepare_m4_provider_sheets() {
    use cathedral_sim::{Character, Control, Vec3, actions, prompt::render_prompt};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let evidence = [
        "features/knowledge_and_rumor",
        "features/implemented/knowledge_and_rumor",
    ]
    .into_iter()
    .map(|p| root.join(p).join("m0_evidence"))
    .find(|p| p.is_dir())
    .unwrap();
    let output =
        PathBuf::from(std::env::var_os("KNOWLEDGE_PROBE_OUT").expect("set KNOWLEDGE_PROBE_OUT"));
    fs::create_dir_all(&output).unwrap();
    let read = |p: &str| fs::read_to_string(root.join(p)).unwrap();
    let template = read("assets/prompts/turn.j2");
    let env = PromptEnv::new(
        &template,
        &read("assets/prompts/night.j2"),
        &read("assets/prompts/strings.toml"),
    )
    .unwrap();
    let fence = template
        .lines()
        .find(|line| line.contains("raise_word {\"topic\": \"bread\""))
        .unwrap()
        .split("%}")
        .last()
        .unwrap();
    assert!(fence.contains("the Wickmarket had no rye at all by midday"));
    let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
    let reader = ActorId::from_raw("fc2hb");
    let player = ActorId::from_raw("player");
    let odo = ActorId::from_raw("fo6gl");
    let mut records = Vec::new();
    for with_occasion in [false, true] {
        let suffix = if with_occasion { "with" } else { "no" };
        let old_name = format!("q5_raise_word_{suffix}_occasion");
        let name = format!("q5b_raise_word_{suffix}_occasion");
        let mut scenario: Value = serde_json::from_str(
            &fs::read_to_string(evidence.join("scenarios").join(format!("{old_name}.json")))
                .unwrap(),
        )
        .unwrap();
        scenario["verbs"]["add"][0]["line"] = json!(fence);
        fs::write(
            evidence.join("scenarios").join(format!("{name}.json")),
            serde_json::to_string_pretty(&scenario).unwrap() + "\n",
        )
        .unwrap();
        let mut world = World::new();
        world.area_map = AreaMap::from_json_str(&read("assets/world/areas.json")).unwrap();
        world.current_time = Some(WorldTime {
            day: 0,
            fraction: 0.0,
            office: Office::Dayspring,
            weekday: Weekday::Bellday,
        });
        let at = seed
            .characters
            .iter()
            .find(|sheet| sheet.id == reader)
            .unwrap()
            .position_m;
        for mut sheet in seed
            .characters
            .clone()
            .into_iter()
            .filter(|sheet| ["fc2hb", "fo6gl", "ft3tb", "player"].contains(&sheet.id.as_str()))
        {
            sheet.holds.clear();
            sheet.pockets.clear();
            let distance = match sheet.id.as_str() {
                "player" => 2.0,
                "ft3tb" => 4.4,
                "fo6gl" => 17.5,
                _ => 0.0,
            };
            sheet.position_m = at + Vec3::new(0.0, 0.0, -distance);
            sheet.facing_yaw = 0.0;
            if sheet.id == reader {
                sheet.control = Control::Llm;
                sheet.back_story = scenario["sheet"]["back_story"].as_str().unwrap().into();
                sheet.goal = scenario["sheet"]["current_goal"].as_str().unwrap().into();
                sheet.memories =
                    serde_json::from_value(scenario["sheet"]["stored_memories"]["items"].clone())
                        .unwrap();
                sheet.knows.insert(odo.clone());
                sheet.knows.insert(ActorId::from_raw("ft3tb"));
            }
            world.add_character(Character::from_sheet(sheet));
        }
        if with_occasion {
            knowledge::mint::note_assertion(
                &mut world,
                &player,
                &reader,
                "Odo Trask takes a cut off every salt cart that comes through the Tallage. Ask anyone.",
                0.0,
            );
        }
        assert_eq!(actions::may_raise_word(&world, &reader), with_occasion);
        let since: Vec<String> =
            serde_json::from_value(scenario["sheet"]["since_your_last_turn"]["items"].clone())
                .unwrap();
        let sheet = render_prompt(&world, &reader, Some(&since), &env).unwrap();
        assert_eq!(sheet.contains("raise_word {"), with_occasion);
        assert!(sheet.contains("Odo Trask"));
        fs::write(output.join(format!("{name}.txt")), sheet).unwrap();
        records.push(json!({"sheet": name, "carrier": reader, "with_occasion": with_occasion,
            "note": "Shipped renderer and original scenario backstory, memory, goal and since-lines; four real cast ids at scenario hearing distances. The no-occasion history is a render override and does not dispatch a say."}));
    }
    fs::write(
        output.join("SETUP.json"),
        serde_json::to_string_pretty(&records).unwrap() + "\n",
    )
    .unwrap();
}
