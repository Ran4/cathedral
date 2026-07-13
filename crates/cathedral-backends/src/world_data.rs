//! Filesystem-owned loading of the pure simulation's world data.

use std::{
    fs,
    path::{Path, PathBuf},
};

use cathedral_sim::{LoreCast, WorldSeed};

/// Load `assets/world/seed.json`, the occupation catalog, and every character
/// below `lore/characters`. File contents are interpreted by `cathedral-sim`;
/// this crate only owns discovery and IO.
pub fn load_world_seed(assets_dir: &Path, lore_dir: &Path) -> Result<WorldSeed, String> {
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
    base.with_lore_cast(cast)
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
    use cathedral_sim::{AreaMap, Vec3};
    use std::collections::BTreeSet;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn shipped_world_loads_all_lore_characters() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore"))
            .expect("the shipped world data loads");
        assert_eq!(seed.characters.len(), 104, "103 NPCs and the player");
        assert_eq!(
            seed.characters
                .iter()
                .filter(|character| character.control == cathedral_sim::Control::Llm)
                .count(),
            103
        );
        let player = seed
            .characters
            .iter()
            .find(|character| character.id.as_str() == "player")
            .expect("the base seed retains the player");
        assert_eq!(player.knows.len(), 103);

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
                profile.occupation_id.as_str(),
                profile.rank.as_deref()
            ),
            (19, "smith", Some("apprentice"))
        );
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
    fn every_authored_spawn_is_distinct_and_inside_its_intended_place() {
        let root = root();
        let seed = load_world_seed(&root.join("assets"), &root.join("lore")).unwrap();
        let areas = AreaMap::from_json_str(
            &fs::read_to_string(root.join("assets/world/areas.json")).unwrap(),
        )
        .unwrap();
        let mut positions = BTreeSet::new();
        for character in seed
            .characters
            .iter()
            .filter(|character| character.lore.is_some())
        {
            let lore = character.lore.as_ref().unwrap();
            let expected = match character.id.as_str() {
                // The original trio deliberately keep their opening-scene
                // transforms even though Sven and Conny work elsewhere.
                "sv3n1" | "cb947" | "k0fb1" => "gradine",
                _ => area_for_district(&lore.district),
            };
            let actual = areas
                .containing_area(character.position_m)
                .unwrap_or_else(|| panic!("{} is outside every named area", character.id));
            assert_eq!(
                actual.id, expected,
                "{} ({}) is in the wrong place",
                character.id, lore.district
            );
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
