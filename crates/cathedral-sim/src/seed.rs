//! The seeded world (`main.py:52-139`): the cast, their items, and the one
//! function that turns them into a [`World`].
//!
//! The data is `assets/world/seed.json` (D19) — the host reads the file and
//! hands the text to [`WorldSeed::from_json_str`]; the crate never touches the
//! filesystem. Production composes its player/items seed with the lore cast;
//! compact compatibility tests retain a dedicated four-character fixture.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    DEFAULT_VIEW_CONE_DEGREES, MAX_VIEW_CONE_DEGREES, MIN_VIEW_CONE_DEGREES,
    areas::AreaMap,
    character::{Character, CharacterSheet},
    ids::{ActorId, ItemId},
    item::Item,
    lore::LoreCast,
    sounds::SoundCatalog,
    world::World,
};

/// A seed file that does not describe a world the sim can build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedError {
    pub message: String,
}

impl SeedError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SeedError {}

fn generic_visual_key() -> String {
    "generic".to_string()
}

/// One item's static definition. (`Item` itself, with the seed file's rules:
/// no unknown keys, `visual_key` optional.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSeed {
    pub id: ItemId,
    pub name: String,
    #[serde(default = "generic_visual_key")]
    pub visual_key: String,
}

impl From<&ItemSeed> for Item {
    fn from(seed: &ItemSeed) -> Self {
        Item {
            id: seed.id.clone(),
            name: seed.name.clone(),
            visual_key: seed.visual_key.clone(),
        }
    }
}

/// The whole cast, in insertion order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSeed {
    #[serde(default)]
    pub items: Vec<ItemSeed>,
    #[serde(default)]
    pub characters: Vec<CharacterSheet>,
}

/// Which authored names a newly created player knows before any conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKnowledge {
    /// Major and minor public figures are known by reputation; ambient people
    /// begin as strangers.
    PublicFigures,
    /// Developer/headless convenience mode.
    Everyone,
}

impl WorldSeed {
    /// Parse and validate `assets/world/seed.json`.
    ///
    /// Validation is what lets [`build_world`] assert instead of erroring: a
    /// seed that gets past this point cannot trip `World`'s duplicate-id panics
    /// or its invariants.
    pub fn from_json_str(source: &str) -> Result<Self, SeedError> {
        let seed: WorldSeed = serde_json::from_str(source)
            .map_err(|error| SeedError::new(format!("invalid world seed: {error}")))?;
        seed.validate()?;
        Ok(seed)
    }

    pub fn validate(&self) -> Result<(), SeedError> {
        let mut item_ids: Vec<&ItemId> = Vec::with_capacity(self.items.len());
        for item in &self.items {
            if !item.id.is_valid() {
                return Err(SeedError::new(format!("invalid item id '{}'", item.id)));
            }
            if item_ids.contains(&&item.id) {
                return Err(SeedError::new(format!("duplicate item id '{}'", item.id)));
            }
            item_ids.push(&item.id);
        }

        let mut character_ids: Vec<&ActorId> = Vec::with_capacity(self.characters.len());
        let mut holders: Vec<(&ItemId, &ActorId)> = Vec::new();
        for character in &self.characters {
            if !character.id.is_valid() {
                return Err(SeedError::new(format!(
                    "invalid character id '{}'",
                    character.id
                )));
            }
            if character_ids.contains(&&character.id) {
                return Err(SeedError::new(format!(
                    "duplicate character id '{}'",
                    character.id
                )));
            }
            character_ids.push(&character.id);

            if !character.facing_yaw.is_finite() {
                return Err(SeedError::new(format!(
                    "character '{}' needs a finite facing_yaw",
                    character.id
                )));
            }
            for item_id in &character.holds {
                if !item_ids.contains(&item_id) {
                    return Err(SeedError::new(format!(
                        "character '{}' holds missing item '{item_id}'",
                        character.id
                    )));
                }
                if let Some((_, other)) = holders.iter().find(|(held, _)| *held == item_id) {
                    return Err(SeedError::new(format!(
                        "item '{item_id}' is held by both '{other}' and '{}'",
                        character.id
                    )));
                }
                holders.push((item_id, &character.id));
            }
            // A `knows` entry pointing at nobody is tolerated, exactly as in
            // Python (`main.py:165` skips ids that are not in the world) — only
            // the id's *shape* has to be sound.
            for known in &character.knows {
                if !known.is_valid() {
                    return Err(SeedError::new(format!(
                        "character '{}' knows an invalid id '{known}'",
                        character.id
                    )));
                }
            }
        }
        Ok(())
    }

