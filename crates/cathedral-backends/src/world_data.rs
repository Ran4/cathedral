//! Filesystem-owned loading of the pure simulation's world data.

use std::{
    fs,
    path::{Path, PathBuf},
};

use cathedral_sim::{LoreCast, PlayerKnowledge, WorldSeed};

/// Load `assets/world/seed.json`, the occupation catalog, and every character
/// below `lore/characters`. File contents are interpreted by `cathedral-sim`;
/// this crate only owns discovery and IO.
pub fn load_world_seed(assets_dir: &Path, lore_dir: &Path) -> Result<WorldSeed, String> {
    load_world_seed_with_knowledge(assets_dir, lore_dir, PlayerKnowledge::PublicFigures)
}

pub fn load_world_seed_with_knowledge(
    assets_dir: &Path,
    lore_dir: &Path,
    knowledge: PlayerKnowledge,
) -> Result<WorldSeed, String> {
    let base_path = assets_dir.join("world/seed.json");
    let base_source = read(&base_path)?;
    let base = WorldSeed::from_json_str(&base_source)
        .map_err(|error| format!("invalid {}: {error}", base_path.display()))?;

    let occupations_path = lore_dir.join("core_lore/occupations.json");
    let occupations = read(&occupations_path)?;
    let characters_root = lore_dir.join("characters");
    let mut files = Vec::new();
    collect_json_files(&characters_root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "no character JSON files found under {}",
            characters_root.display()
        ));
    }

    let mut sources = Vec::with_capacity(files.len());
    for path in files {
        let relative = path.strip_prefix(&characters_root).map_err(|error| {
            format!(
                "could not make {} relative to {}: {error}",
                path.display(),
                characters_root.display()
            )
        })?;
        sources.push((relative.to_string_lossy().into_owned(), read(&path)?));
    }
    let cast = LoreCast::from_json_sources(&occupations, sources)
        .map_err(|error| format!("invalid lore cast: {error}"))?;
    base.with_lore_cast_knowledge(cast, knowledge)
        .map_err(|error| format!("invalid composed world seed: {error}"))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("could not read {}: {error}", path.display()))
}

