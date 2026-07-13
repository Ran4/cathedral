//! The sound catalog (`sounds.py`): one source of truth for the sim (percepts
//! and events), the asset generator (`sfx_prompt`), and Bevy playback (which
//! resolves `assets/sounds/{sound_id}.mp3` from the id alone).
//!
//! The rows themselves are data (`assets/sounds/catalog.toml`, D18) — the
//! percept texts a sound produces are catalog strings, not Rust literals; the
//! sim only substitutes `{actor}`. The host reads the file and hands the text
//! to [`SoundCatalog::from_toml_str`], which validates every row: the crate
//! never touches the filesystem.

use std::fmt;

use serde::Deserialize;

/// A catalog row that fails validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundCatalogError {
    pub message: String,
}

impl SoundCatalogError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SoundCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SoundCatalogError {}

/// `^[a-z_]+$` — the id is also the asset basename, so it stays filesystem-
/// and wire-safe.
pub fn is_valid_sound_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_lowercase() || character == '_')
}

/// One emittable/audible sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    /// `[a-z_]+` — also the asset basename.
    pub sound_id: String,
    /// `body` | `impact` | `bell`; becomes the event's `kind`.
    pub sound_class: String,
    pub audible_distance: f64,
    /// Unattributed percept — everyone in radius.
    pub heard: String,
    /// Attributed percept template; `None` => never attributable. When present
    /// it must contain the literal `{actor}` placeholder.
    pub seen: Option<String>,
    /// Generator-only (ElevenLabs) prompt.
    pub sfx_prompt: String,
    pub duration_seconds: f64,
    /// May an LLM choose this via `make_sound`?
    pub actor_emittable: bool,
}

impl Sound {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sound_id: impl Into<String>,
        sound_class: impl Into<String>,
        audible_distance: f64,
        heard: impl Into<String>,
        seen: Option<String>,
        sfx_prompt: impl Into<String>,
        duration_seconds: f64,
        actor_emittable: bool,
    ) -> Result<Self, SoundCatalogError> {
        let sound = Self {
            sound_id: sound_id.into(),
            sound_class: sound_class.into(),
            audible_distance,
            heard: heard.into(),
            seen,
            sfx_prompt: sfx_prompt.into(),
            duration_seconds,
            actor_emittable,
        };
        sound.validate()?;
        Ok(sound)
    }

    fn validate(&self) -> Result<(), SoundCatalogError> {
        let id = &self.sound_id;
        if !is_valid_sound_id(id) {
            return Err(SoundCatalogError::new(format!("invalid sound id '{id}'")));
        }
        if let Some(seen) = &self.seen
            && !seen.contains("{actor}")
        {
            return Err(SoundCatalogError::new(format!(
                "attributable sound '{id}' needs {{actor}}"
            )));
        }
        if !(self.audible_distance.is_finite() && self.audible_distance > 0.0) {
            return Err(SoundCatalogError::new(format!(
                "sound '{id}' needs a positive audible_distance"
            )));
        }
        if !(self.duration_seconds.is_finite() && self.duration_seconds > 0.0) {
            return Err(SoundCatalogError::new(format!(
                "sound '{id}' needs a positive duration_seconds"
            )));
        }
        if self.sfx_prompt.is_empty() {
            return Err(SoundCatalogError::new(format!(
                "sound '{id}' needs an sfx_prompt"
            )));
        }
        if self.heard.is_empty() {
            return Err(SoundCatalogError::new(format!(
                "sound '{id}' needs a heard percept"
            )));
        }
        Ok(())
    }
}

/// A looping scene asset. Generated from this table, never simulated: ambient
/// loops are plain Bevy audio players and emit no events.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientSound {
    pub sound_id: String,
    pub sfx_prompt: String,
    pub duration_seconds: f64,
}

