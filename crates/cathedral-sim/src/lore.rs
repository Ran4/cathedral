//! Pure parsing and validation for the authored cast in `lore/characters`.
//!
//! This module deliberately accepts source strings and relative names rather
//! than paths. Hosts own filesystem discovery; the simulation owns what the
//! character data means.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{
    GOAL_NONE,
    character::{CharacterSheet, Control},
    ids::{ActorId, ItemId},
    math::Vec3,
};

pub const NO_FIXED_TRADE_FOLDER: &str = "no_fixed_trade";

/// Canonical importance and the corresponding default compute class.
///
/// This is host metadata. It is deliberately not rendered into an NPC's
/// prompt as a statement about the person's worth or self-knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Significance {
    Major,
    Minor,
    Ambient,
}

impl Significance {
    /// Completion budget used by provider-backed cognition. The prompt format
    /// keeps actions compact, so these caps leave ample room for several
    /// actions while making reactive ambient turns materially cheaper.
    pub const fn output_token_budget(self) -> u32 {
        match self {
            Self::Major => 1_200,
            Self::Minor => 700,
            Self::Ambient => 350,
        }
    }
}

/// Broad authoring geography from `lore/places/00_city_plan.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningWard {
    Fabric,
    Wick,
    Cloth,
    Wallwright,
    Cinder,
    Weigh,
    Reed,
    BellAndSluice,
}

/// Status spellings shared by authoring and loader validation. Health and
/// bodily conditions do not belong here; they remain in `conditions`.
pub const CONTROLLED_STATUSES: &[&str] = &[
    "alms_dependent",
    "begs_regularly",
    "dependent",
    "enclosed_religious",
    "illiterate",
    "insecure_lodging",
    "intermittently_employed",
    "noncitizen",
    "orphan",
    "pauper",
    "prisoner",
    "recent_migrant",
    "recanted_heretic",
    "retired",
    "spared",
    "unemployed",
    "unhoused",
    "widow",
    "widower",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoreError {
    pub message: String,
}

impl LoreError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for LoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoreError {}

/// Authored metadata retained alongside the mutable simulation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoreProfile {
    pub significance: Significance,
    pub planning_ward: PlanningWard,
    pub age: u16,
    pub gender: String,
    pub occupation_id: Option<String>,
    pub occupation_display: Option<String>,
    pub title: Option<String>,
    pub rank: Option<String>,
    pub faction_role: Option<String>,
    pub illegal_activity: Option<String>,
    pub district: String,
    pub father: Option<ActorId>,
    pub mother: Option<ActorId>,
    pub children: Vec<ActorId>,
    pub statuses: Vec<String>,
    pub conditions: Vec<String>,
    pub core_character_description: String,
    pub extended_character_description: String,
    /// How readily this person speaks *first* — the authored override on the
    /// curiosity gate (`attention::curiosity_of`). `None` means "derive it from
    /// the metadata above", which is what all 500 shipped files do; a character
    /// the derivation gets wrong says so here, and nobody else is touched.
    ///
    /// It is not [`Significance`], which sets the completion budget. An ambient
    /// child may be cheap and forward, a major canon expensive and aloof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curiosity: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoreSpawnLocation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub facing: f64,
}

/// One JSON document under `lore/characters/{occupation}/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoreCharacterSheet {
    pub id: ActorId,
    pub name: String,
    pub significance: Significance,
    pub planning_ward: PlanningWard,
    pub age: u16,
    pub gender: String,
    pub occupation_id: Option<String>,
    pub title: Option<String>,
    pub rank: Option<String>,
    pub faction_role: Option<String>,
    pub illegal_activity: Option<String>,
    pub district: String,
    #[serde(default)]
    pub knows: Vec<ActorId>,
    pub father: Option<ActorId>,
    pub mother: Option<ActorId>,
    #[serde(default)]
    pub children: Vec<ActorId>,
    pub spawn_location: LoreSpawnLocation,
    #[serde(default)]
    pub statuses: Vec<String>,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(default)]
    pub memories: Vec<String>,
    pub core_character_description: String,
    pub extended_character_description: String,
    /// Optional game-facing overrides. Most authored people use shared visual
    /// and voice defaults; the original demo trio retains its established cast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holds: Vec<ItemId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Overrides the derived willingness to speak first (see [`LoreProfile`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curiosity: Option<f64>,
}