fn collect_json_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not list {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read an entry in {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_json_files(&path, files)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cathedral_sim::{
        ActorId, AreaMap, PlanningWard, PlayerKnowledge, PromptEnv, Significance, Vec3,
        WorldConfig, build_world, render_prompt,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn shipped_world_loads_all_lore_characters() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore"))
            .expect("the shipped world data loads");
        assert_eq!(seed.characters.len(), 501, "500 NPCs and the player");
        assert_eq!(
            seed.characters
                .iter()
                .filter(|character| character.control == cathedral_sim::Control::Llm)
                .count(),
            500
        );
        let player = seed
            .characters
            .iter()
            .find(|character| character.id.as_str() == "player")
            .expect("the base seed retains the player");
        assert_eq!(player.knows.len(), 150);

        let sven = seed
            .characters
            .iter()
            .find(|character| character.id.as_str() == "sv3n1")
            .expect("Sven is lore-backed");
        let profile = sven
            .lore
            .as_ref()
            .expect("the full lore profile survives composition");
        assert_eq!(
            (
                profile.age,
                profile.occupation_id.as_deref(),
                profile.rank.as_deref()
            ),
            (19, Some("smith"), Some("apprentice"))
        );
        assert_eq!(profile.significance, Significance::Major);
        assert!(
            profile
                .extended_character_description
                .contains("Present business")
        );
        assert_eq!(
            sven.holds.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
            ["fzbn9"]
        );
    }

    #[test]
    fn developer_knowledge_mode_still_knows_everybody() {
        let root = root();
        let seed = load_world_seed_with_knowledge(
            &root.join("assets"),
            &root.join("lore"),
            PlayerKnowledge::Everyone,
        )
        .unwrap();
        let player = seed
            .characters
            .iter()
            .find(|character| character.id.as_str() == "player")
            .unwrap();
        assert_eq!(player.knows.len(), 500);
    }

    #[test]
    fn shipped_population_meets_the_authored_distribution_contract() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
        let npcs: Vec<_> = seed
            .characters
            .iter()
            .filter(|character| character.control == cathedral_sim::Control::Llm)
            .collect();
        let profiles: Vec<_> = npcs
            .iter()
            .map(|character| character.lore.as_ref().unwrap())
            .collect();

        let count_significance = |significance| {
            profiles
                .iter()
                .filter(|profile| profile.significance == significance)
                .count()
        };
        assert_eq!(count_significance(Significance::Major), 30);
        assert_eq!(count_significance(Significance::Minor), 120);
        assert_eq!(count_significance(Significance::Ambient), 350);
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| profile.gender == "f")
                .count(),
            250
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| profile.gender == "m")
                .count(),
            250
        );

        let mut age_bins = [0usize; 7];
        for profile in &profiles {
            age_bins[match profile.age {
                0..=7 => 0,
                8..=11 => 1,
                12..=15 => 2,
                16..=19 => 3,
                20..=39 => 4,
                40..=59 => 5,
                _ => 6,
            }] += 1;
        }
        assert_eq!(age_bins, [25, 25, 25, 55, 180, 125, 65]);

        let mut wards = BTreeMap::new();
        for profile in &profiles {
            *wards.entry(profile.planning_ward).or_insert(0usize) += 1;
        }
        assert_eq!(
            wards,
            BTreeMap::from([
                (PlanningWard::Fabric, 42),
                (PlanningWard::Wick, 40),
                (PlanningWard::Cloth, 43),
                (PlanningWard::Wallwright, 31),
                (PlanningWard::Cinder, 37),
                (PlanningWard::Weigh, 74),
                (PlanningWard::Reed, 64),
                (PlanningWard::BellAndSluice, 169),
            ])
        );

        let has_status = |profile: &&cathedral_sim::LoreProfile, status: &str| {
            profile.statuses.iter().any(|candidate| candidate == status)
        };
        assert_eq!(
            profiles.iter().filter(|p| has_status(p, "pauper")).count(),
            100
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|p| has_status(p, "begs_regularly"))
                .count(),
            38
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|p| has_status(p, "unhoused") || has_status(p, "insecure_lodging"))
                .count(),
            32
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|p| has_status(p, "intermittently_employed"))
                .count(),
            60
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| profile
                    .illegal_activity
                    .as_deref()
                    .is_some_and(|activity| !activity.contains("heresy")))
                .count(),
            50
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| matches!(
                    profile.rank.as_deref(),
                    Some("master" | "mistress" | "warden")
                ))
                .count(),
            55
        );
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| profile.occupation_id.is_none())
                .count(),
            10
        );
        let mut occupations = BTreeMap::new();
        for profile in &profiles {
            if let Some(occupation_id) = profile.occupation_id.as_deref() {
                *occupations.entry(occupation_id).or_insert(0usize) += 1;
            }
        }
        assert_eq!(
            occupations,
            BTreeMap::from([
                ("anchoress", 1),
                ("animal_worker", 14),
                ("bailiff_and_gaoler", 8),
                ("baker", 8),
                ("bell_ringer", 3),
                ("bellfounder", 2),
                ("boatworker", 9),
                ("brewer", 4),
                ("butcher", 4),
                ("candor_cleric", 7),
                ("cargo_worker", 16),
                ("carpenter_and_builder", 10),
                ("cartwright_and_wheelwright", 6),
                ("chandler", 7),
                ("church_attendant", 6),
                ("civic_officer", 7),
                ("cloth_worker", 12),
                ("cook", 8),
                ("cooper", 10),
                ("court_officer", 3),
                ("custody_clerk", 3),
                ("domestic_servant", 45),
                ("draper", 3),
                ("entertainer", 10),
                ("executioner", 1),
                ("farmer", 7),
                ("fine_metalworker", 8),
                ("fish_trader", 7),
                ("food_provisioner", 18),
                ("freight_broker", 2),
                ("funerary_worker", 3),
                ("garment_worker", 14),
                ("general_labourer", 23),
                ("glazier", 6),
                ("grocer_and_spicer", 5),
                ("guide", 3),
                ("healer", 6),
                ("instrument_maker", 2),
                ("lamplighter", 4),
                ("laundress", 7),
                ("leather_worker", 5),
                ("market_seller", 12),
                ("mason", 10),
                ("merchant", 4),
                ("messenger", 6),
                ("militia_and_soldier", 15),
                ("miller", 3),
                ("money_dealer", 3),
                ("painter", 2),
                ("pilgrim", 8),
                ("potter", 8),
                ("revenue_worker", 4),
                ("roper", 3),
                ("salt_trader", 3),
                ("salt_worker", 4),
                ("sanitation_worker", 12),
                ("scavenger", 7),
                ("scholar", 2),
                ("scribe_and_clerk", 6),
                ("sex_worker", 8),
                ("shoemaker", 12),
                ("smith", 5),
                ("tavern_worker", 9),
                ("watchman_and_keeper", 9),
                ("water_and_bath_worker", 8),
            ])
        );

        let significance_by_id: BTreeMap<_, _> = npcs
            .iter()
            .map(|character| (&character.id, character.lore.as_ref().unwrap().significance))
            .collect();
        for character in npcs {
            let profile = character.lore.as_ref().unwrap();
            if profile.significance == Significance::Ambient {
                continue;
            }
            for related in character
                .knows
                .iter()
                .chain(profile.father.iter())
                .chain(profile.mother.iter())
                .chain(profile.children.iter())
            {
                assert_ne!(
                    significance_by_id[related],
                    Significance::Ambient,
                    "stable character {} depends on ambient character {related}",
                    character.id
                );
            }
        }
    }

    #[test]
    fn authored_spawns_cover_the_city_without_crowding() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
        let areas = AreaMap::from_json_str(
            &fs::read_to_string(root.join("assets/world/areas.json")).unwrap(),
        )
        .unwrap();
        let npcs: Vec<_> = seed
            .characters
            .iter()
            .filter(|character| character.lore.is_some())
            .collect();
        assert_eq!(npcs.len(), 500);

        let mut positions = BTreeSet::new();
        for character in &npcs {
            let lore = character.lore.as_ref().unwrap();
            if lore.significance == Significance::Major {
                let expected = match character.id.as_str() {
                    // The opening trio deliberately share the Gradine.
                    "sv3n1" | "cb947" | "k0fb1" => "gradine",
                    _ => area_for_district(&lore.district),
                };
                let actual = areas
                    .containing_area(character.position_m)
                    .unwrap_or_else(|| {
                        panic!(
                            "major character {} is outside their named area",
                            character.id
                        )
                    });
                assert_eq!(actual.id, expected);
            }
            let key = (
                character.position_m.x.to_bits(),
                character.position_m.y.to_bits(),
                character.position_m.z.to_bits(),
            );
            assert!(
                positions.insert(key),
                "duplicate spawn at {:?}",
                character.position_m
            );
        }

        let find = |id: &str| {
            seed.characters
                .iter()
                .find(|character| character.id.as_str() == id)
                .unwrap()
        };
        assert_eq!(find("sv3n1").position_m, Vec3::new(-1.8, 0.91, 114.0));
        assert_eq!(find("cb947").position_m, Vec3::new(0.0, 0.91, 112.0));
        assert_eq!(find("k0fb1").position_m, Vec3::new(1.8, 0.91, 114.0));

        let maximum_nearby = npcs
            .iter()
            .map(|left| {
                npcs.iter()
                    .filter(|right| {
                        let dx = left.position_m.x - right.position_m.x;
                        let dz = left.position_m.z - right.position_m.z;
                        dx * dx + dz * dz <= 20.0 * 20.0
                    })
                    .count()
            })
            .max()
            .unwrap();
        assert_eq!(
            maximum_nearby, 3,
            "at most three NPCs may share a 20 m neighborhood"
        );

        let mut maximum_region = 0;
        for left in &npcs {
            let mut zs: Vec<_> = npcs
                .iter()
                .filter(|character| {
                    left.position_m.x <= character.position_m.x
                        && character.position_m.x <= left.position_m.x + 100.0
                })
                .map(|character| character.position_m.z)
                .collect();
            zs.sort_by(f64::total_cmp);
            let mut low = 0;
            for high in 0..zs.len() {
                while zs[low] < zs[high] - 100.0 {
                    low += 1;
                }
                maximum_region = maximum_region.max(high - low + 1);
            }
        }
        assert!(
            maximum_region <= 10,
            "a sliding 100 x 100 m region contains {maximum_region} NPCs"
        );

        let x_span = npcs
            .iter()
            .map(|character| character.position_m.x)
            .reduce(f64::max)
            .unwrap()
            - npcs
                .iter()
                .map(|character| character.position_m.x)
                .reduce(f64::min)
                .unwrap();
        let z_span = npcs
            .iter()
            .map(|character| character.position_m.z)
            .reduce(f64::max)
            .unwrap()
            - npcs
                .iter()
                .map(|character| character.position_m.z)
                .reduce(f64::min)
                .unwrap();
        assert!(
            x_span >= 1_000.0 && z_span >= 1_100.0,
            "the cast covers only {x_span:.1} x {z_span:.1} m"
        );
        let occupied_cells: BTreeSet<_> = npcs
            .iter()
            .map(|character| {
                (
                    (character.position_m.x / 100.0).floor() as i32,
                    (character.position_m.z / 100.0).floor() as i32,
                )
            })
            .collect();
        assert!(
            occupied_cells.len() >= 120,
            "the cast reaches only {} fixed 100 m grid cells",
            occupied_cells.len()
        );

        for (index, left) in npcs.iter().enumerate() {
            for right in &npcs[index + 1..] {
                let dx = left.position_m.x - right.position_m.x;
                let dz = left.position_m.z - right.position_m.z;
                assert!(
                    dx != 0.0 || dz != 0.0,
                    "NPCs {} and {} have the same horizontal spawn",
                    left.id,
                    right.id
                );
            }
        }
    }

    #[test]
    fn full_roster_prompts_and_public_snapshot_remain_bounded() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
        let areas = AreaMap::from_json_str(
            &fs::read_to_string(root.join("assets/world/areas.json")).unwrap(),
        )
        .unwrap();
        let world = build_world(
            &seed,
            WorldConfig {
                area_map: areas,
                sounds_enabled: false,
                ..WorldConfig::default()
            },
        );
        let env = PromptEnv::new(
            &fs::read_to_string(root.join("assets/prompts/turn.j2")).unwrap(),
            &fs::read_to_string(root.join("assets/prompts/strings.toml")).unwrap(),
        )
        .unwrap();

        let mut longest = (0usize, "");
        for character in seed
            .characters
            .iter()
            .filter(|character| character.control == cathedral_sim::Control::Llm)
        {
            let prompt = render_prompt(&world, &character.id, None, &env).unwrap();
            assert!(
                !prompt.contains("\"significance\""),
                "host-only significance leaked into {}'s prompt",
                character.id
            );
            if prompt.len() > longest.0 {
                longest = (prompt.len(), character.id.as_str());
            }
        }
        assert!(
            longest.0 <= 64 * 1024,
            "longest prompt is {} bytes for {}, over 64 KiB",
            longest.0,
            longest.1
        );

        let snapshot = world.public_snapshot(&ActorId::from_raw("player"));
        assert_eq!(snapshot.actors.len(), 501);
        let encoded = serde_json::to_vec(&snapshot).unwrap();
        assert!(
            encoded.len() <= 128 * 1024,
            "public snapshot is {} bytes, over 128 KiB",
            encoded.len()
        );
    }

    fn area_for_district(district: &str) -> &'static str {
        match district {
            "A tall house off the Tallage"
            | "The Tallage"
            | "The Tallage, shopfront facing the Tally Bridge" => "tallage",
            "Bellfoot Passage" => "bellfoot_passage",
            "Cinder Row" => "cinder_row",
            "Coswald's Yard" | "Coswald's Yard and the north tower scaffold" => "coswalds_yard",
            "Gaunt Passage" | "The Tallage and Gaunt Passage" => "gaunt_passage",
            "Lodgings off the Bellstand; the nave daily"
            | "The Lanthorn"
            | "The Lanthorn nave; a corner in Skinners' Court"
            | "The Lanthorn towers" => "lanthorn_interior",
            "Malt Passage" => "malt_passage",
            "Maren's Green" | "Maren's Green, the stall by the fish-hall steps" => "marens_green",
            "Maren's Green and the Alder Moorings" | "The Alder Moorings" => "alder_moorings",
            "Saint Maren of the Reeds" => "saint_marens_church",
            "Saint Maren's and the charnel door" => "charnel_door",
            "Saint Maren's churchyard" => "saint_marens_churchyard",
            "Skinners' Court" => "skinners_court",
            "Tanners' Slip" => "tanners_slip",
            "Tenterhook Lane" => "tenterhook_lane",
            "The Bellstand"
            | "The Bellstand and circuits of the five squares"
            | "The Bellstand, the Bell and Ladle" => "bellstand",
            "The Cut" | "The Cut, the tenter-yards below Maren's Green" => "the_cut",
            "The Draper's Reach" => "drapers_reach",
            "The Gradine" => "gradine",
            "The Hungry Ox" => "hungry_ox",
            "The Ilvane Chapel anchorhold" => "ilvane_anchorhold",
            "The Lanthorn (quire and chapter house)"
            | "The Lanthorn and the chapter close"
            | "The chapter house (the Grey Press)"
            | "The chapter house and the Grey Press" => "chapter_house",
            "The Needle" | "The Needle, and everywhere" => "the_needle",
            "The Old Sluice" => "old_sluice",
            "The Tally Bridge" => "tally_bridge",
            "The Wickmarket" => "wickmarket",
            "The shambles" => "shambles",
            other => panic!("district '{other}' needs an explicit spawn-area mapping"),
        }
    }
}