    /// The sheet for `actor_id`, if the seed has one.
    pub fn character(&self, actor_id: &ActorId) -> Option<&CharacterSheet> {
        self.characters
            .iter()
            .find(|character| character.id == *actor_id)
    }

    /// Compose the production world: all lore NPCs in deterministic lore-path
    /// order, followed by the player records from the compact base seed.
    pub fn with_lore_cast(self, cast: LoreCast) -> Result<Self, SeedError> {
        self.with_lore_cast_knowledge(cast, PlayerKnowledge::PublicFigures)
    }

    pub fn with_lore_cast_knowledge(
        mut self,
        cast: LoreCast,
        knowledge: PlayerKnowledge,
    ) -> Result<Self, SeedError> {
        if let Some(character) = self
            .characters
            .iter()
            .find(|character| character.control != crate::character::Control::Player)
        {
            return Err(SeedError::new(format!(
                "base world seed may only contain player characters; found '{}'",
                character.id
            )));
        }

        let lore_ids: Vec<ActorId> = match knowledge {
            PlayerKnowledge::PublicFigures => cast.public_ids().cloned().collect(),
            PlayerKnowledge::Everyone => cast.ids().cloned().collect(),
        };
        for player in &mut self.characters {
            player.knows.extend(lore_ids.iter().cloned());
        }

        let mut characters = cast.into_character_sheets();
        characters.append(&mut self.characters);
        self.characters = characters;
        self.validate()?;
        Ok(self)
    }
}

/// The world-level knobs a seed does not carry (they come from `config.ron` /
/// the environment, so the host supplies them).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldConfig {
    pub area_map: AreaMap,
    pub sounds_enabled: bool,
    /// Total horizontal FOV of the witness test; clamped to
    /// `[MIN_VIEW_CONE_DEGREES, MAX_VIEW_CONE_DEGREES]` by [`build_world`], which
    /// also replaces a non-finite value with [`DEFAULT_VIEW_CONE_DEGREES`].
    pub view_cone_degrees: f64,
    pub sound_catalog: SoundCatalog,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            area_map: AreaMap::default(),
            sounds_enabled: true,
            view_cone_degrees: DEFAULT_VIEW_CONE_DEGREES,
            sound_catalog: SoundCatalog::empty(),
        }
    }
}

/// Build the seeded world (`main.py:52-139`).
///
/// Items are added first, then characters in file order (that order becomes
/// [`World::roster`], i.e. the turn order). Ends with `assert_invariants`, like
/// Python: a seed that violates them is a programmer error, and
/// [`WorldSeed::from_json_str`] has already rejected every case a data file can
/// express.
pub fn build_world(seed: &WorldSeed, config: WorldConfig) -> World {
    let mut world = World::new();
    world.area_map = config.area_map;
    world.sounds_enabled = config.sounds_enabled;
    world.view_cone_degrees = view_cone_degrees(config.view_cone_degrees);
    world.sound_catalog = config.sound_catalog;

    for item in &seed.items {
        world.add_item(Item::from(item));
    }
    for sheet in &seed.characters {
        world.add_character(Character::from_sheet(sheet.clone()));
    }

    world.assert_invariants();
    world
}