impl LoreCharacterSheet {
    pub fn from_json_str(source: &str) -> Result<Self, LoreError> {
        let sheet: Self = serde_json::from_str(source)
            .map_err(|error| LoreError::new(format!("invalid lore character: {error}")))?;
        sheet.validate()?;
        Ok(sheet)
    }

    fn validate(&self) -> Result<(), LoreError> {
        if !self.id.is_valid() || self.id.as_str().chars().count() != 5 {
            return Err(LoreError::new(format!(
                "invalid character id '{}': lore character ids must be exactly five characters",
                self.id
            )));
        }
        if self.name.trim().is_empty() {
            return Err(LoreError::new(format!(
                "character '{}' has no name",
                self.id
            )));
        }
        match (&self.occupation_id, &self.title) {
            (Some(occupation), Some(title))
                if !occupation.trim().is_empty() && !title.trim().is_empty() => {}
            (None, None) if self.rank.is_none() => {
                if !self.statuses.iter().any(|status| {
                    matches!(
                        status.as_str(),
                        "alms_dependent" | "dependent" | "pauper" | "prisoner"
                    )
                }) {
                    return Err(LoreError::new(format!(
                        "character '{}' has no fixed trade and needs a support status",
                        self.id
                    )));
                }
            }
            _ => {
                return Err(LoreError::new(format!(
                    "character '{}' must have both occupation/title, or null occupation/title/rank",
                    self.id
                )));
            }
        }
        if self.district.trim().is_empty() || self.core_character_description.trim().is_empty() {
            return Err(LoreError::new(format!(
                "character '{}' needs a district and core description",
                self.id
            )));
        }
        let spawn = self.spawn_location;
        if !spawn.x.is_finite()
            || !spawn.y.is_finite()
            || !spawn.z.is_finite()
            || !spawn.facing.is_finite()
        {
            return Err(LoreError::new(format!(
                "character '{}' needs a finite spawn transform",
                self.id
            )));
        }
        // Caught here rather than clamped at the gate: a `curiosity` of 20 is a
        // typo, and silently reading it as 1.0 would hide the file that meant
        // 0.2 and buried its author in prompts.
        if let Some(curiosity) = self.curiosity
            && !(0.0..=1.0).contains(&curiosity)
        {
            return Err(LoreError::new(format!(
                "character '{}' has curiosity {curiosity}, which is not a probability in 0.0..=1.0",
                self.id
            )));
        }
        let mut seen_statuses = BTreeSet::new();
        for status in &self.statuses {
            if !CONTROLLED_STATUSES.contains(&status.as_str()) {
                return Err(LoreError::new(format!(
                    "character '{}' has unknown status '{status}'",
                    self.id
                )));
            }
            if !seen_statuses.insert(status) {
                return Err(LoreError::new(format!(
                    "character '{}' repeats status '{status}'",
                    self.id
                )));
            }
        }
        for related in self
            .knows
            .iter()
            .chain(self.father.iter())
            .chain(self.mother.iter())
            .chain(self.children.iter())
        {
            if !related.is_valid() {
                return Err(LoreError::new(format!(
                    "character '{}' references invalid id '{related}'",
                    self.id
                )));
            }
        }
        Ok(())
    }

