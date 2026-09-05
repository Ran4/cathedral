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

use super::{Fact, Topic};
use crate::character::{Character, Control};
use crate::ids::ActorId;
use crate::knowledge::pollen::Listener;
use crate::world::World;

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

/// `base(topic) × affinity(listener) × household damping` — the pure table
/// product and nothing else.
///
/// **Never decays**: heat answers *is this current*, this answers *is this worth
/// repeating at all*, and what falls out of multiplying them is that a cold
/// scandal out-travels a fresh squabble.
///
/// Not `0.0` for the subject — that is a rule,
/// [`knowledge::may_carry`](crate::knowledge::may_carry), and it sits outside this
/// product so [`SalienceTable::flat`] is exactly a multiplication by one. The
/// player has no lore and is **not** the no-trade quarter: their affinity is 1.0
/// on every topic, so `PLAYER_CURIOSITY` is the whole of their roll.
///
/// Sanitised with `f64::max`/`min` chains and never `clamp`, so a NaN in the table
/// sanitises to the floor instead of propagating through every roll in the city —
/// `clamp` returns NaN for a NaN input, and a NaN salience would poison every roll
/// it reached (`attention.rs`'s own reason for the same allow).
pub fn salience(world: &World, fact: &Fact, listener: &ActorId) -> f64 {
    let (occupation, is_player) = ear_of(world, listener);
    salience_of(&world.salience, fact, listener, occupation, is_player)
}

/// The two things about a listener the table reads — their trade and whether
/// they are the player — off **one** `characters.get`. `World::is_player`'s
/// question, answered off the body already in hand rather than by a second probe
/// of a 20,520-key map; `player_id()` would be a roster scan besides.
fn ear_of<'a>(world: &'a World, listener: &ActorId) -> (Option<&'a str>, bool) {
    let body = world.characters.get(listener);
    (
        body.and_then(Character::lore)
            .and_then(|lore| lore.occupation_id.as_deref()),
        body.is_some_and(|body| body.control() == Control::Player),
    )
}

/// [`salience`] on inputs already resolved — the table product itself, and the
/// one body of it. `poll_person` resolves its listener once per poll
/// ([`Listener`]) and rolls every air row through here, so the innermost roll
/// pays no `characters.get` at all.
#[allow(clippy::manual_clamp)]
pub(crate) fn salience_of(
    table: &SalienceTable,
    fact: &Fact,
    listener: &ActorId,
    occupation: Option<&str>,
    is_player: bool,
) -> f64 {
    let mut value = table.base(fact.topic);

    value *= match (fact.topic, occupation) {
        _ if is_player => 1.0,
        // The subject's own trade hears a spoiled batch; nobody else does.
        // `craft_ear` is `None` when the subject has no trade, and then every
        // *traded* listener is the other trade.
        (Topic::Craft, Some(trade)) if fact.craft_ear.as_deref() == Some(trade) => {
            table.craft_own()
        }
        (Topic::Craft, Some(_)) => table.craft_other(),
        // The no-trade quarter hears everything — on every topic, `Craft` included
        // ("any topic | no occupation at all | 1.4"): they have no round, they
        // loiter where they were stood, and this row is where "the beggars know
        // everything before anyone" lives.
        (_, None) => table.no_trade(),
        (topic, Some(trade)) => {
            let (ears, multiplier) = table.ear_of(topic);
            if ears.iter().any(|ear| ear == trade) {
                multiplier
            } else {
                1.0
            }
        }
    };

    // The last people to hear a scandal are the ones who live with it.
    if fact.quiet_among.contains(listener) {
        value *= table.household();
    }
    value.max(0.0).min(4.0)
}

/// [`salience`], honouring `--pollen-no-salience`.
///
/// A bare `1.0` under the lever: the self-subject rule is `may_carry`, outside this
/// function, so deleting the factor deletes only the table. `--pollen-flat` needs
/// no branch at all — it is a table whose every number is 1.0.
pub(crate) fn salience_for(world: &World, fact: &Fact, who: &ActorId) -> f64 {
    match live_table(world) {
        None => 1.0,
        Some(table) => {
            let (occupation, is_player) = ear_of(world, who);
            salience_of(table, fact, who, occupation, is_player)
        }
    }
}

