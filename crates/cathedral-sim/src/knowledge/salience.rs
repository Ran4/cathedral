//! How far each kind of news travels, and which hedge rung it renders on.
//!
//! `assets/world/salience.json` is the designer's whole tuning surface: nine
//! base bands, the trades that hear each one differently, and the household
//! damping. It is embedded with `include_str!` exactly as `marks.json` is, so
//! both hosts get it with no wiring.
//!
//! M1 reads exactly two things from it — [`SalienceTable::base`] and
//! [`SalienceTable::hedge_band`]. `salience()` itself (the ear, the craft rows,
//! `no_trade`, `household`) is M2's; its numbers are authored here because the
//! asset is authored once and a two-stage file would need a schema migration.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Topic;

const SALIENCE_JSON: &str = include_str!("../../../../assets/world/salience.json");

/// Which rung of the hedge ladder a topic's tellings sit on.
///
/// Authored per topic rather than derived from the base number, so
/// [`SalienceTable::flat`] stays purely arithmetic and cannot silently promote
/// all nine topics to the top band mid-measurement: flattening must move
/// numbers and never move prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HedgeBand {
    Top,
    Default,
    Low,
}

/// Salience data that cannot be used without leaving a topic silently unable to
/// travel.
#[derive(Debug, Clone, PartialEq)]
pub struct SalienceError {
    pub message: String,
}