    fn into_character_sheet(self, occupation_display: Option<String>) -> CharacterSheet {
        let voice_key = self
            .voice_key
            .clone()
            .unwrap_or_else(|| default_voice_key(&self.gender, &self.id));
        CharacterSheet {
            id: self.id,
            name: self.name,
            control: Control::Llm,
            back_story: self.core_character_description.clone(),
            location_description: self.district.clone(),
            appearance_key: self.appearance_key.unwrap_or_else(|| "generic".to_string()),
            voice_key: Some(voice_key),
            position_m: Vec3::new(
                self.spawn_location.x,
                self.spawn_location.y,
                self.spawn_location.z,
            ),
            facing_yaw: self.spawn_location.facing,
            holds: self.holds,
            goal: self.goal.unwrap_or_else(|| GOAL_NONE.to_string()),
            memories: self.memories,
            knows: self.knows.into_iter().collect(),
            lore: Some(LoreProfile {
                significance: self.significance,
                planning_ward: self.planning_ward,
                age: self.age,
                gender: self.gender,
                occupation_id: self.occupation_id,
                occupation_display,
                title: self.title,
                rank: self.rank,
                faction_role: self.faction_role,
                illegal_activity: self.illegal_activity,
                district: self.district,
                father: self.father,
                mother: self.mother,
                children: self.children,
                statuses: self.statuses,
                conditions: self.conditions,
                core_character_description: self.core_character_description,
                extended_character_description: self.extended_character_description,
                curiosity: self.curiosity,
            }),
        }
    }
}

fn default_voice_key(gender: &str, id: &ActorId) -> String {
    if gender.eq_ignore_ascii_case("f") {
        return "ilse".to_string();
    }
    let checksum = id
        .as_str()
        .bytes()
        .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(byte)));
    if checksum % 2 == 0 { "sven" } else { "conny" }.to_string()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Occupation {
    occupation_id: String,
    occupation_display: String,
    lore_locations: Vec<String>,
    lore_example: String,
    alternative_titles: Vec<String>,
}

/// A fully validated cast in deterministic relative-path order.
#[derive(Debug, Clone, PartialEq)]
pub struct LoreCast {
    characters: Vec<(LoreCharacterSheet, Option<String>)>,
}