/// [`salience_for`] for a listener already resolved — the pickup arm's shape.
pub(crate) fn salience_for_listener(world: &World, fact: &Fact, listener: &Listener<'_>) -> f64 {
    match live_table(world) {
        None => 1.0,
        Some(table) => salience_of(
            table,
            fact,
            listener.id,
            listener.occupation,
            listener.is_player,
        ),
    }
}

/// The **one** place `--pollen-no-salience` is read: `None` when the factor is
/// deleted from the expression, so the flat-table identity cannot be broken by a
/// branch that reached the pickup roll and not the deposit gate — both of them,
/// and every other caller, come through here.
fn live_table(world: &World) -> Option<&SalienceTable> {
    if world.pollen_no_salience {
        None
    } else {
        Some(&world.salience)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::{Character, CharacterSheet, Control};
    use crate::knowledge::{
        AIR_HALF_LIFE_GAME_HOURS, FactSource, GarbleMask, HOP_LOSS, Held, VOLUNTEER_HEAT,
        pollen::pickup_chance, volunteers,
    };
    use crate::lore::{LoreProfile, PlanningWard, Significance};
    use crate::math::Vec3;
    use std::collections::BTreeSet;

    fn who(id: &str) -> ActorId {
        ActorId::from_raw(id)
    }

    /// One fact, built straight rather than through the loader: `salience` is a
    /// child of `knowledge`, so the private `source` field is in reach and these
    /// tests are about numbers, not JSON.
    fn fact_of(
        topic: Topic,
        subject: &[&str],
        craft_ear: Option<&str>,
        quiet_among: &[&str],
    ) -> Fact {
        Fact {
            id: crate::ids::FactId::from_raw("test.row"),
            key: crate::ids::FactKey(0),
            sequence: 3,
            subject: subject.iter().map(|id| who(id)).collect(),
            place: None,
            day: None,
            said: "{subject} did a thing".into(),
            own: BTreeMap::new(),
            seeded: BTreeSet::new(),
            garble: GarbleMask::NONE,
            decays: true,
            topic,
            minted_game_days: Some(0.0),
            quiet_among: quiet_among.iter().map(|id| who(id)).collect(),
            craft_ear: craft_ear.map(str::to_string),
            source: FactSource::authored(),
        }
    }

    fn lore_of(occupation: Option<&str>) -> LoreProfile {
        LoreProfile {
            significance: Significance::Minor,
            planning_ward: PlanningWard::Wick,
            age: 30,
            gender: "f".into(),
            occupation_id: occupation.map(str::to_string),
            occupation_display: occupation.map(str::to_string),
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

    fn person(id: &str, occupation: Option<&str>, control: Control) -> Character {
        Character::from_sheet(CharacterSheet {
            pockets: Vec::new(),
            frontbutt: None,
            id: who(id),
            name: id.to_string(),
            control,
            back_story: String::new(),
            location_description: String::new(),
            appearance: Default::default(),
            voice_key: None,
            position_m: Vec3::new(0.0, 0.91, 0.0),
            facing_yaw: 0.0,
            holds: Vec::new(),
            goal: crate::GOAL_NONE.into(),
            memories: Vec::new(),
            knows: BTreeSet::new(),
            lore: Some(lore_of(occupation)),
            presence: crate::character::Presence::InCity,
            presence_epoch: 0,
            economic_class: crate::character::EconomicClass::Resident,
        })
    }

    /// A world with the shipped table and one body per trade this file argues about.
    fn table_world() -> World {
        let mut world = World::new();
        world.add_character(person("subjct", Some("cooper"), Control::Llm));
        world.add_character(person("cooper", Some("cooper"), Control::Llm));
        world.add_character(person("mason1", Some("mason"), Control::Llm));
        world.add_character(person("servnt", Some("domestic_servant"), Control::Llm));
        world.add_character(person("pauper", None, Control::Llm));
        world.add_character(person("player", None, Control::Player));
        world
    }

    /// A holding at `heat`, seated on the clock so a warm life can be read off it.
    fn held_at(heat: f32) -> Held {
        Held::carried(
            0,
            None,
            heat,
            Some(0.0),
            crate::knowledge::FactView::default(),
        )
    }

    /// How long a carrier of this salience goes on saying it, in game hours from the
    /// mint: `τ·log₂(HOP_LOSS^n · s / VH)`. `02_numbers.md` §4's warm-life table.
    fn warm_life(salience: f64, hops: i32) -> f64 {
        let s = f64::from(HOP_LOSS).powi(hops) * salience;
        (AIR_HALF_LIFE_GAME_HOURS * (s / f64::from(VOLUNTEER_HEAT)).log2()).max(0.0)
    }

    /// The subject never carries it as news, under the shipped table, under
    /// `flat()` and with the salience factor deleted from the roll — because the
    /// rule is `may_carry`, outside this product. `salience` itself stays the plain
    /// table product for them, which is what makes the flat-table identity exact.
    #[test]
    fn the_subject_never_carries_under_either_lever() {
        let mut world = table_world();
        let fact = fact_of(Topic::Bed, &["subjct"], None, &[]);
        let subject = who("subjct");
        let held = held_at(1.0);
        for lever in ["shipped", "flat", "no_salience"] {
            match lever {
                "flat" => world.salience = std::sync::Arc::new(SalienceTable::flat()),
                "no_salience" => world.pollen_no_salience = true,
                _ => {}
            }
            assert_eq!(
                pickup_chance(&world, &fact, &subject, 1.0),
                0.0,
                "the subject rolled for their own fact under {lever}"
            );
            assert!(
                !volunteers(&world, &fact, &subject, &held, Some(0.0)),
                "the subject volunteered their own fact under {lever}"
            );
        }
        // And the product itself is untouched: a rule that lived in here would make
        // `flat()` something other than a multiplication by one.
        let mut plain = table_world();
        assert_eq!(salience(&plain, &fact, &subject), 1.00);
        plain.salience = std::sync::Arc::new(SalienceTable::flat());
        assert_eq!(salience(&plain, &fact, &subject), 1.0);
    }

    /// The player has no lore, and that must not read as the no-trade quarter: their
    /// affinity is 1.0 on every topic, so `PLAYER_CURIOSITY` is the whole of their
    /// roll. A lore-less **NPC** is still the pauper's ×1.4.
    #[test]
    fn the_player_is_not_the_no_trade_quarter() {
        let world = table_world();
        let bed = fact_of(Topic::Bed, &["subjct"], None, &[]);
        let craft = fact_of(Topic::Craft, &["subjct"], Some("cooper"), &[]);
        assert_eq!(salience(&world, &bed, &who("player")), 1.00);
        assert!((salience(&world, &craft, &who("player")) - 0.20).abs() < 1e-12);
        assert!((salience(&world, &bed, &who("pauper")) - 1.40).abs() < 1e-12);
        assert!((salience(&world, &craft, &who("pauper")) - 0.28).abs() < 1e-12);
    }

    /// The last people to hear a scandal are the ones who live with it.
    #[test]
    fn the_household_hears_it_quietest() {
        let world = table_world();
        let fact = fact_of(Topic::Bed, &["subjct"], None, &["mason1"]);
        assert!((salience(&world, &fact, &who("mason1")) - 0.15).abs() < 1e-12);
        assert_eq!(salience(&world, &fact, &who("cooper")), 1.00);
        // A housemate at one remove hears it and barely passes it on: 1.19 game
        // hours of warm life against the stranger's 34.
        assert!(
            (warm_life(0.15, 1) - 1.19).abs() < 0.02,
            "{}",
            warm_life(0.15, 1)
        );
    }

    /// The craft rows, which are the arithmetic the slow end is solved from.
    #[test]
    fn a_cooper_hears_a_spoiled_batch_and_nobody_else_does() {
        let world = table_world();
        let fact = fact_of(Topic::Craft, &["subjct"], Some("cooper"), &[]);
        assert!((salience(&world, &fact, &who("cooper")) - 0.40).abs() < 1e-12);
        assert!((salience(&world, &fact, &who("mason1")) - 0.12).abs() < 1e-12);
        assert!((salience(&world, &fact, &who("pauper")) - 0.28).abs() < 1e-12);
        // `craft_ear` is `None` when the subject has no trade, and then every traded
        // listener is the other trade — never the own-trade ×2.0 by accident.
        let tradeless = fact_of(Topic::Craft, &["pauper"], None, &[]);
        assert!((salience(&world, &tradeless, &who("cooper")) - 0.12).abs() < 1e-12);
        // The whole slow end in two numbers: nine game minutes of warm life for the
        // off-trade witness, and none at all at one remove.
        assert!(
            (warm_life(0.12, 0) - 0.145).abs() < 0.005,
            "{}",
            warm_life(0.12, 0)
        );
        assert_eq!(warm_life(0.12, 1), 0.0);
    }

    /// The no-trade quarter hears everything, on all nine topics, and never stacks
    /// with an ear — they have no trade to be in one.
    #[test]
    fn the_no_trade_quarter_hears_everything() {
        let world = table_world();
        let table = SalienceTable::default();
        for topic in Topic::ALL {
            let fact = fact_of(topic, &["subjct"], Some("cooper"), &[]);
            let expected = table.base(topic) * table.no_trade();
            assert!(
                (salience(&world, &fact, &who("pauper")) - expected).abs() < 1e-12,
                "{topic:?} reaches the pauper at {} and not {expected}",
                salience(&world, &fact, &who("pauper"))
            );
            // The ×1.6 `Bed` ear is occupation-based, so nobody gets both it and the
            // ×1.4: the servant is in the ear and is not of the no-trade quarter.
            let servant = salience(&world, &fact, &who("servnt"));
            let (ears, multiplier) = table.ear_of(topic);
            let eared = ears.iter().any(|ear| ear == "domestic_servant");
            let expected = table.base(topic)
                * match topic {
                    Topic::Craft => table.craft_other(),
                    _ if eared => multiplier,
                    _ => 1.0,
                };
            assert!(
                (servant - expected).abs() < 1e-12,
                "{topic:?} reaches the servant at {servant} and not {expected}"
            );
        }
    }

    /// **A cold scandal out-travels a fresh squabble** — the single assertion that
    /// salience is not heat, and it holds with margin rather than by a whisker.
    #[test]
    fn a_cold_scandal_out_travels_a_fresh_squabble() {
        let world = table_world();
        let bed = fact_of(Topic::Bed, &["subjct"], None, &[]);
        let craft = fact_of(Topic::Craft, &["subjct"], Some("cooper"), &[]);
        let cold_scandal = 0.30 * salience(&world, &bed, &who("mason1"));
        let fresh_squabble = 1.00 * salience(&world, &craft, &who("mason1"));
        assert!((cold_scandal - 0.300).abs() < 1e-12);
        assert!((fresh_squabble - 0.120).abs() < 1e-12);
        assert!(
            cold_scandal > fresh_squabble * 2.4,
            "{cold_scandal} against {fresh_squabble} is not the 2.5x the band rests on"
        );
        // And on warm life it is over a hundred times: 16.0 game hours of repeating
        // left against nine game minutes.
        assert!(
            (warm_life(1.00, 0) - 36.9).abs() < 0.1,
            "{}",
            warm_life(1.00, 0)
        );
        assert!(warm_life(0.30, 0) > warm_life(0.12, 0) * 100.0);
    }

    /// Flattening the table is exactly a multiplication by one — the product, not
    /// only the accessors (T18's half) — and it moves no prose.
    #[test]
    fn a_flat_table_is_a_multiplication_by_one() {
        let mut world = table_world();
        let shipped = SalienceTable::default();
        world.salience = std::sync::Arc::new(SalienceTable::flat());
        for topic in Topic::ALL {
            let fact = fact_of(topic, &["subjct"], Some("cooper"), &["mason1"]);
            for listener in ["cooper", "mason1", "servnt", "pauper", "player"] {
                assert_eq!(
                    salience(&world, &fact, &who(listener)),
                    1.0,
                    "{topic:?} × {listener} is not one under a flat table"
                );
            }
            assert_eq!(world.salience.hedge_band(topic), shipped.hedge_band(topic));
        }
    }

    /// A NaN in the table sanitises to the floor instead of propagating through
    /// every roll in the city — `f64::max`/`min` chains, never `clamp`.
    #[test]
    fn a_nan_in_the_table_sanitises_rather_than_propagates() {
        let mut world = table_world();
        let poisoned = SalienceTable {
            bands: Topic::ALL
                .into_iter()
                .map(|topic| {
                    (
                        topic,
                        Band {
                            base: f64::NAN,
                            hedge_band: HedgeBand::Default,
                        },
                    )
                })
                .collect(),
            ears: BTreeMap::new(),
            craft_own: 2.0,
            craft_other: 0.6,
            no_trade: 1.4,
            household: 0.15,
        };
        world.salience = std::sync::Arc::new(poisoned);
        let fact = fact_of(Topic::Bed, &["subjct"], None, &[]);
        let value = salience(&world, &fact, &who("cooper"));
        assert!(
            value.is_finite() && value == 0.0,
            "a NaN band must sanitise to the floor, got {value}"
        );
    }

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