/// `_env_float("SMART_ACTORS_VIEW_CONE_DEGREES", …)` (`server.py:91-101, 449-451`).
///
/// Python discards a non-finite override *before* clamping, so it falls back to
/// the default. `f64::clamp` would pass NaN straight through, and a NaN cone is
/// silent poison: `sees()` compares against `cos(NaN)`, which is false for every
/// observer, so no sound would ever be attributed again.
fn view_cone_degrees(configured: f64) -> f64 {
    if !configured.is_finite() {
        return DEFAULT_VIEW_CONE_DEGREES;
    }
    configured.clamp(MIN_VIEW_CONE_DEGREES, MAX_VIEW_CONE_DEGREES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::Control;

    /// The compact demo cast is retained as a focused domain fixture.
    const SEED_JSON: &str = include_str!("../tests/fixtures/demo_seed.json");

    fn seed() -> WorldSeed {
        WorldSeed::from_json_str(SEED_JSON).expect("the shipped seed must load")
    }

    #[test]
    fn the_roster_is_insertion_order_not_id_order() {
        let world = build_world(&seed(), WorldConfig::default());

        // Round-robin follows this; BTreeMap iteration (cb947, k0fb1, player,
        // sv3n1) would silently reorder the conversation.
        let roster: Vec<&str> = world.roster.iter().map(ActorId::as_str).collect();
        assert_eq!(roster, ["sv3n1", "cb947", "k0fb1", "player"]);

        let llm_order: Vec<&str> = world
            .roster
            .iter()
            .filter(|id| world.characters[*id].control().is_llm())
            .map(ActorId::as_str)
            .collect();
        assert_eq!(llm_order, ["sv3n1", "cb947", "k0fb1"]);
    }

    #[test]
    fn build_world_holds_the_invariants() {
        let world = build_world(&seed(), WorldConfig::default());
        world.assert_invariants();
        assert_eq!(world.characters.len(), 4);
        assert_eq!(world.items.len(), 2);
        assert!(world.offers.is_empty());
        assert_eq!(world.spatial_sequence, -1);
        assert_eq!(world.event_sequence, 0);
    }

    #[test]
    fn the_view_cone_is_clamped_and_the_catalog_is_carried_in() {
        let catalog =
            SoundCatalog::from_toml_str(include_str!("../../../assets/sounds/catalog.toml"))
                .unwrap();
        let world = build_world(
            &seed(),
            WorldConfig {
                area_map: AreaMap::default(),
                sounds_enabled: false,
                view_cone_degrees: 0.0,
                sound_catalog: catalog,
            },
        );
        assert!(!world.sounds_enabled);
        assert_eq!(world.view_cone_degrees, MIN_VIEW_CONE_DEGREES);
        assert!(world.sound_catalog.get("town_bell").is_some());

        let wide = build_world(
            &seed(),
            WorldConfig {
                view_cone_degrees: 720.0,
                ..Default::default()
            },
        );
        assert_eq!(wide.view_cone_degrees, MAX_VIEW_CONE_DEGREES);
    }

    /// `_env_float` discards a non-finite override *before* clamping
    /// (`server.py:99-101`), so NaN and both infinities all fall back to the
    /// default. `f64::clamp` would instead have passed NaN straight through —
    /// and a NaN cone is silent poison: `sees()` compares against `cos(NaN)`,
    /// which is false for every observer, so no sound is ever attributed again.
    #[test]
    fn a_non_finite_view_cone_falls_back_to_the_default() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let world = build_world(
                &seed(),
                WorldConfig {
                    view_cone_degrees: bad,
                    ..Default::default()
                },
            );
            assert_eq!(
                world.view_cone_degrees, DEFAULT_VIEW_CONE_DEGREES,
                "view cone {bad}"
            );
        }
        // A finite out-of-range value still clamps, as before.
        let world = build_world(
            &seed(),
            WorldConfig {
                view_cone_degrees: 1e9,
                ..Default::default()
            },
        );
        assert_eq!(world.view_cone_degrees, MAX_VIEW_CONE_DEGREES);
    }

    #[test]
    fn a_seed_without_a_goal_gets_the_none_sentinel() {
        let world = build_world(&seed(), WorldConfig::default());
        for id in &world.roster {
            // D15: the literal string "None", not null and not an Option.
            assert_eq!(world.characters[id].goal(), "None");
        }
    }

    #[test]
    fn items_default_to_the_generic_visual_key() {
        let seed =
            WorldSeed::from_json_str(r#"{"items": [{"id": "x1", "name": "thing"}]}"#).unwrap();
        assert_eq!(seed.items[0].visual_key, "generic");
        let world = build_world(&seed, WorldConfig::default());
        assert_eq!(world.items[&ItemId::from_raw("x1")].visual_key, "generic");
    }

    #[test]
    fn the_loader_rejects_seeds_that_would_panic_the_world() {
        // Duplicate ids: `World::add_*` treats these as programmer errors.
        let error = WorldSeed::from_json_str(
            r#"{"items": [{"id": "a", "name": "one"}, {"id": "a", "name": "two"}]}"#,
        )
        .unwrap_err();
        assert_eq!(error.message, "duplicate item id 'a'");

        let two_svens = format!(
            r#"{{"characters": [{sheet}, {sheet}]}}"#,
            sheet = character_json("sv3n1", "[]")
        );
        assert_eq!(
            WorldSeed::from_json_str(&two_svens).unwrap_err().message,
            "duplicate character id 'sv3n1'"
        );

        // Invariant violations: a held item must exist, and only once.
        let ghost_item = format!(
            r#"{{"characters": [{}]}}"#,
            character_json("sv3n1", r#"["nope"]"#)
        );
        assert_eq!(
            WorldSeed::from_json_str(&ghost_item).unwrap_err().message,
            "character 'sv3n1' holds missing item 'nope'"
        );

        let two_holders = format!(
            r#"{{"items": [{{"id": "fzbn9", "name": "fish"}}], "characters": [{}, {}]}}"#,
            character_json("sv3n1", r#"["fzbn9"]"#),
            character_json("cb947", r#"["fzbn9"]"#)
        );
        assert_eq!(
            WorldSeed::from_json_str(&two_holders).unwrap_err().message,
            "item 'fzbn9' is held by both 'sv3n1' and 'cb947'"
        );

        // Shape errors the parser itself catches.
        assert!(WorldSeed::from_json_str("{").is_err());
        assert!(WorldSeed::from_json_str(r#"{"itemz": []}"#).is_err());
        // Non-finite coordinates never reach the world (`vec3_serde`).
        let bad_position = character_json("sv3n1", "[]").replace("0.91", "1e400");
        assert!(
            WorldSeed::from_json_str(&format!(r#"{{"characters": [{bad_position}]}}"#)).is_err()
        );
    }

    /// A `knows` entry for someone who is not in the world is tolerated (Python
    /// skips it when rendering), so the loader must not "helpfully" reject it.
    #[test]
    fn a_dangling_knows_entry_is_not_an_error() {
        let dangling = format!(
            r#"{{"characters": [{}]}}"#,
            character_json("sv3n1", "[]").replace(r#""knows": []"#, r#""knows": ["ghost"]"#)
        );
        let seed = WorldSeed::from_json_str(&dangling).unwrap();
        let world = build_world(&seed, WorldConfig::default());
        assert!(
            world.characters[&ActorId::from_raw("sv3n1")]
                .knows()
                .contains(&ActorId::from_raw("ghost"))
        );
    }

    #[test]
    fn the_player_sheet_is_found_by_id() {
        let seed = seed();
        let player = seed.character(&ActorId::from_raw("player")).unwrap();
        assert_eq!(player.control, Control::Player);
        assert_eq!(player.voice_key, None);
        assert!(seed.character(&ActorId::from_raw("nobody")).is_none());
    }

    fn character_json(id: &str, holds: &str) -> String {
        format!(
            r#"{{"id": "{id}", "name": "N", "control": "llm", "back_story": "b",
                 "location_description": "l", "appearance_key": "a", "voice_key": null,
                 "position_m": {{"x": 0.0, "y": 0.91, "z": 0.0}}, "holds": {holds},
                 "knows": []}}"#
        )
    }
}