impl LoreCast {
    /// Parse occupation data and `(relative_path, json)` sources. Relative paths
    /// are rooted at `lore/characters` and therefore have the form
    /// `{occupation}/{id}_{slug}.json`.
    pub fn from_json_sources(
        occupations_source: &str,
        sources: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, LoreError> {
        let occupations: Vec<Occupation> = serde_json::from_str(occupations_source)
            .map_err(|error| LoreError::new(format!("invalid occupation catalog: {error}")))?;
        let mut occupation_map = BTreeMap::new();
        for occupation in occupations {
            let id = occupation.occupation_id.clone();
            if id.trim().is_empty() || occupation.occupation_display.trim().is_empty() {
                return Err(LoreError::new(
                    "occupation ids and displays may not be empty",
                ));
            }
            if occupation.lore_locations.is_empty() || occupation.lore_example.trim().is_empty() {
                return Err(LoreError::new(format!(
                    "occupation '{id}' needs locations and a lore example"
                )));
            }
            if occupation_map.insert(id.clone(), occupation).is_some() {
                return Err(LoreError::new(format!("duplicate occupation id '{id}'")));
            }
        }

        let mut sources: Vec<_> = sources.into_iter().collect();
        sources.sort_by(|left, right| left.0.cmp(&right.0));
        let mut parsed = Vec::with_capacity(sources.len());
        let mut ids = BTreeSet::new();
        for (relative_path, source) in sources {
            let normalized = relative_path.replace('\\', "/");
            let mut components = normalized.split('/');
            let folder = components.next().unwrap_or_default();
            let file_name = components.next().unwrap_or_default();
            if folder.is_empty() || file_name.is_empty() || components.next().is_some() {
                return Err(LoreError::new(format!(
                    "character source '{relative_path}' must be occupation/file.json"
                )));
            }
            let sheet = LoreCharacterSheet::from_json_str(&source)
                .map_err(|error| LoreError::new(format!("{relative_path}: {error}")))?;
            if !file_name.ends_with(".json") || !file_name.starts_with(&format!("{}_", sheet.id)) {
                return Err(LoreError::new(format!(
                    "{relative_path}: filename must start with '{}_' and end in .json",
                    sheet.id
                )));
            }
            let occupation_display = match (&sheet.occupation_id, &sheet.title) {
                (Some(occupation_id), Some(title)) => {
                    if folder != occupation_id {
                        return Err(LoreError::new(format!(
                            "{relative_path}: folder '{folder}' does not match occupation '{occupation_id}'"
                        )));
                    }
                    let Some(occupation) = occupation_map.get(occupation_id) else {
                        return Err(LoreError::new(format!(
                            "{relative_path}: unknown occupation '{occupation_id}'"
                        )));
                    };
                    if !occupation.alternative_titles.contains(title) {
                        return Err(LoreError::new(format!(
                            "{relative_path}: title '{title}' is not valid for occupation '{occupation_id}'"
                        )));
                    }
                    Some(occupation.occupation_display.clone())
                }
                (None, None) => {
                    if folder != NO_FIXED_TRADE_FOLDER {
                        return Err(LoreError::new(format!(
                            "{relative_path}: a null occupation must be stored under '{NO_FIXED_TRADE_FOLDER}'"
                        )));
                    }
                    None
                }
                _ => unreachable!("sheet-level validation checked occupation/title pairing"),
            };
            if !ids.insert(sheet.id.clone()) {
                return Err(LoreError::new(format!(
                    "{relative_path}: duplicate character id '{}'",
                    sheet.id
                )));
            }
            parsed.push((sheet, occupation_display));
        }

        for (sheet, _) in &parsed {
            for related in sheet
                .knows
                .iter()
                .chain(sheet.father.iter())
                .chain(sheet.mother.iter())
                .chain(sheet.children.iter())
            {
                if !ids.contains(related) {
                    return Err(LoreError::new(format!(
                        "character '{}' references missing character '{related}'",
                        sheet.id
                    )));
                }
                if sheet.significance != Significance::Ambient
                    && parsed.iter().any(|(candidate, _)| {
                        candidate.id == *related && candidate.significance == Significance::Ambient
                    })
                {
                    return Err(LoreError::new(format!(
                        "stable character '{}' may not depend on ambient character '{related}'",
                        sheet.id
                    )));
                }
            }
        }
        for (sheet, _) in &parsed {
            for parent in sheet.father.iter().chain(sheet.mother.iter()) {
                let parent_sheet = parsed
                    .iter()
                    .find(|(candidate, _)| candidate.id == *parent)
                    .map(|(candidate, _)| candidate)
                    .expect("references were checked above");
                if !parent_sheet.children.contains(&sheet.id) {
                    return Err(LoreError::new(format!(
                        "character '{}' names '{parent}' as a parent, but the parent does not name the child",
                        sheet.id
                    )));
                }
            }
            for child in &sheet.children {
                let child_sheet = parsed
                    .iter()
                    .find(|(candidate, _)| candidate.id == *child)
                    .map(|(candidate, _)| candidate)
                    .expect("references were checked above");
                if child_sheet.father.as_ref() != Some(&sheet.id)
                    && child_sheet.mother.as_ref() != Some(&sheet.id)
                {
                    return Err(LoreError::new(format!(
                        "character '{}' names '{child}' as a child, but the child does not name the parent",
                        sheet.id
                    )));
                }
            }
        }

        Ok(Self { characters: parsed })
    }

    pub fn len(&self) -> usize {
        self.characters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.characters.is_empty()
    }

    pub fn ids(&self) -> impl ExactSizeIterator<Item = &ActorId> {
        self.characters.iter().map(|(sheet, _)| &sheet.id)
    }

    pub fn public_ids(&self) -> impl Iterator<Item = &ActorId> {
        self.characters
            .iter()
            .filter(|(sheet, _)| sheet.significance != Significance::Ambient)
            .map(|(sheet, _)| &sheet.id)
    }

    pub(crate) fn into_character_sheets(self) -> Vec<CharacterSheet> {
        self.characters
            .into_iter()
            .map(|(sheet, display)| sheet.into_character_sheet(display))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OCCUPATIONS: &str = r#"[{"occupation_id":"smith","occupation_display":"Smith","lore_locations":["forge"],"lore_example":"A smith works.","alternative_titles":["Blacksmith"]}]"#;

    fn character(id: &str, knows: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"N","significance":"minor","planning_ward":"cinder","age":20,"gender":"m","occupation_id":"smith","title":"Blacksmith","rank":null,"faction_role":null,"illegal_activity":null,"district":"Cinder Row","knows":{knows},"father":null,"mother":null,"children":[],"spawn_location":{{"x":1.0,"y":0.91,"z":2.0,"facing":0.0}},"statuses":[],"conditions":[],"memories":[],"core_character_description":"You smith.","extended_character_description":"More."}}"#
        )
    }

    #[test]
    fn cast_order_is_relative_path_order_and_defaults_are_stable() {
        let cast = LoreCast::from_json_sources(
            OCCUPATIONS,
            [
                ("smith/bbbbb_b.json".into(), character("bbbbb", "[]")),
                ("smith/aaaaa_a.json".into(), character("aaaaa", "[]")),
            ],
        )
        .unwrap();
        assert_eq!(
            cast.ids().map(ActorId::as_str).collect::<Vec<_>>(),
            ["aaaaa", "bbbbb"]
        );
        let sheets = cast.into_character_sheets();
        assert_eq!(sheets[0].appearance_key, "generic");
        assert!(matches!(
            sheets[0].voice_key.as_deref(),
            Some("sven" | "conny")
        ));
        assert_eq!(sheets[0].back_story, "You smith.");
        assert_eq!(
            sheets[0]
                .lore
                .as_ref()
                .unwrap()
                .occupation_display
                .as_deref(),
            Some("Smith")
        );
    }

    #[test]
    fn source_path_occupation_title_and_relationships_are_validated() {
        let bad_folder = LoreCast::from_json_sources(
            OCCUPATIONS,
            [("baker/aaaaa_a.json".into(), character("aaaaa", "[]"))],
        )
        .unwrap_err();
        assert!(bad_folder.message.contains("does not match occupation"));

        let dangling = LoreCast::from_json_sources(
            OCCUPATIONS,
            [(
                "smith/aaaaa_a.json".into(),
                character("aaaaa", r#"["ghost"]"#),
            )],
        )
        .unwrap_err();
        assert!(dangling.message.contains("missing character 'ghost'"));
    }

    #[test]
    fn no_fixed_trade_requires_the_structural_folder_null_fields_and_support() {
        let source = character("aaaaa", "[]")
            .replace(r#""occupation_id":"smith""#, r#""occupation_id":null"#)
            .replace(r#""title":"Blacksmith""#, r#""title":null"#)
            .replace(r#""statuses":[]"#, r#""statuses":["dependent"]"#);
        let cast = LoreCast::from_json_sources(
            OCCUPATIONS,
            [("no_fixed_trade/aaaaa_n.json".into(), source)],
        )
        .unwrap();
        let sheets = cast.into_character_sheets();
        let profile = sheets[0].lore.as_ref().unwrap();
        assert_eq!(profile.occupation_id, None);
        assert_eq!(profile.occupation_display, None);
        assert_eq!(profile.title, None);
    }

    #[test]
    fn statuses_are_controlled_and_stable_characters_cannot_name_ambient_people() {
        let unknown_status =
            character("aaaaa", "[]").replace(r#""statuses":[]"#, r#""statuses":["very_poor"]"#);
        let error = LoreCast::from_json_sources(
            OCCUPATIONS,
            [("smith/aaaaa_n.json".into(), unknown_status)],
        )
        .unwrap_err();
        assert!(error.message.contains("unknown status 'very_poor'"));

        let ambient = character("bbbbb", "[]")
            .replace(r#""significance":"minor""#, r#""significance":"ambient""#);
        let stable = character("aaaaa", r#"["bbbbb"]"#);
        let error = LoreCast::from_json_sources(
            OCCUPATIONS,
            [
                ("smith/aaaaa_n.json".into(), stable),
                ("smith/bbbbb_n.json".into(), ambient),
            ],
        )
        .unwrap_err();
        assert!(error.message.contains("may not depend on ambient"));
    }
}