impl SalienceError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SalienceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SalienceError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SalienceDoc {
    schema_version: u32,
    #[serde(default, rename = "_doc")]
    _doc: Option<String>,
    #[serde(default, rename = "_ear_doc")]
    _ear_doc: Option<String>,
    #[serde(default, rename = "_no_trade_why")]
    _no_trade_why: Option<String>,
    #[serde(default, rename = "_household_why")]
    _household_why: Option<String>,
    topics: BTreeMap<String, RawBand>,
    #[serde(default)]
    ears: BTreeMap<String, RawEar>,
    craft: RawCraft,
    no_trade: f64,
    household: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBand {
    base: f64,
    hedge_band: HedgeBand,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEar {
    #[serde(default, rename = "_why")]
    _why: Option<String>,
    occupations: Vec<String>,
    multiplier: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCraft {
    #[serde(default, rename = "_why")]
    _why: Option<String>,
    own_trade: f64,
    other_trade: f64,
}

/// One topic's authored row: how loud it is, and how its hedges erode.
#[derive(Debug, Clone, PartialEq)]
struct Band {
    base: f64,
    hedge_band: HedgeBand,
}

/// One topic's ear: the occupations that hear it harder, and by how much.
#[derive(Debug, Clone, PartialEq)]
struct Ear {
    occupations: Vec<String>,
    multiplier: f64,
}

/// The designer's whole tuning surface: nine base bands and the trades that
/// hear each one differently.
///
/// A float per fact would be five hundred floats nobody can reason about; a
/// topic is a classification with an external check.
#[derive(Debug, Clone, PartialEq)]
pub struct SalienceTable {
    bands: BTreeMap<Topic, Band>,
    ears: BTreeMap<Topic, Ear>,
    craft_own: f64,
    craft_other: f64,
    no_trade: f64,
    household: f64,
}

impl Default for SalienceTable {
    /// The embedded table. Panics only if the *compiled-in* asset is malformed,
    /// which is a build-time fact — the same bargain `marks.rs` makes.
    fn default() -> Self {
        Self::from_embedded().expect("the embedded salience table must parse and validate")
    }
}

impl SalienceTable {
    pub fn from_embedded() -> Result<Self, SalienceError> {
        Self::from_json(SALIENCE_JSON)
    }

    pub fn from_json(json: &str) -> Result<Self, SalienceError> {
        let doc: SalienceDoc = serde_json::from_str(json)
            .map_err(|error| SalienceError::new(format!("invalid salience.json: {error}")))?;
        if doc.schema_version != 1 {
            return Err(SalienceError::new(format!(
                "unsupported salience schema {}; expected 1",
                doc.schema_version
            )));
        }

        let mut bands = BTreeMap::new();
        for topic in Topic::ALL {
            let Some(row) = doc.topics.get(topic.as_str()) else {
                return Err(SalienceError::new(format!(
                    "salience.json is missing a band for topic '{}' — every one of the nine \
                     must be stated, because an omitted band is a silent 0 and the topic \
                     would never travel",
                    topic.as_str()
                )));
            };
            if !(row.base.is_finite() && (0.0..=4.0).contains(&row.base)) {
                return Err(SalienceError::new(format!(
                    "salience.json: topic '{}' has base {}; expected a finite 0..=4",
                    topic.as_str(),
                    row.base
                )));
            }
            bands.insert(
                topic,
                Band {
                    base: row.base,
                    hedge_band: row.hedge_band,
                },
            );
        }
        // An unknown topic key would otherwise be a band nobody ever reads.
        for key in doc.topics.keys() {
            if Topic::parse(key).is_none() {
                return Err(SalienceError::new(format!(
                    "salience.json names unknown topic '{key}'; expected one of {}",
                    Topic::AUTHORED_LIST
                )));
            }
        }

        let mut ears = BTreeMap::new();
        for (key, raw) in &doc.ears {
            let Some(topic) = Topic::parse(key) else {
                return Err(SalienceError::new(format!(
                    "salience.json names unknown topic '{key}'; expected one of {}",
                    Topic::AUTHORED_LIST
                )));
            };
            if !(raw.multiplier.is_finite() && (0.0..=4.0).contains(&raw.multiplier)) {
                return Err(SalienceError::new(format!(
                    "salience.json: the {key} ear has multiplier {}; expected a finite 0..=4",
                    raw.multiplier
                )));
            }
            if raw.occupations.is_empty() {
                return Err(SalienceError::new(format!(
                    "salience.json: the {key} ear names no occupations — omit the entry \
                     instead, so nobody reads an empty list as \"everyone\""
                )));
            }
            let mut seen = std::collections::BTreeSet::new();
            for occupation in &raw.occupations {
                if !seen.insert(occupation.as_str()) {
                    return Err(SalienceError::new(format!(
                        "salience.json: the {key} ear names '{occupation}' twice"
                    )));
                }
            }
            ears.insert(
                topic,
                Ear {
                    occupations: raw.occupations.clone(),
                    multiplier: raw.multiplier,
                },
            );
        }

        for (label, value) in [
            ("craft.own_trade", doc.craft.own_trade),
            ("craft.other_trade", doc.craft.other_trade),
            ("no_trade", doc.no_trade),
            ("household", doc.household),
        ] {
            if !(value.is_finite() && (0.0..=4.0).contains(&value)) {
                return Err(SalienceError::new(format!(
                    "salience.json: the {label} ear has multiplier {value}; \
                     expected a finite 0..=4"
                )));
            }
        }
        if doc.craft.own_trade <= doc.craft.other_trade {
            return Err(SalienceError::new(format!(
                "salience.json: craft.own_trade {} is not above craft.other_trade {} — the \
                 whole point of the craft rows is that a spoiled batch is everything to that \
                 trade and nothing to any other",
                doc.craft.own_trade, doc.craft.other_trade
            )));
        }
        if doc.household >= 1.0 {
            return Err(SalienceError::new(format!(
                "salience.json: household damping is {}; it must be below 1, or the subject's \
                 own house hears it first instead of last",
                doc.household
            )));
        }

        Ok(Self {
            bands,
            ears,
            craft_own: doc.craft.own_trade,
            craft_other: doc.craft.other_trade,
            no_trade: doc.no_trade,
            household: doc.household,
        })
    }

    /// Every base and every multiplier 1.0, **hedge bands untouched**.
    ///
    /// The identity run: the roll becomes `curiosity × heat` again, which is the
    /// model before salience existed, so M2's pre-salience cadence numbers must
    /// reproduce exactly. Bands are authored per topic rather than derived from
    /// `base` for precisely this reason — deriving one would silently promote
    /// all nine topics to `Top` mid-measurement, and flattening must move
    /// numbers and never move prose.
    ///
    /// It sets `household = 1.0`, which the loader rejects: this constructor
    /// bypasses the loader on purpose, because a *damping* of one is exactly
    /// what "no damping" means and the loader's rule is about authored data.
    pub fn flat() -> Self {
        let shipped = Self::default();
        Self {
            bands: Topic::ALL
                .into_iter()
                .map(|topic| {
                    (
                        topic,
                        Band {
                            base: 1.0,
                            hedge_band: shipped.hedge_band(topic),
                        },
                    )
                })
                .collect(),
            ears: shipped
                .ears
                .iter()
                .map(|(topic, ear)| {
                    (
                        *topic,
                        Ear {
                            occupations: ear.occupations.clone(),
                            multiplier: 1.0,
                        },
                    )
                })
                .collect(),
            craft_own: 1.0,
            craft_other: 1.0,
            no_trade: 1.0,
            household: 1.0,
        }
    }

    /// How loud this kind of news is before any listener is consulted. Every
    /// topic is authored, so a missing one is a load error and never a silent 0.
    pub fn base(&self, topic: Topic) -> f64 {
        self.bands.get(&topic).map_or(0.0, |band| band.base)
    }

    /// Which column of the hedge ladder this topic's tellings render in — a
    /// property of the **fact**, never of the reader's affinity.
    pub fn hedge_band(&self, topic: Topic) -> HedgeBand {
        self.bands
            .get(&topic)
            .map_or(HedgeBand::Default, |band| band.hedge_band)
    }

    /// The occupations that hear this topic harder, and by how much. An empty
    /// list with a multiplier of 1.0 for a topic with no authored ear.
    pub fn ear_of(&self, topic: Topic) -> (&[String], f64) {
        self.ears.get(&topic).map_or((&[], 1.0), |ear| {
            (ear.occupations.as_slice(), ear.multiplier)
        })
    }

    /// A `Craft` matter, to the trade it is about.
    pub fn craft_own(&self) -> f64 {
        self.craft_own
    }

    /// A `Craft` matter, to anybody else.
    pub fn craft_other(&self) -> f64 {
        self.craft_other
    }

    /// The no-trade quarter, who hear everything.
    pub fn no_trade(&self) -> f64 {
        self.no_trade
    }

    /// Anyone behind the subject's own door, or their kin: the last people to
    /// hear a scandal are the ones who live with it.
    pub fn household(&self) -> f64 {
        self.household
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T17. The shipped table authors all nine bands, and the numbers are the
    /// ones `02_rumor_pollen.md` names.
    #[test]
    fn the_embedded_table_authors_all_nine_bands() {
        let table = SalienceTable::default();
        let expected: [(Topic, f64, HedgeBand); 9] = [
            (Topic::Bed, 1.00, HedgeBand::Top),
            (Topic::Blood, 1.00, HedgeBand::Top),
            (Topic::Law, 0.80, HedgeBand::Default),
            (Topic::Omen, 0.80, HedgeBand::Default),
            (Topic::Stranger, 0.80, HedgeBand::Default),
            (Topic::Coin, 0.45, HedgeBand::Default),
            (Topic::Bread, 0.35, HedgeBand::Default),
            (Topic::Craft, 0.20, HedgeBand::Low),
            (Topic::Talk, 0.15, HedgeBand::Low),
        ];
        for (topic, base, band) in expected {
            assert!(
                (table.base(topic) - base).abs() < 1e-12,
                "{topic:?} base {} != {base}",
                table.base(topic)
            );
            assert_eq!(table.hedge_band(topic), band, "{topic:?} band");
        }
        assert_eq!(table.craft_own(), 2.0);
        assert_eq!(table.craft_other(), 0.6);
        assert_eq!(table.no_trade(), 1.4);
        assert_eq!(table.household(), 0.15);
        let (occupations, multiplier) = table.ear_of(Topic::Bed);
        assert!(occupations.contains(&"domestic_servant".to_string()));
        assert_eq!(multiplier, 1.6);
        // Omen, stranger, craft and talk have no authored ear at all.
        assert_eq!(table.ear_of(Topic::Omen), (&[][..], 1.0));
    }

    /// T18. The flat table is arithmetically the identity and moves no prose —
    /// the half of the flat-table identity that is provable before any
    /// measurement exists, and the reason `hedge_band` is authored rather than
    /// derived.
    #[test]
    fn flat_moves_numbers_and_never_prose() {
        let shipped = SalienceTable::default();
        let flat = SalienceTable::flat();
        for topic in Topic::ALL {
            assert_eq!(flat.base(topic), 1.0, "{topic:?} base");
            assert_eq!(
                flat.hedge_band(topic),
                shipped.hedge_band(topic),
                "{topic:?} band moved"
            );
            let (flat_ears, flat_multiplier) = flat.ear_of(topic);
            let (shipped_ears, _) = shipped.ear_of(topic);
            assert_eq!(flat_ears, shipped_ears, "{topic:?} ear membership moved");
            assert_eq!(flat_multiplier, 1.0, "{topic:?} ear multiplier");
        }
        assert_eq!(flat.craft_own(), 1.0);
        assert_eq!(flat.craft_other(), 1.0);
        assert_eq!(flat.no_trade(), 1.0);
        assert_eq!(flat.household(), 1.0);
    }

    fn shipped_doc() -> serde_json::Value {
        serde_json::from_str(SALIENCE_JSON).expect("the shipped table is JSON")
    }

    fn rejection(edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut doc = shipped_doc();
        edit(&mut doc);
        SalienceTable::from_json(&doc.to_string())
            .expect_err("the edited table must be refused")
            .message
    }

    /// T19. One case per row of `03_assets.md` §2's validation table, each
    /// asserting the message names the consequence and not the field.
    #[test]
    fn every_salience_rejection_names_its_consequence() {
        let error = SalienceTable::from_json("{").unwrap_err().message;
        assert!(error.contains("invalid salience.json:"), "{error}");

        let error = rejection(|doc| doc["schema_version"] = serde_json::json!(2));
        assert!(error.contains("unsupported salience schema 2"), "{error}");

        let error = rejection(|doc| {
            doc["topics"]
                .as_object_mut()
                .expect("topics is an object")
                .remove("omen");
        });
        assert!(
            error.contains("is missing a band for topic 'omen'"),
            "{error}"
        );
        assert!(error.contains("an omitted band is a silent 0"), "{error}");

        let error = rejection(|doc| doc["topics"]["bed"]["base"] = serde_json::json!(9.0));
        assert!(
            error.contains("topic 'bed' has base 9; expected a finite 0..=4"),
            "{error}"
        );

        let error = rejection(|doc| doc["ears"]["bed"]["multiplier"] = serde_json::json!(11.0));
        assert!(
            error.contains("the bed ear has multiplier 11; expected a finite 0..=4"),
            "{error}"
        );

        let error = rejection(|doc| doc["ears"]["bed"]["occupations"] = serde_json::json!([]));
        assert!(
            error.contains("the bed ear names no occupations"),
            "{error}"
        );
        assert!(error.contains("as \"everyone\""), "{error}");

        let error = rejection(|doc| {
            doc["ears"]["bed"]["occupations"] = serde_json::json!(["laundress", "laundress"]);
        });
        assert!(
            error.contains("the bed ear names 'laundress' twice"),
            "{error}"
        );

        let error = rejection(|doc| doc["craft"]["own_trade"] = serde_json::json!(0.5));
        assert!(
            error.contains("craft.own_trade 0.5 is not above craft.other_trade 0.6"),
            "{error}"
        );

        let error = rejection(|doc| doc["household"] = serde_json::json!(1.0));
        assert!(error.contains("household damping is 1"), "{error}");
        assert!(error.contains("hears it first instead of last"), "{error}");

        let error = rejection(|doc| {
            doc["ears"]["word"] = doc["ears"]["bed"].clone();
        });
        assert!(error.contains("unknown topic 'word'"), "{error}");
    }
}