impl AmbientSound {
    pub fn new(
        sound_id: impl Into<String>,
        sfx_prompt: impl Into<String>,
        duration_seconds: f64,
    ) -> Result<Self, SoundCatalogError> {
        let ambient = Self {
            sound_id: sound_id.into(),
            sfx_prompt: sfx_prompt.into(),
            duration_seconds,
        };
        if !is_valid_sound_id(&ambient.sound_id) {
            return Err(SoundCatalogError::new(format!(
                "invalid ambient sound id '{}'",
                ambient.sound_id
            )));
        }
        if !(ambient.duration_seconds.is_finite() && ambient.duration_seconds > 0.0) {
            return Err(SoundCatalogError::new(format!(
                "ambient '{}' needs a positive duration_seconds",
                ambient.sound_id
            )));
        }
        if ambient.sfx_prompt.is_empty() {
            return Err(SoundCatalogError::new(format!(
                "ambient '{}' needs an sfx_prompt",
                ambient.sound_id
            )));
        }
        Ok(ambient)
    }
}

/// The catalog, in definition order — `emittable_sound_ids` (which the prompt
/// renders) is order-sensitive.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundCatalog {
    sounds: Vec<Sound>,
    ambients: Vec<AmbientSound>,
}

impl SoundCatalog {
    /// Validating constructor: rows must already be valid (they are built
    /// through [`Sound::new`]); this additionally rejects duplicate ids.
    pub fn new(sounds: Vec<Sound>, ambients: Vec<AmbientSound>) -> Result<Self, SoundCatalogError> {
        let catalog = Self { sounds, ambients };
        for (index, sound) in catalog.sounds.iter().enumerate() {
            sound.validate()?;
            if catalog.sounds[..index]
                .iter()
                .any(|other| other.sound_id == sound.sound_id)
            {
                return Err(SoundCatalogError::new(format!(
                    "duplicate sound id '{}'",
                    sound.sound_id
                )));
            }
        }
        for (index, ambient) in catalog.ambients.iter().enumerate() {
            if catalog.ambients[..index]
                .iter()
                .any(|other| other.sound_id == ambient.sound_id)
            {
                return Err(SoundCatalogError::new(format!(
                    "duplicate ambient sound id '{}'",
                    ambient.sound_id
                )));
            }
        }
        Ok(catalog)
    }

