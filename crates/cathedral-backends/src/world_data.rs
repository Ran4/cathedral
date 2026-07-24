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
    let cast = LoreCast::from_json_sources(&occupations, character_sources(lore_dir)?)
        .map_err(|error| format!("invalid lore cast: {error}"))?;
    base.with_lore_cast_knowledge(cast, knowledge)
        .map_err(|error| format!("invalid composed world seed: {error}"))
}

/// Discover and read every character sheet below `lore/characters`, as sorted
/// `(relative path, contents)` pairs — the exact inputs the loader composes
/// into the world. Public so tests and bins can derive cast counts from the
/// same discovery the loader uses, instead of hardcoding a roster size that
/// goes stale whenever the city gains a citizen.
pub fn character_sources(lore_dir: &Path) -> Result<Vec<(String, String)>, String> {
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
    Ok(sources)
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
        ActorId, AreaMap, PlayerKnowledge, PromptEnv, Significance, Vec3,
        WorldConfig, build_world, render_prompt,
    };
    use std::collections::BTreeSet;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Counts derived from the shipped character files themselves: total
    /// sheets, and how many are public figures (non-ambient). Comparing the
    /// composed world against these keeps the tests from going stale when the
    /// city gains a citizen, while still catching a sheet that silently fails
    /// to compose into the seed.
    fn cast_counts(root: &Path) -> (usize, usize) {
        let sources = character_sources(&root.join("lore")).expect("the lore cast is readable");
        let public = sources
            .iter()
            .filter(|(path, source)| {
                let sheet: serde_json::Value = serde_json::from_str(source)
                    .unwrap_or_else(|error| panic!("character file {path} does not parse: {error}"));
                sheet["significance"] != "ambient"
            })
            .count();
        (sources.len(), public)
    }

    #[test]
    fn shipped_world_loads_all_lore_characters() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore"))
            .expect("the shipped world data loads");
        let (cast_total, cast_public) = cast_counts(&root);
        assert_eq!(
            seed.characters.len(),
            cast_total + 1,
            "every lore character and the player"
        );
        assert_eq!(
            seed.characters
                .iter()
                .filter(|character| character.control == cathedral_sim::Control::Llm)
                .count(),
            cast_total
        );
        let player = seed
            .characters
            .iter()
            .find(|character| character.id.as_str() == "player")
            .expect("the base seed retains the player");
        assert_eq!(player.knows.len(), cast_public);

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
        // The baked home-binding's spoken form is joined in during composition
        // (`cathedral_sim::homes`), so every profile can answer "Where do you
        // live?" — Sven is housed; a bedless sheet carries its framing instead.
        assert!(
            profile
                .home
                .as_deref()
                .is_some_and(|home| home.starts_with("a house in the ")),
            "Sven's composed profile lost its home: {:?}",
            profile.home
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
        assert_eq!(player.knows.len(), cast_counts(&root).0);
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
        assert_eq!(npcs.len(), cast_counts(&root).0);

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
        assert!(
            maximum_nearby <= 4,
            "at most four NPCs may share a 20 m neighborhood \
             (was three pre-shrink; the 0.7x city holds the same 519-person cast)"
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
            maximum_region <= 20,
            "a sliding 100 x 100 m region contains {maximum_region} NPCs \
             (cap doubled with the 0.7x shrink: same cast, 0.49x area)"
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
            x_span >= 730.0 && z_span >= 680.0,
            "the cast covers only {x_span:.1} x {z_span:.1} m \
             (spans rescaled with the 0.7x city; measured 858 x 804 post-shrink)"
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
            occupied_cells.len() >= 62,
            "the cast reaches only {} fixed 100 m grid cells \
             (floor rescaled with the 0.7x city; measured 74 post-shrink)",
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
        assert_eq!(snapshot.actors.len(), cast_counts(&root).0 + 1);
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