    /// Parse and validate `assets/sounds/catalog.toml` (D18). Every row goes
    /// through the same checks as [`Sound::new`] / [`AmbientSound::new`], and
    /// definition order is preserved.
    pub fn from_toml_str(source: &str) -> Result<Self, SoundCatalogError> {
        let file: CatalogFile = toml::from_str(source)
            .map_err(|error| SoundCatalogError::new(format!("invalid sound catalog: {error}")))?;
        let sounds = file
            .sounds
            .into_iter()
            .map(|row| {
                Sound::new(
                    row.sound_id,
                    row.sound_class,
                    row.audible_distance,
                    row.heard,
                    row.seen,
                    row.sfx_prompt,
                    row.duration_seconds,
                    row.actor_emittable,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ambients = file
            .ambients
            .into_iter()
            .map(|row| AmbientSound::new(row.sound_id, row.sfx_prompt, row.duration_seconds))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(sounds, ambients)
    }

    /// A world with no sounds at all. `make_sound` then reports every id as
    /// unknown, which is exactly what an unseeded world should do.
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, sound_id: &str) -> Option<&Sound> {
        self.sounds.iter().find(|sound| sound.sound_id == sound_id)
    }

    /// The sounds an LLM may choose, in catalog order (the prompt lists them).
    pub fn emittable_sound_ids(&self) -> Vec<&str> {
        self.sounds
            .iter()
            .filter(|sound| sound.actor_emittable)
            .map(|sound| sound.sound_id.as_str())
            .collect()
    }

    pub fn sounds(&self) -> &[Sound] {
        &self.sounds
    }

    pub fn ambients(&self) -> &[AmbientSound] {
        &self.ambients
    }
}

/// The wire shape of `catalog.toml`. Rows are `[[sounds]]` / `[[ambients]]`
/// array-of-table entries, so the file's order is the catalog's order.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    #[serde(default)]
    sounds: Vec<SoundRow>,
    #[serde(default)]
    ambients: Vec<AmbientRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SoundRow {
    sound_id: String,
    sound_class: String,
    audible_distance: f64,
    heard: String,
    /// Omitted for a sound that is never attributable (the town bell).
    #[serde(default)]
    seen: Option<String>,
    sfx_prompt: String,
    duration_seconds: f64,
    actor_emittable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbientRow {
    sound_id: String,
    sfx_prompt: String,
    duration_seconds: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped catalog, compiled in so the loader is tested against the
    /// real rows. (Only the tests do this — the crate itself reads no files.)
    const CATALOG_TOML: &str = include_str!("../../../assets/sounds/catalog.toml");

    #[test]
    fn rows_are_validated() {
        assert!(
            Sound::new(
                "fart",
                "body",
                20.0,
                "[heard]",
                Some("{actor} farted.".into()),
                "p",
                1.5,
                true
            )
            .is_ok()
        );
        // An attributable sound without the placeholder is a catalog bug.
        assert!(
            Sound::new(
                "fart",
                "body",
                20.0,
                "[heard]",
                Some("farted.".into()),
                "p",
                1.5,
                true
            )
            .is_err()
        );
        assert!(Sound::new("Fart", "body", 20.0, "[heard]", None, "p", 1.5, true).is_err());
        assert!(Sound::new("fart", "body", 0.0, "[heard]", None, "p", 1.5, true).is_err());
        assert!(Sound::new("fart", "body", 20.0, "", None, "p", 1.5, true).is_err());
    }

    #[test]
    fn emittable_ids_keep_catalog_order_and_exclude_the_bell() {
        let catalog = SoundCatalog::new(
            vec![
                Sound::new(
                    "fart",
                    "body",
                    20.0,
                    "[f]",
                    Some("{actor} farted.".into()),
                    "p",
                    1.5,
                    true,
                )
                .unwrap(),
                Sound::new(
                    "glass_break",
                    "impact",
                    25.0,
                    "[g]",
                    Some("{actor} broke it.".into()),
                    "p",
                    2.0,
                    true,
                )
                .unwrap(),
                Sound::new("town_bell", "bell", 600.0, "[b]", None, "p", 9.0, false).unwrap(),
            ],
            vec![],
        )
        .unwrap();
        assert_eq!(catalog.emittable_sound_ids(), ["fart", "glass_break"]);
        assert!(catalog.get("town_bell").is_some());
        assert!(catalog.get("burp").is_none());
    }

    /// Test 28: every shipped row is wire- and filesystem-safe, and the percept
    /// texts the sim renders come from the file, not from Rust.
    #[test]
    fn the_shipped_catalog_loads_with_every_row_intact() {
        let catalog = SoundCatalog::from_toml_str(CATALOG_TOML).unwrap();

        let ids: Vec<&str> = catalog
            .sounds()
            .iter()
            .map(|s| s.sound_id.as_str())
            .collect();
        assert_eq!(ids, ["fart", "glass_break", "town_bell"]);
        for sound in catalog.sounds() {
            assert!(is_valid_sound_id(&sound.sound_id));
            assert!(sound.audible_distance > 0.0);
            assert!(sound.duration_seconds > 0.0);
            assert!(!sound.sfx_prompt.is_empty());
            assert!(!sound.heard.is_empty());
        }

        let fart = catalog.get("fart").unwrap();
        assert_eq!(fart.sound_class, "body");
        assert_eq!(fart.audible_distance, 20.0);
        assert_eq!(fart.duration_seconds, 1.5);
        assert_eq!(fart.heard, "[You heard a big fart!]");
        assert_eq!(fart.seen.as_deref(), Some("{actor} farted."));
        assert!(fart.actor_emittable);

        let glass = catalog.get("glass_break").unwrap();
        assert_eq!(glass.sound_class, "impact");
        assert_eq!(glass.audible_distance, 25.0);
        assert_eq!(glass.duration_seconds, 2.0);
        assert_eq!(glass.heard, "[You heard glass shatter nearby.]");
        assert_eq!(glass.seen.as_deref(), Some("{actor} broke a beer glass."));

        // Test 29: the bell is a world sound — never emittable, never attributed.
        let bell = catalog.get("town_bell").unwrap();
        assert_eq!(bell.sound_class, "bell");
        assert_eq!(bell.audible_distance, 600.0);
        assert_eq!(bell.duration_seconds, 9.0);
        assert_eq!(bell.heard, "[The town bell is ringing.]");
        assert_eq!(bell.seen, None);
        assert!(!bell.actor_emittable);
        assert_eq!(catalog.emittable_sound_ids(), ["fart", "glass_break"]);

        // The sim never simulates ambients; the generator still needs the row.
        assert_eq!(catalog.ambients().len(), 1);
        let fireplace = &catalog.ambients()[0];
        assert_eq!(fireplace.sound_id, "fireplace");
        assert_eq!(fireplace.duration_seconds, 12.0);
        assert!(
            fireplace
                .sfx_prompt
                .starts_with("A steady crackling fireplace")
        );
    }

    /// The sfx_prompts are the generator's only input — a truncated or
    /// re-wrapped prompt would silently resynthesize a different asset.
    #[test]
    fn sfx_prompts_survive_the_move_to_the_data_file_verbatim() {
        let catalog = SoundCatalog::from_toml_str(CATALOG_TOML).unwrap();
        assert_eq!(
            catalog.get("fart").unwrap().sfx_prompt,
            "A single loud comedic wet fart, one short burst, close and dry, \
             no music, no voices, no reverb"
        );
        assert_eq!(
            catalog.get("glass_break").unwrap().sfx_prompt,
            "A single beer glass shattering on a stone floor, sharp impact and \
             scattering shards, medieval tavern room tone, no voices"
        );
        assert_eq!(
            catalog.get("town_bell").unwrap().sfx_prompt,
            "A massive bronze cathedral bell tolling three slow strokes, deep \
             and resonant, carrying over a medieval city, long natural decay, \
             no music"
        );
        assert_eq!(
            catalog.ambients()[0].sfx_prompt,
            "A steady crackling fireplace, gentle wood pops over a warm even \
             room tone, constant level, seamless loop, no voices, no music"
        );
    }

    /// One valid `[[sounds]]` row whose fields are raw TOML fragments, so a
    /// test can replace exactly one of them (including with an empty string or
    /// `nan`) instead of appending a duplicate key the parser would reject
    /// before validation ever ran.
    struct Row {
        sound_id: &'static str,
        sound_class: &'static str,
        audible_distance: &'static str,
        actor_emittable: &'static str,
        seen: Option<&'static str>,
        heard: &'static str,
        sfx_prompt: &'static str,
        duration_seconds: &'static str,
    }

    impl Default for Row {
        fn default() -> Self {
            Self {
                sound_id: r#""fart""#,
                sound_class: r#""body""#,
                audible_distance: "20.0",
                actor_emittable: "true",
                seen: Some(r#""{actor} farted.""#),
                heard: r#""[You heard a big fart!]""#,
                sfx_prompt: r#""a fart""#,
                duration_seconds: "1.5",
            }
        }
    }

    impl Row {
        fn load(&self) -> Result<SoundCatalog, SoundCatalogError> {
            let mut source = String::from("[[sounds]]\n");
            source.push_str(&format!("sound_id = {}\n", self.sound_id));
            source.push_str(&format!("sound_class = {}\n", self.sound_class));
            source.push_str(&format!("audible_distance = {}\n", self.audible_distance));
            source.push_str(&format!("actor_emittable = {}\n", self.actor_emittable));
            if let Some(seen) = self.seen {
                source.push_str(&format!("seen = {seen}\n"));
            }
            source.push_str(&format!("heard = {}\n", self.heard));
            source.push_str(&format!("sfx_prompt = {}\n", self.sfx_prompt));
            source.push_str(&format!("duration_seconds = {}\n", self.duration_seconds));
            SoundCatalog::from_toml_str(&source)
        }
    }

    #[test]
    fn the_loader_rejects_every_malformed_row() {
        assert!(Row::default().load().is_ok());

        // The id is the asset basename: `^[a-z_]+$`, nothing else.
        for bad_id in [
            r#""Fart""#,
            r#""fart2""#,
            r#""fart-2""#,
            r#""fart.mp3""#,
            r#""""#,
            r#""../fart""#,
            r#""fart ""#,
        ] {
            let error = Row {
                sound_id: bad_id,
                ..Default::default()
            }
            .load()
            .unwrap_err();
            assert!(
                error.message.starts_with("invalid sound id"),
                "unexpected error for id {bad_id}: {error}"
            );
        }

        // An attributable percept with no placeholder would name nobody.
        let error = Row {
            seen: Some(r#""somebody farted.""#),
            ..Default::default()
        }
        .load()
        .unwrap_err();
        assert_eq!(error.message, "attributable sound 'fart' needs {actor}");

        // Non-positive (or non-finite) distance: a sound nobody can ever hear.
        for bad in ["0.0", "-1.0", "nan", "inf"] {
            let error = Row {
                audible_distance: bad,
                ..Default::default()
            }
            .load()
            .unwrap_err();
            assert_eq!(
                error.message, "sound 'fart' needs a positive audible_distance",
                "accepted audible_distance {bad}"
            );
        }

        for bad in ["0.0", "-0.5", "nan", "inf"] {
            let error = Row {
                duration_seconds: bad,
                ..Default::default()
            }
            .load()
            .unwrap_err();
            assert_eq!(
                error.message, "sound 'fart' needs a positive duration_seconds",
                "accepted duration_seconds {bad}"
            );
        }

        // Empty generator prompt / percept: a row nobody can hear or synthesize.
        assert_eq!(
            Row {
                sfx_prompt: r#""""#,
                ..Default::default()
            }
            .load()
            .unwrap_err()
            .message,
            "sound 'fart' needs an sfx_prompt"
        );
        assert_eq!(
            Row {
                heard: r#""""#,
                ..Default::default()
            }
            .load()
            .unwrap_err()
            .message,
            "sound 'fart' needs a heard percept"
        );

        // Omitting `seen` is how a sound declares itself unattributable.
        let catalog = Row {
            seen: None,
            ..Default::default()
        }
        .load()
        .unwrap();
        assert_eq!(catalog.get("fart").unwrap().seen, None);

        // A missing field or a typo must not silently produce a default row.
        assert!(SoundCatalog::from_toml_str("[[sounds]]\nsound_id = \"fart\"\n").is_err());
        assert!(SoundCatalog::from_toml_str("[[noises]]\n").is_err());
        let typo = "[[sounds]]\nsound_id = \"fart\"\nsound_class = \"body\"\n\
             audible_distance = 20.0\nactor_emitable = true\nheard = \"[f]\"\n\
             sfx_prompt = \"p\"\nduration_seconds = 1.5\n";
        assert!(SoundCatalog::from_toml_str(typo).is_err());
    }

    #[test]
    fn the_loader_rejects_duplicate_and_malformed_ambients() {
        let duplicate = r#"
            [[sounds]]
            sound_id = "fart"
            sound_class = "body"
            audible_distance = 20.0
            actor_emittable = true
            heard = "[a fart]"
            sfx_prompt = "a fart"
            duration_seconds = 1.5

            [[sounds]]
            sound_id = "fart"
            sound_class = "body"
            audible_distance = 25.0
            actor_emittable = true
            heard = "[another fart]"
            sfx_prompt = "another fart"
            duration_seconds = 1.5
        "#;
        assert_eq!(
            SoundCatalog::from_toml_str(duplicate).unwrap_err().message,
            "duplicate sound id 'fart'"
        );

        let bad_ambient = r#"
            [[ambients]]
            sound_id = "Fireplace"
            sfx_prompt = "crackle"
            duration_seconds = 12.0
        "#;
        assert_eq!(
            SoundCatalog::from_toml_str(bad_ambient)
                .unwrap_err()
                .message,
            "invalid ambient sound id 'Fireplace'"
        );

        let zero_length = r#"
            [[ambients]]
            sound_id = "fireplace"
            sfx_prompt = "crackle"
            duration_seconds = 0.0
        "#;
        assert_eq!(
            SoundCatalog::from_toml_str(zero_length)
                .unwrap_err()
                .message,
            "ambient 'fireplace' needs a positive duration_seconds"
        );
    }

    /// An empty catalog is a legal (sound-less) world, not a load error.
    #[test]
    fn an_empty_catalog_is_valid_and_emits_nothing() {
        let catalog = SoundCatalog::from_toml_str("").unwrap();
        assert_eq!(catalog, SoundCatalog::empty());
        assert!(catalog.emittable_sound_ids().is_empty());
    }
}
